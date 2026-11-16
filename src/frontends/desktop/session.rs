//! The tray owns the recorder. IPC clients can only submit typed commands.
mod files;

use crate::{
    model::{AccessEvent, SCHEMA_VERSION, Snapshot, event_code},
    watcher::TransitionTracker,
};
use anyhow::{Context, Result, ensure};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::{
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
    sync::{Arc, Mutex, mpsc},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;
const FOOTER_RESERVE: u64 = 1024;
const MAX_LINE_BYTES: usize = 256 * 1024;
const MAX_OBSERVATION_BYTES: usize = 4 * 1024 * 1024;
const MAX_PRESENTATION_MS: u32 = 5000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Cadence {
    pub native_ms: u32,
    pub camera_inventory_ms: Option<u32>,
    pub requested_ms: Option<u32>,
    pub presentation_ms: u32,
}

impl Cadence {
    pub fn native(interval: Duration, camera_inventory: Option<Duration>) -> Self {
        let native_ms = interval.as_millis() as u32;
        Self {
            native_ms,
            camera_inventory_ms: camera_inventory.map(|interval| interval.as_millis() as u32),
            requested_ms: None,
            presentation_ms: native_ms,
        }
    }

    fn requested(self, ms: u32) -> Self {
        Self {
            requested_ms: Some(ms),
            presentation_ms: ms.max(self.native_ms),
            ..self
        }
    }
}

pub fn validate_cadence(ms: u32) -> Result<()> {
    ensure!(
        (100..=MAX_PRESENTATION_MS).contains(&ms),
        "Invalid presentation interval"
    );
    Ok(())
}

/// Only the delivery loop consults this clock. It never schedules collection.
#[derive(Default)]
pub struct Presentation {
    dirty: bool,
    last: Option<Instant>,
}

impl Presentation {
    pub fn observed(&mut self) {
        self.dirty = true;
    }

    pub fn due(&self, now: Instant, cadence: Cadence) -> bool {
        self.dirty
            && self.last.is_none_or(|last| {
                now.duration_since(last) >= Duration::from_millis(cadence.presentation_ms.into())
            })
    }

    pub fn published(&mut self, now: Instant) {
        self.last = Some(now);
        self.dirty = false;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Phase {
    Idle,
    Recording,
    Stopping,
    Failed,
}

#[derive(Clone, Debug, Serialize)]
pub struct Status {
    phase: Phase,
    session_id: Option<String>,
    started_at: Option<DateTime<Utc>>,
    path: Option<String>,
    events_written: u64,
    bytes_written: u64,
    last_error: Option<String>,
    max_bytes: u64,
    origin: &'static str,
    pub cadence: Cadence,
    #[serde(skip)]
    revision: u64,
}

impl Status {
    pub fn idle(cadence: Cadence) -> Self {
        Self {
            phase: Phase::Idle,
            session_id: None,
            started_at: None,
            path: None,
            events_written: 0,
            bytes_written: 0,
            last_error: None,
            max_bytes: MAX_FILE_BYTES,
            origin: "native_owner",
            cadence,
            revision: 0,
        }
    }
}

#[derive(Debug)]
enum Command {
    Start(u64, u32, Reply),
    Stop(u64, Reply),
    Cadence(u32, Reply),
    Events(u64, Vec<Vec<u8>>),
    Wake,
    Shutdown,
}

type Reply = mpsc::Sender<Result<Status>>;

#[derive(Clone, Debug)]
pub struct Client {
    status: Arc<Mutex<Status>>,
    sender: mpsc::SyncSender<Command>,
}

pub struct Owner {
    client: Client,
    worker: Option<JoinHandle<()>>,
}

impl Owner {
    pub fn new(cadence: Cadence) -> Result<Self> {
        Self::spawn(cadence, crate::settings::data_dir, MAX_FILE_BYTES)
    }

    fn spawn(
        cadence: Cadence,
        directory: impl Fn() -> Result<std::path::PathBuf> + Send + 'static,
        max_bytes: u64,
    ) -> Result<Self> {
        let (sender, receiver) = mpsc::sync_channel(8);
        let status = Arc::new(Mutex::new(Status {
            max_bytes,
            ..Status::idle(cadence)
        }));
        let worker_status = Arc::clone(&status);
        let worker = thread::Builder::new()
            .name("mcw-session-journal".into())
            .spawn(move || run(receiver, &worker_status, directory))?;
        Ok(Self {
            client: Client { status, sender },
            worker: Some(worker),
        })
    }

    pub fn client(&self) -> Client {
        self.client.clone()
    }

    pub fn status(&self) -> Status {
        self.client.status()
    }

    pub fn finish(&mut self) {
        if let Some(worker) = self.worker.take() {
            let _ = self.client.sender.send(Command::Shutdown);
            let _ = worker.join();
        }
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        self.finish();
    }
}

impl Client {
    pub fn status(&self) -> Status {
        self.status.lock().expect("session status lock").clone()
    }

    pub fn prepare(&self, start: bool) -> Result<u64> {
        let status = self.status.lock().expect("session status lock");
        ensure!(
            !start || matches!(status.phase, Phase::Idle | Phase::Failed),
            "An observation session is already active"
        );
        Ok(status.revision)
    }

    pub fn execute(&self, revision: u64, requested_ms: Option<u32>) -> Result<Status> {
        let (reply, response) = mpsc::channel();
        let command = match requested_ms {
            Some(ms) => {
                validate_cadence(ms)?;
                Command::Start(revision, ms, reply)
            }
            None => Command::Stop(revision, reply),
        };
        self.sender
            .send(command)
            .context("Observation journal worker is unavailable")?;
        response
            .recv()
            .context("Observation journal worker is unavailable")?
    }

    pub fn set_cadence(&self, ms: u32) -> Result<Status> {
        validate_cadence(ms)?;
        let (reply, response) = mpsc::channel();
        self.sender
            .send(Command::Cadence(ms, reply))
            .context("Observation journal worker is unavailable")?;
        response
            .recv()
            .context("Observation journal worker is unavailable")?
    }
}

#[derive(Default)]
pub struct Observations {
    revision: Option<u64>,
    tracker: TransitionTracker,
}

impl Observations {
    pub fn observe(&mut self, client: &Client, snapshot: Option<&Snapshot>, at: DateTime<Utc>) {
        let status = client.status.lock().expect("session status lock");
        if status.phase != Phase::Recording {
            return;
        }
        let revision = status.revision;
        if self.revision != Some(revision) {
            self.tracker = TransitionTracker::default();
            self.revision = Some(revision);
        }
        drop(status);
        let Some(snapshot) = snapshot else {
            self.tracker.observation_gap();
            return;
        };
        let mut lines = Vec::new();
        let mut total = 0;
        let result = self.tracker.reconcile_complete(snapshot, |access, action| {
            let bytes = json_line(&AccessEvent {
                schema_version: SCHEMA_VERSION,
                event_code: event_code(action),
                tool_version: env!("CARGO_PKG_VERSION"),
                observed_at: at,
                action,
                access: access.clone(),
            })?;
            total += bytes.len();
            ensure!(
                total <= MAX_OBSERVATION_BYTES,
                "Observation journal batch limit exceeded"
            );
            lines.push(bytes);
            Ok(())
        });
        let mut status = client.status.lock().expect("session status lock");
        if status.phase == Phase::Recording && status.revision == revision {
            let result = result.and_then(|()| {
                if !lines.is_empty() {
                    client
                        .sender
                        .try_send(Command::Events(revision, lines))
                        .map_err(|_| {
                            anyhow::anyhow!("Observation journal queue unavailable or full")
                        })?;
                }
                Ok(())
            });
            if let Err(error) = result {
                failed(&mut status, &error);
                // If full, the queued work already guarantees a wakeup.
                let _ = client.sender.try_send(Command::Wake);
            }
        }
    }
}

fn failed(status: &mut Status, error: &anyhow::Error) {
    status.phase = Phase::Failed;
    let mut detail = format!("{error:#}");
    let mut end = detail.len().min(1024);
    while !detail.is_char_boundary(end) {
        end -= 1;
    }
    detail.truncate(end);
    status.last_error = Some(detail);
}

fn run(
    receiver: mpsc::Receiver<Command>,
    shared: &Mutex<Status>,
    directory: impl Fn() -> Result<std::path::PathBuf>,
) {
    let mut recorder = None::<Recorder>;
    while let Ok(command) = receiver.recv() {
        match command {
            Command::Start(revision, ms, reply) => {
                let mut status = shared.lock().expect("session status lock").clone();
                let result = if revision != status.revision || status.phase == Phase::Recording {
                    Err(anyhow::anyhow!(
                        "Observation session changed after confirmation"
                    ))
                } else {
                    if let Some(old) = recorder.take() {
                        stop(old, shared, "recording_error");
                    }
                    status.revision += 1;
                    let mut next = Status {
                        revision: status.revision,
                        max_bytes: status.max_bytes,
                        ..Status::idle(status.cadence)
                    };
                    match directory().and_then(|dir| Recorder::start(&dir, &mut next, ms)) {
                        Ok(file) => recorder = Some(file),
                        Err(error) => failed(&mut next, &error),
                    }
                    *shared.lock().expect("session status lock") = next.clone();
                    Ok(next)
                };
                let _ = reply.send(result);
            }
            Command::Stop(revision, reply) => {
                let status = shared.lock().expect("session status lock").clone();
                let result = if revision == status.revision {
                    if let Some(file) = recorder.take() {
                        stop(file, shared, "user");
                    }
                    Ok(shared.lock().expect("session status lock").clone())
                } else {
                    Err(anyhow::anyhow!(
                        "Observation session changed after confirmation"
                    ))
                };
                let _ = reply.send(result);
            }
            Command::Cadence(ms, reply) => {
                let mut status = shared.lock().expect("session status lock");
                status.cadence = status.cadence.requested(ms);
                let _ = reply.send(Ok(status.clone()));
            }
            Command::Events(revision, lines) => {
                let status = shared.lock().expect("session status lock").clone();
                if let Some(file) = recorder.as_mut()
                    && status.revision == revision
                {
                    let result = lines.iter().try_for_each(|line| file.event(line));
                    let mut status = shared.lock().expect("session status lock");
                    status.events_written = file.events;
                    status.bytes_written = file.bytes;
                    if let Err(error) = result {
                        failed(&mut status, &error);
                        drop(status);
                        stop(
                            recorder.take().expect("active file"),
                            shared,
                            "recording_error",
                        );
                    }
                }
            }
            Command::Shutdown => break,
            Command::Wake => {}
        }
        // Producer-side failures end the recording even when there are no
        // pending commands. The last complete row remains readable.
        let phase = shared.lock().expect("session status lock").phase;
        if phase == Phase::Failed
            && let Some(file) = recorder.take()
        {
            stop(file, shared, "recording_error");
        }
    }
    if let Some(file) = recorder {
        stop(file, shared, "owner_exit");
    }
}

fn stop(mut file: Recorder, shared: &Mutex<Status>, reason: &str) {
    let mut status = {
        let mut status = shared.lock().expect("session status lock");
        status.phase = Phase::Stopping;
        status.clone()
    };
    let had_error = status.last_error.is_some();
    let result = file.finish(if had_error { "recording_error" } else { reason });
    status.bytes_written = file.bytes;
    status.events_written = file.events;
    status.phase = if had_error {
        Phase::Failed
    } else {
        Phase::Idle
    };
    if let Err(error) = result {
        if reason == "owner_exit" {
            eprintln!("Observation journal shutdown: {error:#}");
        }
        failed(&mut status, &error);
    }
    *shared.lock().expect("session status lock") = status;
}

// The bounded writer limits memory as well as disallowing partial JSON rows.
fn json_line(value: &impl Serialize) -> Result<Vec<u8>> {
    struct Line(Vec<u8>);
    impl Write for Line {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if bytes.len() >= MAX_LINE_BYTES - self.0.len() {
                return Err(std::io::Error::other(
                    "Observation journal line limit exceeded",
                ));
            }
            self.0.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut line = Line(Vec::new());
    serde_json::to_writer(&mut line, value)?;
    line.0.push(b'\n');
    Ok(line.0)
}

struct Recorder {
    journal: files::JournalFile,
    bytes: u64,
    events: u64,
    max_bytes: u64,
}

impl Recorder {
    fn start(root: &Path, status: &mut Status, ms: u32) -> Result<Self> {
        let at = Utc::now();
        let cadence = status.cadence.requested(ms);
        let (id, path, journal) = files::create(root, at)?;
        status.path = Some(path);
        status.session_id = Some(id.clone());
        status.started_at = Some(at);
        let mut this = Self {
            journal,
            bytes: 0,
            events: 0,
            max_bytes: status.max_bytes,
        };
        let header = json_line(&serde_json::json!({
            "type": "session_start", "session_schema_version": 1, "session_id": id,
            "started_at": at, "consent": "explicit_local_recording", "cadence": cadence,
            "tool_version": env!("CARGO_PKG_VERSION"), "max_bytes": status.max_bytes,
        }))?;
        this.write_line(&header)
            .context("Write observation session header")?;
        status.bytes_written = this.bytes;
        status.cadence = cadence;
        status.phase = Phase::Recording;
        Ok(this)
    }

    fn write_line(&mut self, line: &[u8]) -> Result<()> {
        let file = &mut self.journal.file;
        let result = file.write_all(line).and_then(|()| file.flush());
        if let Err(error) = result {
            file.set_len(self.bytes)
                .and_then(|()| file.seek(SeekFrom::Start(self.bytes)).map(|_| ()))
                .with_context(|| {
                    format!("Could not restore the last complete JSONL line after {error}")
                })?;
            return Err(error.into());
        }
        self.bytes += line.len() as u64;
        Ok(())
    }

    fn event(&mut self, line: &[u8]) -> Result<()> {
        ensure!(
            self.bytes + line.len() as u64 + FOOTER_RESERVE <= self.max_bytes,
            "Observation journal reached its size limit ({} bytes)",
            self.max_bytes
        );
        self.write_line(line).context("Write observation event")?;
        self.events += 1;
        Ok(())
    }

    fn finish(&mut self, reason: &str) -> Result<()> {
        let identity = self.journal.verify();
        let footer = json_line(&serde_json::json!({
            "type": "session_stop", "stopped_at": Utc::now(), "reason": reason,
            "events_written": self.events, "bytes_before_footer": self.bytes,
        }))?;
        let offset = self.bytes;
        ensure!(
            offset + footer.len() as u64 <= self.max_bytes,
            "No room for the session footer"
        );
        self.write_line(&footer)
            .context("Write observation session footer")?;
        let file = &mut self.journal.file;
        file.sync_data()
            .context("Flush observation session to disk")?;
        file.seek(SeekFrom::Start(offset))?;
        let mut readback = vec![0; footer.len()];
        file.read_exact(&mut readback)?;
        ensure!(
            readback == footer && file.metadata()?.len() == self.bytes,
            "Observation session footer or file length readback failed"
        );
        identity
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        frontends::desktop::tools,
        model::{
            Access, Activity, CollectorHealth, CollectorState, Confidence, EnforcementDecision,
            Resource, Risk,
        },
    };
    use serde_json::{Value, json};
    use std::path::PathBuf;

    fn setup(max_bytes: u64) -> (tempfile::TempDir, PathBuf, Owner) {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap().join("MicCamWatch");
        let directory = root.clone();
        let cadence = Cadence::native(Duration::from_secs(2), None);
        let owner = Owner::spawn(cadence, move || Ok(directory.clone()), max_bytes).unwrap();
        (temp, root, owner)
    }

    fn sample(count: usize) -> Snapshot {
        Snapshot {
            accesses: (0..count)
                .map(|index| Access {
                    key: format!("test-{index}"),
                    resource: Resource::Microphone,
                    activity: Activity::Active,
                    risk: Risk::Unexplained,
                    confidence: Confidence::High,
                    enforcement: EnforcementDecision::Alert,
                    application: format!("Test application {index}"),
                    pid: None,
                    parent_pid: None,
                    executable: None,
                    parent_name: None,
                    device: None,
                    started_at: None,
                    evidence: vec![],
                    modules: vec![],
                    signature: None,
                    process: None,
                })
                .collect(),
            collectors: vec![CollectorHealth {
                collector: "wasapi",
                state: CollectorState::Healthy,
                detail: None,
            }],
            observation_gaps: vec![],
        }
    }

    fn rows(status: &Status) -> Vec<Value> {
        let text = std::fs::read_to_string(status.path.as_ref().unwrap()).unwrap();
        assert_eq!(status.bytes_written, text.len() as u64);
        text.lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    fn start(client: &Client, ms: u32) -> Status {
        let status = client
            .execute(client.prepare(true).unwrap(), Some(ms))
            .unwrap();
        assert_eq!(status.phase, Phase::Recording, "{:?}", status.last_error);
        status
    }

    #[test]
    fn journal_uses_full_snapshots_and_survives_client_close() {
        let (_temp, _, mut owner) = setup(MAX_FILE_BYTES);
        let client = owner.client();
        let first = start(&client, 1000);
        assert_eq!(first.cadence.presentation_ms, 2000);
        let mut observations = Observations::default();
        let snapshot = sample(160);
        observations.observe(&client, Some(&snapshot), Utc::now());
        drop(client); // A different IPC child may attach to the same owner.
        let client = owner.client();
        assert_eq!(client.status().session_id, first.session_id);
        assert_eq!(
            client.set_cadence(5000).unwrap().cadence.presentation_ms,
            5000
        );
        observations.observe(&client, Some(&snapshot), Utc::now());
        observations.observe(&client, None, Utc::now());
        let empty = sample(0);
        observations.observe(&client, Some(&empty), Utc::now()); // Rebaseline without STOP.
        let one = sample(1);
        observations.observe(&client, Some(&one), Utc::now());
        observations.observe(&client, Some(&empty), Utc::now());
        owner.finish();
        let stopped = client.status();
        assert_eq!(stopped.phase, Phase::Idle, "{:?}", stopped.last_error);
        assert_eq!(stopped.events_written, 162);
        let rows = rows(&stopped);
        assert_eq!(rows[0]["type"], "session_start");
        assert!(
            rows[1..161]
                .iter()
                .all(|r| r["schema_version"] == 3 && r["action"] == "start")
        );
        assert_eq!(rows[161]["action"], "start");
        assert_eq!(rows[162]["action"], "stop");
        assert_eq!(rows[163]["reason"], "owner_exit");
        assert_eq!(rows[163]["events_written"], 162);
    }

    #[test]
    fn journal_limit_keeps_complete_rows_and_the_next_session_gets_a_new_file() {
        let (_temp, _, owner) = setup(1700);
        let client = owner.client();
        let first = start(&client, 2000);
        Observations::default().observe(&client, Some(&sample(20)), Utc::now());
        let revision = client.prepare(false).unwrap();
        let stopped = client.execute(revision, None).unwrap();
        assert_eq!(stopped.phase, Phase::Failed);
        assert!(stopped.last_error.as_ref().unwrap().contains("size limit"));
        assert!(stopped.bytes_written <= 1700);
        assert_eq!(rows(&stopped).last().unwrap()["reason"], "recording_error");
        let second = start(&client, 5000);
        assert_ne!(first.path, second.path);
        assert!(client.execute(revision, None).is_err());
        let revision = client.prepare(false).unwrap();
        let stopped = client.execute(revision, None).unwrap();
        assert_eq!(rows(&stopped).last().unwrap()["reason"], "user");
        assert_eq!(
            client.execute(revision, None).unwrap().bytes_written,
            stopped.bytes_written
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let path = std::path::Path::new(second.path.as_deref().unwrap());
            assert_eq!(path.metadata().unwrap().permissions().mode() & 0o777, 0o600);
            assert_eq!(
                path.parent()
                    .unwrap()
                    .metadata()
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
        }
    }

    #[test]
    fn typed_preparation_is_read_only_and_consumed_by_confirmation() {
        let (_temp, root, owner) = setup(MAX_FILE_BYTES);
        let request = tools::Request::SessionStart { requested_ms: 1000 };
        let mut ipc = tools::Session::default();
        let context = tools::Context {
            operational: true,
            data_writable: true,
            observation_session: Some(owner.client()),
            ..Default::default()
        };
        assert!(
            ipc.execute(request.clone(), &tools::Context::default())
                .is_err()
        );
        assert!(
            serde_json::from_value::<tools::Request>(json!({
                "action": "session_start", "requested_ms": 1000, "path": "",
            }))
            .is_err()
        );
        let prepared = ipc.execute(request, &context).unwrap();
        assert!(!root.exists());
        let confirm = tools::Request::SessionConfirm {
            token: prepared.token.unwrap(),
            consent: "confirm_observation_session".into(),
        };
        assert_eq!(
            ipc.execute(confirm.clone(), &context).unwrap().data["observation_session"]["phase"],
            "recording"
        );
        assert!(ipc.execute(confirm, &context).is_err());
        let readback = tools::Session::default()
            .execute(tools::Request::SessionStatus {}, &context)
            .unwrap();
        assert_eq!(readback.data["observation_session"]["phase"], "recording");
    }

    #[test]
    fn adjusting_presentation_only_changes_publication_count() {
        let native = Cadence::native(Duration::from_millis(500), Some(Duration::from_secs(5)));
        let run = |cadence: Cadence| {
            let start = Instant::now();
            let mut presentation = Presentation::default();
            let mut collected = 0;
            let mut published = 0;
            for tick in 0..20 {
                let now = start + Duration::from_millis(tick * 500);
                collected += 1;
                presentation.observed();
                if presentation.due(now, cadence) {
                    presentation.published(now);
                    published += 1;
                }
            }
            (collected, published)
        };
        assert_eq!(run(native), (20, 20));
        assert_eq!(run(native.requested(100)), (20, 20));
        assert_eq!(run(native.requested(5000)), (20, 2));
    }

    #[cfg(unix)]
    #[test]
    fn refuses_symlinked_sessions_directory() {
        let (temp, root, owner) = setup(MAX_FILE_BYTES);
        std::fs::create_dir(&root).unwrap();
        let outside = temp.path().join("outside");
        std::fs::create_dir(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("sessions")).unwrap();
        let client = owner.client();
        let failed = client
            .execute(client.prepare(true).unwrap(), Some(1000))
            .unwrap();
        assert_eq!(failed.phase, Phase::Failed);
        assert_eq!(std::fs::read_dir(outside).unwrap().count(), 0);
    }
}
