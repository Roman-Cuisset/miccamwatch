use crate::{
    config::Policy,
    frontends::cli::Filter,
    i18n::Language,
    model::{Activity, CollectorState, MicrophoneMuteState},
    platform::{PlatformMonitor, SessionLockState},
    settings::{PrivacyProfile, Settings},
    watcher::{EventDispatcher, TransitionTracker},
};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
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
    Profile,
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
            "profile" => Some(Self::Profile),
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
}
#[derive(Clone, Serialize)]
struct View {
    kind: &'static str,
    summary: String,
    visual: &'static str,
    items: Vec<MenuEntry>,
    details_label: &'static str,
}

fn uid() -> u32 {
    unsafe { libc::geteuid() }
}
fn runtime_dir() -> Result<PathBuf> {
    let path = crate::settings::data_dir()?.join("tray-runtime");
    fs::create_dir_all(path.parent().context("tray runtime parent unavailable")?)?;
    let mut builder = fs::DirBuilder::new();
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    match builder.create(&path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.into()),
    }
    let metadata = fs::symlink_metadata(&path)?;
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
fn control(command: &[u8]) -> Result<Option<String>> {
    let path = socket_path()?;
    match fs::symlink_metadata(&path) {
        Ok(metadata) => ensure!(
            metadata.file_type().is_socket()
                && metadata.uid() == uid()
                && metadata.mode() & 0o077 == 0,
            "tray socket ownership or permissions are unsafe"
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    }
    let mut stream = match UnixStream::connect(path) {
        Ok(stream) => stream,
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::ConnectionRefused | std::io::ErrorKind::NotFound
            ) =>
        {
            return Ok(None);
        }
        Err(error) => return Err(error.into()),
    };
    verify_peer(&stream)?;
    stream.set_read_timeout(Some(if command == b"stop\n" {
        Duration::from_secs(45)
    } else {
        Duration::from_secs(1)
    }))?;
    stream.set_write_timeout(Some(Duration::from_secs(1)))?;
    stream.write_all(command)?;
    let frame = read_frame(&mut BufReader::new(stream), 64)?
        .context("tray closed control connection before acknowledgment")?;
    Ok(Some(String::from_utf8(frame)?))
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
                            verify_peer(&stream)?;
                            stream.set_read_timeout(Some(Duration::from_millis(250)))?;
                            stream.set_write_timeout(Some(Duration::from_millis(250)))?;
                            let frame = read_frame(&mut BufReader::new(stream.try_clone()?), 16)?
                                .context("empty tray command")?;
                            match frame.as_slice() {
                                b"ping" => Ok(if flags.0.load(Ordering::Acquire) {
                                    "ready\n"
                                } else {
                                    "starting\n"
                                }),
                                b"stop" => {
                                    flags.1.store(true, Ordering::Release);
                                    flags.0.store(false, Ordering::Release);
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
                        if let Ok(reply) = reply {
                            let _ = stream.write_all(reply.as_bytes());
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
    details: &'static str,
}
fn words(lang: Language) -> Words {
    match lang {
        Language::En => Words {
            idle: "No observed capture",
            degraded: "Telemetry degraded",
            failed: "Action failed",
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
            details: "Details…",
        },
        Language::Fr => Words {
            idle: "Aucune capture observée",
            degraded: "Télémétrie dégradée",
            failed: "Échec de l’action",
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
            details: "Détails…",
        },
        Language::De => Words {
            idle: "Keine Erfassung beobachtet",
            degraded: "Telemetrie eingeschränkt",
            failed: "Aktion fehlgeschlagen",
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
            details: "Details…",
        },
        Language::Es => Words {
            idle: "No se observa captura",
            degraded: "Telemetría degradada",
            failed: "Acción fallida",
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
            details: "Detalles…",
        },
        Language::Ja => Words {
            idle: "キャプチャは観測されていません",
            degraded: "テレメトリーが制限されています",
            failed: "操作に失敗しました",
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
            details: "詳細…",
        },
        Language::Zh => Words {
            idle: "未观察到采集",
            degraded: "遥测受限",
            failed: "操作失败",
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
            details: "详细信息…",
        },
        Language::Ru => Words {
            idle: "Захват не наблюдается",
            degraded: "Телеметрия ограничена",
            failed: "Ошибка действия",
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
            details: "Подробности…",
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
    let short = if label == text.lock_unknown {
        Some(text.lock_status)
    } else {
        [text.failed, text.degraded, text.done]
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
        },
        None => entry(action, label, enabled),
    }
}
fn make_view(
    status: (String, &'static str),
    mute: MicrophoneMuteState,
    camera: (bool, Option<&str>),
    settings: &Settings,
    lang: Language,
    feedback: &Option<String>,
    lock: SessionLockState,
) -> View {
    let (summary, visual) = status;
    let (camera_blocked, camera_error) = camera;
    let text = words(lang);
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
        if let Some(error) = camera_error {
            items.push(entry("header", error, false));
        }
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
        camera_error.is_none(),
    ));
    items.push(entry(
        "pause",
        if settings.notifications_paused() {
            text.resume
        } else {
            text.pause
        },
        true,
    ));
    items.push(entry(
        "profile",
        format!(
            "{}: {}",
            text.profile,
            text.profiles[profile_index(settings.profile)]
        ),
        true,
    ));
    let autostart = crate::autostart::state();
    items.push(compact_entry(
        "autostart",
        match &autostart {
            Ok(crate::autostart::AutostartState::Enabled) => text.autostart_on.to_owned(),
            Ok(crate::autostart::AutostartState::Disabled) => text.autostart_off.to_owned(),
            Err(error) => format!("{}: {error:#}", text.failed),
        },
        autostart.is_ok(),
        &text,
    ));
    items.push(entry("exit", text.exit, true));
    View {
        kind: "state",
        summary,
        visual,
        details_label: text.details,
        items,
    }
}

pub fn run_tray(
    mut monitor: PlatformMonitor,
    mut policy: Policy,
    lang: Language,
    mut settings: Settings,
    explicit_policy: bool,
    eventlog: bool,
) -> Result<()> {
    let lifecycle = Lifecycle::acquire()?;
    let (sender, actions) = mpsc::sync_channel(32);
    let initial = make_view(
        (words(lang).degraded.to_owned(), "error"),
        MicrophoneMuteState::Unavailable,
        (
            false,
            if cfg!(target_os = "linux") {
                Some(words(lang).degraded)
            } else {
                None
            },
        ),
        &settings,
        lang,
        &None,
        SessionLockState::Unknown,
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
    #[cfg(target_os = "linux")]
    let mut camera_error: Option<String> = None;
    #[cfg(target_os = "macos")]
    let camera_error: Option<String> = None;
    let mut lock = SessionLockState::Unknown;
    let outcome = (|| -> Result<()> {
        loop {
            if lifecycle.stop.load(Ordering::Acquire) {
                break;
            }
            host.check()?;
            if Instant::now() >= next_poll {
                match Settings::load() {
                    Ok(current) => settings = current,
                    Err(error) => {
                        feedback = Some(format!("{}: {error:#}", words(lang).failed));
                        action_error = true;
                    }
                }
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
                        let unhealthy = snapshot.collectors.is_empty()
                            || snapshot
                                .collectors
                                .iter()
                                .any(|health| health.state != CollectorState::Healthy)
                            || !snapshot.observation_gaps.is_empty();
                        let active = snapshot
                            .accesses
                            .iter()
                            .filter(|access| access.activity == Activity::Active)
                            .collect::<Vec<_>>();
                        let ready = snapshot
                            .accesses
                            .iter()
                            .filter(|access| access.activity == Activity::Ready)
                            .collect::<Vec<_>>();
                        visual = if unhealthy {
                            "error"
                        } else if !active.is_empty() {
                            "active"
                        } else if !ready.is_empty() {
                            "ready"
                        } else {
                            "idle"
                        };
                        let accesses = if active.is_empty() { &ready } else { &active };
                        let details = accesses
                            .iter()
                            .map(|access| {
                                format!(
                                    "{}: {} {}",
                                    access.application,
                                    lang.resource(access.resource),
                                    lang.state_str(None, access.activity)
                                )
                            })
                            .collect::<Vec<_>>()
                            .join(", ");
                        summary = if unhealthy {
                            format!(
                                "{}{}{}",
                                words(lang).degraded,
                                if details.is_empty() { "" } else { ": " },
                                details
                            )
                        } else if details.is_empty() {
                            words(lang).idle.to_owned()
                        } else {
                            details
                        };
                    }
                    Err(error) => {
                        tracker.observation_gap();
                        visual = "error";
                        summary = format!("{}: {error:#}", words(lang).degraded);
                    }
                }
                mute = monitor
                    .microphone_mute_state()
                    .unwrap_or(MicrophoneMuteState::Unavailable);
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
                        if camera_pending_target == Some(camera_blocked) {
                            camera_pending_target = None;
                            feedback = Some(words(lang).done.to_owned());
                            action_error = false;
                        }
                    }
                    Err(error) => {
                        feedback = Some(format!("{}: {error:#}", words(lang).failed));
                        action_error = true;
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
                    lock,
                ))?;
            }
            match actions.recv_timeout(Duration::from_millis(100)) {
                Ok(Command::Exit) => break,
                Ok(command) => {
                    #[cfg(target_os = "macos")]
                    if !matches!(command, Command::Camera) {
                        camera_pending_target = None;
                    }
                    let result: Result<()> = (|| {
                        match command {
                            Command::Status => {
                                #[cfg(target_os = "macos")]
                                crate::notify::notify_message("MicCamWatch", &summary)?;
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
                                        lock,
                                    )),
                                )?;
                            }
                            Command::Mic => {
                                monitor.toggle_microphone_mute()?;
                            }
                            Command::Camera => {
                                #[cfg(target_os = "macos")]
                                {
                                    camera_pending_target = Some(!camera_blocked);
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
                            Command::Profile => {
                                settings = Settings::update(|current| {
                                    current.profile = match current.profile {
                                        PrivacyProfile::Balanced => PrivacyProfile::Private,
                                        PrivacyProfile::Private => PrivacyProfile::Meeting,
                                        PrivacyProfile::Meeting => PrivacyProfile::Development,
                                        PrivacyProfile::Development => PrivacyProfile::Balanced,
                                    };
                                })?;
                            }
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
                        lock,
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
            _ => [110, 110, 110],
        };
        let mut pixels = Vec::with_capacity(32 * 32 * 4);
        for y in 0i32..32 {
            for x in 0i32..32 {
                let inside = (x - 16).pow(2) + (y - 16).pow(2) <= 13 * 13;
                let glyph = match self.view.visual {
                    "error" => {
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
        if self.view.visual == "error" {
            ksni::Status::NeedsAttention
        } else {
            ksni::Status::Active
        }
    }
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        let mut menu: Vec<ksni::MenuItem<Self>> = self
            .view
            .items
            .iter()
            .map(|entry| {
                let command = Command::from_name(entry.action);
                ksni::menu::StandardItem {
                    label: linux_menu_label(&entry.label),
                    enabled: entry.enabled,
                    activate: Box::new(move |this: &mut Self| {
                        if let Some(command) = command {
                            let _ = this.actions.try_send(command);
                        }
                    }),
                    ..Default::default()
                }
                .into()
            })
            .collect();
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
                        if event.kind == "ready" && event.protocol == Some(1) {
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
                protocol: Some(1),
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
            "tray state exceeds native frame limit"
        );
        frame.push(b'\n');
        let stdin = self.stdin.as_mut().context("AppKit helper input closed")?;
        stdin.write_all(&frame)?;
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

    #[test]
    fn compact_status_frames_retain_localized_diagnostics_and_actions() {
        for lang in [
            Language::En,
            Language::Fr,
            Language::De,
            Language::Es,
            Language::Ja,
            Language::Zh,
            Language::Ru,
        ] {
            let text = words(lang);
            for status in [text.failed, text.degraded] {
                let diagnostic =
                    format!("{status}: {}", "camera restriction unknown; ".repeat(500));
                let row = compact_entry("status", &diagnostic, true, &text);
                let frame = serde_json::to_value(&row).unwrap();
                assert_eq!(frame["label"], status);
                assert_eq!(frame["detail"], diagnostic);
                assert_eq!(frame["action"], "status");
                assert_eq!(frame["enabled"], true);
            }
            let lock = compact_entry("header", text.lock_unknown, false, &text);
            assert_eq!(lock.label, text.lock_status);
            assert_eq!(lock.detail.as_deref(), Some(text.lock_unknown));
            assert!(!lock.enabled);
            let control = compact_entry("camera", text.camera, false, &text);
            assert_eq!(control.label, text.camera);
            assert!(control.detail.is_none());
            assert_eq!(control.action, "camera");
            assert!(!control.enabled);
            assert!(!text.details.is_empty());
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
#[path = "linux_width_tests.rs"]
mod linux_width_tests;
