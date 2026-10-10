use crate::{
    i18n::Language,
    model::{Access, AccessEvent, Action, Device, DiagnosticCheck, Risk, SCHEMA_VERSION, Snapshot},
};
use anyhow::Result;
use chrono::Local;
use colored::Colorize;

use serde::ser::SerializeSeq;

struct VisibleAccesses<'a> {
    items: &'a [Access],
    min_risk: Option<Risk>,
}

impl serde::Serialize for VisibleAccesses<'_> {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(None)?;
        for access in self
            .items
            .iter()
            .filter(|access| should_display(access, self.min_risk))
        {
            sequence.serialize_element(access)?;
        }
        sequence.end()
    }
}
#[cfg(windows)]
#[derive(serde::Serialize)]
pub struct ProtectionMetadata<'a> {
    pub microphone: Option<&'a crate::platform::MicrophoneProtectionStatus>,
    pub microphone_error: Option<&'a str>,
    pub camera: Option<&'a crate::privacy::CameraControlObservation>,
    pub camera_error: Option<&'a str>,
}

#[derive(serde::Serialize)]
struct StatusView<'a> {
    schema_version: u8,
    tool_version: &'static str,
    collectors: &'a [crate::model::CollectorHealth],
    accesses: VisibleAccesses<'a>,
    #[cfg(windows)]
    #[serde(skip_serializing_if = "Option::is_none")]
    protection: Option<&'a ProtectionMetadata<'a>>,
}

#[cfg(windows)]
pub fn microphone_protection_summary(
    lang: Language,
    status: &crate::platform::MicrophoneProtectionStatus,
) -> String {
    format!(
        "{}; {}; SDK: {}; endpoints:{} hardware:{} corrections:{}{}{}",
        lang.microphone_protection(status.requested),
        lang.service_state(status.service_active),
        lang.microphone_status(status.mute_state),
        status.endpoint_count,
        status.hardware_mute_count,
        status.corrections,
        if status.detail.is_some() { "; " } else { "" },
        status.detail.as_deref().unwrap_or(""),
    )
}

#[cfg(windows)]
pub fn camera_protection_summary(
    lang: Language,
    status: &crate::privacy::CameraControlObservation,
) -> String {
    format!(
        "{}; actual:{} present:{} blocked:{} pending:{} absent:{} unknown:{}; {}{}{}",
        lang.camera_intent(status.desired_blocked),
        match status.state {
            crate::privacy::CameraPrivacyState::Allowed => "allowed",
            crate::privacy::CameraPrivacyState::Blocked => "blocked",
            crate::privacy::CameraPrivacyState::SystemManaged => "unknown/mixed",
        },
        status.present_total,
        status.owned_blocked_present,
        status.pending_restore,
        status.absent_owned,
        status.unknown_devices,
        lang.service_state(status.helper_active),
        if status.detail.is_some() { "; " } else { "" },
        status.detail.as_deref().unwrap_or(""),
    )
}

#[cfg(windows)]
pub fn print_protected_status(
    snapshot: &Snapshot,
    json: bool,
    min_risk: Option<Risk>,
    lang: Language,
    protection: &ProtectionMetadata<'_>,
) -> Result<()> {
    if json {
        let document = status_view(snapshot, min_risk, Some(protection));
        println!("{}", serde_json::to_string(&document)?);
    } else {
        if let Some(status) = protection.microphone {
            println!("{}", microphone_protection_summary(lang, status));
        }
        if let Some(error) = protection.microphone_error {
            println!("Microphone protection unknown: {error}");
        }
        if let Some(status) = protection.camera {
            println!("{}", camera_protection_summary(lang, status));
        }
        if let Some(error) = protection.camera_error {
            println!("Camera protection unknown: {error}");
        }
        println!("{}", lang.protection_limit());
        print_status(snapshot, false, min_risk, lang)?;
    }
    Ok(())
}

fn status_view<'a>(
    snapshot: &'a Snapshot,
    min_risk: Option<Risk>,
    #[cfg(windows)] protection: Option<&'a ProtectionMetadata<'a>>,
) -> StatusView<'a> {
    StatusView {
        schema_version: SCHEMA_VERSION,
        tool_version: env!("CARGO_PKG_VERSION"),
        collectors: &snapshot.collectors,
        accesses: VisibleAccesses {
            items: &snapshot.accesses,
            min_risk,
        },
        #[cfg(windows)]
        protection,
    }
}

