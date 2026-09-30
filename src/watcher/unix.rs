use crate::{
    frontends::cli::Filter,
    history,
    i18n::Language,
    model::{
        Access, AccessEvent, Action, CollectorState, Resource, SCHEMA_VERSION, Snapshot, event_code,
    },
    output,
    platform::PlatformMonitor,
};
use anyhow::{Context, Result};
use chrono::Utc;
use std::{
    collections::{HashMap, HashSet},
    fs::OpenOptions,
    io::{BufWriter, Write},
    path::Path,
    sync::mpsc,
    time::Duration,
};

pub fn watch(
    monitor: &PlatformMonitor,
    filter: &Filter,
    json: bool,
    interval: Duration,
    log_path: Option<&Path>,
    lang: Language,
    history_enabled: bool,
) -> Result<()> {
    let (stop_tx, stop_rx) = mpsc::channel();
    ctrlc::set_handler(move || {
        let _ = stop_tx.send(());
    })
    .context("cannot install Ctrl+C handler")?;
    let mut log = log_path
        .map(|path| {
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .map(BufWriter::new)
        })
        .transpose()
        .context("cannot open watch JSONL event log")?;
    let mut previous = HashMap::new();
    let mut previous_health = Vec::new();
    let mut unseen = HashSet::new();
    loop {
        let snapshot = monitor.snapshot(filter.into())?;
        let changed = previous_health != snapshot.collectors;
        if changed
            && snapshot
                .collectors
                .iter()
                .any(|health| health.state != CollectorState::Healthy)
        {
            output::print_status(&snapshot, json, filter.risk, lang)?;
        } else if changed && !previous_health.is_empty() && json {
            // Report recovery explicitly, without guessing which capture stopped during
            // the outage. JSON watch lines with `collectors` are status documents.
            output::print_status(&snapshot, true, filter.risk, lang)?;
        }
        reconcile(&mut previous, &mut unseen, &snapshot, |access, action| {
            let event = AccessEvent {
                schema_version: SCHEMA_VERSION,
                event_code: event_code(action),
                tool_version: env!("CARGO_PKG_VERSION"),
                action,
                observed_at: Utc::now(),
                access: access.clone(),
            };
            if history_enabled {
                history::append(&event)?;
            }
            if let Some(writer) = &mut log {
                serde_json::to_writer(&mut *writer, &event)?;
                writer.write_all(b"\n")?;
                writer.flush()?;
            }
            output::print_event(&event, json, filter.risk, lang)?;
            Ok(())
        })?;
        previous_health = snapshot.collectors;
        if stop_rx.recv_timeout(interval).is_ok() {
            return Ok(());
        }
    }
}

fn reconcile(
    previous: &mut HashMap<String, Access>,
    unseen: &mut HashSet<Resource>,
    snapshot: &Snapshot,
    mut emit: impl FnMut(&Access, Action) -> Result<()>,
) -> Result<()> {
    for collector in &snapshot.collectors {
        let resource = match collector.collector {
            "pipewire_audio" | "coreaudio_input" => Resource::Microphone,
            "pipewire_video" | "avfoundation_video" => Resource::Camera,
            _ => continue,
        };
        // Limited coverage is not a scan gap unless the collector explicitly
        // reports one; permanent AVFoundation limitations still permit STOP.
        if collector.state == CollectorState::Unavailable
            || snapshot.observation_gaps.contains(&resource)
        {
            unseen.insert(resource);
        } else if unseen.remove(&resource) {
            // Recovery is not evidence of when an earlier capture stopped.
            // Rebaseline without inventing a STOP across an observation gap.
            previous.retain(|_, access| access.resource != resource);
        }
    }
    let mut current = HashMap::new();
    for access in &snapshot.accesses {
        if !current.contains_key(&access.key) {
            current.insert(access.key.clone(), access.clone());
        }
    }
    for (key, access) in &current {
        match previous.get(key) {
            None => emit(access, Action::Start)?,
            Some(old) if access_changed(old, access) => emit(access, Action::Update)?,
            _ => {}
        }
    }
    for (key, old) in previous.iter() {
        if !current.contains_key(key) && !unseen.contains(&old.resource) {
            emit(old, Action::Stop)?;
        }
    }
    previous.retain(|_, access| unseen.contains(&access.resource));
    previous.extend(current);
    Ok(())
}

