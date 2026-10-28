//! Portable admission and outcome state for one owned desktop connection.
//!
//! The transport must call `admit` only for its current live child, using that
//! child's readiness and last successfully written snapshot sequence. A session
//! challenge is not a substitute for the transport's child/pipe ownership checks.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

const MAX_TEXT_BYTES: usize = 1024;
const MAX_CODE_BYTES: usize = 64;
const MAX_WIRE_COUNTER: u64 = i64::MAX as u64;
const CONFIRMATION: &str = "sdk_mute_all_inputs";
const RELEASE_CONFIRMATION: &str = "release_all_active_inputs";

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommandAction {
    SetMicrophoneProtection,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UiCommand {
    pub session: String,
    #[serde(deserialize_with = "deserialize_request_id")]
    pub id: u64,
    pub action: CommandAction,
    pub requested: bool,
    pub expected_requested: bool,
    #[serde(deserialize_with = "deserialize_generation")]
    pub expected_generation: u64,
    #[serde(deserialize_with = "deserialize_sequence")]
    pub observed_sequence: u64,
    pub confirmation: String,
}

#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
pub struct MicrophoneCapability {
    pub can_enable: bool,
    pub can_release: bool,
    pub current_requested: Option<bool>,
    pub generation: u64,
    pub reason: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommandState {
    Accepted,
    Succeeded,
    Failed,
    Rejected,
    Uncertain,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct CommandResult {
    pub id: u64,
    pub requested: bool,
    pub state: CommandState,
    pub code: Option<String>,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
pub struct Controls {
    pub session: String,
    pub microphone: MicrophoneCapability,
    pub command: Option<CommandResult>,
}

pub struct CommandSession {
    session: String,
    last_request_id: u64,
    pending: Option<(u64, bool)>,
    outcome: Option<CommandResult>,
}

pub(super) struct CommandEnvelope<'a> {
    pub session: &'a str,
    pub id: u64,
    pub observed_sequence: u64,
}

pub(super) struct AdmissionRejection {
    pub code: &'static str,
    pub detail: &'static str,
    pub correlatable: bool,
}

/// Shared private-pipe admission; replay counters and pending work are scoped
/// to each bounded lane. Preferences cannot mutate protections or permissions.
pub(super) fn authorize_envelope(
    request: CommandEnvelope<'_>,
    expected_session: &str,
    last_request_id: &mut u64,
    last_written_sequence: u64,
    ready: bool,
    pending: bool,
) -> std::result::Result<(), AdmissionRejection> {
    let reject = |code, detail, correlatable| AdmissionRejection {
        code, detail, correlatable,
    };
    if request.id == 0 || request.id > u32::MAX as u64
        || request.observed_sequence == 0 || request.observed_sequence > MAX_WIRE_COUNTER
    {
        return Err(reject("invalid_counter", "Invalid command counters", false));
    }
    if request.session != expected_session {
        return Err(reject("unauthorized", "Command session does not match", true));
    }
    if request.id <= *last_request_id {
        return Err(reject("replay", "Command ID has already been consumed", true));
    }
    *last_request_id = request.id;
    if !ready || last_written_sequence == 0 || last_written_sequence > MAX_WIRE_COUNTER {
        return Err(reject("unavailable", "Desktop connection is not ready", true));
    }
    if request.observed_sequence > last_written_sequence {
        return Err(reject("stale", "Command refers to an unwritten snapshot", true));
    }
    if pending {
        return Err(reject("busy", "A command is already pending in this lane", true));
    }
    Ok(())
}

impl CommandSession {
    pub fn new(session: String) -> Result<Self> {
        ensure!(
            session.len() == 32
                && session
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
            "invalid desktop command session"
        );
        Ok(Self {
            session,
            last_request_id: 0,
            pending: None,
            outcome: None,
        })
    }

    pub fn controls(&self, mut capability: MicrophoneCapability) -> Controls {
        // Invalid/unknown internal observations must not advertise an action.
        if capability.generation > MAX_WIRE_COUNTER {
            capability.generation = MAX_WIRE_COUNTER;
            capability.can_enable = false;
            capability.can_release = false;
            capability.reason = Some("Microphone generation is unavailable".into());
        }
        if capability.current_requested.is_none() {
            capability.can_enable = false;
            capability.can_release = false;
        }
        if self.pending() {
            capability.can_enable = false;
            capability.can_release = false;
            capability.reason = Some("A microphone command is pending".into());
        }
        capability.reason = capability
            .reason
            .map(|value| clipped(value, MAX_TEXT_BYTES));
        Controls {
            session: self.session.clone(),
            microphone: capability,
            command: self.outcome.clone(),
        }
    }

    /// Return a ticket at most once; only the owner may dispatch it to the worker.
    /// Rejections replace the displayed outcome, never the active pending ticket.
    pub fn admit(
        &mut self,
        request: UiCommand,
        last_written_sequence: u64,
        ready: bool,
        capability: &MicrophoneCapability,
    ) -> Option<UiCommand> {
        if request.expected_generation > MAX_WIRE_COUNTER {
            return None;
        }
        let pending = self.pending();
        if let Err(rejected) = authorize_envelope(
            CommandEnvelope { session: &request.session, id: request.id,
                observed_sequence: request.observed_sequence },
            &self.session, &mut self.last_request_id, last_written_sequence, ready, pending,
        ) {
            return if rejected.correlatable {
                self.reject(&request, rejected.code, rejected.detail)
            } else {
                None
            };
        }
        let Some(current_requested) = capability.current_requested else {
            return self.reject(
                &request,
                "unavailable",
                "Current microphone request is unknown",
            );
        };
        if capability.generation > MAX_WIRE_COUNTER
            || request.expected_generation != capability.generation
            || request.expected_requested != current_requested
        {
            return self.reject(&request, "stale", "Microphone state has changed");
        }
        let enabled = match request.action {
            CommandAction::SetMicrophoneProtection if request.requested => capability.can_enable,
            CommandAction::SetMicrophoneProtection => capability.can_release,
        };
        if !enabled {
            return self.reject(
                &request,
                "read_only",
                "Requested microphone action is not enabled",
            );
        }
        let confirmation = if request.requested {
            CONFIRMATION
        } else {
            RELEASE_CONFIRMATION
        };
        if request.confirmation != confirmation {
            return self.reject(
                &request,
                "confirmation_required",
                "Explicit SDK microphone confirmation is required",
            );
        }
        self.pending = Some((request.id, request.requested));
        self.outcome = Some(CommandResult {
            id: request.id,
            requested: request.requested,
            state: CommandState::Accepted,
            code: None,
            detail: None,
        });
        Some(request)
    }

    /// Store only a final result for the still-pending ticket. This reports the
    /// worker's readback outcome; it never infers actual protection from intent.
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
        let Some((pending_id, requested)) = self.pending else {
            return false;
        };
        if id != pending_id {
            return false;
        }
        self.pending = None;
        self.outcome = Some(CommandResult {
            id,
            requested,
            state,
            code: code.map(|value| value[..utf8_prefix(value, MAX_CODE_BYTES)].to_owned()),
            detail: detail.map(|value| clipped(value, MAX_TEXT_BYTES)),
        });
        true
    }

    pub(super) fn session(&self) -> &str {
        &self.session
    }

    pub fn pending(&self) -> bool {
        self.pending.is_some()
    }

    fn reject(&mut self, request: &UiCommand, code: &str, detail: &str) -> Option<UiCommand> {
        self.outcome = Some(CommandResult {
            id: request.id,
            requested: request.requested,
            state: CommandState::Rejected,
            code: Some(code.to_owned()),
            detail: Some(detail.to_owned()),
        });
        None
    }
}

pub(super) fn deserialize_request_id<'de, D: serde::Deserializer<'de>>(
    decoder: D,
) -> std::result::Result<u64, D::Error> {
    let value = u32::deserialize(decoder)?;
    if value == 0 {
        return Err(serde::de::Error::custom("command ID must be positive"));
    }
    Ok(u64::from(value))
}

fn deserialize_generation<'de, D: serde::Deserializer<'de>>(
    decoder: D,
) -> std::result::Result<u64, D::Error> {
    let value = u64::deserialize(decoder)?;
    if value > MAX_WIRE_COUNTER {
        return Err(serde::de::Error::custom(
            "command counter exceeds signed 64-bit range",
        ));
    }
    Ok(value)
}

pub(super) fn deserialize_sequence<'de, D: serde::Deserializer<'de>>(
    decoder: D,
) -> std::result::Result<u64, D::Error> {
    let value = deserialize_generation(decoder)?;
    if value == 0 {
        return Err(serde::de::Error::custom(
            "observed sequence must be positive",
        ));
    }
    Ok(value)
}

fn utf8_prefix(value: &str, max_bytes: usize) -> usize {
    let mut end = value.len().min(max_bytes);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    end
}

fn clipped(mut value: String, max_bytes: usize) -> String {
    value.truncate(utf8_prefix(&value, max_bytes));
    // A short visible string must not retain an arbitrarily large allocation.
    if value.capacity() > max_bytes * 2 {
        value.shrink_to_fit();
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const SESSION: &str = "0123456789abcdef0123456789abcdef";

    fn session() -> CommandSession {
        CommandSession::new(SESSION.into()).unwrap()
    }

    fn capability() -> MicrophoneCapability {
        MicrophoneCapability {
            can_enable: true,
            can_release: true,
            current_requested: Some(false),
            generation: 7,
            reason: None,
        }
    }

    fn request(id: u64, requested: bool) -> UiCommand {
        UiCommand {
            session: SESSION.into(),
            id,
            action: CommandAction::SetMicrophoneProtection,
            requested,
            expected_requested: false,
            expected_generation: 7,
            observed_sequence: 9,
            confirmation: if requested {
                CONFIRMATION
            } else {
                RELEASE_CONFIRMATION
            }
            .into(),
        }
    }

    fn code(session: &CommandSession) -> String {
        session
            .controls(capability())
            .command
            .unwrap()
            .code
            .unwrap()
    }

    #[test]
    fn strict_decoding_cannot_smuggle_device_or_privilege_parameters() {
        let valid = serde_json::to_value(request(1, true)).unwrap();
        for extra in [
            "device_id",
            "path",
            "shell",
            "user_id",
            "privilege",
            "endpoint",
        ] {
            let mut payload = valid.clone();
            payload[extra] = json!("injected");
            assert!(serde_json::from_value::<UiCommand>(payload).is_err());
        }
        for action in [
            "toggle",
            "set_camera_protection",
            "SetMicrophoneProtection",
            "shell",
        ] {
            let mut payload = valid.clone();
            payload["action"] = json!(action);
            assert!(serde_json::from_value::<UiCommand>(payload).is_err());
        }
        for (field, value) in [
            ("requested", json!("true")),
            ("expected_requested", json!(null)),
            ("id", json!(-1)),
            ("id", json!(1.5)),
            ("id", json!(0)),
            ("id", json!(u32::MAX as u64 + 1)),
            ("expected_generation", json!(MAX_WIRE_COUNTER + 1)),
            ("observed_sequence", json!(0)),
            ("observed_sequence", json!(MAX_WIRE_COUNTER + 1)),
        ] {
            let mut payload = valid.clone();
            payload[field] = value;
            assert!(serde_json::from_value::<UiCommand>(payload).is_err());
        }
        let mut duplicate = serde_json::to_string(&valid).unwrap();
        duplicate.insert_str(1, "\"id\":2,");
        assert!(serde_json::from_str::<UiCommand>(&duplicate).is_err());
    }

    #[test]
    fn session_challenge_is_canonical_and_not_logged_in_errors() {
        for invalid in [
            "",
            "0123456789abcdef",
            "0123456789ABCDEF0123456789ABCDEF",
            "g123456789abcdef0123456789abcdef",
        ] {
            let error = CommandSession::new(invalid.into())
                .err()
                .unwrap()
                .to_string();
            assert_eq!(error, "invalid desktop command session");
        }
        assert!(CommandSession::new(SESSION.into()).is_ok());
    }

    #[test]
    fn read_only_and_target_specific_permissions_never_dispatch() {
        let mut state = session();
        assert!(
            state
                .admit(request(1, true), 9, true, &MicrophoneCapability::default())
                .is_none()
        );
        assert!(!state.pending());
        let mut cap = capability();
        cap.can_enable = false;
        assert!(state.admit(request(2, true), 9, true, &cap).is_none());
        assert_eq!(code(&state), "read_only");
        assert!(state.admit(request(3, false), 9, true, &cap).is_some());
        assert!(state.finish(3, CommandState::Succeeded, None, None));
        cap.can_enable = true;
        cap.can_release = false;
        assert!(state.admit(request(4, false), 9, true, &cap).is_none());
        assert_eq!(code(&state), "read_only");
        assert!(state.admit(request(5, true), 9, true, &cap).is_some());
    }

    #[test]
    fn readiness_written_snapshot_and_exact_confirmation_are_required() {
        for (ready, written, observed, confirmation) in [
            (false, 9, 9, CONFIRMATION),
            (true, 0, 9, CONFIRMATION),
            (true, 9, 10, CONFIRMATION),
            (true, 9, 9, ""),
            (true, 9, 9, "sdk_mute_all_inputs "),
            (true, 9, 9, "SDK_MUTE_ALL_INPUTS"),
        ] {
            let mut state = session();
            let mut command = request(1, true);
            command.observed_sequence = observed;
            command.confirmation = confirmation.into();
            assert!(
                state
                    .admit(command, written, ready, &capability())
                    .is_none()
            );
            assert!(!state.pending());
            assert_eq!(
                state.controls(capability()).command.unwrap().state,
                CommandState::Rejected
            );
        }
        for (requested, wrong_confirmation) in [(true, RELEASE_CONFIRMATION), (false, CONFIRMATION)]
        {
            let mut state = session();
            let mut command = request(1, requested);
            command.confirmation = wrong_confirmation.into();
            assert!(state.admit(command, 9, true, &capability()).is_none());
            assert_eq!(code(&state), "confirmation_required");
            assert!(
                state
                    .admit(request(2, requested), 9, true, &capability())
                    .is_some()
            );
        }
    }

    #[test]
    fn stale_generation_or_requested_state_requires_new_user_intent() {
        for (generation, expected) in [(6, false), (8, false), (7, true)] {
            let mut state = session();
            let mut command = request(1, true);
            command.expected_generation = generation;
            command.expected_requested = expected;
            assert!(state.admit(command, 9, true, &capability()).is_none());
            assert_eq!(code(&state), "stale");
            assert!(
                state
                    .admit(request(1, true), 9, true, &capability())
                    .is_none()
            );
            assert_eq!(code(&state), "replay");
            assert!(
                state
                    .admit(request(2, true), 10, true, &capability())
                    .is_some()
            );
        }
    }

    #[test]
    fn alien_session_cannot_poison_counter_or_complete_live_work() {
        let mut state = session();
        let mut alien = request(u32::MAX as u64, true);
        alien.session = "fedcba9876543210fedcba9876543210".into();
        assert!(state.admit(alien, 9, true, &capability()).is_none());
        assert_eq!(code(&state), "unauthorized");
        assert!(
            state
                .admit(request(1, true), 9, true, &capability())
                .is_some()
        );
        assert!(!state.finish(u32::MAX as u64, CommandState::Succeeded, None, None));
        assert!(state.pending());
        assert!(state.finish(1, CommandState::Failed, Some("worker_stale"), None));
    }

    #[test]
    fn competing_malformed_and_replay_requests_never_displace_pending_ticket() {
        let mut state = session();
        assert!(
            state
                .admit(request(1, true), 9, true, &capability())
                .is_some()
        );
        assert!(
            state
                .admit(request(2, false), 9, true, &capability())
                .is_none()
        );
        assert_eq!(code(&state), "busy");
        let mut malformed = request(3, false);
        malformed.observed_sequence = u64::MAX;
        assert!(state.admit(malformed, 9, true, &capability()).is_none());
        assert_eq!(code(&state), "busy");
        assert!(
            state
                .admit(request(1, true), 9, true, &capability())
                .is_none()
        );
        assert_eq!(code(&state), "replay");
        let controls = state.controls(capability());
        assert!(!controls.microphone.can_enable && !controls.microphone.can_release);
        assert!(state.pending());
        assert!(!state.finish(2, CommandState::Succeeded, None, None));
        assert!(!state.finish(1, CommandState::Accepted, None, None));
        assert!(state.pending());
        assert!(state.finish(1, CommandState::Succeeded, None, None));
        assert!(!state.pending());
        assert!(!state.finish(1, CommandState::Failed, None, None));
        assert!(
            state
                .admit(request(2, false), 9, true, &capability())
                .is_none()
        );
        assert!(
            state
                .admit(request(4, false), 9, true, &capability())
                .is_some()
        );
    }

    #[test]
    fn strict_numeric_bounds_cannot_wrap_or_poison_valid_ids() {
        let mut state = session();
        for id in [0, u32::MAX as u64 + 1, u64::MAX] {
            assert!(
                state
                    .admit(request(id, true), 9, true, &capability())
                    .is_none()
            );
            assert!(!state.pending());
            assert!(state.controls(capability()).command.is_none());
        }
        assert!(
            state
                .admit(request(1, true), 9, true, &capability())
                .is_some()
        );
        assert!(state.finish(1, CommandState::Rejected, Some("queue_full"), None));
        let retained = state.controls(capability()).command;
        for (generation, observed) in [(MAX_WIRE_COUNTER + 1, 9), (7, MAX_WIRE_COUNTER + 1), (7, 0)]
        {
            let mut command = request(u32::MAX as u64, true);
            command.expected_generation = generation;
            command.observed_sequence = observed;
            assert!(state.admit(command, 9, true, &capability()).is_none());
            assert!(!state.pending());
            assert_eq!(state.controls(capability()).command, retained);
        }
        let mut cap = capability();
        cap.generation = MAX_WIRE_COUNTER;
        let mut command = request(u32::MAX as u64, true);
        command.expected_generation = MAX_WIRE_COUNTER;
        command.observed_sequence = MAX_WIRE_COUNTER;
        assert!(state.admit(command, MAX_WIRE_COUNTER, true, &cap).is_some());
        assert!(state.finish(u32::MAX as u64, CommandState::Succeeded, None, None));
        assert!(
            state
                .admit(request(1, false), 9, true, &capability())
                .is_none()
        );
        assert_eq!(code(&state), "replay");
    }

    #[test]
    fn partial_uncertainty_never_rewrites_observed_protection_or_implies_release() {
        let mut state = session();
        let mut cap = capability();
        cap.current_requested = Some(true);
        let mut release = request(1, false);
        release.expected_requested = true;
        assert!(state.admit(release, 9, true, &cap).is_some());
        assert!(state.finish(
            1,
            CommandState::Uncertain,
            Some("partial_readback"),
            Some("One input could not be read back".into())
        ));
        let controls = state.controls(cap.clone());
        assert_eq!(controls.microphone.current_requested, Some(true));
        assert_eq!(controls.command.unwrap().state, CommandState::Uncertain);
        // Coalescing/publication does not consume the result; an old completion
        // cannot turn uncertainty into success after the ticket was retired.
        assert!(!state.finish(1, CommandState::Succeeded, None, None));
        assert_eq!(
            state.controls(cap).command.unwrap().state,
            CommandState::Uncertain
        );
    }

    #[test]
    fn outcome_and_disabled_reason_are_bounded_at_unicode_boundaries() {
        let mut state = session();
        assert!(
            state
                .admit(request(1, true), 9, true, &capability())
                .is_some()
        );
        let text = format!("{}🦀{}", "x".repeat(1023), "tail".repeat(1000));
        let code = format!("{}é", "c".repeat(63));
        assert!(state.finish(1, CommandState::Failed, Some(&code), Some(text.clone())));
        let mut cap = capability();
        cap.reason = Some(text);
        let controls = state.controls(cap);
        let result = controls.command.unwrap();
        assert_eq!(result.detail.unwrap(), "x".repeat(1023));
        assert_eq!(result.code.unwrap(), "c".repeat(63));
        assert_eq!(controls.microphone.reason.unwrap(), "x".repeat(1023));
        assert_eq!(Controls::default().microphone.current_requested, None);
        let unknown = state.controls(MicrophoneCapability {
            can_enable: true,
            can_release: true,
            ..MicrophoneCapability::default()
        });
        assert!(!unknown.microphone.can_enable && !unknown.microphone.can_release);
    }
}
