#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::PlatformMonitor;
#[cfg(target_os = "macos")]
pub use macos::PlatformMonitor;
#[cfg(windows)]
pub use windows::{
    PlatformMonitor, SessionLockState, play_chime, session_lock_state, terminate_process_by_pid,
};

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
compile_error!("mcw has no platform backend for this target");
