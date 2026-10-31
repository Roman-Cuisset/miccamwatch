use crate::{
    config::Policy,
    frontends::cli::Filter,
    i18n::Language,
    model::{Activity, CollectorState, MicrophoneMuteState, Snapshot},
    platform::{PlatformMonitor, SessionLockState},
    settings::{PrivacyProfile, Settings},
    watcher::{EventDispatcher, TransitionTracker},
};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;
#[cfg(target_os = "macos")]
use std::sync::{Mutex, mpsc::Receiver};
use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Write},
    os::unix::{
        fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt},
        io::AsRawFd,
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender},
    },
    thread,
    time::{Duration, Instant},
};

#[cfg(any(target_os = "macos", test))]
const FRAME_LIMIT: usize = 65_536;
const INTERVAL: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Command {
    Status,
    Mic,
    Camera,
    Pause,
    #[serde(rename = "profile_balanced")]
    ProfileBalanced,
    #[serde(rename = "profile_private")]
    ProfilePrivate,
    #[serde(rename = "profile_meeting")]
    ProfileMeeting,
    #[serde(rename = "profile_development")]
    ProfileDevelopment,
    #[cfg(target_os = "macos")]
    #[serde(rename = "camera_inspect")]
    CameraInspect,
    #[serde(rename = "notifications_enable")]
    NotificationsEnable,
    #[serde(rename = "notifications_disable")]
    NotificationsDisable,
    #[serde(rename = "sound_enable")]
    SoundEnable,
    #[serde(rename = "sound_disable")]
    SoundDisable,
    #[serde(rename = "history_enable")]
    HistoryEnable,
    #[serde(rename = "history_disable")]
    HistoryDisable,
    Autostart,
    Exit,
}
#[cfg(target_os = "linux")]
impl Command {
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "status" => Some(Self::Status),
            "mic" => Some(Self::Mic),
            "camera" => Some(Self::Camera),
            "pause" => Some(Self::Pause),
            "profile_balanced" => Some(Self::ProfileBalanced),
            "profile_private" => Some(Self::ProfilePrivate),
            "profile_meeting" => Some(Self::ProfileMeeting),
            "profile_development" => Some(Self::ProfileDevelopment),
            "notifications_enable" => Some(Self::NotificationsEnable),
            "notifications_disable" => Some(Self::NotificationsDisable),
            "sound_enable" => Some(Self::SoundEnable),
            "sound_disable" => Some(Self::SoundDisable),
            "history_enable" => Some(Self::HistoryEnable),
            "history_disable" => Some(Self::HistoryDisable),
            "autostart" => Some(Self::Autostart),
            "exit" => Some(Self::Exit),
            _ => None,
        }
    }
}
#[derive(Clone, Serialize)]
struct MenuEntry {
    action: &'static str,
    label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
    enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    checked: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    children: Option<Vec<MenuEntry>>,
}
#[derive(Clone, Serialize)]
struct View {
    kind: &'static str,
    summary: String,
    visual: &'static str,
    items: Vec<MenuEntry>,
    diagnostics: String,
    limited: bool,
    language: &'static str,
    version: &'static str,
    protection: DesktopProtection,
    overview: DesktopOverview,
    preferences: DesktopPreferences,
    feedback: Option<String>,
    #[cfg(target_os = "linux")]
    details_label: &'static str,
}
#[derive(Clone, Serialize)]
struct DesktopProtection {
    microphone: &'static str,
    camera: &'static str,
    requested: [Option<bool>; 2],
    microphone_on_lock: bool,
    camera_on_lock: bool,
}

#[derive(Clone, Serialize)]
struct DesktopAccess {
    application: String,
    resource: crate::model::Resource,
    activity: Activity,
    pid: Option<u32>,
}

#[derive(Clone, Serialize)]
struct DesktopOverview {
    microphone: &'static str,
    camera: &'static str,
    observed_at: Option<String>,
    total_accesses: usize,
    omitted_accesses: usize,
    accesses: Vec<DesktopAccess>,
    #[serde(skip)]
    text_limited: bool,
}

impl Default for DesktopOverview {
    fn default() -> Self {
        Self {
            microphone: "unknown",
            camera: "unknown",
            observed_at: None,
            total_accesses: 0,
            omitted_accesses: 0,
            accesses: Vec::new(),
            text_limited: false,
        }
    }
}

impl DesktopOverview {
    fn from_snapshot(snapshot: &Snapshot, observed_at: chrono::DateTime<chrono::Utc>) -> Self {
        let mut text_limited = false;
        let accesses: Vec<_> = snapshot
            .accesses
            .iter()
            .take(8)
            .map(|access| {
                let (application, limited) = bounded_desktop_string(&access.application, 256);
                text_limited |= limited;
                DesktopAccess {
                    application,
                    resource: access.resource,
                    activity: access.activity,
                    pid: access.pid,
                }
            })
            .collect();
        Self {
            microphone: desktop_resource_activity(snapshot, crate::model::Resource::Microphone),
            camera: desktop_resource_activity(snapshot, crate::model::Resource::Camera),
            observed_at: Some(observed_at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)),
            total_accesses: snapshot.accesses.len(),
            omitted_accesses: snapshot.accesses.len() - accesses.len(),
            accesses,
            text_limited,
        }
    }
}

fn desktop_resource_activity(
    snapshot: &Snapshot,
    resource: crate::model::Resource,
) -> &'static str {
    if snapshot
        .accesses
        .iter()
        .any(|access| access.resource == resource && access.activity == Activity::Active)
    {
        return "active";
    }
    if snapshot
        .accesses
        .iter()
        .any(|access| access.resource == resource && access.activity == Activity::Ready)
    {
        return "ready";
    }
    #[cfg(target_os = "macos")]
    let collector = match resource {
        crate::model::Resource::Microphone => "coreaudio_input",
        crate::model::Resource::Camera => "coremediaio_video",
    };
    #[cfg(target_os = "linux")]
    let collector = match resource {
        crate::model::Resource::Microphone => "pipewire_audio",
        crate::model::Resource::Camera => "pipewire_video",
    };
    // No rows is not proof of idle. Require the relevant collector to be
    // present and healthy; degradation or a current gap cannot assert absence.
    if !snapshot.observation_gaps.contains(&resource)
        && snapshot
            .collectors
            .iter()
            .any(|health| health.collector == collector)
        && snapshot
            .collectors
            .iter()
            .filter(|health| health.collector == collector)
            .all(|health| health.state == CollectorState::Healthy)
    {
        "idle"
    } else {
        "unknown"
    }
}

#[derive(Clone, Serialize)]
struct DesktopPreferences {
    writable: bool,
    notifications_enabled: bool,
    sound_enabled: bool,
    history_enabled: bool,
    notifications_paused: bool,
}

fn bounded_desktop_string(text: &str, limit: usize) -> (String, bool) {
    if text.len() <= limit {
        return (text.to_owned(), false);
    }
    let mut end = limit - DESKTOP_OMISSION.len();
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let mut bounded = String::with_capacity(limit);
    bounded.push_str(&text[..end]);
    bounded.push_str(DESKTOP_OMISSION);
    (bounded, true)
}

const DESKTOP_TEXT_BUDGET: usize = 48_000;
const DESKTOP_DIAGNOSTIC_LIMIT: usize = 32_768;
const DESKTOP_OMISSION: &str =
    "\n[… more text omitted by bounded desktop transport; this field is incomplete.]";

// Count JSON-escaped content, not just UTF-8: control characters can expand six
// times. The remaining 17 KiB of a frame covers the bounded protocol structure.
fn desktop_text_cost(text: &str) -> usize {
    text.bytes()
        .map(|byte| match byte {
            b'"' | b'\\' | b'\x08' | b'\x0c' | b'\n' | b'\r' | b'\t' => 2,
            0..=31 => 6,
            _ => 1,
        })
        .sum()
}

fn project_desktop_text(
    text: &mut String,
    limit: usize,
    budget: &mut usize,
    remaining: &mut usize,
) -> bool {
    *remaining -= 1;
    // Every later field retains room for its own explicit omission notice.
    let available = limit.min(budget.saturating_sub(*remaining * 128));
    let original_cost = desktop_text_cost(text);
    if original_cost <= available {
        *budget -= original_cost;
        return false;
    }
    let prefix_budget = available.saturating_sub(desktop_text_cost(DESKTOP_OMISSION));
    let mut used = 0;
    let mut end = 0;
    for (offset, character) in text.char_indices() {
        let cost = match character {
            '"' | '\\' | '\u{08}' | '\u{0c}' | '\n' | '\r' | '\t' => 2,
            '\u{00}'..='\u{1f}' => 6,
            _ => character.len_utf8(),
        };
        if used + cost > prefix_budget {
            break;
        }
        used += cost;
        end = offset + character.len_utf8();
    }
    text.truncate(end);
    text.push_str(DESKTOP_OMISSION);
    *budget -= desktop_text_cost(text);
    true
}

fn bound_desktop_view(view: &mut View) {
    fn field_count(entries: &[MenuEntry]) -> usize {
        entries
            .iter()
            .map(|entry| {
                1 + usize::from(entry.detail.is_some())
                    + entry.children.as_deref().map_or(0, field_count)
            })
            .sum()
    }
    fn labels(entries: &mut [MenuEntry], budget: &mut usize, remaining: &mut usize) -> bool {
        let mut limited = false;
        for entry in entries {
            limited |= project_desktop_text(&mut entry.label, 1024, budget, remaining);
            if let Some(children) = &mut entry.children {
                limited |= labels(children, budget, remaining);
            }
        }
        limited
    }
    fn details(entries: &mut [MenuEntry], budget: &mut usize, remaining: &mut usize) -> bool {
        let mut limited = false;
        for entry in entries {
            if let Some(detail) = &mut entry.detail {
                limited |= project_desktop_text(detail, 8192, budget, remaining);
            }
            if let Some(children) = &mut entry.children {
                limited |= details(children, budget, remaining);
            }
        }
        limited
    }
    let mut budget = DESKTOP_TEXT_BUDGET;
    if let Some(observed_at) = &view.overview.observed_at {
        budget -= desktop_text_cost(observed_at);
    }
    let mut remaining = 2
        + field_count(&view.items)
        + view.overview.accesses.len()
        + usize::from(view.feedback.is_some());
    // These fields have UTF-8 byte bounds as well as a share of the escaped
    // text budget. Project them first so their omission markers stay in bounds.
    for access in &mut view.overview.accesses {
        view.limited |= project_desktop_text(
            &mut access.application,
            256 * 6,
            &mut budget,
            &mut remaining,
        );
    }
    if let Some(feedback) = &mut view.feedback {
        view.limited |= project_desktop_text(feedback, 2048 * 6, &mut budget, &mut remaining);
    }
    view.limited |= project_desktop_text(&mut view.summary, 2048, &mut budget, &mut remaining);
    view.limited |= labels(&mut view.items, &mut budget, &mut remaining);
    view.limited |= project_desktop_text(
        &mut view.diagnostics,
        DESKTOP_DIAGNOSTIC_LIMIT,
        &mut budget,
        &mut remaining,
    );
    view.limited |= details(&mut view.items, &mut budget, &mut remaining);
}

fn uid() -> u32 {
    unsafe { libc::geteuid() }
}
fn runtime_dir() -> Result<PathBuf> {
    let path = crate::settings::data_dir()?.join("tray-runtime");
    fs::create_dir_all(path.parent().context("tray runtime parent unavailable")?)
        .context("tray control: create runtime parent")?;
    let mut builder = fs::DirBuilder::new();
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    match builder.create(&path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error).context("tray control: create runtime directory"),
    }
    let metadata =
        fs::symlink_metadata(&path).context("tray control: inspect runtime directory")?;
    ensure!(
        metadata.is_dir()
            && !metadata.file_type().is_symlink()
            && metadata.uid() == uid()
            && metadata.mode() & 0o077 == 0,
        "tray runtime directory must be owned by this user with mode 0700"
    );
    Ok(path)
}
fn socket_path() -> Result<PathBuf> {
    let path = runtime_dir()?.join("control.sock");
    use std::os::unix::ffi::OsStrExt;
    ensure!(
        path.as_os_str().as_bytes().len() < 104,
        "tray socket path exceeds native Unix socket limit"
    );
    Ok(path)
}
fn verify_peer(stream: &UnixStream) -> Result<()> {
    #[cfg(target_os = "linux")]
    let peer = unsafe {
        let mut credentials: libc::ucred = std::mem::zeroed();
        let mut length = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
        ensure!(
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                (&mut credentials as *mut libc::ucred).cast(),
                &mut length
            ) == 0,
            "cannot authenticate tray socket peer"
        );
        credentials.uid
    };
    #[cfg(target_os = "macos")]
    let peer = unsafe {
        let mut user = 0;
        let mut group = 0;
        ensure!(
            libc::getpeereid(stream.as_raw_fd(), &mut user, &mut group) == 0,
            "cannot authenticate tray socket peer"
        );
        user
    };
    ensure!(peer == uid(), "tray socket peer belongs to another user");
    Ok(())
}
fn read_frame(reader: &mut impl BufRead, limit: usize) -> Result<Option<Vec<u8>>> {
    let mut frame = Vec::new();
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            ensure!(frame.is_empty(), "truncated native frame");
            return Ok(None);
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let length = newline.unwrap_or(available.len());
        ensure!(frame.len() + length <= limit, "native frame exceeds limit");
        frame.extend_from_slice(&available[..length]);
        reader.consume(length + usize::from(newline.is_some()));
        if newline.is_some() {
            return Ok(Some(frame));
        }
    }
}
#[path = "control_peer.rs"]
mod control_peer;

fn connect_control() -> Result<Option<UnixStream>> {
    let path = socket_path()?;
    match fs::symlink_metadata(&path) {
        Ok(metadata) => ensure!(
            metadata.file_type().is_socket()
                && metadata.uid() == uid()
                && metadata.mode() & 0o077 == 0,
            "tray socket ownership or permissions are unsafe"
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).context("tray control: inspect socket"),
    }
    let stream = match UnixStream::connect(path) {
        Ok(stream) => stream,
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::ConnectionRefused | std::io::ErrorKind::NotFound
            ) =>
        {
            return Ok(None);
        }
        Err(error) => return Err(error).context("tray control: connect socket"),
    };
    verify_peer(&stream).context("tray control: authenticate peer")?;
    control_deadline(&stream, false)?;
    stream
        .set_write_timeout(Some(Duration::from_secs(1)))
        .context("tray control: set write timeout")?;
    Ok(Some(stream))
}

fn control_deadline(stream: &UnixStream, stopping: bool) -> Result<()> {
    stream
        .set_read_timeout(Some(Duration::from_secs(if stopping { 45 } else { 1 })))
        .context("tray control: set read timeout")
}

fn control(command: &[u8]) -> Result<Option<String>> {
    let Some(mut stream) = connect_control()? else {
        return Ok(None);
    };
    control_deadline(&stream, command == b"stop\n")?;
    stream
        .write_all(command)
        .context("tray control: write command")?;
    let frame = read_frame(&mut BufReader::new(stream), 64)
        .context("tray control: read acknowledgment")?
        .context("tray closed control connection before acknowledgment")?;
    Ok(Some(
        String::from_utf8(frame).context("tray control: decode acknowledgment")?,
    ))
}

#[derive(Serialize, Deserialize)]
struct ControlNamespace {
    protocol: u8,
    pid: u32,
    config: PathBuf,
    data: PathBuf,
}

fn canonical_namespace(mut path: PathBuf) -> Result<PathBuf> {
    let mut missing = Vec::new();
    loop {
        match fs::canonicalize(&path) {
            Ok(mut resolved) => {
                for component in missing.into_iter().rev() {
                    resolved.push(component);
                }
                return Ok(resolved);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                missing.push(
                    path.file_name()
                        .context("cannot resolve tray namespace")?
                        .to_os_string(),
                );
                ensure!(path.pop(), "cannot resolve tray namespace ancestor");
            }
            Err(error) => return Err(error).context("cannot resolve tray namespace"),
        }
    }
}

impl ControlNamespace {
    fn current() -> Result<Self> {
        Ok(Self {
            protocol: 1,
            pid: std::process::id(),
            config: canonical_namespace(crate::settings::config_dir()?)?,
            data: canonical_namespace(crate::settings::data_dir()?)?,
        })
    }
}

fn owned_control(command: &[u8], wait_for_exit: bool) -> Result<Option<String>> {
    let current = ControlNamespace::current()?;
    let executable = std::env::current_exe().context("cannot locate this tray installation")?;
    let Some(mut stream) = connect_control()? else {
        return Ok(None);
    };
    stream
        .write_all(b"owner\n")
        .context("tray control: request owner namespace")?;
    let peer = control_peer::PeerProcess::retain(&stream)?;
    let mut reader = BufReader::new(stream);
    let frame = read_frame(&mut reader, 32_768)
        .context("tray control: read owner namespace")?
        .context(
            "running tray cannot prove its installation; exit that tray via its menu and retry",
        )?;
    let owner: ControlNamespace =
        serde_json::from_slice(&frame).context("tray control: invalid owner namespace")?;
    ensure!(
        owner.protocol == 1
            && owner.pid == peer.pid()
            && owner.config == current.config
            && owner.data == current.data,
        "another configuration/data namespace owns the tray; no stop or removal permitted"
    );
    peer.verify_image(&executable)?;
    control_deadline(reader.get_ref(), command == b"stop\n")?;
    reader
        .get_mut()
        .write_all(command)
        .context("tray control: write owned command")?;
    let frame = read_frame(&mut reader, 64)
        .context("tray control: read owned acknowledgment")?
        .context("owned tray closed control connection before acknowledgment")?;
    let reply = String::from_utf8(frame).context("tray control: decode owned acknowledgment")?;
    if wait_for_exit && reply == "stopped" {
        peer.wait_exit(Duration::from_secs(45))?;
    }
    Ok(Some(reply))
}

pub(crate) fn stop_current_installation() -> Result<bool> {
    match owned_control(b"stop\n", true)? {
        None => Ok(false),
        Some(reply) if reply == "stopped" => Ok(true),
        Some(reply) => bail!("owned tray shutdown was not acknowledged: {reply}"),
    }
}

