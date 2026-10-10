#[cfg(windows)]
pub mod autostart;
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[path = "autostart/unix.rs"]
pub mod autostart;
pub mod collector;
pub mod config;
pub mod frontends;
pub mod history;
pub mod i18n;
pub mod model;
#[cfg(windows)]
pub mod notify;
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[path = "unix_notify.rs"]
pub mod notify;
pub mod output;
pub mod platform;
pub(crate) mod policy;
#[cfg(windows)]
pub mod privacy;
#[cfg(target_os = "macos")]
#[path = "privacy/macos.rs"]
pub mod privacy;
#[cfg(target_os = "linux")]
#[path = "privacy/linux.rs"]
pub mod privacy;
pub mod settings;
#[cfg(windows)]
pub mod updater;
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[path = "updater/unix.rs"]
pub mod updater;
#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
pub mod watcher;
#[cfg(windows)]
pub(crate) mod windows_control;
#[cfg(windows)]
pub(crate) mod windows_tools;
