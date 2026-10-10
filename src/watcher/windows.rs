use crate::{
    frontends::cli::Filter,
    i18n::Language,
    model::{
        Access, AccessEvent, Action, Activity, CollectorState, EnforcementDecision, Evidence,
        EvidenceKind, SCHEMA_VERSION, Snapshot, event_code,
    },
    output,
    platform::{MicrophoneProtectionStatus, PlatformMonitor, SessionLockState},
    privacy::{self, CameraControlObservation},
};
use anyhow::{Context, Result};
use chrono::Utc;
use colored::Colorize;
use std::{
    collections::{HashMap, HashSet},
    fs::OpenOptions,
    io::{BufWriter, Write},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT},
        System::{
            EventLog::{
                DeregisterEventSource, EVENTLOG_INFORMATION_TYPE, EVENTLOG_WARNING_TYPE,
                RegisterEventSourceW, ReportEventW,
            },
            Registry::{
                HKEY, REG_NOTIFY_CHANGE_LAST_SET, REG_NOTIFY_CHANGE_NAME, RegNotifyChangeKeyValue,
            },
            Threading::{CreateEventW, WaitForSingleObject},
        },
    },
    core::PCWSTR,
};

#[allow(clippy::too_many_arguments)]
pub fn watch(
    monitor: &PlatformMonitor,
    filter: &Filter,
    json: bool,
    interval: Duration,
    notify: bool,
    log_path: Option<&Path>,
    eventlog: bool,
    lang: Language,
    sound: bool,
    defensive_kill: bool,
    history_enabled: bool,
) -> Result<()> {
    let running = Arc::new(AtomicBool::new(true));
    let signal = Arc::clone(&running);
    ctrlc::set_handler(move || signal.store(false, Ordering::SeqCst))
        .context("failed to install Ctrl+C handler")?;

    // Session/registry callbacks request a rescan, not an event delivery. One
    // pending wake preserves that intent without retaining a callback burst.
    let (wake_sender, wake_receiver) = mpsc::sync_channel(1);
    let _audio_notifications = match monitor.register_audio_notifications(wake_sender.clone()) {
        Ok(guard) => Some(guard),
        Err(error) => {
            eprintln!("audio notifications unavailable; polling remains active: {error:#}");
            None
        }
    };
    let _registry_watcher = spawn_registry_watcher(wake_sender, Arc::clone(&running));
    let mut log_writer = log_path
        .map(|path| {
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .map(BufWriter::new)
        })
        .transpose()
        .context("failed to open JSONL log file")?;
    let min_risk = filter.risk;
    let event_source = if eventlog {
        let source: Vec<u16> = "miccamwatch".encode_utf16().chain(Some(0)).collect();
        Some(unsafe { RegisterEventSourceW(None, PCWSTR(source.as_ptr()))? })
    } else {
        None
    };

    let mut protection_health = ProtectionHealthReporter::new();
    protection_health.refresh(monitor, lang);
    let mut previous = snapshot_by_key(monitor, filter)?;
    let mut last_notifications = NotificationCooldowns::default();
    let mut denial_observations = HashMap::new();
    let mut terminated = HashSet::new();

    for access in previous.values() {
        emit_event(
            access,
            Action::Start,
            json,
            min_risk,
            lang,
            notify,
            sound,
            &mut last_notifications,
            &mut log_writer,
            &event_source,
            history_enabled,
        )?;
    }
    update_enforcement_candidates(
        previous.values(),
        defensive_kill,
        &mut denial_observations,
        &mut terminated,
    );

    while running.load(Ordering::SeqCst) {
        match wake_receiver.recv_timeout(interval) {
            Ok(()) | Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        last_notifications.prune(Instant::now());
        protection_health.refresh(monitor, lang);
        let current = snapshot_by_key(monitor, filter)?;

        for (key, access) in &current {
            if !previous.contains_key(key) {
                emit_event(
                    access,
                    Action::Start,
                    json,
                    min_risk,
                    lang,
                    notify,
                    sound,
                    &mut last_notifications,
                    &mut log_writer,
                    &event_source,
                    history_enabled,
                )?;
            }
        }
        for (key, access) in &current {
            if let Some(old) = previous.get(key)
                && access_changed(old, access)
            {
                emit_event(
                    access,
                    Action::Update,
                    json,
                    min_risk,
                    lang,
                    notify,
                    false,
                    &mut last_notifications,
                    &mut log_writer,
                    &event_source,
                    history_enabled,
                )?;
            }
        }
        for (key, access) in &previous {
            if !current.contains_key(key) {
                emit_event(
                    access,
                    Action::Stop,
                    json,
                    min_risk,
                    lang,
                    notify,
                    false,
                    &mut last_notifications,
                    &mut log_writer,
                    &event_source,
                    history_enabled,
                )?;
            }
        }

        update_enforcement_candidates(
            current.values(),
            defensive_kill,
            &mut denial_observations,
            &mut terminated,
        );
        denial_observations.retain(|(key, instance), _| {
            current
                .get(key)
                .and_then(|access| access.process.as_ref())
                .is_some_and(|process| &process.instance_id == instance)
        });
        terminated.retain(|(key, instance)| {
            current
                .get(key)
                .and_then(|access| access.process.as_ref())
                .is_some_and(|process| &process.instance_id == instance)
        });
        previous = current;
    }

    if let Some(handle) = event_source {
        let _ = unsafe { DeregisterEventSource(handle) };
    }
    Ok(())
}

// Protection health is separate from capture events. Polling is read-only,
// rate-limited, and emits only meaningful changes; correction counters would
// otherwise turn an application's repeated unmute into unbounded stderr noise.
struct ProtectionHealthReporter {
    next_poll: Instant,
    microphone: Option<MicrophoneProtectionStatus>,
    microphone_error: Option<String>,
    camera: Option<CameraControlObservation>,
    camera_error: Option<String>,
}

impl ProtectionHealthReporter {
    fn new() -> Self {
        Self {
            next_poll: Instant::now(),
            microphone: None,
            microphone_error: None,
            camera: None,
            camera_error: None,
        }
    }

    fn refresh(&mut self, monitor: &PlatformMonitor, lang: Language) {
        let now = Instant::now();
        if now < self.next_poll {
            return;
        }
        self.next_poll = now + Duration::from_secs(2);
        match monitor.microphone_protection_status() {
            Ok(status) => {
                let changed = self.microphone.as_ref().is_none_or(|old| {
                    old.requested != status.requested
                        || old.service_active != status.service_active
                        || old.mute_state != status.mute_state
                        || old.endpoint_count != status.endpoint_count
                        || old.hardware_mute_count != status.hardware_mute_count
                        || old.detail != status.detail
                });
                if changed || self.microphone_error.is_some() {
                    eprintln!("{}", output::microphone_protection_summary(lang, &status));
                }
                self.microphone = Some(status);
                self.microphone_error = None;
            }
            Err(error) => {
                let detail = format!("{error:#}");
                if self.microphone_error.as_ref() != Some(&detail) {
                    eprintln!("{}: {detail}", lang.unknown_protection(true));
                }
                self.microphone = None;
                self.microphone_error = Some(detail);
            }
        }
        match privacy::camera_observation() {
            Ok(status) => {
                let changed = self.camera.as_ref().is_none_or(|old| {
                    old.desired_blocked != status.desired_blocked
                        || old.state != status.state
                        || old.present_total != status.present_total
                        || old.owned_blocked_present != status.owned_blocked_present
                        || old.pending_restore != status.pending_restore
                        || old.absent_owned != status.absent_owned
                        || old.unknown_devices != status.unknown_devices
                        || old.helper_active != status.helper_active
                        || old.detail != status.detail
                });
                if changed || self.camera_error.is_some() {
                    eprintln!("{}", output::camera_protection_summary(lang, &status));
                }
                self.camera = Some(status);
                self.camera_error = None;
            }
            Err(error) => {
                let detail = format!("{error:#}");
                if self.camera_error.as_ref() != Some(&detail) {
                    eprintln!("{}: {detail}", lang.unknown_protection(false));
                }
                self.camera = None;
                self.camera_error = Some(detail);
            }
        }
    }
}

fn snapshot_by_key(monitor: &PlatformMonitor, filter: &Filter) -> Result<HashMap<String, Access>> {
    let snapshot = monitor.snapshot(filter.into())?;
    ensure_collectors_available(&snapshot)?;
    let mut accesses = snapshot.accesses;
    if crate::platform::session_lock_state() == SessionLockState::Locked {
        for access in &mut accesses {
            if access.activity == Activity::Active {
                access.evidence.push(Evidence::new(
                    EvidenceKind::PrivacyActivity,
                    "session_lock",
                    "Capture was observed while the Windows session was locked.",
                ));
                if access.risk < crate::model::Risk::Suspicious {
                    access.risk = crate::model::Risk::Suspicious;
                }
            }
        }
    }
    Ok(by_key(accesses))
}

fn ensure_collectors_available(snapshot: &Snapshot) -> Result<()> {
    if let Some(failed) = snapshot
        .collectors
        .iter()
        .find(|collector| collector.state == CollectorState::Unavailable)
    {
        anyhow::bail!(
            "{} collector unavailable: {}",
            failed.collector,
            failed.detail.as_deref().unwrap_or("unknown error")
        );
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn emit_event(
    access: &Access,
    action: Action,
    json: bool,
    min_risk: Option<crate::model::Risk>,
    lang: Language,
    notify: bool,
    sound: bool,
    last_notifications: &mut NotificationCooldowns,
    log_writer: &mut Option<BufWriter<std::fs::File>>,
    event_source: &Option<HANDLE>,
    history_enabled: bool,
) -> Result<()> {
    let event = AccessEvent {
        schema_version: SCHEMA_VERSION,
        event_code: event_code(action),
        tool_version: env!("CARGO_PKG_VERSION"),
        action,
        observed_at: Utc::now(),
        access: access.clone(),
    };
    if history_enabled {
        crate::history::append(&event)?;
    }
    if sound && matches!(action, Action::Start) && access.activity == Activity::Active {
        crate::platform::play_chime();
    }
    log_event(&event, log_writer)?;
    write_eventlog(&event, event_source);
    output::print_event(&event, json, min_risk, lang)?;
    if notify
        && notification_due(last_notifications, &access.key)
        && let Err(error) = crate::notify::notify_access(access, action, lang)
    {
        eprintln!("notification failed: {error:#}");
    }
    Ok(())
}

fn access_changed(old: &Access, current: &Access) -> bool {
    old.activity != current.activity
        || old.risk != current.risk
        || old.confidence != current.confidence
        || old.enforcement != current.enforcement
        || old.evidence != current.evidence
}

fn update_enforcement_candidates<'a>(
    accesses: impl Iterator<Item = &'a Access>,
    enabled: bool,
    observations: &mut HashMap<(String, String), u8>,
    terminated: &mut HashSet<(String, String)>,
) {
    if !enabled {
        observations.clear();
        return;
    }
    for access in accesses {
        if access.activity != Activity::Active
            || access.enforcement != EnforcementDecision::Deny
            || access.risk == crate::model::Risk::Blocked
            || protected_application(&access.application)
        {
            observations.retain(|(key, _), _| key != &access.key);
            continue;
        }
        let Some(pid) = access.pid else {
            observations.retain(|(key, _), _| key != &access.key);
            continue;
        };
        let Some(instance) = access
            .process
            .as_ref()
            .map(|context| context.instance_id.as_str())
        else {
            observations.retain(|(key, _), _| key != &access.key);
            continue;
        };
        let identity = (access.key.clone(), instance.to_owned());
        if terminated.contains(&identity) {
            continue;
        }
        let count = observations.entry(identity).or_default();
        *count = count.saturating_add(1);
        if *count < 2 {
            continue;
        }
        match crate::platform::terminate_process_by_pid(pid, instance) {
            Ok(()) => {
                terminated.insert((access.key.clone(), instance.to_owned()));
                eprintln!(
                    "{}",
                    format!(
                        "  TERMINATED policy-denied process {} (PID {}) after two observations",
                        access.application, pid
                    )
                    .red()
                    .bold()
                );
            }
            Err(error) => eprintln!(
                "{}",
                format!(
                    "  Failed to terminate policy-denied process {} (PID {}): {error}",
                    access.application, pid
                )
                .yellow()
            ),
        }
    }
}

fn protected_application(application: &str) -> bool {
    matches!(
        application.to_ascii_lowercase().as_str(),
        "system"
            | "registry"
            | "smss.exe"
            | "csrss.exe"
            | "wininit.exe"
            | "services.exe"
            | "lsass.exe"
            | "winlogon.exe"
            | "dwm.exe"
            | "explorer.exe"
            | "mcw.exe"
    )
}

fn log_event(event: &AccessEvent, writer: &mut Option<BufWriter<std::fs::File>>) -> Result<()> {
    if let Some(writer) = writer {
        serde_json::to_writer(&mut *writer, event)?;
        writeln!(writer)?;
        writer.flush()?;
    }
    Ok(())
}

fn write_eventlog(event: &AccessEvent, handle: &Option<HANDLE>) {
    let Some(handle) = handle else { return };
    let msg = format!(
        "{} {} {} {} (PID {})",
        event.action,
        event.access.resource,
        event.access.application,
        event.access.risk,
        event.access.pid.map_or("?".into(), |pid| pid.to_string()),
    );
    let wide: Vec<u16> = msg.encode_utf16().chain(Some(0)).collect();
    let strings = [PCWSTR(wide.as_ptr())];
    let event_type = if event.access.risk >= crate::model::Risk::Suspicious {
        EVENTLOG_WARNING_TYPE
    } else {
        EVENTLOG_INFORMATION_TYPE
    };
    let _ = unsafe {
        ReportEventW(
            *handle,
            event_type,
            0,
            event.event_code as u32,
            None,
            0,
            Some(&strings),
            None,
        )
    };
}

const NOTIFICATION_COOLDOWN: Duration = Duration::from_secs(30);
const MAX_COOLDOWNS: usize = 4096;

#[derive(Default)]
struct NotificationCooldowns {
    last: HashMap<String, Instant>,
    overloaded: bool,
}
impl NotificationCooldowns {
    fn prune(&mut self, now: Instant) {
        self.last
            .retain(|_, at| now.saturating_duration_since(*at) < NOTIFICATION_COOLDOWN);
        if self.last.len() < MAX_COOLDOWNS {
            self.overloaded = false;
        }
    }
    fn due(&mut self, key: &str, now: Instant) -> bool {
        self.prune(now);
        if self.last.contains_key(key) {
            return false;
        }
        if self.last.len() == MAX_COOLDOWNS {
            if !self.overloaded {
                eprintln!(
                    "notification cooldown overload: {MAX_COOLDOWNS} active identities; new toasts suppressed until cooldown expiry (event output/history preserved)"
                );
                self.overloaded = true;
            }
            return false;
        }
        self.last.insert(key.to_owned(), now);
        true
    }
}
fn notification_due(last: &mut NotificationCooldowns, key: &str) -> bool {
    last.due(key, Instant::now())
}

struct RegistryWatcher {
    running: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Drop for RegistryWatcher {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
fn spawn_registry_watcher(
    sender: mpsc::SyncSender<()>,
    running: Arc<AtomicBool>,
) -> RegistryWatcher {
    let alive = Arc::clone(&running);
    let worker = std::thread::spawn(move || {
        let root = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
        let Ok(key) = root.open_subkey(
            r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore",
        ) else {
            eprintln!("registry notifications unavailable; polling remains active");
            return;
        };
        let event = match unsafe { CreateEventW(None, false, false, None) } {
            Ok(event) => event,
            Err(error) => {
                eprintln!("registry notification event unavailable: {error}");
                return;
            }
        };
        let hkey = HKEY(key.raw_handle().cast());
        'watch: while alive.load(Ordering::Acquire) {
            let status = unsafe {
                RegNotifyChangeKeyValue(
                    hkey,
                    true,
                    REG_NOTIFY_CHANGE_NAME | REG_NOTIFY_CHANGE_LAST_SET,
                    Some(event),
                    true,
                )
            };
            if status.is_err() {
                eprintln!("registry notification registration failed: {status:?}");
                break;
            }
            loop {
                if !alive.load(Ordering::Acquire) {
                    break 'watch;
                }
                match unsafe { WaitForSingleObject(event, 500) } {
                    WAIT_OBJECT_0 => break,
                    WAIT_TIMEOUT => continue,
                    other => {
                        eprintln!("registry notification wait failed: {other:?}");
                        break 'watch;
                    }
                }
            }
            if matches!(
                sender.try_send(()),
                Err(mpsc::TrySendError::Disconnected(_))
            ) {
                break;
            }
        }
        // Closing the registry key cancels its pending asynchronous registration.
        drop(key);
        let _ = unsafe { CloseHandle(event) };
    });
    RegistryWatcher {
        running,
        worker: Some(worker),
    }
}

fn by_key(accesses: Vec<Access>) -> HashMap<String, Access> {
    accesses
        .into_iter()
        .map(|access| (access.key.clone(), access))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Confidence, ProcessContext, Resource, Risk};

    #[test]
    fn callback_burst_retains_one_nonblocking_wake() {
        let (sender, receiver) = mpsc::sync_channel(1);
        for _ in 0..10_000 {
            let _ = sender.try_send(());
        }
        assert_eq!(receiver.try_recv(), Ok(()));
        assert_eq!(receiver.try_recv(), Err(mpsc::TryRecvError::Empty));
    }

    #[test]
    fn cooldown_expiry_prunes_without_rearming_early() {
        let now = Instant::now();
        let mut cooldowns = NotificationCooldowns::default();
        assert!(cooldowns.due("a", now));
        assert!(!cooldowns.due("a", now + Duration::from_secs(29)));
        assert!(cooldowns.due("a", now + NOTIFICATION_COOLDOWN));
        cooldowns.prune(now + NOTIFICATION_COOLDOWN * 2);
        assert!(cooldowns.last.is_empty());
    }

    #[test]
    fn cooldown_saturation_does_not_evict_active_identity() {
        let now = Instant::now();
        let mut cooldowns = NotificationCooldowns::default();
        for n in 0..MAX_COOLDOWNS {
            assert!(cooldowns.due(&n.to_string(), now));
        }
        assert!(!cooldowns.due("overflow", now));
        assert!(!cooldowns.due("0", now + Duration::from_secs(1)));
        assert_eq!(cooldowns.last.len(), MAX_COOLDOWNS);
        assert!(cooldowns.due("overflow", now + NOTIFICATION_COOLDOWN));
        assert_eq!(cooldowns.last.len(), 1);
    }

    fn access(activity: Activity, decision: EnforcementDecision) -> Access {
        Access {
            key: "camera:test:1".into(),
            resource: Resource::Camera,
            activity,
            risk: Risk::Suspicious,
            confidence: Confidence::High,
            enforcement: decision,
            application: "test.exe".into(),
            pid: Some(999_999),
            parent_pid: None,
            parent_name: None,
            executable: Some(r"C:\test.exe".into()),
            signature: None,
            device: None,
            started_at: None,
            modules: vec![],
            evidence: vec![],
            process: Some(ProcessContext {
                instance_id: "999999:123".into(),
                ..ProcessContext::default()
            }),
        }
    }

    #[test]
    fn unavailable_collector_does_not_turn_missing_access_into_a_stop_event() {
        let snapshot = Snapshot {
            collectors: vec![crate::model::CollectorHealth {
                collector: "privacy_store",
                state: CollectorState::Unavailable,
                detail: Some("registry access denied".into()),
            }],
            accesses: vec![],
            observation_gaps: Vec::new(),
        };
        assert!(ensure_collectors_available(&snapshot).is_err());
    }

    #[test]
    fn enforcement_requires_active_explicit_denial_and_two_observations() {
        let ready = access(Activity::Ready, EnforcementDecision::Deny);
        let alert = access(Activity::Active, EnforcementDecision::Alert);
        let denied = access(Activity::Active, EnforcementDecision::Deny);
        let mut observations = HashMap::new();
        let mut terminated = HashSet::new();

        update_enforcement_candidates(
            [&ready, &alert].into_iter(),
            true,
            &mut observations,
            &mut terminated,
        );
        assert!(observations.is_empty());

        update_enforcement_candidates(
            [&denied].into_iter(),
            true,
            &mut observations,
            &mut terminated,
        );
        assert_eq!(observations[&(denied.key.clone(), "999999:123".into())], 1);
    }

    #[test]
    fn replacing_camera_process_starts_new_count_and_retains_kill_identity() {
        let old = access(Activity::Active, EnforcementDecision::Deny);
        let mut replacement = old.clone();
        replacement.process.as_mut().unwrap().instance_id = "999999:456".into();
        let mut observations = HashMap::new();
        let mut terminated = HashSet::new();
        update_enforcement_candidates([&old].into_iter(), true, &mut observations, &mut terminated);
        let old_identity = (old.key.clone(), "999999:123".into());
        terminated.insert(old_identity.clone());
        update_enforcement_candidates(
            [&replacement].into_iter(),
            true,
            &mut observations,
            &mut terminated,
        );
        assert_eq!(observations[&(old.key.clone(), "999999:456".into())], 1);
        assert!(terminated.contains(&old_identity));
    }

    #[test]
    fn missing_process_never_counts_toward_a_new_instance_at_the_same_pid() {
        let mut missing = access(Activity::Active, EnforcementDecision::Deny);
        missing.process = None;
        let mut verified = access(Activity::Active, EnforcementDecision::Deny);
        verified.process.as_mut().unwrap().instance_id = "999999:456".into();
        let mut observations = HashMap::new();
        let mut terminated = HashSet::new();

        update_enforcement_candidates(
            [&missing].into_iter(),
            true,
            &mut observations,
            &mut terminated,
        );
        assert!(observations.is_empty());
        update_enforcement_candidates(
            [&verified].into_iter(),
            true,
            &mut observations,
            &mut terminated,
        );
        assert_eq!(
            observations[&(verified.key.clone(), "999999:456".into())],
            1
        );
        assert!(terminated.is_empty());
    }

    #[test]
    fn critical_process_names_are_protected() {
        assert!(protected_application("LSASS.EXE"));
        assert!(protected_application("mcw.exe"));
        assert!(!protected_application("browser.exe"));
    }
}
