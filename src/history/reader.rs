//! Read-only, recorded-order browsing. Each browser owns at most one five-minute
//! snapshot of four open files, never a writer lock between requests. Refreshing
//! replaces that snapshot; cursors are opaque, single-use and browser-local.
//!
//! Limits: 50 events and 512 KiB scanned per call; 64 KiB per JSONL record;
//! four captured ranges of at most 5 MiB + 64 KiB each per snapshot. A scan-limited
//! page can contain fewer events (even zero) and still provide a continuation.
//! Issues and `incomplete` disclose omitted, unsupported or truncated content.
//! Reader lock waits stop after 100 ms; the existing writer behavior is unchanged.
//! Only the shared coordination lock may be created; history files are read-only.
use super::{HISTORY_FILES, HistoryLock, MAX_HISTORY_BYTES};
use crate::model::SCHEMA_VERSION;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    collections::hash_map::RandomState,
    fs::{self, File, OpenOptions},
    hash::BuildHasher,
    io::{self, Read, Seek, SeekFrom},
    path::Path,
    time::{Duration, Instant},
};

const MAX_LINE_BYTES: usize = 64 * 1024;
const MAX_FILE_BYTES: u64 = MAX_HISTORY_BYTES + MAX_LINE_BYTES as u64;
const PAGE_SCAN_BYTES: usize = 512 * 1024;
const MAX_TEXT_BYTES: usize = 512;
const MAX_ISSUES: usize = 16;
const MAX_CURSOR_BYTES: usize = 256;
const SESSION_LIFETIME: Duration = Duration::from_secs(300);
const LOCK_WAIT: Duration = Duration::from_millis(100);

#[derive(Clone, Debug, Serialize)]
pub struct HistoryEntry {
    pub observed_at: String,
    pub action: String,
    pub resource: String,
    pub activity: String,
    pub confidence: String,
    pub application: Option<String>,
    pub pid: Option<u32>,
}

#[derive(Clone, Debug, Serialize)]
pub struct HistoryPage {
    pub events: Vec<HistoryEntry>,
    pub next_cursor: Option<String>,
    pub incomplete: bool,
    pub issues: Vec<String>,
    pub ordering: &'static str,
}

#[derive(Default)]
pub struct HistoryBrowser {
    session: Option<Snapshot>,
    cursor: Option<String>,
    nonce: RandomState,
    sequence: u64,
}

impl HistoryBrowser {
    /// Read actual persisted events, newest append first (not timestamp sorted).
    /// A fresh request invalidates the previous snapshot. Continuations exclude
    /// subsequent appends and survive the app's rename/delete based rotation and
    /// clear. Unknown/expired/replayed cursors and storage failures are errors.
    pub fn page(&mut self, cursor: Option<&str>, limit: u16) -> Result<HistoryPage> {
        self.page_at(&super::path()?, cursor, limit)
    }

