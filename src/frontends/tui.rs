#[cfg(any(windows, target_os = "macos"))]
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
#[cfg(target_os = "linux")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CameraPrivacyState {
    Allowed,
    Blocked,
}
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
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState, Wrap},
};
use std::{
    io,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, Instant},
};

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
        let _ = lang;
        ""
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

#[cfg(target_os = "linux")]
fn camera_unavailable(lang: Language) -> &'static str {
    text(
        lang,
        [
            "Camera control unavailable on Linux",
            "Contrôle caméra indisponible sous Linux",
            "Kamerasteuerung unter Linux nicht verfügbar",
            "Control de cámara no disponible en Linux",
            "Linux のカメラ制御は利用できません",
            "Linux 摄像头控制不可用",
            "Управление камерой в Linux недоступно",
        ],
    )
}

fn microphone_label(lang: Language, state: MicrophoneMuteState) -> String {
    #[cfg(windows)]
    return lang.microphone_status(state).to_owned();
    #[cfg(unix)]
    format!(
        "{} ({})",
        lang.microphone_status(state),
        microphone_scope(lang)
    )
}

fn camera_label(lang: Language, state: CameraPrivacyState) -> &'static str {
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
    #[cfg(not(target_os = "macos"))]
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
        #[cfg(windows)]
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
    Camera(Option<CameraPrivacyState>),
    Mute,
    Kill { pid: u32, instance: String },
}

enum WorkerResponse {
    Camera(Result<CameraPrivacyState>),
    Mute(Result<MicrophoneMuteState>),
    Kill { pid: u32, result: Result<()> },
}

// Closing the sender drains the one outstanding action before joining. Even an
// I/O error cannot abandon a privileged mutation and imply it was cancelled.
struct ActionWorker {
    commands: Option<Sender<WorkerCommand>>,
    responses: Receiver<WorkerResponse>,
    thread: Option<thread::JoinHandle<()>>,
}

