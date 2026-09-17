use crate::model::{Access, Action, Risk};
use std::process::Command;

/// Sends a Windows desktop toast without interpolating untrusted process data
/// into PowerShell source. The watcher handles deduplication and cooldown.
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
        body.push(
            "Windows permission is denied; loaded modules do not prove frame flow.".to_owned(),
        );
    } else if access.risk == Risk::Suspicious {
        body.push("Multiple risk signals require inspection.".to_owned());
    }
    let detail = body.join(" | ");

    std::thread::spawn(move || {
        const SCRIPT: &str = "[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null; $template = [Windows.UI.Notifications.ToastTemplateType]::ToastText02; $xml = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent($template); $text = $xml.GetElementsByTagName('text'); $text[0].AppendChild($xml.CreateTextNode($env:MCW_TOAST_TITLE)) | Out-Null; $text[1].AppendChild($xml.CreateTextNode($env:MCW_TOAST_DETAIL)) | Out-Null; $notifier = [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\\WindowsPowerShell\\v1.0\\powershell.exe'); $toast = [Windows.UI.Notifications.ToastNotification]::new($xml); $notifier.Show($toast);";
        let _ = Command::new("powershell.exe")
            .env("MCW_TOAST_TITLE", title)
            .env("MCW_TOAST_DETAIL", detail)
            .args(["-NoProfile", "-NonInteractive", "-Command", SCRIPT])
            .output();
    });
}
