use anyhow::{Context, Result};
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
    let output = schtasks(&["/Query", "/TN", TASK_NAME, "/FO", "LIST"])?;
    if output.status.success() {
        return Ok(AutostartState::Enabled);
    }
    let run = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(RUN_KEY)
        .and_then(|key| key.get_value::<String, _>(RUN_VALUE));
    Ok(if run.is_ok() {
        AutostartState::Enabled
    } else {
        AutostartState::Disabled
    })
}

pub fn enable() -> Result<()> {
    let executable = std::env::current_exe().context("failed to locate mcw executable")?;
    let command = format!("\"{}\" tray run", executable.display());
    let output = schtasks(&[
        "/Create", "/TN", TASK_NAME, "/TR", &command, "/SC", "ONLOGON", "/RL", "LIMITED", "/F",
    ])?;
    if output.status.success() {
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
    let _ = schtasks(&["/Delete", "/TN", TASK_NAME, "/F"]);
    let root = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(key) = root.open_subkey_with_flags(RUN_KEY, winreg::enums::KEY_SET_VALUE) {
        match key.delete_value(RUN_VALUE) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).context("failed to disable per-user autostart"),
        }
    }
    Ok(())
}

fn schtasks(arguments: &[&str]) -> Result<std::process::Output> {
    Command::new("schtasks.exe")
        .args(arguments)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .context("failed to execute Windows Task Scheduler")
}
