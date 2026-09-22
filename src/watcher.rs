use crate::{
    cli::Filter,
    i18n::Language,
    model::{
        Access, AccessEvent, Action, Activity, EnforcementDecision, Evidence, EvidenceKind,
        SCHEMA_VERSION, event_code,
    },
    output,
    platform::{PlatformMonitor, SessionLockState},
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
        Foundation::HANDLE,
        System::{
            EventLog::{
                DeregisterEventSource, EVENTLOG_INFORMATION_TYPE, EVENTLOG_WARNING_TYPE,
                RegisterEventSourceW, ReportEventW,
            },
            Registry::{
                HKEY, REG_NOTIFY_CHANGE_LAST_SET, REG_NOTIFY_CHANGE_NAME, RegNotifyChangeKeyValue,
            },
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

    let (wake_sender, wake_receiver) = mpsc::channel();
    let _audio_notifications = monitor
        .register_audio_notifications(wake_sender.clone())
        .ok();
    spawn_registry_watcher(wake_sender);
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

    let mut previous = snapshot_by_key(monitor, filter)?;
    let mut last_notifications = HashMap::new();
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
        denial_observations.retain(|key, _| current.contains_key(key));
        terminated.retain(|key| current.contains_key(key));
        previous = current;
    }

    if let Some(handle) = event_source {
        let _ = unsafe { DeregisterEventSource(handle) };
    }
    Ok(())
}

fn snapshot_by_key(monitor: &PlatformMonitor, filter: &Filter) -> Result<HashMap<String, Access>> {
    let mut accesses = monitor.snapshot(filter)?.accesses;
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

#[allow(clippy::too_many_arguments)]
fn emit_event(
    access: &Access,
    action: Action,
    json: bool,
    min_risk: Option<crate::model::Risk>,
    lang: Language,
    notify: bool,
    sound: bool,
    last_notifications: &mut HashMap<String, Instant>,
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
    observations: &mut HashMap<String, u8>,
    terminated: &mut HashSet<String>,
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
            observations.remove(&access.key);
            continue;
        }
        let count = observations.entry(access.key.clone()).or_default();
        *count = count.saturating_add(1);
        if *count < 2 || terminated.contains(&access.key) {
            continue;
        }
        let Some(pid) = access.pid else { continue };
        match crate::platform::terminate_process_by_pid(pid) {
            Ok(()) => {
                terminated.insert(access.key.clone());
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

fn notification_due(last: &mut HashMap<String, Instant>, key: &str) -> bool {
    const COOLDOWN: Duration = Duration::from_secs(30);
    let now = Instant::now();
    if last
        .get(key)
        .is_some_and(|previous| now.duration_since(*previous) < COOLDOWN)
    {
        return false;
    }
    last.insert(key.to_owned(), now);
    true
}

fn spawn_registry_watcher(sender: mpsc::Sender<()>) {
    std::thread::spawn(move || {
        let root = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
        let Ok(key) = root.open_subkey(
            r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore",
        ) else {
            return;
        };
        let hkey = HKEY(key.raw_handle().cast());
        loop {
            let status = unsafe {
                RegNotifyChangeKeyValue(
                    hkey,
                    true,
                    REG_NOTIFY_CHANGE_NAME | REG_NOTIFY_CHANGE_LAST_SET,
                    None,
                    false,
                )
            };
            if status.is_err() || sender.send(()).is_err() {
                break;
            }
        }
    });
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
    use crate::model::{Confidence, Resource, Risk};

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
            process: None,
        }
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
        assert_eq!(observations[&denied.key], 1);
    }

    #[test]
    fn critical_process_names_are_protected() {
        assert!(protected_application("LSASS.EXE"));
        assert!(protected_application("mcw.exe"));
        assert!(!protected_application("browser.exe"));
    }
}
