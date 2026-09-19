use crate::{
    i18n::Language,
    model::{Access, Action, Risk},
};
use winreg::{RegKey, enums::HKEY_CURRENT_USER};

use windows::{
    Data::Xml::Dom::XmlDocument,
    UI::Notifications::{ToastNotification, ToastNotificationManager, ToastTemplateType},
    core::HSTRING,
};
const APP_ID: &str = "MicCamWatch.MicCamWatch";

pub fn ensure_identity() -> anyhow::Result<()> {
    let root = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = root.create_subkey(format!(r"Software\Classes\AppUserModelId\{APP_ID}"))?;
    key.set_value("DisplayName", &"miccamwatch")?;
    key.set_value("ShowInSettings", &1u32)?;
    Ok(())
}

/// Sends a toast directly through WinRT. The watcher handles deduplication.
pub fn notify_access(access: &Access, action: Action, lang: Language) -> windows::core::Result<()> {
    let title = lang.toast_title(action, access.resource);
    let pid = access
        .pid
        .map(|value| format!("PID {value}"))
        .unwrap_or_else(|| "PID ?".to_owned());
    let mut body = vec![
        format!("{} ({pid})", access.application),
        format!(
            "{}: {} | {}: {}",
            match lang {
                Language::Fr => "risque",
                Language::De => "Risiko",
                Language::Es => "riesgo",
                Language::Ja => "リスク",
                Language::Zh => "风险",
                Language::Ru => "риск",
                Language::En => "risk",
            },
            lang.risk_str(access.risk),
            match lang {
                Language::Fr => "confiance",
                Language::De => "Vertrauen",
                Language::Es => "confianza",
                Language::Ja => "信頼度",
                Language::Zh => "可信度",
                Language::Ru => "достоверность",
                Language::En => "confidence",
            },
            match (lang, access.confidence) {
                (Language::Fr, crate::model::Confidence::High) => "haute",
                (Language::Fr, crate::model::Confidence::Medium) => "moyenne",
                (Language::Fr, crate::model::Confidence::Low) => "basse",
                (Language::De, crate::model::Confidence::High) => "hoch",
                (Language::De, crate::model::Confidence::Medium) => "mittel",
                (Language::De, crate::model::Confidence::Low) => "niedrig",
                (Language::Es, crate::model::Confidence::High) => "alta",
                (Language::Es, crate::model::Confidence::Medium) => "media",
                (Language::Es, crate::model::Confidence::Low) => "baja",
                (Language::Ja, crate::model::Confidence::High) => "高",
                (Language::Ja, crate::model::Confidence::Medium) => "中",
                (Language::Ja, crate::model::Confidence::Low) => "低",
                (Language::Zh, crate::model::Confidence::High) => "高",
                (Language::Zh, crate::model::Confidence::Medium) => "中",
                (Language::Zh, crate::model::Confidence::Low) => "低",
                (Language::Ru, crate::model::Confidence::High) => "высокая",
                (Language::Ru, crate::model::Confidence::Medium) => "средняя",
                (Language::Ru, crate::model::Confidence::Low) => "низкая",
                (Language::En, crate::model::Confidence::High) => "high",
                (Language::En, crate::model::Confidence::Medium) => "medium",
                (Language::En, crate::model::Confidence::Low) => "low",
            }
        ),
    ];
    if let (Some(parent_pid), Some(parent_name)) = (access.parent_pid, &access.parent_name) {
        body.push(format!(
            "{} {parent_name} ({parent_pid})",
            lang.parent_label()
        ));
    }
    if access.risk == Risk::Blocked {
        body.push(match lang {
            Language::Fr => "Permission refusée par Windows.".to_owned(),
            Language::De => "Windows-Berechtigung verweigert.".to_owned(),
            Language::Es => "Permiso denegado por Windows.".to_owned(),
            Language::Ja => "Windowsにより権限が拒否されました。".to_owned(),
            Language::Zh => "Windows权限已被拒绝。".to_owned(),
            Language::Ru => "Разрешение Windows отклонено.".to_owned(),
            Language::En => "Windows permission is denied.".to_owned(),
        });
    } else if access.risk == Risk::Suspicious {
        body.push(match lang {
            Language::Fr => "Plusieurs signaux suspects détectés.".to_owned(),
            Language::De => "Mehrere verdächtige Signale erkannt.".to_owned(),
            Language::Es => "Se detectaron múltiples señales sospechosas.".to_owned(),
            Language::Ja => "複数の不審なシグナルが検出されました。".to_owned(),
            Language::Zh => "检测到多个可疑信号。".to_owned(),
            Language::Ru => "Обнаружено несколько подозрительных сигналов.".to_owned(),
            Language::En => "Multiple suspicious signals detected.".to_owned(),
        });
    }
    let detail = body.join(" | ");

    show_toast(&title, &detail)
}

pub fn notify_message(title: &str, detail: &str) -> windows::core::Result<()> {
    show_toast(title, detail)
}

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
