//! On-demand consultation and one explicitly confirmed native startup command.
//! Never runs on collector/pipe threads; no device, permission or install mutation.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
use super::commands::{CommandEnvelope, CommandState, authorize_envelope,
    deserialize_request_id, deserialize_sequence};
use crate::history::{HistoryBrowser, HistoryPage};

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    History { cursor: Option<String>, limit: u16 },
    Diagnostics,
    AutostartStatus,
    SetAutostart { requested: bool, expected_enabled: bool, confirmation: String },
}
impl Operation {
    fn name(&self) -> &'static str {
        match self {
            Self::History { .. } => "history", Self::Diagnostics => "diagnostics",
            Self::AutostartStatus => "autostart_status", Self::SetAutostart { .. } => "set_autostart",
        }
    }
    fn validate(&self) -> Result<()> {
        if let Self::History { cursor, limit } = self {
            ensure!((1..=50).contains(limit), "History page limit must be1..50");
            ensure!(cursor.as_ref().is_none_or(|value| !value.is_empty() && value.len() <= 256),
                "Invalid history cursor length");
        }
        if let Self::SetAutostart { confirmation, .. } = self {
            ensure!(confirmation == "change_autostart_for_this_installation",
                "Explicit installation startup confirmation is required");
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(try_from = "RequestWire")]
pub struct Request {
    pub session: String,
    pub id: u64,
    pub observed_sequence: u64,
    pub operation: Operation,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestWire {
    session: String,
    #[serde(deserialize_with = "deserialize_request_id")]
    id: u64,
    #[serde(deserialize_with = "deserialize_sequence")]
    observed_sequence: u64,
    operation: Operation,
}
impl TryFrom<RequestWire> for Request {
    type Error = anyhow::Error;
    fn try_from(value: RequestWire) -> Result<Self> {
        value.operation.validate()?;
        ensure!(value.session.len() == 32 && value.session.bytes().all(|byte|
            byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)), "Invalid consultation session");
        Ok(Self { session: value.session, id: value.id,
            observed_sequence: value.observed_sequence, operation: value.operation })
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct Check {
    pub name: String,
    pub status: &'static str,
    pub detail: String,
    pub remedy: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Diagnostics {
    pub checks: Vec<Check>,
    pub limitations: Vec<String>,
    pub engine_version: &'static str,
    pub platform: &'static str,
    pub observed_at: chrono::DateTime<chrono::Utc>,
    pub incomplete: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct Autostart {
    pub enabled: Option<bool>,
    pub writable: bool,
    pub reason: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Response {
    pub id: u64,
    pub state: CommandState,
    pub code: Option<String>,
    pub detail: Option<String>,
    pub operation: &'static str,
    pub history: Option<HistoryPage>,
    pub diagnostics: Option<Diagnostics>,
    pub autostart: Option<Autostart>,
}
#[derive(Clone, Debug, Serialize)]
pub struct View {
    pub available: bool,
    pub reason: Option<String>,
    pub result: Option<Response>,
}
impl Default for View {
    fn default() -> Self {
        Self { available: false, reason: Some("Owned consultation connection unavailable".into()), result: None }
    }
}
#[derive(Default)]
pub struct Session {
    last_id: u64,
    pending: Option<u64>,
    result: Option<Response>,
}
impl Session {
    pub fn view(&self, available: bool) -> View {
        View { available, reason: (!available).then(|| "Native consultation worker unavailable".into()),
            result: self.result.clone() }
    }
    pub fn admit(&mut self, request: Request, session: &str, written: u64,
        ready: bool, worker_pending: bool, autostart_writable: bool) -> Option<Request> {
        if request.operation.validate().is_err() { return None; }
        if let Err(rejection) = authorize_envelope(CommandEnvelope {
            session: &request.session, id: request.id, observed_sequence: request.observed_sequence,
        }, session, &mut self.last_id, written, ready, self.pending.is_some() || worker_pending) {
            if rejection.correlatable {
                self.result = Some(Response { id: request.id, state: CommandState::Rejected,
                    code: Some(rejection.code.into()), detail: Some(rejection.detail.into()),
                    operation: request.operation.name(), history: None, diagnostics: None, autostart: None });
            }
            return None;
        }
        if matches!(request.operation, Operation::SetAutostart { .. }) && !autostart_writable {
            self.result = Some(Response { id: request.id, state: CommandState::Rejected,
                code: Some("read_only".into()), detail: Some("Development startup changes are disabled".into()),
                operation: request.operation.name(), history: None, diagnostics: None, autostart: None });
            return None;
        }
        self.pending = Some(request.id);
        self.result = Some(Response { id: request.id, state: CommandState::Accepted,
            code: None, detail: None, operation: request.operation.name(), history: None, diagnostics: None, autostart: None });
        Some(request)
    }
    pub fn finish(&mut self, id: u64, result: std::result::Result<Payload, String>) -> bool {
        if self.pending != Some(id) { return false; }
        self.pending = None;
        let Some(response) = self.result.as_mut().filter(|row| row.id == id) else {
            // A rejected overlapping request may have replaced the displayed result.
            // Preserve its correlation rather than attach older data to its ID.
            return false;
        };
        match result {
            Ok(Payload::History(page)) => {
                response.state = CommandState::Succeeded;
                response.history = Some(page);
            }
            Ok(Payload::Diagnostics(report)) => {
                response.state = CommandState::Succeeded;
                response.diagnostics = Some(report);
            }
            Ok(Payload::Autostart(status)) => {
                response.state = CommandState::Succeeded;
                response.autostart = Some(status);
            }
            Err(error) => {
                response.state = CommandState::Failed;
                response.code = Some("consultation_failed".into());
                response.detail = Some(bounded(error, 1024));
            }
        }
        true
    }
}
fn bounded(mut value: String, maximum: usize) -> String {
    let mut end = value.len().min(maximum);
    while !value.is_char_boundary(end) { end -= 1; }
    value.truncate(end);
    value
}

pub enum Payload { History(HistoryPage), Diagnostics(Diagnostics), Autostart(Autostart) }
pub struct Completion {
    pub ticket: u64,
    pub result: std::result::Result<Payload, String>,
}
struct Query { ticket: u64, session: String, operation: Operation, autostart_writable: bool }
enum Work { Query(Query), Reset }
/// Bounded lane remains process-owned when a UI closes. Read-only browser handles
/// are explicitly released on close, and at most one native operation can run.
pub struct Worker {
    requests: SyncSender<Work>,
    responses: Receiver<Completion>,
    pending: Option<u64>,
    reset_when_idle: bool,
    disconnected: bool,
}
impl Worker {
    fn start() -> Result<Self> {
        let (requests, work) = mpsc::sync_channel::<Work>(2);
        let (completed, responses) = mpsc::sync_channel::<Completion>(1);
        std::thread::Builder::new().name("miccamwatch-consultation".into()).spawn(move || {
            let mut browser = HistoryBrowser::default();
            let mut session = String::new();
            while let Ok(work) = work.recv() {
                let Work::Query(query) = work else {
                    browser = HistoryBrowser::default();
                    session.clear();
                    continue;
                };
                if session != query.session {
                    browser = HistoryBrowser::default();
                    session = query.session;
                }
                let result = match query.operation {
                    Operation::History { cursor, limit } => browser.page(cursor.as_deref(), limit).map(Payload::History),
                    Operation::Diagnostics => diagnostics().map(Payload::Diagnostics),
                    operation @ (Operation::AutostartStatus | Operation::SetAutostart { .. }) =>
                        autostart(operation, query.autostart_writable).map(Payload::Autostart),
                }.map_err(|error| bounded(format!("{error:#}"), 1024));
                if completed.send(Completion { ticket: query.ticket, result }).is_err() { break; }
            }
        }).context("Cannot start native consultation worker")?;
        Ok(Self { requests, responses, pending: None, reset_when_idle: false, disconnected: false })
    }
    pub fn available(owner: &Option<Self>) -> bool {
        owner.as_ref().is_none_or(|worker| !worker.disconnected)
    }
    pub fn pending(owner: &Option<Self>) -> bool {
        owner.as_ref().is_some_and(|worker| worker.pending.is_some())
    }
    pub fn enqueue(owner: &mut Option<Self>, ticket: u64, request: Request, autostart_writable: bool) -> Result<()> {
        if owner.is_none() { *owner = Some(Self::start()?); }
        let worker = owner.as_mut().expect("created consultation worker");
        ensure!(!worker.disconnected && worker.pending.is_none(), "Native consultation is busy or unavailable");
        worker.flush_reset()?;
        worker.requests.try_send(Work::Query(Query { ticket, session: request.session,
            operation: request.operation, autostart_writable }))
            .context("Native consultation queue unavailable")?;
        worker.pending = Some(ticket);
        Ok(())
    }
    pub fn reset(owner: &mut Option<Self>) {
        if let Some(worker) = owner {
            worker.reset_when_idle = true;
            if worker.pending.is_none() && worker.flush_reset().is_err() { worker.disconnected = true; }
        }
    }
    fn flush_reset(&mut self) -> Result<()> {
        if self.reset_when_idle {
            self.requests.try_send(Work::Reset).context("Cannot release prior history browser")?;
            self.reset_when_idle = false;
        }
        Ok(())
    }
    pub fn try_complete(owner: &mut Option<Self>) -> Option<Completion> {
        let worker = owner.as_mut()?;
        let expected = worker.pending?;
        match worker.responses.try_recv() {
            Ok(completion) if completion.ticket == expected => {
                worker.pending = None;
                // No new query is queued until the reset has been acknowledged by
                // the serial receiver. If full, leave it for the next request.
                if worker.flush_reset().is_err() { worker.disconnected = true; }
                Some(completion)
            }
            Err(TryRecvError::Empty) => None,
            Ok(_) | Err(TryRecvError::Disconnected) => {
                worker.pending = None;
                worker.disconnected = true;
                Some(Completion { ticket: expected, result: Err("Native consultation completion unavailable; restart the tray".into()) })
            }
        }
    }
}
fn autostart(operation: Operation, writable: bool) -> Result<Autostart> {
    let enabled = match operation {
        Operation::AutostartStatus => crate::autostart::state()? == crate::autostart::AutostartState::Enabled,
        Operation::SetAutostart { requested, expected_enabled, confirmation } => {
            ensure!(writable, "Development startup changes are disabled");
            ensure!(confirmation == "change_autostart_for_this_installation", "Startup confirmation is invalid");
            crate::autostart::set_if_expected(expected_enabled, requested)?
        }
        _ => anyhow::bail!("Not an autostart operation"),
    };
    Ok(Autostart { enabled: Some(enabled), writable,
        reason: (!writable).then(|| "Development mode: startup changes are disabled; status is for this installation only".into()) })
}

fn diagnostics() -> Result<Diagnostics> {
    // Same passive doctor as CLI; never resume protection or request elevation.
    // It is demand-driven and isolated from the existing observer's cadence.
    let monitor = crate::platform::PlatformMonitor::new(crate::config::Policy::default())?;
    let original = monitor.doctor();
    let mut incomplete = original.len() > 64;
    let mut budget = 16 * 1024;
    let mut checks = Vec::with_capacity(original.len().min(64));
    for row in original.into_iter().take(64) {
        if budget == 0 { incomplete = true; break; }
        let original_len = row.detail.len();
        let detail = bounded(row.detail, budget.min(1024));
        incomplete |= detail.len() != original_len;
        budget -= detail.len();
        checks.push(Check { name: bounded(row.name.into(), 64), status: match row.status {
            crate::model::DiagnosticStatus::Ok => "ok",
            crate::model::DiagnosticStatus::Warning => "warning",
            crate::model::DiagnosticStatus::Error => "error",
        }, detail, remedy: None });
    }
    let limitations = vec![
        "Point-in-time native diagnostics; successful checks do not guarantee absence of capture or protection.".into(),
        "No elevation, device mutation, installation change or permission grant was performed by this request.".into(),
    ];
    Ok(Diagnostics { checks, limitations, engine_version: env!("CARGO_PKG_VERSION"),
        platform: std::env::consts::OS, observed_at: chrono::Utc::now(), incomplete })
}

#[cfg(test)]
mod tests {
    use super::*;
    const SESSION: &str = "00112233445566778899aabbccddeeff";
    fn request(id: u64) -> Request {
        Request { session: SESSION.into(), id, observed_sequence: 1,
            operation: Operation::Diagnostics }
    }
    #[test]
    fn foreign_unwritten_unready_and_replayed_requests_cannot_dispatch() {
        let mut lane = Session::default();
        assert!(lane.admit(request(1), "ffeeddccbbaa99887766554433221100", 1, true, false, false).is_none());
        assert!(lane.admit(request(2), SESSION, 0, true, false, false).is_none());
        assert!(lane.admit(request(3), SESSION, 1, false, false, false).is_none());
        assert!(lane.admit(request(4), SESSION, 1, true, false, false).is_some());
        assert!(lane.finish(4, Err("denied".into())));
        assert!(lane.admit(request(4), SESSION, 1, true, false, false).is_none());
        assert!(!lane.finish(4, Err("late duplicate".into())));
        assert!(lane.admit(request(5), SESSION, 1, true, false, false).is_some());
    }
    #[test]
    fn old_completion_cannot_be_attached_to_rejected_overlap() {
        let mut lane = Session::default();
        assert!(lane.admit(request(1), SESSION, 1, true, false, false).is_some());
        assert!(lane.admit(request(2), SESSION, 1, true, false, false).is_none());
        assert!(!lane.finish(1, Err("old failure".into())));
        let view = lane.view(true);
        assert_eq!(view.result.as_ref().unwrap().id, 2);
        assert_eq!(view.result.as_ref().unwrap().state, CommandState::Rejected);
        assert!(lane.admit(request(3), SESSION, 1, true, false, false).is_some());
    }
    #[test]
    fn development_cannot_dispatch_startup_writes_even_with_a_valid_confirmed_session() {
        let operation = Operation::SetAutostart { requested: true, expected_enabled: false,
            confirmation: "change_autostart_for_this_installation".into() };
        let mut lane = Session::default();
        let write = Request { operation: operation.clone(), ..request(1) };
        assert!(lane.admit(write, SESSION, 1, true, false, false).is_none());
        assert_eq!(lane.view(true).result.as_ref().unwrap().code.as_deref(), Some("read_only"));
        // Independent worker guard must reject before any real native API.
        assert!(autostart(operation, false).is_err());
        assert!(lane.admit(Request { operation: Operation::AutostartStatus, ..request(2) },
            SESSION, 1, true, false, false).is_some());
    }
    #[test]
    fn query_parser_rejects_paths_excessive_pages_unknown_operations_and_fields() {
        for operation in [r#"{"kind":"history","cursor":null,"limit":0}"#,
            r#"{"kind":"history","cursor":null,"limit":51}"#,
            r#"{"kind":"history","cursor":null,"limit":1,"path":"/private"}"#,
            r#"{"kind":"diagnostics","admin":true}"#,
            r#"{"kind":"delete_history"}"#] {
            let json = format!(r#"{{"session":"{SESSION}","id":1,"observed_sequence":1,"operation":{operation}}}"#);
            assert!(serde_json::from_str::<Request>(&json).is_err(), "{operation}");
        }
    }
}
