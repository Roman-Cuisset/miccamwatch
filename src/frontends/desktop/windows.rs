//! Disposable owned UI, bounded typed commands, and nonblocking presentation I/O.
use super::{
    DesktopSnapshot, EventLines, UiEvent,
    commands::{CommandSession, CommandState, MicrophoneCapability},
};
use anyhow::{Context, Result, ensure};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    mem::{size_of, size_of_val},
    os::windows::{
        ffi::{OsStrExt, OsStringExt},
        fs::{MetadataExt, OpenOptionsExt},
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
    },
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    time::{Duration, Instant},
};
use windows::{
    Win32::{
        Foundation::{
            HANDLE, HANDLE_FLAG_INHERIT, HANDLE_FLAGS, HWND, LPARAM, SetHandleInformation,
            WAIT_OBJECT_0, WPARAM,
        },
        Security::{
            Cryptography::{BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom},
            SECURITY_ATTRIBUTES,
        },
        Storage::FileSystem::{BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle},
        System::{
            JobObjects::{
                AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
                SetInformationJobObject,
            },
            Pipes::CreatePipe,
            Threading::{
                CREATE_SUSPENDED, CreateProcessW, DeleteProcThreadAttributeList,
                EXTENDED_STARTUPINFO_PRESENT, InitializeProcThreadAttributeList,
                LPPROC_THREAD_ATTRIBUTE_LIST, PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
                PROCESS_INFORMATION, PROCESS_NAME_WIN32, QueryFullProcessImageNameW, ResumeThread,
                STARTF_USESTDHANDLES, STARTUPINFOEXW, TerminateProcess, UpdateProcThreadAttribute,
                WaitForSingleObject,
            },
        },
        UI::WindowsAndMessaging::{
            EnumWindows, GetWindowTextW, GetWindowThreadProcessId, PostMessageW, SW_RESTORE,
            SetForegroundWindow, ShowWindowAsync, WM_CLOSE,
        },
    },
    core::{BOOL, PCWSTR, PWSTR},
};

const EVENT_CAPACITY: usize = 64;
const CLOSE_GRACE: Duration = Duration::from_secs(2);
fn handle(value: &impl AsRawHandle) -> HANDLE {
    HANDLE(value.as_raw_handle())
}
unsafe fn own(value: HANDLE) -> OwnedHandle {
    unsafe { OwnedHandle::from_raw_handle(value.0) }
}
fn wide(value: &std::ffi::OsStr) -> Vec<u16> {
    value.encode_wide().chain(Some(0)).collect()
}

struct Frame {
    sequence: u64,
    bytes: Vec<u8>,
}
#[derive(Default)]
struct Pending {
    latest: Option<Frame>,
    highest_sequence: Option<u64>,
    stopped: bool,
}
#[derive(Default)]
struct Mailbox {
    pending: Mutex<Pending>,
    wake: Condvar,
}
impl Mailbox {
    fn accepts(&self, sequence: u64) -> bool {
        let pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        !pending.stopped
            && pending
                .highest_sequence
                .is_none_or(|previous| sequence > previous)
    }
    fn replace(&self, frame: Frame) {
        let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        if !pending.stopped
            && pending
                .highest_sequence
                .is_none_or(|previous| frame.sequence > previous)
        {
            pending.highest_sequence = Some(frame.sequence);
            pending.latest = Some(frame);
            self.wake.notify_one();
        }
    }
    fn take(&self) -> Option<Frame> {
        let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        while pending.latest.is_none() && !pending.stopped {
            pending = self.wake.wait(pending).unwrap_or_else(|e| e.into_inner());
        }
        if pending.stopped {
            None
        } else {
            pending.latest.take()
        }
    }
    fn stop(&self) {
        let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        pending.stopped = true;
        pending.latest = None;
        self.wake.notify_all();
    }
}
struct Received {
    event: UiEvent,
    at: Instant,
}
#[derive(Default)]
struct Signals {
    disconnected: AtomicBool,
    rejected: AtomicUsize,
    dropped: AtomicUsize,
    written_sequence: AtomicU64,
    wrote_frame: AtomicBool,
}
struct Child {
    process: OwnedHandle,
    job: Option<OwnedHandle>,
    pid: u32,
    mailbox: Arc<Mailbox>,
    signals: Arc<Signals>,
    events: Receiver<Received>,
    closing: Option<Instant>,
    started: Instant,
    ready: bool,
    commands: CommandSession,
    capability: MicrophoneCapability,
    queued_command: Option<AuthorizedCommand>,
    pending_ticket: Option<(u64, u64)>,
    controls_changed: bool,
    last_frame_at: Instant,
    // Pin all path components and the single-link executable for the child's lifetime.
    _bundle: Bundle,
}
impl Child {
    fn frame(&mut self, snapshot: &mut DesktopSnapshot) -> Result<Frame> {
        self.capability = std::mem::take(&mut snapshot.controls.microphone);
        snapshot.controls = self.commands.controls(self.capability.clone());
        let frame = Frame {
            sequence: snapshot.sequence,
            bytes: snapshot.encode()?,
        };
        self.last_frame_at = Instant::now();
        self.controls_changed = false;
        Ok(frame)
    }
    fn exited(&self) -> bool {
        unsafe { WaitForSingleObject(handle(&self.process), 0) == WAIT_OBJECT_0 }
    }
    fn terminate(&mut self) {
        self.mailbox.stop();
        // Closing the sole job handle kills the entire owned tree, releasing blocked I/O.
        self.job.take();
    }
}
impl Drop for Child {
    fn drop(&mut self) {
        self.terminate();
    }
}

