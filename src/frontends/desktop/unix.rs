//! Owned Unix Flutter child. No listener, public endpoint, shell, or hardware command worker.
//! Nonblocking private pipes are pumped by the native owner; memory and per-poll work are bounded.
use super::{
    DesktopSnapshot, EventLines, UiEvent,
    commands::{CommandSession, CommandState, MicrophoneCapability},
    preferences::{PreferenceRequest, PreferenceSession, PreferencesSnapshot},
};
use anyhow::{Context, Result, ensure};
use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Write},
    os::unix::{fs::{MetadataExt, OpenOptionsExt}, io::AsRawFd, process::CommandExt},
    path::{Path, PathBuf},
    process::{Child as Process, ChildStdin, ChildStdout, Command, Stdio},
    time::{Duration, Instant},
};

const POLL_BYTES: usize = 65_536;
const POLL_EVENTS: usize = 64;
const READY_TIMEOUT: Duration = Duration::from_secs(15);
const IO_TIMEOUT: Duration = Duration::from_secs(5);
const CLOSE_GRACE: Duration = Duration::from_secs(2);

struct Frame {
    sequence: u64,
    bytes: Vec<u8>,
    offset: usize,
}
#[derive(Default)]
struct Outbox {
    writing: Option<Frame>,
    latest: Option<Frame>,
    highest: u64,
    written: u64,
    progress: Option<Instant>,
    written_at: Option<Instant>,
    stopped: bool,
}
impl Outbox {
    fn accepts(&self, sequence: u64) -> bool {
        !self.stopped && sequence > self.highest && sequence <= i64::MAX as u64
    }
    fn replace(&mut self, frame: Frame) {
        if self.accepts(frame.sequence) {
            self.highest = frame.sequence;
            self.latest = Some(frame);
        }
    }
    fn stop(&mut self) {
        self.stopped = true;
        self.writing = None;
        self.latest = None;
        self.progress = None;
    }
    fn pump(&mut self, pipe: &mut impl Write, now: Instant) -> io::Result<()> {
        let mut budget = POLL_BYTES;
        for _ in 0..POLL_EVENTS {
            if budget == 0 || self.stopped { break; }
            if self.writing.is_none() {
                self.writing = self.latest.take();
                self.progress = self.writing.as_ref().map(|_| now);
            }
            let Some(frame) = self.writing.as_mut() else { break };
            let end = (frame.offset + budget).min(frame.bytes.len());
            match pipe.write(&frame.bytes[frame.offset..end]) {
                Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
                Ok(count) => {
                    frame.offset += count;
                    budget -= count;
                    self.progress = Some(now);
                    if frame.offset == frame.bytes.len() {
                        self.written = frame.sequence;
                        self.written_at = Some(now);
                        self.writing = None;
                        self.progress = None;
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
        }
        if self.progress.is_some_and(|at| now.saturating_duration_since(at) >= IO_TIMEOUT) {
            return Err(io::ErrorKind::TimedOut.into());
        }
        Ok(())
    }
    fn fresh(&self, now: Instant) -> bool {
        self.written_at.is_some_and(|at| now.saturating_duration_since(at) <= IO_TIMEOUT)
    }
}

/// Keep the bundle path components open for the lifetime of the exact owned child.
/// Mutable-by-other-user or symlinked bundles cannot authorize settings writes.
struct Bundle {
    _pins: Vec<File>,
    executable: File,
}
impl Bundle {
    fn open(path: &Path) -> Result<Self> {
        ensure!(path.is_absolute(), "desktop executable must be absolute");
        let mut pins = Vec::new();
        let mut component_path = PathBuf::new();
        for component in path.parent().context("desktop has no directory")?.components() {
            component_path.push(component);
            let directory = OpenOptions::new().read(true)
                .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open(&component_path).with_context(|| format!("open bundle directory {}", component_path.display()))?;
            let metadata = directory.metadata()?;
            let root_sticky = metadata.uid() == 0 && metadata.mode() & 0o1000 != 0;
            ensure!((metadata.uid() == 0 || metadata.uid() == unsafe { libc::geteuid() })
                && (metadata.mode() & 0o022 == 0 || root_sticky), "unsafe desktop directory ownership or permissions");
            pins.push(directory);
        }
        let executable = OpenOptions::new().read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC).open(path)
            .with_context(|| format!("open Flutter bundle executable {}", path.display()))?;
        let metadata = executable.metadata()?;
        ensure!(metadata.is_file() && metadata.nlink() == 1 && metadata.mode() & 0o111 != 0
            && metadata.mode() & 0o022 == 0
            && (metadata.uid() == 0 || metadata.uid() == unsafe { libc::geteuid() }),
            "unsafe desktop executable ownership, links or permissions");
        Ok(Self { _pins: pins, executable })
    }
    fn verify(&self, path: &Path, pid: u32) -> Result<()> {
        let expected = self.executable.metadata()?;
        let actual = std::fs::symlink_metadata(path)?;
        ensure!(expected.dev() == actual.dev() && expected.ino() == actual.ino()
            && !actual.file_type().is_symlink(), "desktop executable changed during spawn");
        #[cfg(target_os = "linux")]
        {
            let image = std::fs::metadata(format!("/proc/{pid}/exe"))?;
            ensure!(image.dev() == expected.dev() && image.ino() == expected.ino(), "owned child image does not match desktop executable");
        }
        #[cfg(target_os = "macos")]
        let _ = pid;
        Ok(())
    }
}

pub struct AuthorizedPreference {
    pub ticket: u64,
    pub request: PreferenceRequest,
}
struct Child {
    process: Process,
    input: Option<ChildStdin>,
    output: ChildStdout,
    outbox: Outbox,
    lines: EventLines,
    events: Vec<UiEvent>,
    commands: CommandSession,
    preferences: PreferenceSession,
    capability: PreferencesSnapshot,
    queued: Option<AuthorizedPreference>,
    pending: Option<(u64, u64)>,
    changed: bool,
    ready: bool,
    started: Instant,
    closing: Option<Instant>,
    _bundle: Bundle,
}
fn disabled_microphone() -> MicrophoneCapability {
    MicrophoneCapability {
        reason: Some("Unix Flutter hardware commands are not qualified; use the native controls".into()),
        ..Default::default()
    }
}
impl Child {
    fn frame(&mut self, mut snapshot: DesktopSnapshot) -> Result<Frame> {
        // Never trust caller-supplied hardware capabilities on this initial Unix route.
        snapshot.controls = self.commands.controls(disabled_microphone());
        self.capability = std::mem::take(&mut snapshot.preferences);
        snapshot.preferences = self.preferences.view(self.capability.clone());
        Ok(Frame { sequence: snapshot.sequence, bytes: snapshot.encode()?, offset: 0 })
    }
    fn live_ready(&self, now: Instant) -> bool {
        self.ready && self.closing.is_none() && self.outbox.fresh(now)
    }
    fn event(&mut self, event: UiEvent, next_ticket: &mut u64, now: Instant) {
        match event {
            UiEvent::Ready { sequence, .. } => {
                if sequence > 0 && sequence <= self.outbox.written && self.closing.is_none() {
                    self.ready = true;
                }
            }
            UiEvent::Timing { .. } => {}
            UiEvent::Command { request, .. } => {
                let ready = self.live_ready(now);
                let _ = self.commands.admit(request, self.outbox.written, ready, &disabled_microphone());
                self.changed = true;
            }
            UiEvent::PreferencesCommand { request, .. } => {
                let ready = self.live_ready(now);
                if let Some(request) = self.preferences.admit(request, self.commands.session(),
                    self.outbox.written, ready, &self.capability) {
                    if let Some(ticket) = next_ticket.checked_add(1) {
                        *next_ticket = ticket;
                        self.pending = Some((ticket, request.id));
                        self.queued = Some(AuthorizedPreference { ticket, request });
                    } else {
                        self.preferences.finish(request.id, CommandState::Rejected,
                            Some("unavailable"), Some("Preference ticket space exhausted".into()));
                    }
                }
                self.changed = true;
            }
        }
    }
    fn begin_close(&mut self, now: Instant) {
        self.closing.get_or_insert(now);
        // EOF is the cross-platform quit command. Dropping input also discards
        // incomplete presentation; already admitted preference work lives outside this child.
        self.input.take();
        self.outbox.stop();
    }
    fn kill_owned(&mut self) {
        if matches!(self.process.try_wait(), Ok(Some(_))) { return; }
        // The process owns a new group; never signal an unowned PID or caller-supplied endpoint.
        unsafe { libc::kill(-(self.process.id() as i32), libc::SIGKILL); }
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
    fn pump(&mut self, next_ticket: &mut u64, now: Instant) -> Result<()> {
        if let Some(input) = self.input.as_mut() { self.outbox.pump(input, now)?; }
        let mut buffer = [0u8; 4096];
        let mut events = std::mem::take(&mut self.events);
        let mut invalid = 0usize;
        let mut consumed = 0;
        for _ in 0..POLL_EVENTS {
            if consumed >= POLL_BYTES { break; }
            match self.output.read(&mut buffer) {
                Ok(0) => anyhow::bail!("Flutter child closed its event pipe"),
                Ok(count) => {
                    consumed += count;
                    self.lines.feed(&buffer[..count], |event| match event {
                        Ok(event) if events.len() < POLL_EVENTS => events.push(event),
                        _ => invalid += 1,
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error.into()),
            }
        }
        // Native Flutter diagnostics on stdout are not commands; reject them.
        // An event flood is retired rather than letting it starve native collection.
        ensure!(invalid < POLL_EVENTS && consumed < POLL_BYTES, "Flutter output exceeded bounded event budget");
        for event in events.drain(..) { self.event(event, next_ticket, now); }
        self.events = events;
        ensure!(self.ready || now.saturating_duration_since(self.started) < READY_TIMEOUT,
            "Flutter readiness timed out");
        Ok(())
    }
}
impl Drop for Child {
    fn drop(&mut self) { self.begin_close(Instant::now()); self.kill_owned(); }
}

/// Lazy process owner. Singleton opens reuse the one owned child and challenge.
/// No UI-close can cancel work already taken by the process-owned preferences worker.
pub struct Supervisor {
    executable: PathBuf,
    child: Option<Child>,
    next_ticket: u64,
    error: Option<String>,
}
impl Supervisor {
    pub fn new() -> Result<Self> {
        let current = std::env::current_exe()?.canonicalize()?;
        let root = current.parent().context("native executable has no parent")?.join("desktop");
        #[cfg(target_os = "linux")]
        let executable = root.join("miccamwatch_desktop");
        #[cfg(target_os = "macos")]
        let executable = root.join("MicCamWatch.app/Contents/MacOS/MicCamWatch");
        Ok(Self { executable, child: None, next_ticket: 0, error: None })
    }
    pub fn is_running(&self) -> bool { self.child.is_some() }
    pub fn open(&mut self, initial: DesktopSnapshot, theme: &str) -> Result<()> {
        ensure!(matches!(theme, "system" | "light" | "dark"), "unsupported desktop theme");
        self.poll();
        if self.child.as_ref().is_some_and(|child| child.closing.is_some()) {
            self.child.take();
        }
        if let Some(child) = self.child.as_mut() {
            if child.ready {
                // Both branded runners install their owned-window activation handler before ready.
                ensure!(unsafe { libc::kill(child.process.id() as i32, libc::SIGUSR1) } == 0,
                    "cannot activate owned Flutter child");
            }
            return self.publish(initial);
        }
        self.child = Some(spawn(&self.executable, initial, theme)?);
        self.error = None;
        Ok(())
    }
    pub fn publish(&mut self, snapshot: DesktopSnapshot) -> Result<()> {
        if let Some(child) = self.child.as_mut() {
            if child.closing.is_none() && child.outbox.accepts(snapshot.sequence) {
                let frame = child.frame(snapshot)?;
                child.outbox.replace(frame);
                child.changed = false;
            }
        }
        Ok(())
    }
    pub fn poll(&mut self) {
        let now = Instant::now();
        let Some(child) = self.child.as_mut() else { return };
        let exited = match child.process.try_wait() {
            Ok(Some(status)) => {
                if !status.success() { self.error = Some(format!("Flutter child exited: {status}")); }
                true
            }
            Ok(None) => false,
            Err(error) => { self.error = Some(error.to_string()); child.begin_close(now); false }
        };
        if !exited && child.closing.is_none() {
            if let Err(error) = child.pump(&mut self.next_ticket, now) {
                self.error = Some(format!("{error:#}"));
                child.begin_close(now);
            }
        }
        if exited || child.closing.is_some_and(|at| now.saturating_duration_since(at) >= CLOSE_GRACE) {
            self.child.take();
        }
    }
    pub fn close(&mut self) -> Result<()> {
        if let Some(child) = self.child.as_mut() { child.begin_close(Instant::now()); }
        Ok(())
    }
    pub fn take_preference(&mut self) -> Option<AuthorizedPreference> { self.child.as_mut()?.queued.take() }
    pub fn take_controls_changed(&mut self) -> bool {
        self.child.as_mut().is_some_and(|child| std::mem::take(&mut child.changed))
    }
    pub fn take_error(&mut self) -> Option<String> { self.error.take() }
    pub fn complete_preference(&mut self, ticket: u64, outcome: CommandState,
        code: Option<&str>, detail: Option<String>) {
        let Some(child) = self.child.as_mut() else { return };
        let Some((expected, id)) = child.pending else { return };
        if expected == ticket && child.preferences.finish(id, outcome, code, detail) {
            child.pending = None;
            child.changed = true;
        }
    }
}
fn nonblocking(pipe: &impl AsRawFd) -> Result<()> {
    let descriptor = pipe.as_raw_fd();
    let flags = unsafe { libc::fcntl(descriptor, libc::F_GETFL) };
    ensure!(flags >= 0 && unsafe { libc::fcntl(descriptor, libc::F_SETFL, flags | libc::O_NONBLOCK) } >= 0,
        "cannot configure private desktop pipe: {}", io::Error::last_os_error());
    Ok(())
}
fn challenge() -> Result<String> {
    let mut bytes = [0u8; 16];
    // Both supported Unix kernels provide the OS CSPRNG here; no fallback PRNG.
    OpenOptions::new().read(true).custom_flags(libc::O_CLOEXEC).open("/dev/urandom")?.read_exact(&mut bytes)?;
    let mut token = String::with_capacity(32);
    use std::fmt::Write as _;
    for byte in bytes { write!(&mut token, "{byte:02x}")?; }
    Ok(token)
}
fn spawn(path: &Path, initial: DesktopSnapshot, theme: &str) -> Result<Child> {
    let bundle = Bundle::open(path)?;
    let commands = CommandSession::new(challenge()?)?;
    let process = Command::new(path).arg(format!("--theme={theme}"))
        .current_dir(path.parent().context("desktop has no bundle directory")?)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit())
        .process_group(0).spawn().context("start bundled Flutter desktop")?;
    let mut process = process;
    let (input, output) = match (process.stdin.take(), process.stdout.take()) {
        (Some(input), Some(output)) => (input, output),
        _ => {
            let _ = process.kill();
            let _ = process.wait();
            anyhow::bail!("owned Flutter private pipes missing");
        }
    };
    // From this point, every fallible step has the child's RAII owner.
    let mut child = Child {
        process, input: Some(input), output, outbox: Outbox::default(), lines: EventLines::default(),
        commands, preferences: PreferenceSession::default(), capability: PreferencesSnapshot::default(),
        queued: None, pending: None, changed: false, ready: false, started: Instant::now(), closing: None,
        _bundle: bundle,
        events: Vec::with_capacity(POLL_EVENTS),
    };
    nonblocking(child.input.as_ref().expect("input owned at spawn"))?;
    nonblocking(&child.output)?;
    child._bundle.verify(path, child.process.id())?;
    let frame = child.frame(initial)?;
    ensure!(frame.sequence > 0 && frame.sequence <= i64::MAX as u64, "invalid initial desktop sequence");
    child.outbox.replace(frame);
    Ok(child)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Limited { bytes: Vec<u8>, remaining: usize }
    impl Write for Limited {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.remaining == 0 { return Err(io::ErrorKind::WouldBlock.into()); }
            let count = bytes.len().min(self.remaining);
            self.bytes.extend_from_slice(&bytes[..count]); self.remaining -= count; Ok(count)
        }
        fn flush(&mut self) -> io::Result<()> { Ok(()) }
    }
    fn frame(sequence: u64, bytes: &[u8]) -> Frame { Frame { sequence, bytes: bytes.to_vec(), offset: 0 } }
    #[test]
    fn partial_frames_are_never_spliced_and_latest_is_bounded() {
        let now = Instant::now();
        let mut outbox = Outbox::default();
        let mut sink = Limited { bytes: Vec::new(), remaining: 2 };
        outbox.replace(frame(1, b"one\n")); outbox.pump(&mut sink, now).unwrap();
        assert_eq!(outbox.written, 0);
        outbox.replace(frame(2, b"two\n")); outbox.replace(frame(3, b"three\n"));
        outbox.replace(frame(2, b"stale\n"));
        sink.remaining = 64; outbox.pump(&mut sink, now).unwrap();
        assert_eq!(sink.bytes, b"one\nthree\n"); assert_eq!(outbox.written, 3);
    }
    #[test]
    fn stale_or_unwritten_frames_never_establish_freshness() {
        let now = Instant::now(); let mut outbox = Outbox::default();
        outbox.replace(frame(1, b"snapshot\n")); assert!(!outbox.fresh(now));
        let mut sink = Limited { bytes: Vec::new(), remaining: 64 };
        outbox.pump(&mut sink, now).unwrap(); assert!(outbox.fresh(now));
        assert!(!outbox.fresh(now + IO_TIMEOUT + Duration::from_millis(1)));
        assert!(!outbox.accepts(1)); assert!(!outbox.accepts(i64::MAX as u64 + 1));
    }
    #[test]
    fn blocked_owned_pipe_has_a_deadline() {
        let now = Instant::now(); let mut outbox = Outbox::default();
        outbox.replace(frame(1, b"snapshot\n"));
        let mut sink = Limited { bytes: Vec::new(), remaining: 0 };
        outbox.pump(&mut sink, now).unwrap();
        assert_eq!(outbox.pump(&mut sink, now + IO_TIMEOUT).unwrap_err().kind(), io::ErrorKind::TimedOut);
    }
    #[test]
    fn hardware_capability_cannot_be_enabled_by_the_presenter() {
        let capability = disabled_microphone();
        assert!(!capability.can_enable && !capability.can_release);
        assert!(capability.current_requested.is_none());
    }
    #[test]
    fn close_discards_partial_presentation_and_permanently_refuses_new_frames() {
        let now = Instant::now();
        let mut outbox = Outbox::default();
        let mut sink = Limited { bytes: Vec::new(), remaining: 2 };
        outbox.replace(frame(1, b"partial\n"));
        outbox.pump(&mut sink, now).unwrap();
        outbox.replace(frame(2, b"pending\n"));
        outbox.stop();
        outbox.replace(frame(3, b"late\n"));
        sink.remaining = 64;
        outbox.pump(&mut sink, now).unwrap();
        assert_eq!(sink.bytes, b"pa");
        assert_eq!(outbox.written, 0);
        assert!(!outbox.accepts(4));
        assert!(outbox.writing.is_none() && outbox.latest.is_none());
    }
    #[test]
    fn bundle_requires_single_link_owned_executable_and_no_symlink() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let directory = tempfile::Builder::new().prefix("mcw-flutter-owner-").tempdir_in("/tmp").unwrap();
        let root = directory.path().canonicalize().unwrap();
        let executable = root.join("runner");
        std::fs::write(&executable, b"fixture; never executed").unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert!(Bundle::open(&executable).is_err());
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert!(Bundle::open(&executable).is_ok());
        let alias = root.join("alias");
        symlink(&executable, &alias).unwrap();
        assert!(Bundle::open(&alias).is_err());
        let link = root.join("hardlink");
        std::fs::hard_link(&executable, &link).unwrap();
        assert!(Bundle::open(&executable).is_err());
        std::fs::remove_file(link).unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o722)).unwrap();
        assert!(Bundle::open(&executable).is_err());
    }
    #[test]
    fn preference_challenge_reconnect_and_replay_are_not_transferable() {
        use super::super::preferences::{PreferenceAction, PreferenceEffects, PreferenceKey, PreferenceValue};
        let old = "0123456789abcdef0123456789abcdef";
        let new = "fedcba9876543210fedcba9876543210";
        let settings = crate::settings::Settings::default();
        let view = PreferencesSnapshot::observed(Ok(&settings), true, true, None, PreferenceEffects::default());
        let request = PreferenceRequest {
            session: old.into(), id: 1, action: PreferenceAction::SetPreference,
            observed_sequence: 1, key: PreferenceKey::HistoryEnabled,
            value: PreferenceValue::Bool(!settings.history_enabled),
            expected_value: PreferenceValue::Bool(settings.history_enabled),
        };
        let mut lane = PreferenceSession::default();
        assert!(lane.admit(request.clone(), new, 1, true, &view).is_none());
        assert!(lane.admit(request.clone(), old, 1, true, &view).is_some());
        assert!(lane.admit(request.clone(), old, 1, true, &view).is_none());
        assert!(lane.pending());
        assert!(lane.finish(1, CommandState::Succeeded, None, None));
        assert!(lane.admit(request, old, 1, true, &view).is_none());
    }
}
