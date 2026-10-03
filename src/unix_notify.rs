#[path = "notify/content.rs"]
mod content;
#[path = "notify/unix.rs"]
mod unix;
pub use unix::{ensure_identity, notify_access, notify_message};