impl ActionWorker {
    fn new(policy: Policy) -> Self {
        let (commands, requests) = mpsc::channel();
        let (responses, results) = mpsc::channel();
        let thread = thread::spawn(move || {
            // Construct and destroy the COM monitor on this thread only.
            let mut monitor = None;
            while let Ok(command) = requests.recv() {
                let response = match command {
                    WorkerCommand::Camera(desired) => {
                        #[cfg(any(windows, target_os = "macos"))]
                        let result = desired.map_or_else(privacy::camera_state, |desired| {
                            privacy::set_camera_state(desired)?;
                            privacy::camera_state()
                        });
                        #[cfg(target_os = "linux")]
                        let result = {
                            let _ = desired;
                            Err(anyhow::anyhow!("Linux camera control is unavailable"))
                        };
                        WorkerResponse::Camera(result)
                    }
                    WorkerCommand::Mute => WorkerResponse::Mute((|| {
                        if monitor.is_none() {
                            monitor = Some(PlatformMonitor::new(policy.clone())?);
                        }
                        let monitor = monitor.as_ref().unwrap();
                        monitor.toggle_microphone_mute()?;
                        monitor.microphone_mute_state()
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
            .send(command)
            .context("action worker stopped")
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
    mute: Option<(Instant, Result<MicrophoneMuteState>)>,
}

fn observe(policy: Policy) -> (Sender<()>, Receiver<Observation>) {
    let (refresh, requests) = mpsc::channel();
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
            let mute = Some((read_started, monitor.microphone_mute_state()));
            let observation = Observation {
                snapshot,
                devices,
                mute,
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
    table_state: TableState,
    events_log: Vec<(String, String, Color)>,
    status_msg: Option<(String, Instant, Color)>,
    mute_state: MicrophoneMuteState,
    mute_changed_at: Option<Instant>,
    devices: Vec<Device>,
    health_error: Option<String>,
    camera_state: Option<CameraPrivacyState>,
    camera_error: Option<String>,
    camera_pending: bool,
    action_pending: Option<Action>,
    controls_error: Option<String>,
    buttons: Vec<(Rect, Action)>,
    pending_kill: Option<(String, u32, String, Instant)>,
}

impl TuiState {
    fn message(&mut self, message: String, color: Color) {
        self.status_msg = Some((message, Instant::now(), color));
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
    refresh: &Sender<()>,
) -> Result<bool> {
    let lang = state.lang;
    #[cfg(target_os = "linux")]
    if matches!(action, Action::BlockCamera | Action::RestoreCamera) {
        state.message(camera_unavailable(lang).to_owned(), Color::Yellow);
        return Ok(false);
    }
    if action != Action::Kill {
        state.pending_kill = None;
    }
    if action == Action::Refresh {
        match refresh.send(()) {
            Ok(()) => state.message(
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
        #[cfg(any(windows, target_os = "macos"))]
        if state.action_pending.is_none() && !state.camera_pending {
            match worker.send(WorkerCommand::Camera(None)) {
                Ok(()) => state.camera_pending = true,
                Err(error) => state.message(format!("{error:#}"), Color::Red),
            }
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
    if action == Action::Mute && state.mute_state == MicrophoneMuteState::Unavailable {
        state.message(microphone_label(lang, state.mute_state), Color::Yellow);
        return Ok(false);
    }
    let command = match action {
        Action::BlockCamera | Action::RestoreCamera => {
            let desired = if action == Action::BlockCamera {
                CameraPrivacyState::Blocked
            } else {
                CameraPrivacyState::Allowed
            };
            if state.camera_state == Some(desired) && state.camera_error.is_none() {
                state.message(camera_label(lang, desired).to_owned(), Color::Yellow);
                return Ok(false);
            }
            state.camera_error = None;
            WorkerCommand::Camera(Some(desired))
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
    #[cfg(any(windows, target_os = "macos"))]
    worker.send(WorkerCommand::Camera(None))?;
    let (refresh, observations) = observe(policy);
    let mut state = TuiState {
        lang,
        selected_access: 0,
        table_state: TableState::default(),
        events_log: Vec::new(),
        status_msg: None,
        mute_state: MicrophoneMuteState::Unavailable,
        mute_changed_at: None,
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
        camera_state: None,
        camera_error: {
            #[cfg(target_os = "linux")]
            {
                Some(camera_unavailable(lang).to_owned())
            }
            #[cfg(not(target_os = "linux"))]
            {
                None
            }
        },
        camera_pending: !cfg!(target_os = "linux"),
        action_pending: None,
        controls_error: None,
        buttons: Vec::new(),
    };
    let mut current_accesses: Vec<Access> = Vec::new();
    #[cfg(unix)]
    let mut tracker = crate::watcher::TransitionTracker::default();
    loop {
        match worker.responses.try_recv() {
            Ok(response) => {
                state.action_pending = None;
                match response {
                    WorkerResponse::Camera(result) => {
                        state.camera_pending = false;
                        match result {
                            Ok(camera) => {
                                state.camera_state = Some(camera);
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
                                        camera_label(lang, camera)
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
                            state.mute_state = actual;
                            state.mute_changed_at = Some(Instant::now());
                            let color = if matches!(
                                actual,
                                MicrophoneMuteState::Mixed | MicrophoneMuteState::Unavailable
                            ) {
                                Color::Yellow
                            } else {
                                Color::Green
                            };
                            state.message(microphone_label(lang, actual), color);
                            let _ = refresh.send(());
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
                                Color::Red,
                            );
                            let _ = refresh.send(());
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
                    Ok(mute) => state.mute_state = mute,
                    Err(error) => {
                        state.mute_state = MicrophoneMuteState::Unavailable;
                        state.controls_error = Some(format!("{error:#}"));
                    }
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
                    state
                        .buttons
                        .iter()
                        .find(|(rect, _)| contains(*rect, mouse.column, mouse.row))
                        .map(|(_, action)| *action)
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
    let footer_height = control_rows(f.area().width, state.lang) + 5;
    let body_height = f.area().height.saturating_sub(footer_height);
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
                    " [MIC MUTED] ",
                    " [MIC COUPÉ] ",
                    " [MIK STUMM] ",
                    " [MIC SILENCIO] ",
                    " [マイク消音] ",
                    " [麦克风静音] ",
                    " [МИК ВЫКЛ] ",
                ],
            ),
            Style::default().bg(Color::Red).fg(Color::White).bold(),
        ),
        MicrophoneMuteState::Unmuted => Span::styled(
            text(
                state.lang,
                [
                    " [MIC ON] ",
                    " [MIC ACTIF] ",
                    " [MIK AN] ",
                    " [MIC ACTIVO] ",
                    " [マイク有効] ",
                    " [麦克风开启] ",
                    " [МИК ВКЛ] ",
                ],
            ),
            Style::default().bg(Color::Green).fg(Color::Black).bold(),
        ),
        MicrophoneMuteState::Mixed => Span::styled(
            text(
                state.lang,
                [
                    " [MIC MIXED] ",
                    " [MIC MIXTE] ",
                    " [MIK GEMISCHT] ",
                    " [MIC MIXTO] ",
                    " [マイク混在] ",
                    " [麦克风混合] ",
                    " [МИК СМЕШАН] ",
                ],
            ),
            Style::default().bg(Color::Yellow).fg(Color::Black).bold(),
        ),
        MicrophoneMuteState::Unavailable => Span::styled(
            text(
                state.lang,
                [
                    " [NO MIC] ",
                    " [MIC INDISPO] ",
                    " [MIK N/V] ",
                    " [MIC NO DISP] ",
                    " [マイク不明] ",
                    " [麦克风不可用] ",
                    " [МИК Н/Д] ",
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
                            "No active capture detected. Microphone and camera are idle.",
                            "Aucune capture active. Le microphone et la caméra sont inactifs.",
                            "Keine aktive Aufnahme. Mikrofon und Kamera sind inaktiv.",
                            "Sin captura activa. Micrófono y cámara inactivos.",
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
                "[b] Block camera",
                "[b] Bloquer caméra",
                "[b] Kamera sperren",
                "[b] Bloquear cámara",
                "[b] カメラをブロック",
                "[b] 禁用摄像头",
                "[b] Блокировать камеру",
            ],
        ),
        Action::RestoreCamera => text(
            lang,
            [
                "[a] Restore camera",
                "[a] Rétablir caméra",
                "[a] Kamera freigeben",
                "[a] Restaurar cámara",
                "[a] カメラを復元",
                "[a] 恢复摄像头",
                "[a] Восстановить камеру",
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

fn control_columns(width: u16, lang: Language) -> u16 {
    let max_width = ACTIONS
        .iter()
        .map(|action| Span::raw(button_label(*action, lang)).width())
        .max()
        .unwrap_or(0);
    if usize::from(width / 2) >= max_width + 2 {
        2
    } else {
        1
    }
}

fn control_rows(width: u16, lang: Language) -> u16 {
    6 / control_columns(width, lang)
}

fn button_rects(area: Rect, lang: Language) -> impl Iterator<Item = (Rect, Action)> {
    let columns = control_columns(area.width, lang);
    ACTIONS
        .into_iter()
        .enumerate()
        .filter_map(move |(index, action)| {
            let row = index as u16 / columns;
            if row >= area.height {
                return None;
            }
            let column = index as u16 % columns;
            let width = area.width / columns;
            Some((
                Rect::new(area.x + column * width, area.y + row, width, 1),
                action,
            ))
        })
}

fn action_disabled(state: &TuiState, action: Action) -> bool {
    if cfg!(target_os = "linux") && matches!(action, Action::BlockCamera | Action::RestoreCamera) {
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
    let rows = control_rows(area.width, state.lang).min(area.height);
    state.buttons.clear();
    state.buttons.extend(button_rects(
        Rect::new(area.x, area.y, area.width, rows),
        state.lang,
    ));
    for (rect, action) in &state.buttons {
        let disabled = action_disabled(state, *action);
        let color = match action {
            Action::BlockCamera | Action::Kill => Color::Red,
            Action::RestoreCamera => Color::Green,
            _ => Color::Cyan,
        };
        let style = if disabled {
            Style::default().bg(Color::DarkGray).fg(Color::Gray)
        } else {
            Style::default().bg(color).fg(Color::Black).bold()
        };
        f.render_widget(
            Paragraph::new(button_label(*action, state.lang)).style(style),
            *rect,
        );
    }
    let feedback = Rect::new(
        area.x,
        area.y + rows,
        area.width,
        area.height.saturating_sub(rows),
    );
    let (message, color) = if let Some((message, created, color)) = &state.status_msg
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
    f.render_widget(
        Paragraph::new(message)
            .style(Style::default().fg(color))
            .wrap(Wrap { trim: false })
            .block(Block::default().borders(Borders::ALL).title(text(
                state.lang,
                [
                    " Actions / ↑↓ select ",
                    " Actions / ↑↓ choisir ",
                    " Aktionen / ↑↓ wählen ",
                    " Acciones / ↑↓ elegir ",
                    " 操作 / ↑↓選択 ",
                    " 操作 / ↑↓选择 ",
                    " Действия / ↑↓ выбор ",
                ],
            ))),
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
            table_state: TableState::default(),
            events_log: Vec::new(),
            status_msg: None,
            mute_state: MicrophoneMuteState::Unavailable,
            mute_changed_at: None,
            devices: Vec::new(),
            health_error: None,
            camera_state: None,
            camera_error: None,
            camera_pending: false,
            action_pending: None,
            controls_error: None,
            buttons: Vec::new(),
            pending_kill: None,
        }
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
        let (commands, _requests) = mpsc::channel();
        let (_responses, responses) = mpsc::channel();
        let worker = ActionWorker {
            commands: Some(commands),
            responses,
            thread: None,
        };
        let (refresh, _refreshes) = mpsc::channel();
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
    fn localized_buttons_fit_and_have_disjoint_click_targets() {
        for lang in [
            Language::En,
            Language::Fr,
            Language::De,
            Language::Es,
            Language::Ja,
            Language::Zh,
            Language::Ru,
        ] {
            for width in [40, 60, 80, 120] {
                let area = Rect::new(2, 3, width, control_rows(width, lang));
                let buttons: Vec<_> = button_rects(area, lang).collect();
                for (rect, action) in &buttons {
                    assert!(
                        Span::raw(button_label(*action, lang)).width() <= usize::from(rect.width)
                    );
                    assert!(contains(area, rect.x, rect.y));
                    assert_eq!(
                        buttons
                            .iter()
                            .filter(|(other, _)| contains(*other, rect.x, rect.y))
                            .count(),
                        1
                    );
                    assert!(!contains(*rect, rect.right(), rect.y));
                }
                assert_eq!(
                    buttons
                        .iter()
                        .map(|(_, action)| *action)
                        .collect::<Vec<_>>(),
                    ACTIONS
                );
            }
        }
    }

    #[test]
    fn narrow_terminal_renders_camera_actions_without_clipping() {
        for width in [40, 80] {
            let mut terminal = Terminal::new(TestBackend::new(width, 24)).unwrap();
            let mut state = state(Language::En);
            terminal
                .draw(|frame| draw_ui(frame, &mut state, &[]))
                .unwrap();
            let buffer = terminal.backend().buffer();
            for action in [Action::BlockCamera, Action::RestoreCamera] {
                let (rect, _) = state
                    .buttons
                    .iter()
                    .find(|(_, candidate)| *candidate == action)
                    .unwrap();
                let rendered: String = (rect.x..rect.right())
                    .map(|x| buffer[(x, rect.y)].symbol())
                    .collect();
                assert!(rendered.contains(button_label(action, Language::En)));
            }
        }
    }
}
