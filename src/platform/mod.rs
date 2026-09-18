#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use windows::{PlatformMonitor, is_session_locked, play_chime, terminate_process_by_pid};

#[cfg(not(windows))]
compile_error!("miccamwatch currently supports Windows only");
