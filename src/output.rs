use crate::model::{Access, AccessEvent, Action, Detection, Device, ThreatLevel};
use anyhow::Result;
use chrono::Local;

pub fn print_status(accesses: &[Access], json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string(accesses)?);
        return Ok(());
    }

    if accesses.is_empty() {
        println!("No microphone or camera access detected.");
        return Ok(());
    }

    for access in accesses {
        print_access(access, "ACTIVE");
    }
    Ok(())
}

pub fn print_event(event: &AccessEvent, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string(event)?);
        return Ok(());
    }

    let action = match event.action {
        Action::Start => "START",
        Action::Stop => "STOP",
    };
    let time = event.observed_at.with_timezone(&Local).format("%H:%M:%S");
    print!("{time}  ");
    print_access(&event.access, action);
    Ok(())
}

pub fn print_devices(devices: &[Device], json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string(devices)?);
    } else if devices.is_empty() {
        println!("No microphone or camera device found.");
    } else {
        for device in devices {
            println!("{}  {}  {}", device.resource, device.name, device.id);
        }
    }
    Ok(())
}

fn print_access(access: &Access, action: &str) {
    let pid = access
        .pid
        .map(|pid| format!("PID {pid}"))
        .unwrap_or_else(|| "PID ?".to_owned());
    let device = access.device.as_deref().unwrap_or("device unavailable");

    match &access.detection {
        Detection::Api { confidence } | Detection::PrivacyActivity { confidence } => {
            let tag = match confidence {
                crate::model::Confidence::Confirmed => "confirmed",
                crate::model::Confidence::Inferred => "inferred",
            };
            println!(
                "{}  {:<14}  {:<28}  {:<10}  {}  [{}]",
                access.resource, action, access.application, pid, device, tag
            );
        }
        Detection::Forensic { threat, .. } => {
            let label = match (threat, action) {
                (_, "STOP") => "CLEARED".to_owned(),
                _ => threat.to_string(),
            };
            println!(
                "{}  {:<14}  {:<28}  {:<10}  {}  [forensic]",
                access.resource, label, access.application, pid, device,
            );
        }
    }

    if let (Some(parent_pid), Some(parent_name)) = (access.parent_pid, &access.parent_name) {
        println!("     parent: {parent_name} (PID {parent_pid})");
    }

    if let Some(sig) = &access.signature {
        if sig.verified {
            let signer_display = sig.signer.as_deref().unwrap_or("trusted certificate");
            println!("     signer: {signer_display} [verified]");
        } else if let Some(err) = &sig.error {
            println!("     signature: {err}");
        }
    }

    if let Detection::Forensic {
        threat,
        modules,
        reasons,
    } = &access.detection
    {
        if !modules.is_empty() {
            println!("     modules: {}", modules.join(", "));
        }
        for reason in reasons {
            println!("     reason: {reason}");
        }
        if *threat == ThreatLevel::Unauthorized {
            println!(
                "     ⚠ Recommended: terminate process / inspect executable / disconnect camera."
            );
        }
    }
}
