#[cfg(feature = "flutter-desktop")]
pub mod enforcement;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub use unix::{EventDispatcher, watch};
#[cfg(any(unix, feature = "flutter-desktop"))]
mod transitions;
#[cfg(any(unix, feature = "flutter-desktop"))]
pub use transitions::TransitionTracker;
#[cfg(windows)]
pub use windows::watch;
