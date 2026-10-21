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
use std::sync::mpsc::Receiver;
use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Write},
    os::unix::{
        fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt},
        io::AsRawFd,
        net::{UnixListener, UnixStream},
    },
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender},
    },
    thread,
    time::{Duration, Instant},
};

#[cfg(target_os = "macos")]
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
    #[cfg(target_os = "macos")]
    diagnostics: String,
    #[cfg(target_os = "macos")]
    limited: bool,
    #[cfg(target_os = "macos")]
    language: &'static str,
    #[cfg(target_os = "macos")]
    version: &'static str,
    #[cfg(target_os = "macos")]
    protection: DesktopProtection,
    #[cfg(target_os = "linux")]
    details_label: &'static str,
}
#[cfg(target_os = "macos")]
#[derive(Clone, Serialize)]
struct DesktopProtection {
    microphone: &'static str,
    camera: &'static str,
    requested: [Option<bool>; 2],
    microphone_on_lock: bool,
    camera_on_lock: bool,
}

#[cfg(target_os = "macos")]
const DESKTOP_TEXT_BUDGET: usize = 48_000;
#[cfg(target_os = "macos")]
const DESKTOP_DIAGNOSTIC_LIMIT: usize = 32_768;
#[cfg(target_os = "macos")]
const DESKTOP_OMISSION: &str =
    "\n[… more text omitted by bounded desktop transport; this field is incomplete.]";

// Count JSON-escaped content, not just UTF-8: control characters can expand six
// times. The remaining 17 KiB of a frame covers the bounded protocol structure.
#[cfg(target_os = "macos")]
fn desktop_text_cost(text: &str) -> usize {
    text.bytes()
        .map(|byte| match byte {
            b'"' | b'\\' | b'\x08' | b'\x0c' | b'\n' | b'\r' | b'\t' => 2,
            0..=31 => 6,
            _ => 1,
        })
        .sum()
}

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "macos")]
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
    let mut remaining = 2 + field_count(&view.items);
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

#[cfg(target_os = "macos")]
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
struct Lifecycle {
    _lock: File,
    path: PathBuf,
    ready: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    finished: Arc<AtomicBool>,
    clean_shutdown: Arc<AtomicBool>,
    server_exit: Arc<AtomicBool>,
    #[cfg(target_os = "macos")]
    open_requested: Arc<AtomicBool>,
    server: Option<thread::JoinHandle<()>>,
}
impl Lifecycle {
    fn acquire() -> Result<Self> {
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
        ensure!(
            unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0,
            "the per-user native tray is already starting or running"
        );
        if let Ok(metadata) = fs::symlink_metadata(&path) {
            ensure!(
                metadata.file_type().is_socket() && metadata.uid() == uid(),
                "refusing to remove an unsafe stale tray socket"
            );
            fs::remove_file(&path)?;
        }
        let listener = UnixListener::bind(&path)?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
        listener.set_nonblocking(true)?;
        let ready = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let finished = Arc::new(AtomicBool::new(false));
        let clean_shutdown = Arc::new(AtomicBool::new(false));
        let server_exit = Arc::new(AtomicBool::new(false));
        #[cfg(target_os = "macos")]
        let open_requested = Arc::new(AtomicBool::new(false));
        #[cfg(target_os = "macos")]
        let server_open = open_requested.clone();
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
                                #[cfg(target_os = "macos")]
                                b"open" => {
                                    if flags.1.load(Ordering::Acquire)
                                        || flags.2.load(Ordering::Acquire)
                                    {
                                        Ok("stopping\n")
                                    } else {
                                        // Coalesce duplicate launches even while AppKit
                                        // initializes; its loop consumes this after start.
                                        server_open.store(true, Ordering::Release);
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
            ready,
            stop,
            finished,
            clean_shutdown,
            server_exit,
            #[cfg(target_os = "macos")]
            open_requested,
            server: Some(server),
        })
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
        let _ = fs::remove_file(&self.path);
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
            details: "Details…",
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
            details: "Détails…",
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
            details: "Details…",
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
            details: "Detalles…",
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
            details: "詳細…",
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
            details: "详细信息…",
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
            details: "Подробности…",
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
            let activity = if snapshot
                .accesses
                .iter()
                .any(|a| a.resource == resource && a.activity == Activity::Active)
            {
                lang.state_str(None, Activity::Active)
            } else if snapshot
                .accesses
                .iter()
                .any(|a| a.resource == resource && a.activity == Activity::Ready)
            {
                lang.state_str(None, Activity::Ready)
            } else if unavailable || snapshot.observation_gaps.contains(&resource) {
                words(lang).state_unknown
            } else {
                words(lang).idle
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
    #[cfg(target_os = "macos")]
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
    } = context;
    let (summary, visual) = status;
    #[cfg(target_os = "macos")]
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
    items.push(entry(
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
    ));
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
    let view = View {
        kind: "state",
        summary,
        #[cfg(target_os = "macos")]
        diagnostics,
        #[cfg(target_os = "macos")]
        limited: false,
        visual,
        #[cfg(target_os = "macos")]
        language: lang.code(),
        #[cfg(target_os = "macos")]
        version: env!("CARGO_PKG_VERSION"),
        #[cfg(target_os = "macos")]
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
            requested,
            microphone_on_lock: settings.mute_on_lock,
            camera_on_lock: settings.block_camera_on_lock,
        },
        #[cfg(target_os = "linux")]
        details_label: text.details,
        items,
    };
    #[cfg(target_os = "macos")]
    let view = {
        let mut bounded = view;
        bound_desktop_view(&mut bounded);
        bounded
    };
    view
}

