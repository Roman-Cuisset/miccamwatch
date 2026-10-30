//! Owned Unix Flutter child. No listener, public endpoint, shell, or hardware command worker.
//! Nonblocking private pipes are pumped by the native owner; memory and per-poll work are bounded.
use super::{
    DesktopSnapshot, EventLines, UiEvent,
    commands::{CommandSession, CommandState, MicrophoneCapability},
    preferences::{PreferenceRequest, PreferenceSession, PreferencesSnapshot},
    consultation,
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

/// Pin every directory by walking relative to the preceding opened directory.
/// A root-owned sticky common ancestor (for example /tmp) cannot be used by
/// another user to replace a child owned by this account.
pub(crate) fn pin_directory_chain(path: &Path) -> Result<Vec<File>> {
    use std::{ffi::CString, os::unix::{ffi::OsStrExt, io::FromRawFd}, path::Component};
    ensure!(path.is_absolute(), "directory must be absolute");
    let mut pins: Vec<File> = Vec::new();
    for component in path.components() {
        let name = match component {
            Component::RootDir => std::ffi::OsStr::new("/"),
            Component::Normal(name) => name,
            _ => anyhow::bail!("directory chain must not contain relative components"),
        };
        let name = CString::new(name.as_bytes())?;
        let parent = pins.last().map_or(libc::AT_FDCWD, AsRawFd::as_raw_fd);
        let descriptor = unsafe {
            libc::openat(parent, name.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY
                | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        };
        ensure!(descriptor >= 0, "open trusted directory {}: {}", path.display(), io::Error::last_os_error());
        let directory = unsafe { File::from_raw_fd(descriptor) };
        let metadata = directory.metadata()?;
        let root_sticky = metadata.uid() == 0 && metadata.mode() & 0o1000 != 0;
        ensure!(trusted_owner(&metadata) && (metadata.mode() & 0o022 == 0 || root_sticky),
            "unsafe directory ownership or permissions: {}", path.display());
        #[cfg(target_os = "macos")]
        reject_mutating_acl(&directory)?;
        pins.push(directory);
    }
    Ok(pins)
}

fn trusted_owner(metadata: &std::fs::Metadata) -> bool {
    metadata.uid() == 0 || metadata.uid() == unsafe { libc::geteuid() }
}

/// Darwin ACL grants are independent of the POSIX mode bits. Keep harmless
/// read/execute grants and ordinary deny-delete home ACLs, but fail closed on
/// any explicit grant capable of changing a pinned object or its children.
#[cfg(target_os = "macos")]
fn reject_mutating_acl(file: &File) -> Result<()> {
    use std::ffi::c_void;
    unsafe extern "C" {
        fn acl_get_fd_np(fd: libc::c_int, kind: libc::c_int) -> *mut c_void;
        fn acl_get_entry(acl: *mut c_void, index: libc::c_int, entry: *mut *mut c_void) -> libc::c_int;
        fn acl_get_tag_type(entry: *mut c_void, tag: *mut libc::c_int) -> libc::c_int;
        fn acl_get_permset_mask_np(entry: *mut c_void, mask: *mut u64) -> libc::c_int;
        fn acl_free(acl: *mut c_void) -> libc::c_int;
    }
    struct Acl(*mut c_void);
    impl Drop for Acl {
        fn drop(&mut self) { unsafe { acl_free(self.0); } }
    }
    // Constants and ABI from Darwin sys/acl.h (ACL_TYPE_EXTENDED).
    let raw = unsafe { acl_get_fd_np(file.as_raw_fd(), 0x100) };
    if raw.is_null() {
        let error = io::Error::last_os_error();
        // On an already-open descriptor, Darwin reports a missing FILESEC_ACL
        // property as ENOENT; there is no pathname lookup to fail here.
        if matches!(error.raw_os_error(), Some(libc::ENOENT) | Some(libc::EOPNOTSUPP)) {
            return Ok(());
        }
        return Err(error).context("read desktop ownership ACL");
    }
    let acl = Acl(raw);
    let mut index = 0; // ACL_FIRST_ENTRY; Darwin uses -1 for ACL_NEXT_ENTRY.
    loop {
        let mut entry = std::ptr::null_mut();
        if unsafe { acl_get_entry(acl.0, index, &mut entry) } != 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::EINVAL) { break; }
            return Err(error).context("enumerate desktop ownership ACL");
        }
        index = -1;
        let mut tag = 0;
        let mut mask = 0u64;
        ensure!(unsafe { acl_get_tag_type(entry, &mut tag) } == 0
            && unsafe { acl_get_permset_mask_np(entry, &mut mask) } == 0,
            "cannot inspect desktop ownership ACL");
        const MUTATING: u64 = (1 << 2) | (1 << 4) | (1 << 5) | (1 << 6)
            | (1 << 8) | (1 << 10) | (1 << 12) | (1 << 13);
        ensure!(tag != 1 || mask & MUTATING == 0, "unsafe desktop ownership ACL grant");
    }
    Ok(())
}