pub fn open_running() -> Result<bool> {
    match owned_control(b"open\n", false)? {
        None => Ok(false),
        Some(reply) if reply == "opened" => Ok(true),
        Some(reply) => bail!("owned tray window request was not acknowledged: {reply}"),
    }
}
/// A successful probe means the native status service, not just a process, is registered.
pub fn is_running() -> bool {
    matches!(control(b"ping\n"), Ok(Some(reply)) if reply == "ready")
}
pub fn stop_running() -> Result<bool> {
    match control(b"stop\n")? {
        None => Ok(false),
        Some(reply) if reply == "stopped" => Ok(true),
        Some(reply) => bail!("native tray shutdown was not acknowledged: {reply}"),
    }
}
#[cfg(target_os = "macos")]
#[derive(Default)]
struct DesktopInput {
    writer: Mutex<Option<std::process::ChildStdin>>,
    closed: AtomicBool,
}
#[cfg(target_os = "macos")]
impl DesktopInput {
    fn write(&self, frame: &[u8]) -> Result<()> {
        let mut writer = self
            .writer
            .lock()
            .map_err(|_| anyhow::anyhow!("AppKit input lock poisoned"))?;
        let input = writer.as_mut().context("AppKit helper input closed")?;
        input.write_all(frame)?;
        input.flush()?;
        Ok(())
    }
    fn close(&self) {
        self.closed.store(true, Ordering::Release);
        if let Ok(mut writer) = self.writer.lock() {
            writer.take();
        }
    }
}

struct StagedSocket {
    directory: PathBuf,
    path: PathBuf,
    directory_identity: (u64, u64),
    socket_identity: Option<(u64, u64)>,
}
impl StagedSocket {
    fn new(destination: &Path) -> Result<Self> {
        use std::os::unix::ffi::{OsStrExt, OsStringExt};
        let template = destination
            .parent()
            .context("tray socket parent unavailable")?
            .join(".sXXXXXX");
        let mut template =
            std::ffi::CString::new(template.into_os_string().into_vec())?.into_bytes_with_nul();
        ensure!(
            !unsafe { libc::mkdtemp(template.as_mut_ptr().cast()) }.is_null(),
            "cannot create private tray socket staging directory: {}",
            std::io::Error::last_os_error()
        );
        template.pop();
        let directory = PathBuf::from(std::ffi::OsString::from_vec(template));
        let metadata = fs::symlink_metadata(&directory)?;
        ensure!(
            metadata.is_dir() && metadata.uid() == uid() && metadata.mode() & 0o077 == 0,
            "tray socket staging directory ownership or permissions are unsafe"
        );
        let path = directory.join("s");
        ensure!(
            path.as_os_str().as_bytes().len() < 104,
            "staged tray socket path exceeds native Unix socket limit"
        );
        Ok(Self {
            directory,
            path,
            directory_identity: (metadata.dev(), metadata.ino()),
            socket_identity: None,
        })
    }

    fn bind(&mut self) -> Result<UnixListener> {
        let listener = UnixListener::bind(&self.path)?;
        let metadata = fs::symlink_metadata(&self.path)?;
        ensure!(
            metadata.file_type().is_socket() && metadata.uid() == uid(),
            "staged tray socket ownership is unsafe"
        );
        self.socket_identity = Some((metadata.dev(), metadata.ino()));
        fs::set_permissions(&self.path, fs::Permissions::from_mode(0o600))?;
        listener.set_nonblocking(true)?;
        Ok(listener)
    }

    fn publish(&self, destination: &Path) -> Result<()> {
        use std::os::unix::ffi::OsStrExt;
        let metadata = fs::symlink_metadata(&self.path)?;
        ensure!(
            self.socket_identity == Some((metadata.dev(), metadata.ino()))
                && metadata.file_type().is_socket()
                && metadata.uid() == uid()
                && metadata.mode() & 0o777 == 0o600,
            "staged tray socket changed before publication"
        );
        let source = std::ffi::CString::new(self.path.as_os_str().as_bytes())?;
        let destination = std::ffi::CString::new(destination.as_os_str().as_bytes())?;
        #[cfg(target_os = "linux")]
        let result = unsafe {
            libc::renameat2(
                libc::AT_FDCWD,
                source.as_ptr(),
                libc::AT_FDCWD,
                destination.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        #[cfg(target_os = "macos")]
        let result =
            unsafe { libc::renamex_np(source.as_ptr(), destination.as_ptr(), libc::RENAME_EXCL) };
        if result != 0 {
            return Err(std::io::Error::last_os_error())
                .context("cannot exclusively publish native tray socket");
        }
        Ok(())
    }
}
fn remove_owned_socket(path: &Path, identity: (u64, u64)) {
    if let Ok(metadata) = fs::symlink_metadata(path)
        && metadata.file_type().is_socket()
        && metadata.uid() == uid()
        && (metadata.dev(), metadata.ino()) == identity
    {
        let _ = fs::remove_file(path);
    }
}
impl Drop for StagedSocket {
    fn drop(&mut self) {
        if let Ok(metadata) = fs::symlink_metadata(&self.directory)
            && metadata.is_dir()
            && !metadata.file_type().is_symlink()
            && metadata.uid() == uid()
            && (metadata.dev(), metadata.ino()) == self.directory_identity
        {
            if let Some(identity) = self.socket_identity {
                remove_owned_socket(&self.path, identity);
            }
            let _ = fs::remove_dir(&self.directory);
        }
    }
}

struct Lifecycle {
    _lock: File,
    path: PathBuf,
    socket_identity: (u64, u64),
    ready: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    finished: Arc<AtomicBool>,
    clean_shutdown: Arc<AtomicBool>,
    server_exit: Arc<AtomicBool>,
    open_requested: Arc<AtomicBool>,
    #[cfg(target_os = "macos")]
    window_input: Arc<DesktopInput>,
    server: Option<thread::JoinHandle<()>>,
}
enum LeaseAcquisition {
    Acquired(Lifecycle),
    Contended,
}
impl Lifecycle {
    fn acquire() -> Result<LeaseAcquisition> {
        let path = socket_path()?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(path.with_extension("lock"))?;
        let metadata = lock.metadata()?;
        ensure!(
            metadata.is_file() && metadata.uid() == uid() && metadata.mode() & 0o077 == 0,
            "tray lock ownership or permissions are unsafe"
        );
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::WouldBlock {
                return Ok(LeaseAcquisition::Contended);
            }
            return Err(error).context("cannot acquire native tray lease");
        }
        Self::bind(path, lock).map(LeaseAcquisition::Acquired)
    }

    fn bind(path: PathBuf, lock: File) -> Result<Self> {
        if let Ok(metadata) = fs::symlink_metadata(&path) {
            ensure!(
                metadata.file_type().is_socket() && metadata.uid() == uid(),
                "refusing to remove an unsafe stale tray socket"
            );
            fs::remove_file(&path)?;
        }
        // Bind only inside a private directory. Publish atomically after the
        // socket is safe for consumers; never expose bind's umask-derived mode
        // or replace an entry appearing while this owner prepares its socket.
        let mut staged = StagedSocket::new(&path)?;
        let listener = staged.bind()?;
        let socket_identity = staged
            .socket_identity
            .context("staged tray socket identity unavailable")?;
        staged.publish(&path)?;
        drop(staged);
        let ready = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let finished = Arc::new(AtomicBool::new(false));
        let clean_shutdown = Arc::new(AtomicBool::new(false));
        let server_exit = Arc::new(AtomicBool::new(false));
        let open_requested = Arc::new(AtomicBool::new(false));
        let server_open = open_requested.clone();
        #[cfg(target_os = "macos")]
        let window_input = Arc::new(DesktopInput::default());
        #[cfg(all(target_os = "macos", not(feature = "flutter-desktop")))]
        let server_input = window_input.clone();
        let flags = (
            ready.clone(),
            stop.clone(),
            finished.clone(),
            server_exit.clone(),
            clean_shutdown.clone(),
        );
        let server = thread::spawn(move || {
            while !flags.3.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let reply = (|| -> Result<&str> {
                            // BSD accept can inherit the listener's O_NONBLOCK.
                            // Command reads must honor their bounded timeouts.
                            stream
                                .set_nonblocking(false)
                                .context("set accepted tray command stream blocking")?;
                            verify_peer(&stream)?;
                            stream.set_read_timeout(Some(Duration::from_millis(250)))?;
                            stream.set_write_timeout(Some(Duration::from_millis(250)))?;
                            let mut reader = BufReader::new(stream.try_clone()?);
                            let mut frame =
                                read_frame(&mut reader, 16)?.context("empty tray command")?;
                            if frame == b"owner" {
                                serde_json::to_writer(&mut stream, &ControlNamespace::current()?)?;
                                stream.write_all(b"\n")?;
                                // This new handshake includes kernel/image verification,
                                // not merely the legacy one-frame status/stop request.
                                stream
                                    .set_read_timeout(Some(Duration::from_secs(5)))
                                    .context("set authenticated command timeout")?;
                                frame = read_frame(&mut reader, 16)
                                    .context("read authenticated tray command")?
                                    .context("missing authenticated tray command")?;
                            }
                            match frame.as_slice() {
                                b"ping" => Ok(if flags.0.load(Ordering::Acquire) {
                                    "ready\n"
                                } else {
                                    "starting\n"
                                }),
                                b"open" => {
                                    if flags.1.load(Ordering::Acquire)
                                        || flags.2.load(Ordering::Acquire)
                                    {
                                        Ok("stopping\n")
                                    } else {
                                        #[cfg(all(target_os = "macos", not(feature = "flutter-desktop")))]
                                        ensure!(
                                            !server_input.closed.load(Ordering::Acquire),
                                            "AppKit helper input closed; window request was not accepted"
                                        );
                                        server_open.store(true, Ordering::Release);
                                        #[cfg(all(target_os = "macos", not(feature = "flutter-desktop")))]
                                        if flags.0.load(Ordering::Acquire)
                                            && server_open.swap(false, Ordering::AcqRel)
                                        {
                                            // Deliver independently of a slow device poll.
                                            // Publishing before checking readiness also
                                            // covers its transition without losing open.
                                            server_input.write(b"{\"kind\":\"show\"}\n")?;
                                        }
                                        Ok("opened\n")
                                    }
                                }
                                b"stop" => {
                                    flags.1.store(true, Ordering::Release);
                                    // Main loop retires readiness after accepting stop.
                                    // Clearing the shared AppKit flag here races host.check().
                                    let deadline = Instant::now() + Duration::from_secs(40);
                                    while !flags.2.load(Ordering::Acquire)
                                        && Instant::now() < deadline
                                    {
                                        thread::sleep(Duration::from_millis(20));
                                    }
                                    Ok(if !flags.2.load(Ordering::Acquire) {
                                        "shutdown-timeout\n"
                                    } else if flags.4.load(Ordering::Acquire) {
                                        "stopped\n"
                                    } else {
                                        "shutdown-failed\n"
                                    })
                                }
                                _ => bail!("invalid tray command"),
                            }
                        })();
                        match reply {
                            Ok(reply) => {
                                let _ = stream.write_all(reply.as_bytes());
                            }
                            Err(error) => eprintln!("MicCamWatch tray control: {error:#}"),
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(20))
                    }
                    Err(_) => {
                        flags.0.store(false, Ordering::Release);
                        flags.1.store(true, Ordering::Release);
                        break;
                    }
                }
            }
        });
        Ok(Self {
            _lock: lock,
            path,
            socket_identity,
            ready,
            stop,
            finished,
            clean_shutdown,
            server_exit,
            open_requested,
            #[cfg(target_os = "macos")]
            window_input,
            server: Some(server),
        })
    }

    fn acquire_for_launch(initial_window: bool, explicit_policy: bool) -> Result<Option<Self>> {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            match Self::acquire()? {
                LeaseAcquisition::Acquired(lifecycle) => return Ok(Some(lifecycle)),
                LeaseAcquisition::Contended => {}
            }
            ensure!(
                initial_window || !explicit_policy,
                "MicCamWatch is already starting or running; an explicit policy cannot replace its active policy"
            );
            // The lease is acquired before the socket is bound. Losing that
            // lease must not discard an explicit launch in this interval.
            // Every reuse authenticates both namespace and executable image;
            // lock contention alone never proves an owned native frontend.
            if initial_window {
                if open_running()? {
                    return Ok(None);
                }
            } else if let Some(reply) = owned_control(b"ping\n", false)? {
                match reply.as_str() {
                    "ready" => return Ok(None),
                    // Do not claim a successful background service until its
                    // native host is ready; initialization can fail headlessly.
                    "starting" => {}
                    _ => bail!("owned tray startup was not acknowledged: {reply}"),
                }
            }
            ensure!(
                Instant::now() < deadline,
                "MicCamWatch's native tray lease is held but its owned frontend did not become available; exit that tray via its menu and retry"
            );
            thread::sleep(Duration::from_millis(20));
            // If startup exited, acquire its released lease normally. Never
            // unlink an active owner's socket or force another process out.
        }
    }
}
impl Drop for Lifecycle {
    fn drop(&mut self) {
        self.ready.store(false, Ordering::Release);
        self.finished.store(true, Ordering::Release);
        self.server_exit.store(true, Ordering::Release);
        if let Some(server) = self.server.take() {
            let _ = server.join();
        }
        remove_owned_socket(&self.path, self.socket_identity);
    }
}

