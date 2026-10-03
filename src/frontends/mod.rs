pub mod cli;
#[cfg(windows)]
pub mod tray;
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[path = "tray/unix.rs"]
pub mod tray;
#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
pub mod tui;
