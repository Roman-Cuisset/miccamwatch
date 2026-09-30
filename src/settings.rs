use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
use std::{
    env, fs,
    io::Write,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
#[cfg(unix)]
use std::{ffi::OsStr, path::Path};
#[cfg(windows)]
use windows::{
    Win32::Storage::FileSystem::{MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW},
    core::PCWSTR,
};

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
        save_at(self, &settings_path()?)
    }

    pub fn notifications_paused(&self) -> bool {
        self.pause_notifications_until
            .is_some_and(|until| until > Utc::now())
    }
}

fn save_at(settings: &Settings, path: &std::path::Path) -> Result<()> {
    let parent = path.parent().context("settings path has no parent")?;
    fs::create_dir_all(parent)?;
    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
    let (temporary, mut file) = loop {
        let nonce = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let candidate = path.with_extension(format!("toml.tmp.{}.{}", std::process::id(), nonce));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => break (candidate, file),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error).context("failed to stage settings"),
        }
    };
    let result: Result<()> = (|| {
        file.write_all(toml::to_string_pretty(settings)?.as_bytes())?;
        file.sync_all()?;
        drop(file);
        #[cfg(windows)]
        {
            let from: Vec<u16> = temporary.as_os_str().encode_wide().chain(Some(0)).collect();
            let to: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
            unsafe {
                MoveFileExW(
                    PCWSTR(from.as_ptr()),
                    PCWSTR(to.as_ptr()),
                    MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
                )
            }?;
        }
        #[cfg(unix)]
        fs::rename(&temporary, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.with_context(|| format!("failed to replace settings {}", path.display()))
}

#[cfg(windows)]
pub fn config_dir() -> Result<PathBuf> {
    env::var_os("APPDATA")
        .map(PathBuf::from)
        .map(|path| path.join("MicCamWatch"))
        .context("APPDATA is unavailable")
}

#[cfg(windows)]
pub fn data_dir() -> Result<PathBuf> {
    env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .map(|path| path.join("MicCamWatch"))
        .context("LOCALAPPDATA is unavailable")
}

#[cfg(unix)]
fn unix_app_dir(xdg: Option<&OsStr>, home: Option<&OsStr>, fallback: &str) -> Result<PathBuf> {
    let base = xdg
        .map(Path::new)
        .filter(|path| path.is_absolute())
        .map(Path::to_path_buf)
        .or_else(|| {
            home.map(Path::new)
                .filter(|path| path.is_absolute())
                .map(|path| path.join(fallback))
        })
        .context("neither an absolute XDG directory nor an absolute HOME is available")?;
    Ok(base.join("MicCamWatch"))
}

#[cfg(unix)]
pub fn config_dir() -> Result<PathBuf> {
    unix_app_dir(
        env::var_os("XDG_CONFIG_HOME").as_deref(),
        env::var_os("HOME").as_deref(),
        ".config",
    )
}

#[cfg(unix)]
pub fn data_dir() -> Result<PathBuf> {
    unix_app_dir(
        env::var_os("XDG_DATA_HOME").as_deref(),
        env::var_os("HOME").as_deref(),
        ".local/share",
    )
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

    #[test]
    fn subsequent_save_replaces_settings_without_losing_existing_file() {
        let path = std::env::temp_dir().join(format!(
            "mcw-settings-{}-{}.toml",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut settings = Settings {
            notifications_enabled: false,
            ..Settings::default()
        };
        save_at(&settings, &path).unwrap();
        settings.notifications_enabled = true;
        settings.block_camera_on_lock = true;
        save_at(&settings, &path).unwrap();
        let saved: Settings = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert!(saved.notifications_enabled);
        assert!(saved.block_camera_on_lock);
        fs::remove_file(path).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn xdg_directories_require_absolute_paths_and_fall_back_to_home() {
        let home = OsStr::new("/home/test");
        assert_eq!(
            unix_app_dir(Some(OsStr::new("/custom/config")), None, ".config").unwrap(),
            PathBuf::from("/custom/config/MicCamWatch")
        );
        assert_eq!(
            unix_app_dir(Some(OsStr::new("relative")), Some(home), ".config").unwrap(),
            PathBuf::from("/home/test/.config/MicCamWatch")
        );
        assert_eq!(
            unix_app_dir(Some(OsStr::new("")), Some(home), ".local/share").unwrap(),
            PathBuf::from("/home/test/.local/share/MicCamWatch")
        );
        assert!(unix_app_dir(None, None, ".config").is_err());
        assert!(unix_app_dir(None, Some(OsStr::new("relative")), ".config").is_err());
    }
}