pub fn run_tray(
    mut monitor: PlatformMonitor,
    mut policy: Policy,
    lang: Language,
    mut settings: Settings,
    explicit_policy: bool,
    eventlog: bool,
    #[cfg(target_os = "macos")] initial_window: bool,
) -> Result<()> {
    let lifecycle = Lifecycle::acquire()?;
    let (sender, actions) = mpsc::sync_channel(32);
    #[cfg(target_os = "macos")]
    let mut inspection = CameraInspectionWorker::default();
    #[cfg(target_os = "linux")]
    let inspection = CameraInspectionWorker::default();
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
        },
    );
    let mut host = DesktopHost::start(sender, lifecycle.ready.clone(), initial)?;
    let mut tracker = TransitionTracker::default();
    let mut dispatcher = EventDispatcher::new(
        lang,
        false,
        false,
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
    #[cfg(target_os = "macos")]
    let mut show_initial_window = initial_window;
    let outcome = (|| -> Result<()> {
        loop {
            if lifecycle.stop.load(Ordering::Acquire) {
                break;
            }
            host.check()?;
            #[cfg(target_os = "macos")]
            if lifecycle.open_requested.swap(false, Ordering::AcqRel) {
                host.show_window()?;
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
                        tracker.reconcile(&snapshot, |access, action| {
                            dispatcher.dispatch(access, action)
                        })?;
                        (summary, visual) = observation_status(&snapshot, lang);
                    }
                    Err(error) => {
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
                        inspection.ui.system_managed =
                            state == crate::privacy::CameraPrivacyState::SystemManaged;
                        camera_error = None;
                        inspection.ui.inventory_incomplete = false;
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
                    },
                ))?;
                #[cfg(target_os = "macos")]
                if show_initial_window {
                    host.show_window()?;
                    show_initial_window = false;
                }
            }
            match actions.recv_timeout(Duration::from_millis(100)) {
                Ok(Command::Exit) => break,
                Ok(command) => {
                    #[cfg(target_os = "macos")]
                    if matches!(command, Command::Status) {
                        host.show_window()?;
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
                            Command::Status => {
                                #[cfg(target_os = "macos")]
                                host.show_window()?;
                                #[cfg(target_os = "linux")]
                                crate::notify::notify_message(
                                    "MicCamWatch",
                                    &diagnostic_document(&make_view(
                                        (
                                            summary.clone(),
                                            if action_error { "error" } else { visual },
                                        ),
                                        mute,
                                        (camera_blocked, camera_error.as_deref()),
                                        &settings,
                                        lang,
                                        &feedback,
                                        MenuContext {
                                            lock,
                                            explicit_policy,
                                            inspection: &inspection.ui,
                                        },
                                    )),
                                )?;
                            }
                            Command::Mic => {
                                #[cfg(target_os = "macos")]
                                {
                                    requested[0] = Some(mute != MicrophoneMuteState::Muted);
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
                                    camera_blocked = crate::privacy::toggle_camera_state()?
                                        != crate::privacy::CameraPrivacyState::Allowed;
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
    let capacity = view.summary.len()
        + view
            .items
            .iter()
            .filter_map(|entry| {
                let original = entry.detail.as_deref().unwrap_or(&entry.label);
                (original != view.summary).then_some(original.len() + 2)
            })
            .sum::<usize>();
    let mut document = String::with_capacity(capacity);
    document.push_str(&view.summary);
    for entry in &view.items {
        let original = entry.detail.as_deref().unwrap_or(&entry.label);
        if original != view.summary {
            document.push_str("\n\n");
            document.push_str(original);
        }
    }
    document
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
        "miccamwatch".into()
    }
    fn title(&self) -> String {
        "MicCamWatch".into()
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
            menu.len().saturating_sub(1),
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
struct DesktopHost {
    handle: ksni::blocking::Handle<LinuxTray>,
}
#[cfg(target_os = "linux")]
impl DesktopHost {
    fn start(actions: SyncSender<Command>, ready: Arc<AtomicBool>, view: View) -> Result<Self> {
        use ksni::blocking::TrayMethods;
        // ksni's default registration verifies that an actual compatible SNI host exists.
        // In particular, do not use assume_sni_available on GNOME without AppIndicator.
        let handle = LinuxTray { view, actions, ready: ready.clone() }.spawn()
            .context("native tray requires a session D-Bus and a registered StatusNotifierItem desktop host (e.g. KDE, or GNOME AppIndicator)")?;
        ensure!(
            !handle.is_closed(),
            "native desktop host disappeared during tray registration"
        );
        ready.store(true, Ordering::Release);
        Ok(Self { handle })
    }
    fn update(&mut self, view: View) -> Result<()> {
        self.handle
            .update(|tray| tray.view = view)
            .context("Linux tray service closed")
    }
    fn check(&mut self) -> Result<()> {
        ensure!(
            !self.handle.is_closed(),
            "native tray service closed or desktop host disappeared"
        );
        Ok(())
    }
    fn shutdown(&mut self) -> Result<()> {
        if !self.handle.is_closed() {
            self.handle.shutdown().wait();
        }
        Ok(())
    }
}
#[cfg(target_os = "linux")]
impl Drop for DesktopHost {
    fn drop(&mut self) {
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
    stdin: Option<std::process::ChildStdin>,
    events: Receiver<Result<DesktopEvent>>,
    ready: Arc<AtomicBool>,
}
#[cfg(target_os = "macos")]
impl DesktopHost {
    fn start(actions: SyncSender<Command>, ready: Arc<AtomicBool>, view: View) -> Result<Self> {
        let mut child = crate::platform::macos::desktop_process(&["desktop"])?;
        let stdin = child
            .stdin
            .take()
            .context("AppKit helper stdin unavailable")?;
        let stdout = child
            .stdout
            .take()
            .context("AppKit helper stdout unavailable")?;
        let (sender, events) = mpsc::sync_channel(32);
        let readiness = ready.clone();
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
                        if event.kind == "ready" && event.protocol == Some(3) {
                            readiness.store(true, Ordering::Release);
                        }
                        if event.kind == "error" || event.kind == "stopped" {
                            readiness.store(false, Ordering::Release);
                        }
                        sender
                            .send(Ok(event))
                            .context("tray event receiver closed")?;
                    }
                }
                bail!("AppKit helper closed its event stream")
            })();
            readiness.store(false, Ordering::Release);
            if let Err(error) = result {
                let _ = sender.send(Err(error));
            }
        });
        let mut host = Self {
            child,
            stdin: Some(stdin),
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
                protocol: Some(3),
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
        debug_assert!(
            frame.len() <= FRAME_LIMIT,
            "bounded desktop projection exceeded frame budget"
        );
        frame.push(b'\n');
        let stdin = self.stdin.as_mut().context("AppKit helper input closed")?;
        stdin.write_all(&frame)?;
        stdin.flush()?;
        Ok(())
    }
    fn show_window(&mut self) -> Result<()> {
        let stdin = self.stdin.as_mut().context("AppKit helper input closed")?;
        stdin.write_all(b"{\"kind\":\"show\"}\n")?;
        stdin.flush()?;
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
        self.ready.store(false, Ordering::Release);
        if let Some(mut stdin) = self.stdin.take() {
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
        self.stdin.take();
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

    #[cfg(target_os = "macos")]
    fn run_isolated_menu_fixture(test: &str) -> bool {
        if std::env::var("MCW_TEST_MENU_CASE").as_deref() == Ok(test) {
            return false;
        }
        // make_view reads real autostart state, whose ownership store creates
        // directories even for a status query. Isolate that production path in
        // a child, never by changing process-global HOME during parallel tests.
        let directory = tempfile::tempdir().unwrap();
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
            output.status.success(),
            "isolated menu fixture {test} failed: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        true
    }

    fn observed() -> Snapshot {
        Snapshot {
            collectors: vec![CollectorHealth {
                collector: "coremediaio_video",
                state: CollectorState::Degraded,
                detail: Some("running-state does not identify the client".into()),
            }],
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
        assert!(detail.contains("coremediaio_video"));
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
                explicit_policy: false,
                inspection: &inspection,
                requested: [None; 2],
            },
        );
        assert_eq!(managed.protection.camera, "system_managed");
        assert_eq!(managed.protection.requested, [None; 2]);
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
                },
            );
            assert!(view.summary.len() <= 2048);
            assert!(view.diagnostics.len() <= DESKTOP_DIAGNOSTIC_LIMIT);
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
        std::fs::write(output, serde_json::to_vec_pretty(&cases).unwrap()).unwrap();
    }
}

#[cfg(all(test, target_os = "linux"))]
#[path = "linux_width_tests.rs"]
mod linux_width_tests;
