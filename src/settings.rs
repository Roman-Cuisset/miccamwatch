use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{env, fs, path::PathBuf};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyProfile {
    Private,
    Meeting,
    Development,
    #[default]
    Balanced,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub notifications_enabled: bool,
    pub sound_enabled: bool,
    pub show_ready: bool,
    pub history_enabled: bool,
    pub profile: PrivacyProfile,
    pub pause_notifications_until: Option<DateTime<Utc>>,
    pub mute_on_lock: bool,
    pub block_camera_on_lock: bool,
    pub restore_on_unlock: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            notifications_enabled: true,
            sound_enabled: false,
            show_ready: false,
            history_enabled: true,
            profile: PrivacyProfile::Balanced,
            pause_notifications_until: None,
            mute_on_lock: false,
            block_camera_on_lock: false,
            restore_on_unlock: true,
        }
    }
}

impl Settings {
    pub fn load() -> Result<Self> {
        let path = settings_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("failed to read settings {}", path.display()))?;
        toml::from_str(&contents).with_context(|| format!("invalid settings {}", path.display()))
    }

    pub fn save(&self) -> Result<()> {
        let path = settings_path()?;
        let parent = path.parent().context("settings path has no parent")?;
        fs::create_dir_all(parent)?;
        let temporary = path.with_extension("toml.tmp");
        fs::write(&temporary, toml::to_string_pretty(self)?)?;
        if path.exists() {
            fs::remove_file(&path)?;
        }
        fs::rename(&temporary, &path)?;
        Ok(())
    }

    pub fn notifications_paused(&self) -> bool {
        self.pause_notifications_until
            .is_some_and(|until| until > Utc::now())
    }
}

pub fn config_dir() -> Result<PathBuf> {
    env::var_os("APPDATA")
        .map(PathBuf::from)
        .map(|path| path.join("MicCamWatch"))
        .context("APPDATA is unavailable")
}

pub fn data_dir() -> Result<PathBuf> {
    env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .map(|path| path.join("MicCamWatch"))
        .context("LOCALAPPDATA is unavailable")
}

pub fn settings_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("settings.toml"))
}

pub fn default_policy_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("policy.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_safe_and_round_trip() {
        let settings = Settings::default();
        assert!(!settings.mute_on_lock);
        assert!(!settings.block_camera_on_lock);
        assert!(!settings.notifications_paused());
        let encoded = toml::to_string(&settings).unwrap();
        let decoded: Settings = toml::from_str(&encoded).unwrap();
        assert_eq!(decoded.profile, PrivacyProfile::Balanced);
    }
}