    fn page_at(&mut self, path: &Path, cursor: Option<&str>, limit: u16) -> Result<HistoryPage> {
        if !(1..=50).contains(&limit) {
            bail!("history page limit must be between 1 and 50");
        }
        if self.session.as_ref().is_some_and(|session| session.created.elapsed() >= SESSION_LIFETIME) {
            self.session = None;
            self.cursor = None;
        }
        if let Some(cursor) = cursor {
            if cursor.len() > MAX_CURSOR_BYTES || self.cursor.as_deref() != Some(cursor) {
                bail!("history cursor is invalid, expired or already consumed; refresh history");
            }
        } else {
            self.session = None;
            self.cursor = None;
            self.session = Some(Snapshot::capture(path)?);
        }
        // On I/O failure, discard all continuation state instead of returning an
        // invented empty success or continuing from an uncertain position.
        let mut snapshot = self.session.take().context("history snapshot expired; refresh history")?;
        self.cursor = None;
        let mut events = Vec::with_capacity(usize::from(limit));
        let mut budget = PAGE_SCAN_BYTES;
        while events.len() < usize::from(limit) && snapshot.current < snapshot.files.len() && budget > 0 {
            let file = &mut snapshot.files[snapshot.current];
            match file.next_record(&mut budget)? {
                Record::Line(mut bytes) => {
                    let decoded = decode(&bytes);
                    bytes.clear();
                    snapshot.files[snapshot.current].line = bytes;
                    match decoded {
                        Ok(mut entry) => {
                            if let Some(application) = &mut entry.application {
                                if application.len() > MAX_TEXT_BYTES {
                                    truncate_utf8(application, MAX_TEXT_BYTES);
                                    snapshot.issue("An application name was truncated to 512 UTF-8 bytes.");
                                }
                            }
                            events.push(entry);
                        }
                        Err(issue) => snapshot.issue(issue),
                    }
                }
                Record::Issue(issue) => snapshot.issue(issue),
                Record::Finished => snapshot.current += 1,
                Record::Budget => break,
            }
        }
        // Completed files can be recognized without additional scanning, so an
        // exact page boundary does not unnecessarily retain exhausted handles.
        while snapshot.current < snapshot.files.len() && snapshot.files[snapshot.current].finished {
            snapshot.current += 1;
        }
        let more = snapshot.current < snapshot.files.len();
        let mut issues = snapshot.issues.clone();
        let scan_limited = more && budget == 0;
        if scan_limited {
            add_issue(&mut issues, "Page scan budget reached; continue with the next cursor.");
        }
        let incomplete = snapshot.incomplete || scan_limited;
        let next_cursor = if more {
            self.sequence = self.sequence.checked_add(1).context("history cursor sequence exhausted")?;
            let first = self.nonce.hash_one((self.sequence, 0_u8));
            let second = self.nonce.hash_one((self.sequence, 1_u8));
            let token = format!("{first:016x}{second:016x}{:016x}", self.sequence);
            self.cursor = Some(token.clone());
            self.session = Some(snapshot);
            Some(token)
        } else {
            None
        };
        Ok(HistoryPage { events, next_cursor, incomplete, issues, ordering: "recorded_newest_first" })
    }
}

struct Snapshot {
    files: Vec<ReverseFile>,
    current: usize,
    created: Instant,
    incomplete: bool,
    issues: Vec<String>,
}

impl Snapshot {
    fn capture(base: &Path) -> Result<Self> {
        let mut snapshot = Self {
            files: Vec::with_capacity(HISTORY_FILES + 1),
            current: 0,
            created: Instant::now(),
            incomplete: false,
            issues: Vec::new(),
        };
        // Only handle acquisition and length capture are serialized with writers.
        // No content scanning occurs under the lock.
        let Some(_lock) = reader_lock(base)? else { return Ok(snapshot) };
        for index in 0..=HISTORY_FILES {
            let path = if index == 0 { base.to_path_buf() } else { base.with_extension(format!("jsonl.{index}")) };
            let metadata = match fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error).context("failed to inspect history storage"),
            };
            if !metadata.file_type().is_file() {
                bail!("history storage is not a regular file");
            }
            let mut options = OpenOptions::new();
            options.read(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW);
            }
            #[cfg(windows)]
            {
                use std::os::windows::fs::OpenOptionsExt;
                // FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE keeps
                // current writers and rename/remove rotation/clear unblocked.
                options.share_mode(0x7).custom_flags(0x00200000);
            }
            let file = options.open(path).context("failed to open history storage")?;
            let metadata = file.metadata().context("failed to inspect opened history storage")?;
            if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
                bail!("opened history storage is not a regular file");
            }
            if metadata.len() > MAX_FILE_BYTES {
                snapshot.issue("An oversized history file was limited to its newest bounded byte range.");
            }
            if metadata.len() > 0 {
                snapshot.files.push(ReverseFile::new(file, metadata.len()));
            }
        }
        Ok(snapshot)
    }

    fn issue(&mut self, issue: &str) {
        self.incomplete = true;
        add_issue(&mut self.issues, issue);
    }
}