// Localized controls are passed unchanged to both native hosts. Native error details are retained.
struct Words {
    idle: &'static str,
    degraded: &'static str,
    failed: &'static str,
    state_unknown: &'static str,
    mute: &'static str,
    unmute: &'static str,
    unavailable: &'static str,
    scope: &'static str,
    camera: &'static str,
    restore: &'static str,
    linux_camera: &'static str,
    linux_restore: &'static str,
    pause: &'static str,
    resume: &'static str,
    profile: &'static str,
    profiles: [&'static str; 4],
    autostart_on: &'static str,
    autostart_off: &'static str,
    exit: &'static str,
    done: &'static str,
    lock_unknown: &'static str,
    lock_status: &'static str,
    #[cfg(target_os = "linux")]
    details: &'static str,
    camera_inspect: &'static str,
    inspecting: &'static str,
    config_override: &'static str,
}
fn words(lang: Language) -> Words {
    match lang {
        Language::En => Words {
            idle: "No observed capture",
            degraded: "Observation incomplete",
            failed: "Action failed",
            state_unknown: "State unknown",
            mute: "Mute",
            unmute: "Restore",
            unavailable: "Microphone control unavailable",
            scope: "writable inputs / PipeWire session only",
            camera: "Approve camera restriction profile",
            restore: "Remove camera restriction profile",
            linux_camera: "Block supported USB cameras (administrator approval)",
            linux_restore: "Restore owned USB camera blocks (administrator approval)",
            pause: "Pause alerts (30 min)",
            resume: "Resume alerts",
            profile: "Profile",
            profiles: ["Balanced", "Private", "Meeting", "Development"],
            autostart_on: "Disable autostart",
            autostart_off: "Enable autostart",
            exit: "Exit",
            done: "Applied",
            lock_unknown: "Session lock unknown; automatic controls unavailable",
            lock_status: "Session lock unknown",
            #[cfg(target_os = "linux")]
            details: "Open MicCamWatch…",
            camera_inspect: "Inspect camera profiles (admin, read-only)",
            inspecting: "Read-only camera inspection awaiting administrator approval / running…",
            config_override: "Profile controlled by explicit --config; menu changes disabled",
        },
        Language::Fr => Words {
            idle: "Aucune capture observée",
            degraded: "Observation incomplète",
            failed: "Échec de l’action",
            state_unknown: "État inconnu",
            mute: "Couper",
            unmute: "Restaurer",
            unavailable: "Contrôle du microphone indisponible",
            scope: "entrées modifiables / session PipeWire uniquement",
            camera: "Approuver le profil de restriction caméra",
            restore: "Retirer le profil de restriction caméra",
            linux_camera: "Bloquer les caméras USB compatibles (autorisation administrateur)",
            linux_restore: "Restaurer les caméras USB bloquées par MCW (autorisation administrateur)",
            pause: "Suspendre les alertes (30 min)",
            resume: "Reprendre les alertes",
            profile: "Profil",
            profiles: ["Équilibré", "Privé", "Réunion", "Développement"],
            autostart_on: "Désactiver le démarrage automatique",
            autostart_off: "Activer le démarrage automatique",
            exit: "Quitter",
            done: "Appliqué",
            lock_unknown: "Verrouillage inconnu ; contrôles automatiques indisponibles",
            lock_status: "Verrouillage inconnu",
            #[cfg(target_os = "linux")]
            details: "Ouvrir MicCamWatch…",
            camera_inspect: "Inspecter profils caméra (admin, lecture seule)",
            inspecting: "Inspection caméra en lecture seule : autorisation administrateur en attente / en cours…",
            config_override: "Profil contrôlé par --config explicite ; modifications désactivées",
        },
        Language::De => Words {
            idle: "Keine Erfassung beobachtet",
            degraded: "Beobachtung unvollständig",
            failed: "Aktion fehlgeschlagen",
            state_unknown: "Zustand unbekannt",
            mute: "Stummschalten",
            unmute: "Wiederherstellen",
            unavailable: "Mikrofonsteuerung nicht verfügbar",
            scope: "nur schreibbare Eingänge / PipeWire-Sitzung",
            camera: "Kamerasperrprofil genehmigen",
            restore: "Kamerasperrprofil entfernen",
            linux_camera: "Unterstützte USB-Kameras sperren (Administratorfreigabe)",
            linux_restore: "Eigene USB-Kamerasperren aufheben (Administratorfreigabe)",
            pause: "Hinweise pausieren (30 Min.)",
            resume: "Hinweise fortsetzen",
            profile: "Profil",
            profiles: ["Ausgewogen", "Privat", "Besprechung", "Entwicklung"],
            autostart_on: "Autostart deaktivieren",
            autostart_off: "Autostart aktivieren",
            exit: "Beenden",
            done: "Angewendet",
            lock_unknown: "Sitzungssperre unbekannt; automatische Steuerung nicht verfügbar",
            lock_status: "Sitzungssperre unbekannt",
            #[cfg(target_os = "linux")]
            details: "MicCamWatch öffnen…",
            camera_inspect: "Kameraprofile prüfen (Admin, nur lesen)",
            inspecting: "Nur lesende Kameraprüfung wartet auf Administratorfreigabe / läuft…",
            config_override: "Profil durch explizites --config gesteuert; Menüänderungen deaktiviert",
        },
        Language::Es => Words {
            idle: "No se observa captura",
            degraded: "Observación incompleta",
            failed: "Acción fallida",
            state_unknown: "Estado desconocido",
            mute: "Silenciar",
            unmute: "Restaurar",
            unavailable: "Control del micrófono no disponible",
            scope: "solo entradas modificables / sesión PipeWire",
            camera: "Aprobar perfil de restricción de cámara",
            restore: "Eliminar perfil de restricción de cámara",
            linux_camera: "Bloquear cámaras USB compatibles (aprobación de administrador)",
            linux_restore: "Restaurar bloqueos USB propios (aprobación de administrador)",
            pause: "Pausar alertas (30 min)",
            resume: "Reanudar alertas",
            profile: "Perfil",
            profiles: ["Equilibrado", "Privado", "Reunión", "Desarrollo"],
            autostart_on: "Desactivar inicio automático",
            autostart_off: "Activar inicio automático",
            exit: "Salir",
            done: "Aplicado",
            lock_unknown: "Bloqueo de sesión desconocido; controles automáticos no disponibles",
            lock_status: "Bloqueo de sesión desconocido",
            #[cfg(target_os = "linux")]
            details: "Abrir MicCamWatch…",
            camera_inspect: "Perfiles de cámara (admin, solo lectura)",
            inspecting: "Inspección de cámara de solo lectura: esperando aprobación de administrador / en curso…",
            config_override: "Perfil controlado por --config explícito; cambios desactivados",
        },
        Language::Ja => Words {
            idle: "キャプチャは観測されていません",
            degraded: "観測範囲が不完全です",
            failed: "操作に失敗しました",
            state_unknown: "状態不明",
            mute: "ミュート",
            unmute: "復元",
            unavailable: "マイク制御は利用できません",
            scope: "書き込み可能な入力 / PipeWire セッションのみ",
            camera: "カメラ制限プロファイルを承認",
            restore: "カメラ制限プロファイルを削除",
            linux_camera: "対応 USB カメラをブロック (管理者承認)",
            linux_restore: "自分の USB カメラブロックを復元 (管理者承認)",
            pause: "通知を一時停止 (30 分)",
            resume: "通知を再開",
            profile: "プロファイル",
            profiles: ["バランス", "プライベート", "会議", "開発"],
            autostart_on: "自動起動を無効化",
            autostart_off: "自動起動を有効化",
            exit: "終了",
            done: "適用済み",
            lock_unknown: "セッションロック不明・自動制御は利用できません",
            lock_status: "セッションロック不明",
            #[cfg(target_os = "linux")]
            details: "MicCamWatchを開く…",
            camera_inspect: "カメラプロファイル（管理者・読み取り専用）",
            inspecting: "読み取り専用カメラ調査：管理者承認待ち／実行中…",
            config_override: "明示的な --config がプロファイルを制御・メニュー変更は無効",
        },
        Language::Zh => Words {
            idle: "未观察到采集",
            degraded: "观测范围不完整",
            failed: "操作失败",
            state_unknown: "状态未知",
            mute: "静音",
            unmute: "恢复",
            unavailable: "麦克风控制不可用",
            scope: "仅可写输入 / PipeWire 会话",
            camera: "批准摄像头限制描述文件",
            restore: "移除摄像头限制描述文件",
            linux_camera: "禁用支持的 USB 摄像头（管理员授权）",
            linux_restore: "恢复自有 USB 摄像头禁用（管理员授权）",
            pause: "暂停提醒 (30 分钟)",
            resume: "恢复提醒",
            profile: "配置",
            profiles: ["平衡", "隐私", "会议", "开发"],
            autostart_on: "禁用自动启动",
            autostart_off: "启用自动启动",
            exit: "退出",
            done: "已应用",
            lock_unknown: "会话锁定状态未知；自动控制不可用",
            lock_status: "会话锁定状态未知",
            #[cfg(target_os = "linux")]
            details: "打开 MicCamWatch…",
            camera_inspect: "检查摄像头配置（管理员；只读）",
            inspecting: "只读摄像头检查：等待管理员授权／进行中…",
            config_override: "由显式 --config 控制配置；已禁用菜单更改",
        },
        Language::Ru => Words {
            idle: "Захват не наблюдается",
            degraded: "Неполное наблюдение",
            failed: "Ошибка действия",
            state_unknown: "Состояние неизвестно",
            mute: "Выключить звук",
            unmute: "Восстановить",
            unavailable: "Управление микрофоном недоступно",
            scope: "только доступные для записи входы / сеанс PipeWire",
            camera: "Одобрить профиль ограничения камеры",
            restore: "Удалить профиль ограничения камеры",
            linux_camera: "Блокировать поддерживаемые USB-камеры (разрешение администратора)",
            linux_restore: "Восстановить свои блокировки USB-камер (разрешение администратора)",
            pause: "Приостановить уведомления (30 мин)",
            resume: "Возобновить уведомления",
            profile: "Профиль",
            profiles: ["Сбалансированный", "Приватный", "Встреча", "Разработка"],
            autostart_on: "Отключить автозапуск",
            autostart_off: "Включить автозапуск",
            exit: "Выход",
            done: "Применено",
            lock_unknown: "Блокировка сеанса неизвестна; автоматическое управление недоступно",
            lock_status: "Блокировка сеанса неизвестна",
            #[cfg(target_os = "linux")]
            details: "Открыть MicCamWatch…",
            camera_inspect: "Профили камер (админ, только чтение)",
            inspecting: "Проверка камеры только для чтения: ожидание разрешения администратора / выполняется…",
            config_override: "Профиль задан явным --config; изменения в меню отключены",
        },
    }
}
fn profile_index(profile: PrivacyProfile) -> usize {
    match profile {
        PrivacyProfile::Balanced => 0,
        PrivacyProfile::Private => 1,
        PrivacyProfile::Meeting => 2,
        PrivacyProfile::Development => 3,
    }
}
fn entry(action: &'static str, label: impl Into<String>, enabled: bool) -> MenuEntry {
    let label = label.into();
    MenuEntry {
        action,
        detail: None,
        label,
        enabled,
        checked: None,
        children: None,
    }
}
// Keep full diagnostics separate from compact visible statuses on both hosts.
fn compact_entry(
    action: &'static str,
    label: impl Into<String>,
    enabled: bool,
    text: &Words,
) -> MenuEntry {
    let label = label.into();
    if let Some((first_line, _)) = label.split_once('\n') {
        return MenuEntry {
            action,
            label: first_line.to_owned(),
            detail: Some(label),
            enabled,
            checked: None,
            children: None,
        };
    }
    let short = if label == text.lock_unknown {
        Some(text.lock_status)
    } else {
        [text.failed, text.degraded, text.done, text.state_unknown]
            .into_iter()
            .find(|status| {
                label != *status
                    && label
                        .strip_prefix(*status)
                        .is_some_and(|detail| detail.starts_with(": "))
            })
    };
    match short {
        Some(short) => MenuEntry {
            action,
            label: short.to_owned(),
            detail: Some(label),
            enabled,
            checked: None,
            children: None,
        },
        None => entry(action, label, enabled),
    }
}

fn observation_status(snapshot: &Snapshot, lang: Language) -> (String, &'static str) {
    let unavailable = snapshot.collectors.is_empty()
        || snapshot
            .collectors
            .iter()
            .any(|health| health.state == CollectorState::Unavailable);
    #[cfg(target_os = "macos")]
    let expected = ["coreaudio_input", "coremediaio_video"];
    #[cfg(target_os = "linux")]
    let expected = ["pipewire_audio", "pipewire_video"];
    let unavailable = unavailable
        || expected.iter().any(|collector| {
            !snapshot
                .collectors
                .iter()
                .any(|health| health.collector == *collector)
        });
    let partial = !snapshot.observation_gaps.is_empty()
        || snapshot
            .collectors
            .iter()
            .any(|health| health.state == CollectorState::Degraded);
    let active = snapshot
        .accesses
        .iter()
        .any(|access| access.activity == Activity::Active);
    let ready = snapshot
        .accesses
        .iter()
        .any(|access| access.activity == Activity::Ready);
    let visual = if unavailable {
        "error"
    } else if partial {
        "partial"
    } else if active {
        "active"
    } else if ready {
        "ready"
    } else {
        "idle"
    };
    let mut summary = String::new();
    // Show both active captures and ready pipelines, including during partial observation.
    for access in &snapshot.accesses {
        if !summary.is_empty() {
            summary.push_str(", ");
        }
        let _ = write!(
            summary,
            "{}: {} {}",
            access.application,
            lang.resource(access.resource),
            lang.state_str(None, access.activity)
        );
    }
    if unavailable || partial {
        if !summary.is_empty() {
            summary.push_str(" — ");
        }
        summary.push_str(words(lang).degraded);
    } else if summary.is_empty() {
        summary.push_str(words(lang).idle);
    }
    #[cfg(target_os = "macos")]
    {
        let accesses = std::mem::take(&mut summary);
        for resource in [
            crate::model::Resource::Microphone,
            crate::model::Resource::Camera,
        ] {
            let activity = match desktop_resource_activity(snapshot, resource) {
                "active" => lang.state_str(None, Activity::Active),
                "ready" => lang.state_str(None, Activity::Ready),
                "idle" => words(lang).idle,
                _ => words(lang).state_unknown,
            };
            if !summary.is_empty() {
                summary.push_str(" · ");
            }
            let _ = write!(summary, "{}: {activity}", lang.resource(resource));
        }
        if unavailable || partial {
            let _ = write!(summary, " — {}", words(lang).degraded);
        }
        let _ = write!(
            summary,
            "\n\nObserved accesses: {}\n{accesses}",
            snapshot.accesses.len()
        );
        for access in &snapshot.accesses {
            let _ = write!(
                summary,
                "\n\n{} — PID: {:?}; {:?}; {:?}",
                access.application, access.pid, access.confidence, access.enforcement
            );
            if let Some(device) = &access.device {
                let _ = write!(summary, "\n{device}");
            }
            if let Some(executable) = &access.executable {
                let _ = write!(summary, "\n{executable}");
            }
            for evidence in &access.evidence {
                let _ = write!(summary, "\n{}: {}", evidence.source, evidence.detail);
            }
        }
    }
    for health in &snapshot.collectors {
        if health.state != CollectorState::Healthy {
            let _ = write!(
                summary,
                "\n\n{}: {:?}\n{}",
                health.collector,
                health.state,
                health
                    .detail
                    .as_deref()
                    .unwrap_or("No diagnostic detail supplied")
            );
        }
    }
    #[cfg(target_os = "macos")]
    for collector in ["coreaudio_input", "coremediaio_video"] {
        if !snapshot
            .collectors
            .iter()
            .any(|health| health.collector == collector)
        {
            let _ = write!(
                summary,
                "\n\n{collector}: collector absent from current observation; activity is unknown"
            );
        }
    }
    for resource in &snapshot.observation_gaps {
        let _ = write!(
            summary,
            "\n\n{}: current observation unavailable; absence does not prove capture stopped",
            lang.resource(*resource)
        );
    }
    (summary, visual)
}

#[derive(Default)]
struct CameraInspectionUi {
    system_managed: bool,
    inventory_incomplete: bool,
    administrator_required: bool,
    running: bool,
    detail: Option<String>,
}

#[derive(Default)]
struct CameraInspectionWorker {
    #[cfg(target_os = "macos")]
    pending: Option<Receiver<Result<crate::privacy::CameraInspection>>>,
    ui: CameraInspectionUi,
}

#[cfg(target_os = "macos")]
impl CameraInspectionWorker {
    fn observe_passive_state(&mut self, state: crate::privacy::CameraPrivacyState) {
        self.ui.system_managed = state == crate::privacy::CameraPrivacyState::SystemManaged;
        self.ui.inventory_incomplete = false;
        self.ui.administrator_required = false;
        // Keep the timestamped point-in-time inspection result as diagnostics.
        // A current passive success supersedes only its obsolete action guidance.
    }

    fn start(&mut self, lang: Language) -> Result<()> {
        if self.pending.is_some()
            || !(self.ui.inventory_incomplete || self.ui.administrator_required)
        {
            return Ok(());
        }
        let (sender, receiver) = mpsc::sync_channel(1);
        thread::Builder::new()
            .name("camera-inspection".into())
            .spawn(move || {
                // Only this explicit menu action can request administrator approval.
                let _ = sender.send(crate::privacy::camera_inspection(true));
            })?;
        self.pending = Some(receiver);
        self.ui.running = true;
        self.ui.detail = Some(words(lang).inspecting.to_owned());
        Ok(())
    }

    fn poll(&mut self) -> bool {
        let Some(receiver) = &self.pending else {
            return false;
        };
        let result = match receiver.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return false,
            Err(mpsc::TryRecvError::Disconnected) => Err(anyhow::anyhow!(
                "camera inspection worker closed without a result"
            )),
        };
        self.pending = None;
        self.ui.running = false;
        self.ui.detail = Some(match result {
            Ok(inspection) => {
                self.ui.administrator_required = inspection.administrator_required;
                let authorization = match inspection.authorization {
                    crate::privacy::CameraInspectionAuthorization::NotRequested => "not requested",
                    crate::privacy::CameraInspectionAuthorization::Granted => "granted",
                    crate::privacy::CameraInspectionAuthorization::Cancelled => "cancelled",
                    crate::privacy::CameraInspectionAuthorization::Denied => "denied",
                };
                format!(
                    "Read-only camera inspection — point-in-time result at {}\nAdministrator authorization: {authorization}\n{}",
                    inspection.observed_at.to_rfc3339(),
                    inspection.detail
                )
            }
            Err(error) => format!("Read-only camera inspection failed: {error:#}"),
        });
        true
    }
}
struct MenuContext<'a> {
    lock: SessionLockState,
    explicit_policy: bool,
    inspection: &'a CameraInspectionUi,
    #[cfg(target_os = "macos")]
    requested: [Option<bool>; 2],
    #[cfg(target_os = "macos")]
    overview: &'a DesktopOverview,
}

