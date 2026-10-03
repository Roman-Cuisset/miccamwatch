use super::content::access_content;
use crate::{
    i18n::Language,
    model::{Access, Action},
};
use anyhow::{Context, Result, bail};

/// Verifies the native notification service/permission on explicit request.
/// Linux identifies each notification with its freedesktop application name;
/// macOS authorization belongs to the embedded helper's application identity.
pub fn ensure_identity() -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        let connection = notification_connection()?;
        let proxy = notification_proxy(&connection)?;
        let _: (String, String, String, String) = proxy
            .call("GetServerInformation", &())
            .context("freedesktop notification service is unavailable")?;
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        crate::platform::unix::native_effect("notification-identity", &[])?;
        Ok(())
    }
}

/// The watcher owns deduplication; this function reports native delivery errors.
pub fn notify_access(access: &Access, action: Action, lang: Language) -> Result<()> {
    let (title, detail) = access_content(access, action, lang);
    notify_message(&title, &detail)
}

pub fn notify_message(title: &str, detail: &str) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        let connection = notification_connection()?;
        let proxy = notification_proxy(&connection)?;
        let body = escape_body(detail);
        let id: u32 = proxy
            .call(
                "Notify",
                &(
                    "miccamwatch",
                    0u32,
                    "",
                    title,
                    body.as_ref(),
                    Vec::<&str>::new(),
                    std::collections::HashMap::<&str, zbus::zvariant::OwnedValue>::new(),
                    -1i32,
                ),
            )
            .context("freedesktop notification service rejected delivery")?;
        if id == 0 {
            bail!("freedesktop notification service returned an invalid notification identifier");
        }
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        crate::platform::unix::native_effect("notify", &[title, detail])?;
        Ok(())
    }
}

#[cfg(target_os = "linux")]
fn notification_connection() -> Result<zbus::blocking::Connection> {
    zbus::blocking::connection::Builder::session()
        .context("a graphical user's D-Bus session is required for desktop notifications")?
        .method_timeout(std::time::Duration::from_secs(3))
        .build()
        .context("cannot connect to the user's D-Bus notification session")
}

#[cfg(target_os = "linux")]
fn notification_proxy(
    connection: &zbus::blocking::Connection,
) -> Result<zbus::blocking::Proxy<'_>> {
    zbus::blocking::proxy::Builder::new(connection)
        .destination("org.freedesktop.Notifications")?
        .path("/org/freedesktop/Notifications")?
        .interface("org.freedesktop.Notifications")?
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .context("cannot access the freedesktop desktop notification service")
}

// Notification body markup must not turn an executable/application name into
// a link or formatting. Return borrowed ordinary text without an allocation.
#[cfg(target_os = "linux")]
fn escape_body(value: &str) -> std::borrow::Cow<'_, str> {
    if !value.contains(['&', '<', '>']) {
        return std::borrow::Cow::Borrowed(value);
    }
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            character => escaped.push(character),
        }
    }
    std::borrow::Cow::Owned(escaped)
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn untrusted_application_text_cannot_insert_notification_body_markup() {
        assert_eq!(
            escape_body("<a href='https://example.invalid'>capture & input</a>"),
            "&lt;a href='https://example.invalid'&gt;capture &amp; input&lt;/a&gt;"
        );
        assert_eq!(escape_body("Telegram — микрофон"), "Telegram — микрофон");
    }
}
