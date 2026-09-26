use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::{os::windows::process::CommandExt, process::Command};
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
    let executable = tray_executable()?;
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
    let root = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = root
        .create_subkey(RUN_KEY)
        .context("failed to open the per-user Run registry key")?;
    key.set_value(RUN_VALUE, &command)
        .context("failed to configure per-user autostart fallback")?;
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

fn tray_executable() -> Result<std::path::PathBuf> {
    let current = std::env::current_exe().context("failed to locate mcw executable")?;
    let tray = current.with_file_name("mcw-tray.exe");
    Ok(if tray.exists() { tray } else { current })
}

fn schtasks(arguments: &[&str]) -> Result<std::process::Output> {
    Command::new("schtasks.exe")
        .args(arguments)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .context("failed to execute Windows Task Scheduler")
}