fn make_view(
    status: (String, &'static str),
    mute: MicrophoneMuteState,
    camera: (bool, Option<&str>),
    settings: &Settings,
    lang: Language,
    feedback: &Option<String>,
    context: MenuContext<'_>,
) -> View {
    let MenuContext {
        lock,
        explicit_policy,
        inspection,
        #[cfg(target_os = "macos")]
        requested,
        #[cfg(target_os = "macos")]
        overview,
    } = context;
    let (desktop_feedback, feedback_limited) = match feedback {
        Some(text) => {
            let (text, limited) = bounded_desktop_string(text, 2048);
            (Some(text), limited)
        }
        None => (None, false),
    };
    let (summary, visual) = status;
    let (summary, diagnostics) = {
        let compact = summary.lines().next().unwrap_or(&summary).to_owned();
        (compact, summary)
    };
    let (camera_blocked, camera_error) = camera;
    let text = words(lang);
    let visual = if camera_error.is_some() && visual != "error" {
        "partial"
    } else {
        visual
    };
    let scope = if cfg!(target_os = "macos") {
        text.scope.split(" / ").next().unwrap_or(text.scope)
    } else {
        text.scope.split(" / ").nth(1).unwrap_or(text.scope)
    };
    let mic = match mute {
        MicrophoneMuteState::Unavailable => text.unavailable.to_owned(),
        MicrophoneMuteState::Muted => format!("{} — {}", text.unmute, scope),
        MicrophoneMuteState::Unmuted | MicrophoneMuteState::Mixed => {
            format!("{} — {}", text.mute, scope)
        }
    };
    let mut items = vec![
        entry(
            "header",
            format!("MicCamWatch {}", env!("CARGO_PKG_VERSION")),
            false,
        ),
        compact_entry("status", &summary, true, &text),
    ];
    if let Some(feedback) = feedback {
        items.push(compact_entry("header", feedback, false, &text));
    }
    if lock == SessionLockState::Unknown {
        items.push(compact_entry("header", text.lock_unknown, false, &text));
    }
    items.push(entry("mic", mic, mute != MicrophoneMuteState::Unavailable));
    #[cfg(target_os = "linux")]
    {
        items.push(entry("header", crate::privacy::camera_capability(), false));
    }
    if let Some(error) = camera_error {
        items.push(MenuEntry {
            action: "header",
            label: format!(
                "{}: {}",
                lang.resource(crate::model::Resource::Camera),
                text.state_unknown
            ),
            detail: Some(error.to_owned()),
            enabled: false,
            checked: None,
            children: None,
        });
    }
    let camera_entry = entry(
        "camera",
        if cfg!(target_os = "linux") {
            if camera_blocked {
                text.linux_restore
            } else {
                text.linux_camera
            }
        } else if camera_blocked {
            text.restore
        } else {
            text.camera
        },
        cfg!(target_os = "macos") || camera_error.is_none(),
    );
    #[cfg(target_os = "linux")]
    let camera_entry = {
        let mut camera_entry = camera_entry;
        if let Err(error) = crate::privacy::camera_control_preflight() {
            camera_entry.enabled = false;
            camera_entry.detail = Some(format!("{error:#}"));
        }
        camera_entry
    };
    items.push(camera_entry);
    if inspection.inventory_incomplete || inspection.administrator_required {
        items.push(entry(
            "camera_inspect",
            text.camera_inspect,
            !inspection.running,
        ));
    }
    if let Some(detail) = &inspection.detail {
        let mut diagnostic = entry(
            "header",
            if inspection.running {
                text.inspecting
            } else {
                text.camera_inspect
            },
            false,
        );
        diagnostic.detail = Some(detail.clone());
        items.push(diagnostic);
    }
    items.push(entry(
        "pause",
        if settings.notifications_paused() {
            text.resume
        } else {
            text.pause
        },
        true,
    ));
    let profile = entry(
        "profile",
        if explicit_policy {
            format!("{} — {}", text.profile, text.config_override)
        } else {
            format!(
                "{}: {}",
                text.profile,
                text.profiles[profile_index(settings.profile)]
            )
        },
        !explicit_policy,
    );
    let profile = {
        let mut profile = profile;
        profile.children = Some(
            [
                "profile_balanced",
                "profile_private",
                "profile_meeting",
                "profile_development",
            ]
            .into_iter()
            .enumerate()
            .map(|(index, action)| {
                let mut choice = entry(action, text.profiles[index], !explicit_policy);
                choice.checked = Some(!explicit_policy && index == profile_index(settings.profile));
                choice
            })
            .collect(),
        );
        profile
    };
    items.push(profile);
    let autostart = crate::autostart::state();
    items.push(compact_entry(
        "autostart",
        match &autostart {
            Ok(crate::autostart::AutostartState::Enabled) => text.autostart_on.to_owned(),
            Ok(crate::autostart::AutostartState::Disabled) => text.autostart_off.to_owned(),
            Err(error) => format!("{}: {error:#}", text.state_unknown),
        },
        autostart.is_ok(),
        &text,
    ));
    items.push(entry("exit", text.exit, true));
    let mut view = View {
        kind: "state",
        summary,
        diagnostics,
        limited: feedback_limited,
        visual,
        language: lang.code(),
        version: env!("CARGO_PKG_VERSION"),
        protection: DesktopProtection {
            microphone: match mute {
                MicrophoneMuteState::Muted => "muted",
                MicrophoneMuteState::Unmuted => "unmuted",
                MicrophoneMuteState::Mixed => "mixed",
                MicrophoneMuteState::Unavailable => "unknown",
            },
            camera: if camera_error.is_some() {
                "unknown"
            } else if inspection.system_managed {
                "system_managed"
            } else if camera_blocked {
                "owned"
            } else {
                "absent"
            },
            #[cfg(target_os = "macos")]
            requested,
            #[cfg(target_os = "linux")]
            requested: [None; 2],
            microphone_on_lock: settings.mute_on_lock,
            camera_on_lock: settings.block_camera_on_lock,
        },
        #[cfg(target_os = "macos")]
        overview: overview.clone(),
        #[cfg(target_os = "linux")]
        overview: DesktopOverview::default(),
        preferences: DesktopPreferences {
            writable: true,
            notifications_enabled: settings.notifications_enabled,
            sound_enabled: settings.sound_enabled,
            history_enabled: settings.history_enabled,
            notifications_paused: settings.notifications_paused(),
        },
        feedback: desktop_feedback,
        #[cfg(target_os = "linux")]
        details_label: text.details,
        items,
    };
    view.limited |= view.overview.text_limited;
    bound_desktop_view(&mut view);
    view
}

/// Shared process-owned Flutter presentation/preferences lane. Native collectors,
/// controls and alerts never depend on the disposable child remaining open.
#[cfg(feature = "flutter-desktop")]
struct FlutterFrontend {
    ui: Option<crate::frontends::desktop::unix::Supervisor>,
    worker: Option<crate::frontends::desktop::preferences::PreferenceWorker>,
    current: Option<(Result<Snapshot, String>, chrono::DateTime<chrono::Utc>)>,
    sequence: u64,
    writable: bool,
    profile_writable: bool,
    operational: bool,
    applied_profile: Option<PrivacyProfile>,
    lang: Language,
}
#[cfg(feature = "flutter-desktop")]
impl FlutterFrontend {
    fn new(lang: Language, writable: bool, profile_writable: bool, operational: bool) -> Self {
        Self {
            ui: None, worker: None, sequence: 1, writable, profile_writable, operational,
            applied_profile: None, lang,
            current: None,
        }
    }
    fn attached(&self) -> bool {
        self.ui.as_ref().is_some_and(crate::frontends::desktop::unix::Supervisor::is_running)
    }
    fn frame(&mut self) -> crate::frontends::desktop::DesktopSnapshot {
        self.sequence = self.sequence.checked_add(1).expect("desktop sequence exhausted");
        let (snapshot, observed_at) = self.current.as_ref().map_or(
            (Err("Initial native observation pending"), chrono::Utc::now()),
            |(snapshot, at)| (snapshot.as_ref().map_err(String::as_str), *at),
        );
        let mut frame = crate::frontends::desktop::DesktopSnapshot::from_snapshot(
            self.sequence, observed_at, self.lang.code(), snapshot,
            if self.operational { "operational" } else if self.writable {
                "settings_development"
            } else { "read_only_development" },
        );
        frame.preferences = crate::frontends::desktop::observe_preferences(
            self.writable, self.profile_writable, self.applied_profile,
            crate::frontends::desktop::preferences::PreferenceEffects {
                capture_notifications_supported: self.operational,
                capture_sounds_supported: self.operational,
                history_recording_supported: self.operational,
                history_recording_active: false,
            }, self.worker.as_ref(),
        );
        frame.preferences.effects.history_recording_active = self.operational
            && frame.preferences.values.as_ref().is_some_and(|values| values.history_enabled);
        frame
    }
    fn observe(&mut self, snapshot: Result<Snapshot, String>, observed_at: chrono::DateTime<chrono::Utc>) {
        // Move the existing native observation; no DTO copies/serialization while
        // closed, and reopening never starts with a deliberately stale last UI.
        self.current = Some((snapshot, observed_at));
    }
    fn publish(&mut self) -> Result<()> {
        if self.attached() {
            let frame = self.frame();
            self.ui.as_mut().expect("attached UI").publish(frame)?;
        }
        Ok(())
    }
    fn open(&mut self, theme: &str) -> Result<()> {
        if self.ui.is_none() {
            self.ui = Some(crate::frontends::desktop::unix::Supervisor::new()?);
        }
        let frame = self.frame();
        self.ui.as_mut().expect("initialized UI").open(frame, theme)
    }
    fn refresh(&mut self) -> Result<bool> {
        use crate::frontends::desktop::{
            commands::CommandState,
            preferences::{PreferenceWork, PreferenceWorker},
            unix::Supervisor,
        };
        if let Some(ui) = self.ui.as_mut() { ui.poll(); }
        let mut changed = false;
        if let Some(completion) = self.worker.as_mut().and_then(PreferenceWorker::try_complete) {
            if let Some(ui) = self.ui.as_mut() {
                match completion.result {
                    Ok(_) => ui.complete_preference(completion.ticket, CommandState::Succeeded, None,
                        Some("Saved preference readback confirmed; applied policy and delivery remain separate".into())),
                    Err(error) => ui.complete_preference(completion.ticket, CommandState::Failed,
                        Some("preferences_error"), Some(format!("Preference write/readback failed or may be partial: {error}"))),
                }
            }
            changed = true;
        }
        if let Some(command) = self.ui.as_mut().and_then(Supervisor::take_preference) {
            let ticket = command.ticket;
            let work = PreferenceWork { ticket, request: command.request, profile_writable: self.profile_writable };
            if let Err(error) = PreferenceWorker::enqueue(&mut self.worker, work) {
                self.ui.as_mut().expect("request UI").complete_preference(ticket, CommandState::Rejected,
                    Some("preferences_worker"), Some(format!("Preference request not queued: {error:#}")));
            }
            changed = true;
        }
        changed |= self.ui.as_mut().is_some_and(Supervisor::take_controls_changed);
        let presentation_changed = self.ui.as_mut().is_some_and(Supervisor::take_presentation_changed);
        if changed || presentation_changed { self.publish()?; }
        if let Some(error) = self.ui.as_mut().and_then(Supervisor::take_error) {
            eprintln!("MicCamWatch Flutter desktop: {error}; native controls remain available");
        }
        Ok(changed)
    }
    fn take_open_failure(&mut self) -> Option<String> {
        self.ui.as_mut().and_then(crate::frontends::desktop::unix::Supervisor::take_open_failure)
    }
    fn finish(&mut self) {
        if let Some(ui) = self.ui.as_mut() { let _ = ui.close(); }
        // Persistence admitted by this process is not cancelled by a close or stop.
        while self.worker.as_ref().is_some_and(crate::frontends::desktop::preferences::PreferenceWorker::is_pending) {
            let _ = self.refresh();
            thread::sleep(Duration::from_millis(20));
        }
        self.ui.take();
    }
}
#[cfg(feature = "flutter-desktop")]
impl Drop for FlutterFrontend {
    fn drop(&mut self) { self.finish(); }
}

#[cfg(feature = "flutter-desktop")]
fn show_desktop(flutter: &mut FlutterFrontend, native: &mut DesktopHost) -> Result<()> {
    match flutter.open("system") {
        Ok(()) => Ok(()),
        Err(error) => {
            // A synchronous failure is handled here, not again on the next poll.
            let _ = flutter.take_open_failure();
            eprintln!("MicCamWatch Flutter unavailable, opening native desktop: {error:#}");
            native.show_window()
        }
    }
}

/// Explicit development modes never enter run_tray, resume protection, inspect
/// camera controls, dispatch capture alerts, or run automatic lock policies.
#[cfg(feature = "flutter-desktop")]
pub fn run_read_only_development(lang: Language, initial_window: bool, theme: &str) -> Result<()> {
    run_flutter_development(lang, initial_window, theme, false)
}
#[cfg(feature = "flutter-desktop")]
pub fn run_settings_development(lang: Language, initial_window: bool, theme: &str) -> Result<()> {
    run_flutter_development(lang, initial_window, theme, true)
}

#[cfg(feature = "flutter-desktop")]
fn development_paths() -> Result<Vec<File>> {
    use crate::frontends::desktop::unix::pin_directory_chain;
    let home = std::env::var_os("HOME").context("development HOME is required")?;
    let home = Path::new(&home);
    let mut pins = pin_directory_chain(home).context("pin development HOME ancestors")?;
    let home = home.canonicalize().context("resolve development HOME")?;
    // HOME may itself be redirected by the development launcher. Also exclude
    // the account's real normal trees, so that cannot disguise the stable store.
    let mut record: libc::passwd = unsafe { std::mem::zeroed() };
    let mut result = std::ptr::null_mut();
    let mut buffer = [0u8; 16_384];
    let status = unsafe {
        libc::getpwuid_r(uid(), &mut record, buffer.as_mut_ptr().cast(), buffer.len(), &mut result)
    };
    ensure!(status == 0 && !result.is_null() && !record.pw_dir.is_null(),
        "cannot identify the account home for development isolation");
    use std::os::unix::ffi::OsStrExt;
    let account = unsafe { std::ffi::CStr::from_ptr(record.pw_dir) };
    let account = Path::new(std::ffi::OsStr::from_bytes(account.to_bytes()));
    ensure!(account.is_absolute(), "account home is not absolute");
    pins.extend(pin_directory_chain(account).context("pin account home ancestors")?);
    let account = account.canonicalize().context("resolve account home")?;
    let mut roots = Vec::new();
    for (variable, fallback) in [("XDG_CONFIG_HOME", ".config"), ("XDG_DATA_HOME", ".local/share")] {
        let value = std::env::var_os(variable).with_context(|| format!("explicit private {variable} required"))?;
        let path = Path::new(&value);
        ensure!(path.is_absolute(), "{variable} must be absolute");
        // Validate the original environment pathname before resolving it: later
        // settings accesses use this spelling, not just its canonical target.
        pins.extend(pin_directory_chain(path).with_context(|| format!("pin {variable} ancestors"))?);
        let metadata = fs::symlink_metadata(path).with_context(|| format!("prepare a private {variable} directory first"))?;
        ensure!(metadata.is_dir() && !metadata.file_type().is_symlink()
            && metadata.uid() == uid() && metadata.mode() & 0o077 == 0,
            "{variable} must be a private owned directory, not a symlink");
        let root = path.canonicalize()?;
        for home in [&home, &account] {
            let normal = home.join(fallback);
            let normal = normal.canonicalize().unwrap_or(normal);
            ensure!(!root.starts_with(&normal) && !normal.starts_with(&root),
                "{variable} must be isolated from the normal preference/data tree");
        }
        let app = root.join("MicCamWatch");
        match fs::symlink_metadata(&app) {
            Ok(metadata) => ensure!(metadata.is_dir() && !metadata.file_type().is_symlink()
                && metadata.uid() == uid() && metadata.mode() & 0o022 == 0,
                "existing development app directory must be owned, non-writable by others, and not a symlink"),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        if variable == "XDG_CONFIG_HOME" {
            match fs::symlink_metadata(app.join("settings.toml")) {
                Ok(metadata) => ensure!(metadata.is_file() && !metadata.file_type().is_symlink()
                    && metadata.nlink() == 1 && metadata.uid() == uid() && metadata.mode() & 0o022 == 0,
                    "existing development settings must be an owned non-writable-by-others single-link file"),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        roots.push(root);
    }
    ensure!(!roots[0].starts_with(&roots[1]) && !roots[1].starts_with(&roots[0]),
        "development config/data roots must be separate");
    Ok(pins)
}

/// Build the real native presentation without probing control capabilities or
/// autostart. Qualification is a separate observer, never an operational owner.
#[cfg(feature = "flutter-desktop")]
fn qualification_view(lang: Language, snapshot: Option<&Snapshot>, writable: bool) -> View {
    let text = words(lang);
    let loaded = Settings::load();
    let settings = loaded.as_ref().cloned().unwrap_or_default();
    // AppKit protocol4 requires the four profile choices even when disabled.
    // Construct them here, not through make_view (which probes control/autostart).
    let mut profile = entry("profile", text.profile, false);
    profile.children = Some([
        "profile_balanced", "profile_private", "profile_meeting", "profile_development",
    ].into_iter().enumerate().map(|(index, action)| {
        let mut choice = entry(action, text.profiles[index], false);
        choice.checked = Some(index == profile_index(settings.profile));
        choice
    }).collect());
    let (summary, visual) = snapshot.map_or_else(
        || ("Isolated qualification — native observation unavailable".to_owned(), "partial"),
        |snapshot| observation_status(snapshot, lang),
    );
    let mut view = View {
        kind: "state",
        summary: summary.clone(),
        visual,
        items: vec![
            entry("header", "MicCamWatch — isolated qualification", false),
            entry("status", &summary, true),
            entry("mic", text.unavailable, false),
            entry("camera", text.unavailable, false),
            entry("pause", text.pause, false),
            profile,
            entry("autostart", text.unavailable, false),
            entry("exit", text.exit, true),
        ],
        diagnostics: summary,
        limited: false,
        language: lang.code(),
        version: env!("CARGO_PKG_VERSION"),
        protection: DesktopProtection {
            microphone: "unknown", camera: "unknown", requested: [None; 2],
            microphone_on_lock: false, camera_on_lock: false,
        },
        overview: snapshot.map_or_else(DesktopOverview::default, |snapshot|
            DesktopOverview::from_snapshot(snapshot, chrono::Utc::now())),
        preferences: DesktopPreferences {
            notifications_enabled: settings.notifications_enabled,
            writable: false,
            sound_enabled: settings.sound_enabled,
            history_enabled: settings.history_enabled,
            notifications_paused: settings.notifications_paused(),
        },
        feedback: Some(match loaded {
            Err(error) => format!("Isolated settings unavailable: {error:#}"),
            Ok(_) if writable => "Qualification only: preferences in Flutter; native mutations and all device/autostart commands disabled.".into(),
            Ok(_) => "Read-only qualification: all mutations disabled.".into(),
        }),
        #[cfg(target_os = "linux")]
        details_label: text.details,
    };
    bound_desktop_view(&mut view);
    view
}

/// Qualification admits presentation and owned shutdown only, regardless of
/// which disabled action an untrusted/misbehaving native peer sends.
#[cfg(feature = "flutter-desktop")]
fn dispatch_qualification_action(command: Command, open: &mut bool, stop: &AtomicBool) -> bool {
    match command {
        Command::Status => *open = true,
        Command::Exit => stop.store(true, Ordering::Release),
        _ => return false,
    }
    true
}

#[cfg(feature = "flutter-desktop")]
fn run_flutter_development(lang: Language, initial_window: bool, theme: &str, writable: bool) -> Result<()> {
    ensure!(matches!(theme, "system" | "light" | "dark"), "unsupported desktop theme");
    // Keep every ancestor pinned until the worker, frontend and lifecycle stop.
    let _directory_pins = development_paths()?;
    // Reuse the authenticated native singleton implementation unchanged, including
    // Linux SO_PEERPIDFD/image verification. The isolated XDG roots own this namespace.
    let Some(lifecycle) = Lifecycle::acquire_for_launch(initial_window, false)? else { return Ok(()) };
    let mut frontend = FlutterFrontend::new(lang, writable, writable, false);
    // Opt-in genuine native registration; never label window-only mode as tray
    // proof. The Mac host uses the same pinned helper/input as operational mode.
    #[cfg(target_os = "linux")]
    let (mut native, native_actions) = {
        let (sender, actions) = mpsc::sync_channel(32);
        let host = if std::env::var_os("MCW_LINUX_QUALIFICATION_TRAY").as_deref()
            == Some(std::ffi::OsStr::new("1"))
        {
            let host = DesktopHost::start(sender, lifecycle.ready.clone(),
                qualification_view(lang, None, writable))?;
            eprintln!("MicCamWatch qualification: actual SNI registration succeeded; device/autostart actions disabled");
            Some(host)
        } else { None };
        (host, actions)
    };
    #[cfg(target_os = "macos")]
    let (mut native, native_actions) = {
        let (sender, actions) = mpsc::sync_channel(32);
        let host = if std::env::var_os("MCW_MACOS_QUALIFICATION_TRAY").as_deref()
            == Some(std::ffi::OsStr::new("1"))
        {
            ensure!(!writable, "Mac native qualification requires read-only development");
            let host = DesktopHost::start(sender, lifecycle.ready.clone(),
                qualification_view(lang, None, writable), lifecycle.window_input.clone())?;
            eprintln!("MCW_MACOS_NATIVE event=ready owner={} pid={} protocol=4 mutations=disabled",
                std::process::id(), host.child.id());
            Some(host)
        } else { None };
        (host, actions)
    };
    #[cfg(target_os = "linux")]
    let native_only = std::env::var_os("MCW_LINUX_QUALIFICATION_NATIVE").as_deref()
        == Some(std::ffi::OsStr::new("1"));
    #[cfg(target_os = "linux")]
    ensure!(!native_only || native.is_some(), "native qualification requires the real SNI host");
    struct Reading {
        snapshot: Result<Snapshot, String>,
        at: chrono::DateTime<chrono::Utc>,
        applied_profile: Option<PrivacyProfile>,
    }
    let pending = Arc::new(std::sync::Mutex::new(None::<Reading>));
    let stop = Arc::new(AtomicBool::new(false));
    struct Observer {
        stop: Arc<AtomicBool>,
        thread: Option<thread::JoinHandle<()>>,
    }
    impl Observer {
        fn stop_and_join(&mut self) -> Result<()> {
            self.stop.store(true, Ordering::Release);
            if let Some(thread) = self.thread.take() {
                thread.join().map_err(|_| anyhow::anyhow!("development Unix observer panicked"))?;
            }
            Ok(())
        }
    }
    impl Drop for Observer {
        fn drop(&mut self) {
            if let Err(error) = self.stop_and_join() {
                eprintln!("MicCamWatch development observer shutdown failed: {error:#}");
            }
        }
    }
    let readings = pending.clone();
    let observer_stop = stop.clone();
    let observer_thread = thread::Builder::new().name("miccamwatch-unix-observer".into()).spawn(move || {
        let result = PlatformMonitor::new(Policy::default());
        let mut monitor = match result {
            Ok(monitor) => monitor,
            Err(error) => {
                *readings.lock().unwrap_or_else(|e| e.into_inner()) = Some(Reading {
                    snapshot: Err(format!("{error:#}")), at: chrono::Utc::now(), applied_profile: None,
                });
                return;
            }
        };
        let filter = Filter { include_ready: true, ..Filter::default() };
        while !observer_stop.load(Ordering::Acquire) {
            let profile = Settings::load().ok().map(|settings| settings.profile);
            if let Some(profile) = profile {
                monitor.set_policy(Policy { profile: profile.assessment_profile(), ..Policy::default() });
            }
            let snapshot = monitor.snapshot((&filter).into()).map_err(|error| format!("{error:#}"));
            *readings.lock().unwrap_or_else(|e| e.into_inner()) =
                Some(Reading { snapshot, at: chrono::Utc::now(), applied_profile: profile });
            let until = Instant::now() + INTERVAL;
            while !observer_stop.load(Ordering::Acquire) && Instant::now() < until {
                thread::sleep(Duration::from_millis(50));
            }
        }
    }).context("start real Unix collector")?;
    let mut observer = Observer { stop, thread: Some(observer_thread) };
    let outcome = (|| -> Result<()> {
        let mut open = initial_window;
        lifecycle.ready.store(true, Ordering::Release);
        while !lifecycle.stop.load(Ordering::Acquire) {
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            if let Some(host) = native.as_mut() {
                host.check()?;
                #[cfg(target_os = "linux")]
                if let Some(error) = host.take_window_error() {
                    eprintln!("MicCamWatch qualification native window: {error}");
                }
                while let Ok(command) = native_actions.try_recv() {
                    if !dispatch_qualification_action(command, &mut open, &lifecycle.stop) {
                        eprintln!("MicCamWatch qualification: rejected native mutation");
                    }
                    #[cfg(target_os = "macos")]
                    eprintln!("MCW_MACOS_NATIVE event=dispatch owner={} pid={} action={command:?} protocol=4",
                        std::process::id(), host.child.id());
                }
            }
            frontend.refresh()?;
            if let Some(error) = frontend.take_open_failure() {
                #[cfg(target_os = "linux")]
                if let Some(host) = native.as_mut() {
                    eprintln!("MicCamWatch qualification Flutter failed; opening native fallback: {error}");
                    host.show_window()?;
                } else {
                    bail!("development Flutter desktop failed before ready: {error}");
                }
                #[cfg(target_os = "macos")]
                bail!("development Flutter desktop failed before ready: {error}");
            }
            let requested = lifecycle.open_requested.swap(false, Ordering::AcqRel);
            #[cfg(target_os = "linux")]
            { open |= requested; }
            #[cfg(target_os = "macos")]
            if requested {
                if let Some(host) = native.as_mut() {
                    // Exercise the same native Open handler as menu-bar status,
                    // then consume its Status event in this supervisor loop.
                    host.check()?;
                    host.show_window()?;
                } else { open = true; }
            }
            if open {
                open = false;
                #[cfg(target_os = "linux")]
                if native_only {
                    native.as_mut().expect("required native host").show_window()?;
                } else if let Err(error) = frontend.open(theme) {
                    if let Some(host) = native.as_mut() {
                        eprintln!("MicCamWatch qualification Flutter unavailable; opening native fallback: {error:#}");
                        let _ = frontend.take_open_failure();
                        host.show_window()?;
                    } else { return Err(error); }
                }
                #[cfg(target_os = "macos")]
                frontend.open(theme)?;
            }
            if let Some(reading) = pending.lock().unwrap_or_else(|e| e.into_inner()).take() {
                #[cfg(target_os = "macos")]
                if let Some(host) = native.as_mut() {
                    host.update(qualification_view(lang, reading.snapshot.as_ref().ok(), writable))?;
                }
                #[cfg(target_os = "linux")]
                if let Some(host) = native.as_mut() {
                    host.observe(reading.snapshot.as_ref().ok());
                    host.update(qualification_view(lang, reading.snapshot.as_ref().ok(), writable))?;
                }
                frontend.applied_profile = reading.applied_profile;
                frontend.observe(reading.snapshot, reading.at);
                frontend.publish()?;
            }
            thread::sleep(Duration::from_millis(20));
        }
        Ok(())
    })();
    lifecycle.ready.store(false, Ordering::Release);
    frontend.finish();
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    if let Some(host) = native.as_mut() { host.shutdown()?; }
    #[cfg(target_os = "macos")]
    if let Some(host) = native.as_ref() {
        eprintln!("MCW_MACOS_NATIVE event=stopped owner={} pid={} protocol=4",
            std::process::id(), host.child.id());
    }
    let outcome = outcome.and(observer.stop_and_join());
    lifecycle.clean_shutdown.store(outcome.is_ok(), Ordering::Release);
    outcome
}

pub fn run_tray(
    mut monitor: PlatformMonitor,
    mut policy: Policy,
    lang: Language,
    mut settings: Settings,
    explicit_policy: bool,
    eventlog: bool,
    initial_window: bool,
) -> Result<()> {
    let Some(lifecycle) = Lifecycle::acquire_for_launch(initial_window, explicit_policy)? else {
        return Ok(());
    };
    #[cfg(feature = "flutter-desktop")]
    let mut flutter = FlutterFrontend::new(lang, true, !explicit_policy, true);
    let (sender, actions) = mpsc::sync_channel(32);
    let mut inspection = CameraInspectionWorker::default();
    #[cfg(target_os = "macos")]
    let mut overview = DesktopOverview::default();
    let initial = make_view(
        (words(lang).degraded.to_owned(), "error"),
        MicrophoneMuteState::Unavailable,
        (false, Some(words(lang).degraded)),
        &settings,
        lang,
        &None,
        MenuContext {
            lock: SessionLockState::Unknown,
            explicit_policy,
            inspection: &inspection.ui,
            #[cfg(target_os = "macos")]
            requested: [None; 2],
            #[cfg(target_os = "macos")]
            overview: &overview,
        },
    );
    #[cfg(target_os = "linux")]
    let mut host = DesktopHost::start(sender, lifecycle.ready.clone(), initial)?;
    #[cfg(target_os = "macos")]
    let mut host = DesktopHost::start(
        sender,
        lifecycle.ready.clone(),
        initial,
        lifecycle.window_input.clone(),
    )?;
    #[cfg(feature = "flutter-desktop")]
    if initial_window { show_desktop(&mut flutter, &mut host)?; }
    #[cfg(all(target_os = "macos", not(feature = "flutter-desktop")))]
    if initial_window {
        // The initial unavailable state is already native-ready. Opening must
        // not wait for the first potentially slow device/privacy observation.
        host.show_window()?;
    }
    let mut tracker = TransitionTracker::default();
    let mut dispatcher = EventDispatcher::new(
        lang,
        false,
        true,
        None,
        eventlog,
        settings.sound_enabled,
        settings.history_enabled,
    )?;
    let filter = Filter {
        include_ready: true,
        ..Filter::default()
    };
    let mut applied_profile = settings.profile;
    let mut lock_previous = SessionLockState::Unknown;
    let mut lock_intent = false;
    let mut feedback = None;
    let mut action_error = false;
    let mut summary = words(lang).degraded.to_owned();
    let mut visual = "error";
    let mut next_poll = Instant::now();
    let mut mute = MicrophoneMuteState::Unavailable;
    let mut camera_blocked = false;
    #[cfg(target_os = "macos")]
    let mut camera_pending_target = None;
    let mut camera_error: Option<String> = None;
    let mut lock = SessionLockState::Unknown;
    #[cfg(target_os = "macos")]
    let mut requested = [None; 2];
    #[cfg(all(target_os = "linux", not(feature = "flutter-desktop")))]
    let mut show_initial_window = initial_window;
    let outcome = (|| -> Result<()> {
        loop {
            if lifecycle.stop.load(Ordering::Acquire) {
                break;
            }
            host.check()?;
            #[cfg(feature = "flutter-desktop")]
            {
                if flutter.refresh()? { next_poll = Instant::now(); }
                if let Some(error) = flutter.take_open_failure() {
                    eprintln!("MicCamWatch Flutter failed before ready, opening native desktop: {error}");
                    if let Err(error) = host.show_window() {
                        feedback = Some(format!("{}: {error:#}", words(lang).failed));
                        action_error = true;
                    }
                    next_poll = Instant::now();
                }
            }
            #[cfg(target_os = "linux")]
            if let Some(error) = host.take_window_error() {
                feedback = Some(format!("{}: {error}", words(lang).failed));
                action_error = true;
                next_poll = Instant::now();
            }
            if lifecycle.open_requested.swap(false, Ordering::AcqRel) {
                #[cfg(feature = "flutter-desktop")]
                let opened = show_desktop(&mut flutter, &mut host);
                #[cfg(not(feature = "flutter-desktop"))]
                let opened = host.show_window();
                if let Err(error) = opened {
                    feedback = Some(format!("{}: {error:#}", words(lang).failed));
                    action_error = true;
                    next_poll = Instant::now();
                }
            }
            #[cfg(target_os = "macos")]
            if inspection.poll() {
                // Diagnostic completion never changes controls or manual action feedback.
                next_poll = Instant::now();
            }
            if Instant::now() >= next_poll {
                let settings_error = match Settings::load() {
                    Ok(current) => {
                        settings = current;
                        None
                    }
                    Err(error) => Some(format!("{error:#}")),
                };
                dispatcher.preferences(settings.sound_enabled, settings.history_enabled);
                if !explicit_policy && settings.profile != applied_profile {
                    // An explicit --config remains authoritative across preference changes.
                    policy.profile = match settings.profile {
                        PrivacyProfile::Private => crate::config::Profile::Strict,
                        PrivacyProfile::Development => crate::config::Profile::Conservative,
                        PrivacyProfile::Meeting | PrivacyProfile::Balanced => {
                            crate::config::Profile::Balanced
                        }
                    };
                    monitor.set_policy(policy.clone());
                    applied_profile = settings.profile;
                }
                #[cfg(feature = "flutter-desktop")]
                { flutter.applied_profile = (!explicit_policy).then_some(applied_profile); }
                lock = crate::platform::session_lock_state();
                if lock == SessionLockState::Locked
                    && lock_previous != SessionLockState::Locked
                    && settings.mute_on_lock
                {
                    // The monitor persists the intent before any mutation, including partial failures.
                    lock_intent = true;
                    #[cfg(target_os = "linux")]
                    let result = monitor.begin_lock_microphone_mute();
                    #[cfg(target_os = "macos")]
                    let result = monitor.begin_lock_mute();
                    if let Err(error) = result {
                        feedback = Some(format!("{}: {error:#}", words(lang).failed));
                        action_error = true;
                    }
                } else if lock == SessionLockState::Unlocked
                    && settings.restore_on_unlock
                    && ((lock_previous == SessionLockState::Locked && lock_intent)
                        || lock_previous == SessionLockState::Unknown)
                {
                    #[cfg(target_os = "linux")]
                    let result = monitor.restore_lock_microphone_mute();
                    #[cfg(target_os = "macos")]
                    let result = monitor.restore_lock_mute();
                    match result {
                        Ok(()) => lock_intent = false,
                        Err(error) => {
                            feedback = Some(format!("{}: {error:#}", words(lang).failed));
                            action_error = true;
                        }
                    }
                }
                // An unknown probe is never an unlock and does not discard a known lock.
                if lock != SessionLockState::Unknown {
                    lock_previous = lock;
                }
                match monitor.snapshot((&filter).into()) {
                    Ok(snapshot) => {
                        #[cfg(feature = "flutter-desktop")]
                        let observed_at = chrono::Utc::now();
                        #[cfg(target_os = "macos")]
                        {
                            overview =
                                DesktopOverview::from_snapshot(&snapshot, chrono::Utc::now());
                        }
                        #[cfg(target_os = "linux")]
                        host.observe(Some(&snapshot));
                        tracker.reconcile(&snapshot, |access, action| {
                            dispatcher.dispatch(access, action)
                        })?;
                        (summary, visual) = observation_status(&snapshot, lang);
                        #[cfg(feature = "flutter-desktop")]
                        flutter.observe(Ok(snapshot), observed_at);
                    }
                    Err(error) => {
                        #[cfg(feature = "flutter-desktop")]
                        flutter.observe(Err(format!("{error:#}")), chrono::Utc::now());
                        #[cfg(target_os = "macos")]
                        {
                            overview = DesktopOverview::default();
                        }
                        #[cfg(target_os = "linux")]
                        host.observe(None);
                        tracker.observation_gap();
                        visual = "error";
                        summary = format!("{}: {error:#}", words(lang).degraded);
                    }
                }
                match monitor.microphone_mute_state() {
                    Ok(state) => mute = state,
                    Err(error) => {
                        mute = MicrophoneMuteState::Unavailable;
                        let _ = write!(summary, "\n\n{}: {error:#}", words(lang).unavailable);
                        if visual != "error" {
                            visual = "partial";
                        }
                    }
                }
                #[cfg(target_os = "macos")]
                if matches!(
                    mute,
                    MicrophoneMuteState::Unavailable | MicrophoneMuteState::Mixed
                ) {
                    match monitor.microphone_control_detail() {
                        Ok(detail) => {
                            let _ = write!(summary, "\n\n{detail}");
                        }
                        Err(error) => {
                            let _ = write!(summary, "\n\n{}: {error:#}", words(lang).unavailable);
                        }
                    }
                }
                if let Some(error) = settings_error {
                    let _ = write!(
                        summary,
                        "\n\nSettings — {}: {error}",
                        words(lang).state_unknown
                    );
                    visual = "error";
                }
                #[cfg(target_os = "linux")]
                match crate::privacy::camera_state() {
                    Ok(state) => {
                        // Mixed/stale ownership still offers explicit restoration.
                        camera_blocked = state != crate::privacy::CameraPrivacyState::Allowed;
                        inspection.ui.system_managed =
                            state == crate::privacy::CameraPrivacyState::SystemManaged;
                        camera_error = None;
                    }
                    Err(error) => {
                        camera_error = Some(format!("{error:#}"));
                    }
                }
                #[cfg(target_os = "macos")]
                match crate::privacy::camera_state() {
                    Ok(state) => {
                        camera_blocked = state == crate::privacy::CameraPrivacyState::Blocked;
                        inspection.observe_passive_state(state);
                        camera_error = None;
                        if camera_pending_target == Some(camera_blocked) {
                            camera_pending_target = None;
                            feedback = Some(words(lang).done.to_owned());
                            action_error = false;
                        }
                    }
                    Err(error) => {
                        inspection.ui.inventory_incomplete =
                            error.is::<crate::privacy::CameraInventoryIncomplete>();
                        camera_error = Some(format!("{error:#}"));
                    }
                }
                next_poll = Instant::now() + INTERVAL;
                #[cfg(feature = "flutter-desktop")]
                flutter.publish()?;
                host.update(make_view(
                    (summary.clone(), if action_error { "error" } else { visual }),
                    mute,
                    (camera_blocked, camera_error.as_deref()),
                    &settings,
                    lang,
                    &feedback,
                    MenuContext {
                        lock,
                        explicit_policy,
                        inspection: &inspection.ui,
                        #[cfg(target_os = "macos")]
                        requested,
                        #[cfg(target_os = "macos")]
                        overview: &overview,
                    },
                ))?;
                #[cfg(all(target_os = "linux", not(feature = "flutter-desktop")))]
                if show_initial_window {
                    if let Err(error) = host.show_window() {
                        feedback = Some(format!("{}: {error:#}", words(lang).failed));
                        action_error = true;
                        next_poll = Instant::now();
                    }
                    show_initial_window = false;
                }
            }
            match actions.recv_timeout(Duration::from_millis(100)) {
                Ok(Command::Exit) => break,
                Ok(command) => {
                    if matches!(command, Command::Status) {
                        #[cfg(feature = "flutter-desktop")]
                        let opened = show_desktop(&mut flutter, &mut host);
                        #[cfg(not(feature = "flutter-desktop"))]
                        let opened = host.show_window();
                        if let Err(error) = opened {
                            feedback = Some(format!("{}: {error:#}", words(lang).failed));
                            action_error = true;
                            next_poll = Instant::now();
                        }
                        continue;
                    }
                    #[cfg(target_os = "linux")]
                    if !host.allows(command) {
                        feedback = Some(format!(
                            "{}: {}",
                            words(lang).failed,
                            words(lang).unavailable
                        ));
                        action_error = true;
                        next_poll = Instant::now();
                        continue;
                    }
                    #[cfg(target_os = "macos")]
                    if matches!(command, Command::CameraInspect) {
                        if let Err(error) = inspection.start(lang) {
                            inspection.ui.detail =
                                Some(format!("Read-only camera inspection failed: {error:#}"));
                        }
                        next_poll = Instant::now();
                        continue;
                    }
                    #[cfg(target_os = "macos")]
                    if !matches!(command, Command::Camera) {
                        camera_pending_target = None;
                    }
                    let result: Result<()> = (|| {
                        match command {
                            Command::Status => unreachable!(),
                            Command::Mic => {
                                #[cfg(target_os = "macos")]
                                {
                                    requested[0] = Some(mute != MicrophoneMuteState::Muted);
                                }
                                #[cfg(target_os = "linux")]
                                {
                                    host.requested[0] = Some(mute != MicrophoneMuteState::Muted);
                                }
                                monitor.toggle_microphone_mute()?;
                            }
                            Command::Camera => {
                                #[cfg(target_os = "macos")]
                                {
                                    camera_pending_target = Some(!camera_blocked);
                                    requested[1] = Some(!camera_blocked);
                                    camera_blocked = crate::privacy::toggle_camera()?
                                        == crate::privacy::CameraPrivacyState::Blocked;
                                    camera_pending_target = None;
                                }
                                #[cfg(target_os = "linux")]
                                {
                                    host.requested[1] = Some(!camera_blocked);
                                    let state = crate::privacy::toggle_camera_state()?;
                                    camera_blocked =
                                        state != crate::privacy::CameraPrivacyState::Allowed;
                                    inspection.ui.system_managed =
                                        state == crate::privacy::CameraPrivacyState::SystemManaged;
                                    camera_error = None;
                                }
                            }
                            Command::Pause => {
                                settings = Settings::update(|current| {
                                    current.pause_notifications_until =
                                        if current.notifications_paused() {
                                            None
                                        } else {
                                            Some(chrono::Utc::now() + chrono::Duration::minutes(30))
                                        };
                                })?;
                            }
                            Command::NotificationsEnable
                            | Command::NotificationsDisable
                            | Command::SoundEnable
                            | Command::SoundDisable
                            | Command::HistoryEnable
                            | Command::HistoryDisable => {
                                settings = select_desktop_preference(command)?;
                            }
                            Command::ProfileBalanced
                            | Command::ProfilePrivate
                            | Command::ProfileMeeting
                            | Command::ProfileDevelopment => {
                                settings = select_profile(command, explicit_policy, lang)?;
                            }
                            #[cfg(target_os = "macos")]
                            Command::CameraInspect => unreachable!(),
                            Command::Autostart => match crate::autostart::state()? {
                                crate::autostart::AutostartState::Enabled => {
                                    crate::autostart::disable()?
                                }
                                crate::autostart::AutostartState::Disabled => {
                                    crate::autostart::enable()?
                                }
                            },
                            Command::Exit => unreachable!(),
                        }
                        Ok(())
                    })();
                    match result {
                        Ok(()) => {
                            feedback = Some(words(lang).done.to_owned());
                            action_error = false;
                        }
                        Err(error) => {
                            feedback = Some(format!("{}: {error:#}", words(lang).failed));
                            action_error = true;
                        }
                    }
                    next_poll = Instant::now();
                    host.update(make_view(
                        (summary.clone(), if action_error { "error" } else { visual }),
                        mute,
                        (camera_blocked, camera_error.as_deref()),
                        &settings,
                        lang,
                        &feedback,
                        MenuContext {
                            lock,
                            explicit_policy,
                            inspection: &inspection.ui,
                            #[cfg(target_os = "macos")]
                            requested,
                            #[cfg(target_os = "macos")]
                            overview: &overview,
                        },
                    ))?;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    bail!("native tray action channel closed")
                }
            }
        }
        Ok(())
    })();
    lifecycle.ready.store(false, Ordering::Release);
    #[cfg(feature = "flutter-desktop")]
    flutter.finish();
    // Unknown or still-locked sessions never justify restoring lock-owned controls.
    // Manual operations invalidate stale lock restoration inside the platform monitor.
    let restore = if lock == SessionLockState::Unlocked && lock_intent && settings.restore_on_unlock
    {
        #[cfg(target_os = "linux")]
        {
            monitor.restore_lock_microphone_mute()
        }
        #[cfg(target_os = "macos")]
        {
            monitor.restore_lock_mute()
        }
    } else {
        Ok(())
    };
    let shutdown = host.shutdown();
    let result = outcome.and(restore).and(shutdown);
    lifecycle
        .clean_shutdown
        .store(result.is_ok(), Ordering::Release);
    result
}

#[cfg(target_os = "linux")]
fn diagnostic_document(view: &View) -> String {
    let capacity = view.diagnostics.len()
        + view
            .items
            .iter()
            .filter_map(|entry| {
                let original = entry.detail.as_deref().unwrap_or(&entry.label);
                (original != view.summary).then_some(original.len() + 2)
            })
            .sum::<usize>();
    let mut document = String::with_capacity(capacity);
    document.push_str(&view.diagnostics);
    for entry in &view.items {
        let original = entry.detail.as_deref().unwrap_or(&entry.label);
        if original != view.summary {
            document.push_str("\n\n");
            document.push_str(original);
        }
    }
    document
}
fn select_desktop_preference(command: Command) -> Result<Settings> {
    Settings::update(|current| match command {
        Command::NotificationsEnable => current.notifications_enabled = true,
        Command::NotificationsDisable => current.notifications_enabled = false,
        Command::SoundEnable => current.sound_enabled = true,
        Command::SoundDisable => current.sound_enabled = false,
        Command::HistoryEnable => current.history_enabled = true,
        Command::HistoryDisable => current.history_enabled = false,
        _ => unreachable!("not a desktop preference assignment"),
    })
}

fn select_profile(command: Command, explicit_policy: bool, lang: Language) -> Result<Settings> {
    ensure!(!explicit_policy, "{}", words(lang).config_override);
    let profile = match command {
        Command::ProfileBalanced => PrivacyProfile::Balanced,
        Command::ProfilePrivate => PrivacyProfile::Private,
        Command::ProfileMeeting => PrivacyProfile::Meeting,
        Command::ProfileDevelopment => PrivacyProfile::Development,
        _ => bail!("not a profile selection"),
    };
    Settings::update(|current| current.profile = profile)
}

// DBusMenu hosts choose their own fonts and geometry. Bound display cells, not
// alleged pixels; native proof measures the actual XFCE GTK popup separately.
#[cfg(target_os = "linux")]
fn linux_menu_label(label: &str) -> String {
    const CELLS: usize = 48;
    const SCALARS: usize = 96;
    let mut result = String::with_capacity((label.len() + CELLS).min(SCALARS * 4 + CELLS));
    let mut cells = 0;
    for (count, (index, character)) in label.char_indices().enumerate() {
        let visible = if character.is_whitespace() || character.is_control() {
            ' '
        } else {
            character
        };
        let width = if visible == ' ' {
            1
        } else {
            ratatui::text::Span::raw(&label[index..index + character.len_utf8()]).width()
        };
        if count == SCALARS || cells + width > CELLS - 1 {
            result.push('…');
            break;
        }
        cells += width;
        // DBusMenu uses underscores as mnemonics; double literal underscores.
        if visible == '_' {
            result.push('_');
        }
        result.push(visible);
    }
    result
}

#[cfg(target_os = "linux")]
fn linux_menu_entry(entry: &MenuEntry) -> ksni::MenuItem<LinuxTray> {
    if let Some(children) = &entry.children {
        return ksni::menu::SubMenu {
            label: linux_menu_label(&entry.label),
            enabled: entry.enabled,
            submenu: children.iter().map(linux_menu_entry).collect(),
            ..Default::default()
        }
        .into();
    }
    let command = Command::from_name(entry.action);
    let activate = Box::new(move |this: &mut LinuxTray| {
        if let Some(command) = command {
            let _ = this.actions.try_send(command);
        }
    });
    if let Some(checked) = entry.checked {
        ksni::menu::CheckmarkItem {
            label: linux_menu_label(&entry.label),
            enabled: entry.enabled,
            checked,
            activate,
            ..Default::default()
        }
        .into()
    } else {
        ksni::menu::StandardItem {
            label: linux_menu_label(&entry.label),
            enabled: entry.enabled,
            activate,
            ..Default::default()
        }
        .into()
    }
}

#[cfg(target_os = "linux")]
struct LinuxTray {
    view: View,
    actions: SyncSender<Command>,
    ready: Arc<AtomicBool>,
}
#[cfg(target_os = "linux")]
impl ksni::Tray for LinuxTray {
    fn id(&self) -> String {
        if self.view.preferences.writable { "miccamwatch" } else { "miccamwatch-qualification" }.into()
    }
    fn title(&self) -> String {
        if self.view.preferences.writable { "MicCamWatch" } else { "MicCamWatch — isolated qualification" }.into()
    }
    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        let color: [u8; 3] = match self.view.visual {
            "idle" => [35, 180, 90],
            "ready" => [235, 185, 25],
            "active" => [230, 55, 65],
            "partial" => [235, 145, 25],
            _ => [110, 110, 110],
        };
        let mut pixels = Vec::with_capacity(32 * 32 * 4);
        for y in 0i32..32 {
            for x in 0i32..32 {
                let inside = (x - 16).pow(2) + (y - 16).pow(2) <= 13 * 13;
                let glyph = match self.view.visual {
                    "error" | "partial" => {
                        (14..=17).contains(&x) && ((8..=19).contains(&y) || (23..=25).contains(&y))
                    }
                    "ready" => {
                        ((11..=13).contains(&x) || (19..=21).contains(&x)) && (9..=23).contains(&y)
                    }
                    "active" => (x - 16).pow(2) + (y - 16).pow(2) <= 5 * 5,
                    _ => {
                        (7..=15).contains(&x) && (y - x - 7).abs() <= 1
                            || (14..=25).contains(&x) && (y + x - 36).abs() <= 1
                    }
                };
                pixels.extend_from_slice(&[
                    if inside { 255 } else { 0 },
                    if glyph && inside { 255 } else { color[0] },
                    if glyph && inside { 255 } else { color[1] },
                    if glyph && inside { 255 } else { color[2] },
                ]);
            }
        }
        vec![ksni::Icon {
            width: 32,
            height: 32,
            data: pixels,
        }]
    }
    fn attention_icon_pixmap(&self) -> Vec<ksni::Icon> {
        self.icon_pixmap()
    }
    fn status(&self) -> ksni::Status {
        if matches!(self.view.visual, "error" | "partial") {
            ksni::Status::NeedsAttention
        } else {
            ksni::Status::Active
        }
    }
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        let mut menu: Vec<ksni::MenuItem<Self>> =
            self.view.items.iter().map(linux_menu_entry).collect();
        menu.insert(
            0,
            ksni::menu::StandardItem {
                label: linux_menu_label(self.view.details_label),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.actions.try_send(Command::Status);
                }),
                ..Default::default()
            }
            .into(),
        );
        menu
    }
    fn activate(&mut self, _: i32, _: i32) {
        let _ = self.actions.try_send(Command::Status);
    }
    fn secondary_activate(&mut self, _: i32, _: i32) {
        let _ = self.actions.try_send(Command::Status);
    }
    fn watcher_offline(&self, _: ksni::OfflineReason) -> bool {
        self.ready.store(false, Ordering::Release);
        false // Do not leave a headless process pretending to be a tray.
    }
}
#[cfg(target_os = "linux")]
#[path = "linux_desktop.rs"]
mod linux_desktop;

