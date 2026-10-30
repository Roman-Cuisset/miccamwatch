//! Portable, bounded preferences lane for the owned desktop connection.
//! Saved preferences are not assertions about applied collector policy or devices.
use anyhow::{Context, Result, ensure};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use super::commands::{
    CommandEnvelope, CommandState, authorize_envelope, deserialize_request_id, deserialize_sequence,
};
use crate::settings::{PrivacyProfile, Settings};

const MAX_TEXT_BYTES: usize = 1024;
const MAX_CODE_BYTES: usize = 64;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PreferenceKey {
    NotificationsEnabled,
    SoundEnabled,
    HistoryEnabled,
    Profile,
    NotificationPause,
}

impl PreferenceKey {
    fn current_value(self, settings: &Settings) -> PreferenceValue {
        match self {
            Self::NotificationsEnabled => PreferenceValue::Bool(settings.notifications_enabled),
            Self::SoundEnabled => PreferenceValue::Bool(settings.sound_enabled),
            Self::HistoryEnabled => PreferenceValue::Bool(settings.history_enabled),
            Self::Profile => PreferenceValue::Profile(settings.profile),
            Self::NotificationPause => PreferenceValue::PauseUntil(settings.pause_notifications_until),
        }
    }

    fn accepts(self, value: PreferenceValue) -> bool {
        matches!(
            (self, value),
            (Self::Profile, PreferenceValue::Profile(_))
                | (Self::NotificationsEnabled, PreferenceValue::Bool(_))
                | (Self::SoundEnabled, PreferenceValue::Bool(_))
                | (Self::HistoryEnabled, PreferenceValue::Bool(_))
                | (Self::NotificationPause, PreferenceValue::PauseAction(_))
        )
    }

