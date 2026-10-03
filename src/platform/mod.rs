#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub mod unix;
#[cfg(windows)]
mod windows;
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub use unix::{SessionLockState, play_chime, session_lock_state, terminate_process_by_pid};

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
