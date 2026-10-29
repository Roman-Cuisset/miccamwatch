mod content;
#[cfg(windows)]
pub(crate) mod capture;
#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use unix::{ensure_identity, notify_access, notify_message};

#[cfg(windows)]
use crate::{
    i18n::Language,
    model::{Access, Action},
};
#[cfg(windows)]
use winreg::{RegKey, enums::HKEY_CURRENT_USER};

#[cfg(windows)]
use windows::{
    Data::Xml::Dom::XmlDocument,
    UI::Notifications::{NotificationSetting, ToastNotification, ToastNotificationManager, ToastTemplateType},
    core::HSTRING,
};
#[cfg(windows)]
pub(crate) const APP_ID: &HSTRING = windows::core::h!("MicCamWatch.MicCamWatch");

#[cfg(windows)]
#[derive(Clone, Copy)]
pub(crate) enum NotificationIdentity {
    Installed,
    Qualification,
}

#[cfg(windows)]
fn identity_id(identity: NotificationIdentity) -> &'static HSTRING {
    match identity {
        NotificationIdentity::Installed => APP_ID,
        NotificationIdentity::Qualification => windows::core::h!("MicCamWatch.Qualification"),
    }
}

#[cfg(windows)]
fn ensure_identity_for(identity: NotificationIdentity) -> anyhow::Result<()> {
    let root = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = root.create_subkey(format!(
        r"Software\Classes\AppUserModelId\{}", identity_id(identity)
    ))?;
    key.set_value("DisplayName", &match identity {
        NotificationIdentity::Installed => "MicCamWatch",
        NotificationIdentity::Qualification => "MicCamWatch TEST Qualification",
    })?;
    key.set_value("ShowInSettings", &1u32)?;
    Ok(())
}

#[cfg(windows)]
pub fn ensure_identity() -> anyhow::Result<()> {
    ensure_identity_for(NotificationIdentity::Installed)
}

#[cfg(windows)]
/// Sends a toast directly through WinRT. The watcher handles deduplication.
pub fn notify_access(access: &Access, action: Action, lang: Language) -> windows::core::Result<()> {
    let (title, detail) = content::access_content(access, action, lang);

    show_toast(&title, &detail)
}

#[cfg(windows)]
pub fn notify_message(title: &str, detail: &str) -> windows::core::Result<()> {
    show_toast(title, detail)
}

#[cfg(windows)]
fn show_toast(title: &str, detail: &str) -> windows::core::Result<()> {
    deliver_message(title, detail, NotificationIdentity::Installed).map(|_| ())
}

#[cfg(windows)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeliveryOutcome {
    SuppressedDisabled,
    SuppressedPaused,
    Submitted,
}

#[cfg(windows)]
pub(crate) fn deliver_message(
    title: &str,
    detail: &str,
    identity: NotificationIdentity,
) -> windows::core::Result<DeliveryOutcome> {
    let settings = crate::settings::Settings::load().map_err(|error| {
        windows::core::Error::new(
            windows::core::HRESULT(0x80004005u32 as i32),
            format!("failed to read notification preferences: {error:#}"),
        )
    })?;
    if let Some(suppressed) = preference_gate(&settings) {
        return Ok(suppressed);
    }
    let xml = toast_document(title, detail, settings.sound_enabled)?;
    let toast = ToastNotification::CreateToastNotification(&xml)?;
    ensure_identity_for(identity).map_err(|error| {
        windows::core::Error::new(
            windows::core::HRESULT(0x80004005u32 as i32),
            format!("failed to register notification identity: {error:#}"),
        )
    })?;
    let notifier = ToastNotificationManager::CreateToastNotifierWithId(identity_id(identity))?;
    let catalog_missing = match notifier.Setting() {
        Ok(NotificationSetting::Enabled) => false,
        Ok(setting) => return Err(unavailable_notification_setting(setting)),
        // An unpackaged AUMID's notification catalog entry is created by its
        // first official Show, not by CreateToastNotifier/registry metadata.
        // Show still enforces Windows policy; no OS preference is changed here.
        Err(error) if error.code() == windows::core::HRESULT(0x80070490u32 as i32) => true,
        Err(error) => return Err(error),
    };
    notifier.Show(&toast)?;
    if catalog_missing {
        // Never turn absent/unknown permission into an enabled claim.
        let setting = notifier.Setting()?;
        if setting != NotificationSetting::Enabled {
            return Err(unavailable_notification_setting(setting));
        }
    }
    Ok(DeliveryOutcome::Submitted)
}

