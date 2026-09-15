use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "mcw",
    version,
    about = "See which applications are using your microphone or camera"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Show capture access active right now
    Status(Options),
    /// Print access start and stop events until Ctrl+C
    Watch(WatchOptions),
    /// List active microphone capture devices
    Devices(OutputOptions),
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
}

#[derive(Args, Debug, Default)]
pub struct Filter {
    /// Only report microphone access
    #[arg(long, conflicts_with = "camera")]
    pub microphone: bool,
    /// Only report camera access
    #[arg(long, conflicts_with = "microphone")]
    pub camera: bool,
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
}
