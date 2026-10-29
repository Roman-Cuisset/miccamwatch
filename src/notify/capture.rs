//! Native tray capture alerts. Consumes the owner's existing snapshots only.
use super::{DeliveryOutcome, NotificationIdentity};
use crate::{
    i18n::Language,
    model::{Access, Action, Activity, CollectorState, Resource, Snapshot},
};
use std::{
    collections::HashMap,
    sync::{Arc, atomic::{AtomicBool, AtomicU64, Ordering}, mpsc::{self, SyncSender}},
    thread,
    time::{Duration, Instant},
};

pub(crate) const NOTIFICATION_QUEUE_CAPACITY: usize = 32;
pub(crate) const NOTIFICATION_COOLDOWN: Duration = Duration::from_secs(30);
pub(crate) const MAX_COOLDOWNS: usize = 4096;

// Shared with the CLI watcher: never evict an unexpired identity to admit another.
#[derive(Default)]
pub(crate) struct NotificationCooldowns {
    pub(crate) last: HashMap<String, Instant>,
    overloaded: bool,
}
impl NotificationCooldowns {
    pub(crate) fn prune(&mut self, now: Instant) {
        self.last.retain(|_, at| now.saturating_duration_since(*at) < NOTIFICATION_COOLDOWN);
        if self.last.len() < MAX_COOLDOWNS { self.overloaded = false; }
    }
    pub(crate) fn due(&mut self, key: &str, now: Instant) -> bool {
        self.prune(now);
        if self.last.contains_key(key) { return false; }
        if self.last.len() == MAX_COOLDOWNS {
            if !self.overloaded {
                eprintln!("notification cooldown overload: {MAX_COOLDOWNS} active identities; new toasts suppressed until cooldown expiry (event output/history preserved)");
                self.overloaded = true;
            }
            return false;
        }
        self.last.insert(key.to_owned(), now);
        true
    }
}

fn incomplete(snapshot: &Snapshot, resource: Resource) -> bool {
    snapshot.observation_gaps.contains(&resource) || snapshot.collectors.iter().any(|collector| {
        collector.state != CollectorState::Healthy && match collector.collector {
            "wasapi" => resource == Resource::Microphone,
            "privacy_store" | "module_scanner" => resource == Resource::Camera,
            _ => true,
        }
    })
}

/// Missing records during a partial/unavailable scan are not evidence of stops.
/// Shared by native history and alerts, rather than collecting a second snapshot.
pub(crate) fn current_observations(previous: &HashMap<String, Access>, snapshot: &Snapshot) -> HashMap<String, Access> {
    let mut current: HashMap<_, _> = snapshot.accesses.iter()
        .map(|access| (access.key.clone(), access.clone())).collect();
    for (key, access) in previous {
        if incomplete(snapshot, access.resource) {
            // A Ready observation from an incomplete source cannot disprove an
            // earlier Active capture. Recovery must not manufacture a new start.
            let observed = current.get(key);
            if observed.is_none_or(|new| new.activity != Activity::Active) {
                current.insert(key.clone(), access.clone());
            }
        }
    }
    current
}

#[derive(Default)]
struct CaptureTransitions {
    active: HashMap<String, (Resource, bool)>,
    cooldowns: NotificationCooldowns,
}
impl CaptureTransitions {
    fn observe(&mut self, snapshot: &Snapshot, now: Instant, mut start: impl FnMut(&Access, bool)) {
        for (_, seen) in self.active.values_mut() { *seen = false; }
        for access in snapshot.accesses.iter().filter(|access| access.activity == Activity::Active) {
            if let Some((_, seen)) = self.active.get_mut(&access.key) {
                *seen = true;
            } else {
                start(access, self.cooldowns.due(&access.key, now));
                self.active.insert(access.key.clone(), (access.resource, true));
            }
        }
        self.active.retain(|_, (resource, seen)| *seen || incomplete(snapshot, *resource));
        self.cooldowns.prune(now);
    }
}