#[cfg(windows)]
fn unavailable_notification_setting(setting: NotificationSetting) -> windows::core::Error {
    windows::core::Error::new(
        windows::core::HRESULT(0x80070005u32 as i32),
        format!("Windows notifications unavailable for this application (setting {})", setting.0),
    )
}

#[cfg(windows)]
fn preference_gate(settings: &crate::settings::Settings) -> Option<DeliveryOutcome> {
    preference_gate_at(settings, chrono::Utc::now())
}

#[cfg(windows)]
fn preference_gate_at(
    settings: &crate::settings::Settings,
    now: chrono::DateTime<chrono::Utc>,
) -> Option<DeliveryOutcome> {
    if !settings.notifications_enabled {
        Some(DeliveryOutcome::SuppressedDisabled)
    } else if settings.pause_notifications_until.is_some_and(|until| until > now) {
        Some(DeliveryOutcome::SuppressedPaused)
    } else {
        None
    }
}

#[cfg(windows)]
fn toast_document(title: &str, detail: &str, sound: bool) -> windows::core::Result<XmlDocument> {
    let xml: XmlDocument =
        ToastNotificationManager::GetTemplateContent(ToastTemplateType::ToastText02)?;
    let text = xml.GetElementsByTagName(&HSTRING::from("text"))?;
    let title_node = text.Item(0)?;
    title_node.AppendChild(&xml.CreateTextNode(&HSTRING::from(title))?)?;
    let detail_node = text.Item(1)?;
    detail_node.AppendChild(&xml.CreateTextNode(&HSTRING::from(detail))?)?;
    let audio = xml.CreateElement(&HSTRING::from("audio"))?;
    audio.SetAttribute(
        &HSTRING::from("silent"),
        &HSTRING::from(if sound { "false" } else { "true" }),
    )?;
    xml.DocumentElement()?.AppendChild(&audio)?;
    Ok(xml)
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};

    #[test]
    fn live_master_and_pause_gates_are_conservative_at_exact_expiry() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-10-29T12:00:00Z")
            .unwrap().with_timezone(&chrono::Utc);
        let mut settings = crate::settings::Settings {
            notifications_enabled: false,
            sound_enabled: true,
            pause_notifications_until: Some(now + chrono::Duration::seconds(1)),
            ..Default::default()
        };
        assert_eq!(preference_gate_at(&settings, now), Some(DeliveryOutcome::SuppressedDisabled));
        settings.notifications_enabled = true;
        assert_eq!(preference_gate_at(&settings, now), Some(DeliveryOutcome::SuppressedPaused));
        assert_eq!(preference_gate_at(&settings, now + chrono::Duration::seconds(1)), None);
        settings.pause_notifications_until = None;
        assert_eq!(preference_gate_at(&settings, now), None);
        settings.notifications_enabled = false;
        assert_eq!(preference_gate_at(&settings, now), Some(DeliveryOutcome::SuppressedDisabled));
    }

    #[test]
    fn qualification_identity_is_not_the_installed_identity() {
        assert_ne!(identity_id(NotificationIdentity::Qualification), APP_ID);
    }

    #[test]
    fn native_toast_audio_obeys_the_sound_preference_without_delivery() -> anyhow::Result<()> {
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok()? };
        let result = (|| -> anyhow::Result<()> {
            for sound in [false, true] {
                let document = toast_document("MCW", "Preference qualification", sound)?;
                let toast = ToastNotification::CreateToastNotification(&document)?;
                let audio = toast
                    .Content()?
                    .GetElementsByTagName(&HSTRING::from("audio"))?;
                assert_eq!(audio.Length()?, 1);
                let silent = audio
                    .Item(0)?
                    .Attributes()?
                    .GetNamedItem(&HSTRING::from("silent"))?;
                assert_eq!(
                    silent.InnerText()?.to_string(),
                    if sound { "false" } else { "true" }
                );
            }
            Ok(())
        })();
        unsafe { CoUninitialize() };
        result
    }
}
