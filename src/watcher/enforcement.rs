//! Opt-in CLI-equivalent enforcement owned by the Rust observer, never the UI.
//! Reuses the platform watcher's two-observation and retained-instance rules.
use anyhow::{Result, ensure};
use serde::Serialize;
use std::{collections::{HashMap, HashSet}, sync::{Arc, Mutex}};
use crate::model::Snapshot;

#[derive(Clone, Default, Debug)]
pub struct Runtime(Arc<Mutex<State>>);
#[derive(Default, Debug)]
struct State {
    enabled: bool,
    generation: u64,
    observations: HashMap<(String, String), u8>,
    attempted: HashSet<(String, String)>,
    accepted_requests: u64,
    failed_requests: u64,
    last_error: Option<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lease { enabled: bool, generation: u64 }
#[derive(Serialize)]
pub struct Status {
    pub enabled: bool,
    pub generation: u64,
    pub accepted_requests: u64,
    pub failed_requests: u64,
    pub last_error: Option<String>,
    pub scope: &'static str,
}
impl Runtime {
    pub fn prepare(&self) -> Result<Lease> {
        let state = self.0.lock().map_err(|_| anyhow::anyhow!("Native enforcement state unavailable"))?;
        Ok(Lease { enabled: state.enabled, generation: state.generation })
    }
    pub fn set(&self, expected: Lease, enabled: bool) -> Result<Status> {
        let mut state = self.0.lock().map_err(|_| anyhow::anyhow!("Native enforcement state unavailable"))?;
        ensure!(expected == (Lease { enabled: state.enabled, generation: state.generation }),
            "Enforcement changed concurrently; confirm again");
        if enabled != state.enabled {
            let generation = state.generation.checked_add(1).ok_or_else(|| anyhow::anyhow!("Enforcement generation exhausted"))?;
            state.enabled = enabled;
            state.generation = generation;
            state.observations.clear();
            state.attempted.clear();
        }
        Ok(status(&state))
    }
    pub fn status(&self) -> Result<Status> {
        let state = self.0.lock().map_err(|_| anyhow::anyhow!("Native enforcement state unavailable"))?;
        Ok(status(&state))
    }
    pub fn observe(&self, snapshot: &Snapshot) {
        self.observe_with(snapshot, crate::platform::terminate_process_by_pid);
    }
    pub fn observation_gap(&self) {
        if let Ok(mut state) = self.0.lock() { state.observations.clear(); }
    }
    fn observe_with(&self, snapshot: &Snapshot, mut terminate: impl FnMut(u32, &str) -> Result<()>) {
        // A disable command cannot report success while an older admitted effect
        // is still running. No collector/UI thread is added and no scan is slowed.
        let Ok(mut state) = self.0.lock() else { return };
        if !state.enabled { return; }
        let mut accepted = 0u64;
        let mut failed = 0u64;
        let mut error = None;
        let mut execute = |pid, identity: &str| {
            let result = terminate(pid, identity);
            match &result {
                Ok(()) => accepted += 1,
                Err(issue) => {
                    failed += 1;
                    let text = format!("{issue:#}");
                    error = Some(text.chars().take(512).collect());
                }
            }
            result
        };
        let State { observations, attempted, .. } = &mut *state;
        #[cfg(windows)]
        {
            observations.retain(|(key, _), _| snapshot.accesses.iter().any(|access|
                &access.key == key && !snapshot.observation_gaps.contains(&access.resource)));
            super::windows::update_enforcement_candidates_with(snapshot.accesses.iter()
                .filter(|access| !snapshot.observation_gaps.contains(&access.resource)),
                true, observations, attempted, &mut execute);
            let current = |identity: &(String, String)| snapshot.accesses.iter().any(|a|
                a.key == identity.0 && a.process.as_ref().is_some_and(|p| p.instance_id == identity.1));
            observations.retain(|identity, _| current(identity));
            attempted.retain(|identity| current(identity));
        }
        #[cfg(unix)]
        super::unix::enforce_policy_with(snapshot, observations, attempted, &mut execute);
        state.accepted_requests = state.accepted_requests.saturating_add(accepted);
        state.failed_requests = state.failed_requests.saturating_add(failed);
        if error.is_some() { state.last_error = error; }
    }
}
fn status(state: &State) -> Status {
    Status { enabled: state.enabled, generation: state.generation,
        accepted_requests: state.accepted_requests, failed_requests: state.failed_requests,
        last_error: state.last_error.clone(),
        scope: "This Rust session only, like CLI --kill/--no-kill; disabled after engine restart. Explicitly denied active processes require two observations and native instance revalidation. No device access prohibition and no undo of termination. Closing Flutter does not change this state." }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Access, Activity, Confidence, EnforcementDecision, ProcessContext, Resource, Risk};
    fn snapshot(instance: &str) -> Snapshot {
        Snapshot { collectors: vec![], observation_gaps: vec![], accesses: vec![Access {
            key: "fixture:camera".into(), resource: Resource::Camera, activity: Activity::Active,
            confidence: Confidence::High, enforcement: EnforcementDecision::Deny,
            application: "fixture-recorder".into(), pid: Some(12345), parent_pid: None, parent_name: None,
            executable: Some("fixture-recorder".into()), device: None, started_at: None,
            signature: None, modules: vec![], evidence: vec![], risk: Risk::Suspicious,
            process: Some(ProcessContext { instance_id: instance.into(), ancestry: vec![], user: None, session_id: None, integrity: None }),
        }] }
    }
    #[test]
    fn opt_in_two_fresh_observations_instance_changes_and_close_are_not_ui_authority() {
        let runtime = Runtime::default();
        let ui = runtime.clone();
        let source = snapshot("12345:one");
        runtime.observe_with(&source, |_,_| panic!("disabled enforcement executed"));
        let stale = ui.prepare().unwrap();
        ui.set(stale, true).unwrap();
        assert!(ui.set(stale, false).is_err());
        drop(ui); // Native owner continues; frontend lifetime is not a guard lease.
        let mut calls = vec![];
        runtime.observe_with(&source, |pid,id| { calls.push((pid,id.to_owned())); Ok(()) });
        assert!(calls.is_empty());
        runtime.observe_with(&snapshot("12345:replacement"), |_,_| panic!("PID reuse inherited an observation"));
        runtime.observe_with(&snapshot("12345:replacement"), |pid,id| { calls.push((pid,id.to_owned())); Ok(()) });
        assert_eq!(calls, vec![(12345, "12345:replacement".into())]);
        runtime.set(runtime.prepare().unwrap(), false).unwrap();
        runtime.observe_with(&source, |_,_| panic!("disabled enforcement executed"));
        assert_eq!(runtime.status().unwrap().accepted_requests, 1);
        assert!(!Runtime::default().status().unwrap().enabled);
    }
    #[test]
    fn ready_alert_missing_identity_and_native_refusal_do_not_become_success() {
        let runtime = Runtime::default();
        runtime.set(runtime.prepare().unwrap(), true).unwrap();
        let mut source = snapshot("12345:one");
        source.accesses[0].activity = Activity::Ready;
        for _ in 0..3 { runtime.observe_with(&source, |_,_| panic!("Ready was enforced")); }
        source.accesses[0].activity = Activity::Active;
        source.accesses[0].enforcement = EnforcementDecision::Alert;
        for _ in 0..3 { runtime.observe_with(&source, |_,_| panic!("Alert was enforced")); }
        source.accesses[0].enforcement = EnforcementDecision::Deny;
        source.accesses[0].process = None;
        for _ in 0..3 { runtime.observe_with(&source, |_,_| panic!("Missing identity was enforced")); }
        let source = snapshot("12345:denied");
        runtime.observe_with(&source, |_,_| panic!("First observation was enforced"));
        runtime.observe_with(&source, |_,_| anyhow::bail!("fixture permission denied"));
        let status = runtime.status().unwrap();
        assert_eq!(status.accepted_requests, 0);
        assert_eq!(status.failed_requests, 1);
        assert!(status.last_error.unwrap().contains("permission denied"));
    }
    #[test]
    fn observation_gaps_reset_the_two_observation_requirement() {
        let runtime = Runtime::default();
        runtime.set(runtime.prepare().unwrap(), true).unwrap();
        let source = snapshot("12345:one");
        runtime.observe_with(&source, |_,_| panic!("First observation enforced"));
        let mut gap = snapshot("12345:one");
        gap.observation_gaps.push(Resource::Camera);
        runtime.observe_with(&gap, |_,_| panic!("Incomplete resource enforced"));
        runtime.observe_with(&source, |_,_| panic!("Pre-gap observation reused"));
        runtime.observation_gap();
        runtime.observe_with(&source, |_,_| panic!("Failed scan reused"));
        let mut effects = 0;
        runtime.observe_with(&source, |_,_| { effects += 1; Ok(()) });
        assert_eq!(effects, 1);
    }
}