#[derive(Default)]
pub(crate) struct NotificationHealth {
    pub(crate) undelivered: AtomicU64,
    pub(crate) unavailable: AtomicBool,
    pub(crate) submitted: AtomicU64,
    pub(crate) suppressed: AtomicU64,
    pub(crate) suppressed_disabled: AtomicU64,
    pub(crate) suppressed_paused: AtomicU64,
    pub(crate) suppressed_cooldown: AtomicU64,
}
impl NotificationHealth {
    pub(crate) fn failed(&self) {
        self.undelivered.fetch_add(1, Ordering::Relaxed);
        self.unavailable.store(true, Ordering::Relaxed);
    }
    pub(crate) fn delivered(&self) {
        self.submitted.fetch_add(1, Ordering::Relaxed);
        self.unavailable.store(false, Ordering::Relaxed);
    }
    fn record(&self, result: Result<DeliveryOutcome, String>) {
        match result {
            Ok(DeliveryOutcome::Submitted) => self.delivered(),
            Ok(outcome) => {
                self.suppressed.fetch_add(1, Ordering::Relaxed);
                match outcome {
                    DeliveryOutcome::SuppressedDisabled => &self.suppressed_disabled,
                    DeliveryOutcome::SuppressedPaused => &self.suppressed_paused,
                    DeliveryOutcome::Submitted => unreachable!(),
                }.fetch_add(1, Ordering::Relaxed);
                // Suppression is not successful shell submission and must not
                // clear a previous delivery failure.
            }
            Err(error) => { self.failed(); eprintln!("notification failed: {error}"); }
        }
    }
}

#[derive(Clone)]
pub(crate) struct NotificationWorker {
    pub(crate) sender: SyncSender<(String, String)>,
    pub(crate) health: Arc<NotificationHealth>,
}
impl NotificationWorker {
    pub(crate) fn start() -> Self { Self::with_identity(NotificationIdentity::Installed) }
    pub(crate) fn with_identity(identity: NotificationIdentity) -> Self {
        let (sender, pending) = mpsc::sync_channel::<(String, String)>(NOTIFICATION_QUEUE_CAPACITY);
        let health = Arc::new(NotificationHealth::default());
        let delivery = Arc::clone(&health);
        thread::spawn(move || {
            use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};
            let apartment = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok() };
            // One bounded queue and one native delivery worker. No chime outside
            // the toast XML: exactly one audio request per submitted toast.
            while let Ok((title, message)) = pending.recv() {
                let result = match &apartment {
                    Ok(()) => super::deliver_message(&title, &message, identity)
                        .map_err(|error| error.to_string()),
                    Err(error) => Err(format!("notification apartment unavailable: {error}")),
                };
                delivery.record(result);
            }
            if apartment.is_ok() { unsafe { CoUninitialize() }; }
        });
        Self { sender, health }
    }
    pub(crate) fn submit(&self, title: &str, message: &str) -> bool {
        if self.sender.try_send((title.to_owned(), message.to_owned())).is_err() {
            self.health.failed();
            eprintln!("notification queue overloaded/unavailable; toast not submitted: {title}: {message}");
            false
        } else { true }
    }
}

#[derive(Default)]
pub(crate) struct CaptureAlerts { transitions: CaptureTransitions }
impl CaptureAlerts {
    pub(crate) fn observe(&mut self, snapshot: &Snapshot, worker: &NotificationWorker, lang: Language, test: bool) {
        self.transitions.observe(snapshot, Instant::now(), |access, due| {
            if !due {
                worker.health.suppressed.fetch_add(1, Ordering::Relaxed);
                worker.health.suppressed_cooldown.fetch_add(1, Ordering::Relaxed);
                return;
            }
            let (mut title, mut detail) = super::content::access_content(access, Action::Start, lang);
            if test {
                title = format!("TEST — {title}");
                detail = format!("TEST fixture only; no hardware capture. {detail}");
            }
            worker.submit(&title, &detail);
        });
    }
}