fn access_changed(old: &Access, current: &Access) -> bool {
    old.activity != current.activity
        || old.risk != current.risk
        || old.confidence != current.confidence
        || old.enforcement != current.enforcement
        || old.evidence != current.evidence
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Activity, CollectorHealth, Confidence, EnforcementDecision, Risk};

    fn access() -> Access {
        Access {
            key: "capture-instance".into(),
            resource: Resource::Microphone,
            activity: Activity::Active,
            risk: Risk::Unexplained,
            confidence: Confidence::High,
            enforcement: EnforcementDecision::Alert,
            application: "recorder".into(),
            pid: Some(10),
            parent_pid: None,
            parent_name: None,
            executable: None,
            signature: None,
            device: None,
            started_at: None,
            modules: Vec::new(),
            evidence: Vec::new(),
            process: None,
        }
    }

    fn snapshot(state: CollectorState, accesses: Vec<Access>) -> Snapshot {
        named_snapshot("pipewire_audio", state, accesses)
    }

    fn named_snapshot(
        collector: &'static str,
        state: CollectorState,
        accesses: Vec<Access>,
    ) -> Snapshot {
        Snapshot {
            collectors: vec![CollectorHealth {
                collector,
                state,
                detail: None,
            }],
            accesses,
            observation_gaps: Vec::new(),
        }
    }

    #[test]
    fn outage_never_fabricates_stop_and_recovery_rebaselines() {
        let mut previous = HashMap::new();
        let mut unseen = HashSet::new();
        let mut actions = Vec::new();
        for snap in [
            snapshot(CollectorState::Healthy, vec![access()]),
            snapshot(CollectorState::Unavailable, vec![]),
            snapshot(CollectorState::Unavailable, vec![]),
            snapshot(CollectorState::Healthy, vec![access()]),
            snapshot(CollectorState::Healthy, vec![]),
        ] {
            reconcile(&mut previous, &mut unseen, &snap, |_, action| {
                actions.push(action);
                Ok(())
            })
            .unwrap();
        }
        assert_eq!(actions.len(), 3);
        assert!(matches!(
            actions.as_slice(),
            [Action::Start, Action::Start, Action::Stop]
        ));
    }

    #[test]
    fn degraded_camera_scan_preserves_stop_and_subsequent_start() {
        let mut previous = HashMap::new();
        let mut unseen = HashSet::new();
        let mut actions = Vec::new();
        let mut camera = access();
        camera.resource = Resource::Camera;
        camera.key = "camera-1".into();
        for snap in [
            named_snapshot(
                "avfoundation_video",
                CollectorState::Degraded,
                vec![camera.clone()],
            ),
            named_snapshot("avfoundation_video", CollectorState::Degraded, vec![]),
            named_snapshot(
                "avfoundation_video",
                CollectorState::Degraded,
                vec![camera.clone()],
            ),
            named_snapshot("avfoundation_video", CollectorState::Degraded, vec![]),
        ] {
            reconcile(&mut previous, &mut unseen, &snap, |_, action| {
                actions.push(action);
                Ok(())
            })
            .unwrap();
        }
        assert!(matches!(
            actions.as_slice(),
            [Action::Start, Action::Stop, Action::Start, Action::Stop]
        ));
    }

    fn assert_degraded_gap_rebaselines(resource: Resource, collector: &'static str) {
        let mut previous = HashMap::new();
        let mut unseen = HashSet::new();
        let mut actions = Vec::new();
        let mut observed = access();
        observed.resource = resource;
        let active = named_snapshot(collector, CollectorState::Degraded, vec![observed.clone()]);
        reconcile(&mut previous, &mut unseen, &active, |_, action| {
            actions.push(action);
            Ok(())
        })
        .unwrap();

        let mut gap = named_snapshot(collector, CollectorState::Degraded, vec![]);
        gap.observation_gaps.push(resource);
        reconcile(&mut previous, &mut unseen, &gap, |_, action| {
            actions.push(action);
            Ok(())
        })
        .unwrap();
        assert!(previous.contains_key(&observed.key));
        assert!(unseen.contains(&resource));
        assert!(matches!(actions.as_slice(), [Action::Start]));

        // Recovery with no activity must not invent a STOP during the gap.
        let recovered = named_snapshot(collector, CollectorState::Degraded, vec![]);
        reconcile(&mut previous, &mut unseen, &recovered, |_, action| {
            actions.push(action);
            Ok(())
        })
        .unwrap();
        assert!(previous.is_empty());
        assert!(unseen.is_empty());
        assert!(matches!(actions.as_slice(), [Action::Start]));

        reconcile(&mut previous, &mut unseen, &active, |_, action| {
            actions.push(action);
            Ok(())
        })
        .unwrap();
        reconcile(&mut previous, &mut unseen, &recovered, |_, action| {
            actions.push(action);
            Ok(())
        })
        .unwrap();
        assert!(matches!(
            actions.as_slice(),
            [Action::Start, Action::Start, Action::Stop]
        ));
    }

    #[test]
    fn camera_discovery_gap_retains_access_and_rebaselines() {
        assert_degraded_gap_rebaselines(Resource::Camera, "avfoundation_video");
    }

    #[test]
    fn partial_audio_property_gap_retains_access_and_rebaselines() {
        assert_degraded_gap_rebaselines(Resource::Microphone, "coreaudio_input");
    }
}