#[cfg(target_os = "linux")]
struct DesktopHost {
    handle: ksni::blocking::Handle<LinuxTray>,
    view: View,
    actions: SyncSender<Command>,
    window: Option<linux_desktop::Desktop>,
    overview: DesktopOverview,
    requested: [Option<bool>; 2],
}
#[cfg(target_os = "linux")]
impl DesktopHost {
    fn start(actions: SyncSender<Command>, ready: Arc<AtomicBool>, view: View) -> Result<Self> {
        use ksni::blocking::TrayMethods;
        // ksni's default registration verifies that an actual compatible SNI host exists.
        // In particular, do not use assume_sni_available on GNOME without AppIndicator.
        let handle = LinuxTray { view: view.clone(), actions: actions.clone(), ready: ready.clone() }.spawn()
            .context("native tray requires a session D-Bus and a registered StatusNotifierItem desktop host (e.g. KDE, or GNOME AppIndicator)")?;
        ensure!(
            !handle.is_closed(),
            "native desktop host disappeared during tray registration"
        );
        ready.store(true, Ordering::Release);
        Ok(Self {
            handle,
            view,
            actions,
            window: None,
            overview: DesktopOverview::default(),
            requested: [None; 2],
        })
    }
    fn observe(&mut self, snapshot: Option<&Snapshot>) {
        self.overview = snapshot.map_or_else(DesktopOverview::default, |snapshot| {
            DesktopOverview::from_snapshot(snapshot, chrono::Utc::now())
        });
    }
    fn update(&mut self, mut view: View) -> Result<()> {
        view.overview = self.overview.clone();
        view.protection.requested = self.requested;
        view.limited |= view.overview.text_limited;
        bound_desktop_view(&mut view);
        self.view = view.clone();
        if let Some(window) = &self.window {
            window.update(view.clone());
        }
        self.handle
            .update(|tray| tray.view = view)
            .context("Linux tray service closed")
    }
    fn show_window(&mut self) -> Result<()> {
        if self
            .window
            .as_ref()
            .is_some_and(linux_desktop::Desktop::finished)
        {
            self.window.take();
        }
        if let Some(window) = &self.window {
            window.open();
        } else {
            self.window = Some(linux_desktop::Desktop::start(
                self.view.clone(),
                self.actions.clone(),
            )?);
        }
        Ok(())
    }
    fn take_window_error(&self) -> Option<String> {
        self.window
            .as_ref()
            .and_then(linux_desktop::Desktop::take_error)
    }
    fn allows(&self, command: Command) -> bool {
        let action = match command {
            Command::Mic => "mic",
            Command::Camera => "camera",
            Command::Autostart => "autostart",
            Command::Pause => "pause",
            Command::ProfileBalanced
            | Command::ProfilePrivate
            | Command::ProfileMeeting
            | Command::ProfileDevelopment => "profile",
            _ => return true,
        };
        self.view
            .items
            .iter()
            .any(|entry| entry.action == action && entry.enabled)
    }
    fn check(&mut self) -> Result<()> {
        ensure!(
            !self.handle.is_closed(),
            "native tray service closed or desktop host disappeared"
        );
        Ok(())
    }
    fn shutdown(&mut self) -> Result<()> {
        self.window.take();
        if !self.handle.is_closed() {
            self.handle.shutdown().wait();
        }
        Ok(())
    }
}
#[cfg(target_os = "linux")]
impl Drop for DesktopHost {
    fn drop(&mut self) {
        self.window.take();
        if !self.handle.is_closed() {
            self.handle.shutdown().wait();
        }
    }
}