fn add_issue(issues: &mut Vec<String>, issue: &str) {
    if issues.iter().any(|existing| existing == issue) { return; }
    if issues.len() < MAX_ISSUES {
        let mut issue = issue.to_owned();
        truncate_utf8(&mut issue, 256);
        issues.push(issue);
    }
}

fn truncate_utf8(text: &mut String, max: usize) {
    if text.len() <= max { return; }
    let mut end = max;
    while !text.is_char_boundary(end) { end -= 1; }
    text.truncate(end);
}

// Separate from the existing blocking writer acquisition: readers must not wait
// indefinitely behind a writer or hold the writer lock while a user browses.
#[cfg(windows)]
fn reader_lock(_base: &Path) -> Result<Option<HistoryLock>> {
    use windows::{Win32::{Foundation::{CloseHandle, WAIT_ABANDONED, WAIT_OBJECT_0}, System::Threading::{CreateMutexW, WaitForSingleObject}}, core::w};
    let handle = unsafe { CreateMutexW(None, false, w!("Local\\MicCamWatch.History")) }
        .context("failed to create history reader mutex")?;
    let result = unsafe { WaitForSingleObject(handle, LOCK_WAIT.as_millis() as u32) };
    if result != WAIT_OBJECT_0 && result != WAIT_ABANDONED {
        let _ = unsafe { CloseHandle(handle) };
        bail!("history is busy or its reader mutex failed; retry the request");
    }
    Ok(Some(HistoryLock(handle)))
}

#[cfg(unix)]
fn reader_lock(base: &Path) -> Result<Option<HistoryLock>> {
    use std::os::unix::fs::OpenOptionsExt;
    let lock_path = base.with_extension("jsonl.lock");
    let file = match OpenOptions::new().create(true).truncate(false).write(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW).open(lock_path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).context("failed to open history reader lock"),
    };
    if !file.metadata()?.is_file() { bail!("history lock is not a regular file"); }
    let started = Instant::now();
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(Some(HistoryLock { _file: file })),
            Err(fs::TryLockError::WouldBlock) if started.elapsed() < LOCK_WAIT => std::thread::sleep(Duration::from_millis(2)),
            Err(fs::TryLockError::WouldBlock) => bail!("history is busy; retry the request"),
            Err(fs::TryLockError::Error(error)) => return Err(error).context("failed to lock history for reading"),
        }
    }
}

struct ReverseFile {
    file: File,
    position: u64,
    floor: u64,
    buffer: [u8; 8192],
    buffered: usize,
    line: Vec<u8>,
    oversized: bool,
    terminated: bool,
    finished: bool,
}

enum Record {
    Line(Vec<u8>),
    Issue(&'static str),
    Finished,
    Budget,
}

impl ReverseFile {
    fn new(file: File, length: u64) -> Self {
        Self { file, position: length, floor: length.saturating_sub(MAX_FILE_BYTES), buffer: [0; 8192], buffered: 0, line: Vec::new(), oversized: false, terminated: false, finished: false }
    }