pub fn print_status(
    snapshot: &Snapshot,
    json: bool,
    min_risk: Option<Risk>,
    lang: Language,
) -> Result<()> {
    if json {
        println!(
            "{}",
            serde_json::to_string(&status_view(
                snapshot,
                min_risk,
                #[cfg(windows)]
                None,
            ))?
        );
        return Ok(());
    }
    let mut visible = snapshot
        .accesses
        .iter()
        .filter(|access| should_display(access, min_risk))
        .peekable();

    if visible.peek().is_none() {
        if snapshot
            .collectors
            .iter()
            .any(|health| health.state != crate::model::CollectorState::Healthy)
        {
            println!("{}", lang.observation_incomplete().yellow());
        } else if snapshot.accesses.is_empty() {
            println!("{}", lang.no_activity().green().bold());
        } else {
            println!("{}", lang.no_matching_activity().yellow());
        }
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
    for access in visible {
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
    if !should_display(&event.access, min_risk) {
        return Ok(());
    }
    if json {
        println!("{}", serde_json::to_string(event)?);
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
    if matches!(action, Some(Action::Stop)) {
        println!("     {}", lang.historical_observation_note().dimmed());
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Activity, Confidence, EnforcementDecision, Resource};

    fn access(risk: Risk) -> Access {
        Access {
            key: "test".into(),
            resource: Resource::Microphone,
            activity: Activity::Active,
            risk,
            confidence: Confidence::High,
            enforcement: EnforcementDecision::Alert,
            application: "test.exe".into(),
            pid: Some(42),
            parent_pid: None,
            parent_name: None,
            executable: None,
            signature: None,
            device: None,
            started_at: None,
            modules: vec![],
            evidence: vec![],
            process: None,
        }
    }

    #[test]
    fn risk_filter_applies_identically_before_output_formatting() {
        assert!(!should_display(
            &access(Risk::Expected),
            Some(Risk::Suspicious)
        ));
        assert!(should_display(
            &access(Risk::Suspicious),
            Some(Risk::Suspicious)
        ));
    }

    #[cfg(windows)]
    #[test]
    fn typed_status_keeps_protection_intent_independent_of_sdk_and_capture() {
        let microphone = crate::platform::MicrophoneProtectionStatus {
            requested: true,
            service_active: false,
            mute_state: crate::model::MicrophoneMuteState::Muted,
            endpoint_count: 1,
            hardware_mute_count: 0,
            corrections: 0,
            detail: Some("guard unavailable; software mute only".into()),
        };
        let camera = crate::privacy::CameraControlObservation {
            desired_blocked: true,
            state: crate::privacy::CameraPrivacyState::SystemManaged,
            present_total: 0,
            owned_blocked_present: 0,
            pending_restore: 0,
            absent_owned: 0,
            unknown_devices: 0,
            helper_active: true,
            detail: None,
        };
        let metadata = ProtectionMetadata {
            microphone: Some(&microphone),
            microphone_error: None,
            camera: Some(&camera),
            camera_error: None,
        };
        let snapshot = Snapshot {
            collectors: vec![],
            accesses: vec![access(Risk::Expected), access(Risk::Suspicious)],
            observation_gaps: vec![],
        };
        let document = status_view(&snapshot, Some(Risk::Suspicious), Some(&metadata));
        assert!(std::ptr::eq(
            document.accesses.items,
            snapshot.accesses.as_slice()
        ));
        let value = serde_json::to_value(document).unwrap();
        assert_eq!(value["schema_version"], 3);
        assert_eq!(value["accesses"].as_array().unwrap().len(), 1);
        assert_eq!(value["accesses"][0]["activity"], "active");
        assert_eq!(value["protection"]["microphone"]["requested"], true);
        assert_eq!(value["protection"]["microphone"]["service_active"], false);
        assert_eq!(value["protection"]["microphone"]["mute_state"], "muted");
        assert_eq!(value["protection"]["camera"]["desired_blocked"], true);
        assert_eq!(value["protection"]["camera"]["present_total"], 0);
        assert_eq!(value["protection"]["camera"]["owned_blocked_present"], 0);
        let errors = ProtectionMetadata {
            microphone: None,
            microphone_error: Some("intent unreadable"),
            camera: None,
            camera_error: Some("inventory unreadable"),
        };
        let value = serde_json::to_value(status_view(&snapshot, None, Some(&errors))).unwrap();
        assert!(value["protection"]["microphone"].is_null());
        assert_eq!(value["protection"]["microphone_error"], "intent unreadable");
        assert!(value["protection"]["camera"].is_null());
    }
}
