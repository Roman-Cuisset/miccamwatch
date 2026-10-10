#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
use crate::privacy::{self, CameraPrivacyState};
use crate::{
    config::Policy,
    frontends::cli::Filter,
    i18n::Language,
    model::{
        Access, Activity, CollectorState, Device, MicrophoneMuteState, Resource, Risk, Snapshot,
    },
    platform::PlatformMonitor,
};
use anyhow::{Context, Result};
use chrono::Local;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseButton,
        MouseEventKind,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
#[cfg(not(windows))]
use ratatui::widgets::Wrap;
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
};
use std::{
    io,
    sync::mpsc::{self, Receiver, SyncSender},
    thread,
    time::{Duration, Instant},
};

#[cfg(windows)]
type MicObservation = crate::platform::MicrophoneProtectionStatus;
#[cfg(not(windows))]
type MicObservation = MicrophoneMuteState;
#[cfg(windows)]
type CameraObservation = privacy::CameraControlObservation;
#[cfg(not(windows))]
type CameraObservation = CameraPrivacyState;

fn read_microphone(monitor: &PlatformMonitor) -> Result<MicObservation> {
    #[cfg(windows)]
    return monitor.microphone_protection_status();
    #[cfg(not(windows))]
    monitor.microphone_mute_state()
}

fn read_camera() -> Result<CameraObservation> {
    #[cfg(windows)]
    return privacy::camera_observation();
    #[cfg(not(windows))]
    privacy::camera_state()
}

struct TerminalSessionGuard;

impl Drop for TerminalSessionGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
    }
}