    fn next_record(&mut self, budget: &mut usize) -> Result<Record> {
        loop {
            if self.buffered == 0 && self.position == self.floor {
                if self.finished { return Ok(Record::Finished); }
                self.finished = true;
                if self.floor != 0 {
                    self.line.clear();
                    return Ok(Record::Issue("The oldest partial record at the byte-range boundary was omitted."));
                }
                if !self.terminated && (!self.line.is_empty() || self.oversized) {
                    self.line.clear();
                    return Ok(Record::Issue("An incomplete final JSONL record was omitted."));
                }
                if self.oversized { return Ok(Record::Issue("A history record exceeded the 64 KiB line limit and was omitted.")); }
                if self.line.is_empty() { return Ok(Record::Finished); }
                self.line.reverse();
                return Ok(Record::Line(std::mem::take(&mut self.line)));
            }
            if *budget == 0 { return Ok(Record::Budget); }
            if self.buffered == 0 {
                let count = (self.position - self.floor).min(self.buffer.len() as u64) as usize;
                let start = self.position - count as u64;
                self.file.seek(SeekFrom::Start(start)).context("failed to seek history snapshot")?;
                self.file.read_exact(&mut self.buffer[..count]).context("history snapshot changed or could not be read")?;
                self.position = start;
                self.buffered = count;
            }
            self.buffered -= 1;
            *budget -= 1;
            let byte = self.buffer[self.buffered];
            if byte == b'\n' {
                if !self.terminated {
                    self.terminated = true;
                    let incomplete = !self.line.is_empty() || self.oversized;
                    self.line.clear();
                    self.oversized = false;
                    if incomplete { return Ok(Record::Issue("An incomplete final JSONL record was omitted.")); }
                    continue;
                }
                if self.oversized {
                    self.line.clear();
                    self.oversized = false;
                    return Ok(Record::Issue("A history record exceeded the 64 KiB line limit and was omitted."));
                }
                self.line.reverse();
                return Ok(Record::Line(std::mem::take(&mut self.line)));
            }
            if !self.oversized {
                if self.line.len() == MAX_LINE_BYTES {
                    self.oversized = true;
                    self.line.clear();
                } else {
                    self.line.push(byte);
                }
            }
        }
    }
}

#[derive(Deserialize)]
struct StoredEntry {
    schema_version: u64,
    observed_at: String,
    action: String,
    resource: String,
    activity: String,
    confidence: String,
    application: Option<String>,
    pid: Option<u32>,
}

fn decode(bytes: &[u8]) -> std::result::Result<HistoryEntry, &'static str> {
    // Inspect the version before decoding the current fields: future schemas
    // must be reported as unknown rather than guessed to fit today's model.
    #[derive(Deserialize)]
    struct Version { schema_version: u64 }
    let version: Version = serde_json::from_slice(bytes).map_err(|_| "A malformed history record was omitted.")?;
    if version.schema_version != u64::from(SCHEMA_VERSION) {
        return Err("A history record uses an unsupported schema version and was omitted.");
    }
    let entry: StoredEntry = serde_json::from_slice(bytes).map_err(|_| "A malformed history record was omitted.")?;
    if entry.schema_version != u64::from(SCHEMA_VERSION)
        || !matches!(entry.action.as_str(), "start" | "update" | "stop")
        || !matches!(entry.resource.as_str(), "microphone" | "camera")
        || !matches!(entry.activity.as_str(), "active" | "ready")
        || !matches!(entry.confidence.as_str(), "high" | "medium" | "low")
        || entry.observed_at.len() > MAX_TEXT_BYTES
        || chrono::DateTime::parse_from_rfc3339(&entry.observed_at).is_err()
    {
        return Err("A history record contains unsupported or invalid event fields and was omitted.");
    }
    Ok(HistoryEntry { observed_at: entry.observed_at, action: entry.action, resource: entry.resource, activity: entry.activity, confidence: entry.confidence, application: entry.application, pid: entry.pid })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::{append_at, rotate};
    use crate::model::{Access, AccessEvent, Action, Activity, Confidence, EnforcementDecision, Resource, Risk, event_code};
    use std::{io::Write, sync::{Mutex, MutexGuard, mpsc}};

    // Windows uses the same named mutex for all storage locations.
    static READER_TESTS: Mutex<()> = Mutex::new(());

    struct Storage {
        _serial: MutexGuard<'static, ()>,
        directory: tempfile::TempDir,
    }

    impl Storage {
        fn new() -> Self {
            Self { _serial: READER_TESTS.lock().unwrap(), directory: tempfile::tempdir().unwrap() }
        }
        fn path(&self) -> std::path::PathBuf { self.directory.path().join("history.jsonl") }
    }

