#[path = "notify/content.rs"]
mod content;
#[path = "notify/unix.rs"]
mod unix;
#[cfg(target_os = "macos")]
pub use unix::native_status;
pub use unix::{ensure_identity, notify_access, notify_message};
