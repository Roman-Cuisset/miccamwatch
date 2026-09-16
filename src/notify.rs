use crate::model::{Access, Action, Detection, ThreatLevel};
use std::process::Command;

/// Sends a Windows desktop toast notification for access events.
pub fn notify_access(access: &Access, action: Action) {
    let status_str = match &access.detection {
        Detection::Forensic { threat, .. } => match (threat, action) {
            (_, Action::Stop) => "CLEARED".to_owned(),
            _ => threat.to_string(),
        },
        _ => match action {
            Action::Start => "ACTIVE".to_owned(),
            Action::Stop => "STOPPED".to_owned(),
        },
    };

    let title = format!("miccamwatch: {} {}", access.resource, status_str);
    let app = &access.application;
    let pid_str = access
        .pid
        .map(|p| format!("PID {p}"))
        .unwrap_or_else(|| "PID ?".to_owned());

    let mut body_lines = vec![format!("{app} ({pid_str})")];
    if let (Some(parent_pid), Some(parent_name)) = (access.parent_pid, &access.parent_name) {
        body_lines.push(format!("parent: {parent_name} ({parent_pid})"));
    }

    if let Detection::Forensic { threat, .. } = &access.detection {
        if *threat == ThreatLevel::Unauthorized {
            body_lines.push("ALERT: Camera access while permission is DENIED!".to_owned());
        } else if *threat == ThreatLevel::Suspect {
            body_lines
                .push("WARNING: Camera capture stack active without privacy event.".to_owned());
        }
    }

    let detail = body_lines.join(" | ");

    // Fire-and-forget background notification so the watcher thread never blocks
    std::thread::spawn(move || {
        let script = format!(
            "[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null; \
             $template = [Windows.UI.Notifications.ToastTemplateType]::ToastText02; \
             $xml = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent($template); \
             $text = $xml.GetElementsByTagName('text'); \
             $text[0].AppendChild($xml.CreateTextNode('{title}')) | Out-Null; \
             $text[1].AppendChild($xml.CreateTextNode('{detail}')) | Out-Null; \
             $notifier = [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('{{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}}\\WindowsPowerShell\\v1.0\\powershell.exe'); \
             $toast = [Windows.UI.Notifications.ToastNotification]::new($xml); \
             $notifier.Show($toast);"
        );

        let _ = Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .output();
    });
}
