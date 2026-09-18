mod cli;
mod config;
mod model;
mod notify;
mod output;
mod platform;
mod updater;
mod watcher;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command, ConfigCommand};
use config::Policy;
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
    let policy = Policy::load(cli.config.as_deref())?;
    match cli.command {
        Command::Status(options) => {
            let monitor = PlatformMonitor::new(policy.clone())?;
            let snapshot = monitor.snapshot(&options.filter)?;
            let has_access = !snapshot.accesses.is_empty();
            output::print_status(&snapshot, options.output.json, options.filter.risk)?;
            Ok(u8::from(has_access))
        }
        Command::Watch(options) => {
            let monitor = PlatformMonitor::new(policy.clone())?;
            watcher::watch(
                &monitor,
                &options.filter,
                options.output.json,
                Duration::from_millis(options.interval),
                options.notify,
                options.log.as_deref(),
                options.eventlog,
            )?;
            Ok(0)
        }
        Command::Devices(options) => {
            let monitor = PlatformMonitor::new(policy.clone())?;
            output::print_devices(&monitor.devices()?, options.json)?;
            Ok(0)
        }
        Command::Explain(options) => {
            let monitor = PlatformMonitor::new(policy.clone())?;
            let mut snapshot = monitor.snapshot(&Default::default())?;
            snapshot
                .accesses
                .retain(|access| access.pid == Some(options.pid));
            let missing = snapshot.accesses.is_empty();
            output::print_explanation(&snapshot, options.output.json, None)?;
            Ok(u8::from(missing))
        }
        Command::Update => {
            updater::update()?;
            Ok(0)
        }
        Command::Doctor(options) => {
            let monitor = PlatformMonitor::new(policy)?;
            let checks = monitor.doctor();
            output::print_doctor(&checks, options.json)?;
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
