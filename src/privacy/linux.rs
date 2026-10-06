//! Explicit, administrator-authorized USB/UVC camera controls.
//! Observation never invokes Polkit. Only the fixed, root-owned helper mutates sysfs.
#[path = "linux/helper.rs"]
mod helper;
#[path = "linux/store.rs"]
mod store;
#[path = "linux/usb.rs"]
mod usb;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::process::Command;

const PROTOCOL: u32 = 1;
const VERSION: &str = env!("CARGO_PKG_VERSION");
const HELPER: &str = "/usr/local/libexec/miccamwatch/mcw-camera-helper";
const PKEXEC: &str = "/usr/bin/pkexec";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CameraPrivacyState {
    Allowed,
    Blocked,
    SystemManaged,
}
impl CameraPrivacyState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Allowed => "allowed",
            Self::Blocked => "blocked",
            Self::SystemManaged => "system_managed",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Action {
    Block,
    Allow,
    Toggle,
    Status,
}
impl Action {
    fn as_str(self) -> &'static str {
        match self {
            Self::Block => "block",
            Self::Allow => "allow",
            Self::Toggle => "toggle",
            Self::Status => "status",
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Reply {
    protocol: u32,
    version: String,
    ok: bool,
    state: Option<CameraPrivacyState>,
    detail: String,
    error: Option<String>,
}

pub fn camera_capability() -> &'static str {
    "USB/UVC only (Linux >=5.9); explicit admin approval, owned restoration; no hotplug/lock blocking"
}

pub fn camera_state() -> Result<CameraPrivacyState> {
    Ok(observe()?.0)
}

pub fn camera_detail() -> Result<String> {
    Ok(observe()?.1)
}

fn observe() -> Result<(CameraPrivacyState, String)> {
    // A root-owned cache is evidence of ownership, not evidence of driver state.
    // Re-enumerate the kernel on every observation and reject changed generations.
    let inventory = usb::inventory()?;
    let cache = store::read_cache()?;
    helper::established_state(
        &inventory,
        cache
            .as_ref()
            .map(|cache| cache.entries.as_slice())
            .unwrap_or(&[]),
    )
}

pub fn set_camera_state(state: CameraPrivacyState) -> Result<()> {
    let action = match state {
        CameraPrivacyState::Allowed => Action::Allow,
        CameraPrivacyState::Blocked => Action::Block,
        CameraPrivacyState::SystemManaged => {
            bail!("system-managed camera state cannot be requested")
        }
    };
    request(action)?;
    Ok(())
}

pub fn toggle_camera_state() -> Result<CameraPrivacyState> {
    request(Action::Toggle)
}

fn request(action: Action) -> Result<CameraPrivacyState> {
    store::trusted_executable(HELPER)
        .context("Linux USB camera helper is absent or unsafe; administrator review and explicit root setup are required: PREFIX/share/miccamwatch/linux-camera/install-camera-helper.sh --archive ABS --sha256 TRUSTED_HASH")?;
    store::trusted_executable(PKEXEC).context("trusted /usr/bin/pkexec is unavailable")?;
    let output = Command::new(PKEXEC)
        .env_clear()
        .env("LANG", "C.UTF-8")
        .arg(HELPER)
        .args([
            "--protocol",
            "1",
            "--version",
            VERSION,
            "--action",
            action.as_str(),
        ])
        .output()
        .context("could not request administrator authorization for USB camera control")?;
    if output.stdout.len() > store::MAX_BYTES as usize {
        bail!("camera helper returned an oversized response");
    }
    if !output.status.success() && output.stdout.is_empty() {
        bail!(
            "USB camera authorization was denied/cancelled or the helper failed ({}); no successful camera change was established",
            output.status
        );
    }
    let reply: Reply = serde_json::from_slice(&output.stdout)
        .context("camera helper returned an invalid response; camera state is not established")?;
    if reply.protocol != PROTOCOL || reply.version != VERSION {
        bail!("camera helper version/protocol mismatch; install the helper matching mcw {VERSION}");
    }
    if !output.status.success() || !reply.ok {
        bail!("{}", reply.error.as_deref().unwrap_or(&reply.detail));
    }
    let state = reply
        .state
        .context("camera helper did not establish camera state")?;
    if (matches!(action, Action::Block) && state != CameraPrivacyState::Blocked)
        || (matches!(action, Action::Allow) && state != CameraPrivacyState::Allowed)
        || (matches!(action, Action::Toggle) && state == CameraPrivacyState::SystemManaged)
    {
        bail!(
            "helper did not establish the requested complete camera state: {}",
            reply.detail
        );
    }
    Ok(state)
}

/// Entry point for the separately packaged, fixed-path privileged binary.
pub fn run_helper() -> Result<()> {
    helper::run()
}
