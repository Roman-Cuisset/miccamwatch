use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::{io::Read, os::windows::process::CommandExt, path::Path, process::Command};
use winreg::{RegKey, enums::HKEY_CURRENT_USER};

const TASK_NAME: &str = "MicCamWatch Tray";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "MicCamWatch";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AutostartState {
    Enabled,
    Disabled,
}

pub fn state() -> Result<AutostartState> {
    let task = schtasks(&["/Query", "/TN", TASK_NAME, "/FO", "LIST"]);
    if task.as_ref().is_ok_and(|output| output.status.success()) {
        return Ok(AutostartState::Enabled);
    }
    let run = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(RUN_KEY)
        .and_then(|key| key.get_value::<String, _>(RUN_VALUE));
    if run.is_ok() {
        return Ok(AutostartState::Enabled);
    }
    task?;
    Ok(AutostartState::Disabled)
}

pub fn enable() -> Result<()> {
    enable_for_version(env!("CARGO_PKG_VERSION"))
}

fn enable_for_version(version: &str) -> Result<()> {
    let executable = tray_executable(version)?;
    let command = if executable
        .file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("mcw-tray.exe"))
    {
        format!("\"{}\"", executable.display())
    } else {
        format!("\"{}\" tray run", executable.display())
    };
    let output = schtasks(&[
        "/Create", "/TN", TASK_NAME, "/TR", &command, "/SC", "ONLOGON", "/RL", "LIMITED", "/F",
    ]);
    if output.as_ref().is_ok_and(|output| output.status.success()) {
        remove_run_value()?;
        return Ok(());
    }
    // Do not add a Run entry if an older task could still launch another executable.
    if schtasks(&["/Query", "/TN", TASK_NAME, "/FO", "LIST"])
        .is_ok_and(|output| output.status.success())
    {
        bail!("failed to replace the existing MicCamWatch autostart task");
    }
    let root = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = root
        .create_subkey(RUN_KEY)
        .context("failed to open the per-user Run registry key")?;
    key.set_value(RUN_VALUE, &command)
        .context("failed to configure per-user autostart fallback")?;
    Ok(())
}

/// Preserve the user's opt-in while repointing a registration after a binary update.
pub(crate) fn refresh_if_enabled(version: &str) -> Result<()> {
    if state()? == AutostartState::Enabled {
        enable_for_version(version)?;
    }
    Ok(())
}

pub fn disable() -> Result<()> {
    let deleted = schtasks(&["/Delete", "/TN", TASK_NAME, "/F"]);
    // The Run fallback must be removed even when Task Scheduler is unavailable.
    remove_run_value()?;
    let deleted =
        deleted.context("failed to execute Windows Task Scheduler while disabling autostart")?;
    if !deleted.status.success()
        && schtasks(&["/Query", "/TN", TASK_NAME, "/FO", "LIST"])?
            .status
            .success()
    {
        bail!(
            "failed to disable scheduled autostart: {}",
            String::from_utf8_lossy(&deleted.stderr)
        );
    }
    Ok(())
}

fn remove_run_value() -> Result<()> {
    let root = RegKey::predef(HKEY_CURRENT_USER);
    match root.open_subkey_with_flags(RUN_KEY, winreg::enums::KEY_SET_VALUE) {
        Ok(key) => match key.delete_value(RUN_VALUE) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).context("failed to disable per-user autostart"),
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).context("failed to open the per-user Run registry key"),
    }
    Ok(())
}

fn tray_executable(version: &str) -> Result<std::path::PathBuf> {
    let current = std::env::current_exe().context("failed to locate mcw executable")?;
    let tray = current.with_file_name("mcw-tray.exe");
    if tray.exists() {
        if !tray_matches_version(&tray, version)? {
            bail!(
                "the companion {} does not match mcw {}; install matching binaries before enabling autostart",
                tray.display(),
                version
            );
        }
        return Ok(tray);
    }
    Ok(current)
}

/// The tray menu embeds this exact version label, including in older releases
/// that predate an explicit Windows file-version resource. Inspect it without
/// executing a possibly stale tray binary.
pub(crate) fn tray_matches_current_version(path: &Path) -> Result<bool> {
    tray_contains_marker(
        path,
        concat!("miccamwatch v", env!("CARGO_PKG_VERSION")).as_bytes(),
    )
}

pub(crate) fn tray_matches_version(path: &Path, version: &str) -> Result<bool> {
    tray_contains_marker(path, format!("miccamwatch v{version}").as_bytes())
}

fn tray_contains_marker(path: &Path, marker: &[u8]) -> Result<bool> {
    let mut file = std::fs::File::open(path)
        .with_context(|| format!("failed to inspect companion tray {}", path.display()))?;
    let mut bytes = [0u8; 8192];
    let mut retained = 0;
    loop {
        let count = file.read(&mut bytes[retained..])?;
        if count == 0 {
            return Ok(bytes[..retained].ends_with(marker));
        }
        let end = retained + count;
        if bytes[..end].windows(marker.len() + 1).any(|part| {
            part.starts_with(marker)
                && !matches!(part[marker.len()], b'0'..=b'9' | b'.' | b'-' | b'+')
        }) {
            return Ok(true);
        }
        retained = marker.len().min(end);
        bytes.copy_within(end - retained..end, 0);
    }
}

fn schtasks(arguments: &[&str]) -> Result<std::process::Output> {
    Command::new("schtasks.exe")
        .args(arguments)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .context("failed to execute Windows Task Scheduler")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_version_rejects_prefixes_and_handles_read_boundaries() -> Result<()> {
        let path =
            std::env::temp_dir().join(format!("mcw-tray-version-{}.bin", std::process::id()));
        let check = || -> Result<()> {
            for padding in [0, 8190, 8192] {
                for (suffix, expected) in [
                    ("", true),
                    ("Microphone unavailable", true),
                    ("\0", true),
                    ("0", false),
                    (".1", false),
                    ("-beta", false),
                    ("+build", false),
                ] {
                    let mut bytes = vec![0; padding];
                    bytes.extend_from_slice(b"miccamwatch v0.13.5");
                    bytes.extend_from_slice(suffix.as_bytes());
                    std::fs::write(&path, bytes)?;
                    assert_eq!(tray_matches_version(&path, "0.13.5")?, expected);
                }
            }
            Ok(())
        };
        let result = check();
        let _ = std::fs::remove_file(path);
        result
    }
}
