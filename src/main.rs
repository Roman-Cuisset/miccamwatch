mod cli;
mod config;
mod i18n;
mod model;
mod notify;
mod output;
mod platform;
mod tray;
mod tui;
mod updater;
mod watcher;
use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command, ConfigCommand};
use colored::Colorize;
use config::Policy;
use i18n::Language;
use platform::PlatformMonitor;
use std::{process::ExitCode, time::Duration};

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
    } else {
        let _ = colored::control::set_virtual_terminal(true);
    }
    let policy = Policy::load(cli.config.as_deref())?;
    let lang = cli
        .lang
        .or_else(|| policy.language.as_deref().and_then(Language::from_code))
        .unwrap_or_else(Language::detect);
    match cli.command {
        Command::Status(options) => {
            let monitor = PlatformMonitor::new(policy.clone())?;
            let snapshot = monitor.snapshot(&options.filter)?;
            let has_access = !snapshot.accesses.is_empty();
            output::print_status(&snapshot, options.output.json, options.filter.risk, lang)?;
            Ok(u8::from(has_access))
        }
        Command::Watch(options) => {
            let monitor = PlatformMonitor::new(policy.clone())?;
            let defensive_kill = if options.no_kill {
                false
            } else if options.kill_unauthorized {
                true
            } else {
                policy.action == crate::config::DefensiveAction::Kill
            };
            watcher::watch(
                &monitor,
                &options.filter,
                options.output.json,
                Duration::from_millis(options.interval),
                options.notify,
                options.log.as_deref(),
                options.eventlog,
                lang,
                options.sound,
                defensive_kill,
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
            let mut snapshot = monitor.snapshot(&Default::default())?;
            snapshot
                .accesses
                .retain(|access| access.pid == Some(options.pid));
            let missing = snapshot.accesses.is_empty();
            output::print_explanation(&snapshot, options.output.json, None, lang)?;
            Ok(u8::from(missing))
        }
        Command::Update => {
            updater::update()?;
            Ok(0)
        }
        Command::Doctor(options) => {
            let monitor = PlatformMonitor::new(policy)?;
            let checks = monitor.doctor();
            output::print_doctor(&checks, options.json, lang)?;
            Ok(0)
        }
        Command::Mute(opts) => {
            let monitor = PlatformMonitor::new(policy)?;
            if opts.status {
                let muted = monitor.get_microphone_mute()?;
                if muted {
                    println!("{}", "Microphone is MUTED.".red().bold());
                } else {
                    println!("{}", "Microphone is UNMUTED (active).".green().bold());
                }
                return Ok(if muted { 1 } else { 0 });
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
        Command::Unmute => {
            let monitor = PlatformMonitor::new(policy)?;
            monitor.set_microphone_mute(false)?;
            println!("{}", "✔ Microphone UNMUTED.".green().bold());
            Ok(0)
        }
        Command::Top => {
            let monitor = PlatformMonitor::new(policy)?;
            tui::run_tui(monitor, lang)?;
            Ok(0)
        }
        Command::Tray => {
            let monitor = PlatformMonitor::new(policy)?;
            tray::run_tray(monitor, lang)?;
            Ok(0)
        }
        Command::Config {
            command: ConfigCommand::Validate,
        } => {
            if cli.config.is_none() {
                anyhow::bail!("--config <PATH> is required for config validate");
            }
            println!("Policy configuration is valid.");
            Ok(0)
        }
    }
}
