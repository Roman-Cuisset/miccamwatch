use anyhow::Result;
use colored::Colorize;
#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
use miccamwatch::frontends::cli::CameraCommand;
#[cfg(not(windows))]
use miccamwatch::model::DiagnosticStatus;
#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
use miccamwatch::privacy;
#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
use miccamwatch::watcher;
use miccamwatch::{
    autostart,
    frontends::{
        cli::{AutostartCommand, LockPolicyCommand, NotificationCommand, TrayCommand},
        tray, tui,
    },
    model::MicrophoneMuteState,
    updater,
};
use miccamwatch::{
    collector::CaptureScope,
    config::{DefensiveAction, Policy, Profile},
    frontends::cli::{Cli, Command, ConfigCommand, HistoryCommand, ProfileArg},
    history,
    i18n::Language,
    model::{CollectorHealth, CollectorState},
    output,
    platform::PlatformMonitor,
    settings,
    settings::{PrivacyProfile, Settings},
};
use std::process::ExitCode;
#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
use std::time::Duration;

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("mcw: {error:#}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<u8> {
    let cli = Cli::parse_localized();
    #[cfg(unix)]
    let tray_eventlog = match &cli.command {
        Command::Tray { eventlog, .. } => *eventlog,
        _ => false,
    };
    #[cfg(unix)]
    if matches!(
        &cli.command,
        Command::Autostart {
            command: AutostartCommand::Refresh
        }
    ) {
        // Installer refresh depends only on the existing owned registration,
        // not on whether an unrelated capture policy currently validates.
        autostart::refresh_if_enabled(env!("CARGO_PKG_VERSION"))?;
        return Ok(0);
    }
    let no_color = match &cli.command {
        Command::Status(opts) => opts.output.no_color,
        Command::Watch(opts) => opts.output.no_color,
        Command::Devices(opts) => opts.no_color,
        Command::Explain(opts) => opts.output.no_color,
        Command::Doctor(opts) => opts.no_color,
        _ => false,
    };
    if no_color {
        colored::control::set_override(false);
    }
    #[cfg(windows)]
    if !no_color {
        let _ = colored::control::set_virtual_terminal(true);
    }
    let mut settings = Settings::load()?;
    let default_policy = settings::default_policy_path()?;
    let policy_path = cli
        .config
        .as_deref()
        .or_else(|| default_policy.exists().then_some(default_policy.as_path()));
    let mut policy = Policy::load(policy_path)?;
    if cli.config.is_none() && !default_policy.exists() {
        policy.profile = match settings.profile {
            PrivacyProfile::Private => Profile::Strict,
            PrivacyProfile::Development => Profile::Conservative,
            PrivacyProfile::Meeting | PrivacyProfile::Balanced => Profile::Balanced,
        };
    }
    let lang = cli
        .lang
        .or_else(|| policy.language.as_deref().and_then(Language::from_code))
        .unwrap_or_else(Language::detect);
    match cli.command {
        Command::Status(options) => {
            let monitor = PlatformMonitor::new(policy.clone())?;
            let snapshot = monitor.snapshot((&options.filter).into())?;
            let has_access = snapshot
                .accesses
                .iter()
                .any(|access| options.filter.risk.is_none_or(|risk| access.risk >= risk));
            let code = status_exit_code(has_access, &snapshot.collectors);
            output::print_status(&snapshot, options.output.json, options.filter.risk, lang)?;
            Ok(code)
        }
        Command::Watch(options) => {
            let monitor = PlatformMonitor::new(policy.clone())?;
            let defensive_kill = if options.no_kill {
                false
            } else if options.kill_unauthorized {
                true
            } else {
                policy.action == DefensiveAction::Kill
            };
            #[cfg(windows)]
            let notify = options.notify
                && settings.notifications_enabled
                && !settings.notifications_paused();
            #[cfg(unix)]
            let notify = options.notify;
            watcher::watch(
                &monitor,
                &options.filter,
                options.output.json,
                Duration::from_millis(options.interval),
                notify,
                options.log.as_deref(),
                options.eventlog,
                lang,
                options.sound || settings.sound_enabled,
                defensive_kill,
                settings.history_enabled,
            )?;
            Ok(0)
        }
        Command::Devices(options) => {
            let monitor = PlatformMonitor::new(policy.clone())?;
            output::print_devices(&monitor.devices()?, options.json, lang)?;
            Ok(0)
        }
        Command::Explain(options) => {
            let monitor = PlatformMonitor::new(policy.clone())?;
            let mut snapshot = monitor.snapshot(CaptureScope::default())?;
            snapshot
                .accesses
                .retain(|access| access.pid == Some(options.pid));
            let unavailable = snapshot
                .collectors
                .iter()
                .any(|health| health.state == CollectorState::Unavailable);
            let missing = snapshot.accesses.is_empty();
            output::print_explanation(&snapshot, options.output.json, None, lang)?;
            Ok(if cfg!(not(windows)) && unavailable {
                2
            } else {
                u8::from(missing)
            })
        }
        Command::Update => {
            updater::update()?;
            Ok(0)
        }
        Command::Doctor(options) => {
            let monitor = PlatformMonitor::new(policy)?;
            let checks = monitor.doctor();
            #[cfg(not(windows))]
            let unavailable = checks
                .iter()
                .any(|check| check.status == DiagnosticStatus::Error);
            output::print_doctor(&checks, options.json, lang)?;
            #[cfg(not(windows))]
            if unavailable {
                return Ok(2);
            }
            Ok(0)
        }
        Command::Mute(opts) => {
            let monitor = PlatformMonitor::new(policy)?;
            if opts.status {
                let state = monitor.microphone_mute_state()?;
                let message = microphone_message(lang, state);
                match state {
                    MicrophoneMuteState::Muted => println!("{}", message.red().bold()),
                    MicrophoneMuteState::Unmuted => println!("{}", message.green().bold()),
                    MicrophoneMuteState::Unavailable | MicrophoneMuteState::Mixed => {
                        println!("{}", message.yellow().bold())
                    }
                }
                return Ok(0);
            }
            if opts.toggle {
                monitor.toggle_microphone_mute()?;
            } else {
                monitor.set_microphone_mute(true)?;
            }
            println!(
                "{}",
                microphone_message(lang, monitor.microphone_mute_state()?)
            );
            Ok(0)
        }
        Command::Unmute => {
            let monitor = PlatformMonitor::new(policy)?;
            monitor.set_microphone_mute(false)?;
            println!(
                "{}",
                microphone_message(lang, monitor.microphone_mute_state()?)
            );
            Ok(0)
        }
        Command::Top => {
            tui::run_tui(policy, lang)?;
            Ok(0)
        }
        Command::Tray { command, .. } => match command.unwrap_or(TrayCommand::Run) {
            TrayCommand::Run => {
                let monitor = PlatformMonitor::new(policy.clone())?;
                #[cfg(windows)]
                tray::run_tray(monitor, policy, lang, settings)?;
                #[cfg(unix)]
                tray::run_tray(
                    monitor,
                    policy,
                    lang,
                    settings,
                    cli.config.is_some(),
                    tray_eventlog,
                )?;
                Ok(0)
            }
            TrayCommand::Stop => {
                let stopped = tray::stop_running()?;
                println!(
                    "{}",
                    if stopped {
                        "Tray stopped."
                    } else {
                        "Tray is not running."
                    }
                );
                Ok(u8::from(!stopped))
            }
            TrayCommand::Status => {
                let running = tray::is_running();
                println!("{}", if running { "running" } else { "stopped" });
                Ok(u8::from(!running))
            }
        },
        #[cfg(any(windows, target_os = "macos"))]
        Command::Camera { command } => {
            let state = match command {
                CameraCommand::Status => privacy::camera_state()?,
                CameraCommand::Allow => {
                    privacy::set_camera_state(privacy::CameraPrivacyState::Allowed)?;
                    // A camera that is unplugged right now leaves the switch partially
                    // engaged, so report what Windows actually ends up in.
                    privacy::camera_state()?
                }
                CameraCommand::Block => {
                    privacy::set_camera_state(privacy::CameraPrivacyState::Blocked)?;
                    privacy::camera_state()?
                }
                CameraCommand::Toggle => privacy::toggle_camera()?,
            };
            println!("{}", state.as_str());
            #[cfg(target_os = "macos")]
            println!("{}", privacy::camera_detail()?);
            Ok(0)
        }
        Command::Autostart { command } => {
            let status_query = matches!(&command, AutostartCommand::Status);
            match command {
                AutostartCommand::Status => {}
                AutostartCommand::Enable => autostart::enable()?,
                AutostartCommand::Disable => autostart::disable()?,
                #[cfg(unix)]
                AutostartCommand::Refresh => {
                    unreachable!("refresh is handled before policy loading")
                }
            }
            let state = autostart::state()?;
            println!("{}", format!("{state:?}").to_ascii_lowercase());
            Ok(u8::from(
                status_query && state == autostart::AutostartState::Disabled,
            ))
        }
        Command::Profile { profile } => {
            if let Some(profile) = profile {
                settings = Settings::update(|current| {
                    current.profile = match profile {
                        ProfileArg::Private => PrivacyProfile::Private,
                        ProfileArg::Meeting => PrivacyProfile::Meeting,
                        ProfileArg::Development => PrivacyProfile::Development,
                        ProfileArg::Balanced => PrivacyProfile::Balanced,
                    };
                })?;
            }
            println!("{}", format!("{:?}", settings.profile).to_ascii_lowercase());
            Ok(0)
        }
        Command::Notifications { command } => {
            match command {
                NotificationCommand::Status => {}
                NotificationCommand::Pause { minutes } => {
                    let minutes = i64::try_from(minutes)?;
                    settings = Settings::update(|current| {
                        current.pause_notifications_until =
                            Some(chrono::Utc::now() + chrono::Duration::minutes(minutes));
                    })?;
                }
                NotificationCommand::Resume => {
                    settings =
                        Settings::update(|current| current.pause_notifications_until = None)?;
                }
            }
            println!(
                "{}",
                if !settings.notifications_enabled {
                    "disabled"
                } else if settings.notifications_paused() {
                    "paused"
                } else {
                    "enabled"
                }
            );
            Ok(0)
        }
        Command::LockPolicy { command } => {
            #[cfg(target_os = "macos")]
            if !matches!(command, LockPolicyCommand::Disable) {
                anyhow::bail!(
                    "macOS has no supported public session-lock signal; automatic lock actions are unavailable"
                );
            }
            match command {
                LockPolicyCommand::Status => {}
                LockPolicyCommand::Enable { microphone, camera } => {
                    #[cfg(target_os = "linux")]
                    if camera {
                        anyhow::bail!(
                            "Automatic Linux camera-on-lock is unavailable: USB controls require explicit administrator/Polkit authorization before locking; use camera block manually or lock-policy enable --microphone"
                        );
                    }
                    let both = !microphone && !camera;
                    settings = Settings::update(|current| {
                        current.mute_on_lock = microphone || both;
                        current.block_camera_on_lock = cfg!(windows) && (camera || both);
                        current.restore_on_unlock = true;
                    })?;
                }
                LockPolicyCommand::Disable => {
                    settings = Settings::update(|current| {
                        current.mute_on_lock = false;
                        current.block_camera_on_lock = false;
                    })?;
                }
            }
            println!(
                "microphone={} camera={} restore={}",
                settings.mute_on_lock, settings.block_camera_on_lock, settings.restore_on_unlock
            );
            Ok(0)
        }

        Command::History { command } => {
            match command {
                HistoryCommand::Path => println!("{}", history::path()?.display()),
                HistoryCommand::Clear => {
                    history::clear()?;
                    println!("History cleared.");
                }
            }
            Ok(0)
        }
        Command::Config { command } => {
            match command {
                ConfigCommand::Validate => {
                    if cli.config.is_none() && !default_policy.exists() {
                        anyhow::bail!(
                            "no policy found; pass --config <PATH> or create {}",
                            default_policy.display()
                        );
                    }
                    println!("Policy configuration is valid.");
                }
                ConfigCommand::Path => println!("{}", default_policy.display()),
                ConfigCommand::SettingsPath => {
                    println!("{}", settings::settings_path()?.display())
                }
            }
            Ok(0)
        }
        #[cfg(target_os = "linux")]
        Command::Camera { command } => {
            let state = match command {
                CameraCommand::Status => privacy::camera_state()?,
                CameraCommand::Allow => {
                    privacy::set_camera_state(privacy::CameraPrivacyState::Allowed)?;
                    privacy::camera_state()?
                }
                CameraCommand::Block => {
                    privacy::set_camera_state(privacy::CameraPrivacyState::Blocked)?;
                    privacy::camera_state()?
                }
                CameraCommand::Toggle => privacy::toggle_camera_state()?,
            };
            println!(
                "{}",
                match state {
                    privacy::CameraPrivacyState::Allowed => "allowed",
                    privacy::CameraPrivacyState::Blocked => "blocked",
                    privacy::CameraPrivacyState::SystemManaged => "system_managed",
                }
            );
            println!("{}", privacy::camera_detail()?);
            Ok(0)
        }
    }
}