#[cfg(target_os = "macos")]
#[derive(Deserialize)]
struct DesktopEvent {
    kind: String,
    protocol: Option<u32>,
    action: Option<Command>,
    error: Option<String>,
}
#[cfg(target_os = "macos")]
struct DesktopHost {
    child: std::process::Child,
    stdin: Arc<DesktopInput>,
    events: Receiver<Result<DesktopEvent>>,
    ready: Arc<AtomicBool>,
}
#[cfg(target_os = "macos")]
impl DesktopHost {
    fn start(
        actions: SyncSender<Command>,
        ready: Arc<AtomicBool>,
        view: View,
        input: Arc<DesktopInput>,
    ) -> Result<Self> {
        let mut child = crate::platform::macos::desktop_process(&["desktop"])?;
        let stdin = child
            .stdin
            .take()
            .context("AppKit helper stdin unavailable")?;
        *input
            .writer
            .lock()
            .map_err(|_| anyhow::anyhow!("AppKit input lock poisoned"))? = Some(stdin);
        let stdout = child
            .stdout
            .take()
            .context("AppKit helper stdout unavailable")?;
        let (sender, events) = mpsc::sync_channel(32);
        let readiness = ready.clone();
        let readiness_input = input.clone();
        thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let result = (|| -> Result<()> {
                while let Some(frame) = read_frame(&mut reader, FRAME_LIMIT)? {
                    let event: DesktopEvent = serde_json::from_slice(&frame)?;
                    if event.kind == "action" {
                        ensure!(
                            readiness.load(Ordering::Acquire),
                            "action before native readiness"
                        );
                        let action = event.action.context("native action lacks a command")?;
                        actions
                            .send(action)
                            .context("tray action receiver closed")?;
                    } else {
                        if event.kind == "ready" && event.protocol == Some(4) {
                            readiness.store(true, Ordering::Release);
                        }
                        if event.kind == "error" || event.kind == "stopped" {
                            readiness_input.close();
                            readiness.store(false, Ordering::Release);
                        }
                        sender
                            .send(Ok(event))
                            .context("tray event receiver closed")?;
                    }
                }
                bail!("AppKit helper closed its event stream")
            })();
            readiness_input.close();
            readiness.store(false, Ordering::Release);
            if let Err(error) = result {
                let _ = sender.send(Err(error));
            }
        });
        let mut host = Self {
            child,
            stdin: input,
            events,
            ready,
        };
        host.update(view)?;
        match host
            .events
            .recv_timeout(Duration::from_secs(15))
            .context("AppKit native readiness timed out")??
        {
            DesktopEvent {
                kind,
                protocol: Some(4),
                ..
            } if kind == "ready" => Ok(host),
            event => bail!(
                "AppKit status registration failed: {}",
                event.error.unwrap_or(event.kind)
            ),
        }
    }
    fn update(&mut self, view: View) -> Result<()> {
        let mut frame = serde_json::to_vec(&view)?;
        ensure!(
            frame.len() <= FRAME_LIMIT,
            "bounded desktop projection exceeded frame budget"
        );
        frame.push(b'\n');
        self.stdin.write(&frame)?;
        Ok(())
    }
    fn show_window(&mut self) -> Result<()> {
        self.stdin.write(b"{\"kind\":\"show\"}\n")?;
        Ok(())
    }
    fn check(&mut self) -> Result<()> {
        if let Ok(event) = self.events.try_recv() {
            let event = event?;
            bail!(
                "AppKit event stream closed or violated protocol: {}",
                event.error.unwrap_or(event.kind)
            );
        }
        ensure!(
            self.ready.load(Ordering::Acquire),
            "AppKit status service disconnected"
        );
        ensure!(
            self.child.try_wait()?.is_none(),
            "AppKit status helper exited"
        );
        Ok(())
    }
    fn shutdown(&mut self) -> Result<()> {
        self.stdin.closed.store(true, Ordering::Release);
        self.ready.store(false, Ordering::Release);
        let input = self
            .stdin
            .writer
            .lock()
            .map_err(|_| anyhow::anyhow!("AppKit input lock poisoned"))?
            .take();
        if let Some(mut stdin) = input {
            stdin.write_all(b"{\"kind\":\"stop\"}\n")?;
            drop(stdin);
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = self.child.try_wait()? {
                ensure!(status.success(), "AppKit helper exit: {status}");
                return Ok(());
            }
            if Instant::now() >= deadline {
                self.child.kill()?;
                let _ = self.child.wait();
                bail!("AppKit helper did not acknowledge clean shutdown");
            }
            thread::sleep(Duration::from_millis(20));
        }
    }
}
#[cfg(target_os = "macos")]
impl Drop for DesktopHost {
    fn drop(&mut self) {
        self.ready.store(false, Ordering::Release);
        self.stdin.close();
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod menu_tests {
    use super::*;
    use crate::model::{Access, CollectorHealth, Confidence, EnforcementDecision, Resource, Risk};

    fn run_isolated_menu_fixture(test: &str) -> bool {
        if std::env::var("MCW_TEST_MENU_CASE").as_deref() == Ok(test) {
            return false;
        }
        // make_view reads real autostart state, whose ownership store creates
        // directories even for a status query. Isolate that production path in
        // a child, never by changing process-global HOME during parallel tests.
        let directory = tempfile::Builder::new()
            .prefix("mcw-tray-")
            .tempdir_in("/tmp")
            .unwrap();
        let home = directory.path().canonicalize().unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                test,
                "--include-ignored",
                "--test-threads=1",
                "--nocapture",
            ])
            .env("MCW_TEST_MENU_CASE", test)
            .env("HOME", &home)
            .env("XDG_CONFIG_HOME", home.join("config"))
            .env("XDG_DATA_HOME", home.join("data"))
            .env("XDG_STATE_HOME", home.join("state"))
            .env("XDG_RUNTIME_DIR", home.join("runtime"))
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap();
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("running 1 test"),
            "isolated menu fixture {test} must execute exactly one selected test: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(
            output.status.success(),
            "isolated menu fixture {test} failed: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        true
    }