    fn accepts_expected(self, value: PreferenceValue) -> bool {
        if self == Self::NotificationPause {
            matches!(value, PreferenceValue::PauseUntil(_))
        } else {
            self.accepts(value)
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum PreferenceValue {
    Bool(bool),
    Profile(PrivacyProfile),
    PauseAction(NotificationPauseAction),
    PauseUntil(Option<DateTime<Utc>>),
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NotificationPauseAction {
    #[serde(rename = "pause_30_minutes")]
    Pause30Minutes,
    Resume,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PreferenceAction {
    SetPreference,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(try_from = "PreferenceRequestWire")]
pub struct PreferenceRequest {
    pub session: String,
    pub id: u64,
    pub action: PreferenceAction,
    pub observed_sequence: u64,
    pub key: PreferenceKey,
    pub value: PreferenceValue,
    pub expected_value: PreferenceValue,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PreferenceRequestWire {
    session: String,
    #[serde(deserialize_with = "deserialize_request_id")]
    id: u64,
    action: PreferenceAction,
    #[serde(deserialize_with = "deserialize_sequence")]
    observed_sequence: u64,
    key: PreferenceKey,
    value: PreferenceValue,
    expected_value: PreferenceValue,
}

impl TryFrom<PreferenceRequestWire> for PreferenceRequest {
    type Error = anyhow::Error;

    fn try_from(wire: PreferenceRequestWire) -> Result<Self> {
        let request = Self {
            session: wire.session,
            id: wire.id,
            action: wire.action,
            observed_sequence: wire.observed_sequence,
            key: wire.key,
            value: wire.value,
            expected_value: wire.expected_value,
        };
        request.validate_types()?;
        Ok(request)
    }
}

impl PreferenceRequest {
    fn validate_types(&self) -> Result<()> {
        ensure!(
            self.key.accepts(self.value) && self.key.accepts_expected(self.expected_value),
            "preference key and value types do not match"
        );
        Ok(())
    }

    /// The caller holds the stable settings lock. Validate before any mutation,
    /// then change precisely one field of the latest settings, never a snapshot.
    pub fn apply_to(&self, settings: &mut Settings) -> Result<()> {
        self.apply_with_clock(settings, Utc::now)
    }

    fn apply_with_clock(
        &self,
        settings: &mut Settings,
        now: impl FnOnce() -> DateTime<Utc>,
    ) -> Result<()> {
        self.validate_types()?;
        ensure!(
            self.key.current_value(settings) == self.expected_value,
            "preference changed since the observed snapshot"
        );
        match (self.key, self.value) {
            (PreferenceKey::NotificationsEnabled, PreferenceValue::Bool(value)) => {
                settings.notifications_enabled = value;
            }
            (PreferenceKey::SoundEnabled, PreferenceValue::Bool(value)) => {
                settings.sound_enabled = value;
            }
            (PreferenceKey::HistoryEnabled, PreferenceValue::Bool(value)) => {
                settings.history_enabled = value;
            }
            (PreferenceKey::Profile, PreferenceValue::Profile(value)) => {
                settings.profile = value;
            }
            (PreferenceKey::NotificationPause, PreferenceValue::PauseAction(action)) => {
                settings.pause_notifications_until = match action {
                    NotificationPauseAction::Pause30Minutes => Some(
                        now()
                            .checked_add_signed(Duration::minutes(30))
                            .context("notification pause deadline is out of range")?,
                    ),
                    NotificationPauseAction::Resume => None,
                };
            }
            _ => unreachable!("preference types were validated before mutation"),
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PreferenceValues {
    pub notifications_enabled: bool,
    pub sound_enabled: bool,
    pub history_enabled: bool,
    pub profile: PrivacyProfile,
    pub pause_notifications_until: Option<DateTime<Utc>>,
    pub notifications_paused: bool,
}

impl PreferenceValues {
    pub fn from_settings(settings: &Settings) -> Self {
        Self {
            notifications_enabled: settings.notifications_enabled,
            sound_enabled: settings.sound_enabled,
            history_enabled: settings.history_enabled,
            profile: settings.profile,
            pause_notifications_until: settings.pause_notifications_until,
            notifications_paused: settings.notifications_paused(),
        }
    }

    pub fn current_value(&self, key: PreferenceKey) -> PreferenceValue {
        match key {
            PreferenceKey::NotificationsEnabled => {
                PreferenceValue::Bool(self.notifications_enabled)
            }
            PreferenceKey::SoundEnabled => PreferenceValue::Bool(self.sound_enabled),
            PreferenceKey::HistoryEnabled => PreferenceValue::Bool(self.history_enabled),
            PreferenceKey::Profile => PreferenceValue::Profile(self.profile),
            PreferenceKey::NotificationPause => PreferenceValue::PauseUntil(self.pause_notifications_until),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, PartialEq, Eq)]
pub struct PreferenceEffects {
    pub capture_notifications_supported: bool,
    pub capture_sounds_supported: bool,
    pub history_recording_supported: bool,
    pub history_recording_active: bool,
}

#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
pub struct PreferencesSnapshot {
    pub available: bool,
    pub writable: bool,
    pub profile_writable: bool,
    pub values: Option<PreferenceValues>,
    pub applied_profile: Option<PrivacyProfile>,
    pub effects: PreferenceEffects,
    pub reason: Option<String>,
    pub command: Option<PreferenceResult>,
}

impl PreferencesSnapshot {
    pub fn observed(
        settings: std::result::Result<&Settings, &str>,
        writable: bool,
        profile_writable: bool,
        applied_profile: Option<PrivacyProfile>,
        mut effects: PreferenceEffects,
    ) -> Self {
        effects.history_recording_active &= effects.history_recording_supported;
        let (values, reason) = match settings {
            Ok(settings) => (Some(PreferenceValues::from_settings(settings)), None),
            Err(reason) => (None, Some(bounded_text(reason, MAX_TEXT_BYTES))),
        };
        let available = values.is_some();
        let writable = available && writable;
        Self {
            available,
            writable,
            profile_writable: writable && profile_writable,
            values,
            applied_profile,
            effects,
            reason,
            command: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct PreferenceResult {
    pub id: u64,
    pub key: PreferenceKey,
    pub value: PreferenceValue,
    pub state: CommandState,
    pub code: Option<String>,
    pub detail: Option<String>,
}

#[derive(Default)]
pub struct PreferenceSession {
    last_request_id: u64,
    pending: Option<(u64, PreferenceKey, PreferenceValue)>,
    outcome: Option<PreferenceResult>,
}

impl PreferenceSession {
    pub fn view(&self, mut snapshot: PreferencesSnapshot) -> PreferencesSnapshot {
        snapshot.command = self.outcome.clone();
        if self.pending() {
            snapshot.writable = false;
            snapshot.profile_writable = false;
        }
        snapshot
    }

    /// Use the owning connection's existing challenge and last written sequence.
    /// This lane has its own replay counter and cannot cancel another pending ID.
    pub fn admit(
        &mut self,
        request: PreferenceRequest,
        expected_session: &str,
        last_written_sequence: u64,
        ready: bool,
        view: &PreferencesSnapshot,
    ) -> Option<PreferenceRequest> {
        let pending = self.pending();
        if let Err(rejected) = authorize_envelope(
            CommandEnvelope {
                session: &request.session,
                id: request.id,
                observed_sequence: request.observed_sequence,
            },
            expected_session,
            &mut self.last_request_id,
            last_written_sequence,
            ready,
            pending,
        ) {
            return if rejected.correlatable {
                self.reject(&request, rejected.code, rejected.detail)
            } else {
                None
            };
        }
        if request.validate_types().is_err() {
            return self.reject(
                &request,
                "invalid_type",
                "Preference value type does not match its key",
            );
        }
        let Some(values) = view.values.as_ref().filter(|_| view.available) else {
            return self.reject(
                &request,
                "unavailable",
                "Current preferences are unavailable",
            );
        };
        if !view.writable {
            return self.reject(&request, "read_only", "Preferences are not writable");
        }
        if request.key == PreferenceKey::Profile && !view.profile_writable {
            return self.reject(
                &request,
                "profile_managed",
                "Assessment profile is managed by configuration",
            );
        }
        if values.current_value(request.key) != request.expected_value {
            return self.reject(
                &request,
                "stale",
                "Preference changed since the observed snapshot",
            );
        }
        self.pending = Some((request.id, request.key, request.value));
        self.outcome = Some(PreferenceResult {
            id: request.id,
            key: request.key,
            value: request.value,
            state: CommandState::Accepted,
            code: None,
            detail: None,
        });
        Some(request)
    }

    pub fn finish(
        &mut self,
        id: u64,
        state: CommandState,
        code: Option<&str>,
        detail: Option<String>,
    ) -> bool {
        if state == CommandState::Accepted {
            return false;
        }
        let Some((pending_id, key, value)) = self.pending else {
            return false;
        };
        if id != pending_id {
            return false;
        }
        self.pending = None;
        self.outcome = Some(PreferenceResult {
            id,
            key,
            value,
            state,
            code: code.map(|value| bounded_text(value, MAX_CODE_BYTES)),
            detail: detail.map(|mut value| {
                value.truncate(utf8_prefix(&value, MAX_TEXT_BYTES));
                value
            }),
        });
        true
    }

    pub fn pending(&self) -> bool {
        self.pending.is_some()
    }

    fn reject(
        &mut self,
        request: &PreferenceRequest,
        code: &str,
        detail: &str,
    ) -> Option<PreferenceRequest> {
        // A manually constructed invalid value cannot be published as a valid
        // wire result. The strict decoder rejects it before normal admission.
        if !request.key.accepts(request.value) {
            return None;
        }
        self.outcome = Some(PreferenceResult {
            id: request.id,
            key: request.key,
            value: request.value,
            state: CommandState::Rejected,
            code: Some(bounded_text(code, MAX_CODE_BYTES)),
            detail: Some(bounded_text(detail, MAX_TEXT_BYTES)),
        });
        None
    }
}

/// A successful result means the selected field was saved and read back, not
/// that the collector has applied a profile or that any protection has changed.
pub fn apply_preference(request: &PreferenceRequest, profile_writable: bool) -> Result<Settings> {
    request.validate_types()?;
    ensure!(
        request.key != PreferenceKey::Profile || profile_writable,
        "assessment profile is managed by configuration"
    );
    let saved = Settings::update_checked(|settings| request.apply_to(settings))?;
    let confirmed = Settings::load().context("saved preference could not be read back")?;
    ensure!(
        request.key.current_value(&confirmed) == request.key.current_value(&saved),
        "saved preference changed before readback; requested value is not confirmed"
    );
    Ok(confirmed)
}

#[derive(Debug)]
pub struct PreferenceWork {
    pub ticket: u64,
    pub request: PreferenceRequest,
    pub profile_writable: bool,
}

#[derive(Debug)]
pub struct PreferenceCompletion {
    pub ticket: u64,
    pub result: std::result::Result<Settings, String>,
}

/// One process-owned persistence worker, created lazily by the native owner.
/// Dropping a UI must not drop this worker or cancel an admitted settings write.
pub struct PreferenceWorker {
    requests: std::sync::mpsc::SyncSender<PreferenceWork>,
    responses: std::sync::mpsc::Receiver<PreferenceCompletion>,
    pending: Option<u64>,
    disconnected: bool,
}

impl PreferenceWorker {
    pub fn start() -> Result<Self> {
        let (requests, work) = std::sync::mpsc::sync_channel::<PreferenceWork>(1);
        let (completed, responses) = std::sync::mpsc::sync_channel::<PreferenceCompletion>(1);
        std::thread::Builder::new()
            .name("miccamwatch-preferences".into())
            .spawn(move || {
                while let Ok(work) = work.recv() {
                    let result = apply_preference(&work.request, work.profile_writable)
                        .map_err(|error| bounded_text(&format!("{error:#}"), MAX_TEXT_BYTES));
                    if completed
                        .send(PreferenceCompletion {
                            ticket: work.ticket,
                            result,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .context("cannot start preferences persistence worker")?;
        Ok(Self {
            requests,
            responses,
            pending: None,
            disconnected: false,
        })
    }

    pub fn enqueue(owner: &mut Option<Self>, work: PreferenceWork) -> Result<()> {
        if owner.is_none() {
            *owner = Some(Self::start()?);
        }
        let worker = owner.as_mut().expect("created persistence worker");
        ensure!(
            !worker.disconnected,
            "Preferences worker unavailable; restart the tray"
        );
        ensure!(
            worker.pending.is_none(),
            "A native preference write is pending"
        );
        let ticket = work.ticket;
        worker
            .requests
            .try_send(work)
            .context("native preference queue unavailable")?;
        worker.pending = Some(ticket);
        Ok(())
    }

    pub fn is_pending(&self) -> bool {
        self.pending.is_some()
    }

    pub fn is_available(&self) -> bool {
        !self.disconnected
    }

    pub fn try_complete(&mut self) -> Option<PreferenceCompletion> {
        use std::sync::mpsc::TryRecvError;
        let expected = self.pending?;
        match self.responses.try_recv() {
            Ok(completion) if completion.ticket == expected => {
                self.pending = None;
                Some(completion)
            }
            Err(TryRecvError::Empty) => None,
            Ok(_) | Err(TryRecvError::Disconnected) => {
                self.pending = None;
                self.disconnected = true;
                Some(PreferenceCompletion { ticket: expected,
                    result: Err("Native preference completion unavailable; saved state may be uncertain. Restart the tray before another write".into()) })
            }
        }
    }
}

fn utf8_prefix(value: &str, max_bytes: usize) -> usize {
    let mut end = value.len().min(max_bytes);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    end
}

fn bounded_text(value: &str, max_bytes: usize) -> String {
    value[..utf8_prefix(value, max_bytes)].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const SESSION: &str = "0123456789abcdef0123456789abcdef";

    fn request(id: u64) -> PreferenceRequest {
        PreferenceRequest {
            session: SESSION.into(),
            id,
            action: PreferenceAction::SetPreference,
            observed_sequence: 9,
            key: PreferenceKey::HistoryEnabled,
            value: PreferenceValue::Bool(false),
            expected_value: PreferenceValue::Bool(true),
        }
    }

    fn profile_request(id: u64) -> PreferenceRequest {
        PreferenceRequest {
            key: PreferenceKey::Profile,
            value: PreferenceValue::Profile(PrivacyProfile::Private),
            expected_value: PreferenceValue::Profile(PrivacyProfile::Balanced),
            ..request(id)
        }
    }

    fn snapshot() -> PreferencesSnapshot {
        PreferencesSnapshot::observed(
            Ok(&Settings::default()),
            true,
            true,
            Some(PrivacyProfile::Balanced),
            PreferenceEffects::default(),
        )
    }

    fn admit(
        session: &mut PreferenceSession,
        request: PreferenceRequest,
    ) -> Option<PreferenceRequest> {
        session.admit(request, SESSION, 9, true, &snapshot())
    }

    fn outcome(session: &PreferenceSession) -> PreferenceResult {
        session.view(snapshot()).command.unwrap()
    }

    #[test]
    fn strict_request_decoder_checks_types_keys_actions_and_counters() {
        let valid = serde_json::to_value(request(1)).unwrap();
        assert_eq!(
            serde_json::from_value::<PreferenceRequest>(valid.clone()).unwrap(),
            request(1)
        );
        for (field, value) in [
            ("key", json!("mute_on_lock")),
            ("key", json!("camera")),
            ("action", json!("toggle")),
            ("value", json!("false")),
            ("value", json!("private")),
            ("value", json!(0)),
            ("expected_value", json!(null)),
            ("expected_value", json!("balanced")),
            ("id", json!(0)),
            ("id", json!(-1)),
            ("id", json!(1.5)),
            ("id", json!(u32::MAX as u64 + 1)),
            ("observed_sequence", json!(0)),
            ("observed_sequence", json!(-1)),
            ("observed_sequence", json!(1.5)),
            ("observed_sequence", json!(i64::MAX as u64 + 1)),
        ] {
            let mut payload = valid.clone();
            payload[field] = value;
            assert!(
                serde_json::from_value::<PreferenceRequest>(payload).is_err(),
                "{field}"
            );
        }
        for extra in ["path", "args", "settings", "method", "environment"] {
            let mut payload = valid.clone();
            payload[extra] = json!("injected");
            assert!(serde_json::from_value::<PreferenceRequest>(payload).is_err());
        }
        for missing in [
            "session",
            "id",
            "action",
            "observed_sequence",
            "key",
            "value",
            "expected_value",
        ] {
            let mut payload = valid.clone();
            payload.as_object_mut().unwrap().remove(missing);
            assert!(serde_json::from_value::<PreferenceRequest>(payload).is_err());
        }
        let mut duplicate = serde_json::to_string(&valid).unwrap();
        duplicate.insert_str(1, "\"id\":2,");
        assert!(serde_json::from_str::<PreferenceRequest>(&duplicate).is_err());
    }

    #[test]
    fn finite_profile_values_and_boolean_pairs_decode() {
        for profile in [
            PrivacyProfile::Private,
            PrivacyProfile::Meeting,
            PrivacyProfile::Development,
            PrivacyProfile::Balanced,
        ] {
            let mut request = profile_request(u32::MAX as u64);
            request.value = PreferenceValue::Profile(profile);
            request.observed_sequence = i64::MAX as u64;
            let wire = serde_json::to_value(&request).unwrap();
            assert_eq!(
                serde_json::from_value::<PreferenceRequest>(wire).unwrap(),
                request
            );
        }
        let valid = serde_json::to_value(profile_request(1)).unwrap();
        for (field, value) in [
            ("value", json!(true)),
            ("expected_value", json!(false)),
            ("value", json!("unknown")),
        ] {
            let mut wire = valid.clone();
            wire[field] = value;
            assert!(serde_json::from_value::<PreferenceRequest>(wire).is_err());
        }
        for key in [
            PreferenceKey::NotificationsEnabled,
            PreferenceKey::SoundEnabled,
            PreferenceKey::HistoryEnabled,
        ] {
            let mut request = request(1);
            request.key = key;
            let wire = serde_json::to_value(&request).unwrap();
            assert_eq!(
                serde_json::from_value::<PreferenceRequest>(wire).unwrap(),
                request
            );
        }
    }

    #[test]
    fn observation_preserves_actual_policy_and_normalizes_capabilities() {
        let effects = PreferenceEffects {
            history_recording_active: true,
            ..PreferenceEffects::default()
        };
        let observed = PreferencesSnapshot::observed(
            Ok(&Settings::default()),
            false,
            true,
            Some(PrivacyProfile::Meeting),
            effects,
        );
        assert!(observed.available);
        assert!(!observed.writable);
        assert!(!observed.profile_writable);
        assert_eq!(observed.applied_profile, Some(PrivacyProfile::Meeting));
        assert_eq!(observed.values.unwrap().profile, PrivacyProfile::Balanced);
        assert!(!observed.effects.history_recording_active);
        let unavailable = PreferencesSnapshot::observed(
            Err("unavailable"),
            true,
            true,
            Some(PrivacyProfile::Private),
            effects,
        );
        assert!(!unavailable.available);
        assert!(!unavailable.writable);
        assert!(!unavailable.profile_writable);
        assert!(unavailable.values.is_none());
        assert_eq!(unavailable.applied_profile, Some(PrivacyProfile::Private));
        assert_eq!(
            PreferencesSnapshot::default().effects,
            PreferenceEffects::default()
        );
        assert!(!PreferencesSnapshot::default().available);
        assert!(PreferencesSnapshot::default().values.is_none());
    }

    #[test]
    fn snapshot_reason_is_bounded_at_utf8_boundary() {
        let reason = format!("{}é", "a".repeat(1023));
        let observed = PreferencesSnapshot::observed(
            Err(&reason),
            true,
            true,
            None,
            PreferenceEffects::default(),
        );
        assert_eq!(observed.reason.unwrap(), "a".repeat(1023));
    }

    #[test]
    fn wrong_session_does_not_consume_authenticated_id() {
        let mut session = PreferenceSession::default();
        let mut bad = request(1);
        bad.session = "another child".into();
        assert!(admit(&mut session, bad).is_none());
        assert_eq!(outcome(&session).code.as_deref(), Some("unauthorized"));
        assert!(!session.pending());
        assert!(admit(&mut session, request(1)).is_some());
        assert!(session.pending());
    }

    #[test]
    fn live_connection_session_is_the_only_authority() {
        let mut session = PreferenceSession::default();
        assert!(
            session
                .admit(request(1), "different-live-session", 9, true, &snapshot())
                .is_none()
        );
        assert_eq!(outcome(&session).code.as_deref(), Some("unauthorized"));
        assert!(
            session
                .admit(request(1), SESSION, 9, true, &snapshot())
                .is_some()
        );
        let mut independent = PreferenceSession::default();
        assert!(admit(&mut independent, request(1)).is_some());
    }

    #[test]
    fn invalid_counters_never_publish_or_replace_results() {
        let mut session = PreferenceSession::default();
        assert!(admit(&mut session, request(1)).is_some());
        let accepted = outcome(&session);
        for (id, sequence) in [
            (0, 9),
            (u32::MAX as u64 + 1, 9),
            (2, 0),
            (2, i64::MAX as u64 + 1),
        ] {
            let mut invalid = request(id);
            invalid.observed_sequence = sequence;
            assert!(admit(&mut session, invalid).is_none());
            assert_eq!(outcome(&session), accepted);
            assert!(session.pending());
        }
        assert!(session.finish(1, CommandState::Succeeded, None, None));
        assert!(admit(&mut session, request(2)).is_some());
    }

    #[test]
    fn readiness_future_sequence_and_replay_are_rejected() {
        let mut session = PreferenceSession::default();
        assert!(
            session
                .admit(request(1), SESSION, 9, false, &snapshot())
                .is_none()
        );
        assert_eq!(outcome(&session).code.as_deref(), Some("unavailable"));
        assert!(admit(&mut session, request(1)).is_none());
        assert_eq!(outcome(&session).code.as_deref(), Some("replay"));
        let mut future = request(2);
        future.observed_sequence = 10;
        assert!(admit(&mut session, future).is_none());
        assert_eq!(outcome(&session).code.as_deref(), Some("stale"));
        assert!(admit(&mut session, request(3)).is_some());
    }

    #[test]
    fn pending_rejections_cannot_steal_or_clear_original_correlation() {
        let mut session = PreferenceSession::default();
        assert!(admit(&mut session, request(1)).is_some());
        let masked = session.view(snapshot());
        assert!(!masked.writable);
        assert!(!masked.profile_writable);
        assert_eq!(masked.values, snapshot().values);
        assert!(admit(&mut session, profile_request(2)).is_none());
        assert_eq!(outcome(&session).code.as_deref(), Some("busy"));
        assert!(session.pending());
        assert!(!session.finish(2, CommandState::Succeeded, None, None));
        assert!(!session.finish(1, CommandState::Accepted, None, None));
        assert!(session.pending());
        let mut unauthorized = request(3);
        unauthorized.session.clear();
        assert!(admit(&mut session, unauthorized).is_none());
        assert!(session.pending());
        assert!(session.finish(1, CommandState::Succeeded, None, None));
        let result = outcome(&session);
        assert_eq!(result.id, 1);
        assert_eq!(result.key, PreferenceKey::HistoryEnabled);
        assert_eq!(result.value, PreferenceValue::Bool(false));
        assert_eq!(result.state, CommandState::Succeeded);
        assert!(!session.pending());
        assert_eq!(outcome(&session), result);
        assert!(session.view(snapshot()).writable);
        assert!(!session.finish(1, CommandState::Failed, None, None));
        assert_eq!(outcome(&session), result);
    }

    #[test]
    fn unavailable_read_only_and_managed_profile_gates_are_independent() {
        for view in [
            PreferencesSnapshot::default(),
            PreferencesSnapshot {
                available: true,
                writable: true,
                ..PreferencesSnapshot::default()
            },
        ] {
            let mut session = PreferenceSession::default();
            assert!(session.admit(request(1), SESSION, 9, true, &view).is_none());
            assert_eq!(outcome(&session).code.as_deref(), Some("unavailable"));
        }
        let mut read_only = snapshot();
        read_only.writable = false;
        let mut session = PreferenceSession::default();
        assert!(
            session
                .admit(request(1), SESSION, 9, true, &read_only)
                .is_none()
        );
        assert_eq!(outcome(&session).code.as_deref(), Some("read_only"));
        let mut managed = snapshot();
        managed.profile_writable = false;
        assert!(
            session
                .admit(profile_request(2), SESSION, 9, true, &managed)
                .is_none()
        );
        assert_eq!(outcome(&session).code.as_deref(), Some("profile_managed"));
        assert!(
            session
                .admit(request(3), SESSION, 9, true, &managed)
                .is_some()
        );
    }

    #[test]
    fn stale_expectations_and_internal_type_mismatches_do_not_become_pending() {
        let mut session = PreferenceSession::default();
        let mut stale = request(1);
        stale.expected_value = PreferenceValue::Bool(false);
        assert!(admit(&mut session, stale).is_none());
        assert_eq!(outcome(&session).code.as_deref(), Some("stale"));
        let mut invalid = request(2);
        invalid.expected_value = PreferenceValue::Profile(PrivacyProfile::Balanced);
        assert!(admit(&mut session, invalid).is_none());
        assert_eq!(outcome(&session).code.as_deref(), Some("invalid_type"));
        let last = outcome(&session);
        let mut invalid_value = request(3);
        invalid_value.value = PreferenceValue::Profile(PrivacyProfile::Private);
        assert!(admit(&mut session, invalid_value).is_none());
        assert_eq!(outcome(&session), last);
        assert!(!session.pending());
        assert!(admit(&mut session, request(4)).is_some());
    }

    #[test]
    fn all_final_states_retain_results_and_bound_utf8_diagnostics() {
        for state in [
            CommandState::Succeeded,
            CommandState::Failed,
            CommandState::Rejected,
            CommandState::Uncertain,
        ] {
            let mut session = PreferenceSession::default();
            assert!(admit(&mut session, profile_request(1)).is_some());
            let code = format!("{}é", "a".repeat(63));
            let detail = format!("{}🦀", "b".repeat(1023));
            assert!(session.finish(1, state, Some(&code), Some(detail)));
            let result = outcome(&session);
            assert_eq!(result.state, state);
            assert_eq!(result.key, PreferenceKey::Profile);
            assert_eq!(
                result.value,
                PreferenceValue::Profile(PrivacyProfile::Private)
            );
            assert_eq!(result.code.unwrap(), "a".repeat(63));
            assert_eq!(result.detail.unwrap(), "b".repeat(1023));
            assert!(!session.pending());
            assert!(admit(&mut session, request(1)).is_none());
            assert_eq!(outcome(&session).code.as_deref(), Some("replay"));
        }
    }

    fn protected_settings() -> Settings {
        Settings {
            show_ready: true,
            mute_on_lock: true,
            block_camera_on_lock: true,
            restore_on_unlock: false,
            pause_notifications_until: Some(
                chrono::DateTime::parse_from_rfc3339("2025-01-02T03:04:05Z")
                    .unwrap()
                    .with_timezone(&chrono::Utc),
            ),
            ..Settings::default()
        }
    }

    #[test]
    fn apply_to_changes_only_selected_latest_field() {
        for key in [
            PreferenceKey::NotificationsEnabled,
            PreferenceKey::SoundEnabled,
            PreferenceKey::HistoryEnabled,
            PreferenceKey::Profile,
        ] {
            let mut settings = protected_settings();
            let before = serde_json::to_value(&settings).unwrap();
            let expected_value = key.current_value(&settings);
            let value = match expected_value {
                PreferenceValue::Bool(value) => PreferenceValue::Bool(!value),
                PreferenceValue::Profile(_) => {
                    PreferenceValue::Profile(PrivacyProfile::Development)
                }
                _ => unreachable!("this case covers boolean and profile fields"),
            };
            let request = PreferenceRequest {
                key,
                value,
                expected_value,
                ..request(1)
            };
            request.apply_to(&mut settings).unwrap();
            assert_eq!(key.current_value(&settings), value);
            let mut expected = before;
            let field = serde_json::to_value(key).unwrap();
            expected[field.as_str().unwrap()] = serde_json::to_value(value).unwrap();
            assert_eq!(serde_json::to_value(&settings).unwrap(), expected);
        }
    }

    #[test]
    fn apply_to_rejected_stale_or_wrong_types_never_mutates_settings() {
        for mut invalid in [request(1), profile_request(1)] {
            let mut settings = protected_settings();
            let before = serde_json::to_value(&settings).unwrap();
            invalid.expected_value = match invalid.expected_value {
                PreferenceValue::Bool(value) => PreferenceValue::Bool(!value),
                PreferenceValue::Profile(_) => PreferenceValue::Profile(PrivacyProfile::Meeting),
                _ => unreachable!("this case covers boolean and profile fields"),
            };
            assert!(invalid.apply_to(&mut settings).is_err());
            assert_eq!(serde_json::to_value(&settings).unwrap(), before);
            invalid.expected_value = invalid.key.current_value(&settings);
            invalid.value = match invalid.value {
                PreferenceValue::Bool(_) => PreferenceValue::Profile(PrivacyProfile::Private),
                PreferenceValue::Profile(_) => PreferenceValue::Bool(false),
                _ => unreachable!("this case covers boolean and profile fields"),
            };
            assert!(invalid.apply_to(&mut settings).is_err());
            assert_eq!(serde_json::to_value(&settings).unwrap(), before);
        }
    }

    #[test]
    fn apply_to_merges_unrelated_changes_and_checks_the_latest_field() {
        let request = request(1);
        let mut latest = protected_settings();
        latest.sound_enabled = true;
        latest.profile = PrivacyProfile::Meeting;
        request.apply_to(&mut latest).unwrap();
        assert!(latest.sound_enabled);
        assert_eq!(latest.profile, PrivacyProfile::Meeting);
        assert!(!latest.history_enabled);
        assert!(latest.mute_on_lock);
        assert!(request.apply_to(&mut latest).is_err());
        assert!(!latest.history_enabled);
    }

    fn pause_request(action: NotificationPauseAction, until: Option<DateTime<Utc>>) -> PreferenceRequest {
        PreferenceRequest {
            key: PreferenceKey::NotificationPause,
            value: PreferenceValue::PauseAction(action),
            expected_value: PreferenceValue::PauseUntil(until),
            ..request(1)
        }
    }

    #[test]
    fn pause_contract_accepts_only_server_actions_and_observed_deadlines() {
        let pause = pause_request(NotificationPauseAction::Pause30Minutes, None);
        let wire = serde_json::to_value(&pause).unwrap();
        assert_eq!(serde_json::from_value::<PreferenceRequest>(wire.clone()).unwrap(), pause);
        for (field, value) in [
            ("value", json!(null)), ("value", json!(true)),
            ("value", json!("2026-10-30T08:00:00Z")), ("value", json!(60)),
            ("expected_value", json!(true)), ("expected_value", json!("resume")),
            ("expected_value", json!("not-a-timestamp")),
        ] {
            let mut invalid = wire.clone();
            invalid[field] = value;
            assert!(serde_json::from_value::<PreferenceRequest>(invalid).is_err());
        }
        let mut missing = wire;
        missing.as_object_mut().unwrap().remove("expected_value");
        assert!(serde_json::from_value::<PreferenceRequest>(missing).is_err());
    }

    #[test]
    fn pause_uses_server_clock_preserves_other_settings_and_checks_exact_deadline() {
        let now: DateTime<Utc> = "2026-10-30T08:00:00.123456789Z".parse().unwrap();
        let mut settings = protected_settings();
        settings.notifications_enabled = false;
        let before = serde_json::to_value(&settings).unwrap();
        pause_request(NotificationPauseAction::Pause30Minutes, settings.pause_notifications_until)
            .apply_with_clock(&mut settings, || now).unwrap();
        let until = now + Duration::minutes(30);
        assert_eq!(settings.pause_notifications_until, Some(until));
        let mut expected = before;
        expected["pause_notifications_until"] = json!(until);
        assert_eq!(serde_json::to_value(&settings).unwrap(), expected);

        let resume = pause_request(NotificationPauseAction::Resume, Some(until));
        settings.pause_notifications_until = Some(until + Duration::nanoseconds(1));
        let concurrent = serde_json::to_value(&settings).unwrap();
        assert!(resume.apply_with_clock(&mut settings, || now).is_err());
        assert_eq!(serde_json::to_value(&settings).unwrap(), concurrent);
        pause_request(NotificationPauseAction::Resume, settings.pause_notifications_until)
            .apply_with_clock(&mut settings, || panic!("resume must not need a clock")).unwrap();
        assert!(settings.pause_notifications_until.is_none());
        assert!(!settings.notifications_enabled);
    }

    #[test]
    fn pause_admission_rejects_stale_deadlines_and_readonly_connections() {
        let until: DateTime<Utc> = "2026-10-30T08:00:00Z".parse().unwrap();
        let settings = Settings { pause_notifications_until: Some(until), ..Settings::default() };
        let mut view = PreferencesSnapshot::observed(
            Ok(&settings), true, true, None, PreferenceEffects::default());
        let mut session = PreferenceSession::default();
        let mut stale = pause_request(NotificationPauseAction::Resume, None);
        assert!(session.admit(stale.clone(), SESSION, 9, true, &view).is_none());
        assert_eq!(outcome(&session).code.as_deref(), Some("stale"));
        assert!(!session.pending());
        stale.id = 2;
        stale.expected_value = PreferenceValue::PauseUntil(Some(until));
        view.writable = false;
        assert!(session.admit(stale, SESSION, 9, true, &view).is_none());
        assert_eq!(outcome(&session).code.as_deref(), Some("read_only"));
        assert!(!session.pending());
    }

    #[test]
    fn overflowing_pause_clock_never_mutates_saved_intent() {
        let mut settings = protected_settings();
        let before = serde_json::to_value(&settings).unwrap();
        assert!(pause_request(NotificationPauseAction::Pause30Minutes, settings.pause_notifications_until)
            .apply_with_clock(&mut settings, || DateTime::<Utc>::MAX_UTC).is_err());
        assert_eq!(serde_json::to_value(&settings).unwrap(), before);
    }

    #[test]
    fn managed_profile_rejects_before_any_store_access() {
        // This must return before resolving the settings path or opening a file.
        // No test environment or filesystem is needed to exercise the gate.
        assert!(apply_preference(&profile_request(1), false).is_err());
    }
}
