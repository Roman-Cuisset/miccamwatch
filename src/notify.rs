use crate::model::{Access, Action, Risk};
use windows::{
    Data::Xml::Dom::XmlDocument,
    UI::Notifications::{ToastNotification, ToastNotificationManager, ToastTemplateType},
    core::HSTRING,
};

/// Sends a toast directly through WinRT. The watcher handles deduplication.
pub fn notify_access(access: &Access, action: Action) {
    let state = match action {
        Action::Start => access.activity.to_string(),
        Action::Update => "UPDATED".to_owned(),
        Action::Stop => "STOPPED".to_owned(),
    };
    let title = format!("miccamwatch: {} {state}", access.resource);
    let pid = access
        .pid
        .map(|value| format!("PID {value}"))
        .unwrap_or_else(|| "PID ?".to_owned());
    let mut body = vec![
        format!("{} ({pid})", access.application),
        format!("risk: {} | confidence: {}", access.risk, access.confidence),
    ];
    if let (Some(parent_pid), Some(parent_name)) = (access.parent_pid, &access.parent_name) {
        body.push(format!("parent: {parent_name} ({parent_pid})"));
    }
    if access.risk == Risk::Blocked {
        body.push("Windows permission is denied; frame flow is not proven.".to_owned());
    } else if access.risk == Risk::Suspicious {
        body.push("Multiple risk signals require inspection.".to_owned());
    }
    let detail = body.join(" | ");

    std::thread::spawn(move || {
        let _ = show_toast(&title, &detail);
    });
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
    // Reuse the registered Windows PowerShell AUMID only as the toast identity;
    // no PowerShell process or script is started.
    let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(
        r"{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\WindowsPowerShell\v1.0\powershell.exe",
    ))?;
    notifier.Show(&toast)
}