    #[cfg(feature = "flutter-desktop")]
    #[test]
    fn qualification_native_view_has_no_mutating_actions() {
        let test = "frontends::tray::menu_tests::qualification_native_view_has_no_mutating_actions";
        if run_isolated_menu_fixture(test) { return; }
        for writable in [false, true] {
            let view = qualification_view(Language::En, None, writable);
            assert!(!view.preferences.writable);
            assert_eq!(view.protection.requested, [None; 2]);
            assert_eq!(view.protection.microphone, "unknown");
            assert_eq!(view.protection.camera, "unknown");
            for item in &view.items {
                assert_eq!(item.enabled, matches!(item.action, "status" | "exit"));
                if item.action == "profile" {
                    let choices = item.children.as_ref().unwrap();
                    assert_eq!(choices.len(), 4);
                    assert!(choices.iter().all(|choice| !choice.enabled && choice.checked.is_some()
                        && choice.children.is_none()));
                } else { assert!(item.children.is_none()); }
            }
        }
        let home = PathBuf::from(std::env::var_os("HOME").unwrap());
        assert!(!home.join(".config/autostart").exists());
    }

    #[cfg(feature = "flutter-desktop")]
    #[test]
    fn qualification_dispatch_rejects_every_native_mutation() {
        let mut open = false;
        let stop = AtomicBool::new(false);
        let mutations = [
            Command::Mic, Command::Camera, Command::Pause, Command::ProfileBalanced,
            Command::ProfilePrivate, Command::ProfileMeeting, Command::ProfileDevelopment,
            Command::NotificationsEnable, Command::NotificationsDisable, Command::SoundEnable,
            Command::SoundDisable, Command::HistoryEnable, Command::HistoryDisable, Command::Autostart,
            #[cfg(target_os = "macos")]
            Command::CameraInspect,
        ];
        for command in mutations {
            assert!(!dispatch_qualification_action(command, &mut open, &stop));
            assert!(!open);
            assert!(!stop.load(Ordering::Acquire));
        }
        assert!(dispatch_qualification_action(Command::Status, &mut open, &stop));
        assert!(open);
        assert!(!stop.load(Ordering::Acquire));
        open = false;
        assert!(dispatch_qualification_action(Command::Exit, &mut open, &stop));
        assert!(!open);
        assert!(stop.load(Ordering::Acquire));
    }

    fn held_startup_lease() -> (PathBuf, File) {
        let path = socket_path().unwrap();
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(path.with_extension("lock"))
            .unwrap();
        assert_eq!(
            unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
            0
        );
        assert!(matches!(
            Lifecycle::acquire().unwrap(),
            LeaseAcquisition::Contended
        ));
        assert!(!path.exists());
        (path, lock)
    }

    #[test]
    fn cold_explicit_launch_waits_for_socket_and_queues_open() {
        if run_isolated_menu_fixture(
            "frontends::tray::menu_tests::cold_explicit_launch_waits_for_socket_and_queues_open",
        ) {
            return;
        }
        let (path, lock) = held_startup_lease();
        let (sent, received) = mpsc::channel();
        let client = thread::spawn(move || {
            assert!(
                sent.send(Lifecycle::acquire_for_launch(true, false))
                    .is_ok()
            );
        });
        assert!(received.recv_timeout(Duration::from_millis(100)).is_err());
        let owner = Lifecycle::bind(path, lock).unwrap();
        assert!(!owner.ready.load(Ordering::Acquire));
        assert!(
            received
                .recv_timeout(Duration::from_secs(5))
                .unwrap()
                .unwrap()
                .is_none()
        );
        assert!(owner.open_requested.load(Ordering::Acquire));
        client.join().unwrap();
    }

    #[test]
    fn cold_background_duplicate_waits_for_native_readiness_without_opening() {
        if run_isolated_menu_fixture(
            "frontends::tray::menu_tests::cold_background_duplicate_waits_for_native_readiness_without_opening",
        ) {
            return;
        }
        let (path, lock) = held_startup_lease();
        let (sent, received) = mpsc::channel();
        let client = thread::spawn(move || {
            assert!(
                sent.send(Lifecycle::acquire_for_launch(false, false))
                    .is_ok()
            );
        });
        assert!(received.recv_timeout(Duration::from_millis(100)).is_err());
        let owner = Lifecycle::bind(path, lock).unwrap();
        assert!(received.recv_timeout(Duration::from_millis(100)).is_err());
        assert!(!owner.open_requested.load(Ordering::Acquire));
        owner.ready.store(true, Ordering::Release);
        assert!(
            received
                .recv_timeout(Duration::from_secs(5))
                .unwrap()
                .unwrap()
                .is_none()
        );
        assert!(!owner.open_requested.load(Ordering::Acquire));
        client.join().unwrap();
    }

    #[test]
    fn cold_launch_reacquires_released_lease_and_rejects_unsafe_lock() {
        if run_isolated_menu_fixture(
            "frontends::tray::menu_tests::cold_launch_reacquires_released_lease_and_rejects_unsafe_lock",
        ) {
            return;
        }
        let (path, lock) = held_startup_lease();
        let (sent, received) = mpsc::channel();
        let client = thread::spawn(move || {
            assert!(
                sent.send(Lifecycle::acquire_for_launch(true, false))
                    .is_ok()
            );
        });
        assert!(received.recv_timeout(Duration::from_millis(100)).is_err());
        drop(lock);
        let replacement = received
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap();
        assert!(replacement.is_some());
        drop(replacement);
        client.join().unwrap();
        fs::set_permissions(
            path.with_extension("lock"),
            fs::Permissions::from_mode(0o666),
        )
        .unwrap();
        assert!(Lifecycle::acquire_for_launch(true, false).is_err());
    }

    #[test]
    fn cold_launch_refuses_foreign_namespace_without_sending_open() {
        if run_isolated_menu_fixture(
            "frontends::tray::menu_tests::cold_launch_refuses_foreign_namespace_without_sending_open",
        ) {
            return;
        }
        let (path, _lock) = held_startup_lease();
        let listener = UnixListener::bind(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            assert_eq!(read_frame(&mut reader, 16).unwrap().unwrap(), b"owner");
            let mut namespace = ControlNamespace::current().unwrap();
            namespace.config.push("foreign");
            serde_json::to_writer(&mut stream, &namespace).unwrap();
            stream.write_all(b"\n").unwrap();
            assert!(read_frame(&mut reader, 16).unwrap().is_none());
        });
        assert!(Lifecycle::acquire_for_launch(true, false).is_err());
        server.join().unwrap();
        fs::remove_file(path).unwrap();
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn authenticated_open_refuses_closed_native_input_despite_stale_readiness() {
        if run_isolated_menu_fixture(
            "frontends::tray::menu_tests::authenticated_open_refuses_closed_native_input_despite_stale_readiness",
        ) {
            return;
        }
        let owner = match Lifecycle::acquire().unwrap() {
            LeaseAcquisition::Acquired(owner) => owner,
            LeaseAcquisition::Contended => panic!("isolated owner lease was contended"),
        };
        owner.ready.store(true, Ordering::Release);
        owner.window_input.close();
        // A stale readiness observation must not acknowledge an open against
        // a disconnected native transport or leave it queued for later.
        assert!(owner.ready.load(Ordering::Acquire));
        assert!(open_running().is_err());
        assert!(!owner.open_requested.load(Ordering::Acquire));
    }

    #[test]
    fn control_socket_is_private_until_prepared_publication() {
        if run_isolated_menu_fixture(
            "frontends::tray::menu_tests::control_socket_is_private_until_prepared_publication",
        ) {
            return;
        }
        let destination = socket_path().unwrap();
        let mut staged = StagedSocket::new(&destination).unwrap();
        let directory = staged.directory.clone();
        assert_eq!(
            fs::symlink_metadata(&directory).unwrap().mode() & 0o777,
            0o700
        );
        let listener = staged.bind().unwrap();
        assert_eq!(
            fs::symlink_metadata(&staged.path).unwrap().mode() & 0o777,
            0o600
        );
        let flags = unsafe { libc::fcntl(listener.as_raw_fd(), libc::F_GETFL) };
        assert!(flags >= 0 && flags & libc::O_NONBLOCK != 0);
        assert!(connect_control().unwrap().is_none());
        staged.publish(&destination).unwrap();
        let identity = staged.socket_identity.unwrap();
        drop(staged);
        assert!(!directory.exists());
        let consumer = connect_control()
            .unwrap()
            .expect("published socket was unavailable to the real consumer");
        let (peer, _) = listener.accept().unwrap();
        verify_peer(&peer).unwrap();
        drop(consumer);
        drop(peer);
        drop(listener);
        remove_owned_socket(&destination, identity);
        assert!(!destination.exists());
    }

