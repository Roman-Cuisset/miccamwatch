#[cfg(any(target_os = "linux", target_os = "macos"))]
mod unix;
#[cfg(windows)]
mod windows;
#[cfg(feature = "flutter-desktop")]
pub mod enforcement;

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub use unix::{EventDispatcher, TransitionTracker, watch};
#[cfg(windows)]
pub use windows::watch;
