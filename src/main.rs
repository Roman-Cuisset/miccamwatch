mod cli;
mod model;
mod output;
mod platform;
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
    let monitor = PlatformMonitor::new()?;

    match cli.command {
        Command::Status(options) => {
            let accesses = monitor.snapshot(&options.filter)?;
            output::print_status(&accesses, options.output.json)?;
            Ok(u8::from(!accesses.is_empty()))
        }
        Command::Watch(options) => {
            watcher::watch(
                &monitor,
                &options.filter,
                options.output.json,
                Duration::from_millis(options.interval),
            )?;
            Ok(0)
        }
        Command::Devices(options) => {
            output::print_devices(&monitor.devices()?, options.json)?;
            Ok(0)
        }
    }
}
