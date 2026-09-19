use crate::i18n::Language;
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
    /// User interface language (en, fr, de, es, ja, zh, ru)
    #[arg(long, global = true, value_parser = parse_language)]
    pub lang: Option<Language>,
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
    /// Mute or query microphone hardware capture level
    Mute(MuteOptions),
    /// Unmute all microphone capture devices
    Unmute,
    /// Launch the interactive full-terminal live dashboard
    Top,
    /// Run in background as a system tray icon in the Windows notification area
    Tray,
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
    /// Play a discreet chime when microphone or camera access starts
    #[arg(long)]
    pub sound: bool,
    /// Automatically terminate suspicious unauthorized processes accessing camera or microphone (disabled by default)
    #[arg(long, conflicts_with = "no_kill")]
    pub kill_unauthorized: bool,
    /// Disable automatic process termination, overriding policy configuration
    #[arg(long, conflicts_with = "kill_unauthorized")]
    pub no_kill: bool,
}

#[derive(Args, Debug)]
pub struct ExplainOptions {
    /// Process identifier to inspect
    pub pid: u32,
    #[command(flatten)]
    pub output: OutputOptions,
}

#[derive(Args, Debug, Default)]
pub struct MuteOptions {
    /// Toggle mute state (mute if unmuted, unmute if muted)
    #[arg(long, short = 't')]
    pub toggle: bool,
    /// Only check whether microphone is currently muted
    #[arg(long, short = 's')]
    pub status: bool,
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
    /// Include low-confidence camera-ready pipelines without confirmed frame flow
    #[arg(long)]
    pub include_ready: bool,
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

fn parse_language(s: &str) -> Result<Language, String> {
    Language::from_code(s).ok_or_else(|| {
        format!("unknown language '{s}'; supported languages: en, fr, de, es, ja, zh, ru")
    })
}