struct PayloadPin {
    path: PathBuf,
    file: File,
}
/// Validate the entire runtime, not only the launcher. The retained directory
/// and file handles identify everything that may consume the private challenge.
struct Bundle {
    _ancestors: Vec<File>,
    payload: Vec<PayloadPin>,
    executable: File,
}
impl Bundle {
    fn open(path: &Path) -> Result<Self> {
        ensure!(path.is_absolute(), "desktop executable must be absolute");
        let parent = path.parent().context("desktop has no directory")?;
        let ancestors = pin_directory_chain(parent)?;
        #[cfg(target_os = "linux")]
        let root = parent.to_path_buf();
        #[cfg(target_os = "macos")]
        let root = {
            ensure!(parent.file_name().is_some_and(|name| name == "MacOS"),
                "desktop executable is not in an App bundle");
            let contents = parent.parent().context("desktop has no Contents directory")?;
            ensure!(contents.file_name().is_some_and(|name| name == "Contents"),
                "desktop executable is not in an App bundle");
            let app = contents.parent().context("desktop has no App bundle")?;
            ensure!(app.extension().is_some_and(|extension| extension == "app"),
                "desktop executable is not in an App bundle");
            app.to_path_buf()
        };
        let executable = Self::open_file(path, true)?;
        let mut payload = Vec::new();
        Self::pin_tree(&root, &root, &mut payload)?;
        #[cfg(target_os = "linux")]
        for relative in ["lib/libflutter_linux_gtk.so", "lib/libapp.so"] {
            let library = root.join(relative);
            ensure!(library.canonicalize()?.starts_with(&root),
                "Flutter runtime library escapes bundle");
            ensure!(std::fs::metadata(&library)?.is_file(), "missing Flutter runtime library");
        }
        Ok(Self { _ancestors: ancestors, payload, executable })
    }
    fn open_file(path: &Path, executable: bool) -> Result<File> {
        let file = OpenOptions::new().read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK).open(path)
            .with_context(|| format!("open Flutter payload {}", path.display()))?;
        let metadata = file.metadata()?;
        ensure!(metadata.is_file() && metadata.nlink() == 1 && metadata.mode() & 0o022 == 0
            && trusted_owner(&metadata) && (!executable || metadata.mode() & 0o111 != 0),
            "unsafe desktop payload ownership, links or permissions: {}", path.display());
        #[cfg(target_os = "macos")]
        reject_mutating_acl(&file)?;
        Ok(file)
    }
    fn pin_tree(root: &Path, path: &Path, pins: &mut Vec<PayloadPin>) -> Result<()> {
        let metadata = std::fs::symlink_metadata(path)?;
        if metadata.file_type().is_symlink() {
            ensure!(trusted_owner(&metadata), "unsafe desktop symlink owner");
            let target = std::fs::read_link(path)?;
            let (mut normalized, relative) = if target.is_absolute() {
                (root.to_path_buf(), target.strip_prefix(root)
                    .context("absolute desktop payload symlink leaves bundle")?)
            } else {
                (path.parent().context("payload link has no parent")?.to_path_buf(), target.as_path())
            };
            for component in relative.components() {
                match component {
                    std::path::Component::ParentDir => { normalized.pop(); }
                    std::path::Component::CurDir => {}
                    component => normalized.push(component),
                }
                ensure!(normalized.starts_with(root),
                    "desktop payload symlink leaves bundle: {}", path.display());
            }
            // Standard macOS frameworks use Versions/Current and top-level
            // aliases. Keep those layouts, but never follow a link outside .app.
            let resolved = path.canonicalize()?;
            ensure!(resolved.starts_with(root), "desktop payload symlink escapes bundle: {}", path.display());
            return Ok(());
        }
        if metadata.is_dir() {
            let directory = OpenOptions::new().read(true)
                .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC).open(path)?;
            let opened = directory.metadata()?;
            ensure!(opened.dev() == metadata.dev() && opened.ino() == metadata.ino()
                && trusted_owner(&opened) && opened.mode() & 0o022 == 0,
                "unsafe desktop payload directory: {}", path.display());
            #[cfg(target_os = "macos")]
            reject_mutating_acl(&directory)?;
            pins.push(PayloadPin { path: path.to_path_buf(), file: directory });
            for entry in std::fs::read_dir(path)? {
                Self::pin_tree(root, &entry?.path(), pins)?;
            }
        } else {
            pins.push(PayloadPin { path: path.to_path_buf(), file: Self::open_file(path, false)? });
        }
        Ok(())
    }
    fn verify(&self, path: &Path, pid: u32) -> Result<()> {
        for pin in &self.payload {
            let expected = pin.file.metadata()?;
            let actual = std::fs::symlink_metadata(&pin.path)?;
            ensure!(expected.dev() == actual.dev() && expected.ino() == actual.ino()
                && !actual.file_type().is_symlink() && trusted_owner(&actual)
                && actual.mode() & 0o022 == 0, "desktop payload changed during spawn");
        }
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

/// The unreaped leader reserves the PID/PGID until the final group signal.
/// Neither normal exit observation nor a failed opening may release it early.
struct OwnedProcess {
    process: Process,
    owns_pid: bool,
}
impl OwnedProcess {
    fn new(process: Process) -> Self { Self { process, owns_pid: true } }
    fn id(&self) -> u32 { self.process.id() }
    fn completion(&mut self) -> io::Result<Option<std::process::ExitStatus>> {
        use std::os::unix::process::ExitStatusExt;
        if !self.owns_pid { return Err(io::Error::from_raw_os_error(libc::ECHILD)); }
        let mut info = unsafe { std::mem::zeroed::<libc::siginfo_t>() };
        let result = unsafe {
            libc::waitid(libc::P_PID, self.id(), &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT)
        };
        if result == -1 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::ECHILD) { self.owns_pid = false; }
            if error.kind() == io::ErrorKind::Interrupted { return Ok(None); }
            return Err(error);
        }
        if unsafe { info.si_pid() } == 0 { return Ok(None); }
        let code = unsafe { info.si_status() };
        let raw = match info.si_code {
            libc::CLD_EXITED => code << 8,
            libc::CLD_KILLED => code,
            libc::CLD_DUMPED => code | 0x80,
            _ => return Err(io::Error::other("unexpected Flutter waitid completion state")),
        };
        Ok(Some(std::process::ExitStatus::from_raw(raw)))
    }
    fn cleanup(&mut self) {
        // Detect an external reaper before touching either numeric ID.
        if !self.owns_pid { return; }
        let _ = self.completion();
        if !self.owns_pid { return; }
        unsafe { libc::kill(-(self.id() as i32), libc::SIGKILL); }
        let _ = self.process.kill();
        let waited = self.process.wait();
        if waited.is_ok() || waited.as_ref().err().is_some_and(|error| error.raw_os_error() == Some(libc::ECHILD)) {
            self.owns_pid = false;
        }
    }
}
impl Drop for OwnedProcess {
    fn drop(&mut self) { self.cleanup(); }
}