    fn event(pid: u32) -> AccessEvent {
        AccessEvent {
            schema_version: SCHEMA_VERSION,
            event_code: event_code(Action::Update),
            tool_version: env!("CARGO_PKG_VERSION"),
            action: Action::Update,
            // Deliberately descending dates: pagination must use append order.
            observed_at: chrono::DateTime::parse_from_rfc3339(
                if pid % 2 == 0 { "2020-01-01T01:02:03Z" } else { "2025-01-01T04:05:06Z" },
            ).unwrap().with_timezone(&chrono::Utc),
            access: Access {
                key: format!("camera:{pid}"),
                resource: Resource::Camera,
                activity: Activity::Ready,
                risk: Risk::Expected,
                confidence: Confidence::Low,
                enforcement: EnforcementDecision::Alert,
                application: format!("application-{pid}.exe"),
                pid: Some(pid),
                parent_pid: None,
                parent_name: None,
                executable: None,
                signature: None,
                device: None,
                started_at: None,
                modules: vec![],
                evidence: vec![],
                process: None,
            },
        }
    }

    fn line(pid: u32) -> Vec<u8> {
        let mut bytes = serde_json::to_vec(&event(pid)).unwrap();
        bytes.push(b'\n');
        bytes
    }

    fn pids(page: &HistoryPage) -> Vec<u32> {
        page.events.iter().map(|entry| entry.pid.unwrap()).collect()
    }

    #[test]
    fn fifty_event_bound_and_remaining_model_values_are_preserved() {
        let storage = Storage::new();
        let path = storage.path();
        let mut first_event = event(0);
        first_event.action = Action::Start;
        first_event.event_code = event_code(Action::Start);
        first_event.access.resource = Resource::Microphone;
        first_event.access.activity = Activity::Active;
        first_event.access.confidence = Confidence::High;
        first_event.access.pid = None;
        append_at(&path, &first_event).unwrap();
        for pid in 1..=55 {
            let mut next = event(pid);
            if pid == 55 {
                next.action = Action::Stop;
                next.event_code = event_code(Action::Stop);
                next.access.confidence = Confidence::Medium;
            }
            append_at(&path, &next).unwrap();
        }
        let mut browser = HistoryBrowser::default();
        let page = browser.page_at(&path, None, 50).unwrap();
        assert_eq!(page.events.len(), 50);
        assert_eq!(pids(&page), (6..=55).rev().collect::<Vec<_>>());
        assert_eq!(page.events[0].action, "stop");
        assert_eq!(page.events[0].confidence, "medium");
        let rest = browser.page_at(&path, page.next_cursor.as_deref(), 50).unwrap();
        assert_eq!(rest.events.len(), 6);
        let oldest = rest.events.last().unwrap();
        assert_eq!(oldest.action, "start");
        assert_eq!(oldest.resource, "microphone");
        assert_eq!(oldest.activity, "active");
        assert_eq!(oldest.confidence, "high");
        assert_eq!(oldest.pid, None);
        assert!(rest.next_cursor.is_none());
    }

    #[test]
    fn reads_real_events_in_recorded_order_and_does_not_modify_history() {
        let storage = Storage::new();
        let path = storage.path();
        for pid in 1..=5 { append_at(&path, &event(pid)).unwrap(); }
        let original = fs::read(&path).unwrap();
        let mut browser = HistoryBrowser::default();
        let first = browser.page_at(&path, None, 2).unwrap();
        assert_eq!(pids(&first), [5, 4]);
        assert_eq!(first.ordering, "recorded_newest_first");
        assert!(!first.incomplete);
        let entry = &first.events[0];
        assert_eq!(entry.observed_at, "2025-01-01T04:05:06Z");
        assert_eq!(entry.action, "update");
        assert_eq!(entry.resource, "camera");
        assert_eq!(entry.activity, "ready");
        assert_eq!(entry.confidence, "low");
        assert_eq!(entry.application.as_deref(), Some("application-5.exe"));
        let second = browser.page_at(&path, first.next_cursor.as_deref(), 2).unwrap();
        assert_eq!(pids(&second), [3, 2]);
        let last = browser.page_at(&path, second.next_cursor.as_deref(), 2).unwrap();
        assert_eq!(pids(&last), [1]);
        assert!(last.next_cursor.is_none());
        assert!(browser.session.is_none());
        assert_eq!(fs::read(path).unwrap(), original);
    }

