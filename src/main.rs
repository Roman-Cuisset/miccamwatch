mod cli;
mod model;
mod notify;
mod output;
mod platform;
mod updater;
mod watcher;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command};
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
    match cli.command {
        Command::Status(options) => {
            let monitor = PlatformMonitor::new()?;
            let accesses = monitor.snapshot(&options.filter)?;
            output::print_status(&accesses, options.output.json)?;
            Ok(u8::from(!accesses.is_empty()))
        }
        Command::Watch(options) => {
            let monitor = PlatformMonitor::new()?;
            watcher::watch(
                &monitor,
                &options.filter,
                options.output.json,
                Duration::from_millis(options.interval),
                options.notify,
            )?;
            Ok(0)
        }
        Command::Devices(options) => {
            let monitor = PlatformMonitor::new()?;
            output::print_devices(&monitor.devices()?, options.json)?;
            Ok(0)
        }
        Command::Explain(options) => {
            let monitor = PlatformMonitor::new()?;
            let accesses: Vec<_> = monitor
                .snapshot(&Default::default())?
                .into_iter()
                .filter(|access| access.pid == Some(options.pid))
                .collect();
            output::print_explanation(&accesses, options.output.json)?;
            Ok(u8::from(accesses.is_empty()))
        }
        Command::Update => {
            updater::update()?;
            Ok(0)
        }
    }
}