    #[test]
    fn socket_publication_refuses_foreign_destination_and_cleans_own_staging() {
        if run_isolated_menu_fixture(
            "frontends::tray::menu_tests::socket_publication_refuses_foreign_destination_and_cleans_own_staging",
        ) {
            return;
        }
        let destination = socket_path().unwrap();
        let mut staged = StagedSocket::new(&destination).unwrap();
        let directory = staged.directory.clone();
        let listener = staged.bind().unwrap();
        let mut foreign = OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&destination)
            .unwrap();
        foreign.write_all(b"unrelated destination").unwrap();
        let before = foreign.metadata().unwrap();
        assert!(staged.publish(&destination).is_err());
        assert!(connect_control().is_err());
        let after = fs::symlink_metadata(&destination).unwrap();
        assert_eq!((before.dev(), before.ino()), (after.dev(), after.ino()));
        assert_eq!(fs::read(&destination).unwrap(), b"unrelated destination");
        drop(staged);
        drop(listener);
        assert!(!directory.exists());
        assert!(destination.is_file());
    }

    #[test]
    fn lifecycle_cleanup_preserves_replacement_socket() {
        if run_isolated_menu_fixture(
            "frontends::tray::menu_tests::lifecycle_cleanup_preserves_replacement_socket",
        ) {
            return;
        }
        let owner = match Lifecycle::acquire().unwrap() {
            LeaseAcquisition::Acquired(owner) => owner,
            LeaseAcquisition::Contended => panic!("isolated owner lease was contended"),
        };
        let destination = owner.path.clone();
        let retired = destination.with_extension("retired");
        fs::rename(&destination, &retired).unwrap();
        let foreign = UnixListener::bind(&destination).unwrap();
        fs::set_permissions(&destination, fs::Permissions::from_mode(0o600)).unwrap();
        let before = fs::symlink_metadata(&destination).unwrap();
        drop(owner);
        let after = fs::symlink_metadata(&destination).unwrap();
        assert_eq!((before.dev(), before.ino()), (after.dev(), after.ino()));
        drop(foreign);
        fs::remove_file(destination).unwrap();
        fs::remove_file(retired).unwrap();
    }

    fn observed() -> Snapshot {
        Snapshot {
            collectors: vec![
                CollectorHealth {
                    collector: if cfg!(target_os = "linux") {
                        "pipewire_video"
                    } else {
                        "coremediaio_video"
                    },
                    state: CollectorState::Degraded,
                    detail: Some("running-state does not identify the client".into()),
                },
                CollectorHealth {
                    collector: if cfg!(target_os = "linux") {
                        "pipewire_audio"
                    } else {
                        "coreaudio_input"
                    },
                    state: CollectorState::Healthy,
                    detail: None,
                },
            ],
            accesses: vec![Access {
                key: "input-client".into(),
                resource: Resource::Microphone,
                activity: Activity::Active,
                application: "corespeechd".into(),
                pid: Some(42),
                parent_pid: None,
                parent_name: None,
                risk: Risk::Unexplained,
                confidence: Confidence::High,
                enforcement: EnforcementDecision::Unknown,
                executable: None,
                signature: None,
                device: None,
                started_at: None,
                modules: vec![],
                evidence: vec![],
                process: None,
            }],
            observation_gaps: vec![],
        }
    }

    #[test]
    fn partial_observation_preserves_activity_and_explains_collector_then_recovers() {
        let mut snapshot = observed();
        let (summary, visual) = observation_status(&snapshot, Language::En);
        assert_eq!(visual, "partial");
        let (first, detail) = summary.split_once('\n').unwrap();
        #[cfg(target_os = "linux")]
        assert!(first.contains("corespeechd"));
        #[cfg(target_os = "macos")]
        assert!(first.contains("MIC: ACTIVE") && summary.contains("corespeechd"));
        assert!(detail.contains(snapshot.collectors[0].collector));
        assert!(detail.contains(snapshot.collectors[0].detail.as_deref().unwrap()));
        snapshot.collectors[0].state = CollectorState::Healthy;
        assert_eq!(observation_status(&snapshot, Language::En).1, "active");
        snapshot.accesses.clear();
        snapshot.observation_gaps.push(Resource::Camera);
        assert_eq!(observation_status(&snapshot, Language::En).1, "partial");
        snapshot.collectors[0].state = CollectorState::Unavailable;
        assert_eq!(observation_status(&snapshot, Language::En).1, "error");
    }

    #[test]
    fn unavailable_collector_does_not_erase_observed_capture() {
        let mut snapshot = observed();
        snapshot.collectors[0].state = CollectorState::Unavailable;
        let (summary, visual) = observation_status(&snapshot, Language::En);
        assert_eq!(visual, "error");
        assert!(summary.contains("corespeechd"));
        #[cfg(target_os = "macos")]
        assert!(summary.lines().next().unwrap().contains("MIC: ACTIVE"));
    }

    #[test]
    fn current_overview_requires_resource_coverage_and_retains_positive_observations() {
        let observed_at = chrono::DateTime::parse_from_rfc3339("2026-10-22T12:34:56Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let mut snapshot = observed();
        snapshot.accesses[0].activity = Activity::Ready;
        let overview = DesktopOverview::from_snapshot(&snapshot, observed_at);
        assert_eq!(overview.microphone, "ready");
        assert_eq!(overview.camera, "unknown");
        assert_eq!(
            overview.observed_at.as_deref(),
            Some("2026-10-22T12:34:56Z")
        );
        snapshot.accesses.clear();
        let overview = DesktopOverview::from_snapshot(&snapshot, observed_at);
        assert_eq!(overview.microphone, "idle");
        assert_eq!(overview.camera, "unknown");
        snapshot.observation_gaps.push(Resource::Microphone);
        assert_eq!(
            DesktopOverview::from_snapshot(&snapshot, observed_at).microphone,
            "unknown"
        );
        snapshot.observation_gaps.clear();
        snapshot.collectors[1].state = CollectorState::Degraded;
        assert_eq!(
            DesktopOverview::from_snapshot(&snapshot, observed_at).microphone,
            "unknown"
        );
        snapshot.collectors[1].state = CollectorState::Unavailable;
        assert_eq!(
            DesktopOverview::from_snapshot(&snapshot, observed_at).microphone,
            "unknown"
        );
        snapshot.collectors.remove(1);
        assert_eq!(
            DesktopOverview::from_snapshot(&snapshot, observed_at).microphone,
            "unknown"
        );
        assert_eq!(observation_status(&snapshot, Language::En).1, "error");
        snapshot.accesses = observed().accesses;
        assert_eq!(
            DesktopOverview::from_snapshot(&snapshot, observed_at).microphone,
            "active"
        );
        let cleared = DesktopOverview::default();
        assert_eq!(cleared.microphone, "unknown");
        assert_eq!(cleared.camera, "unknown");
        assert!(cleared.observed_at.is_none());
        assert_eq!(cleared.total_accesses, 0);
        assert_eq!(cleared.omitted_accesses, 0);
        assert!(cleared.accesses.is_empty());
    }

    #[test]
    fn desktop_preference_assignments_are_idempotent_and_merge_latest_settings() {
        if run_isolated_menu_fixture(
            "frontends::tray::menu_tests::desktop_preference_assignments_are_idempotent_and_merge_latest_settings",
        ) {
            return;
        }
        Settings::update(|current| {
            current.show_ready = true;
            current.mute_on_lock = true;
            current.block_camera_on_lock = true;
            current.restore_on_unlock = false;
            current.pause_notifications_until =
                Some(chrono::Utc::now() + chrono::Duration::minutes(30));
        })
        .unwrap();
        for (action, field, enabled) in [
            ("notifications_disable", "notifications_enabled", false),
            ("notifications_enable", "notifications_enabled", true),
            ("sound_enable", "sound_enabled", true),
            ("sound_disable", "sound_enabled", false),
            ("history_disable", "history_enabled", false),
            ("history_enable", "history_enabled", true),
        ] {
            // A concurrent settings writer's latest unrelated value must survive.
            let latest =
                Settings::update(|current| current.profile = PrivacyProfile::Private).unwrap();
            let mut expected = serde_json::to_value(latest).unwrap();
            expected[field] = serde_json::json!(enabled);
            let command: Command = serde_json::from_value(serde_json::json!(action)).unwrap();
            for _ in 0..2 {
                let saved = select_desktop_preference(command).unwrap();
                assert_eq!(serde_json::to_value(saved).unwrap(), expected);
                assert_eq!(
                    serde_json::to_value(Settings::load().unwrap()).unwrap(),
                    expected
                );
            }
            assert!(select_profile(Command::ProfileBalanced, true, Language::En).is_err());
            assert_eq!(
                serde_json::to_value(Settings::load().unwrap()).unwrap(),
                expected
            );
        }
    }

    #[cfg(target_os = "macos")]
    fn incomplete_inspection_worker(
        authorization: crate::privacy::CameraInspectionAuthorization,
    ) -> CameraInspectionWorker {
        let (sender, receiver) = mpsc::sync_channel(1);
        sender
            .send(Ok(crate::privacy::CameraInspection {
                observed_at: chrono::DateTime::parse_from_rfc3339("2026-10-22T12:34:56Z")
                    .unwrap()
                    .with_timezone(&chrono::Utc),
                state: "unknown",
                complete_inventory: false,
                administrator_required: true,
                authorization,
                detail: "Simulated incomplete inventory; no physical camera proof.".to_owned(),
            }))
            .unwrap();
        let mut worker = CameraInspectionWorker {
            pending: Some(receiver),
            ui: CameraInspectionUi {
                inventory_incomplete: true,
                running: true,
                ..Default::default()
            },
        };
        assert!(worker.poll());
        assert!(worker.pending.is_none());
        assert!(!worker.ui.running);
        assert!(worker.ui.administrator_required);
        worker
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn passive_recovery_retires_inspection_guidance_without_erasing_historical_evidence() {
        if run_isolated_menu_fixture(
            "frontends::tray::menu_tests::passive_recovery_retires_inspection_guidance_without_erasing_historical_evidence",
        ) {
            return;
        }
        for authorization in [
            crate::privacy::CameraInspectionAuthorization::Cancelled,
            crate::privacy::CameraInspectionAuthorization::Denied,
        ] {
            for passive in [
                crate::privacy::CameraPrivacyState::Allowed,
                crate::privacy::CameraPrivacyState::Blocked,
                crate::privacy::CameraPrivacyState::SystemManaged,
            ] {
                let mut worker = incomplete_inspection_worker(authorization);
                let historical = worker.ui.detail.clone();
                worker.observe_passive_state(passive);
                assert!(!worker.ui.inventory_incomplete);
                assert!(!worker.ui.administrator_required);
                assert_eq!(worker.ui.detail, historical);
                let feedback = Some("Unrelated earlier action failed".to_owned());
                let snapshot = observed();
                let overview = DesktopOverview::from_snapshot(&snapshot, chrono::Utc::now());
                let frame = make_view(
                    observation_status(&snapshot, Language::En),
                    MicrophoneMuteState::Unmuted,
                    (passive == crate::privacy::CameraPrivacyState::Blocked, None),
                    &Settings::default(),
                    Language::En,
                    &feedback,
                    MenuContext {
                        lock: SessionLockState::Unknown,
                        explicit_policy: false,
                        inspection: &worker.ui,
                        requested: [None; 2],
                        overview: &overview,
                    },
                );
                assert!(
                    !frame
                        .items
                        .iter()
                        .any(|entry| entry.action == "camera_inspect")
                );
                assert!(frame.items.iter().any(|entry| entry.detail == historical));
                assert_eq!(frame.feedback, feedback);
                assert_eq!(
                    frame.protection.camera,
                    match passive {
                        crate::privacy::CameraPrivacyState::Allowed => "absent",
                        crate::privacy::CameraPrivacyState::Blocked => "owned",
                        crate::privacy::CameraPrivacyState::SystemManaged => "system_managed",
                    }
                );
            }
        }
    }

    #[test]
    fn desktop_projection_bounds_current_rows_utf8_and_escaped_text() {
        if run_isolated_menu_fixture(
            "frontends::tray::menu_tests::desktop_projection_bounds_current_rows_utf8_and_escaped_text",
        ) {
            return;
        }
        let mut snapshot = observed();
        let template = snapshot.accesses[0].clone();
        snapshot.accesses = (0..12)
            .map(|index| {
                let mut access = template.clone();
                access.application = format!("{index}:{}", "\u{1}\\\"界".repeat(2000));
                access.pid = None;
                access
            })
            .collect();
        let overview = DesktopOverview::from_snapshot(&snapshot, chrono::Utc::now());
        let feedback = Some("\u{1}\\\"界".repeat(10_000));
        let frame = make_view(
            observation_status(&snapshot, Language::En),
            MicrophoneMuteState::Unavailable,
            (false, Some("current camera inventory unknown")),
            &Settings::default(),
            Language::En,
            &feedback,
            MenuContext {
                lock: SessionLockState::Unknown,
                explicit_policy: false,
                inspection: &CameraInspectionUi::default(),
                #[cfg(target_os = "macos")]
                requested: [None; 2],
                #[cfg(target_os = "macos")]
                overview: &overview,
            },
        );
        #[cfg(target_os = "linux")]
        let frame = {
            let mut frame = frame;
            frame.overview = overview;
            bound_desktop_view(&mut frame);
            frame
        };
        assert!(frame.limited);
        assert_eq!(frame.overview.total_accesses, 12);
        assert_eq!(frame.overview.accesses.len(), 8);
        assert_eq!(frame.overview.omitted_accesses, 4);
        for access in &frame.overview.accesses {
            assert!(access.application.len() <= 256);
            assert!(access.application.ends_with(DESKTOP_OMISSION));
        }
        assert!(frame.feedback.as_ref().unwrap().len() <= 2048);
        assert!(frame.feedback.as_ref().unwrap().ends_with(DESKTOP_OMISSION));
        fn menu_cost(entries: &[MenuEntry]) -> usize {
            entries
                .iter()
                .map(|entry| {
                    desktop_text_cost(&entry.label)
                        + entry.detail.as_deref().map_or(0, desktop_text_cost)
                        + entry.children.as_deref().map_or(0, menu_cost)
                })
                .sum()
        }
        let cost = desktop_text_cost(&frame.summary)
            + desktop_text_cost(&frame.diagnostics)
            + menu_cost(&frame.items)
            + frame
                .overview
                .accesses
                .iter()
                .map(|access| desktop_text_cost(&access.application))
                .sum::<usize>()
            + desktop_text_cost(frame.feedback.as_ref().unwrap())
            + desktop_text_cost(frame.overview.observed_at.as_ref().unwrap());
        assert!(cost <= DESKTOP_TEXT_BUDGET);
        assert!(serde_json::to_vec(&frame).unwrap().len() <= FRAME_LIMIT);
        assert!(serde_json::to_value(&frame).unwrap()["overview"]["accesses"][0]["pid"].is_null());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn consultation_keeps_requested_effective_and_observation_independent() {
        if run_isolated_menu_fixture(
            "frontends::tray::menu_tests::consultation_keeps_requested_effective_and_observation_independent",
        ) {
            return;
        }
        let settings = Settings::default();
        let mut inspection = CameraInspectionUi::default();
        let state = make_view(
            observation_status(&observed(), Language::En),
            MicrophoneMuteState::Mixed,
            (false, Some("device-wide profile inventory unavailable")),
            &settings,
            Language::En,
            &Some("Partial action: original control state retained".into()),
            MenuContext {
                lock: SessionLockState::Unknown,
                explicit_policy: false,
                inspection: &inspection,
                requested: [Some(true), Some(true)],
                overview: &DesktopOverview::from_snapshot(&observed(), chrono::Utc::now()),
            },
        );
        assert_eq!(state.protection.requested, [Some(true), Some(true)]);
        assert_eq!(state.protection.microphone, "mixed");
        assert_eq!(state.protection.camera, "unknown");
        assert!(state.summary.contains("MIC: ACTIVE"));
        assert!(state.items.iter().any(
            |item| item.detail.as_deref() == Some("device-wide profile inventory unavailable")
        ));
        assert!(
            state
                .items
                .iter()
                .any(|item| item.label.contains("original control state retained"))
        );
        let frame = serde_json::to_value(&state).unwrap();
        assert_eq!(frame["language"], "en");
        assert_eq!(frame["protection"]["camera"], "unknown");
        assert_eq!(
            frame["protection"]["requested"],
            serde_json::json!([true, true])
        );
        assert_eq!(frame["overview"]["microphone"], "active");
        assert_eq!(frame["overview"]["camera"], "unknown");
        assert_eq!(frame["overview"]["total_accesses"], 1);
        assert_eq!(frame["overview"]["omitted_accesses"], 0);
        assert_eq!(
            frame["overview"]["accesses"][0]["application"],
            "corespeechd"
        );
        assert_eq!(
            frame["preferences"]["notifications_enabled"],
            settings.notifications_enabled
        );
        assert_eq!(
            frame["preferences"]["sound_enabled"],
            settings.sound_enabled
        );
        assert_eq!(
            frame["preferences"]["history_enabled"],
            settings.history_enabled
        );
        assert_eq!(
            frame["preferences"]["notifications_paused"],
            settings.notifications_paused()
        );
        assert_eq!(
            frame["feedback"],
            "Partial action: original control state retained"
        );
        assert!(frame.get("details_label").is_none());
        inspection.system_managed = true;
        let managed = make_view(
            observation_status(&observed(), Language::En),
            MicrophoneMuteState::Unmuted,
            (false, None),
            &settings,
            Language::En,
            &None,
            MenuContext {
                lock: SessionLockState::Unlocked,
                explicit_policy: true,
                inspection: &inspection,
                requested: [None; 2],
                overview: &DesktopOverview::default(),
            },
        );
        assert_eq!(managed.protection.camera, "system_managed");
        assert_eq!(managed.protection.requested, [None; 2]);
        assert_eq!(managed.overview.microphone, "unknown");
        assert_eq!(managed.overview.camera, "unknown");
        assert!(managed.overview.observed_at.is_none());
        assert!(managed.overview.accesses.is_empty());
        assert!(managed.feedback.is_none());
        let profile = managed
            .items
            .iter()
            .find(|entry| entry.action == "profile")
            .unwrap();
        assert!(!profile.enabled);
        assert!(
            profile
                .children
                .as_ref()
                .unwrap()
                .iter()
                .all(|entry| { !entry.enabled && entry.checked == Some(false) })
        );
        let managed_frame = serde_json::to_value(&managed).unwrap();
        assert!(managed_frame["overview"]["observed_at"].is_null());
        assert!(managed_frame["feedback"].is_null());
        let mut snapshot = observed();
        snapshot.accesses.clear();
        snapshot.collectors[0].state = CollectorState::Healthy;
        let (idle, visual) = observation_status(&snapshot, Language::En);
        assert_eq!(visual, "idle");
        assert!(
            idle.lines()
                .next()
                .unwrap()
                .contains("MIC: No observed capture")
        );
        snapshot.observation_gaps.push(Resource::Camera);
        let (unknown, visual) = observation_status(&snapshot, Language::En);
        assert_eq!(visual, "partial");
        assert!(
            unknown
                .lines()
                .next()
                .unwrap()
                .contains("CAM: State unknown")
        );
    }

    // Invoked only by the isolated native AppKit fixture. Its output is the
    // actual production make_view projection, not a second hand-authored schema.
    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "writes producer frames for the disposable native AppKit fixture"]
    fn native_consultation_producer_frames() {
        let output = std::env::var_os("MCW_NATIVE_MENU_FRAMES")
            .expect("native fixture must provide its private output path");
        if run_isolated_menu_fixture(
            "frontends::tray::menu_tests::native_consultation_producer_frames",
        ) {
            return;
        }
        let mut cases = Vec::new();
        for (name, count, escaped) in [
            ("producer-long-accesses", 20, false),
            ("producer-omitted-details", 100, false),
            ("producer-json-escaped", 20, true),
        ] {
            let mut snapshot = observed();
            snapshot.collectors[0].state = CollectorState::Healthy;
            let template = snapshot.accesses[0].clone();
            snapshot.accesses = (0..count).map(|index| {
                let mut access = template.clone();
                access.key = format!("fixture-client-{index:03}");
                access.application = format!("fixture-client-{index:03}");
                access.pid = Some(1000 + index);
                let component = if escaped {
                    "\u{1}\t\\\"界".repeat(150)
                } else {
                    "p".repeat(840)
                };
                access.executable = Some(format!(
                    "/Applications/client-{index:03}/{component}.app/Contents/MacOS/client-{index:03}"
                ));
                access.evidence = vec![crate::model::Evidence::new(
                    crate::model::EvidenceKind::LiveApi, "fixture-live-api",
                    format!("Client {index:03} is visible; no physical capture proof is asserted."),
                )];
                access
            }).collect();
            let expected_text = snapshot
                .accesses
                .iter()
                .map(|access| {
                    if count == 20 && !escaped {
                        access.executable.clone().unwrap()
                    } else {
                        access.application.clone()
                    }
                })
                .collect::<Vec<_>>();
            let feedback =
                escaped.then(|| format!("Action failed: {}", "\u{1}\n\t\\\"界".repeat(10_000)));
            let camera_error = escaped.then(|| "Camera inventory: \u{1}\n\\\"界".repeat(3000));
            let inspection = CameraInspectionUi {
                inventory_incomplete: escaped,
                detail: escaped.then(|| "Inspection remains read-only: \u{1}\n\\\"界".repeat(3000)),
                ..Default::default()
            };
            let overview = DesktopOverview::from_snapshot(&snapshot, chrono::Utc::now());
            let view = make_view(
                observation_status(&snapshot, Language::En),
                MicrophoneMuteState::Unmuted,
                (false, camera_error.as_deref()),
                &Settings::default(),
                Language::En,
                &feedback,
                MenuContext {
                    lock: SessionLockState::Unlocked,
                    explicit_policy: false,
                    inspection: &inspection,
                    requested: [None; 2],
                    overview: &overview,
                },
            );
            assert!(view.summary.len() <= 2048);
            assert!(view.diagnostics.len() <= DESKTOP_DIAGNOSTIC_LIMIT);
            assert_eq!(view.overview.total_accesses, count as usize);
            assert_eq!(view.overview.accesses.len(), 8);
            assert_eq!(view.overview.omitted_accesses, count as usize - 8);
            assert!(view.overview.observed_at.is_some());
            assert!(view.feedback.as_ref().is_none_or(|text| text.len() <= 2048));
            assert!(
                view.diagnostics
                    .contains(&format!("Observed accesses: {count}"))
            );
            assert!(
                expected_text
                    .iter()
                    .all(|text| view.diagnostics.contains(text))
            );
            assert_eq!(view.limited, count > 20 || escaped);
            if !view.limited {
                assert!(
                    view.diagnostics.len() > 16_384,
                    "fixture must cross the original rejection boundary"
                );
            }
            assert!(serde_json::to_vec(&view).unwrap().len() <= FRAME_LIMIT);
            cases.push(serde_json::json!({
                "name": name,
                "frame": view,
                "selected_profile": "profile_balanced",
                "expected_text": expected_text,
                "expected_accesses": count,
                "omission_expected": count > 20 || escaped,
            }));
        }
        let mut worker =
            incomplete_inspection_worker(crate::privacy::CameraInspectionAuthorization::Cancelled);
        let historical = worker.ui.detail.clone().unwrap();
        worker.observe_passive_state(crate::privacy::CameraPrivacyState::SystemManaged);
        let snapshot = observed();
        let overview = DesktopOverview::from_snapshot(&snapshot, chrono::Utc::now());
        let view = make_view(
            observation_status(&snapshot, Language::En),
            MicrophoneMuteState::Unmuted,
            (false, None),
            &Settings::default(),
            Language::En,
            &Some("Unrelated earlier action failed".to_owned()),
            MenuContext {
                lock: SessionLockState::Unknown,
                explicit_policy: false,
                inspection: &worker.ui,
                requested: [None; 2],
                overview: &overview,
            },
        );
        assert!(
            !view
                .items
                .iter()
                .any(|entry| entry.action == "camera_inspect")
        );
        assert!(
            view.items
                .iter()
                .any(|entry| entry.detail.as_deref() == Some(historical.as_str()))
        );
        assert_eq!(view.protection.camera, "system_managed");
        cases.push(serde_json::json!({
            "name": "producer-passive-recovery",
            "frame": view,
            "selected_profile": "profile_balanced",
            "expected_text": ["corespeechd"],
            "expected_accesses": 1,
            "omission_expected": false,
            "expected_inspection_detail": historical,
        }));
        std::fs::write(output, serde_json::to_vec_pretty(&cases).unwrap()).unwrap();
    }
}

#[cfg(all(test, target_os = "linux"))]
#[path = "linux_width_tests.rs"]
mod linux_width_tests;
