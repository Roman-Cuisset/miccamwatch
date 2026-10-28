//! Bounded presentation and typed-command boundary. Never export the complete model.
use crate::model::{Activity, CollectorState, Confidence, Resource, Snapshot};
use anyhow::{Result, ensure};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::io::{self, Write};

pub mod commands;
pub mod preferences;
#[cfg(windows)]
pub mod windows;

pub const PROTOCOL_VERSION: u8 = 3;
pub const MAX_FRAME_BYTES: usize = 262_144;
pub const MAX_EVENT_BYTES: usize = 4096;
const MAX_STRING_BYTES: usize = 1024;
const MAX_ACCESSES: usize = 128;
const MAX_ITEMS: usize = 64;

#[derive(Clone, Debug, Serialize)]
pub struct DesktopSnapshot {
    #[serde(rename = "type")]
    kind: &'static str,
    pub protocol_version: u8,
    pub sequence: u64,
    pub observed_at: DateTime<Utc>,
    pub engine: Engine,
    pub locale: String,
    pub resources: [ResourceSummary; 2],
    pub accesses: Vec<DesktopAccess>,
    pub collectors: Vec<DesktopCollector>,
    pub observation_gaps: Vec<Resource>,
    pub protection: Protection,
    pub diagnostics: Vec<String>,
    pub truncated: bool,
    pub controls: commands::Controls,
    pub preferences: preferences::PreferencesSnapshot,
}
#[derive(Clone, Debug, Serialize)]
pub struct Engine {
    pub version: &'static str,
    pub platform: &'static str,
    pub mode: String,
    pub state: &'static str,
    pub detail: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct ResourceSummary {
    pub resource: Resource,
    pub activity: &'static str,
    pub coverage: &'static str,
    pub confidence: Option<Confidence>,
    pub active_count: usize,
    pub ready_count: usize,
}
#[derive(Clone, Debug, Serialize)]
pub struct DesktopAccess {
    pub resource: Resource,
    pub activity: Activity,
    pub confidence: Confidence,
    pub application: String,
    pub pid: Option<u32>,
    pub device: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
}
#[derive(Clone, Debug, Serialize)]
pub struct DesktopCollector {
    pub collector: String,
    pub state: CollectorState,
    pub detail: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Protection {
    pub microphone: ProtectionStatus,
    pub camera: ProtectionStatus,
}
#[derive(Clone, Debug, Serialize)]
pub struct ProtectionStatus {
    pub state: &'static str,
    pub requested: Option<bool>,
    pub service_active: Option<bool>,
    pub detail: Option<String>,
}
impl Default for ProtectionStatus {
    fn default() -> Self {
        Self {
            state: "unknown",
            requested: None,
            service_active: None,
            detail: None,
        }
    }
}
fn clip(value: &str, truncated: &mut bool) -> String {
    let mut end = value.len().min(MAX_STRING_BYTES);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    *truncated |= end != value.len();
    value[..end].to_owned()
}
fn relevant(name: &str, resource: Resource) -> bool {
    match name {
        "wasapi" | "pipewire_audio" | "coreaudio_input" => resource == Resource::Microphone,
        "privacy_store" | "module_scanner" | "sensor_activity" | "pipewire_video"
        | "coremediaio_video" => resource == Resource::Camera,
        // Unknown healthy collectors cannot establish coverage.
        _ => false,
    }
}
fn summary(resource: Resource, snapshot: Option<&Snapshot>) -> ResourceSummary {
    let mut result = ResourceSummary {
        resource,
        activity: "unknown",
        coverage: "unavailable",
        confidence: None,
        active_count: 0,
        ready_count: 0,
    };
    let Some(snapshot) = snapshot else {
        return result;
    };
    let mut seen = false;
    let mut healthy = false;
    let mut complete = true;
    for collector in &snapshot.collectors {
        let known = relevant(collector.collector, Resource::Microphone)
            || relevant(collector.collector, Resource::Camera);
        if relevant(collector.collector, resource) {
            seen = true;
            healthy |= collector.state != CollectorState::Unavailable;
            complete &= collector.state == CollectorState::Healthy;
        } else if !known && collector.state != CollectorState::Healthy {
            // A failed unknown collector can affect either resource.
            complete = false;
        }
    }
    complete &= seen && !snapshot.observation_gaps.contains(&resource);
    result.coverage = if complete {
        "complete"
    } else if healthy || snapshot.accesses.iter().any(|a| a.resource == resource) {
        "incomplete"
    } else {
        "unavailable"
    };
    for access in snapshot.accesses.iter().filter(|a| a.resource == resource) {
        match access.activity {
            Activity::Active => result.active_count += 1,
            Activity::Ready => result.ready_count += 1,
        }
    }
    result.activity = if result.active_count > 0 {
        "active"
    } else if result.ready_count > 0 {
        "ready"
    } else if complete {
        "none_observed"
    } else {
        "unknown"
    };
    // Confidence describes the strongest evidence of the displayed activity, not coverage.
    for access in snapshot.accesses.iter().filter(|a| {
        a.resource == resource && (result.active_count == 0 || a.activity == Activity::Active)
    }) {
        result.confidence = match (result.confidence, access.confidence) {
            (Some(Confidence::High), _) | (_, Confidence::High) => Some(Confidence::High),
            (Some(Confidence::Medium), _) | (_, Confidence::Medium) => Some(Confidence::Medium),
            _ => Some(Confidence::Low),
        };
    }
    result
}
impl DesktopSnapshot {
    pub fn from_snapshot(
        sequence: u64,
        observed_at: DateTime<Utc>,
        locale: &str,
        snapshot: std::result::Result<&Snapshot, &str>,
        mode: &str,
    ) -> Self {
        let mut truncated = false;
        let source = snapshot.as_ref().ok().copied();
        let state = match source {
            None => "unavailable",
            Some(s)
                if s.collectors.is_empty()
                    || !s.observation_gaps.is_empty()
                    || s.collectors
                        .iter()
                        .any(|c| c.state != CollectorState::Healthy) =>
            {
                "degraded"
            }
            Some(_) => "healthy",
        };
        let detail = snapshot.err().map(|s| clip(s, &mut truncated));
        let mut dto = Self {
            kind: "snapshot",
            protocol_version: PROTOCOL_VERSION,
            sequence,
            observed_at,
            engine: Engine {
                version: env!("CARGO_PKG_VERSION"),
                platform: std::env::consts::OS,
                mode: clip(mode, &mut truncated),
                state,
                detail,
            },
            locale: clip(locale, &mut truncated),
            resources: [
                summary(Resource::Microphone, source),
                summary(Resource::Camera, source),
            ],
            accesses: Vec::new(),
            collectors: Vec::new(),
            observation_gaps: Vec::new(),
            protection: Protection {
                microphone: ProtectionStatus::default(),
                camera: ProtectionStatus::default(),
            },
            diagnostics: Vec::new(),
            controls: commands::Controls::default(),
            preferences: preferences::PreferencesSnapshot::default(),
            truncated,
        };
        if let Some(s) = source {
            dto.truncated |= s.accesses.len() > MAX_ACCESSES || s.collectors.len() > MAX_ITEMS;
            dto.accesses = s
                .accesses
                .iter()
                .take(MAX_ACCESSES)
                .map(|a| DesktopAccess {
                    resource: a.resource,
                    activity: a.activity,
                    confidence: a.confidence,
                    application: clip(&a.application, &mut dto.truncated),
                    pid: a.pid,
                    device: a.device.as_deref().map(|s| clip(s, &mut dto.truncated)),
                    started_at: a.started_at,
                })
                .collect();
            dto.collectors = s
                .collectors
                .iter()
                .take(MAX_ITEMS)
                .map(|c| DesktopCollector {
                    collector: clip(c.collector, &mut dto.truncated),
                    state: c.state,
                    detail: c.detail.as_deref().map(|s| clip(s, &mut dto.truncated)),
                })
                .collect();
            // Resource is a two-value enum: preserve semantics without duplicate amplification.
            for resource in [Resource::Microphone, Resource::Camera] {
                if s.observation_gaps.contains(&resource) {
                    dto.observation_gaps.push(resource);
                }
            }
            dto.truncated |= dto.observation_gaps.len() != s.observation_gaps.len();
        }
        dto
    }