    #[test]
    fn continuations_survive_rotation_archive_deletion_and_clear() {
        let storage = Storage::new();
        let path = storage.path();
        for pid in 1..=4 {
            if pid > 1 {
                let _lock = HistoryLock::acquire(&path).unwrap();
                rotate(&path).unwrap();
            }
            append_at(&path, &event(pid)).unwrap();
        }
        let mut browser = HistoryBrowser::default();
        let first = browser.page_at(&path, None, 1).unwrap();
        assert_eq!(pids(&first), [4]);
        for pid in 5..=8 {
            {
                let _lock = HistoryLock::acquire(&path).unwrap();
                rotate(&path).unwrap();
            }
            append_at(&path, &event(pid)).unwrap();
        }
        {
            let _lock = HistoryLock::acquire(&path).unwrap();
            for index in 0..=HISTORY_FILES {
                let candidate = if index == 0 { path.clone() } else { path.with_extension(format!("jsonl.{index}")) };
                fs::remove_file(candidate).unwrap();
            }
        }
        append_at(&path, &event(9)).unwrap();
        let rest = browser.page_at(&path, first.next_cursor.as_deref(), 50).unwrap();
        assert_eq!(pids(&rest), [3, 2, 1]);
        assert!(rest.next_cursor.is_none());
        assert!(!rest.incomplete);
        let refreshed = browser.page_at(&path, None, 50).unwrap();
        assert_eq!(pids(&refreshed), [9]);
    }

    #[test]
    fn current_writer_stays_open_and_later_appends_are_excluded() {
        let storage = Storage::new();
        let path = storage.path();
        append_at(&path, &event(1)).unwrap();
        append_at(&path, &event(2)).unwrap();
        let mut writer = OpenOptions::new().append(true).open(&path).unwrap();
        let mut browser = HistoryBrowser::default();
        let first = browser.page_at(&path, None, 1).unwrap();
        {
            let _lock = HistoryLock::acquire(&path).unwrap();
            writer.write_all(&line(3)).unwrap();
        }
        let last = browser.page_at(&path, first.next_cursor.as_deref(), 50).unwrap();
        assert_eq!(pids(&last), [1]);
        assert!(!last.incomplete);
        assert_eq!(pids(&browser.page_at(&path, None, 50).unwrap()), [3, 2, 1]);
    }

    #[test]
    fn rejects_paths_replays_other_browsers_expiration_and_invalid_limits() {
        let storage = Storage::new();
        let path = storage.path();
        for pid in 1..=4 { append_at(&path, &event(pid)).unwrap(); }
        let mut browser = HistoryBrowser::default();
        for limit in [0, 51, u16::MAX] {
            assert!(browser.page_at(&path, None, limit).is_err());
        }
        let first = browser.page_at(&path, None, 1).unwrap();
        let cursor = first.next_cursor.unwrap();
        assert!(cursor.len() <= MAX_CURSOR_BYTES);
        let mut other = HistoryBrowser::default();
        assert!(other.page_at(&path, Some(&cursor), 1).is_err());
        for invalid in ["../history.jsonl", "C:\\history.jsonl", "", &"x".repeat(257)] {
            assert!(browser.page_at(&path, Some(invalid), 1).is_err());
        }
        let second = browser.page_at(&path, Some(&cursor), 1).unwrap();
        assert!(browser.page_at(&path, Some(&cursor), 1).is_err());
        let fresh = browser.page_at(&path, None, 1).unwrap();
        assert!(browser.page_at(&path, second.next_cursor.as_deref(), 1).is_err());
        browser.session.as_mut().unwrap().created = Instant::now() - SESSION_LIFETIME;
        assert!(browser.page_at(&path, fresh.next_cursor.as_deref(), 1).is_err());
        assert!(browser.session.is_none());
        assert!(browser.cursor.is_none());
    }