#[derive(Clone, Copy, Debug)]
pub struct AuthorizedCommand {
    pub ticket: u64,
    pub requested: bool,
    pub expected_requested: bool,
    pub expected_generation: u64,
}

fn new_command_session() -> Result<CommandSession> {
    let mut random = [0u8; 16];
    unsafe { BCryptGenRandom(None, &mut random, BCRYPT_USE_SYSTEM_PREFERRED_RNG) }.ok()?;
    let mut token = String::with_capacity(32);
    use std::fmt::Write as _;
    for byte in random {
        write!(&mut token, "{byte:02x}")?;
    }
    CommandSession::new(token)
}

/// No pipes, threads, job, file reservations or child exist until `open`.
pub struct Supervisor {
    executable: PathBuf,
    child: Option<Child>,
    reopen: Option<(DesktopSnapshot, &'static str)>,
    next_ticket: u64,
}
impl Supervisor {
    pub fn new() -> Result<Self> {
        let current = std::env::current_exe().context("locate desktop bundle")?;
        Ok(Self {
            executable: current
                .parent()
                .context("executable has no parent")?
                .join("desktop")
                .join("miccamwatch_desktop.exe"),
            child: None,
            reopen: None,
            next_ticket: 0,
        })
    }
    pub fn is_running(&self) -> bool {
        self.child.as_ref().is_some_and(|child| !child.exited())
    }
    pub fn open(&mut self, mut initial: DesktopSnapshot, theme: &str) -> Result<()> {
        let theme = theme_value(theme)?;
        self.poll();
        if let Some(child) = self.child.as_mut() {
            let window = owned_window(child.pid);
            if needs_reopen(
                child.closing.is_some(),
                child.signals.disconnected.load(Ordering::Acquire),
                child.exited(),
                child.ready,
                window.is_some(),
            ) {
                child.closing.get_or_insert_with(Instant::now);
                self.reopen = Some((initial, theme));
            } else {
                if child.mailbox.accepts(initial.sequence) {
                    let frame = child.frame(&mut initial)?;
                    child.mailbox.replace(frame);
                }
                if let Some(window) = window {
                    unsafe {
                        let _ = ShowWindowAsync(window, SW_RESTORE);
                        let _ = SetForegroundWindow(window);
                    }
                }
                if child.exited() || child.signals.disconnected.load(Ordering::Acquire) {
                    child.closing.get_or_insert_with(Instant::now);
                    self.reopen = Some((initial, theme));
                }
            }
            return Ok(());
        }
        self.child = Some(spawn(&self.executable, initial, theme)?);
        Ok(())
    }
    pub fn publish(&mut self, mut snapshot: DesktopSnapshot) -> Result<()> {
        let Some(child) = self.child.as_mut() else {
            return Ok(());
        };
        if self.reopen.is_none()
            && (child.closing.is_some()
                || child.exited()
                || child.signals.disconnected.load(Ordering::Acquire))
        {
            return Ok(());
        }
        if let Some((pending, _)) = self.reopen.as_mut() {
            if snapshot.sequence > pending.sequence {
                *pending = snapshot;
            }
        } else if child.mailbox.accepts(snapshot.sequence) && child.closing.is_none() {
            let frame = child.frame(&mut snapshot)?;
            child.mailbox.replace(frame);
        }
        Ok(())
    }
    pub fn take_command(&mut self) -> Option<AuthorizedCommand> {
        self.child.as_mut()?.queued_command.take()
    }
    pub fn take_controls_changed(&mut self) -> bool {
        self.child
            .as_mut()
            .is_some_and(|child| std::mem::take(&mut child.controls_changed))
    }
    pub fn complete(
        &mut self,
        ticket: u64,
        outcome: CommandState,
        code: Option<&str>,
        detail: Option<String>,
    ) {
        let Some(child) = self.child.as_mut() else {
            return;
        };
        let Some((expected, id)) = child.pending_ticket else {
            return;
        };
        if ticket == expected && child.commands.finish(id, outcome, code, detail) {
            child.pending_ticket = None;
            child.controls_changed = true;
        }
    }
    pub fn poll(&mut self) {
        if let Some(child) = self.child.as_mut() {
            for received in child.events.try_iter().take(EVENT_CAPACITY) {
                let receipt_ms = received
                    .at
                    .saturating_duration_since(child.started)
                    .as_millis();
                match received.event {
                    UiEvent::Ready { sequence, .. } => {
                        if sequence > 0
                            && child.signals.wrote_frame.load(Ordering::Acquire)
                            && sequence <= child.signals.written_sequence.load(Ordering::Acquire)
                        {
                            if !child.ready {
                                eprintln!(
                                    "desktop event=ready endpoint=parent_received pid={} sequence={} parent_elapsed_ms={receipt_ms}",
                                    child.pid, sequence
                                );
                                child.ready = true;
                            }
                        } else {
                            child.signals.rejected.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    UiEvent::Timing {
                        stage, elapsed_ms, ..
                    } => {
                        eprintln!(
                            "desktop event=timing endpoint=parent_received pid={} stage={} child_elapsed_ms={elapsed_ms} parent_elapsed_ms={receipt_ms}",
                            child.pid,
                            stage.as_str()
                        );
                    }
                    UiEvent::Command { request, .. } => {
                        let ready = child.ready
                            && child.closing.is_none()
                            && !child.exited()
                            && !child.signals.disconnected.load(Ordering::Acquire)
                            && child.last_frame_at.elapsed() <= Duration::from_secs(5);
                        if let Some(request) = child.commands.admit(
                            request,
                            child.signals.written_sequence.load(Ordering::Acquire),
                            ready,
                            &child.capability,
                        ) {
                            if let Some(ticket) = self.next_ticket.checked_add(1) {
                                self.next_ticket = ticket;
                                child.pending_ticket = Some((ticket, request.id));
                                child.queued_command = Some(AuthorizedCommand {
                                    ticket,
                                    requested: request.requested,
                                    expected_requested: request.expected_requested,
                                    expected_generation: request.expected_generation,
                                });
                            } else {
                                child.commands.finish(
                                    request.id,
                                    CommandState::Rejected,
                                    Some("unavailable"),
                                    Some(
                                        "Command ticket space exhausted; restart the frontend"
                                            .into(),
                                    ),
                                );
                            }
                        }
                        child.controls_changed = true;
                    }
                }
            }
            let rejected = child.signals.rejected.swap(0, Ordering::Relaxed);
            let dropped = child.signals.dropped.swap(0, Ordering::Relaxed);
            if rejected != 0 || dropped != 0 {
                eprintln!(
                    "desktop event=output_rejected pid={} invalid={} overflow={}",
                    child.pid, rejected, dropped
                );
            }
            if child.signals.disconnected.load(Ordering::Acquire) && child.closing.is_none() {
                eprintln!(
                    "desktop event=pipe_disconnected endpoint=parent_observed pid={} parent_elapsed_ms={}",
                    child.pid,
                    child.started.elapsed().as_millis()
                );
                child.closing = Some(Instant::now());
                child.terminate();
            }
            if child
                .closing
                .is_some_and(|start| start.elapsed() >= CLOSE_GRACE)
            {
                child.terminate();
            }
            if child.exited() {
                eprintln!(
                    "desktop event=process_exit endpoint=parent_observed pid={} parent_elapsed_ms={}",
                    child.pid,
                    child.started.elapsed().as_millis()
                );
                self.child.take();
            }
        }
        if self.child.is_none()
            && let Some((frame, theme)) = self.reopen.take()
        {
            match spawn(&self.executable, frame, theme) {
                Ok(child) => self.child = Some(child),
                Err(_) => eprintln!("desktop event=reopen_failed"),
            }
        }
    }
    pub fn close(&mut self) -> Result<()> {
        self.reopen = None;
        let Some(child) = self.child.as_mut() else {
            return Ok(());
        };
        if child.exited() || child.closing.is_some() {
            return Ok(());
        }
        child.closing = Some(Instant::now());
        child.mailbox.stop();
        eprintln!(
            "desktop event=close_requested endpoint=parent pid={} parent_elapsed_ms={}",
            child.pid,
            child.started.elapsed().as_millis()
        );
        if let Some(window) = owned_window(child.pid) {
            unsafe { PostMessageW(Some(window), WM_CLOSE, WPARAM(0), LPARAM(0)) }?;
        }
        // A child still creating its first window is killed after the same bounded grace.
        Ok(())
    }
}
impl Drop for Supervisor {
    fn drop(&mut self) {
        self.reopen = None;
        self.child.take();
    }
}
fn theme_value(theme: &str) -> Result<&'static str> {
    match theme {
        "light" => Ok("light"),
        "dark" => Ok("dark"),
        "system" => Ok("system"),
        _ => anyhow::bail!("unsupported desktop theme"),
    }
}
fn needs_reopen(
    closing: bool,
    disconnected: bool,
    exited: bool,
    ready: bool,
    has_window: bool,
) -> bool {
    closing || disconnected || exited || (ready && !has_window)
}

struct Search {
    pid: u32,
    window: Option<HWND>,
}
unsafe extern "system" fn find_window(window: HWND, parameter: LPARAM) -> BOOL {
    let search = unsafe { &mut *(parameter.0 as *mut Search) };
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(window, Some(&mut pid));
    }
    if pid == search.pid {
        let mut title = [0u16; 64];
        let length = unsafe { GetWindowTextW(window, &mut title) };
        if length > 0
            && title[..length as usize]
                .iter()
                .copied()
                .eq("MicCamWatch".encode_utf16())
        {
            search.window = Some(window);
            return BOOL(0);
        }
    }
    BOOL(1)
}
fn owned_window(pid: u32) -> Option<HWND> {
    let mut search = Search { pid, window: None };
    unsafe {
        let _ = EnumWindows(
            Some(find_window),
            LPARAM((&mut search as *mut Search) as isize),
        );
    }
    search.window
}

/// Deny write/delete sharing while checking every ancestor with OPEN_REPARSE_POINT.
/// Holding these handles closes the check/use rename, junction and hardlink races.
struct Bundle {
    _reservations: Vec<File>,
    identity: (u32, u32, u32),
}
fn info(file: &File) -> Result<BY_HANDLE_FILE_INFORMATION> {
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    unsafe { GetFileInformationByHandle(handle(file), &mut info) }?;
    Ok(info)
}
fn regular_identity(file: &File) -> Result<(u32, u32, u32)> {
    let metadata = file.metadata()?;
    let info = info(file)?;
    validate_regular(
        metadata.is_file(),
        metadata.file_attributes(),
        info.nNumberOfLinks,
    )?;
    Ok((
        info.dwVolumeSerialNumber,
        info.nFileIndexHigh,
        info.nFileIndexLow,
    ))
}
fn validate_regular(is_file: bool, attributes: u32, links: u32) -> Result<()> {
    ensure!(
        is_file && attributes & 0x400 == 0 && links == 1,
        "desktop executable must be a regular single-link non-reparse file"
    );
    Ok(())
}
fn validate_directory(is_directory: bool, attributes: u32) -> Result<()> {
    ensure!(
        is_directory && attributes & 0x400 == 0,
        "desktop bundle ancestor must be a non-reparse directory"
    );
    Ok(())
}
fn reserve_file(path: &Path) -> Result<File> {
    Ok(OpenOptions::new()
        .read(true)
        .share_mode(1)
        .custom_flags(0x0020_0000)
        .open(path)?)
}
impl Bundle {
    fn open(path: &Path) -> Result<Self> {
        ensure!(path.is_absolute(), "desktop bundle path must be absolute");
        let mut reservations = Vec::new();
        let ancestors: Vec<_> = path
            .parent()
            .context("desktop bundle has no parent")?
            .ancestors()
            .collect();
        for ancestor in ancestors.into_iter().rev() {
            let directory = OpenOptions::new()
                .access_mode(0x80)
                .share_mode(1)
                .custom_flags(0x0200_0000 | 0x0020_0000)
                .open(ancestor)
                .context("reserve desktop bundle ancestor")?;
            let metadata = directory.metadata()?;
            validate_directory(metadata.is_dir(), metadata.file_attributes())?;
            reservations.push(directory);
        }
        let file = reserve_file(path).context("reserve fixed desktop executable")?;
        let identity = regular_identity(&file)?;
        reservations.push(file);
        Ok(Self {
            _reservations: reservations,
            identity,
        })
    }
    fn verify_process(&self, process: &OwnedHandle) -> Result<()> {
        let mut image = vec![0u16; 32768];
        let mut length = image.len() as u32;
        unsafe {
            QueryFullProcessImageNameW(
                handle(process),
                PROCESS_NAME_WIN32,
                PWSTR(image.as_mut_ptr()),
                &mut length,
            )
        }?;
        let image = PathBuf::from(std::ffi::OsString::from_wide(&image[..length as usize]));
        ensure!(
            regular_identity(&reserve_file(&image)?)? == self.identity,
            "desktop child image identity changed"
        );
        Ok(())
    }
}

struct Attributes {
    _storage: Vec<usize>,
    pointer: LPPROC_THREAD_ATTRIBUTE_LIST,
}
impl Attributes {
    fn new(handles: &[HANDLE]) -> Result<Self> {
        let mut bytes = 0;
        unsafe {
            let _ = InitializeProcThreadAttributeList(None, 1, None, &mut bytes);
        }
        ensure!(
            bytes > 0 && bytes < 1024 * 1024,
            "invalid process attribute size"
        );
        let mut storage = vec![0usize; bytes.div_ceil(size_of::<usize>())];
        let pointer = LPPROC_THREAD_ATTRIBUTE_LIST(storage.as_mut_ptr().cast());
        unsafe { InitializeProcThreadAttributeList(Some(pointer), 1, None, &mut bytes) }?;
        let attributes = Self {
            _storage: storage,
            pointer,
        };
        unsafe {
            UpdateProcThreadAttribute(
                pointer,
                0,
                PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
                Some(handles.as_ptr().cast()),
                size_of_val(handles),
                None,
                None,
            )
        }?;
        Ok(attributes)
    }
}
impl Drop for Attributes {
    fn drop(&mut self) {
        unsafe {
            DeleteProcThreadAttributeList(self.pointer);
        }
    }
}
fn pipe() -> Result<(OwnedHandle, OwnedHandle)> {
    let security = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: std::ptr::null_mut(),
        bInheritHandle: BOOL(1),
    };
    let mut read = HANDLE::default();
    let mut write = HANDLE::default();
    unsafe { CreatePipe(&mut read, &mut write, Some(&security), 0) }?;
    Ok(unsafe { (own(read), own(write)) })
}
fn spawn(path: &Path, mut initial: DesktopSnapshot, theme: &'static str) -> Result<Child> {
    let started = Instant::now();
    eprintln!("desktop event=open_requested endpoint=parent elapsed_ms=0");
    let bundle = Bundle::open(path)?;
    let commands = new_command_session()?;
    let job = unsafe { own(CreateJobObjectW(None, PCWSTR::null())?) };
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    unsafe {
        SetInformationJobObject(
            handle(&job),
            JobObjectExtendedLimitInformation,
            (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            size_of_val(&limits) as u32,
        )
    }?;
    let (child_stdin, parent_stdin) = pipe()?;
    let (parent_stdout, child_stdout) = pipe()?;
    unsafe {
        SetHandleInformation(
            handle(&parent_stdin),
            HANDLE_FLAG_INHERIT.0,
            HANDLE_FLAGS(0),
        )?;
        SetHandleInformation(
            handle(&parent_stdout),
            HANDLE_FLAG_INHERIT.0,
            HANDLE_FLAGS(0),
        )?;
    }
    let inherited = [handle(&child_stdin), handle(&child_stdout)];
    let attributes = Attributes::new(&inherited)?;
    let mut startup = STARTUPINFOEXW::default();
    startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
    startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    startup.StartupInfo.hStdInput = handle(&child_stdin);
    startup.StartupInfo.hStdOutput = handle(&child_stdout);
    startup.StartupInfo.hStdError = handle(&child_stdout);
    startup.lpAttributeList = attributes.pointer;
    let application = wide(path.as_os_str());
    // Theme is an allowlisted presentation value, never an executable/endpoint argument.
    let mut command = wide(std::ffi::OsStr::new(&format!(
        "\"{}\" --theme={theme}",
        path.display()
    )));
    let directory = wide(
        path.parent()
            .context("desktop executable has no parent")?
            .as_os_str(),
    );
    let mut process = PROCESS_INFORMATION::default();
    unsafe {
        CreateProcessW(
            PCWSTR(application.as_ptr()),
            Some(PWSTR(command.as_mut_ptr())),
            None,
            None,
            true,
            CREATE_SUSPENDED | EXTENDED_STARTUPINFO_PRESENT,
            None,
            PCWSTR(directory.as_ptr()),
            &startup.StartupInfo,
            &mut process,
        )
    }?;
    let process_handle = unsafe { own(process.hProcess) };
    let thread = unsafe { own(process.hThread) };
    // Before job assignment succeeds, only this failure guard can terminate the suspended child.
    let assigned = unsafe { AssignProcessToJobObject(handle(&job), handle(&process_handle)) };
    if let Err(error) = assigned {
        unsafe {
            let _ = TerminateProcess(handle(&process_handle), 1);
        }
        return Err(error.into());
    }
    bundle.verify_process(&process_handle)?;
    drop(child_stdin);
    drop(child_stdout);
    drop(attributes);
    let mailbox = Arc::new(Mailbox::default());
    let signals = Arc::new(Signals::default());
    let (events_tx, events) = mpsc::sync_channel(EVENT_CAPACITY);
    let mut child = Child {
        process: process_handle,
        job: Some(job),
        pid: process.dwProcessId,
        mailbox,
        signals,
        events,
        closing: None,
        started,
        ready: false,
        commands,
        capability: MicrophoneCapability::default(),
        queued_command: None,
        pending_ticket: None,
        controls_changed: false,
        last_frame_at: started,
        _bundle: bundle,
    };
    let initial_frame = child.frame(&mut initial)?;
    child.mailbox.replace(initial_frame);
    start_writer(
        File::from(parent_stdin),
        child.mailbox.clone(),
        child.signals.clone(),
    )?;
    start_reader(File::from(parent_stdout), events_tx, child.signals.clone())?;
    ensure!(
        unsafe { ResumeThread(handle(&thread)) } != u32::MAX,
        "cannot resume desktop child"
    );
    eprintln!(
        "desktop event=process_resumed endpoint=parent pid={} parent_elapsed_ms={}",
        child.pid,
        started.elapsed().as_millis()
    );
    Ok(child)
}
fn start_writer(mut pipe: File, mailbox: Arc<Mailbox>, signals: Arc<Signals>) -> Result<()> {
    std::thread::Builder::new()
        .name("desktop-input".into())
        .spawn(move || {
            while let Some(frame) = mailbox.take() {
                if pipe.write_all(&frame.bytes).is_err() {
                    signals.disconnected.store(true, Ordering::Release);
                    break;
                }
                signals
                    .written_sequence
                    .store(frame.sequence, Ordering::Release);
                signals.wrote_frame.store(true, Ordering::Release);
            }
        })?;
    Ok(())
}
fn start_reader(mut pipe: File, events: SyncSender<Received>, signals: Arc<Signals>) -> Result<()> {
    std::thread::Builder::new()
        .name("desktop-output".into())
        .spawn(move || {
            let mut lines = EventLines::default();
            let mut buffer = [0u8; 4096];
            loop {
                match pipe.read(&mut buffer) {
                    Ok(0) | Err(_) => {
                        signals.disconnected.store(true, Ordering::Release);
                        break;
                    }
                    Ok(count) => lines.feed(&buffer[..count], |result| match result {
                        Ok(event) => {
                            if events
                                .try_send(Received {
                                    event,
                                    at: Instant::now(),
                                })
                                .is_err()
                            {
                                signals.dropped.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                        Err(_) => {
                            signals.rejected.fetch_add(1, Ordering::Relaxed);
                        }
                    }),
                }
            }
        })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mailbox_coalesces_and_stop_discards_without_history() {
        let queue = Mailbox::default();
        for sequence in 0..1000 {
            queue.replace(Frame {
                sequence,
                bytes: vec![sequence as u8],
            });
        }
        assert_eq!(queue.take().unwrap().sequence, 999);
        // Opening an existing window with the current DTO must not emit a duplicate
        // sequence (Flutter correctly rejects non-monotonic input).
        assert!(!queue.accepts(999));
        assert!(!queue.accepts(998));
        queue.replace(Frame {
            sequence: 999,
            bytes: vec![0],
        });
        assert!(queue.pending.lock().unwrap().latest.is_none());
        queue.replace(Frame {
            sequence: 1000,
            bytes: vec![1],
        });
        queue.stop();
        assert!(queue.take().is_none());
        queue.replace(Frame {
            sequence: 1001,
            bytes: vec![2],
        });
        assert!(queue.take().is_none());
    }
    #[test]
    fn theme_is_not_a_command_channel() {
        for theme in ["light", "dark", "system"] {
            assert!(theme_value(theme).is_ok());
        }
        for theme in ["dark --unmute", "../evil.exe", "", "Light"] {
            assert!(theme_value(theme).is_err());
        }
    }
    #[test]
    fn path_policy_rejects_reparse_directories_and_hardlinks() {
        assert!(validate_regular(true, 0x80, 1).is_ok());
        for (file, attributes, links) in [
            (false, 0x10, 1),
            (true, 0x480, 1),
            (true, 0x80, 2),
            (true, 0x80, 0),
        ] {
            assert!(validate_regular(file, attributes, links).is_err());
        }
        assert!(validate_directory(true, 0x10).is_ok());
        assert!(validate_directory(true, 0x410).is_err());
        assert!(validate_directory(false, 0x80).is_err());
        let directory = tempfile::tempdir().unwrap();
        let original = directory.path().join("original.exe");
        let alias = directory.path().join("alias.exe");
        std::fs::write(&original, b"not executed").unwrap();
        std::fs::hard_link(&original, &alias).unwrap();
        assert!(regular_identity(&File::open(original).unwrap()).is_err());
        assert!(regular_identity(&File::open(alias).unwrap()).is_err());
    }
    #[test]
    fn reserved_executable_cannot_be_written_or_replaced() {
        let directory = tempfile::tempdir().unwrap();
        let original = directory.path().join("reserved.exe");
        let renamed = directory.path().join("renamed.exe");
        std::fs::write(&original, b"never executed").unwrap();
        let reservation = reserve_file(&original).unwrap();
        let identity = regular_identity(&reservation).unwrap();
        assert!(std::fs::write(&original, b"replacement").is_err());
        assert!(std::fs::rename(&original, renamed).is_err());
        assert_eq!(
            regular_identity(&reserve_file(&original).unwrap()).unwrap(),
            identity
        );
    }
    #[test]
    fn reopening_distinguishes_connecting_from_exiting() {
        assert!(!needs_reopen(false, false, false, false, false));
        assert!(!needs_reopen(false, false, false, true, true));
        for (closing, disconnected, exited, ready, window) in [
            (true, false, false, true, true),
            (false, true, false, true, true),
            (false, false, true, true, true),
            (false, false, false, true, false),
        ] {
            assert!(needs_reopen(closing, disconnected, exited, ready, window));
        }
    }
}