    #[cfg(windows)]
    pub fn set_protection(
        &mut self,
        microphone: std::result::Result<&crate::platform::MicrophoneProtectionStatus, &str>,
        camera: std::result::Result<&crate::privacy::CameraControlObservation, &str>,
    ) {
        use crate::{model::MicrophoneMuteState, privacy::CameraPrivacyState};
        self.protection.microphone = match microphone {
            Ok(s) => ProtectionStatus {
                state: if s.endpoint_count == 0 {
                    "unknown"
                } else {
                    match s.mute_state {
                        MicrophoneMuteState::Muted => "muted",
                        MicrophoneMuteState::Unmuted => "unmuted",
                        MicrophoneMuteState::Mixed => "mixed",
                        MicrophoneMuteState::Unavailable => "unknown",
                    }
                },
                requested: Some(s.requested),
                service_active: Some(s.service_active),
                detail: s.detail.as_deref().map(|s| clip(s, &mut self.truncated)),
            },
            Err(error) => ProtectionStatus {
                detail: Some(clip(error, &mut self.truncated)),
                ..ProtectionStatus::default()
            },
        };
        self.protection.camera = match camera {
            Ok(s) => ProtectionStatus {
                state: if s.present_total == 0 || s.unknown_devices != 0 {
                    "unknown"
                } else if s.pending_restore != 0 {
                    "system_managed"
                } else {
                    match s.state {
                        CameraPrivacyState::Allowed => "allowed",
                        CameraPrivacyState::Blocked => "blocked",
                        CameraPrivacyState::SystemManaged => "system_managed",
                    }
                },
                requested: Some(s.desired_blocked),
                service_active: Some(s.helper_active),
                detail: s.detail.as_deref().map(|s| clip(s, &mut self.truncated)),
            },
            Err(error) => ProtectionStatus {
                detail: Some(clip(error, &mut self.truncated)),
                ..ProtectionStatus::default()
            },
        };
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        fn attempt(value: &DesktopSnapshot) -> Result<Vec<u8>> {
            let mut output = BoundedFrame(Vec::with_capacity(8192));
            serde_json::to_writer(&mut output, value)?;
            output.write_all(b"\n")?;
            Ok(output.0)
        }
        if let Ok(bytes) = attempt(self) {
            return Ok(bytes);
        }
        // Escaped control characters can expand sixfold. Reduce only exported detail,
        // never the aggregate known activity/coverage or desired/actual protection.
        let mut reduced = self.clone();
        reduced.truncated = true;
        loop {
            if !reduced.accesses.is_empty() {
                reduced.accesses.truncate(reduced.accesses.len() / 2);
            } else if !reduced.collectors.is_empty() {
                reduced.collectors.truncate(reduced.collectors.len() / 2);
            } else if !reduced.diagnostics.is_empty() {
                reduced.diagnostics.truncate(reduced.diagnostics.len() / 2);
            } else {
                return attempt(&reduced);
            }
            if let Ok(bytes) = attempt(&reduced) {
                return Ok(bytes);
            }
        }
    }
}
struct BoundedFrame(Vec<u8>);
impl Write for BoundedFrame {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.0.len() + bytes.len() > MAX_FRAME_BYTES {
            return Err(io::Error::other("desktop frame exceeds limit"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "type", deny_unknown_fields)]
pub(crate) enum UiEvent {
    #[serde(rename = "ready")]
    Ready { protocol_version: u8, sequence: u64 },
    #[serde(rename = "timing")]
    Timing {
        protocol_version: u8,
        stage: TimingStage,
        elapsed_ms: u64,
    },
    #[serde(rename = "command")]
    Command {
        protocol_version: u8,
        request: commands::UiCommand,
    },
    #[serde(rename = "preferences_command")]
    PreferencesCommand {
        protocol_version: u8,
        request: preferences::PreferenceRequest,
    },
}
#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TimingStage {
    RunnerStarted,
    EngineCreated,
    DartStarted,
    FirstFrame,
    SnapshotReceived,
}
impl TimingStage {
    #[cfg(windows)]
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::RunnerStarted => "runner_started",
            Self::EngineCreated => "engine_created",
            Self::DartStarted => "dart_started",
            Self::FirstFrame => "first_frame",
            Self::SnapshotReceived => "snapshot_received",
        }
    }
}
pub(crate) fn parse_event(line: &[u8]) -> Result<UiEvent> {
    ensure!(
        !line.is_empty() && line.len() <= MAX_EVENT_BYTES,
        "invalid UI event size"
    );
    let event: UiEvent = serde_json::from_slice(line)?;
    let version = match &event {
        UiEvent::Ready {
            protocol_version, ..
        }
        | UiEvent::Timing {
            protocol_version, ..
        }
        | UiEvent::PreferencesCommand {
            protocol_version, ..
        }
        | UiEvent::Command {
            protocol_version, ..
        } => *protocol_version,
    };
    ensure!(version == PROTOCOL_VERSION, "unsupported UI protocol");
    Ok(event)
}

/// Oversized/native-log lines are discarded through newline, not split into commands.
#[derive(Default)]
pub(crate) struct EventLines {
    bytes: Vec<u8>,
    discarding: bool,
}
impl EventLines {
    pub(crate) fn feed(&mut self, bytes: &[u8], mut emit: impl FnMut(Result<UiEvent>)) {
        for &byte in bytes {
            if byte == b'\n' {
                if self.discarding {
                    emit(Err(anyhow::anyhow!("oversized UI event")));
                } else {
                    emit(parse_event(&self.bytes));
                }
                self.bytes.clear();
                self.discarding = false;
            } else if !self.discarding {
                if self.bytes.len() == MAX_EVENT_BYTES {
                    self.bytes.clear();
                    self.discarding = true;
                } else {
                    self.bytes.push(byte);
                }
            }
        }
    }
}

#[cfg(windows)]
pub fn microphone_capability(
    status: Result<&crate::platform::MicrophoneProtectionStatus, &str>,
    generation: u64,
    busy: bool,
    enabled: bool,
) -> commands::MicrophoneCapability {
    let mut capability = commands::MicrophoneCapability {
        generation,
        ..Default::default()
    };
    let status = match status {
        Ok(status) => {
            capability.current_requested = Some(status.requested);
            status
        }
        Err(_) => {
            capability.reason =
                Some("Microphone control state is unavailable; no command can be confirmed".into());
            return capability;
        }
    };
    if !enabled {
        capability.reason =
            Some("Read-only development connection; microphone commands are disabled".into());
    } else if busy {
        capability.reason = Some("A native microphone operation is pending".into());
    } else if !status.service_active
        && crate::windows_control::resource_conflict("microphone").unwrap_or(true)
    {
        capability.reason = Some(
            "Another or unavailable native owner holds the microphone; no takeover is permitted"
                .into(),
        );
    } else {
        let observed = status.endpoint_count > 0
            && status.mute_state != crate::model::MicrophoneMuteState::Unavailable;
        capability.can_enable = observed;
        capability.can_release = status.requested || observed;
        if !observed {
            capability.reason = Some("Endpoint mute cannot be observed; only an existing protection request can be released".into());
        }
    }
    capability
}

#[cfg(windows)]
fn microphone_readback_confirmed(
    requested: bool,
    status: &crate::platform::MicrophoneProtectionStatus,
) -> bool {
    let expected = if requested {
        crate::model::MicrophoneMuteState::Muted
    } else {
        crate::model::MicrophoneMuteState::Unmuted
    };
    status.requested == requested
        && status.endpoint_count > 0
        && status.mute_state == expected
        && (!requested || status.service_active)
}

#[cfg(windows)]
pub fn complete_microphone(
    supervisor: &mut windows::Supervisor,
    ticket: u64,
    requested: bool,
    result: Result<&crate::platform::MicrophoneProtectionStatus, &str>,
) {
    use commands::CommandState;
    match result {
        Err(error) => {
            supervisor.complete(
                ticket,
                CommandState::Failed,
                Some("native_error"),
                Some(format!(
                    "Native operation failed or may be partial; inspect fresh state: {error}"
                )),
            );
            eprintln!(
                "{{\"event\":\"microphone_result\",\"ticket\":{ticket},\"requested\":{requested},\"outcome\":\"failed\",\"actual\":\"unknown\"}}"
            );
        }
        Ok(status) => {
            let confirmed = microphone_readback_confirmed(requested, status);
            let outcome = if confirmed { "succeeded" } else { "uncertain" };
            eprintln!(
                "{{\"event\":\"microphone_result\",\"ticket\":{ticket},\"requested\":{requested},\"outcome\":\"{outcome}\",\"actual\":\"{:?}\",\"actual_requested\":{},\"guard_active\":{},\"endpoints\":{}}}",
                status.mute_state, status.requested, status.service_active, status.endpoint_count
            );
            if confirmed {
                let detail = if requested {
                    "Native SDK mute readback confirmed; this is not capture-permission denial"
                } else {
                    "Native SDK unmute readback confirmed; active inputs released, not restored to original states"
                };
                supervisor.complete(ticket, CommandState::Succeeded, None, Some(detail.into()));
            } else {
                supervisor.complete(ticket, CommandState::Uncertain, Some("readback_unconfirmed"),
                    Some("The native request completed, but effective endpoint mute/guard could not be confirmed; do not assume protection or restoration".into()));
            }
        }
    }
}

/// Read the native preference store only when preparing an attached/opening UI.
/// Saving is delegated to the process-owned worker; never block collection on
/// the stable settings lock or infer applied policy from the saved selection.
pub fn observe_preferences(
    writable: bool,
    profile_writable: bool,
    applied_profile: Option<crate::settings::PrivacyProfile>,
    effects: preferences::PreferenceEffects,
    worker: Option<&preferences::PreferenceWorker>,
) -> preferences::PreferencesSnapshot {
    let settings = crate::settings::Settings::load().map_err(|error| format!("{error:#}"));
    let mut view = preferences::PreferencesSnapshot::observed(
        settings.as_ref().map_err(String::as_str),
        writable,
        profile_writable,
        applied_profile,
        effects,
    );
    if worker.is_some_and(preferences::PreferenceWorker::is_pending) {
        view.writable = false;
        view.profile_writable = false;
        view.reason =
            Some("Native preference write pending; saved values are not an acknowledgement".into());
    } else if worker.is_some_and(|worker| !worker.is_available()) {
        view.writable = false;
        view.profile_writable = false;
        view.reason = Some("Preferences worker unavailable; restart the tray".into());
    } else if view.writable && !view.profile_writable {
        view.reason = Some("Assessment profile is managed by explicit configuration".into());
    }
    view
}

#[cfg(windows)]
pub fn complete_preference(
    supervisor: Option<&mut windows::Supervisor>,
    completion: &preferences::PreferenceCompletion,
) {
    let outcome = if completion.result.is_ok() {
        "succeeded"
    } else {
        "failed"
    };
    eprintln!(
        "{{\"event\":\"preference_result\",\"ticket\":{},\"outcome\":\"{outcome}\"}}",
        completion.ticket
    );
    if let Some(supervisor) = supervisor {
        match &completion.result {
            Ok(_) => supervisor.complete_preference(completion.ticket,
                commands::CommandState::Succeeded, None,
                Some("Saved preference readback confirmed; applied policy and delivery remain separate".into())),
            Err(error) => supervisor.complete_preference(completion.ticket,
                commands::CommandState::Failed, Some("preferences_error"),
                Some(format!("Preference write/readback failed or may be partial: {error}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Access, CollectorHealth, EnforcementDecision, Risk};
    #[cfg(windows)]
    #[test]
    fn sdk_mute_needs_readback_endpoints_and_a_live_requested_guard() {
        let mut status = crate::platform::MicrophoneProtectionStatus {
            requested: true,
            service_active: true,
            mute_state: crate::model::MicrophoneMuteState::Muted,
            endpoint_count: 2,
            hardware_mute_count: 1,
            corrections: 0,
            detail: None,
        };
        assert!(microphone_readback_confirmed(true, &status));
        status.service_active = false;
        assert!(!microphone_readback_confirmed(true, &status));
        status.service_active = true;
        status.mute_state = crate::model::MicrophoneMuteState::Mixed;
        assert!(!microphone_readback_confirmed(true, &status));
        status.mute_state = crate::model::MicrophoneMuteState::Muted;
        status.endpoint_count = 0;
        assert!(!microphone_readback_confirmed(true, &status));
    }
    #[cfg(windows)]
    #[test]
    fn release_needs_native_unmute_not_merely_cleared_intent_or_owner_exit() {
        let mut status = crate::platform::MicrophoneProtectionStatus {
            requested: false,
            service_active: false,
            mute_state: crate::model::MicrophoneMuteState::Muted,
            endpoint_count: 1,
            hardware_mute_count: 0,
            corrections: 0,
            detail: None,
        };
        assert!(!microphone_readback_confirmed(false, &status));
        status.mute_state = crate::model::MicrophoneMuteState::Unmuted;
        status.service_active = true;
        assert!(microphone_readback_confirmed(false, &status));
        status.requested = true;
        assert!(!microphone_readback_confirmed(false, &status));
    }
    fn source() -> Snapshot {
        Snapshot {
            accesses: vec![],
            collectors: vec![],
            observation_gaps: vec![],
        }
    }
    fn dto(source: &Snapshot) -> DesktopSnapshot {
        DesktopSnapshot::from_snapshot(4, Utc::now(), "en", Ok(source), "read_only_development")
    }
    fn access() -> Access {
        Access {
            key: "secret-key".into(),
            resource: Resource::Microphone,
            activity: Activity::Active,
            risk: Risk::Suspicious,
            confidence: Confidence::High,
            enforcement: EnforcementDecision::Unknown,
            application: "Observed app".into(),
            pid: Some(123),
            parent_pid: Some(12),
            parent_name: Some("secret-parent".into()),
            executable: Some("secret-path".into()),
            signature: None,
            device: None,
            started_at: None,
            modules: vec!["secret-module".into()],
            evidence: vec![],
            process: None,
        }
    }
    #[test]
    fn empty_and_failed_are_not_idle() {
        let s = source();
        let d = dto(&s);
        assert_eq!(d.resources[0].activity, "unknown");
        assert_eq!(d.resources[1].coverage, "unavailable");
        assert_eq!(d.protection.microphone.state, "unknown");
        let error = DesktopSnapshot::from_snapshot(
            1,
            Utc::now(),
            "en",
            Err("collector failed"),
            "read_only_development",
        );
        assert_eq!(error.engine.state, "unavailable");
        assert_eq!(error.resources[0].activity, "unknown");
    }
    #[test]
    fn mixed_health_and_gaps_retain_active_without_safe_claim() {
        let mut s = source();
        s.accesses.push(access());
        s.collectors = vec![
            CollectorHealth {
                collector: "wasapi",
                state: CollectorState::Healthy,
                detail: None,
            },
            CollectorHealth {
                collector: "privacy_store",
                state: CollectorState::Degraded,
                detail: Some("partial".into()),
            },
        ];
        s.observation_gaps.push(Resource::Microphone);
        let d = dto(&s);
        assert_eq!(d.resources[0].activity, "active");
        assert_eq!(d.resources[0].coverage, "incomplete");
        assert_eq!(d.resources[0].confidence, Some(Confidence::High));
        assert_eq!(d.resources[1].activity, "unknown");
        s.collectors.push(CollectorHealth {
            collector: "future_collector",
            state: CollectorState::Unavailable,
            detail: None,
        });
        s.accesses.clear();
        s.observation_gaps.clear();
        assert_eq!(dto(&s).resources[0].activity, "unknown");
    }
    #[test]
    fn healthy_empty_requires_relevant_evidence() {
        let mut s = source();
        s.collectors.push(CollectorHealth {
            collector: "wasapi",
            state: CollectorState::Healthy,
            detail: None,
        });
        let d = dto(&s);
        assert_eq!(d.resources[0].activity, "none_observed");
        assert_eq!(d.resources[1].activity, "unknown");
        s.collectors[0].collector = "future_healthy_collector";
        let d = dto(&s);
        assert_eq!(d.resources[0].activity, "unknown");
        assert_eq!(d.resources[1].activity, "unknown");
    }
    #[test]
    fn portable_collector_groups_keep_resource_health_separate() {
        for (microphone, camera) in [
            ("pipewire_audio", "pipewire_video"),
            ("coreaudio_input", "coremediaio_video"),
        ] {
            let s = Snapshot {
                accesses: vec![],
                collectors: vec![
                    CollectorHealth {
                        collector: microphone,
                        state: CollectorState::Healthy,
                        detail: None,
                    },
                    CollectorHealth {
                        collector: camera,
                        state: CollectorState::Unavailable,
                        detail: Some("not readable".into()),
                    },
                ],
                observation_gaps: vec![Resource::Camera],
            };
            let d = dto(&s);
            assert_eq!(d.resources[0].activity, "none_observed");
            assert_eq!(d.resources[0].coverage, "complete");
            assert_eq!(d.resources[1].activity, "unknown");
        }
    }
    #[test]
    fn whitelist_clips_utf8_and_escape_expansion_but_retains_counts() {
        let mut s = source();
        let mut a = access();
        a.application = "é".repeat(1000);
        a.device = Some("\u{0}".repeat(2000));
        s.accesses = vec![a; 200];
        s.collectors = (0..80)
            .map(|_| CollectorHealth {
                collector: "wasapi",
                state: CollectorState::Degraded,
                detail: Some("\u{0}".repeat(2000)),
            })
            .collect();
        let d = dto(&s);
        assert!(d.truncated);
        assert_eq!(d.accesses.len(), 128);
        assert_eq!(d.accesses[0].application.len(), 1024);
        let bytes = d.encode().unwrap();
        assert!(bytes.len() <= MAX_FRAME_BYTES);
        assert_eq!(bytes.last(), Some(&b'\n'));
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["resources"][0]["active_count"], 200);
        let text = String::from_utf8(bytes).unwrap();
        for secret in [
            "secret-key",
            "secret-path",
            "secret-parent",
            "secret-module",
            "enforcement",
            "evidence",
        ] {
            assert!(!text.contains(secret));
        }
    }
    #[test]
    fn parser_rejects_mutations_versions_extra_fields_and_bad_types() {
        for line in [
            r#"{"type":"mute","protocol_version":1}"#,
            r#"{"type":"ready","protocol_version":4,"sequence":1}"#,
            r#"{"type":"ready","protocol_version":1,"sequence":1,"command":"unmute"}"#,
            r#"{"type":"ready","protocol_version":1,"sequence":"1"}"#,
            r#"{"type":"timing","protocol_version":1,"stage":"shell","elapsed_ms":0}"#,
        ] {
            assert!(parse_event(line.as_bytes()).is_err());
        }
        assert!(parse_event(br#"{"type":"ready","protocol_version":3,"sequence":1}"#).is_ok());
    }
    #[test]
    fn oversized_line_cannot_smuggle_suffix_and_next_line_recovers() {
        let mut lines = EventLines::default();
        let mut accepted = 0;
        let mut rejected = 0;
        let mut feed = |result: Result<UiEvent>| {
            if result.is_ok() {
                accepted += 1
            } else {
                rejected += 1
            }
        };
        lines.feed(&vec![b'x'; MAX_EVENT_BYTES + 1], &mut feed);
        lines.feed(
            b"{\"type\":\"ready\",\"protocol_version\":3,\"sequence\":1}\n",
            &mut feed,
        );
        lines.feed(
            b"{\"type\":\"ready\",\"protocol_version\":3,\"sequence\":2}\n",
            &mut feed,
        );
        assert_eq!((accepted, rejected), (1, 1));
        assert!(lines.bytes.is_empty());
    }
    #[test]
    fn bounded_writer_rejects_overflow_and_split_events_are_complete() {
        let mut output = BoundedFrame(vec![0; MAX_FRAME_BYTES - 1]);
        assert_eq!(output.write(b"x").unwrap(), 1);
        assert!(output.write(b"y").is_err());
        assert_eq!(output.0.len(), MAX_FRAME_BYTES);
        let mut lines = EventLines::default();
        let mut results = Vec::new();
        lines.feed(b"{\"type\":\"timing\",\"protocol_version\":3,", |e| {
            results.push(e)
        });
        assert!(results.is_empty());
        lines.feed(b"\"stage\":\"first_frame\",\"elapsed_ms\":5}\n", |e| {
            results.push(e)
        });
        assert!(matches!(
            results.pop().unwrap().unwrap(),
            UiEvent::Timing {
                stage: TimingStage::FirstFrame,
                elapsed_ms: 5,
                ..
            }
        ));
    }
    #[cfg(windows)]
    #[test]
    fn protection_preserves_request_actual_and_service_independently() {
        use crate::{
            model::MicrophoneMuteState,
            privacy::{CameraControlObservation, CameraPrivacyState},
        };
        let mut d = dto(&source());
        let microphone = crate::platform::MicrophoneProtectionStatus {
            requested: true,
            service_active: false,
            mute_state: MicrophoneMuteState::Unmuted,
            endpoint_count: 1,
            hardware_mute_count: 0,
            corrections: 0,
            detail: None,
        };
        let mut camera = CameraControlObservation {
            desired_blocked: true,
            state: CameraPrivacyState::Blocked,
            present_total: 0,
            owned_blocked_present: 0,
            pending_restore: 0,
            absent_owned: 1,
            unknown_devices: 0,
            helper_active: false,
            detail: None,
        };
        d.set_protection(Ok(&microphone), Ok(&camera));
        assert_eq!(d.protection.microphone.state, "unmuted");
        assert_eq!(d.protection.microphone.requested, Some(true));
        assert_eq!(d.protection.microphone.service_active, Some(false));
        assert_eq!(d.protection.camera.state, "unknown");
        camera.present_total = 2;
        camera.unknown_devices = 1;
        d.set_protection(Ok(&microphone), Ok(&camera));
        assert_eq!(d.protection.camera.state, "unknown");
        camera.unknown_devices = 0;
        camera.pending_restore = 1;
        d.set_protection(Ok(&microphone), Ok(&camera));
        assert_eq!(d.protection.camera.state, "system_managed");
        d.set_protection(Err("passive query failed"), Err("passive query failed"));
        assert_eq!(d.protection.microphone.requested, None);
        assert_eq!(d.protection.camera.state, "unknown");
    }
}
