use crate::model::{
    Access, AccessEvent, Action, Activity, Confidence, Device, DiagnosticCheck, DiagnosticStatus,
    Resource, Risk, SCHEMA_VERSION, Snapshot, StatusDocument,
};
use anyhow::Result;
use chrono::Local;
use colored::Colorize;

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
        println!(
            "{}",
            "✔ No microphone or camera activity detected."
                .green()
                .bold()
        );
    }
    for collector in &snapshot.collectors {
        match collector.state {
            crate::model::CollectorState::Healthy => {}
            crate::model::CollectorState::Degraded => {
                println!(
                    "  {} collector {}: {:?}",
                    "⚠".yellow().bold(),
                    collector.collector.bold(),
                    collector.state
                );
            }
            crate::model::CollectorState::Unavailable => {
                println!(
                    "  {} collector {}: {:?}",
                    "✖".red().bold(),
                    collector.collector.bold(),
                    collector.state
                );
            }
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
    print!("{}  ", time.to_string().dimmed());
    print_access(&event.access, Some(event.action));
    Ok(())
}

pub fn print_devices(devices: &[Device], json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string(devices)?);
    } else if devices.is_empty() {
        println!("{}", "No microphone or camera device found.".yellow());
    } else {
        for device in devices {
            let res = match device.resource {
                Resource::Microphone => format!("{:<4}", "MIC").bold().magenta(),
                Resource::Camera => format!("{:<4}", "CAM").bold().cyan(),
            };
            println!(
                "{}  {:<32}  {}",
                res,
                device.name.bold(),
                device.id.dimmed()
            );
        }
    }
    Ok(())
}

pub fn print_doctor(checks: &[DiagnosticCheck], json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string(checks)?);
    } else {
        for check in checks {
            let badge = match check.status {
                DiagnosticStatus::Ok => "[  OK]".bold().green(),
                DiagnosticStatus::Warning => "[WARN]".bold().yellow(),
                DiagnosticStatus::Error => "[ ERR]".bold().red(),
            };
            println!("{}  {:<22}  {}", badge, check.name.bold(), check.detail);
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

    let resource_col = match access.resource {
        Resource::Microphone => format!("{:<4}", "MIC").bold().magenta(),
        Resource::Camera => format!("{:<4}", "CAM").bold().cyan(),
    };

    let state_str = match action {
        Some(Action::Start) => "START",
        Some(Action::Update) => "UPDATED",
        Some(Action::Stop) => "STOPPED",
        None => match access.activity {
            Activity::Active => "ACTIVE",
            Activity::Ready => "READY",
        },
    };
    let state_col = match state_str {
        "ACTIVE" | "START" => format!("{:<9}", state_str).bold().green(),
        "READY" => format!("{:<9}", state_str).bold().yellow(),
        "UPDATED" => format!("{:<9}", state_str).bold().cyan(),
        "STOPPED" => format!("{:<9}", state_str).dimmed(),
        _ => format!("{:<9}", state_str).normal(),
    };

    let risk_str = access.risk.to_string();
    let risk_col = match access.risk {
        Risk::Expected => format!("{:<12}", risk_str).bold().green(),
        Risk::Unexplained => format!("{:<12}", risk_str).bold().yellow(),
        Risk::Suspicious => format!("{:<12}", risk_str).bold().truecolor(255, 140, 0),
        Risk::Blocked => format!("{:<12}", risk_str).bold().red(),
    };

    let app_col = format!("{:<26}", access.application).bold().white();
    let pid_col = format!("{:<10}", pid).cyan();
    let device_col = if access.device.is_some() {
        format!("{:<20}", device).normal()
    } else {
        format!("{:<20}", device).dimmed()
    };

    let conf_badge = match access.confidence {
        Confidence::High => "[confidence: high]".green(),
        Confidence::Medium => "[confidence: medium]".yellow(),
        Confidence::Low => "[confidence: low]".dimmed(),
    };

    println!(
        "{resource_col}  {state_col}  {risk_col}  {app_col}  {pid_col}  {device_col}  {conf_badge}"
    );

    if let (Some(parent_pid), Some(parent_name)) = (access.parent_pid, &access.parent_name) {
        println!(
            "     {} {} {}",
            "parent:".dimmed(),
            parent_name.bold(),
            format!("(PID {parent_pid})").dimmed()
        );
    }
    if let Some(process) = &access.process {
        if let Some(session_id) = process.session_id {
            println!("     {} {session_id}", "session:".dimmed());
        }
        if !process.ancestry.is_empty() {
            let chain = process
                .ancestry
                .iter()
                .map(|ancestor| format!("{} ({})", ancestor.name.bold(), ancestor.pid))
                .collect::<Vec<_>>()
                .join(&" <- ".dimmed().to_string());
            println!("     {} {chain}", "ancestry:".dimmed());
        }
    }

    if let Some(sig) = &access.signature {
        if sig.verified {
            let signer_display = sig
                .signer
                .as_deref()
                .unwrap_or("trusted certificate; signer unavailable");
            println!(
                "     {} {} {}",
                "signer:".dimmed(),
                signer_display,
                "[verified]".bold().green()
            );
        } else if let Some(err) = &sig.error {
            println!(
                "     {} {} {}",
                "signature:".dimmed(),
                err.red(),
                "[unverified]".bold().red()
            );
        }
    }

    if !access.modules.is_empty() {
        println!(
            "     {} {}",
            "modules:".dimmed(),
            access.modules.join(", ").dimmed()
        );
    }

    for evidence in &access.evidence {
        let kind_str = format!("{:?}", evidence.kind);
        let kind_col = match evidence.kind {
            crate::model::EvidenceKind::Permission => kind_str.blue(),
            crate::model::EvidenceKind::Signature => kind_str.magenta(),
            crate::model::EvidenceKind::ProcessLineage => kind_str.cyan(),
            crate::model::EvidenceKind::ApplicationProfile => kind_str.green(),
            _ => kind_str.yellow(),
        };
        println!(
            "     {} {kind_col} {} {} {} {}",
            "evidence:".dimmed(),
            "via".dimmed(),
            evidence.source.dimmed(),
            "—".dimmed(),
            evidence.detail
        );
    }

    if access.risk >= Risk::Suspicious {
        println!(
            "     {}",
            "⚠ recommended: inspect the process and executable before taking action."
                .bold()
                .truecolor(255, 140, 0)
        );
    }
}
