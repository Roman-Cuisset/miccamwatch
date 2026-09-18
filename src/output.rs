use crate::model::{
    Access, AccessEvent, Action, Device, DiagnosticCheck, Risk, SCHEMA_VERSION, Snapshot,
    StatusDocument,
};
use anyhow::Result;
use chrono::Local;

pub fn print_status(snapshot: &Snapshot, json: bool, min_risk: Option<Risk>) -> Result<()> {
    if json {
        println!(
            "{}",
            serde_json::to_string(&StatusDocument {
                schema_version: SCHEMA_VERSION,
                tool_version: env!("CARGO_PKG_VERSION"),
                collectors: &snapshot.collectors,
                accesses: &snapshot.accesses,
            })?
        );
        return Ok(());
    }

    let visible: Vec<_> = snapshot
        .accesses
        .iter()
        .filter(|a| should_display(a, min_risk))
        .collect();
    if visible.is_empty() {
        println!("No microphone or camera activity detected.");
    }
    for collector in &snapshot.collectors {
        if collector.state != crate::model::CollectorState::Healthy {
            println!("collector {}: {:?}", collector.collector, collector.state);
        }
    }
    for access in &visible {
        print_access(access, None);
    }
    Ok(())
}

pub fn print_event(event: &AccessEvent, json: bool, min_risk: Option<Risk>) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string(event)?);
        return Ok(());
    }
    if !should_display(&event.access, min_risk) {
        return Ok(());
    }

    let time = event.observed_at.with_timezone(&Local).format("%H:%M:%S");
    print!("{time}  ");
    print_access(&event.access, Some(event.action));
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

pub fn print_doctor(checks: &[DiagnosticCheck], json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string(checks)?);
    } else {
        for check in checks {
            println!("[{:>4}]  {}: {}", check.status, check.name, check.detail);
        }
    }
    Ok(())
}

fn should_display(access: &Access, min_risk: Option<Risk>) -> bool {
    min_risk.is_none_or(|min| access.risk >= min)
}

pub fn print_explanation(snapshot: &Snapshot, json: bool, min_risk: Option<Risk>) -> Result<()> {
    print_status(snapshot, json, min_risk)
}

fn print_access(access: &Access, action: Option<Action>) {
    let pid = access
        .pid
        .map(|pid| format!("PID {pid}"))
        .unwrap_or_else(|| "PID ?".to_owned());
    let device = access.device.as_deref().unwrap_or("device unavailable");
    let state = match action {
        Some(Action::Stop) => "STOPPED".to_owned(),
        Some(Action::Update) => "UPDATED".to_owned(),
        _ => access.activity.to_string(),
    };

    println!(
        "{}  {:<10}  {:<12}  {:<28}  {:<10}  {}  [confidence: {}]",
        access.resource, state, access.risk, access.application, pid, device, access.confidence,
    );

    if let (Some(parent_pid), Some(parent_name)) = (access.parent_pid, &access.parent_name) {
        println!("     parent: {parent_name} (PID {parent_pid})");
    }
    if let Some(process) = &access.process {
        if let Some(session_id) = process.session_id {
            println!("     session: {session_id}");
        }
        if !process.ancestry.is_empty() {
            let chain = process
                .ancestry
                .iter()
                .map(|ancestor| format!("{} ({})", ancestor.name, ancestor.pid))
                .collect::<Vec<_>>()
                .join(" <- ");
            println!("     ancestry: {chain}");
        }
    }

    if let Some(sig) = &access.signature {
        if sig.verified {
            let signer_display = sig
                .signer
                .as_deref()
                .unwrap_or("trusted certificate; signer unavailable");
            println!("     signer: {signer_display} [verified]");
        } else if let Some(err) = &sig.error {
            println!("     signature: {err}");
        }
    }

    if !access.modules.is_empty() {
        println!("     modules: {}", access.modules.join(", "));
    }
    for evidence in &access.evidence {
        println!(
            "     evidence: {:?} via {} — {}",
            evidence.kind, evidence.source, evidence.detail
        );
    }
    if access.risk >= Risk::Suspicious {
        println!("     recommended: inspect the process and executable before taking action.");
    }
}
