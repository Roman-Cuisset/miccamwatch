mod content;
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
    UI::Notifications::{ToastNotification, ToastNotificationManager, ToastTemplateType},
    core::HSTRING,
};
#[cfg(windows)]
const APP_ID: &str = "MicCamWatch.MicCamWatch";

#[cfg(windows)]
pub fn ensure_identity() -> anyhow::Result<()> {
    let root = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = root.create_subkey(format!(r"Software\Classes\AppUserModelId\{APP_ID}"))?;
    key.set_value("DisplayName", &"miccamwatch")?;
    key.set_value("ShowInSettings", &1u32)?;
    Ok(())
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
    let xml: XmlDocument =
        ToastNotificationManager::GetTemplateContent(ToastTemplateType::ToastText02)?;
    let text = xml.GetElementsByTagName(&HSTRING::from("text"))?;
    let title_node = text.Item(0)?;
    title_node.AppendChild(&xml.CreateTextNode(&HSTRING::from(title))?)?;
    let detail_node = text.Item(1)?;
    detail_node.AppendChild(&xml.CreateTextNode(&HSTRING::from(detail))?)?;
    let toast = ToastNotification::CreateToastNotification(&xml)?;
    ensure_identity().map_err(|error| {
        windows::core::Error::new(
            windows::core::HRESULT(0x80004005u32 as i32),
            format!("failed to register notification identity: {error:#}"),
        )
    })?;
    let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(APP_ID))?;
    notifier.Show(&toast)
}
