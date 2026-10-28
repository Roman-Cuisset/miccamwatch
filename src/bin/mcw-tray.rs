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
    // A default/native build must refuse this flag before any operational
    // singleton, settings or protection path. Old stable binaries lack this gate.
    #[cfg(not(feature = "flutter-desktop"))]
    anyhow::ensure!(
        !std::env::args_os().any(|argument| argument == "--read-only-development"
            || argument == "--controlled-development"
            || argument == "--settings-development"),
        "Development modes require a build with flutter-desktop; no tray started"
    );
    let initial_window = std::env::args_os()
        .skip(1)
        .any(|argument| argument == "--open");
    #[cfg(feature = "flutter-desktop")]
    if std::env::args_os().any(|argument| {
        argument == "--read-only-development"
            || argument == "--controlled-development"
            || argument == "--settings-development"
    }) {
        // No operational singleton/resume. Controlled mode additionally requires
        // isolated data paths and a released native resource at startup.
        let arguments = std::env::args().collect::<Vec<_>>();
        let controlled = arguments
            .iter()
            .any(|argument| argument == "--controlled-development");
        let settings = arguments
            .iter()
            .any(|argument| argument == "--settings-development");
        anyhow::ensure!(
            arguments
                .iter()
                .filter(|argument| matches!(
                    argument.as_str(),
                    "--read-only-development"
                        | "--controlled-development"
                        | "--settings-development"
                ))
                .count()
                == 1,
            "Exactly one development mode is required"
        );
        let theme = arguments
            .windows(2)
            .find(|pair| pair[0] == "--desktop-theme")
            .map_or("light", |pair| pair[1].as_str());
        anyhow::ensure!(matches!(theme, "light" | "dark"), "Invalid desktop theme");
        if settings {
            return tray::run_settings_development(Language::detect(), initial_window, theme);
        }
        if controlled {
            return tray::run_controlled_development(Language::detect(), initial_window, theme);
        }
        return tray::run_read_only_development(Language::detect(), initial_window, theme);
    }
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