pub struct AuthorizedPreference {
    pub ticket: u64,
    pub request: PreferenceRequest,
}
struct Child {
    process: OwnedProcess,
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
    consultation: consultation::Session,
    queued_consultation: Option<(u64, consultation::Request)>,
    pending_consultation: Option<(u64, u64)>,
    consultation_available: bool,
    autostart_writable: bool,
    changed: bool,
    presentation_changed: bool,
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
        self.autostart_writable = snapshot.engine.mode == "operational";
        snapshot.consultation = self.consultation.view(self.consultation_available);
        Ok(Frame { sequence: snapshot.sequence, bytes: snapshot.encode()?, offset: 0 })
    }
    fn live_ready(&self, now: Instant) -> bool {
        self.ready && self.closing.is_none() && self.outbox.fresh(now)
    }
    fn event(&mut self, event: UiEvent, next_ticket: &mut u64, now: Instant) {
        match event {
            UiEvent::Ready { sequence, .. } => {
                if sequence > 0 && sequence <= self.outbox.written && self.closing.is_none() {
                    if !self.ready {
                        eprintln!("desktop event=ready endpoint=parent_received pid={} sequence={sequence}",
                            self.process.id());
                    }
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
            UiEvent::ConsultationRequest { request, .. } => {
                let ready = self.live_ready(now) && self.consultation_available;
                if let Some(request) = self.consultation.admit(request, self.commands.session(),
                    self.outbox.written, ready, false, self.autostart_writable) {
                    if let Some(ticket) = next_ticket.checked_add(1) {
                        *next_ticket = ticket;
                        self.pending_consultation = Some((ticket, request.id));
                        self.queued_consultation = Some((ticket, request));
                    } else {
                        self.consultation.finish(request.id, Err("Consultation tickets exhausted".into()));
                    }
                }
                self.presentation_changed = true;
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
    fn failed_opening(&self) -> bool { !self.ready && self.closing.is_none() }
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
    fn drop(&mut self) { self.begin_close(Instant::now()); }
}

/// Lazy process owner. Singleton opens reuse the one owned child and challenge.
/// No UI-close can cancel work already taken by the process-owned preferences worker.
pub struct Supervisor {
    executable: PathBuf,
    child: Option<Child>,
    next_ticket: u64,
    error: Option<String>,
    open_failure: Option<String>,
    consultation_worker: Option<consultation::Worker>,
}
impl Supervisor {
    pub fn new() -> Result<Self> {
        let current = std::env::current_exe()?.canonicalize()?;
        let root = current.parent().context("native executable has no parent")?.join("desktop");
        #[cfg(target_os = "linux")]
        let executable = root.join("miccamwatch_desktop");
        #[cfg(target_os = "macos")]
        let executable = root.join("MicCamWatch.app/Contents/MacOS/MicCamWatch");
        Ok(Self { executable, child: None, next_ticket: 0, error: None, open_failure: None,
            consultation_worker: None })
    }
    pub fn is_running(&self) -> bool { self.child.is_some() }
    pub fn open(&mut self, initial: DesktopSnapshot, theme: &str) -> Result<()> {
        ensure!(matches!(theme, "system" | "light" | "dark"), "unsupported desktop theme");
        self.poll();
        if self.child.as_ref().is_some_and(|child| child.closing.is_some()) {
            self.child.take();
            consultation::Worker::reset(&mut self.consultation_worker);
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
        self.open_failure = None;
        Ok(())
    }
    pub fn publish(&mut self, snapshot: DesktopSnapshot) -> Result<()> {
        if let Some(child) = self.child.as_mut()
            && child.closing.is_none() && child.outbox.accepts(snapshot.sequence) {
            let frame = child.frame(snapshot)?;
            child.outbox.replace(frame);
            child.changed = false;
            child.presentation_changed = false;
        }
        Ok(())
    }
    pub fn poll(&mut self) {
        if let Some(completion) = consultation::Worker::try_complete(&mut self.consultation_worker)
            && let Some(child) = self.child.as_mut()
            && let Some((ticket, id)) = child.pending_consultation
            && ticket == completion.ticket
        {
            child.pending_consultation = None;
            child.consultation.finish(id, completion.result);
            child.presentation_changed = true;
        }
        let now = Instant::now();
        let Some(child) = self.child.as_mut() else { return };
        let available = consultation::Worker::available(&self.consultation_worker);
        child.presentation_changed |= child.consultation_available != available;
        child.consultation_available = available;
        let exited = match child.process.completion() {
            Ok(Some(status)) => {
                let failure = format!("Flutter child exited before readiness: {status}");
                if child.failed_opening() { self.open_failure = Some(failure); }
                if !status.success() { self.error = Some(format!("Flutter child exited: {status}")); }
                true
            }
            Ok(None) => false,
            Err(error) => {
                let failure = format!("observe owned Flutter child: {error}");
                if child.failed_opening() { self.open_failure = Some(failure.clone()); }
                self.error = Some(failure);
                child.begin_close(now);
                false
            }
        };
        if !exited && child.closing.is_none()
            && let Err(error) = child.pump(&mut self.next_ticket, now) {
            let failure = format!("{error:#}");
            if child.failed_opening() { self.open_failure = Some(failure.clone()); }
            self.error = Some(failure);
            child.begin_close(now);
        }
        if let Some((ticket, request)) = child.queued_consultation.take() {
            let id = request.id;
            if let Err(error) = consultation::Worker::enqueue(&mut self.consultation_worker, ticket, request, child.autostart_writable) {
                child.pending_consultation = None;
                child.consultation.finish(id, Err(format!("{error:#}")));
                child.presentation_changed = true;
            }
        }
        if exited || child.closing.is_some_and(|at| now.saturating_duration_since(at) >= CLOSE_GRACE) {
            self.child.take();
            consultation::Worker::reset(&mut self.consultation_worker);
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
    pub fn take_presentation_changed(&mut self) -> bool {
        self.child.as_mut().is_some_and(|child| std::mem::take(&mut child.presentation_changed))
    }
    pub fn take_error(&mut self) -> Option<String> { self.error.take() }
    /// A requested opening failed before ready; explicit closes are not failures.
    pub fn take_open_failure(&mut self) -> Option<String> { self.open_failure.take() }
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
    let mut command = Command::new(path);
    command.arg(format!("--theme={theme}"))
        .current_dir(path.parent().context("desktop has no bundle directory")?)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit())
        .process_group(0);
    #[cfg(target_os = "linux")]
    {
        // Capture before fork, not inside the child: death between fork and
        // prctl must fail closed. Only async-signal-safe libc calls run here.
        let expected_parent = unsafe { libc::getpid() };
        unsafe {
            command.pre_exec(move || {
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) == -1 {
                    return Err(io::Error::last_os_error());
                }
                if libc::getppid() != expected_parent {
                    return Err(io::Error::from_raw_os_error(libc::ESRCH));
                }
                Ok(())
            });
        }
    }
    #[cfg(target_os = "macos")]
    command.env("MCW_DESKTOP_OWNER_PID", unsafe { libc::getpid() }.to_string());
    let mut process = OwnedProcess::new(command.spawn().context("start bundled Flutter desktop")?);
    let (input, output) = match (process.process.stdin.take(), process.process.stdout.take()) {
        (Some(input), Some(output)) => (input, output),
        _ => anyhow::bail!("owned Flutter private pipes missing"),
    };
    // From this point, every fallible step has the child's RAII owner.
    let mut child = Child {
        process, input: Some(input), output, outbox: Outbox::default(), lines: EventLines::default(),
        commands, preferences: PreferenceSession::default(), capability: PreferencesSnapshot::default(),
        queued: None, pending: None, changed: false, presentation_changed: false,
        ready: false, started: Instant::now(), closing: None,
        consultation: consultation::Session::default(), queued_consultation: None,
        pending_consultation: None, consultation_available: true,
        autostart_writable: false,
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
    fn private_fixture_dirs(root: &Path, path: &Path) {
        use std::os::unix::fs::PermissionsExt;
        std::fs::create_dir_all(path).unwrap();
        let mut directory = root.to_path_buf();
        for component in path.strip_prefix(root).unwrap().components() {
            directory.push(component);
            std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
    }
    fn bundle_fixture() -> (tempfile::TempDir, PathBuf, PathBuf) {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::Builder::new().prefix("mcw-flutter-owner-").tempdir_in("/tmp").unwrap();
        let base = directory.path().canonicalize().unwrap();
        #[cfg(target_os = "linux")]
        let (root, executable) = (base.clone(), base.join("runner"));
        #[cfg(target_os = "macos")]
        let (root, executable) = {
            let root = base.join("Fixture.app");
            (root.clone(), root.join("Contents/MacOS/runner"))
        };
        private_fixture_dirs(&base, executable.parent().unwrap());
        #[cfg(target_os = "linux")]
        {
            private_fixture_dirs(&root, &root.join("lib"));
            for library in ["libflutter_linux_gtk.so", "libapp.so"] {
                std::fs::write(root.join("lib").join(library), b"fixture library").unwrap();
                std::fs::set_permissions(root.join("lib").join(library),
                    std::fs::Permissions::from_mode(0o600)).unwrap();
            }
        }
        std::fs::write(&executable, b"fixture; never executed").unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
        (directory, root, executable)
    }
    #[test]
    fn bundle_requires_single_link_owned_executable_and_no_symlink() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let (_directory, root, executable) = bundle_fixture();
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
    fn entire_runtime_rejects_mutable_payload_and_external_links() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let (_directory, root, executable) = bundle_fixture();
        let resources = root.join("resources");
        private_fixture_dirs(&root, &resources);
        let payload = resources.join("runtime");
        std::fs::write(&payload, b"code or resources").unwrap();
        std::fs::set_permissions(&payload, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert!(Bundle::open(&executable).is_ok());
        std::fs::set_permissions(&payload, std::fs::Permissions::from_mode(0o622)).unwrap();
        assert!(Bundle::open(&executable).is_err());
        std::fs::set_permissions(&payload, std::fs::Permissions::from_mode(0o600)).unwrap();
        std::fs::set_permissions(&resources, std::fs::Permissions::from_mode(0o722)).unwrap();
        assert!(Bundle::open(&executable).is_err());
        std::fs::set_permissions(&resources, std::fs::Permissions::from_mode(0o700)).unwrap();
        let outside = tempfile::tempdir_in("/tmp").unwrap();
        symlink(outside.path(), resources.join("external")).unwrap();
        assert!(Bundle::open(&executable).is_err());
    }
    #[test]
    fn framework_version_links_stay_inside_bundle() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let (_directory, root, executable) = bundle_fixture();
        let framework = root.join("Contents/Frameworks/Fixture.framework");
        private_fixture_dirs(&root, &framework.join("Versions/A/Resources"));
        std::fs::write(framework.join("Versions/A/Fixture"), b"framework").unwrap();
        std::fs::set_permissions(framework.join("Versions/A/Fixture"),
            std::fs::Permissions::from_mode(0o600)).unwrap();
        symlink("A", framework.join("Versions/Current")).unwrap();
        symlink("Versions/Current/Fixture", framework.join("Fixture")).unwrap();
        symlink("Versions/Current/Resources", framework.join("Resources")).unwrap();
        assert!(Bundle::open(&executable).is_ok());
        let outside = tempfile::tempdir_in("/tmp").unwrap();
        // Even an external alias that resolves back inside is not a trusted route.
        symlink(&framework, outside.path().join("back")).unwrap();
        symlink(outside.path().join("back/Fixture"), framework.join("external-hop")).unwrap();
        assert!(Bundle::open(&executable).is_err());
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn runtime_engine_and_aot_library_are_required() {
        let (_directory, root, executable) = bundle_fixture();
        std::fs::remove_file(root.join("lib/libapp.so")).unwrap();
        assert!(Bundle::open(&executable).is_err());
    }
    #[test]
    fn directory_chain_rejects_mutable_ancestors_and_symlinks() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let directory = tempfile::tempdir_in("/tmp").unwrap();
        let root = directory.path().canonicalize().unwrap();
        let ancestor = root.join("ancestor");
        let leaf = ancestor.join("private");
        private_fixture_dirs(&root, &leaf);
        std::fs::set_permissions(&leaf, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert!(pin_directory_chain(&leaf).is_ok());
        std::fs::set_permissions(&ancestor, std::fs::Permissions::from_mode(0o722)).unwrap();
        assert!(pin_directory_chain(&leaf).is_err());
        std::fs::set_permissions(&ancestor, std::fs::Permissions::from_mode(0o700)).unwrap();
        let alias = root.join("alias");
        symlink(&ancestor, &alias).unwrap();
        assert!(pin_directory_chain(&alias.join("private")).is_err());
        assert!(pin_directory_chain(&ancestor.join("../ancestor/private")).is_err());
    }
    fn await_completion(process: &mut OwnedProcess) -> std::process::ExitStatus {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = process.completion().unwrap() { return status; }
            assert!(Instant::now() < deadline, "fixture child did not exit");
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    #[test]
    fn exited_leader_remains_waitable_until_group_cleanup() {
        let mut process = OwnedProcess::new(Command::new("/bin/sh")
            .args(["-c", "sleep 30 & exit 0"]).process_group(0).stdout(Stdio::piped()).spawn().unwrap());
        let mut output = process.process.stdout.take().unwrap();
        nonblocking(&output).unwrap();
        assert!(await_completion(&mut process).success());
        assert!(process.completion().unwrap().unwrap().success());
        assert!(process.owns_pid);
        process.cleanup();
        assert!(!process.owns_pid);
        // The descendant inherited stdout. EOF proves cleanup did not merely
        // reap the already-exited shell and leave the group member running.
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match output.read(&mut [0u8; 1]) {
                Ok(0) => break,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                result => panic!("unexpected fixture output: {result:?}"),
            }
            assert!(Instant::now() < deadline, "descendant survived owned group cleanup");
            std::thread::sleep(Duration::from_millis(5));
        }
        process.cleanup(); // Idempotent: never signal the released group ID.
    }
    #[test]
    fn external_reap_revokes_numeric_signal_ownership() {
        let mut process = OwnedProcess::new(Command::new("/bin/sh")
            .args(["-c", "exit 0"]).process_group(0).spawn().unwrap());
        assert!(process.process.wait().unwrap().success());
        assert_eq!(process.completion().unwrap_err().raw_os_error(), Some(libc::ECHILD));
        assert!(!process.owns_pid);
        process.cleanup();
    }
    // Lifecycle fixtures never execute a Flutter bundle or touch a device.
    fn opening_fixture(script: &str) -> (tempfile::TempDir, Supervisor) {
        let (directory, _root, executable) = bundle_fixture();
        let bundle = Bundle::open(&executable).unwrap();
        let mut process = OwnedProcess::new(Command::new("/bin/sh")
            .args(["-c", script]).process_group(0).stdin(Stdio::piped())
            .stdout(Stdio::piped()).spawn().unwrap());
        let input = process.process.stdin.take().unwrap();
        let output = process.process.stdout.take().unwrap();
        nonblocking(&input).unwrap();
        nonblocking(&output).unwrap();
        let child = Child {
            process, input: Some(input), output, outbox: Outbox::default(),
            lines: EventLines::default(), events: Vec::new(),
            commands: CommandSession::new("0123456789abcdef0123456789abcdef".into()).unwrap(),
            preferences: PreferenceSession::default(), capability: PreferencesSnapshot::default(),
            queued: None, pending: None, changed: false, presentation_changed: false, ready: false,
            started: Instant::now(), closing: None, _bundle: bundle,
            consultation: consultation::Session::default(), queued_consultation: None,
            pending_consultation: None, consultation_available: true, autostart_writable: false,
        };
        (directory, Supervisor { executable, child: Some(child), next_ticket: 0,
            error: None, open_failure: None, consultation_worker: None })
    }
    #[test]
    fn before_ready_exit_is_open_failure_but_ready_exit_and_explicit_close_are_not() {
        for (ready, closing, expected) in [(false, false, true), (true, false, false), (false, true, false)] {
            let (_directory, mut supervisor) = opening_fixture("exit 0");
            let child = supervisor.child.as_mut().unwrap();
            child.ready = ready;
            await_completion(&mut child.process);
            if closing { supervisor.close().unwrap(); }
            supervisor.poll();
            assert_eq!(supervisor.take_open_failure().is_some(), expected);
            assert!(supervisor.take_open_failure().is_none());
            assert!(!supervisor.is_running());
        }
    }
    #[test]
    fn readiness_timeout_and_event_eof_report_failed_opening() {
        let (_directory, mut supervisor) = opening_fixture("sleep 30");
        supervisor.child.as_mut().unwrap().started = Instant::now() - READY_TIMEOUT;
        supervisor.poll();
        assert!(supervisor.take_open_failure().unwrap().contains("readiness timed out"));
        drop(supervisor);
        let (_directory, mut supervisor) = opening_fixture("exec 1>&-; sleep 30");
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            supervisor.poll();
            if let Some(failure) = supervisor.take_open_failure() {
                assert!(failure.contains("closed its event pipe"));
                break;
            }
            assert!(Instant::now() < deadline, "event EOF was not observed");
            std::thread::sleep(Duration::from_millis(5));
        }
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