    #[test]
    fn absent_is_empty_but_invalid_storage_and_busy_writer_are_errors() {
        let storage = Storage::new();
        let path = storage.path();
        let mut browser = HistoryBrowser::default();
        let missing = browser.page_at(&path, None, 50).unwrap();
        assert!(missing.events.is_empty());
        assert!(missing.next_cursor.is_none());
        assert!(!missing.incomplete);
        let nested = storage.directory.path().join("absent").join("history.jsonl");
        assert!(!browser.page_at(&nested, None, 50).unwrap().incomplete);
        fs::create_dir(&path).unwrap();
        assert!(browser.page_at(&path, None, 50).is_err());
        fs::remove_dir(&path).unwrap();
        append_at(&path, &event(1)).unwrap();
        let (ready_tx, ready_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let held_path = path.clone();
        let worker = std::thread::spawn(move || {
            let _lock = HistoryLock::acquire(&held_path).unwrap();
            ready_tx.send(()).unwrap();
            release_rx.recv().unwrap();
        });
        ready_rx.recv().unwrap();
        let busy = browser.page_at(&path, None, 50);
        release_tx.send(()).unwrap();
        worker.join().unwrap();
        assert!(busy.is_err());
        assert_eq!(pids(&browser.page_at(&path, None, 50).unwrap()), [1]);
    }

    #[test]
    fn malformed_future_invalid_utf8_and_incomplete_lines_are_reported() {
        let storage = Storage::new();
        let path = storage.path();
        let mut bytes = line(1);
        bytes.extend_from_slice(b"{malformed}\n");
        let mut future = serde_json::to_value(event(2)).unwrap();
        future["schema_version"] = serde_json::json!(SCHEMA_VERSION + 1);
        bytes.extend_from_slice(&serde_json::to_vec(&future).unwrap());
        bytes.extend_from_slice(b"\n\xff\n\n");
        let mut unsupported = serde_json::to_value(event(3)).unwrap();
        unsupported["confidence"] = serde_json::json!("certain");
        bytes.extend_from_slice(&serde_json::to_vec(&unsupported).unwrap());
        bytes.push(b'\n');
        bytes.extend_from_slice(&line(4));
        // Even valid JSON is incomplete without the writer's terminating LF.
        bytes.extend_from_slice(&serde_json::to_vec(&event(5)).unwrap());
        fs::write(&path, bytes).unwrap();
        let page = HistoryBrowser::default().page_at(&path, None, 50).unwrap();
        assert_eq!(pids(&page), [4, 1]);
        assert!(page.incomplete);
        assert!(page.next_cursor.is_none());
        assert!(page.issues.iter().any(|issue| issue.contains("unsupported schema")));
        assert!(page.issues.iter().any(|issue| issue.contains("malformed")));
        assert!(page.issues.iter().any(|issue| issue.contains("incomplete final")));
        assert!(page.issues.iter().any(|issue| issue.contains("event fields")));
        assert!(page.issues.len() <= MAX_ISSUES);
        assert!(page.issues.iter().all(|issue| issue.len() <= 256));
        fs::write(&path, b"{\"unfinished\":").unwrap();
        let only_partial = HistoryBrowser::default().page_at(&path, None, 50).unwrap();
        assert!(only_partial.events.is_empty());
        assert!(only_partial.incomplete);
    }

    fn padded_line(pid: u32, length: usize) -> Vec<u8> {
        let mut record = serde_json::to_value(event(pid)).unwrap();
        record["padding"] = serde_json::json!("");
        let base = serde_json::to_vec(&record).unwrap().len();
        record["padding"] = serde_json::json!("x".repeat(length - base));
        let mut bytes = serde_json::to_vec(&record).unwrap();
        assert_eq!(bytes.len(), length);
        bytes.push(b'\n');
        bytes
    }

    #[test]
    fn line_and_scan_boundaries_continue_without_inventing_or_losing_events() {
        let storage = Storage::new();
        let path = storage.path();
        let mut bytes = padded_line(1, MAX_LINE_BYTES);
        bytes.extend_from_slice(&padded_line(2, MAX_LINE_BYTES + 1));
        bytes.extend_from_slice(&padded_line(3, PAGE_SCAN_BYTES + 17));
        bytes.extend_from_slice(&line(4));
        fs::write(&path, bytes).unwrap();
        let mut browser = HistoryBrowser::default();
        let first = browser.page_at(&path, None, 50).unwrap();
        assert_eq!(pids(&first), [4]);
        assert!(first.incomplete);
        assert!(first.issues.iter().any(|issue| issue.contains("scan budget")));
        let last = browser.page_at(&path, first.next_cursor.as_deref(), 50).unwrap();
        assert_eq!(pids(&last), [1]);
        assert!(last.incomplete);
        assert!(last.next_cursor.is_none());
        assert!(last.issues.iter().any(|issue| issue.contains("64 KiB")));
    }

    #[test]
    fn oversized_file_scan_is_bounded_and_unicode_text_is_truncated_safely() {
        let storage = Storage::new();
        let path = storage.path();
        let mut file = File::create(&path).unwrap();
        file.set_len(MAX_FILE_BYTES + 4096).unwrap();
        file.seek(SeekFrom::End(0)).unwrap();
        file.write_all(b"\n").unwrap();
        let mut record = serde_json::to_value(event(7)).unwrap();
        record["application"] = serde_json::json!("é".repeat(300));
        serde_json::to_writer(&mut file, &record).unwrap();
        file.write_all(b"\n").unwrap();
        drop(file);
        let mut browser = HistoryBrowser::default();
        let mut page = browser.page_at(&path, None, 50).unwrap();
        assert_eq!(pids(&page), [7]);
        assert!(page.incomplete);
        let app = page.events[0].application.as_ref().unwrap();
        assert_eq!(app.len(), MAX_TEXT_BYTES);
        assert_eq!(app, &"é".repeat(256));
        assert!(page.issues.iter().any(|issue| issue.contains("oversized history")));
        assert!(page.issues.iter().any(|issue| issue.contains("truncated")));
        let mut pages = 1;
        while let Some(cursor) = page.next_cursor.take() {
            page = browser.page_at(&path, Some(&cursor), 50).unwrap();
            assert!(page.events.is_empty());
            pages += 1;
            assert!(pages <= (MAX_FILE_BYTES as usize / PAGE_SCAN_BYTES) + 2);
        }
        assert!(page.incomplete);
        assert!(browser.session.is_none());
    }

    #[test]
    fn truncated_open_snapshot_is_an_error_not_empty_success() {
        let storage = Storage::new();
        let path = storage.path();
        // More than one I/O block ensures the continuation needs a fresh read.
        fs::write(&path, [padded_line(1, 20_000), padded_line(2, 20_000)].concat()).unwrap();
        let mut browser = HistoryBrowser::default();
        let first = browser.page_at(&path, None, 1).unwrap();
        assert_eq!(pids(&first), [2]);
        OpenOptions::new().write(true).open(&path).unwrap().set_len(0).unwrap();
        assert!(browser.page_at(&path, first.next_cursor.as_deref(), 50).is_err());
        assert!(browser.session.is_none());
        assert!(browser.cursor.is_none());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinks_and_special_files_without_blocking() {
        use std::{ffi::CString, os::unix::{ffi::OsStrExt, fs::symlink}};
        let storage = Storage::new();
        let path = storage.path();
        let target = storage.directory.path().join("other.jsonl");
        fs::write(&target, line(1)).unwrap();
        symlink(&target, &path).unwrap();
        assert!(HistoryBrowser::default().page_at(&path, None, 50).is_err());
        fs::remove_file(&path).unwrap();
        let fifo = CString::new(path.as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
        assert!(HistoryBrowser::default().page_at(&path, None, 50).is_err());
    }
}