fn microphone_message(lang: Language, state: MicrophoneMuteState) -> String {
    #[cfg(windows)]
    return lang.microphone_status(state).to_owned();
    #[cfg(unix)]
    {
        let scope = match lang {
            Language::En => [
                "PipeWire session sources only",
                "writable CoreAudio inputs only",
            ],
            Language::Fr => [
                "sources de session PipeWire uniquement",
                "entrées CoreAudio modifiables uniquement",
            ],
            Language::De => [
                "nur PipeWire-Sitzungsquellen",
                "nur schreibbare CoreAudio-Eingänge",
            ],
            Language::Es => [
                "solo fuentes de sesión PipeWire",
                "solo entradas CoreAudio modificables",
            ],
            Language::Ja => [
                "PipeWire セッションソースのみ",
                "書き込み可能な CoreAudio 入力のみ",
            ],
            Language::Zh => ["仅 PipeWire 会话源", "仅可写 CoreAudio 输入"],
            Language::Ru => [
                "только источники сеанса PipeWire",
                "только доступные для записи входы CoreAudio",
            ],
        };
        format!(
            "{} ({})",
            lang.microphone_status(state),
            scope[usize::from(cfg!(target_os = "macos"))]
        )
    }
}

fn status_exit_code(has_access: bool, collectors: &[CollectorHealth]) -> u8 {
    if collectors.iter().any(|health| {
        health.state == CollectorState::Unavailable
            || (cfg!(not(windows)) && health.state == CollectorState::Degraded)
    }) {
        2
    } else {
        u8::from(has_access)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_reports_unavailable_collector_instead_of_false_all_clear() {
        let failed = CollectorHealth {
            collector: "wasapi",
            state: CollectorState::Unavailable,
            detail: Some("capture unavailable".into()),
        };
        assert_eq!(status_exit_code(false, std::slice::from_ref(&failed)), 2);
        assert_eq!(status_exit_code(true, &[failed]), 2);
        assert_eq!(status_exit_code(false, &[]), 0);
        assert_eq!(status_exit_code(true, &[]), 1);
        #[cfg(not(windows))]
        assert_eq!(
            status_exit_code(
                false,
                &[CollectorHealth {
                    collector: "pipewire_video",
                    state: CollectorState::Degraded,
                    detail: Some("direct V4L2 streaming cannot be confirmed".into()),
                }]
            ),
            2
        );
    }
}
