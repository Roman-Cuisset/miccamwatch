#[cfg(windows)]
pub mod autostart;
pub mod collector;
pub mod config;
pub mod frontends;
pub mod history;
pub mod i18n;
pub mod model;
#[cfg(windows)]
pub mod notify;
pub mod output;
pub mod platform;
#[cfg(windows)]
pub mod privacy;
pub mod settings;
#[cfg(windows)]
pub mod updater;
#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
pub mod watcher;
