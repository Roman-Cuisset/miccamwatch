//! On-demand native operations. Presentation never owns device authority.
use anyhow::{Context as _, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::{Duration, Instant}};
use crate::{config::Policy, model::MicrophoneMuteState, platform::PlatformMonitor, settings::{PrivacyProfile, Settings}};

#[derive(Clone, Debug, Default)]
pub struct Context {
    pub policy: Option<Arc<Policy>>,
    pub explicit_policy: bool,
    pub applied_profile: Option<PrivacyProfile>,
    pub operational: bool,
    pub data_writable: bool,
    #[cfg(windows)]
    pub camera_owner: Option<super::camera_owner::Client>,
}
impl Context {
    fn policy(&self) -> Result<Policy> {
        let mut policy = self.policy.as_deref().context("The native owner's applied policy is unavailable")?.clone();
        if !self.explicit_policy {
            policy.profile = self.applied_profile.context("The observer has not confirmed its applied profile")?.assessment_profile();
        }
        Ok(policy)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Controls {},
    Prepare { control: Control },
    Commit { token: String, confirmation: String },
    Devices {},
    Application { pid: u32 },
    Paths {},
    Policy {},
    ValidatePolicy { path: Option<String> },
    ClearHistory { confirmation: String },
    Maintenance {},
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Control {
    MicrophoneMute,
    MicrophoneRestore,
    CameraBlock,
    CameraAllow,
    CameraResume,
    CameraRestoreLegacy { instance_id: String },
    CameraInspect,
    LockPolicy { microphone: bool, camera: bool, restore: bool },
    Terminate { pid: u32 },
}
impl Request {
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Prepare { control: Control::Terminate { pid } } | Self::Application { pid } => ensure!(*pid > 1, "Invalid process identity"),
            Self::Prepare { control: Control::CameraRestoreLegacy { instance_id } } =>
                ensure!(!instance_id.trim().is_empty() && instance_id.len() <= 2048 && !instance_id.chars().any(char::is_control), "An exact native camera instance ID is required"),
            Self::ValidatePolicy { path: Some(path) } => ensure!(!path.is_empty() && path.len() <= 4096 && !path.chars().any(char::is_control), "Invalid policy file path"),
            Self::Commit { token, confirmation } => ensure!(
                token.len() == 32 && token.bytes().all(|b| b.is_ascii_hexdigit()) && confirmation == "apply_prepared_native_action", "Invalid native confirmation"),
            Self::ClearHistory { confirmation } => ensure!(confirmation == "clear_only_recorded_history", "History removal requires explicit confirmation"),
            _ => {}
        }
        Ok(())
    }
    pub fn needs_operational(&self) -> bool { matches!(self, Self::Prepare { .. } | Self::Commit { .. }) }
    pub fn needs_data_write(&self) -> bool { matches!(self, Self::ClearHistory { .. }) }
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct NativeState {
    pub microphone: String,
    pub microphone_scope: &'static str,
    pub camera: String,
    pub camera_scope: &'static str,
    pub camera_requested: Option<bool>,
    pub camera_service: Option<bool>,
    pub camera_present: Option<usize>,
    pub camera_pending: Option<usize>,
    pub camera_error: Option<String>,
    pub microphone_error: Option<String>,
    pub can_mute: bool,
    pub can_restore: bool,
    pub can_camera: bool,
    pub can_resume: bool,
    pub can_inspect: bool,
    pub lock_microphone: bool,
    pub lock_camera: bool,
    pub restore_on_unlock: bool,
    pub lock_supported: bool,
    pub camera_lock_supported: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub outcome: &'static str,
    pub state: Option<NativeState>,
    pub token: Option<String>,
    pub control: Option<Control>,
    pub data: serde_json::Value,
    pub error: Option<String>,
}
impl Report {
    fn data(value: serde_json::Value) -> Self {
        Self { outcome: "completed", state: None, token: None, control: None, data: value, error: None }
    }
}
struct Prepared {
    token: String,
    control: Control,
    state: NativeState,
    process_instance: Option<String>,
    #[cfg(windows)]
    camera_stamp: Option<super::camera_owner::Stamp>,
    created: Instant,
}
#[derive(Default)]
pub struct Session {
    prepared: Option<Prepared>,
}
impl Session {
    pub fn reset(&mut self) { self.prepared = None; }
    fn take_prepared(&mut self, token: &str, now: Instant) -> Result<Prepared> {
        let prepared = self.prepared.take().context("No current native confirmation; refresh and confirm again")?;
        let lifetime = if matches!(prepared.control, Control::Terminate { .. }) { Duration::from_secs(3) } else { Duration::from_secs(30) };
        ensure!(prepared.token == token && now.checked_duration_since(prepared.created)
            .is_some_and(|elapsed| elapsed <= lifetime), "Native confirmation is stale or expired");
        Ok(prepared)
    }
    pub fn execute(&mut self, request: Request, context: &Context) -> Result<Report> {
        request.validate()?;
        ensure!(!request.needs_operational() || context.operational, "Device/process commands are disabled in development mode");
        ensure!(!request.needs_data_write() || context.data_writable, "Local history is read-only");
        match request {
            Request::Controls {} => Ok(Report { state: Some(native_state(context)?), ..Report::data(serde_json::Value::Null) }),
            Request::Prepare { control } => {
                self.reset();
                let state = native_state(context)?;
                validate_control(&control, &state)?;
                #[cfg(windows)]
                let camera_stamp = camera_action(&control).map(|_| context.camera_owner.as_ref()
                    .context("Native camera coordinator unavailable")?.prepare()).transpose()?;
                let target = if let Control::Terminate { pid } = control {
                    let target = process_access(pid, context)?;
                    let executable = target.executable.as_deref().context("Executable identity is unavailable")?;
                    let filename = std::path::Path::new(executable).file_name().and_then(|s| s.to_str()).context("Executable identity is ambiguous")?;
                    ensure!(!["mcw", "mcw-tray", "mcw.exe", "mcw-tray.exe"].iter().any(|name| filename.eq_ignore_ascii_case(name)),
                        "MicCamWatch engine/guard processes are not termination targets");
                    Some(target)
                } else { None };
                let instance = target.as_ref().map(|access| {
                    let identity = &access.process.as_ref().context("Process identity is unavailable")?.instance_id;
                    ensure!(!identity.is_empty() && identity.len() <= 1024, "Process identity is ambiguous");
                    Ok::<_,anyhow::Error>(identity.clone())
                }).transpose()?;
                let target_description = target.map(|access| serde_json::json!({
                    "application": bounded(access.application), "pid": access.pid,
                    "executable": access.executable.map(bounded),
                    "instance": instance,
                }));
                let token = super::new_private_token()?;
                self.prepared = Some(Prepared { token: token.clone(), control: control.clone(), state: state.clone(), process_instance: instance,
                    #[cfg(windows)] camera_stamp,
                    created: Instant::now() });
                let seconds = if matches!(control, Control::Terminate { .. }) { 3 } else { 30 };
                Ok(Report { outcome: "prepared", state: Some(state), token: Some(token), control: Some(control), data: serde_json::json!({"confirmation_seconds":seconds,"target":target_description}), error: None })
            }
            Request::Commit { token, .. } => {
                // Consume BEFORE observation/effect. A failed/stale/expired request cannot be replayed.
                let prepared = self.take_prepared(&token, Instant::now())?;
                let state = native_state(context)?;
                ensure!(same_control_state(&prepared.state, &state, &prepared.control), "Native state changed; refresh and confirm again");
                validate_control(&prepared.control, &state)?;
                #[cfg(windows)]
                let result = match camera_action(&prepared.control) {
                    Some(action) => context.camera_owner.as_ref().context("Native camera coordinator unavailable")
                        .and_then(|owner| owner.execute(action, prepared.camera_stamp.context("No retained native camera revision")?))
                        .map(|()| serde_json::Value::Null),
                    None => apply(&prepared.control, prepared.process_instance.as_deref(), &state),
                };
                #[cfg(not(windows))]
                let result = apply(&prepared.control, prepared.process_instance.as_deref(), &state);
                let readback = native_state(context);
                let error = result.as_ref().err().map(|e| bounded(format!("{e:#}")));
                let state_error = readback.as_ref().err().map(|e| bounded(format!("Native readback unavailable: {e:#}")));
                let mut error = error.or(state_error);
                if error.is_none() {
                    let confirmed = readback.as_ref().ok().is_some_and(|actual| effect_confirmed(&prepared.control, actual));
                    if !confirmed { error = Some("The native request completed, but the requested effective state was not confirmed; consult the actual state and scope.".into()); }
                }
                Ok(Report {
                    outcome: if error.is_some() { "not_confirmed" } else { "completed" },
                    state: readback.ok(), token: None, control: Some(prepared.control),
                    data: result.unwrap_or(serde_json::Value::Null), error,
                })
            }
            Request::Devices {} => {
                let devices = PlatformMonitor::new(Policy::default())?.devices()?;
                let truncated = devices.len() > 128;
                let values: Vec<_> = devices.into_iter().take(128).map(|d| serde_json::json!({"resource":d.resource,"id":bounded(d.id),"name":bounded(d.name)})).collect();
                Ok(Report::data(serde_json::json!({"devices":values,"truncated":truncated})))
            }
            Request::Application { pid } => {
                Ok(Report::data(application_details(process_access(pid, context)?)))
            }
            Request::Paths {} => Ok(Report::data(serde_json::json!({
                "settings":crate::settings::settings_path()?.display().to_string(),
                "history":crate::history::path()?.display().to_string(),
                "default_policy":crate::settings::default_policy_path()?.display().to_string(),
                "explicit_policy":context.explicit_policy,
            }))),
            Request::Policy {} => {
                let policy = context.policy()?;
                policy.validate()?;
                Ok(policy_report(policy, "native owner's applied policy; not a new disk-file reload"))
            }
            Request::ValidatePolicy { path } => {
                match path {
                    Some(path) => Ok(policy_report(read_policy_file(&path)?, &bounded(path))),
                    None => {
                        let policy = context.policy()?;
                        policy.validate()?;
                        Ok(policy_report(policy, "native owner's applied policy"))
                    }
                }
            }
            Request::ClearHistory { .. } => {
                self.reset();
                crate::history::clear()?;
                Ok(Report::data(serde_json::json!({"cleared":true,"history":crate::history::path()?.display().to_string(),"scope":"current history and its rotations only; preferences, explicit logs and protection records preserved"})))
            }
            Request::Maintenance {} => Ok(Report::data(serde_json::json!({
                "version":env!("CARGO_PKG_VERSION"),
                "update":"Hybrid binary-only update is disabled until the complete bundle transaction is qualified. Use the verified whole installer or the actual package manager; do not replace only the Rust pair.",
                "uninstall":"Use the owner of this installation (MSI, Cargo, Homebrew, app or managed installer). First resolve owned protection/restoration and disable only its startup registration. User preferences and history are retained. No removal is performed by this query.",
                "releases":"https://github.com/Roman-Cuisset/miccamwatch/releases",
                "mutation_available":false,
            }))),
        }
    }
}
fn application_details(access: crate::model::Access) -> serde_json::Value {
    let truncated = access.evidence.len() > 32 || access.modules.len() > 32
        || access.process.as_ref().is_some_and(|process| process.ancestry.len() > 16);
    let evidence: Vec<_> = access.evidence.into_iter().take(32).map(|e| serde_json::json!({
        "kind":e.kind,"source":e.source,"detail":bounded(e.detail)})).collect();
    let signature = access.signature.map(|s| serde_json::json!({
        "verified":s.verified,"signer":s.signer.map(bounded),"error":s.error.map(bounded)}));
    let process = access.process.map(|p| {
        let ancestry: Vec<_> = p.ancestry.into_iter().take(16).map(|a| serde_json::json!({
            "pid":a.pid,"name":bounded(a.name),"created_at_filetime":a.created_at_filetime})).collect();
        serde_json::json!({"instance":bounded(p.instance_id),"ancestry":ancestry,
            "user":p.user.map(bounded),"session_id":p.session_id,"integrity":p.integrity.map(bounded)})
    });
    let modules: Vec<_> = access.modules.into_iter().take(32).map(bounded).collect();
    serde_json::json!({"application":bounded(access.application),"pid":access.pid,
        "resource":access.resource,"activity":access.activity,"confidence":access.confidence,
        "risk":access.risk,"enforcement":access.enforcement,"executable":access.executable.map(bounded),
        "signature":signature,"evidence":evidence,"process":process,"modules":modules,
        "parent_pid":access.parent_pid,"parent_name":access.parent_name.map(bounded),
        "device":access.device.map(bounded),"started_at":access.started_at,"truncated":truncated})
}
fn read_policy_file(path: &str) -> Result<Policy> {
    use std::io::Read as _;
    const MAX_POLICY_BYTES: u64 = 1_048_576;
    let path = std::path::Path::new(path).canonicalize().context("Resolve selected policy file")?;
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC);
    }
    let file = options.open(&path).context("Open selected policy file")?;
    ensure!(file.metadata()?.is_file(), "Policy validation requires a regular file");
    let mut text = String::new();
    file.take(MAX_POLICY_BYTES + 1).read_to_string(&mut text)?;
    ensure!(text.len() as u64 <= MAX_POLICY_BYTES, "Policy exceeds the1 MiB desktop validation limit");
    Policy::from_toml(&text)
}
fn policy_report(policy: Policy, source: &str) -> Report {
    Report::data(serde_json::json!({"valid":true,"source":source,
        "profile":format!("{:?}",policy.profile).to_lowercase(),
        "trust_policy":format!("{:?}",policy.trust_policy).to_lowercase(),
        "action":format!("{:?}",policy.action).to_lowercase(),
        "rule_count":policy.applications.len(),
        "notice":"Validation only. No policy file or running observer was changed."}))
}
fn bounded(mut text: String) -> String {
    let mut end = text.len().min(1024);
    while !text.is_char_boundary(end) { end -= 1; }
    text.truncate(end); text
}
fn process_access(pid: u32, context: &Context) -> Result<crate::model::Access> {
    let monitor = PlatformMonitor::new(context.policy()?)?;
    let snapshot = monitor.snapshot(crate::collector::CaptureScope { include_ready: true, ..Default::default() })?;
    let mut matches = snapshot.accesses.into_iter().filter(|a| a.pid == Some(pid));
    let first = matches.next().context("No current observed access is attributed to this process")?;
    let identity = first.process.as_ref().map(|process| &process.instance_id);
    ensure!(matches.all(|access| access.process.as_ref().map(|process| &process.instance_id) == identity),
        "Observed process identity is ambiguous");
    Ok(first)
}
fn same_control_state(before: &NativeState, after: &NativeState, control: &Control) -> bool {
    match control {
        Control::MicrophoneMute | Control::MicrophoneRestore => before.microphone == after.microphone,
        Control::LockPolicy { .. } => before.lock_microphone == after.lock_microphone && before.lock_camera == after.lock_camera && before.restore_on_unlock == after.restore_on_unlock,
        Control::Terminate { .. } => true, // retained instance is revalidated by the native termination API.
        _ => before.camera == after.camera && before.camera_requested == after.camera_requested && before.camera_pending == after.camera_pending,
    }
}
fn effect_confirmed(control: &Control, actual: &NativeState) -> bool {
    match control {
        Control::CameraBlock => actual.camera == "blocked" && (!cfg!(windows) ||
            (actual.camera_requested == Some(true) && actual.camera_service == Some(true))),
        Control::CameraAllow | Control::CameraRestoreLegacy { .. } => actual.camera == "allowed",
        Control::CameraResume => actual.camera_requested == Some(true) && actual.camera_service == Some(true),
        Control::MicrophoneMute => actual.microphone == "muted",
        _ => true,
    }
}
fn merge_lock_preferences(settings: &mut Settings, expected: (bool,bool,bool), requested: (bool,bool,bool)) -> Result<()> {
    ensure!((settings.mute_on_lock,settings.block_camera_on_lock,settings.restore_on_unlock) == expected,
        "Lock preferences changed concurrently; no values were overwritten");
    (settings.mute_on_lock,settings.block_camera_on_lock,settings.restore_on_unlock) = requested;
    Ok(())
}
fn validate_control(control: &Control, state: &NativeState) -> Result<()> {
    match control {
        Control::MicrophoneMute => ensure!(!cfg!(windows) && state.can_mute, "Use the Windows microphone owner, or writable Unix input controls"),
        Control::MicrophoneRestore => ensure!(!cfg!(windows) && state.can_restore, "No supported Unix original-state restoration is available"),
        Control::CameraBlock | Control::CameraAllow => ensure!(state.can_camera, "Camera controls are unavailable; no authorization requested"),
        Control::CameraResume => ensure!(cfg!(windows) && state.can_resume, "No stopped Windows camera protection is available to resume"),
        Control::CameraRestoreLegacy { .. } => ensure!(cfg!(windows) && state.can_camera, "Explicit historical camera restoration requires the Windows coordinator"),
        Control::CameraInspect => ensure!(cfg!(target_os="macos") && state.can_inspect, "Expanded profile inspection is macOS-only"),
        Control::LockPolicy { microphone, camera, .. } => {
            ensure!(!(*microphone || *camera) || state.lock_supported, "Automatic session lock protection is unavailable");
            ensure!(!*camera || state.camera_lock_supported, "Automatic camera-on-lock is Windows-only");
        }
        Control::Terminate { pid } => ensure!(*pid > 4 && *pid != std::process::id(), "Protected or ambiguous process target"),
    }
    Ok(())
}
fn native_state(context: &Context) -> Result<NativeState> {
    let settings = Settings::load()?;
    let microphone = PlatformMonitor::new(Policy::default()).and_then(|m| m.microphone_mute_state());
    let can_mute = !cfg!(windows) && microphone.as_ref().is_ok_and(|s| *s != MicrophoneMuteState::Unavailable);
    let mut state = NativeState {
        microphone: microphone.as_ref().map(|s| format!("{s:?}").to_lowercase()).unwrap_or_else(|_| "unknown".into()),
        microphone_error: microphone.err().map(|e| bounded(format!("{e:#}"))),
        microphone_scope: if cfg!(windows) { "Windows endpoint SDK mute, not access denial" } else if cfg!(target_os="linux") { "Writable capture sources in this PipeWire session only; restore owned originals" } else { "Writable CoreAudio INPUT mute controls only; restore owned originals" },
        camera: "unknown".into(), camera_scope: if cfg!(windows) { "Owned Windows PnP camera disables; explicit UAC, arrival enforcement is not instantaneous" } else if cfg!(target_os="linux") { "Authorized USB/UVC interfaces only; no non-USB, global hotplug or lock guarantee" } else { "Owned Restrictions profile; manual System Settings approval, not per-app TCC" },
        camera_requested: None, camera_service: None, camera_present: None, camera_pending: None, camera_error: None,
        can_mute, can_restore: !cfg!(windows), can_camera: true, can_resume: false, can_inspect: cfg!(target_os="macos"),
        lock_microphone: settings.mute_on_lock, lock_camera: settings.block_camera_on_lock, restore_on_unlock: settings.restore_on_unlock,
        lock_supported: !cfg!(target_os="macos"), camera_lock_supported: cfg!(windows),
    };
    #[cfg(windows)]
    match crate::privacy::camera_observation() {
        Ok(camera) => {
            state.camera = if camera.present_total==0 || camera.unknown_devices>0 || camera.pending_restore>0 { "unknown".into() } else { camera.state.as_str().into() };
            state.camera_requested=Some(camera.desired_blocked);state.camera_service=Some(camera.helper_active);
            state.camera_present=Some(camera.present_total);state.camera_pending=Some(camera.pending_restore);
            let owner_available = context.camera_owner.as_ref().is_some_and(|owner| owner.prepare().is_ok());
            state.can_camera = owner_available;
            state.can_resume=camera.desired_blocked && !camera.helper_active && owner_available;
            state.camera_error=camera.detail.map(bounded);
            if !owner_available {
                let detail = state.camera_error.take().unwrap_or_default();
                state.camera_error = Some(bounded(format!("{detail}\nNative camera coordinator busy or unavailable; no action is enabled.")));
            }
        }
        Err(error) => { state.can_camera=false;state.camera_error=Some(bounded(format!("{error:#}"))); }
    }
    #[cfg(not(windows))]
    match crate::privacy::camera_state() {
        Ok(camera) => state.camera=camera.as_str().into(),
        Err(error) => state.camera_error=Some(bounded(format!("{error:#}"))),
    }
    #[cfg(target_os="linux")]
    if let Err(error)=crate::privacy::camera_control_preflight() { state.can_camera=false;state.camera_error=Some(bounded(format!("{error:#}"))); }
    #[cfg(not(windows))]
    let _ = context;
    Ok(state)
}
fn apply(control: &Control, expected_instance: Option<&str>, before: &NativeState) -> Result<serde_json::Value> {
    match control {
        #[cfg(not(windows))]
        Control::MicrophoneMute | Control::MicrophoneRestore => {
            let count=PlatformMonitor::new(Policy::default())?.set_microphone_mute(matches!(control,Control::MicrophoneMute))?;
            return Ok(serde_json::json!({"changed":count,"restoration":"Existing native owned-original semantics; a mixed/premuted aggregate is not blindly unmuted"}));
        }
        #[cfg(not(windows))]
        Control::CameraBlock => crate::privacy::set_camera_state(crate::privacy::CameraPrivacyState::Blocked)?,
        #[cfg(not(windows))]
        Control::CameraAllow => crate::privacy::set_camera_state(crate::privacy::CameraPrivacyState::Allowed)?,
        #[cfg(target_os="macos")]
        Control::CameraInspect => return Ok(serde_json::to_value(crate::privacy::camera_inspection(true)?)?),
        Control::LockPolicy { microphone, camera, restore } => {
            Settings::update_checked(|s| merge_lock_preferences(s,
                (before.lock_microphone,before.lock_camera,before.restore_on_unlock),
                (*microphone,*camera,*restore)))?;
            let saved=Settings::load()?;
            ensure!((saved.mute_on_lock,saved.block_camera_on_lock,saved.restore_on_unlock)==(*microphone,*camera,*restore), "Lock policy readback does not confirm the requested values");
        }
        Control::Terminate { pid } => {
            let instance=expected_instance.filter(|s| !s.is_empty()).context("No retained process instance; target is ambiguous")?;
            crate::platform::terminate_process_by_pid(*pid,instance)?;
            return Ok(serde_json::json!({"pid":pid,"instance":instance,
                "native_termination_request_accepted":true,"process_exit_confirmed":false}));
        }
        #[allow(unreachable_patterns)]
        _ => bail!("This native control is not supported on this platform"),
    }
    Ok(serde_json::Value::Null)
}