fn text(lang: Language, values: [&'static str; 7]) -> &'static str {
    values[match lang {
        Language::En => 0,
        Language::Fr => 1,
        Language::De => 2,
        Language::Es => 3,
        Language::Ja => 4,
        Language::Zh => 5,
        Language::Ru => 6,
    }]
}

fn microphone_scope(lang: Language) -> &'static str {
    #[cfg(windows)]
    {
        lang.protection_limit()
    }
    #[cfg(unix)]
    if cfg!(target_os = "linux") {
        text(
            lang,
            [
                "PipeWire session sources only",
                "Sources de session PipeWire uniquement",
                "Nur PipeWire-Sitzungsquellen",
                "Solo fuentes de sesión PipeWire",
                "PipeWire セッションソースのみ",
                "仅 PipeWire 会话源",
                "Только источники сеанса PipeWire",
            ],
        )
    } else {
        text(
            lang,
            [
                "Writable CoreAudio inputs only",
                "Entrées CoreAudio modifiables uniquement",
                "Nur schreibbare CoreAudio-Eingänge",
                "Solo entradas CoreAudio modificables",
                "書き込み可能な CoreAudio 入力のみ",
                "仅可写 CoreAudio 输入",
                "Только доступные для записи входы CoreAudio",
            ],
        )
    }
}

#[cfg(not(windows))]
fn microphone_label(lang: Language, state: MicrophoneMuteState) -> String {
    #[cfg(unix)]
    format!(
        "{} ({})",
        lang.microphone_status(state),
        microphone_scope(lang)
    )
}

fn camera_label(lang: Language, state: CameraPrivacyState) -> &'static str {
    #[cfg(windows)]
    if state == CameraPrivacyState::SystemManaged {
        return text(
            lang,
            [
                "Actual unknown/mixed",
                "État inconnu/mixte",
                "Zustand unbekannt/gemischt",
                "Estado desconocido/mixto",
                "実状態は不明/混在",
                "实际状态未知/混合",
                "Состояние неизвестно/смешанное",
            ],
        );
    }
    #[cfg(target_os = "macos")]
    return text(
        lang,
        match state {
            CameraPrivacyState::Blocked => [
                "Owned restriction installed",
                "Restriction possédée installée",
                "Eigene Einschränkung installiert",
                "Restricción propia instalada",
                "所有制限を導入済み",
                "已安装本应用限制",
                "Собственное ограничение установлено",
            ],
            CameraPrivacyState::Allowed => [
                "Owned restriction absent",
                "Restriction possédée absente",
                "Eigene Einschränkung nicht installiert",
                "Restricción propia ausente",
                "所有制限なし",
                "无本应用限制",
                "Собственное ограничение отсутствует",
            ],
            CameraPrivacyState::SystemManaged => [
                "System-managed",
                "Gérée par le système",
                "Systemverwaltet",
                "Gestionada por el sistema",
                "システム管理",
                "系统管理",
                "Управляется системой",
            ],
        },
    );
    #[cfg(target_os = "linux")]
    return text(
        lang,
        match state {
            CameraPrivacyState::Blocked => [
                "Supported USB cameras blocked",
                "Caméras USB compatibles bloquées",
                "Unterstützte USB-Kameras gesperrt",
                "Cámaras USB compatibles bloqueadas",
                "対応 USB カメラをブロック中",
                "支持的 USB 摄像头已禁用",
                "Поддерживаемые USB-камеры заблокированы",
            ],
            CameraPrivacyState::Allowed => [
                "No owned USB camera blocks",
                "Aucun blocage USB possédé",
                "Keine eigenen USB-Kamerasperren",
                "Sin bloqueos USB propios",
                "自分の USB カメラブロックなし",
                "无自有 USB 摄像头禁用",
                "Своих блокировок USB-камер нет",
            ],
            CameraPrivacyState::SystemManaged => [
                "USB ownership partial/stale — inspect camera status",
                "État USB partiel/périmé — voir camera status",
                "USB-Besitz teilweise/veraltet — camera status prüfen",
                "Estado USB parcial/obsoleto — consulte camera status",
                "USB 所有状態が一部・古い — camera status を確認",
                "USB 状态部分或过期 — 查看 camera status",
                "USB владение неполное/устарело — см. camera status",
            ],
        },
    );
    #[cfg(windows)]
    match state {
        CameraPrivacyState::Blocked => text(
            lang,
            [
                "Blocked",
                "Bloquée",
                "Gesperrt",
                "Bloqueada",
                "ブロック中",
                "已禁用",
                "Заблокирована",
            ],
        ),
        CameraPrivacyState::Allowed => text(
            lang,
            [
                "Allowed",
                "Autorisée",
                "Zugelassen",
                "Permitida",
                "許可",
                "已允许",
                "Разрешена",
            ],
        ),
        CameraPrivacyState::SystemManaged => text(
            lang,
            [
                "System-managed",
                "Gérée par le système",
                "Systemverwaltet",
                "Gestionada por el sistema",
                "システム管理",
                "系统管理",
                "Управляется системой",
            ],
        ),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Action {
    BlockCamera,
    RestoreCamera,
    Mute,
    Kill,
    Refresh,
    Quit,
}

enum WorkerCommand {
    Camera(CameraPrivacyState),
    Mute,
    Kill { pid: u32, instance: String },
}

enum WorkerResponse {
    Camera(Result<CameraObservation>),
    Mute(Result<MicObservation>),
    Kill { pid: u32, result: Result<()> },
}

// Closing the sender drains the one outstanding action before joining. Even an
// I/O error cannot abandon a privileged mutation and imply it was cancelled.
struct ActionWorker {
    commands: Option<SyncSender<WorkerCommand>>,
    responses: Receiver<WorkerResponse>,
    thread: Option<thread::JoinHandle<()>>,
}

impl ActionWorker {
    fn new(policy: Policy) -> Self {
        let (commands, requests) = mpsc::sync_channel(1);
        let (responses, results) = mpsc::sync_channel(1);
        let thread = thread::spawn(move || {
            // Construct and destroy the COM monitor on this thread only.
            let mut monitor = None;
            while let Ok(command) = requests.recv() {
                let response = match command {
                    WorkerCommand::Camera(desired) => {
                        let result = (|| {
                            privacy::set_camera_state(desired)?;
                            read_camera()
                        })();
                        WorkerResponse::Camera(result)
                    }
                    WorkerCommand::Mute => WorkerResponse::Mute((|| {
                        if monitor.is_none() {
                            monitor = Some(PlatformMonitor::new(policy.clone())?);
                        }
                        let monitor = monitor.as_ref().unwrap();
                        monitor.toggle_microphone_mute()?;
                        read_microphone(monitor)
                    })()),
                    WorkerCommand::Kill { pid, instance } => WorkerResponse::Kill {
                        pid,
                        result: crate::platform::terminate_process_by_pid(pid, &instance),
                    },
                };
                if responses.send(response).is_err() {
                    break;
                }
            }
        });
        Self {
            commands: Some(commands),
            responses: results,
            thread: Some(thread),
        }
    }

    fn send(&self, command: WorkerCommand) -> Result<()> {
        self.commands
            .as_ref()
            .unwrap()
            .try_send(command)
            .context("action worker busy/stopped")
    }
}

impl Drop for ActionWorker {
    fn drop(&mut self) {
        self.commands.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

struct Observation {
    snapshot: Result<Snapshot>,
    devices: Option<Result<Vec<Device>>>,
    mute: Option<(Instant, Result<MicObservation>)>,
    camera: Option<(Instant, Result<CameraObservation>)>,
}

fn observe(policy: Policy) -> (SyncSender<()>, Receiver<Observation>) {
    let (refresh, requests) = mpsc::sync_channel(1);
    // Bound the queue: a slow terminal must not accumulate stale scans.
    let (updates, observations) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let monitor = match PlatformMonitor::new(policy) {
            Ok(monitor) => monitor,
            Err(error) => {
                let _ = updates.send(Observation {
                    snapshot: Err(error),
                    devices: None,
                    mute: None,
                    camera: None,
                });
                return;
            }
        };
        let filter = Filter {
            include_ready: true,
            ..Filter::default()
        };
        let mut refresh_controls = true;
        loop {
            let devices = refresh_controls.then(|| monitor.devices());
            let snapshot = monitor.snapshot((&filter).into());
            let read_started = Instant::now();
            let mute = Some((read_started, read_microphone(&monitor)));
            #[cfg(windows)]
            let camera = Some((Instant::now(), read_camera()));
            #[cfg(not(windows))]
            let camera = refresh_controls.then(|| (Instant::now(), read_camera()));
            let observation = Observation {
                snapshot,
                devices,
                mute,
                camera,
            };
            if updates.send(observation).is_err() {
                break;
            }
            refresh_controls = false;
            match requests.recv_timeout(Duration::from_millis(250)) {
                Ok(()) => {
                    while requests.try_recv().is_ok() {}
                    refresh_controls = true;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    });
    (refresh, observations)
}

pub fn run_tui(policy: Policy, lang: Language) -> Result<()> {
    #[cfg(windows)]
    crate::platform::resume_requested_microphone_protection()?;
    enable_raw_mode().context("failed to enable raw mode")?;
    let mut stdout = io::stdout();
    let _session = TerminalSessionGuard;
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
        .context("failed to enter alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("failed to initialize ratatui terminal")?;

    let result = tui_loop(&mut terminal, policy, lang);
    let _ = terminal.show_cursor();
    result
}

struct TuiState {
    lang: Language,
    selected_access: usize,
    kill_available: bool,
    table_state: TableState,
    events_log: Vec<(String, String, Color)>,
    status_msg: Option<(String, Instant, Color)>,
    mute_state: MicrophoneMuteState,
    mute_changed_at: Option<Instant>,
    #[cfg(windows)]
    microphone_protection: Option<MicObservation>,
    #[cfg(windows)]
    camera_observation: Option<CameraObservation>,
    camera_changed_at: Option<Instant>,
    #[cfg(windows)]
    protection_feedback: String,
    devices: Vec<Device>,
    health_error: Option<String>,
    camera_state: Option<CameraPrivacyState>,
    camera_error: Option<String>,
    operation_error: Option<String>,
    camera_pending: bool,
    action_pending: Option<Action>,
    controls_error: Option<String>,
    buttons: Vec<(Rect, Action)>,
    pending_kill: Option<(String, u32, String, Instant)>,
}

impl TuiState {
    fn message(&mut self, message: String, color: Color) {
        self.status_msg = Some((message, Instant::now(), color));
        if color == Color::Red {
            self.operation_error = self
                .status_msg
                .as_ref()
                .map(|(message, _, _)| message.clone());
        }
        #[cfg(windows)]
        self.update_protection_feedback();
    }

    fn apply_microphone(&mut self, observation: MicObservation) {
        #[cfg(windows)]
        {
            self.mute_state = observation.mute_state;
            self.microphone_protection = Some(observation);
        }
        #[cfg(not(windows))]
        {
            self.mute_state = observation;
        }
        #[cfg(windows)]
        self.update_protection_feedback();
    }

    fn apply_camera(&mut self, observation: CameraObservation) {
        #[cfg(windows)]
        {
            self.camera_state = Some(observation.state);
            self.camera_observation = Some(observation);
        }
        #[cfg(not(windows))]
        {
            self.camera_state = Some(observation);
        }
        #[cfg(windows)]
        self.update_protection_feedback();
    }

    fn microphone_summary(&self) -> String {
        #[cfg(windows)]
        if let Some(status) = &self.microphone_protection {
            return crate::output::microphone_protection_summary(self.lang, status);
        }
        #[cfg(windows)]
        return format!(
            "{}; SDK: {}",
            self.lang.unknown_protection(true),
            self.lang.microphone_status(self.mute_state)
        );
        #[cfg(not(windows))]
        microphone_label(self.lang, self.mute_state)
    }

    fn camera_summary(&self) -> String {
        #[cfg(windows)]
        if let Some(status) = &self.camera_observation {
            return crate::output::camera_protection_summary(self.lang, status);
        }
        #[cfg(windows)]
        return self.lang.unknown_protection(false).to_owned();
        #[cfg(not(windows))]
        self.camera_state.map_or_else(
            || "Unknown".to_owned(),
            |state| camera_label(self.lang, state).to_owned(),
        )
    }

    #[cfg(windows)]
    fn update_protection_feedback(&mut self) {
        let feedback = self
            .operation_error
            .as_deref()
            .or(self.camera_error.as_deref())
            .or(self.controls_error.as_deref())
            .or(self.health_error.as_deref())
            .or_else(|| {
                self.status_msg
                    .as_ref()
                    .map(|(message, _, _)| message.as_str())
            })
            .unwrap_or("");
        self.protection_feedback = format!(
            "{}\n{}\n{}\n{}",
            self.microphone_summary(),
            self.camera_summary(),
            self.lang.protection_limit(),
            feedback
        );
    }
}

fn normalize_key(code: KeyCode) -> KeyCode {
    match code {
        KeyCode::Char(letter) => KeyCode::Char(letter.to_ascii_lowercase()),
        other => other,
    }
}

fn key_action(code: KeyCode) -> Option<Action> {
    match code {
        KeyCode::Char('b') => Some(Action::BlockCamera),
        KeyCode::Char('a') => Some(Action::RestoreCamera),
        KeyCode::Char('m') => Some(Action::Mute),
        KeyCode::Char('k') => Some(Action::Kill),
        KeyCode::Char('r') => Some(Action::Refresh),
        KeyCode::Char('q') | KeyCode::Esc => Some(Action::Quit),
        _ => None,
    }
}

fn contains(rect: Rect, x: u16, y: u16) -> bool {
    x >= rect.x && x < rect.right() && y >= rect.y && y < rect.bottom()
}

fn kill_confirmed(
    pending: &Option<(String, u32, String, Instant)>,
    key: &str,
    pid: u32,
    instance: &str,
) -> bool {
    pending
        .as_ref()
        .is_some_and(|(pending_key, pending_pid, pending_instance, created)| {
            pending_key == key
                && *pending_pid == pid
                && pending_instance == instance
                && !instance.is_empty()
                && created.elapsed() < Duration::from_secs(3)
        })
}

fn handle_action(
    action: Action,
    state: &mut TuiState,
    accesses: &[Access],
    worker: &ActionWorker,
    refresh: &SyncSender<()>,
) -> Result<bool> {
    let lang = state.lang;
    if action != Action::Kill {
        state.pending_kill = None;
    }
    if action == Action::Refresh {
        match refresh.try_send(()) {
            Ok(()) | Err(mpsc::TrySendError::Full(())) => state.message(
                text(
                    lang,
                    [
                        "Refreshing devices and controls…",
                        "Actualisation des périphériques et commandes…",
                        "Geräte und Steuerung aktualisieren…",
                        "Actualizando dispositivos y controles…",
                        "デバイスと制御を更新中…",
                        "正在刷新设备和控制…",
                        "Обновление устройств и управления…",
                    ],
                )
                .to_owned(),
                Color::Yellow,
            ),
            Err(_) => state.message(
                text(
                    lang,
                    [
                        "Observation worker stopped",
                        "Service d'observation arrêté",
                        "Beobachtungsdienst gestoppt",
                        "Servicio de observación detenido",
                        "監視ワーカー停止",
                        "监视线程已停止",
                        "Обработчик наблюдения остановлен",
                    ],
                )
                .to_owned(),
                Color::Red,
            ),
        }
        return Ok(false);
    }
    if state.action_pending.is_some() || (state.camera_pending && action != Action::Quit) {
        state.message(
            text(
                lang,
                [
                    "Busy: wait for the operation; cancel administrator approval to cancel.",
                    "En cours : attendez ou annulez l'autorisation administrateur.",
                    "Belegt: Vorgang abwarten oder Administratorfreigabe abbrechen.",
                    "Ocupado: espere o cancele la aprobación del administrador.",
                    "処理中です。完了を待つか管理者承認を取り消してください。",
                    "正在操作，请等待完成或取消管理员授权。",
                    "Занято: дождитесь операции или отмените разрешение администратора.",
                ],
            )
            .to_owned(),
            Color::Yellow,
        );
        return Ok(false);
    }
    if action == Action::Quit {
        return Ok(true);
    }
    #[cfg(not(windows))]
    if action == Action::Mute && state.mute_state == MicrophoneMuteState::Unavailable {
        state.message(microphone_label(lang, state.mute_state), Color::Yellow);
        return Ok(false);
    }
    if action_disabled(state, action) {
        state.message(
            text(
                lang,
                [
                    "This control is unavailable.",
                    "Cette commande est indisponible.",
                    "Diese Steuerung ist nicht verfügbar.",
                    "Este control no está disponible.",
                    "この操作は利用できません。",
                    "此操作不可用。",
                    "Это управление недоступно.",
                ],
            )
            .to_owned(),
            Color::Yellow,
        );
        return Ok(false);
    }
    let command = match action {
        Action::BlockCamera | Action::RestoreCamera => {
            let desired = if action == Action::BlockCamera {
                CameraPrivacyState::Blocked
            } else {
                CameraPrivacyState::Allowed
            };
            #[cfg(not(windows))]
            if !cfg!(target_os = "linux")
                && state.camera_state == Some(desired)
                && state.camera_error.is_none()
            {
                state.message(camera_label(lang, desired).to_owned(), Color::Yellow);
                return Ok(false);
            }
            state.camera_error = None;
            WorkerCommand::Camera(desired)
        }
        Action::Mute => WorkerCommand::Mute,
        Action::Kill => {
            let Some(target) = accesses.get(state.selected_access) else {
                state.pending_kill = None;
                state.message(
                    text(
                        lang,
                        [
                            "Select a process first",
                            "Sélectionnez un processus",
                            "Zuerst einen Prozess auswählen",
                            "Seleccione un proceso",
                            "プロセスを選択してください",
                            "请先选择进程",
                            "Сначала выберите процесс",
                        ],
                    )
                    .to_owned(),
                    Color::Yellow,
                );
                return Ok(false);
            };
            let Some(pid) = target.pid else {
                state.pending_kill = None;
                state.message(
                    text(
                        lang,
                        [
                            "Cannot terminate: no process PID was identified",
                            "Arrêt impossible : PID non identifié",
                            "Beenden nicht möglich: keine Prozess-PID",
                            "No se puede finalizar: PID no identificado",
                            "終了不可：プロセスPIDが不明です",
                            "无法终止：未识别进程PID",
                            "Завершение невозможно: PID не определён",
                        ],
                    )
                    .to_owned(),
                    Color::Yellow,
                );
                return Ok(false);
            };
            if pid == 0 || pid == 4 || pid == std::process::id() {
                state.pending_kill = None;
                state.message(
                    text(
                        lang,
                        [
                            "Cannot terminate a protected system or monitor process",
                            "Impossible d'arrêter un processus système ou le moniteur",
                            "System- oder Monitorprozess ist geschützt",
                            "Proceso del sistema o monitor protegido",
                            "システムまたは監視プロセスは終了できません",
                            "无法终止受保护的系统或监视进程",
                            "Системный процесс или монитор защищён",
                        ],
                    )
                    .to_owned(),
                    Color::Red,
                );
                return Ok(false);
            }
            let Some(instance) = target
                .process
                .as_ref()
                .map(|process| process.instance_id.as_str())
                .filter(|instance| !instance.is_empty())
            else {
                state.pending_kill = None;
                state.message(
                    text(
                        lang,
                        [
                            "Cannot terminate: process identity is not verified",
                            "Arrêt impossible : identité du processus non vérifiée",
                            "Beenden nicht möglich: Prozessidentität ungeprüft",
                            "No se puede finalizar: identidad no verificada",
                            "終了不可：プロセスの同一性を確認できません",
                            "无法终止：进程身份未经验证",
                            "Завершение невозможно: личность процесса не проверена",
                        ],
                    )
                    .to_owned(),
                    Color::Yellow,
                );
                return Ok(false);
            };
            if !kill_confirmed(&state.pending_kill, &target.key, pid, instance) {
                state.pending_kill =
                    Some((target.key.clone(), pid, instance.to_owned(), Instant::now()));
                state.message(
                    format!(
                        "{} {} (PID {pid})",
                        text(
                            lang,
                            [
                                "Press [k] or click Terminate again within 3s; Esc cancels:",
                                "Appuyez sur [k] ou cliquez encore sous 3 s ; Échap annule :",
                                "[k] oder Beenden innerhalb 3 s erneut; Esc bricht ab:",
                                "Pulse [k] o haga clic otra vez en 3 s; Esc cancela:",
                                "3秒以内に[k]または終了を再度押す。Escで取消:",
                                "3秒内再次按[k]或点击终止；Esc取消：",
                                "Нажмите [k] или Завершить ещё раз за 3 с; Esc — отмена:"
                            ]
                        ),
                        target.application
                    ),
                    Color::Yellow,
                );
                return Ok(false);
            }
            state.pending_kill = None;
            WorkerCommand::Kill {
                pid,
                instance: instance.to_owned(),
            }
        }
        Action::Refresh | Action::Quit => unreachable!(),
    };
    match worker.send(command) {
        Ok(()) => {
            state.action_pending = Some(action);
            state.camera_pending = matches!(action, Action::BlockCamera | Action::RestoreCamera);
            state.message(
                text(
                    lang,
                    [
                        "Working…",
                        "Opération en cours…",
                        "Vorgang läuft…",
                        "En curso…",
                        "処理中…",
                        "处理中…",
                        "Выполняется…",
                    ],
                )
                .to_owned(),
                Color::Yellow,
            );
        }
        Err(error) => state.message(format!("{error:#}"), Color::Red),
    }
    Ok(false)
}

fn tui_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    policy: Policy,
    lang: Language,
) -> Result<()> {
    let worker = ActionWorker::new(policy.clone());
    let (refresh, observations) = observe(policy);
    let mut state = TuiState {
        lang,
        selected_access: 0,
        kill_available: false,
        table_state: TableState::default(),
        events_log: Vec::new(),
        status_msg: None,
        mute_state: MicrophoneMuteState::Unavailable,
        mute_changed_at: None,
        #[cfg(windows)]
        microphone_protection: None,
        #[cfg(windows)]
        camera_observation: None,
        camera_changed_at: None,
        #[cfg(windows)]
        protection_feedback: lang.protection_limit().to_owned(),
        devices: Vec::new(),
        health_error: Some(
            text(
                lang,
                [
                    "Waiting for the first native scan…",
                    "En attente du premier relevé natif…",
                    "Warten auf die erste native Erfassung…",
                    "Esperando el primer análisis nativo…",
                    "最初のネイティブスキャンを待機中…",
                    "正在等待首次本机扫描…",
                    "Ожидание первого системного сканирования…",
                ],
            )
            .to_owned(),
        ),
        pending_kill: None,
        operation_error: None,
        camera_state: None,
        camera_error: None,
        camera_pending: false,
        action_pending: None,
        controls_error: None,
        buttons: Vec::with_capacity(ACTIONS.len()),
    };
    let mut current_accesses: Vec<Access> = Vec::new();
    #[cfg(unix)]
    let mut tracker = crate::watcher::TransitionTracker::default();
    loop {
        match worker.responses.try_recv() {
            Ok(response) => {
                let completed_action = state.action_pending.take();
                match response {
                    WorkerResponse::Camera(result) => {
                        state.camera_pending = false;
                        state.camera_changed_at = Some(Instant::now());
                        match result {
                            Ok(camera) => {
                                if completed_action.is_some() {
                                    state.operation_error = None;
                                }
                                state.apply_camera(camera);
                                state.camera_error = None;
                                state.message(
                                    format!(
                                        "{}: {}",
                                        text(
                                            lang,
                                            [
                                                "Camera",
                                                "Caméra",
                                                "Kamera",
                                                "Cámara",
                                                "カメラ",
                                                "摄像头",
                                                "Камера"
                                            ]
                                        ),
                                        state.camera_summary()
                                    ),
                                    Color::Cyan,
                                );
                            }
                            Err(error) => {
                                state.camera_state = None;
                                state.camera_error = Some(format!("{error:#}"));
                                state.message(format!("{error:#}"), Color::Red);
                            }
                        }
                    }
                    WorkerResponse::Mute(result) => match result {
                        Ok(actual) => {
                            state.operation_error = None;
                            state.apply_microphone(actual);
                            state.mute_changed_at = Some(Instant::now());
                            let color = if matches!(
                                state.mute_state,
                                MicrophoneMuteState::Mixed | MicrophoneMuteState::Unavailable
                            ) {
                                Color::Yellow
                            } else {
                                Color::Green
                            };
                            state.message(state.microphone_summary(), color);
                            let _ = refresh.try_send(());
                        }
                        Err(error) => state.message(format!("{error:#}"), Color::Red),
                    },
                    WorkerResponse::Kill { pid, result } => match result {
                        Ok(()) => {
                            state.message(
                                format!(
                                    "{} PID {pid}",
                                    text(
                                        lang,
                                        [
                                            "Terminated process",
                                            "Processus terminé",
                                            "Prozess beendet",
                                            "Proceso finalizado",
                                            "プロセス終了",
                                            "已终止进程",
                                            "Процесс завершён"
                                        ]
                                    )
                                ),
                                Color::Green,
                            );
                            let _ = refresh.try_send(());
                        }
                        Err(error) => state.message(format!("PID {pid}: {error:#}"), Color::Red),
                    },
                }
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                state.camera_pending = false;
                state.action_pending = None;
                state.message(
                    text(
                        lang,
                        [
                            "Action worker stopped",
                            "Service d'action arrêté",
                            "Aktionsdienst gestoppt",
                            "Servicio de acciones detenido",
                            "操作ワーカー停止",
                            "操作线程已停止",
                            "Обработчик действий остановлен",
                        ],
                    )
                    .to_owned(),
                    Color::Red,
                );
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
        if let Ok(observation) = observations.try_recv() {
            if let Some(devices) = observation.devices {
                state.controls_error = None;
                match devices {
                    Ok(devices) => state.devices = devices,
                    Err(error) => state.controls_error = Some(format!("{error:#}")),
                }
            }
            if let Some((read_started, mute)) = observation.mute
                && state
                    .mute_changed_at
                    .is_none_or(|changed| read_started >= changed)
            {
                match mute {
                    Ok(mute) => state.apply_microphone(mute),
                    Err(error) => {
                        state.mute_state = MicrophoneMuteState::Unavailable;
                        state.controls_error = Some(format!("{error:#}"));
                    }
                }
            }
            if let Some((read_started, camera)) = observation.camera
                && !state.camera_pending
                && state
                    .camera_changed_at
                    .is_none_or(|changed| read_started >= changed)
            {
                match camera {
                    Ok(camera) => {
                        state.apply_camera(camera);
                        state.camera_error = None;
                    }
                    Err(error) => state.camera_error = Some(format!("{error:#}")),
                }
            }
            match observation.snapshot {
                Ok(snapshot) => {
                    #[cfg(unix)]
                    tracker.reconcile(&snapshot, |access, action| {
                        let label = match action {
                            crate::model::Action::Start => text(
                                lang,
                                [
                                    "START",
                                    "DÉBUT",
                                    "START",
                                    "INICIO",
                                    "開始",
                                    "开始",
                                    "НАЧАЛО",
                                ],
                            ),
                            crate::model::Action::Update => text(
                                lang,
                                [
                                    "UPDATE",
                                    "MISE À JOUR",
                                    "ÄNDERUNG",
                                    "CAMBIO",
                                    "更新",
                                    "更新",
                                    "ОБНОВЛЕНИЕ",
                                ],
                            ),
                            crate::model::Action::Stop => text(
                                lang,
                                ["STOP", "FIN", "ENDE", "FIN", "終了", "结束", "КОНЕЦ"],
                            ),
                        };
                        state.events_log.push((
                            Local::now().format("%H:%M:%S").to_string(),
                            format!(
                                "{label} {} ({})",
                                access.application,
                                lang.resource(access.resource)
                            ),
                            if matches!(action, crate::model::Action::Stop) {
                                Color::DarkGray
                            } else {
                                Color::Green
                            },
                        ));
                        if state.events_log.len() > 100 {
                            state.events_log.remove(0);
                        }
                        Ok(())
                    })?;
                    #[cfg(windows)]
                    for access in &snapshot.accesses {
                        if !current_accesses.iter().any(|a| a.key == access.key) {
                            let time = Local::now().format("%H:%M:%S").to_string();
                            let text = format!(
                                "{} {} ({})",
                                text(
                                    lang,
                                    [
                                        "START",
                                        "DÉBUT",
                                        "START",
                                        "INICIO",
                                        "開始",
                                        "开始",
                                        "НАЧАЛО"
                                    ]
                                ),
                                access.application,
                                lang.resource(access.resource)
                            );
                            state.events_log.push((time, text, Color::Green));
                            if state.events_log.len() > 100 {
                                state.events_log.remove(0);
                            }
                        }
                    }
                    #[cfg(windows)]
                    for previous in &current_accesses {
                        if !snapshot.accesses.iter().any(|a| a.key == previous.key) {
                            let time = Local::now().format("%H:%M:%S").to_string();
                            let text = format!(
                                "{} {} ({})",
                                text(
                                    lang,
                                    ["STOP", "FIN", "ENDE", "FIN", "終了", "结束", "КОНЕЦ"]
                                ),
                                previous.application,
                                lang.resource(previous.resource)
                            );
                            state.events_log.push((time, text, Color::DarkGray));
                            if state.events_log.len() > 100 {
                                state.events_log.remove(0);
                            }
                        }
                    }
                    state.health_error = snapshot
                        .collectors
                        .iter()
                        .find(|collector| collector.state != CollectorState::Healthy)
                        .map(|collector| {
                            format!(
                                "{}: {}",
                                collector.collector,
                                collector.detail.as_deref().unwrap_or(text(
                                    lang,
                                    [
                                        "degraded",
                                        "dégradée",
                                        "eingeschränkt",
                                        "degradada",
                                        "低下",
                                        "降级",
                                        "Снижение"
                                    ]
                                ))
                            )
                        });
                    current_accesses = snapshot.accesses;
                    state.selected_access = state
                        .selected_access
                        .min(current_accesses.len().saturating_sub(1));
                }
                Err(error) => {
                    #[cfg(unix)]
                    tracker.observation_gap();
                    state.health_error = Some(format!(
                        "{}: {error:#}",
                        text(
                            lang,
                            [
                                "Collection failed",
                                "Collecte échouée",
                                "Erfassung fehlgeschlagen",
                                "Error de recolección",
                                "収集失敗",
                                "采集失败",
                                "Ошибка сбора"
                            ]
                        )
                    ))
                }
            }
            #[cfg(windows)]
            state.update_protection_feedback();
        }

        // Draw UI
        terminal.draw(|f| draw_ui(f, &mut state, &current_accesses))?;

        if event::poll(Duration::from_millis(50))? {
            let action = match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    let code = normalize_key(key.code);
                    match code {
                        KeyCode::Esc if state.pending_kill.is_some() => {
                            state.pending_kill = None;
                            state.message(
                                text(
                                    lang,
                                    [
                                        "Termination cancelled",
                                        "Arrêt annulé",
                                        "Beenden abgebrochen",
                                        "Finalización cancelada",
                                        "終了をキャンセル",
                                        "已取消终止",
                                        "Завершение отменено",
                                    ],
                                )
                                .to_owned(),
                                Color::Yellow,
                            );
                            None
                        }
                        KeyCode::Up => {
                            state.pending_kill = None;
                            state.selected_access = state.selected_access.saturating_sub(1);
                            None
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            state.pending_kill = None;
                            state.selected_access = (state.selected_access + 1)
                                .min(current_accesses.len().saturating_sub(1));
                            None
                        }
                        _ => key_action(code),
                    }
                }
                Event::Mouse(mouse) if mouse.kind == MouseEventKind::Down(MouseButton::Left) => {
                    mouse_action(&state, mouse.column, mouse.row)
                }
                _ => None,
            };
            if let Some(action) = action
                && handle_action(action, &mut state, &current_accesses, &worker, &refresh)?
            {
                break;
            }
        }
    }

    Ok(())
}

fn draw_ui(f: &mut Frame, state: &mut TuiState, accesses: &[Access]) {
    state.kill_available = accesses.get(state.selected_access).is_some_and(|access| {
        access
            .pid
            .is_some_and(|pid| pid != 0 && pid != 4 && pid != std::process::id())
            && access
                .process
                .as_ref()
                .is_some_and(|process| !process.instance_id.is_empty())
    });
    let area = f.area();
    let footer_height = if area.height >= 18 {
        7
    } else if area.height >= 8 {
        5
    } else {
        area.height.min(2)
    };
    let body_height = area.height.saturating_sub(footer_height);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(if body_height >= 12 { 3 } else { 0 }),
            Constraint::Length(if body_height >= 20 { 9 } else { 0 }),
            Constraint::Min(0),
            Constraint::Length(if body_height >= 30 { 7 } else { 0 }),
            Constraint::Length(footer_height),
        ])
        .split(f.area());

    // 1. Header
    let time_str = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let mute_badge = match state.mute_state {
        MicrophoneMuteState::Muted => Span::styled(
            text(
                state.lang,
                [
                    " [SDK MUTED] ",
                    " [SDK COUPÉ] ",
                    " [SDK STUMM] ",
                    " [SDK SILENCIO] ",
                    " [SDK消音] ",
                    " [SDK静音] ",
                    " [SDK ЗАГЛУШЕН] ",
                ],
            ),
            Style::default().bg(Color::Red).fg(Color::White).bold(),
        ),
        MicrophoneMuteState::Unmuted => Span::styled(
            text(
                state.lang,
                [
                    " [SDK UNMUTED] ",
                    " [SDK NON COUPÉ] ",
                    " [SDK NICHT STUMM] ",
                    " [SDK SIN SILENCIO] ",
                    " [SDK非消音] ",
                    " [SDK未静音] ",
                    " [SDK НЕ ЗАГЛУШЕН] ",
                ],
            ),
            Style::default().bg(Color::Green).fg(Color::Black).bold(),
        ),
        MicrophoneMuteState::Mixed => Span::styled(
            text(
                state.lang,
                [
                    " [SDK MIXED] ",
                    " [SDK MIXTE] ",
                    " [SDK GEMISCHT] ",
                    " [SDK MIXTO] ",
                    " [SDK混在] ",
                    " [SDK混合] ",
                    " [SDK СМЕШАН] ",
                ],
            ),
            Style::default().bg(Color::Yellow).fg(Color::Black).bold(),
        ),
        MicrophoneMuteState::Unavailable => Span::styled(
            text(
                state.lang,
                [
                    " [SDK UNKNOWN] ",
                    " [SDK INCONNU] ",
                    " [SDK UNBEKANNT] ",
                    " [SDK DESCONOCIDO] ",
                    " [SDK不明] ",
                    " [SDK未知] ",
                    " [SDK НЕИЗВЕСТНО] ",
                ],
            ),
            Style::default().bg(Color::DarkGray).fg(Color::White).bold(),
        ),
    };

    let title_line = Line::from(vec![
        Span::styled(" miccamwatch ", Style::default().fg(Color::Cyan).bold()),
        Span::styled(
            format!("v{} ", env!("CARGO_PKG_VERSION")),
            Style::default().fg(Color::DarkGray),
        ),
        Span::raw("— "),
        Span::styled(time_str, Style::default().fg(Color::White)),
        Span::raw("   "),
        mute_badge,
        Span::styled(
            microphone_scope(state.lang),
            Style::default().fg(Color::Yellow),
        ),
    ]);

    let header_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let header_para = Paragraph::new(title_line).block(header_block);
    f.render_widget(header_para, chunks[0]);

    // 2. Top panels
    let top_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(chunks[1]);

    // Devices & Health
    let mut dev_lines = Vec::new();
    for dev in state.devices.iter().take(4) {
        let tag = match dev.resource {
            Resource::Microphone => {
                Span::styled("MIC ", Style::default().fg(Color::Magenta).bold())
            }
            Resource::Camera => Span::styled("CAM ", Style::default().fg(Color::Cyan).bold()),
        };
        dev_lines.push(Line::from(vec![
            tag,
            Span::styled(&dev.name, Style::default().fg(Color::White)),
        ]));
    }
    if dev_lines.is_empty() {
        dev_lines.push(Line::from(Span::styled(
            text(
                state.lang,
                [
                    "No capture devices found",
                    "Aucun périphérique de capture",
                    "Keine Aufnahmegeräte gefunden",
                    "No se encontraron dispositivos",
                    "キャプチャデバイスなし",
                    "未找到采集设备",
                    "Устройства захвата не найдены",
                ],
            ),
            Style::default().fg(Color::DarkGray),
        )));
    }
    let dev_block = Block::default()
        .title(text(
            state.lang,
            [
                " Devices ",
                " Périphériques ",
                " Geräte ",
                " Dispositivos ",
                " デバイス ",
                " 设备 ",
                " Устройства ",
            ],
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue));
    f.render_widget(Paragraph::new(dev_lines).block(dev_block), top_chunks[0]);

    // Stats & Status Message
    let active_mics = accesses
        .iter()
        .filter(|a| a.resource == Resource::Microphone && a.activity == Activity::Active)
        .count();
    let active_cams = accesses
        .iter()
        .filter(|a| a.resource == Resource::Camera && a.activity == Activity::Active)
        .count();
    let ready_cams = accesses
        .iter()
        .filter(|a| a.resource == Resource::Camera && a.activity == Activity::Ready)
        .count();
    let suspicious_count = accesses
        .iter()
        .filter(|a| a.risk >= Risk::Suspicious)
        .count();

    let mut stat_lines = vec![
        Line::from(vec![
            Span::raw(text(
                state.lang,
                [
                    "Active Microphones: ",
                    "Micros actifs : ",
                    "Aktive Mikrofone: ",
                    "Micrófonos activos: ",
                    "使用中のマイク: ",
                    "活动麦克风: ",
                    "Активные микрофоны: ",
                ],
            )),
            Span::styled(
                active_mics.to_string(),
                Style::default()
                    .fg(if active_mics > 0 {
                        Color::Green
                    } else {
                        Color::DarkGray
                    })
                    .bold(),
            ),
            Span::raw(text(
                state.lang,
                [
                    "   Active Cameras: ",
                    "   Caméras actives : ",
                    "   Aktive Kameras: ",
                    "   Cámaras activas: ",
                    "   使用中のカメラ: ",
                    "   活动摄像头: ",
                    "   Активные камеры: ",
                ],
            )),
            Span::styled(
                active_cams.to_string(),
                Style::default()
                    .fg(if active_cams > 0 {
                        Color::Cyan
                    } else {
                        Color::DarkGray
                    })
                    .bold(),
            ),
        ]),
        Line::from(vec![
            Span::raw(text(
                state.lang,
                [
                    "Camera-ready pipelines: ",
                    "Pipelines caméra prêts : ",
                    "Bereite Kamerapipelines: ",
                    "Cámaras preparadas: ",
                    "カメラ準備中: ",
                    "摄像头就绪: ",
                    "Готовые потоки камеры: ",
                ],
            )),
            Span::styled(
                ready_cams.to_string(),
                Style::default().fg(Color::Yellow).bold(),
            ),
        ]),
        Line::from(vec![
            Span::raw(text(
                state.lang,
                [
                    "Risk alerts (Suspicious/Blocked): ",
                    "Alertes (suspect/bloqué) : ",
                    "Alarme (verdächtig/gesperrt): ",
                    "Alertas (sospechoso/bloqueado): ",
                    "警告 (疑わしい/ブロック): ",
                    "风险警报(可疑/阻止): ",
                    "Риски (подозр./блок.): ",
                ],
            )),
            Span::styled(
                suspicious_count.to_string(),
                Style::default()
                    .fg(if suspicious_count > 0 {
                        Color::Red
                    } else {
                        Color::Green
                    })
                    .bold(),
            ),
        ]),
    ];
    #[cfg(unix)]
    stat_lines.push(Line::from(Span::styled(
        microphone_scope(state.lang),
        Style::default().fg(Color::Yellow),
    )));
    #[cfg(target_os = "macos")]
    stat_lines.push(Line::from(Span::styled(
        text(
            state.lang,
            [
                "Camera profile: manual approval/removal only",
                "Profil caméra : approbation/retrait manuels",
                "Kameraprofil: nur manuell genehmigen/entfernen",
                "Perfil de cámara: aprobación/eliminación manual",
                "カメラプロファイル：手動承認・削除のみ",
                "摄像头描述文件：仅手动批准或移除",
                "Профиль камеры: только ручное одобрение/удаление",
            ],
        ),
        Style::default().fg(Color::Yellow),
    )));
    #[cfg(target_os = "linux")]
    stat_lines.push(Line::from(Span::styled(
        privacy::camera_capability(),
        Style::default().fg(Color::Yellow),
    )));

    let camera_status = if state.camera_pending {
        text(
            state.lang,
            [
                "Working…",
                "Opération en cours…",
                "Vorgang läuft…",
                "En curso…",
                "処理中…",
                "处理中…",
                "Выполняется…",
            ],
        )
    } else if state.camera_error.is_some() {
        text(
            state.lang,
            [
                "Error",
                "Erreur",
                "Fehler",
                "Error",
                "エラー",
                "错误",
                "Ошибка",
            ],
        )
    } else {
        state.camera_state.map_or(
            text(
                state.lang,
                [
                    "Unknown",
                    "Inconnue",
                    "Unbekannt",
                    "Desconocida",
                    "不明",
                    "未知",
                    "Неизвестно",
                ],
            ),
            |camera| camera_label(state.lang, camera),
        )
    };
    stat_lines.push(Line::from(vec![
        Span::raw(text(
            state.lang,
            [
                "Camera privacy: ",
                "Confidentialité caméra : ",
                "Kameraschutz: ",
                "Privacidad cámara: ",
                "カメラ保護: ",
                "摄像头隐私: ",
                "Приватность камеры: ",
            ],
        )),
        Span::styled(
            camera_status,
            Style::default()
                .fg(if state.camera_error.is_some() {
                    Color::Red
                } else {
                    match state.camera_state {
                        Some(CameraPrivacyState::Blocked) => Color::Red,
                        Some(CameraPrivacyState::Allowed) if !state.camera_pending => Color::Green,
                        _ => Color::Yellow,
                    }
                })
                .bold(),
        ),
    ]));
    if let Some(error) = &state.camera_error {
        stat_lines.push(Line::from(Span::styled(
            error.as_str(),
            Style::default().fg(Color::Red),
        )));
    }
    if let Some((msg, created, color)) = &state.status_msg
        && created.elapsed() < Duration::from_secs(4)
    {
        stat_lines.push(Line::from(vec![
            Span::styled(
                text(
                    state.lang,
                    [
                        "Action: ",
                        "Action : ",
                        "Aktion: ",
                        "Acción: ",
                        "操作: ",
                        "操作: ",
                        "Действие: ",
                    ],
                ),
                Style::default().bold(),
            ),
            Span::styled(msg, Style::default().fg(*color).bold()),
        ]));
    }
    if let Some(error) = &state.health_error {
        stat_lines.push(Line::from(vec![
            Span::styled(
                text(
                    state.lang,
                    [
                        "Telemetry: ",
                        "Télémétrie : ",
                        "Telemetrie: ",
                        "Telemetría: ",
                        "監視状態: ",
                        "遥测: ",
                        "Телеметрия: ",
                    ],
                ),
                Style::default().bold(),
            ),
            Span::styled(error, Style::default().fg(Color::Red).bold()),
        ]));
    }

    let stats_block = Block::default()
        .title(text(
            state.lang,
            [
                " Summary ",
                " Résumé ",
                " Übersicht ",
                " Resumen ",
                " 概要 ",
                " 摘要 ",
                " Сводка ",
            ],
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue));
    f.render_widget(Paragraph::new(stat_lines).block(stats_block), top_chunks[1]);

    // 3. Active Accesses Table
    let table_block = Block::default()
        .title(text(
            state.lang,
            [
                " Observed Access & Camera-Ready Pipelines ",
                " Accès observés et pipelines caméra prêts ",
                " Beobachtete Zugriffe und Kamerabereitschaft ",
                " Accesos observados y cámaras preparadas ",
                " 監視中のアクセスとカメラ準備状態 ",
                " 已观察访问和摄像头就绪管线 ",
                " Наблюдаемые доступы и готовность камеры ",
            ],
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::White));

    if accesses.is_empty() {
        let empty_msg = if state.health_error.is_some() {
            Paragraph::new(Line::from(Span::styled(
                text(
                    state.lang,
                    [
                        "Capture activity cannot be determined.",
                        "Impossible de déterminer l'activité de capture.",
                        "Aufnahmeaktivität nicht feststellbar.",
                        "No se puede determinar la actividad de captura.",
                        "キャプチャ状態を確認できません。",
                        "无法确定采集活动。",
                        "Активность захвата не определена.",
                    ],
                ),
                Style::default().fg(Color::Yellow),
            )))
        } else {
            Paragraph::new(Line::from(vec![
                Span::styled("✔ ", Style::default().fg(Color::Green).bold()),
                Span::styled(
                    text(
                        state.lang,
                        [
                            "No active capture observed.",
                            "Aucune capture active observée.",
                            "Keine aktive Aufnahme beobachtet.",
                            "No se observó captura activa.",
                            "アクティブなキャプチャはありません。",
                            "未检测到活动采集。",
                            "Активный захват не обнаружен.",
                        ],
                    ),
                    Style::default().fg(Color::Green),
                ),
            ]))
        }
        .block(table_block);
        f.render_widget(empty_msg, chunks[2]);
    } else {
        let rows = accesses.iter().enumerate().map(|(idx, a)| {
            let res_cell = match a.resource {
                Resource::Microphone => Cell::from(state.lang.resource(a.resource))
                    .style(Style::default().fg(Color::Magenta).bold()),
                Resource::Camera => Cell::from(state.lang.resource(a.resource))
                    .style(Style::default().fg(Color::Cyan).bold()),
            };
            let act_str = state.lang.state_str(None, a.activity);
            let act_cell = match a.activity {
                Activity::Active => {
                    Cell::from(act_str).style(Style::default().fg(Color::Green).bold())
                }
                Activity::Ready => {
                    Cell::from(act_str).style(Style::default().fg(Color::Yellow).bold())
                }
            };
            let risk_color = match a.risk {
                Risk::Expected => Color::Green,
                Risk::Unexplained => Color::Yellow,
                Risk::Suspicious => Color::Rgb(255, 140, 0),
                Risk::Blocked => Color::Red,
            };
            let risk_cell = Cell::from(state.lang.risk_str(a.risk))
                .style(Style::default().fg(risk_color).bold());
            let app_cell =
                Cell::from(a.application.clone()).style(Style::default().fg(Color::White).bold());
            let pid_cell = Cell::from(a.pid.map_or("?".into(), |p| p.to_string()))
                .style(Style::default().fg(Color::Cyan));
            let parent_cell = Cell::from(a.parent_name.clone().unwrap_or_else(|| "-".into()))
                .style(Style::default().fg(Color::DarkGray));
            let sig_cell = match &a.signature {
                Some(s) if s.verified => Cell::from(text(
                    state.lang,
                    [
                        "Verified",
                        "Vérifiée",
                        "Verifiziert",
                        "Verificada",
                        "検証済み",
                        "已验证",
                        "Проверена",
                    ],
                ))
                .style(Style::default().fg(Color::Green)),
                Some(_) => Cell::from(text(
                    state.lang,
                    [
                        "Unverified",
                        "Non vérifiée",
                        "Ungeprüft",
                        "No verificada",
                        "未検証",
                        "未验证",
                        "Не проверена",
                    ],
                ))
                .style(Style::default().fg(Color::Red)),
                None => Cell::from("-").style(Style::default().fg(Color::DarkGray)),
            };

            let row = Row::new(vec![
                res_cell,
                act_cell,
                risk_cell,
                app_cell,
                pid_cell,
                parent_cell,
                sig_cell,
            ]);
            if idx == state.selected_access {
                row.style(Style::default().bg(Color::Rgb(30, 41, 59)))
            } else {
                row
            }
        });

        let header = Row::new(vec![
            Cell::from(text(
                state.lang,
                ["Type", "Type", "Typ", "Tipo", "種類", "类型", "Тип"],
            ))
            .style(Style::default().bold()),
            Cell::from(text(
                state.lang,
                [
                    "State",
                    "État",
                    "Status",
                    "Estado",
                    "状態",
                    "状态",
                    "Статус",
                ],
            ))
            .style(Style::default().bold()),
            Cell::from(text(
                state.lang,
                [
                    "Risk",
                    "Risque",
                    "Risiko",
                    "Riesgo",
                    "リスク",
                    "风险",
                    "Риск",
                ],
            ))
            .style(Style::default().bold()),
            Cell::from(text(
                state.lang,
                [
                    "Process",
                    "Processus",
                    "Prozess",
                    "Proceso",
                    "プロセス",
                    "进程",
                    "Процесс",
                ],
            ))
            .style(Style::default().bold()),
            Cell::from("PID").style(Style::default().bold()),
            Cell::from(text(
                state.lang,
                [
                    "Parent",
                    "Parent",
                    "Eltern",
                    "Padre",
                    "親",
                    "父进程",
                    "Родитель",
                ],
            ))
            .style(Style::default().bold()),
            Cell::from(text(
                state.lang,
                [
                    "Signature",
                    "Signature",
                    "Signatur",
                    "Firma",
                    "署名",
                    "签名",
                    "Подпись",
                ],
            ))
            .style(Style::default().bold()),
        ])
        .style(Style::default().fg(Color::Cyan));

        let widths = if chunks[2].width >= 110 {
            [
                Constraint::Length(6),
                Constraint::Length(9),
                Constraint::Length(12),
                Constraint::Min(24),
                Constraint::Length(8),
                Constraint::Length(18),
                Constraint::Length(12),
            ]
        } else {
            [
                Constraint::Length(6),
                Constraint::Length(9),
                Constraint::Length(0),
                Constraint::Min(8),
                Constraint::Length(7),
                Constraint::Length(0),
                Constraint::Length(0),
            ]
        };

        let table = Table::new(rows, widths).header(header).block(table_block);
        state.table_state.select(Some(state.selected_access));
        f.render_stateful_widget(table, chunks[2], &mut state.table_state);
    }

    // 4. Event Stream Log
    let log_block = Block::default()
        .title(text(
            state.lang,
            [
                " Event Stream ",
                " Flux d’événements ",
                " Ereignisse ",
                " Eventos ",
                " イベント ",
                " 事件流 ",
                " События ",
            ],
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    let log_lines: Vec<Line> = state
        .events_log
        .iter()
        .rev()
        .take(5)
        .rev()
        .map(|(time, text, col)| {
            Line::from(vec![
                Span::styled(format!("{time}  "), Style::default().fg(Color::DarkGray)),
                Span::styled(text, Style::default().fg(*col)),
            ])
        })
        .collect();
    f.render_widget(Paragraph::new(log_lines).block(log_block), chunks[3]);

    draw_controls(f, state, chunks[4]);
}

const ACTIONS: [Action; 6] = [
    Action::BlockCamera,
    Action::RestoreCamera,
    Action::Mute,
    Action::Kill,
    Action::Refresh,
    Action::Quit,
];

fn button_label(action: Action, lang: Language) -> &'static str {
    #[cfg(windows)]
    if action == Action::Mute {
        return text(
            lang,
            [
                "[m] Protect/Release mic",
                "[m] Protéger/libérer micro",
                "[m] Mikrofonschutz/Freigabe",
                "[m] Proteger/liberar mic",
                "[m] マイク保護/解除",
                "[m] 麦克风保护/解除",
                "[m] Защитить/снять мик",
            ],
        );
    }
    #[cfg(target_os = "linux")]
    if matches!(action, Action::BlockCamera | Action::RestoreCamera) {
        return text(
            lang,
            match action {
                Action::BlockCamera => [
                    "[b] Camera control unavailable",
                    "[b] Commande caméra indisponible",
                    "[b] Kamerasteuerung nicht verfügbar",
                    "[b] Control de cámara no disponible",
                    "[b] カメラ操作は利用不可",
                    "[b] 摄像头控制不可用",
                    "[b] Управление камерой недоступно",
                ],
                _ => [
                    "[a] Camera control unavailable",
                    "[a] Commande caméra indisponible",
                    "[a] Kamerasteuerung nicht verfügbar",
                    "[a] Control de cámara no disponible",
                    "[a] カメラ操作は利用不可",
                    "[a] 摄像头控制不可用",
                    "[a] Управление камерой недоступно",
                ],
            },
        );
    }
    #[cfg(target_os = "macos")]
    match action {
        Action::BlockCamera => {
            return text(
                lang,
                [
                    "[b] Approve camera profile",
                    "[b] Approuver profil caméra",
                    "[b] Kameraprofil genehmigen",
                    "[b] Aprobar perfil de cámara",
                    "[b] カメラプロファイルを承認",
                    "[b] 批准摄像头描述文件",
                    "[b] Одобрить профиль камеры",
                ],
            );
        }
        Action::RestoreCamera => {
            return text(
                lang,
                [
                    "[a] Remove owned profile",
                    "[a] Retirer notre profil",
                    "[a] Eigenes Profil entfernen",
                    "[a] Eliminar perfil propio",
                    "[a] 所有プロファイル削除",
                    "[a] 移除本应用描述文件",
                    "[a] Удалить собственный профиль",
                ],
            );
        }
        _ => {}
    }
    match action {
        Action::BlockCamera => text(
            lang,
            [
                "[b] Block all cameras",
                "[b] Bloquer les caméras",
                "[b] Alle Kameras sperren",
                "[b] Bloquear todas las cámaras",
                "[b] 全カメラをブロック",
                "[b] 阻止全部摄像头",
                "[b] Блокировать все камеры",
            ],
        ),
        Action::RestoreCamera => text(
            lang,
            [
                "[a] Allow all cameras",
                "[a] Autoriser les caméras",
                "[a] Alle Kameras freigeben",
                "[a] Permitir todas las cámaras",
                "[a] 全カメラを許可",
                "[a] 允许全部摄像头",
                "[a] Разрешить все камеры",
            ],
        ),
        Action::Mute => text(
            lang,
            [
                "[m] Toggle mic mute",
                "[m] Couper/activer micro",
                "[m] Mikrofon umschalten",
                "[m] Silenciar/activar mic",
                "[m] マイク消音切替",
                "[m] 切换麦克风静音",
                "[m] Микрофон вкл/выкл",
            ],
        ),
        Action::Kill => text(
            lang,
            [
                "[k] Terminate process",
                "[k] Arrêter processus",
                "[k] Prozess beenden",
                "[k] Finalizar proceso",
                "[k] プロセス終了",
                "[k] 终止进程",
                "[k] Завершить процесс",
            ],
        ),
        Action::Refresh => text(
            lang,
            [
                "[r] Refresh",
                "[r] Actualiser",
                "[r] Aktualisieren",
                "[r] Actualizar",
                "[r] 更新",
                "[r] 刷新",
                "[r] Обновить",
            ],
        ),
        Action::Quit => text(
            lang,
            [
                "[q] Quit",
                "[q] Quitter",
                "[q] Ende",
                "[q] Salir",
                "[q] 終了",
                "[q] 退出",
                "[q] Выход",
            ],
        ),
    }
}

fn compact_button_label(action: Action, lang: Language) -> &'static str {
    #[cfg(windows)]
    if action == Action::Mute {
        return text(
            lang,
            [
                "[m] Protection",
                "[m] Protection",
                "[m] Schutz",
                "[m] Protección",
                "[m] 保護切替",
                "[m] 保护切换",
                "[m] Защита",
            ],
        );
    }
    match action {
        Action::BlockCamera if cfg!(target_os = "linux") => text(
            lang,
            [
                "[b] Camera N/A",
                "[b] Caméra indispo.",
                "[b] Kamera n.v.",
                "[b] Cámara N/D",
                "[b] カメラ不可",
                "[b] 摄像头不可用",
                "[b] Камера недост.",
            ],
        ),
        Action::RestoreCamera if cfg!(target_os = "linux") => text(
            lang,
            [
                "[a] Camera N/A",
                "[a] Caméra indispo.",
                "[a] Kamera n.v.",
                "[a] Cámara N/D",
                "[a] カメラ不可",
                "[a] 摄像头不可用",
                "[a] Камера недост.",
            ],
        ),
        Action::BlockCamera if cfg!(target_os = "macos") => text(
            lang,
            [
                "[b] Approve profile",
                "[b] Approuver profil",
                "[b] Profil erlauben",
                "[b] Aprobar perfil",
                "[b] プロファイル承認",
                "[b] 批准描述文件",
                "[b] Одобрить профиль",
            ],
        ),
        Action::RestoreCamera if cfg!(target_os = "macos") => text(
            lang,
            [
                "[a] Remove profile",
                "[a] Retirer profil",
                "[a] Profil entfernen",
                "[a] Quitar perfil",
                "[a] プロファイル削除",
                "[a] 移除描述文件",
                "[a] Удалить профиль",
            ],
        ),
        Action::BlockCamera => text(
            lang,
            [
                "[b] Block",
                "[b] Bloquer",
                "[b] Sperren",
                "[b] Bloquear",
                "[b] ブロック",
                "[b] 禁用",
                "[b] Блокировать",
            ],
        ),
        Action::RestoreCamera => text(
            lang,
            [
                "[a] Allow",
                "[a] Autoriser",
                "[a] Freigeben",
                "[a] Permitir",
                "[a] 許可",
                "[a] 允许",
                "[a] Разрешить",
            ],
        ),
        Action::Mute => text(
            lang,
            [
                "[m] Mic mute",
                "[m] Micro",
                "[m] Mikrofon",
                "[m] Micrófono",
                "[m] マイク",
                "[m] 麦克风",
                "[m] Микрофон",
            ],
        ),
        Action::Kill => text(
            lang,
            [
                "[k] Terminate",
                "[k] Arrêter",
                "[k] Beenden",
                "[k] Finalizar",
                "[k] 終了",
                "[k] 终止",
                "[k] Завершить",
            ],
        ),
        Action::Refresh | Action::Quit => button_label(action, lang),
    }
}

#[derive(Clone, Copy)]
enum ControlLabels {
    Full,
    Compact,
    Keys,
    EssentialKeys,
    BareKeys,
}

impl ControlLabels {
    fn label(self, action: Action, lang: Language) -> &'static str {
        match self {
            Self::Full => button_label(action, lang),
            Self::Compact => compact_button_label(action, lang),
            Self::Keys => &button_label(action, lang)[..3],
            Self::EssentialKeys if matches!(action, Action::Refresh | Action::Quit) => {
                &button_label(action, lang)[..3]
            }
            Self::EssentialKeys | Self::BareKeys => &button_label(action, lang)[1..2],
        }
    }
}

fn control_format(area: Rect, lang: Language) -> (ControlLabels, bool, [u16; ACTIONS.len()]) {
    for (labels, bordered) in [
        (ControlLabels::Full, true),
        (ControlLabels::Compact, true),
        (ControlLabels::Full, false),
        (ControlLabels::Compact, false),
        (ControlLabels::Keys, false),
        (ControlLabels::EssentialKeys, false),
        (ControlLabels::BareKeys, false),
    ] {
        if bordered && area.height < 3 {
            continue;
        }
        let widths = ACTIONS.map(|action| {
            Span::raw(labels.label(action, lang)).width() as u16 + if bordered { 4 } else { 0 }
        });
        let width = widths
            .iter()
            .map(|width| usize::from(*width))
            .sum::<usize>()
            + ACTIONS.len()
            - 1;
        if width <= usize::from(area.width) {
            return (labels, bordered, widths);
        }
    }
    (ControlLabels::BareKeys, false, [1; ACTIONS.len()])
}

fn button_rects(
    area: Rect,
    widths: [u16; ACTIONS.len()],
    bordered: bool,
) -> impl Iterator<Item = (Rect, Action)> {
    // Below eleven cells, even six bare keys plus gaps cannot fit. Keep quit
    // and refresh first; never introduce a second line or overlapping hitboxes.
    let actions = if area.width < 11 {
        [
            Action::Quit,
            Action::Refresh,
            Action::Mute,
            Action::Kill,
            Action::BlockCamera,
            Action::RestoreCamera,
        ]
    } else {
        ACTIONS
    };
    let mut x = area.x;
    actions
        .into_iter()
        .zip(widths)
        .filter_map(move |(action, width)| {
            let height = if bordered { 3 } else { 1 };
            if area.height < height || x.saturating_add(width) > area.right() {
                return None;
            }
            let rect = Rect::new(x, area.y, width, height);
            x = x.saturating_add(width + 1);
            Some((rect, action))
        })
}

fn mouse_action(state: &TuiState, x: u16, y: u16) -> Option<Action> {
    state
        .buttons
        .iter()
        .find(|(rect, action)| contains(*rect, x, y) && !action_disabled(state, *action))
        .map(|(_, action)| *action)
}

fn action_disabled(state: &TuiState, action: Action) -> bool {
    if cfg!(target_os = "linux") && matches!(action, Action::BlockCamera | Action::RestoreCamera) {
        return true;
    }
    if action == Action::Kill && !state.kill_available {
        return true;
    }
    if action == Action::Refresh {
        return false;
    }
    if state.action_pending.is_some() {
        return true;
    }
    if action == Action::Quit {
        return false;
    }
    if state.camera_pending {
        return true;
    }
    #[cfg(windows)]
    {
        match action {
            Action::Mute => false,
            Action::BlockCamera => {
                state.camera_observation.as_ref().is_some_and(|status| {
                    status.desired_blocked && status.helper_active && status.unknown_devices == 0
                }) && state.camera_error.is_none()
            }
            Action::RestoreCamera => {
                state.camera_observation.as_ref().is_some_and(|status| {
                    !status.desired_blocked
                        && status.pending_restore == 0
                        && status.unknown_devices == 0
                }) && state.camera_error.is_none()
            }
            _ => false,
        }
    }
    #[cfg(not(windows))]
    match action {
        Action::Mute => state.mute_state == MicrophoneMuteState::Unavailable,
        Action::BlockCamera => {
            state.camera_state == Some(CameraPrivacyState::Blocked) && state.camera_error.is_none()
        }
        Action::RestoreCamera => {
            state.camera_state == Some(CameraPrivacyState::Allowed) && state.camera_error.is_none()
        }
        _ => false,
    }
}

fn draw_controls(f: &mut Frame, state: &mut TuiState, area: Rect) {
    let heading_height = if cfg!(windows) {
        0
    } else {
        u16::from(area.height >= 5)
    };
    let feedback_height = if cfg!(windows) && area.height >= 5 {
        area.height
            .saturating_sub(if area.height >= 7 { 3 } else { 1 })
    } else if area.height >= 10 {
        5
    } else if area.height >= 7 {
        3
    } else {
        u16::from(area.height >= 4)
    };
    let controls = Rect::new(
        area.x,
        area.y + heading_height,
        area.width,
        area.height.saturating_sub(heading_height + feedback_height),
    );
    if heading_height != 0 {
        f.render_widget(
            Paragraph::new(text(
                state.lang,
                [
                    "Actions",
                    "Actions",
                    "Aktionen",
                    "Acciones",
                    "操作",
                    "操作",
                    "Действия",
                ],
            ))
            .style(Style::default().fg(Color::Gray).bold()),
            Rect::new(area.x, area.y, area.width, heading_height),
        );
    }
    let (labels, bordered, widths) = control_format(controls, state.lang);
    state.buttons.clear();
    state
        .buttons
        .extend(button_rects(controls, widths, bordered));
    for (rect, action) in &state.buttons {
        let disabled = action_disabled(state, *action);
        let color = if disabled {
            Color::DarkGray
        } else {
            match action {
                Action::BlockCamera | Action::Kill => Color::LightRed,
                Action::RestoreCamera => Color::LightGreen,
                _ => Color::LightCyan,
            }
        };
        let label = labels.label(*action, state.lang);
        let style = Style::default().fg(if disabled {
            Color::DarkGray
        } else {
            Color::White
        });
        let inner = if bordered {
            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(color));
            let inner = block.inner(*rect);
            f.render_widget(block, *rect);
            Rect::new(inner.x + 1, inner.y, inner.width.saturating_sub(2), 1)
        } else {
            *rect
        };
        let key_style = Style::default().fg(color).bold();
        let buffer = f.buffer_mut();
        if label.starts_with('[') {
            buffer.set_stringn(
                inner.x,
                inner.y,
                &label[..3],
                usize::from(inner.width),
                key_style,
            );
            if inner.width > 3 {
                buffer.set_stringn(
                    inner.x + 3,
                    inner.y,
                    &label[3..],
                    usize::from(inner.width - 3),
                    style,
                );
            }
        } else {
            buffer.set_stringn(inner.x, inner.y, label, usize::from(inner.width), key_style);
        }
    }
    let feedback = Rect::new(area.x, controls.bottom(), area.width, feedback_height);
    let (message, color) = if cfg!(windows) {
        #[cfg(windows)]
        {
            (
                state.protection_feedback.as_str(),
                if state.operation_error.is_some()
                    || state.camera_error.is_some()
                    || state.controls_error.is_some()
                {
                    Color::Yellow
                } else {
                    Color::White
                },
            )
        }
        #[cfg(not(windows))]
        {
            unreachable!()
        }
    } else if let Some((message, created, color)) = &state.status_msg
        && (*color == Color::Red
            || state.action_pending.is_some()
            || created.elapsed() < Duration::from_secs(8))
    {
        (message.as_str(), *color)
    } else if let Some(error) = state
        .camera_error
        .as_ref()
        .or(state.controls_error.as_ref())
        .or(state.health_error.as_ref())
    {
        (error.as_str(), Color::Red)
    } else if state.camera_pending {
        (
            text(
                state.lang,
                [
                    "Reading controls…",
                    "Lecture des commandes…",
                    "Steuerung lesen…",
                    "Leyendo controles…",
                    "制御状態を確認中…",
                    "正在读取控制状态…",
                    "Чтение состояния управления…",
                ],
            ),
            Color::Yellow,
        )
    } else {
        (
            text(
                state.lang,
                [
                    "Click a button or use its key. ↑/↓ selects a process.",
                    "Cliquez ou utilisez la touche. ↑/↓ sélectionne un processus.",
                    "Klicken oder Taste drücken. ↑/↓ wählt einen Prozess.",
                    "Haga clic o use la tecla. ↑/↓ selecciona un proceso.",
                    "ボタンまたはキーで操作。↑/↓でプロセス選択。",
                    "点击按钮或按快捷键。↑/↓选择进程。",
                    "Щёлкните кнопку или нажмите клавишу. ↑/↓ — выбор процесса.",
                ],
            ),
            Color::White,
        )
    };
    let paragraph = Paragraph::new(message).style(Style::default().fg(color));
    #[cfg(not(windows))]
    let paragraph = paragraph.wrap(Wrap { trim: false });
    f.render_widget(
        paragraph.block(
            Block::default()
                .borders(if feedback.height >= 3 && !cfg!(windows) {
                    Borders::ALL
                } else {
                    Borders::NONE
                })
                .title(text(
                    state.lang,
                    [
                        " Feedback / ↑↓ select ",
                        " Retour / ↑↓ choisir ",
                        " Meldung / ↑↓ wählen ",
                        " Mensaje / ↑↓ elegir ",
                        " メッセージ / ↑↓選択 ",
                        " 提示 / ↑↓选择 ",
                        " Сообщение / ↑↓ выбор ",
                    ],
                )),
        ),
        feedback,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;

    fn state(lang: Language) -> TuiState {
        TuiState {
            lang,
            selected_access: 0,
            kill_available: false,
            table_state: TableState::default(),
            events_log: Vec::new(),
            status_msg: None,
            mute_state: MicrophoneMuteState::Unavailable,
            mute_changed_at: None,
            #[cfg(windows)]
            microphone_protection: None,
            #[cfg(windows)]
            camera_observation: None,
            camera_changed_at: None,
            #[cfg(windows)]
            protection_feedback: lang.protection_limit().to_owned(),
            devices: Vec::new(),
            health_error: None,
            camera_state: None,
            camera_error: None,
            camera_pending: false,
            action_pending: None,
            operation_error: None,
            controls_error: None,
            buttons: Vec::new(),
            pending_kill: None,
        }
    }

    #[cfg(windows)]
    #[test]
    fn global_camera_controls_follow_intent_with_no_present_devices() {
        let mut state = state(Language::En);
        state.apply_camera(privacy::CameraControlObservation {
            desired_blocked: true,
            state: CameraPrivacyState::SystemManaged,
            present_total: 0,
            owned_blocked_present: 0,
            pending_restore: 0,
            absent_owned: 0,
            unknown_devices: 0,
            helper_active: true,
            detail: None,
        });
        assert!(action_disabled(&state, Action::BlockCamera));
        assert!(!action_disabled(&state, Action::RestoreCamera));
        state.camera_observation.as_mut().unwrap().desired_blocked = false;
        state.camera_observation.as_mut().unwrap().helper_active = false;
        assert!(!action_disabled(&state, Action::BlockCamera));
        assert!(action_disabled(&state, Action::RestoreCamera));
        state.camera_observation.as_mut().unwrap().pending_restore = 1;
        assert!(!action_disabled(&state, Action::RestoreCamera));
        state.camera_observation.as_mut().unwrap().desired_blocked = true;
        assert!(
            !action_disabled(&state, Action::BlockCamera),
            "an inactive helper permits retry"
        );
    }

    #[cfg(windows)]
    #[test]
    fn requested_microphone_protection_remains_releasable_when_sdk_is_unavailable() {
        let mut state = state(Language::En);
        state.apply_microphone(crate::platform::MicrophoneProtectionStatus {
            requested: true,
            service_active: false,
            mute_state: MicrophoneMuteState::Unavailable,
            endpoint_count: 0,
            hardware_mute_count: 0,
            corrections: 0,
            detail: Some("service unavailable".into()),
        });
        assert!(!action_disabled(&state, Action::Mute));
        assert!(state.microphone_protection.as_ref().unwrap().requested);
        state.message("operation failed".into(), Color::Red);
        state.apply_microphone(crate::platform::MicrophoneProtectionStatus {
            requested: true,
            service_active: true,
            mute_state: MicrophoneMuteState::Muted,
            endpoint_count: 1,
            hardware_mute_count: 0,
            corrections: 1,
            detail: None,
        });
        assert_eq!(state.operation_error.as_deref(), Some("operation failed"));
    }
    #[test]
    fn termination_confirmation_requires_same_live_identity() {
        let pending = Some((
            "access".to_owned(),
            42,
            "instance".to_owned(),
            Instant::now(),
        ));
        assert!(kill_confirmed(&pending, "access", 42, "instance"));
        assert!(!kill_confirmed(&pending, "other-access", 42, "instance"));
        assert!(!kill_confirmed(&pending, "access", 43, "instance"));
        assert!(!kill_confirmed(&pending, "access", 42, "reused-pid"));
        assert!(!kill_confirmed(&pending, "access", 42, ""));
        let expired = Some((
            "access".to_owned(),
            42,
            "instance".to_owned(),
            Instant::now() - Duration::from_secs(4),
        ));
        assert!(!kill_confirmed(&expired, "access", 42, "instance"));
    }

    #[test]
    fn privileged_operation_blocks_exit_until_completion() {
        let (commands, _requests) = mpsc::sync_channel(1);
        let (_responses, responses) = mpsc::sync_channel(1);
        let worker = ActionWorker {
            commands: Some(commands),
            responses,
            thread: None,
        };
        let (refresh, _refreshes) = mpsc::sync_channel(1);
        let mut state = state(Language::En);
        state.action_pending = Some(Action::BlockCamera);
        state.camera_pending = true;
        assert!(!handle_action(Action::Quit, &mut state, &[], &worker, &refresh).unwrap());
        assert_eq!(state.action_pending, Some(Action::BlockCamera));
        assert!(action_disabled(&state, Action::Quit));
        state.action_pending = None;
        state.camera_pending = false;
        assert!(handle_action(Action::Quit, &mut state, &[], &worker, &refresh).unwrap());
    }

    #[test]
    fn control_requests_are_bounded_and_refresh_requests_coalesce() {
        let (commands, requests) = mpsc::sync_channel(1);
        let (_responses, responses) = mpsc::sync_channel(1);
        let worker = ActionWorker {
            commands: Some(commands),
            responses,
            thread: None,
        };
        assert!(worker.send(WorkerCommand::Mute).is_ok());
        assert!(worker.send(WorkerCommand::Mute).is_err());
        assert!(matches!(requests.try_recv().unwrap(), WorkerCommand::Mute));
        assert!(requests.try_recv().is_err());
        let (refresh, refreshes) = mpsc::sync_channel(1);
        let mut state = state(Language::En);
        state.camera_pending = true;
        assert!(!handle_action(Action::Refresh, &mut state, &[], &worker, &refresh).unwrap());
        assert!(!handle_action(Action::Refresh, &mut state, &[], &worker, &refresh).unwrap());
        assert!(refreshes.try_recv().is_ok());
        assert!(refreshes.try_recv().is_err());
        assert!(state.operation_error.is_none());
    }

    const LANGUAGES: [Language; 7] = [
        Language::En,
        Language::Fr,
        Language::De,
        Language::Es,
        Language::Ja,
        Language::Zh,
        Language::Ru,
    ];

    #[test]
    fn terminal_sizes_keep_quit_and_refresh_inside_disjoint_visible_buttons() {
        for lang in LANGUAGES {
            for (width, height) in [
                (1, 2),
                (3, 6),
                (8, 8),
                (20, 12),
                (40, 24),
                (60, 24),
                (80, 24),
                (120, 40),
                (150, 40),
            ] {
                let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                let mut state = state(lang);
                terminal
                    .draw(|frame| draw_ui(frame, &mut state, &[]))
                    .unwrap();
                let buffer = terminal.backend().buffer();
                for action in [Action::Quit, Action::Refresh] {
                    if action == Action::Refresh && width < 3 {
                        continue;
                    }
                    assert!(
                        state
                            .buttons
                            .iter()
                            .any(|(_, candidate)| *candidate == action),
                        "{lang:?} {width}x{height}: {action:?}"
                    );
                }
                let button_y = state.buttons.first().unwrap().0.y;
                assert!(
                    state.buttons.iter().all(|(rect, _)| rect.y == button_y),
                    "{lang:?} {width}x{height}: actions must stay on one row"
                );
                if width >= 20 {
                    assert!(
                        ACTIONS.iter().all(|action| state
                            .buttons
                            .iter()
                            .any(|(_, visible)| visible == action)),
                        "{lang:?} {width}x{height}: all six actions must remain visible"
                    );
                }
                for (rect, action) in &state.buttons {
                    assert!(rect.width > 0 && rect.height > 0);
                    assert!(rect.right() <= width && rect.bottom() <= height);
                    let visible: String = (rect.y..rect.bottom())
                        .flat_map(|y| (rect.x..rect.right()).map(move |x| (x, y)))
                        .map(|position| buffer[position].symbol())
                        .collect();
                    assert!(visible.contains(&button_label(*action, lang)[1..2]));
                    for y in rect.y..rect.bottom() {
                        for x in rect.x..rect.right() {
                            assert_eq!(
                                state
                                    .buttons
                                    .iter()
                                    .filter(|(other, _)| contains(*other, x, y))
                                    .count(),
                                1
                            );
                            assert_eq!(
                                mouse_action(&state, x, y),
                                if action_disabled(&state, *action) {
                                    None
                                } else {
                                    Some(*action)
                                }
                            );
                        }
                    }
                }
                for y in 0..height {
                    for x in 0..width {
                        if !state.buttons.iter().any(|(rect, _)| contains(*rect, x, y)) {
                            assert_eq!(mouse_action(&state, x, y), None);
                        }
                    }
                }
                assert_eq!(state.selected_access, 0);
                assert!(action_disabled(&state, Action::Kill));
            }
        }
    }
}
