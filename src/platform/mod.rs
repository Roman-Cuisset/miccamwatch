#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use windows::PlatformMonitor;

#[cfg(not(windows))]
compile_error!("miccamwatch currently supports Windows only");
