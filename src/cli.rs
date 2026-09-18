use crate::model::Risk;
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "mcw",
    version,
    about = "See which applications are using your microphone or camera"
)]
pub struct Cli {
    /// Policy file in TOML format
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Show capture access active right now
    Status(Options),
    /// Print access start and stop events until Ctrl+C
    Watch(WatchOptions),
    /// List microphone and camera devices
    Devices(OutputOptions),
    /// Explain all evidence currently associated with one process
    Explain(ExplainOptions),
    /// Download and install the latest GitHub release
    Update,
    /// Run self-diagnostics and report system compatibility
    Doctor(OutputOptions),
    /// Validate policy configuration
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Parse and validate the selected policy file
    Validate,
}
#[derive(Args, Debug)]
pub struct Options {
    #[command(flatten)]
    pub filter: Filter,
    #[command(flatten)]
    pub output: OutputOptions,
}

#[derive(Args, Debug)]
pub struct WatchOptions {
    #[command(flatten)]
    pub filter: Filter,
    #[command(flatten)]
    pub output: OutputOptions,
    /// Polling interval in milliseconds
    #[arg(long, default_value_t = 750, value_parser = clap::value_parser!(u64).range(100..))]
    pub interval: u64,
    /// Send Windows desktop toast notifications on access events
    #[arg(long)]
    pub notify: bool,
    /// Append JSONL events to a log file
    #[arg(long)]
    pub log: Option<PathBuf>,
    /// Also write events to the Windows Application event log
    #[arg(long)]
    pub eventlog: bool,
}

#[derive(Args, Debug)]
pub struct ExplainOptions {
    /// Process identifier to inspect
    pub pid: u32,
    #[command(flatten)]
    pub output: OutputOptions,
}

#[derive(Args, Debug, Default)]
pub struct Filter {
    /// Only report microphone access
    #[arg(long, conflicts_with = "camera")]
    pub microphone: bool,
    /// Only report camera access
    #[arg(long, conflicts_with = "microphone")]
    pub camera: bool,
    /// Minimum risk level to display (expected, unexplained, suspicious, blocked)
    #[arg(long, value_parser = parse_risk)]
    pub risk: Option<Risk>,
}

impl Filter {
    pub fn includes_microphone(&self) -> bool {
        self.microphone || !self.camera
    }

    pub fn includes_camera(&self) -> bool {
        self.camera || !self.microphone
    }
}

#[derive(Args, Debug, Default)]
pub struct OutputOptions {
    /// Emit newline-delimited JSON
    #[arg(long)]
    pub json: bool,
    /// Disable colored terminal output
    #[arg(long)]
    pub no_color: bool,
}

fn parse_risk(s: &str) -> Result<Risk, String> {
    match s.to_ascii_lowercase().as_str() {
        "expected" => Ok(Risk::Expected),
        "unexplained" => Ok(Risk::Unexplained),
        "suspicious" => Ok(Risk::Suspicious),
        "blocked" => Ok(Risk::Blocked),
        _ => Err(format!(
            "unknown risk level '{s}'; expected one of: expected, unexplained, suspicious, blocked"
        )),
    }
}
