#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use windows::{
    PlatformMonitor, SessionLockState, play_chime, session_lock_state, terminate_process_by_pid,
};

#[cfg(not(windows))]
compile_error!("miccamwatch currently supports Windows only");
