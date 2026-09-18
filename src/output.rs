use crate::{
    i18n::Language,
    model::{
        Access, AccessEvent, Action, Device, DiagnosticCheck, Risk, SCHEMA_VERSION, Snapshot,
        StatusDocument,
    },
};
use anyhow::Result;
use chrono::Local;
use colored::Colorize;

pub fn print_status(
    snapshot: &Snapshot,
    json: bool,
    min_risk: Option<Risk>,
    lang: Language,
) -> Result<()> {
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
        println!("{}", lang.no_activity().green().bold());
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
        print_access(access, None, lang);
    }
    Ok(())
}

pub fn print_event(
    event: &AccessEvent,
    json: bool,
    min_risk: Option<Risk>,
    lang: Language,
) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string(event)?);
        return Ok(());
    }
    if !should_display(&event.access, min_risk) {
        return Ok(());
    }

    let time = event.observed_at.with_timezone(&Local).format("%H:%M:%S");
    print!("{}  ", time.to_string().dimmed());
    print_access(&event.access, Some(event.action), lang);
    Ok(())
}

pub fn print_devices(devices: &[Device], json: bool, lang: Language) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string(devices)?);
    } else if devices.is_empty() {
        println!("{}", lang.no_devices_found().yellow());
    } else {
        for device in devices {
            let res = match device.resource {
                crate::model::Resource::Microphone => {
                    format!("{:<4}", lang.resource(device.resource))
                        .bold()
                        .magenta()
                }
                crate::model::Resource::Camera => format!("{:<4}", lang.resource(device.resource))
                    .bold()
                    .cyan(),
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

pub fn print_doctor(checks: &[DiagnosticCheck], json: bool, lang: Language) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string(checks)?);
    } else {
        for check in checks {
            let badge = lang.doctor_status(check.status);
            println!("{}  {:<22}  {}", badge, check.name.bold(), check.detail);
        }
    }
    Ok(())
}

fn should_display(access: &Access, min_risk: Option<Risk>) -> bool {
    min_risk.is_none_or(|min| access.risk >= min)
}

pub fn print_explanation(
    snapshot: &Snapshot,
    json: bool,
    min_risk: Option<Risk>,
    lang: Language,
) -> Result<()> {
    print_status(snapshot, json, min_risk, lang)
}

fn print_access(access: &Access, action: Option<Action>, lang: Language) {
    let pid = access
        .pid
        .map(|pid| format!("PID {pid}"))
        .unwrap_or_else(|| "PID ?".to_owned());
    let device = access
        .device
        .as_deref()
        .unwrap_or_else(|| lang.device_unavailable());

    let resource_col = match access.resource {
        crate::model::Resource::Microphone => format!("{:<4}", lang.resource(access.resource))
            .bold()
            .magenta(),
        crate::model::Resource::Camera => format!("{:<4}", lang.resource(access.resource))
            .bold()
            .cyan(),
    };

    let state_str = lang.state_str(action, access.activity);
    let state_col = match action {
        Some(Action::Start) => format!("{:<9}", state_str).bold().green(),
        Some(Action::Update) => format!("{:<9}", state_str).bold().cyan(),
        Some(Action::Stop) => format!("{:<9}", state_str).dimmed(),
        None => match access.activity {
            crate::model::Activity::Active => format!("{:<9}", state_str).bold().green(),
            crate::model::Activity::Ready => format!("{:<9}", state_str).bold().yellow(),
        },
    };

    let risk_str = lang.risk_str(access.risk);
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

    let conf_badge = lang.confidence_badge(access.confidence);

    println!(
        "{resource_col}  {state_col}  {risk_col}  {app_col}  {pid_col}  {device_col}  {conf_badge}"
    );

    if let (Some(parent_pid), Some(parent_name)) = (access.parent_pid, &access.parent_name) {
        println!(
            "     {} {} {}",
            lang.parent_label().dimmed(),
            parent_name.bold(),
            format!("(PID {parent_pid})").dimmed()
        );
    }
    if let Some(process) = &access.process {
        if let Some(session_id) = process.session_id {
            println!("     {} {session_id}", lang.session_label().dimmed());
        }
        if !process.ancestry.is_empty() {
            let chain = process
                .ancestry
                .iter()
                .map(|ancestor| format!("{} ({})", ancestor.name.bold(), ancestor.pid))
                .collect::<Vec<_>>()
                .join(&" <- ".dimmed().to_string());
            println!("     {} {chain}", lang.ancestry_label().dimmed());
        }
    }

    if let Some(sig) = &access.signature {
        if sig.verified {
            let signer_display = sig
                .signer
                .as_deref()
                .unwrap_or_else(|| lang.signer_unavailable());
            println!(
                "     {} {} {}",
                lang.signer_label().dimmed(),
                signer_display,
                lang.verified_badge().bold().green()
            );
        } else if let Some(err) = &sig.error {
            println!(
                "     {} {} {}",
                lang.signature_label().dimmed(),
                err.red(),
                lang.unverified_badge().bold().red()
            );
        }
    }

    if !access.modules.is_empty() {
        println!(
            "     {} {}",
            lang.modules_label().dimmed(),
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
            lang.evidence_label().dimmed(),
            lang.via_label().dimmed(),
            evidence.source.dimmed(),
            "—".dimmed(),
            evidence.detail
        );
    }

    if access.risk >= Risk::Suspicious {
        println!(
            "     {}",
            lang.recommended_warning().bold().truecolor(255, 140, 0)
        );
    }
}
