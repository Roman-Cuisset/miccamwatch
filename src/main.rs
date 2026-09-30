use anyhow::Result;
use clap::Parser;
#[cfg(windows)]
use colored::Colorize;
#[cfg(not(windows))]
use miccamwatch::model::DiagnosticStatus;
#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
use miccamwatch::watcher;
#[cfg(windows)]
use miccamwatch::{
    autostart,
    frontends::{
        cli::{
            AutostartCommand, CameraCommand, LockPolicyCommand, NotificationCommand, TrayCommand,
        },
        tray, tui,
    },
    model::MicrophoneMuteState,
    privacy, updater,
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

#[cfg(not(windows))]
fn unsupported_command(command: &Command, policy: &Policy) -> Option<&'static str> {
    match command {
        Command::Tray { .. } => Some("tray is only supported on Windows"),
        Command::Autostart { .. } => Some("autostart is only supported on Windows"),
        Command::Camera { .. } => Some("camera privacy control is only supported on Windows"),
        Command::LockPolicy { .. } => Some("lock-policy is only supported on Windows"),
        Command::Update => Some("update is only supported on Windows"),
        Command::Mute(_) | Command::Unmute => {
            Some("microphone mute control is only supported on Windows")
        }
        Command::Notifications { .. } => Some("notifications are only supported on Windows"),
        Command::Top => Some("top is only supported on Windows"),
        Command::Watch(options) if options.eventlog => {
            Some("--eventlog is only supported on Windows")
        }
        Command::Watch(options) if options.notify => Some("--notify is only supported on Windows"),
        Command::Watch(options) if options.sound => Some("--sound is only supported on Windows"),
        Command::Watch(options) if options.kill_unauthorized => {
            Some("--kill-unauthorized is only supported on Windows")
        }
        Command::Watch(options) if !options.no_kill && policy.action == DefensiveAction::Kill => {
            Some("policy action Kill is only supported on Windows")
        }
        _ => None,
    }
}