#[cfg(feature = "flutter-desktop")]
/// Explicit qualification-only fixture, never returned as dashboard observations.
/// Two independent resource starts; duplicates, Ready, and an observation gap
/// traverse exactly the same transition/cooldown/delivery path as native snapshots.
pub(crate) fn submit_test_sequence(alerts: &mut CaptureAlerts, worker: &NotificationWorker, lang: Language) {
    use crate::model::{Confidence, EnforcementDecision, Risk};
    let make = |resource, key: &str| Access {
        key: key.to_owned(), resource, activity: Activity::Active,
        risk: Risk::Expected, confidence: Confidence::High,
        enforcement: EnforcementDecision::Allow,
        application: "TEST native notification fixture".to_owned(), pid: None,
        parent_pid: None, parent_name: None, executable: None, signature: None,
        device: None, started_at: None, modules: Vec::new(), evidence: Vec::new(), process: None,
    };
    let mut snapshot = Snapshot {
        collectors: Vec::new(),
        accesses: vec![make(Resource::Microphone, "test:microphone"), make(Resource::Camera, "test:camera")],
        observation_gaps: Vec::new(),
    };
    let empty = Snapshot { collectors: Vec::new(), accesses: Vec::new(), observation_gaps: Vec::new() };
    alerts.observe(&empty, worker, lang, true);
    let ready = Snapshot {
        collectors: Vec::new(),
        accesses: snapshot.accesses.iter().cloned().map(|mut access| {
            access.activity = Activity::Ready;
            access
        }).collect(),
        observation_gaps: Vec::new(),
    };
    alerts.observe(&ready, worker, lang, true);
    alerts.observe(&snapshot, worker, lang, true);
    alerts.observe(&snapshot, worker, lang, true);
    snapshot.accesses.clear();
    snapshot.observation_gaps = vec![Resource::Microphone, Resource::Camera];
    alerts.observe(&snapshot, worker, lang, true);
    snapshot.accesses = vec![make(Resource::Microphone, "test:microphone"), make(Resource::Camera, "test:camera")];
    snapshot.observation_gaps.clear();
    alerts.observe(&snapshot, worker, lang, true);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CollectorHealth, Confidence, EnforcementDecision, Risk};

    fn access(resource: Resource, key: &str, activity: Activity) -> Access {
        Access {
            key: key.into(), resource, activity, risk: Risk::Expected,
            confidence: Confidence::High, enforcement: EnforcementDecision::Alert,
            application: "unit-test fixture".into(), pid: None, parent_pid: None,
            parent_name: None, executable: None, signature: None, device: None,
            started_at: None, modules: Vec::new(), evidence: Vec::new(), process: None,
        }
    }
    fn snapshot(accesses: Vec<Access>) -> Snapshot {
        Snapshot { collectors: Vec::new(), accesses, observation_gaps: Vec::new() }
    }
    fn observe(policy: &mut CaptureTransitions, snapshot: &Snapshot, at: Instant) -> Vec<(String, bool)> {
        let mut events = Vec::new();
        policy.observe(snapshot, at, |access, due| events.push((access.key.clone(), due)));
        events.sort();
        events
    }
    #[test]
    fn only_confirmed_active_edges_alert_not_ready_metadata_updates_or_stops() {
        let mut policy = CaptureTransitions::default();
        let now = Instant::now();
        let mut frame = snapshot(vec![access(Resource::Microphone, "mic", Activity::Ready)]);
        assert!(observe(&mut policy, &frame, now).is_empty());
        frame.accesses[0].activity = Activity::Active;
        assert_eq!(observe(&mut policy, &frame, now), vec![("mic".into(), true)]);
        frame.accesses[0].risk = Risk::Suspicious;
        frame.accesses[0].application = "changed metadata".into();
        assert!(observe(&mut policy, &frame, now + Duration::from_secs(60)).is_empty());
        frame.accesses[0].activity = Activity::Ready;
        assert!(observe(&mut policy, &frame, now + Duration::from_secs(61)).is_empty());
        assert!(observe(&mut policy, &snapshot(Vec::new()), now + Duration::from_secs(62)).is_empty());
    }
    #[test]
    fn real_restarts_obey_exact_cooldown_boundary_without_retroactive_retries() {
        let mut policy = CaptureTransitions::default();
        let now = Instant::now();
        let active = snapshot(vec![access(Resource::Camera, "cam", Activity::Active)]);
        let empty = snapshot(Vec::new());
        assert_eq!(observe(&mut policy, &active, now), vec![("cam".into(), true)]);
        observe(&mut policy, &empty, now + Duration::from_secs(1));
        assert_eq!(observe(&mut policy, &active, now + Duration::from_secs(29)), vec![("cam".into(), false)]);
        assert!(observe(&mut policy, &active, now + Duration::from_secs(30)).is_empty());
        observe(&mut policy, &empty, now + Duration::from_secs(30));
        assert_eq!(observe(&mut policy, &active, now + NOTIFICATION_COOLDOWN), vec![("cam".into(), true)]);
    }
    #[test]
    fn per_resource_gap_preserves_active_through_missing_and_ready_then_recovery() {
        let mut policy = CaptureTransitions::default();
        let now = Instant::now();
        let active = snapshot(vec![access(Resource::Microphone, "mic", Activity::Active)]);
        observe(&mut policy, &active, now);
        let mut partial = snapshot(Vec::new());
        partial.observation_gaps.push(Resource::Microphone);
        assert!(observe(&mut policy, &partial, now + Duration::from_secs(60)).is_empty());
        partial.accesses.push(access(Resource::Microphone, "mic", Activity::Ready));
        assert!(observe(&mut policy, &partial, now + Duration::from_secs(61)).is_empty());
        assert!(observe(&mut policy, &active, now + Duration::from_secs(62)).is_empty());
        observe(&mut policy, &snapshot(Vec::new()), now + Duration::from_secs(63));
        assert_eq!(observe(&mut policy, &active, now + Duration::from_secs(64)), vec![("mic".into(), true)]);
    }
    #[test]
    fn unavailable_and_degraded_mic_do_not_hide_healthy_camera_start_or_invent_mic_restart() {
        for state in [CollectorState::Unavailable, CollectorState::Degraded] {
            let mut policy = CaptureTransitions::default();
            let now = Instant::now();
            let mic = access(Resource::Microphone, "mic", Activity::Active);
            observe(&mut policy, &snapshot(vec![mic.clone()]), now);
            let mut partial = snapshot(vec![access(Resource::Camera, "cam", Activity::Active)]);
            partial.collectors.push(CollectorHealth { collector: "wasapi", state, detail: None });
            assert_eq!(observe(&mut policy, &partial, now + Duration::from_secs(60)), vec![("cam".into(), true)]);
            partial.collectors.clear();
            partial.accesses.push(mic);
            assert!(observe(&mut policy, &partial, now + Duration::from_secs(61)).is_empty());
        }
    }
    #[test]
    fn unrecognized_unavailable_source_preserves_both_resources() {
        let mut policy = CaptureTransitions::default();
        let now = Instant::now();
        let active = snapshot(vec![
            access(Resource::Microphone, "mic", Activity::Active),
            access(Resource::Camera, "cam", Activity::Active),
        ]);
        assert_eq!(observe(&mut policy, &active, now).len(), 2);
        let mut unknown = snapshot(Vec::new());
        unknown.collectors.push(CollectorHealth {
            collector: "unknown_source", state: CollectorState::Unavailable, detail: None,
        });
        assert!(observe(&mut policy, &unknown, now + Duration::from_secs(60)).is_empty());
        assert!(observe(&mut policy, &active, now + Duration::from_secs(61)).is_empty());
    }
    #[test]
    fn suppression_is_not_submission_or_recovery_and_failures_remain_counted() {
        let health = NotificationHealth::default();
        health.record(Err("native submission failure".into()));
        health.record(Ok(DeliveryOutcome::SuppressedDisabled));
        health.record(Ok(DeliveryOutcome::SuppressedPaused));
        assert_eq!(health.submitted.load(Ordering::Relaxed), 0);
        assert_eq!(health.suppressed.load(Ordering::Relaxed), 2);
        assert_eq!(health.suppressed_disabled.load(Ordering::Relaxed), 1);
        assert_eq!(health.suppressed_paused.load(Ordering::Relaxed), 1);
        assert!(health.unavailable.load(Ordering::Relaxed));
        health.record(Ok(DeliveryOutcome::Submitted));
        assert!(!health.unavailable.load(Ordering::Relaxed));
        assert_eq!(health.submitted.load(Ordering::Relaxed), 1);
        assert_eq!(health.undelivered.load(Ordering::Relaxed), 1);
    }
    #[test]
    fn repeated_capture_cooldown_and_disconnected_delivery_are_distinct() {
        let (sender, pending) = mpsc::sync_channel(NOTIFICATION_QUEUE_CAPACITY);
        let worker = NotificationWorker { sender, health: Arc::new(NotificationHealth::default()) };
        let mut alerts = CaptureAlerts::default();
        let active = Snapshot {
            collectors: Vec::new(),
            accesses: vec![
                access(Resource::Microphone, "microphone", Activity::Active),
                access(Resource::Camera, "camera", Activity::Active),
            ],
            observation_gaps: Vec::new(),
        };
        let stopped = Snapshot { collectors: Vec::new(), accesses: Vec::new(), observation_gaps: Vec::new() };
        alerts.observe(&active, &worker, Language::En, false);
        let _microphone = pending.try_recv().unwrap();
        let _camera = pending.try_recv().unwrap();
        alerts.observe(&stopped, &worker, Language::En, false);
        alerts.observe(&active, &worker, Language::En, false);
        assert!(matches!(pending.try_recv(), Err(mpsc::TryRecvError::Empty)));
        assert_eq!(worker.health.suppressed_cooldown.load(Ordering::Relaxed), 2);
        drop(pending);
        assert!(!worker.submit("capture", "disconnected"));
        assert_eq!(worker.health.undelivered.load(Ordering::Relaxed), 1);
    }
}
