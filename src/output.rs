use crate::model::{Access, AccessEvent, Action, Device};
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
        println!("No active microphone capture device found.");
    } else {
        for device in devices {
            println!("{}  {}  {}", device.resource, device.name, device.id);
        }
        println!("CAM  Physical camera identity is unavailable from the privacy activity backend.");
    }
    Ok(())
}

fn print_access(access: &Access, action: &str) {
    let pid = access
        .pid
        .map(|pid| format!("PID {pid}"))
        .unwrap_or_else(|| "PID ?".to_owned());
    let device = access.device.as_deref().unwrap_or("device unavailable");
    let confidence = match access.confidence {
        crate::model::Confidence::Confirmed => "confirmed",
        crate::model::Confidence::Inferred => "inferred",
    };
    println!(
        "{}  {:<6}  {:<28}  {:<10}  {}  [{}]",
        access.resource, action, access.application, pid, device, confidence
    );
}