fn run() -> Result<u8> {
    let cli = Cli::parse();
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
    #[cfg(not(windows))]
    if let Some(message) = unsupported_command(&cli.command, &policy) {
        anyhow::bail!("{message}");
    }
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
        #[cfg(windows)]
        Command::Watch(options) => {
            let monitor = PlatformMonitor::new(policy.clone())?;
            let defensive_kill = if options.no_kill {
                false
            } else if options.kill_unauthorized {
                true
            } else {
                policy.action == DefensiveAction::Kill
            };
            watcher::watch(
                &monitor,
                &options.filter,
                options.output.json,
                Duration::from_millis(options.interval),
                options.notify
                    && settings.notifications_enabled
                    && !settings.notifications_paused(),
                options.log.as_deref(),
                options.eventlog,
                lang,
                options.sound || settings.sound_enabled,
                defensive_kill,
                settings.history_enabled,
            )?;
            Ok(0)
        }
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        Command::Watch(options) => {
            let monitor = PlatformMonitor::new(policy)?;
            watcher::watch(
                &monitor,
                &options.filter,
                options.output.json,
                Duration::from_millis(options.interval),
                options.log.as_deref(),
                lang,
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
        #[cfg(windows)]
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
        #[cfg(windows)]
        Command::Mute(opts) => {
            let monitor = PlatformMonitor::new(policy)?;
            if opts.status {
                let state = monitor.microphone_mute_state()?;
                let message = lang.microphone_status(state);
                match state {
                    MicrophoneMuteState::Muted => println!("{}", message.red().bold()),
                    MicrophoneMuteState::Unmuted => println!("{}", message.green().bold()),
                    MicrophoneMuteState::Unavailable | MicrophoneMuteState::Mixed => {
                        println!("{}", message.yellow().bold())
                    }
                }
                return Ok(0);
            }
            let new_state = if opts.toggle {
                monitor.toggle_microphone_mute()?
            } else {
                monitor.set_microphone_mute(true)?;
                true
            };
            if new_state {
                println!("{}", "✔ Microphone MUTED.".red().bold());
            } else {
                println!("{}", "✔ Microphone UNMUTED.".green().bold());
            }
            Ok(0)
        }
        #[cfg(windows)]
        Command::Unmute => {
            let monitor = PlatformMonitor::new(policy)?;
            monitor.set_microphone_mute(false)?;
            println!("{}", "✔ Microphone UNMUTED.".green().bold());
            Ok(0)
        }
        #[cfg(windows)]
        Command::Top => {
            let monitor = PlatformMonitor::new(policy)?;
            tui::run_tui(monitor, lang)?;
            Ok(0)
        }
        #[cfg(windows)]
        Command::Tray { command } => match command.unwrap_or(TrayCommand::Run) {
            TrayCommand::Run => {
                let monitor = PlatformMonitor::new(policy.clone())?;
                tray::run_tray(monitor, policy, lang, settings)?;
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
        #[cfg(windows)]
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
            Ok(0)
        }
        #[cfg(windows)]
        Command::Autostart { command } => {
            let status_query = matches!(&command, AutostartCommand::Status);
            match command {
                AutostartCommand::Status => {}
                AutostartCommand::Enable => autostart::enable()?,
                AutostartCommand::Disable => autostart::disable()?,
            }
            let state = autostart::state()?;
            println!("{}", format!("{state:?}").to_ascii_lowercase());
            Ok(u8::from(
                status_query && state == autostart::AutostartState::Disabled,
            ))
        }
        Command::Profile { profile } => {
            if let Some(profile) = profile {
                settings.profile = match profile {
                    ProfileArg::Private => PrivacyProfile::Private,
                    ProfileArg::Meeting => PrivacyProfile::Meeting,
                    ProfileArg::Development => PrivacyProfile::Development,
                    ProfileArg::Balanced => PrivacyProfile::Balanced,
                };
                settings.save()?;
            }
            println!("{}", format!("{:?}", settings.profile).to_ascii_lowercase());
            Ok(0)
        }
        #[cfg(windows)]
        Command::Notifications { command } => {
            match command {
                NotificationCommand::Status => {}
                NotificationCommand::Pause { minutes } => {
                    let minutes = i64::try_from(minutes)?;
                    settings.pause_notifications_until =
                        Some(chrono::Utc::now() + chrono::Duration::minutes(minutes));
                    settings.save()?;
                }
                NotificationCommand::Resume => {
                    settings.pause_notifications_until = None;
                    settings.save()?;
                }
            }
            println!(
                "{}",
                if settings.notifications_paused() {
                    "paused"
                } else {
                    "enabled"
                }
            );
            Ok(0)
        }
        #[cfg(windows)]
        Command::LockPolicy { command } => {
            match command {
                LockPolicyCommand::Status => {}
                LockPolicyCommand::Enable { microphone, camera } => {
                    let both = !microphone && !camera;
                    settings.mute_on_lock = microphone || both;
                    settings.block_camera_on_lock = camera || both;
                    settings.restore_on_unlock = true;
                    settings.save()?;
                }
                LockPolicyCommand::Disable => {
                    settings.mute_on_lock = false;
                    settings.block_camera_on_lock = false;
                    settings.save()?;
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
        #[cfg(not(windows))]
        Command::Update
        | Command::Mute(_)
        | Command::Unmute
        | Command::Top
        | Command::Tray { .. }
        | Command::Camera { .. }
        | Command::Autostart { .. }
        | Command::Notifications { .. }
        | Command::LockPolicy { .. } => {
            anyhow::bail!("this command is only supported on Windows")
        }
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
    #[cfg(not(windows))]
    #[test]
    fn unsupported_commands_are_rejected_before_dispatch() {
        let policy = Policy::default();
        for args in [
            vec!["mcw", "tray", "status"],
            vec!["mcw", "autostart", "enable"],
            vec!["mcw", "camera", "block"],
            vec!["mcw", "lock-policy", "disable"],
            vec!["mcw", "update"],
            vec!["mcw", "mute"],
            vec!["mcw", "unmute"],
            vec!["mcw", "top"],
            vec!["mcw", "notifications", "pause", "1"],
            vec!["mcw", "watch", "--notify"],
            vec!["mcw", "watch", "--sound"],
            vec!["mcw", "watch", "--eventlog"],
            vec!["mcw", "watch", "--kill-unauthorized"],
        ] {
            let cli = Cli::try_parse_from(&args).expect("CLI syntax stays available");
            assert!(
                unsupported_command(&cli.command, &policy).is_some(),
                "{args:?}"
            );
        }
    }
    #[cfg(not(windows))]
    #[test]
    fn policy_kill_is_rejected_even_without_explicit_watch_flag() {
        let policy = Policy {
            action: DefensiveAction::Kill,
            ..Policy::default()
        };
        let watch = Cli::try_parse_from(["mcw", "watch"]).unwrap();
        assert_eq!(
            unsupported_command(&watch.command, &policy),
            Some("policy action Kill is only supported on Windows")
        );
    }
}