#[cfg(windows)]
fn camera_action(control: &Control) -> Option<super::camera_owner::Action> {
    use super::camera_owner::Action;
    match control {
        Control::CameraBlock => Some(Action::Block),
        Control::CameraAllow => Some(Action::Allow),
        Control::CameraResume => Some(Action::Resume),
        Control::CameraRestoreLegacy { instance_id } => Some(Action::RestoreLegacy { instance_id: instance_id.clone() }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_validation_uses_the_native_parser_and_rejects_oversized_or_nonregular_inputs() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("selected-policy.toml");
        std::fs::write(&path, b"profile = \"strict\"\n").unwrap();
        let parsed = read_policy_file(path.to_str().unwrap()).unwrap();
        assert_eq!(parsed.profile, crate::config::Profile::Strict);
        std::fs::write(&path, b"profile = \"invented\"\n").unwrap();
        assert!(read_policy_file(path.to_str().unwrap()).is_err());
        use std::io::{Read as _, Write as _};
        let header = b"profile = \"strict\"\n";
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(header).unwrap();
        std::io::copy(&mut std::io::repeat(b' ').take(1_048_576 - header.len() as u64), &mut file).unwrap();
        drop(file);
        assert_eq!(read_policy_file(path.to_str().unwrap()).unwrap().profile, crate::config::Profile::Strict);
        std::fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(b" ").unwrap();
        assert!(read_policy_file(path.to_str().unwrap()).is_err());
        assert!(read_policy_file(directory.path().to_str().unwrap()).is_err());
    }

    fn state() -> NativeState {
        NativeState {
            microphone:"unmuted".into(),microphone_scope:"test-owned input",
            camera:"unknown".into(),camera_scope:"test-owned camera",
            camera_requested:None,camera_service:None,camera_present:None,camera_pending:None,
            camera_error:None,microphone_error:None,can_mute:false,can_restore:false,
            can_camera:false,can_resume:false,can_inspect:false,
            lock_microphone:false,lock_camera:false,restore_on_unlock:true,
            lock_supported:false,camera_lock_supported:false,
        }
    }
    fn prepared(control: Control, now: Instant) -> Session {
        Session { prepared: Some(Prepared { token:"a".repeat(32),control,state:state(),
            process_instance:Some("retained-native-instance".into()),
            #[cfg(windows)] camera_stamp: None,
            created:now }) }
    }
    #[test]
    fn confirmation_is_single_use_expiring_and_invalidated_on_session_reset() {
        let now=Instant::now();
        let mut session=prepared(Control::Terminate{pid:12345},now);
        let accepted=session.take_prepared(&"a".repeat(32),now+Duration::from_secs(2)).unwrap();
        assert_eq!(accepted.process_instance.as_deref(),Some("retained-native-instance"));
        assert!(session.take_prepared(&"a".repeat(32),now).is_err());
        for (token,elapsed) in [("b".repeat(32),Duration::ZERO),("a".repeat(32),Duration::from_secs(4))] {
            let mut session=prepared(Control::Terminate{pid:12345},now);
            assert!(session.take_prepared(&token,now+elapsed).is_err());
            assert!(session.prepared.is_none());
        }
        let mut session=prepared(Control::CameraBlock,now);
        assert!(session.take_prepared(&"a".repeat(32),now+Duration::from_secs(31)).is_err());
        let mut session=prepared(Control::MicrophoneRestore,now);
        session.reset();
        assert!(session.take_prepared(&"a".repeat(32),now).is_err());
    }
    #[test]
    fn effective_readback_never_promotes_unknown_or_mixed_to_requested_protection() {
        let mut actual=state();
        assert!(!effect_confirmed(&Control::CameraBlock,&actual));
        actual.camera="allowed".into();
        assert!(effect_confirmed(&Control::CameraAllow,&actual));
        assert!(!effect_confirmed(&Control::CameraBlock,&actual));
        actual.camera="blocked".into();
        #[cfg(windows)]
        {
            assert!(!effect_confirmed(&Control::CameraBlock,&actual));
            actual.camera_requested=Some(true);
            actual.camera_service=Some(false);
            assert!(!effect_confirmed(&Control::CameraResume,&actual));
            actual.camera_service=Some(true);
            assert!(effect_confirmed(&Control::CameraResume,&actual));
        }
        assert!(effect_confirmed(&Control::CameraBlock,&actual));
        actual.microphone="mixed".into();
        assert!(!effect_confirmed(&Control::MicrophoneMute,&actual));
        actual.microphone="muted".into();
        assert!(effect_confirmed(&Control::MicrophoneMute,&actual));
    }
    #[test]
    fn a_concurrent_lock_preference_change_is_not_overwritten_and_unrelated_preferences_survive() {
        let mut settings=Settings { history_enabled:false,profile:PrivacyProfile::Meeting,
            mute_on_lock:false,block_camera_on_lock:false,restore_on_unlock:true,..Settings::default() };
        let saved=serde_json::to_value(&settings).unwrap();
        assert!(merge_lock_preferences(&mut settings,(true,false,true),(false,true,false)).is_err());
        assert_eq!(serde_json::to_value(&settings).unwrap(),saved);
        merge_lock_preferences(&mut settings,(false,false,true),(true,false,false)).unwrap();
        assert!(settings.mute_on_lock);
        assert!(!settings.restore_on_unlock);
        assert!(!settings.history_enabled);
        assert_eq!(settings.profile,PrivacyProfile::Meeting);
        let before=state();
        let mut changed=state();
        changed.lock_camera=true;
        assert!(!same_control_state(&before,&changed,&Control::LockPolicy{microphone:true,camera:false,restore:false}));
    }
    #[test]
    fn unknown_actions_fields_and_invalid_confirmations_are_refused() {
        assert!(serde_json::from_str::<Request>(r#"{"action":"execute","argv":[]}"#).is_err());
        assert!(serde_json::from_str::<Request>(r#"{"action":"controls","elevate":true}"#).is_err());
        assert!(Request::Commit{token:"a".repeat(32),confirmation:"yes".into()}.validate().is_err());
        assert!(Request::ClearHistory{confirmation:"all_data".into()}.validate().is_err());
    }
    #[test]
    fn development_cannot_prepare_or_replay_any_hardware_or_process_action() {
        let mut session=Session::default();
        let context=Context::default();
        for control in [Control::MicrophoneMute,Control::CameraBlock,Control::Terminate{pid:12345}] {
            assert!(session.execute(Request::Prepare{control},&context).is_err());
        }
        assert!(session.execute(Request::Commit{token:"a".repeat(32),confirmation:"apply_prepared_native_action".into()},&context).is_err());
        assert!(session.prepared.is_none());
    }
    #[test]
    fn missing_applied_policy_is_not_replaced_with_default_assessment() {
        let context=Context{policy:Some(Arc::new(Policy::default())),..Default::default()};
        assert!(context.policy().is_err());
        let explicit=Context{explicit_policy:true,..context};
        assert!(explicit.policy().is_ok());
    }
    #[test]
    fn process_authority_never_comes_from_an_arbitrary_instance_in_the_wire() {
        assert!(serde_json::from_str::<Request>(r#"{"action":"prepare","control":{"kind":"terminate","pid":12345,"instance":"forged"}}"#).is_err());
    }
}
