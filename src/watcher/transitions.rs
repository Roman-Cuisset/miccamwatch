use crate::model::{Access, Action, CollectorState, Resource, Snapshot};
use anyhow::Result;
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub struct TransitionTracker {
    previous: HashMap<String, Access>,
    unseen: HashSet<Resource>,
}

impl TransitionTracker {
    pub fn reconcile(
        &mut self,
        snapshot: &Snapshot,
        emit: impl FnMut(&Access, Action) -> Result<()>,
    ) -> Result<()> {
        reconcile(&mut self.previous, &mut self.unseen, snapshot, emit)
    }

    pub(crate) fn observation_gap(&mut self) {
        self.unseen.extend([Resource::Microphone, Resource::Camera]);
    }

    #[cfg(feature = "flutter-desktop")]
    pub(crate) fn reconcile_complete(
        &mut self,
        snapshot: &Snapshot,
        mut emit: impl FnMut(&Access, Action) -> Result<()>,
    ) -> Result<()> {
        let mut incomplete = HashSet::new();
        let mut recovered = HashSet::new();
        for resource in [Resource::Microphone, Resource::Camera] {
            let mut collectors = snapshot
                .collectors
                .iter()
                .filter(|collector| collector_resource(collector.collector) == Some(resource))
                .peekable();
            let missing = collectors.peek().is_none()
                || collectors.any(|collector| collector.state == CollectorState::Unavailable)
                || snapshot.observation_gaps.contains(&resource);
            if missing {
                self.unseen.insert(resource);
                incomplete.insert(resource);
            } else if self.unseen.remove(&resource) {
                recovered.insert(resource);
            }
        }
        let current = current_accesses(snapshot, |access| !incomplete.contains(&access.resource));
        self.previous
            .retain(|_, access| !recovered.contains(&access.resource));
        emit_changes(&self.previous, &current, &self.unseen, |access, action| {
            if recovered.contains(&access.resource) {
                return Ok(());
            }
            emit(access, action)
        })?;
        self.previous = current;
        Ok(())
    }
}

pub(super) fn reconcile(
    previous: &mut HashMap<String, Access>,
    unseen: &mut HashSet<Resource>,
    snapshot: &Snapshot,
    mut emit: impl FnMut(&Access, Action) -> Result<()>,
) -> Result<()> {
    for collector in &snapshot.collectors {
        let resource = match collector.collector {
            "pipewire_audio" | "coreaudio_input" | "wasapi" => Resource::Microphone,
            "pipewire_video" | "coremediaio_video" | "privacy_store" => Resource::Camera,
            _ => continue,
        };
        // Limited coverage is not a scan gap unless the collector explicitly
        // reports one; CoreMediaIO's unknown-client limitation still permits STOP.
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
    let current = current_accesses(snapshot, |_| true);
    emit_changes(previous, &current, unseen, &mut emit)?;
    previous.retain(|_, access| unseen.contains(&access.resource));
    previous.extend(current);
    Ok(())
}

fn current_accesses(
    snapshot: &Snapshot,
    include: impl Fn(&Access) -> bool,
) -> HashMap<String, Access> {
    let mut current = HashMap::new();
    for access in snapshot.accesses.iter().filter(|access| include(access)) {
        current
            .entry(access.key.clone())
            .or_insert_with(|| access.clone());
    }
    current
}

fn emit_changes(
    previous: &HashMap<String, Access>,
    current: &HashMap<String, Access>,
    unseen: &HashSet<Resource>,
    mut emit: impl FnMut(&Access, Action) -> Result<()>,
) -> Result<()> {
    for (key, access) in current {
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
    Ok(())
}

#[cfg(feature = "flutter-desktop")]
fn collector_resource(collector: &str) -> Option<Resource> {
    match collector {
        "pipewire_audio" | "coreaudio_input" | "wasapi" => Some(Resource::Microphone),
        "pipewire_video" | "coremediaio_video" | "privacy_store" | "sensor_activity" => {
            Some(Resource::Camera)
        }
        _ => None,
    }
}

fn access_changed(old: &Access, current: &Access) -> bool {
    old.activity != current.activity
        || old.risk != current.risk
        || old.confidence != current.confidence
        || old.enforcement != current.enforcement
        || old.evidence != current.evidence
}
