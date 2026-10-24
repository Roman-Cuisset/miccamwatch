#![windows_subsystem = "windows"]

use anyhow::Result;
use miccamwatch::{
    config::Policy,
    frontends::tray,
    i18n::Language,
    settings::{self, Settings},
};
use std::process::ExitCode;
use windows::{
    Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW},
    core::PCWSTR,
};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            show_error(&format!("MicCamWatch could not start:\n\n{error:#}"));
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<()> {
    let initial_window = std::env::args_os()
        .skip(1)
        .any(|argument| argument == "--open");
    if tray::reuse_running(initial_window)? {
        return Ok(());
    }
    let settings = Settings::load()?;
    let default_policy = settings::default_policy_path()?;
    let policy = Policy::load(default_policy.exists().then_some(default_policy.as_path()))?;
    let lang = policy
        .language
        .as_deref()
        .and_then(Language::from_code)
        .unwrap_or_else(Language::detect);
    tray::run_tray(policy, lang, settings, false, initial_window)
}

fn show_error(message: &str) {
    let title = wide("MicCamWatch");
    let message = wide(message);
    unsafe {
        let _ = MessageBoxW(
            None,
            PCWSTR(message.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK | MB_ICONERROR,
        );
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}
