use anyhow::{Context, Result};
use serde::Serialize;
use winreg::{RegKey, enums::HKEY_CURRENT_USER};

const WEBCAM_KEY: &str =
    r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\webcam";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
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

pub fn camera_state() -> Result<CameraPrivacyState> {
    let root = RegKey::predef(HKEY_CURRENT_USER);
    let key = match root.open_subkey(WEBCAM_KEY) {
        Ok(key) => key,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(CameraPrivacyState::SystemManaged);
        }
        Err(error) => return Err(error).context("failed to open Windows camera privacy settings"),
    };
    match key.get_value::<String, _>("Value") {
        Ok(value) if value.eq_ignore_ascii_case("Allow") => Ok(CameraPrivacyState::Allowed),
        Ok(value) if value.eq_ignore_ascii_case("Deny") => Ok(CameraPrivacyState::Blocked),
        Ok(value) => anyhow::bail!("unsupported Windows camera privacy value '{value}'"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(CameraPrivacyState::SystemManaged)
        }
        Err(error) => Err(error).context("failed to read Windows camera privacy state"),
    }
}

pub fn set_camera_state(state: CameraPrivacyState) -> Result<()> {
    let value = match state {
        CameraPrivacyState::Allowed => "Allow",
        CameraPrivacyState::Blocked => "Deny",
        CameraPrivacyState::SystemManaged => {
            anyhow::bail!("system-managed camera privacy cannot be written directly")
        }
    };
    let root = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = root
        .create_subkey(WEBCAM_KEY)
        .context("failed to open writable Windows camera privacy settings")?;
    key.set_value("Value", &value)
        .context("failed to update Windows camera privacy state")?;
    Ok(())
}

pub fn toggle_camera() -> Result<CameraPrivacyState> {
    let next = match camera_state()? {
        CameraPrivacyState::Allowed => CameraPrivacyState::Blocked,
        CameraPrivacyState::Blocked | CameraPrivacyState::SystemManaged => {
            CameraPrivacyState::Allowed
        }
    };
    set_camera_state(next)?;
    Ok(next)
}
