#![windows_subsystem = "windows"]

use anyhow::Result;
use miccamwatch::{
    config::Policy,
    frontends::tray,
    i18n::Language,
    platform::PlatformMonitor,
    settings::{self, Settings},
};
use windows::{
    Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW},
    core::PCWSTR,
};

fn main() {
    if let Err(error) = run() {
        show_error(&format!("MicCamWatch tray failed:\n\n{error:#}"));
    }
}

fn run() -> Result<()> {
    let settings = Settings::load()?;
    let default_policy = settings::default_policy_path()?;
    let policy = Policy::load(default_policy.exists().then_some(default_policy.as_path()))?;
    let lang = policy
        .language
        .as_deref()
        .and_then(Language::from_code)
        .unwrap_or_else(Language::detect);
    let monitor = PlatformMonitor::new(policy.clone())?;
    tray::run_tray(monitor, policy, lang, settings)
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
