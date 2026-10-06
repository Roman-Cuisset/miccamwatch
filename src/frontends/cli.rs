use crate::i18n::Language;
use crate::model::Risk;
use clap::{Args, CommandFactory, FromArgMatches, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "mcw",
    version,
    about = "See which applications are using your microphone or camera"
)]
pub struct Cli {
    /// Policy file in TOML format
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,
    /// User interface language (en, fr, de, es, ja, zh, ru)
    #[arg(long, global = true, value_parser = parse_language)]
    pub lang: Option<Language>,
    #[command(subcommand)]
    pub command: Command,
}

impl Cli {
    pub fn parse_localized() -> Self {
        let arguments: Vec<_> = std::env::args_os().collect();
        let before_separator = arguments
            .iter()
            .take_while(|argument| argument.as_os_str() != "--");
        let wants_help = before_separator
            .clone()
            .any(|argument| matches!(argument.to_str(), Some("-h" | "--help" | "help")));
        let command = if wants_help {
            let mut explicit = None;
            let mut arguments = before_separator.skip(1);
            while let Some(argument) = arguments.next() {
                if argument == "--lang" {
                    explicit = arguments
                        .next()
                        .and_then(|value| value.to_str())
                        .and_then(Language::from_code);
                } else if let Some(value) = argument
                    .to_str()
                    .and_then(|value| value.strip_prefix("--lang="))
                {
                    explicit = Language::from_code(value);
                }
            }
            let mut command = Self::command();
            // Materialize Clap's own help command/flags before translating them.
            command.build();
            localized_help(command, explicit.unwrap_or_else(Language::detect), false)
        } else {
            Self::command()
        };
        let matches = command.get_matches_from(arguments);
        Self::from_arg_matches(&matches).unwrap_or_else(|error| error.exit())
    }
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Show capture access active right now
    Status(Options),
    /// Print access start and stop events until Ctrl+C
    Watch(WatchOptions),
    /// List microphone and camera devices
    Devices(OutputOptions),
    /// Explain all evidence currently associated with one process
    Explain(ExplainOptions),
    /// Download and install the latest GitHub release
    Update,
    /// Run self-diagnostics and report system compatibility
    Doctor(OutputOptions),
    /// Mute/query Windows capture, Linux PipeWire session sources, or writable macOS inputs
    Mute(MuteOptions),
    /// Restore microphone capture within the supported platform scope
    Unmute,
    /// Launch the interactive full-terminal live dashboard
    Top,
    /// Control the background notification-area service
    Tray {
        #[command(subcommand)]
        command: Option<TrayCommand>,
        /// Also deliver tray access events to the native system journal (Unix)
        #[cfg(unix)]
        #[arg(long, global = true)]
        eventlog: bool,
    },
    /// Windows camera switch, authorized Linux USB controls, or approved macOS profile
    Camera {
        #[command(subcommand)]
        command: CameraCommand,
    },
    /// Manage the per-user native autostart registration
    Autostart {
        #[command(subcommand)]
        command: AutostartCommand,
    },
    /// Select a privacy profile
    Profile {
        #[arg(value_enum)]
        profile: Option<ProfileArg>,
    },
    /// Pause, resume, or inspect desktop notifications
    Notifications {
        #[command(subcommand)]
        command: NotificationCommand,
    },
    /// Configure supported session-lock privacy actions (macOS lock signal unavailable)
    LockPolicy {
        #[command(subcommand)]
        command: LockPolicyCommand,
    },
    /// Inspect or clear the rotating local activity history
    History {
        #[command(subcommand)]
        command: HistoryCommand,
    },
    /// Validate policy configuration
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum TrayCommand {
    /// Run the notification-area service in this process
    Run,
    /// Stop the running notification-area service
    Stop,
    /// Report whether the notification-area service is running
    Status,
}

#[derive(Debug, Subcommand)]
pub enum CameraCommand {
    Status,
    Allow,
    Block,
    Toggle,
}

#[derive(Debug, Subcommand)]
pub enum AutostartCommand {
    Status,
    Enable,
    Disable,
    #[cfg(unix)]
    #[command(hide = true)]
    Refresh,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ProfileArg {
    Private,
    Meeting,
    Development,
    Balanced,
}

#[derive(Debug, Subcommand)]
pub enum NotificationCommand {
    Status,
    /// Pause notifications for a bounded number of minutes
    Pause {
        #[arg(value_parser = clap::value_parser!(u64).range(1..=10080))]
        minutes: u64,
    },
    Resume,
}

#[derive(Debug, Subcommand)]
pub enum LockPolicyCommand {
    Status,
    Enable {
        #[arg(long)]
        microphone: bool,
        #[arg(long)]
        camera: bool,
    },
    Disable,
}

#[derive(Debug, Subcommand)]
pub enum HistoryCommand {
    Path,
    Clear,
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Parse and validate the selected policy file
    Validate,
    /// Print the default user policy path
    Path,
    /// Print the persistent application settings path
    SettingsPath,
}
#[derive(Args, Debug)]
pub struct Options {
    #[command(flatten)]
    pub filter: Filter,
    #[command(flatten)]
    pub output: OutputOptions,
}

#[derive(Args, Debug)]
pub struct WatchOptions {
    #[command(flatten)]
    pub filter: Filter,
    #[command(flatten)]
    pub output: OutputOptions,
    /// Polling interval in milliseconds
    #[arg(long, default_value_t = 750, value_parser = clap::value_parser!(u64).range(100..))]
    pub interval: u64,
    /// Send native desktop notifications on access events
    #[arg(long)]
    pub notify: bool,
    /// Append JSONL events to a log file
    #[arg(long)]
    pub log: Option<PathBuf>,
    /// Also write events to Windows Application log or the native Unix system journal
    #[arg(long)]
    pub eventlog: bool,
    /// Play a discreet chime when microphone or camera access starts
    #[arg(long)]
    pub sound: bool,
    /// Automatically terminate suspicious unauthorized processes accessing camera or microphone (disabled by default)
    #[arg(long, conflicts_with = "no_kill")]
    pub kill_unauthorized: bool,
    /// Disable automatic process termination, overriding policy configuration
    #[arg(long, conflicts_with = "kill_unauthorized")]
    pub no_kill: bool,
}

#[derive(Args, Debug)]
pub struct ExplainOptions {
    /// Process identifier to inspect
    pub pid: u32,
    #[command(flatten)]
    pub output: OutputOptions,
}

#[derive(Args, Debug, Default)]
pub struct MuteOptions {
    /// Toggle mute state (mute if unmuted, unmute if muted)
    #[arg(long, short = 't')]
    pub toggle: bool,
    /// Only check whether microphone is currently muted
    #[arg(long, short = 's')]
    pub status: bool,
}

#[derive(Args, Debug, Default)]
pub struct Filter {
    /// Only report microphone access
    #[arg(long, conflicts_with = "camera")]
    pub microphone: bool,
    /// Only report camera access
    #[arg(long, conflicts_with = "microphone")]
    pub camera: bool,
    /// Minimum risk level to display (expected, unexplained, suspicious, blocked)
    #[arg(long, value_parser = parse_risk)]
    pub risk: Option<Risk>,
    /// Include low-confidence camera-ready pipelines without confirmed frame flow
    #[arg(long)]
    pub include_ready: bool,
}

impl Filter {
    pub fn includes_microphone(&self) -> bool {
        self.microphone || !self.camera
    }

    pub fn includes_camera(&self) -> bool {
        self.camera || !self.microphone
    }
}
impl From<&Filter> for crate::collector::CaptureScope {
    fn from(filter: &Filter) -> Self {
        Self {
            microphone: filter.includes_microphone(),
            camera: filter.includes_camera(),
            include_ready: filter.include_ready,
        }
    }
}

#[derive(Args, Debug, Default)]
pub struct OutputOptions {
    /// Emit newline-delimited JSON
    #[arg(long)]
    pub json: bool,
    /// Disable colored terminal output
    #[arg(long)]
    pub no_color: bool,
}

fn parse_risk(s: &str) -> Result<Risk, String> {
    match s.to_ascii_lowercase().as_str() {
        "expected" => Ok(Risk::Expected),
        "unexplained" => Ok(Risk::Unexplained),
        "suspicious" => Ok(Risk::Suspicious),
        "blocked" => Ok(Risk::Blocked),
        _ => Err(format!(
            "unknown risk level '{s}'; expected one of: expected, unexplained, suspicious, blocked"
        )),
    }
}

fn parse_language(s: &str) -> Result<Language, String> {
    Language::from_code(s).ok_or_else(|| {
        format!("unknown language '{s}'; supported languages: en, fr, de, es, ja, zh, ru")
    })
}

fn help_text(lang: Language, values: [&'static str; 7]) -> &'static str {
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

fn localized_help(mut command: clap::Command, lang: Language, root_parent: bool) -> clap::Command {
    let root = command.get_name() == "mcw";
    if let Some(description) = command_help(command.get_name(), root_parent, lang) {
        command = command.about(description);
    }
    command = command
        .subcommand_help_heading(help_text(
            lang,
            [
                "Commands",
                "Commandes",
                "Befehle",
                "Comandos",
                "コマンド",
                "命令",
                "Команды",
            ],
        ))
        .help_template(help_text(
            lang,
            [
                "{about-with-newline}\nUsage: {usage}\n\n{all-args}{after-help}",
                "{about-with-newline}\nUtilisation : {usage}\n\n{all-args}{after-help}",
                "{about-with-newline}\nAufruf: {usage}\n\n{all-args}{after-help}",
                "{about-with-newline}\nUso: {usage}\n\n{all-args}{after-help}",
                "{about-with-newline}\n使用方法: {usage}\n\n{all-args}{after-help}",
                "{about-with-newline}\n用法: {usage}\n\n{all-args}{after-help}",
                "{about-with-newline}\nИспользование: {usage}\n\n{all-args}{after-help}",
            ],
        ))
        .after_help(platform_help(lang))
        .mut_args(|argument| {
            let heading = if argument.is_positional() {
                help_text(
                    lang,
                    [
                        "Arguments",
                        "Arguments",
                        "Argumente",
                        "Argumentos",
                        "引数",
                        "参数",
                        "Аргументы",
                    ],
                )
            } else {
                help_text(
                    lang,
                    [
                        "Options",
                        "Options",
                        "Optionen",
                        "Opciones",
                        "オプション",
                        "选项",
                        "Параметры",
                    ],
                )
            };
            let description = argument_help(argument.get_id().as_str(), lang);
            let argument = argument.help_heading(heading);
            match description {
                Some(description) => argument.help(description),
                None => argument,
            }
        });
    command.mut_subcommands(|child| localized_help(child, lang, root))
}

fn platform_help(lang: Language) -> &'static str {
    #[cfg(windows)]
    return help_text(
        lang,
        [
            "Scope: capture-endpoint software mute; camera device controls need explicit administrator approval. Monitoring receives no media.",
            "Portée : mute logiciel des entrées ; contrôle caméra avec approbation administrateur explicite. Aucun média reçu par le moniteur.",
            "Umfang: Software-Stummschaltung der Eingänge; Kamerasteuerung benötigt ausdrückliche Administratorfreigabe. Keine Medienaufnahme durch den Monitor.",
            "Alcance: silencio por software de entradas; control de cámara con aprobación explícita de administrador. El monitor no recibe medios.",
            "範囲: 入力のソフトウェアミュート。カメラ制御には明示的な管理者承認が必要です。監視はメディアを受信しません。",
            "范围：输入端点软件静音；摄像头控制需要明确的管理员批准。监控器不接收媒体。",
            "Область: программное отключение звука входов; управление камерой требует явного согласия администратора. Монитор не получает медиаданные.",
        ],
    );
    #[cfg(target_os = "linux")]
    return help_text(
        lang,
        [
            "Scope: PipeWire session-source mute and owned restoration, not global ALSA denial. USB uvcvideo cameras need a matching root-installed helper and explicit Polkit authorization; non-USB cameras and camera-on-lock are unsupported. Status never prompts. Tray needs a real StatusNotifier host; microphone lock actions need a local graphical logind session.",
            "Portée : mute/restauration des sources de session PipeWire, pas de blocage global ALSA. Caméras USB uvcvideo : helper correspondant installé par root et autorisation Polkit explicite ; non-USB et caméra au verrouillage non pris en charge. Le statut ne demande jamais d'autorisation. Tray : hôte StatusNotifier réel ; verrouillage micro : session graphique logind locale.",
            "Umfang: PipeWire-Sitzungsquellen und eigene Wiederherstellung, keine globale ALSA-Sperre. USB-uvcvideo-Kameras benötigen einen passenden root-installierten Helper und ausdrückliche Polkit-Freigabe; Nicht-USB und Kamera-beim-Sperren nicht unterstützt. Status fragt nie nach Freigabe. Tray benötigt StatusNotifier; Mikrofon-Sperraktionen eine lokale grafische logind-Sitzung.",
            "Alcance: fuentes de sesión PipeWire y restauración propia, no bloqueo global ALSA. Cámaras USB uvcvideo: helper coincidente instalado por root y autorización Polkit explícita; no USB y cámara al bloquear no admitidos. El estado nunca solicita autorización. Bandeja: StatusNotifier real; bloqueo de micrófono: sesión gráfica local logind.",
            "範囲: PipeWire セッション入力のミュートと自分の変更の復元。ALSA 全体の禁止ではありません。USB uvcvideo カメラには同一バージョンの root インストール済みヘルパーと明示的な Polkit 承認が必要です。非 USB とロック時カメラ制御は未対応。状態確認は承認を要求しません。トレイには StatusNotifier、マイクのロック操作にはローカル logind GUI セッションが必要です。",
            "范围：PipeWire 会话输入静音及自有更改恢复，不是全局 ALSA 禁用。USB uvcvideo 摄像头需要 root 安装的同版本助手和明确的 Polkit 授权；不支持非 USB 或锁屏时摄像头控制。状态查询绝不请求授权。托盘需要真实 StatusNotifier 主机；锁屏麦克风操作需要本地图形 logind 会话。",
            "Область: источники сеанса PipeWire и восстановление своих изменений, не глобальный запрет ALSA. USB uvcvideo требует помощник той же версии, установленный root, и явное разрешение Polkit; не-USB и камера при блокировке не поддерживаются. Статус не запрашивает разрешение. Tray требует StatusNotifier; микрофон при блокировке — локальный графический сеанс logind.",
        ],
    );
    #[cfg(target_os = "macos")]
    return help_text(
        lang,
        [
            "Scope: writable CoreAudio INPUT mute only. Camera restriction needs manual approval of MCW's own profile. Public lock signal unavailable (Unknown); automatic lock actions refused. Menu bar needs an Aqua session. Camera client PID remains unknown.",
            "Portée : mute des entrées CoreAudio modifiables uniquement. Restriction caméra : approbation manuelle du profil MCW. Signal public de verrouillage indisponible (Unknown), actions automatiques refusées. Menu bar : session Aqua. PID caméra inconnu.",
            "Umfang: nur schreibbare CoreAudio-Eingänge. Kameraprofil muss manuell genehmigt werden. Öffentlicher Sperrstatus nicht verfügbar (Unknown); automatische Sperraktionen abgelehnt. Menüleiste benötigt Aqua. Kamera-PID unbekannt.",
            "Alcance: solo entradas CoreAudio modificables. Restricción de cámara: aprobación manual del perfil propio. Señal pública de bloqueo no disponible (Unknown); acciones automáticas rechazadas. Barra de menú: sesión Aqua. PID de cámara desconocido.",
            "範囲: 書き込み可能な CoreAudio 入力のみ。カメラ制限には MCW のプロファイルの手動承認が必要です。公開ロック信号は不明 (Unknown) で自動操作は拒否されます。メニューバーには Aqua が必要です。カメラ PID は不明です。",
            "范围：仅可写 CoreAudio 输入静音。摄像头限制需要手动批准 MCW 自有配置。公开锁定信号不可用 (Unknown)，拒绝自动操作。菜单栏需要 Aqua 会话。摄像头 PID 未知。",
            "Область: только изменяемые входы CoreAudio. Ограничение камеры требует ручного одобрения собственного профиля MCW. Публичный сигнал блокировки недоступен (Unknown); автоматические действия отклоняются. Меню требует Aqua. PID камеры неизвестен.",
        ],
    );
}

fn command_help(name: &str, root: bool, lang: Language) -> Option<&'static str> {
    let values = match name {
        "mcw" => [
            "Observe microphone/camera access without recording media",
            "Observer les accès micro/caméra sans enregistrer de média",
            "Mikrofon-/Kamerazugriffe ohne Medienaufnahme beobachten",
            "Observar acceso al micrófono/cámara sin grabar medios",
            "メディアを記録せずにマイク・カメラのアクセスを監視",
            "监控麦克风和摄像头访问，不录制媒体",
            "Наблюдать доступ к микрофону/камере без записи медиаданных",
        ],
        "status" if root => [
            "Show current capture observations and coverage",
            "Afficher les observations de capture et leur couverture",
            "Aktuelle Aufnahmebeobachtungen und Abdeckung anzeigen",
            "Mostrar observaciones de captura y cobertura",
            "現在のキャプチャ観測と監視範囲を表示",
            "显示当前采集观测及覆盖范围",
            "Показать текущие наблюдения захвата и покрытие",
        ],
        "status" => [
            "Show effective state",
            "Afficher l'état effectif",
            "Tatsächlichen Zustand anzeigen",
            "Mostrar el estado efectivo",
            "実際の状態を表示",
            "显示实际状态",
            "Показать фактическое состояние",
        ],
        "watch" => [
            "Observe start/update/stop until Ctrl+C",
            "Observer START/UPDATE/STOP jusqu'à Ctrl+C",
            "START/UPDATE/STOP bis Ctrl+C beobachten",
            "Observar START/UPDATE/STOP hasta Ctrl+C",
            "Ctrl+C まで START/UPDATE/STOP を監視",
            "监控 START/UPDATE/STOP，按 Ctrl+C 退出",
            "Наблюдать START/UPDATE/STOP до Ctrl+C",
        ],
        "devices" => [
            "List microphone and camera devices",
            "Lister les micros et caméras",
            "Mikrofone und Kameras auflisten",
            "Listar micrófonos y cámaras",
            "マイクとカメラを一覧表示",
            "列出麦克风和摄像头",
            "Показать микрофоны и камеры",
        ],
        "explain" => [
            "Explain current evidence for a PID",
            "Expliquer les preuves actuelles d'un PID",
            "Aktuelle Belege für eine PID erklären",
            "Explicar las pruebas actuales de un PID",
            "PID の現在の証拠を説明",
            "解释指定 PID 的当前证据",
            "Объяснить текущие доказательства для PID",
        ],
        "doctor" => [
            "Diagnose native prerequisites and coverage limits",
            "Diagnostiquer les prérequis natifs et limites de couverture",
            "Native Voraussetzungen und Abdeckungsgrenzen prüfen",
            "Diagnosticar requisitos nativos y límites de cobertura",
            "ネイティブ要件と監視範囲の制限を診断",
            "诊断原生依赖及覆盖限制",
            "Проверить нативные требования и ограничения покрытия",
        ],
        "update" => [
            "Upgrade an owned installation with verified release files",
            "Mettre à niveau une installation gérée avec des fichiers vérifiés",
            "Eigene Installation mit geprüften Release-Dateien aktualisieren",
            "Actualizar una instalación gestionada con archivos verificados",
            "検証済みリリースで管理対象のインストールを更新",
            "使用已验证发行文件更新受管安装",
            "Обновить управляемую установку проверенными файлами выпуска",
        ],
        "mute" => [
            "Mute/query inputs within the platform scope below",
            "Muter/interroger les entrées dans la portée ci-dessous",
            "Eingänge im unten genannten Umfang stummschalten/abfragen",
            "Silenciar/consultar entradas dentro del alcance indicado",
            "下記の範囲で入力をミュート・確認",
            "在下述范围内静音或查询输入",
            "Отключить звук/проверить входы в указанной ниже области",
        ],
        "unmute" => [
            "Restore input state within the supported platform scope",
            "Restaurer les entrées dans la portée prise en charge",
            "Eingangszustand im unterstützten Umfang wiederherstellen",
            "Restaurar entradas dentro del alcance admitido",
            "対応範囲内で入力の状態を復元",
            "在支持范围内恢复输入状态",
            "Восстановить входы в поддерживаемой области",
        ],
        "top" => [
            "Open the live terminal dashboard",
            "Ouvrir le dashboard terminal en direct",
            "Live-Terminalübersicht öffnen",
            "Abrir el panel de terminal en vivo",
            "リアルタイム端末ダッシュボードを開く",
            "打开实时终端面板",
            "Открыть панель наблюдения в терминале",
        ],
        "tray" => [
            "Control the native desktop tray/menu bar",
            "Contrôler le tray/menu bar natif",
            "Nativen Tray/Menüleiste steuern",
            "Controlar la bandeja/barra de menú nativa",
            "ネイティブトレイ・メニューバーを操作",
            "控制原生托盘或菜单栏",
            "Управлять нативным tray/строкой меню",
        ],
        "camera" => [
            "Query/control camera privacy within the platform scope below",
            "Interroger/contrôler la caméra dans la portée ci-dessous",
            "Kamerazustand im unten genannten Umfang abfragen/steuern",
            "Consultar/controlar la cámara dentro del alcance indicado",
            "下記の範囲でカメラ状態を確認・操作",
            "在下述范围内查询或控制摄像头隐私",
            "Проверить/управлять камерой в указанной ниже области",
        ],
        "autostart" => [
            "Manage owned per-user native desktop autostart",
            "Gérer le démarrage natif du bureau pour cet utilisateur",
            "Eigenen nativen Desktop-Autostart verwalten",
            "Gestionar el inicio nativo del escritorio del usuario",
            "ユーザー単位のネイティブ自動起動を管理",
            "管理用户级原生桌面自启动",
            "Управлять собственным автозапуском рабочего стола пользователя",
        ],
        "profile" => [
            "Select a privacy profile",
            "Sélectionner un profil de confidentialité",
            "Datenschutzprofil wählen",
            "Seleccionar un perfil de privacidad",
            "プライバシープロファイルを選択",
            "选择隐私配置",
            "Выбрать профиль конфиденциальности",
        ],
        "notifications" => [
            "Inspect/pause/resume native desktop notifications",
            "Interroger/suspendre/reprendre les notifications natives",
            "Native Benachrichtigungen prüfen/pausieren/fortsetzen",
            "Consultar/pausar/reanudar notificaciones nativas",
            "通知の確認・一時停止・再開",
            "查询、暂停或恢复原生通知",
            "Проверить/приостановить/возобновить нативные уведомления",
        ],
        "lock-policy" => [
            "Configure opt-in lock actions; see platform limitations below",
            "Configurer les actions au verrouillage ; voir les limites ci-dessous",
            "Optionale Sperraktionen konfigurieren; Plattformgrenzen beachten",
            "Configurar acciones de bloqueo optativas; consulte límites",
            "任意のロック操作を設定。下記の制限を参照",
            "配置可选锁定操作；请查看下述平台限制",
            "Настроить действия при блокировке; ограничения указаны ниже",
        ],
        "history" => [
            "Inspect or clear local rotating activity history",
            "Interroger ou effacer l'historique local rotatif",
            "Lokalen rotierenden Verlauf prüfen oder löschen",
            "Consultar o borrar el historial local rotativo",
            "ローカルのローテーション履歴を確認・消去",
            "查询或清除本地轮换活动历史",
            "Проверить или очистить локальную историю активности",
        ],
        "config" => [
            "Validate or locate policy and preferences",
            "Valider ou localiser politique et préférences",
            "Richtlinie/Einstellungen prüfen oder finden",
            "Validar o localizar políticas y preferencias",
            "ポリシーと設定の検証・場所の確認",
            "验证或定位策略和偏好设置",
            "Проверить или найти политику и настройки",
        ],
        "run" => [
            "Run the native desktop service in this process",
            "Exécuter le service de bureau dans ce processus",
            "Desktop-Dienst in diesem Prozess ausführen",
            "Ejecutar el servicio de escritorio en este proceso",
            "このプロセスでデスクトップサービスを実行",
            "在本进程中运行桌面服务",
            "Запустить службу рабочего стола в этом процессе",
        ],
        "stop" => [
            "Stop the owned desktop service",
            "Arrêter le service de bureau géré",
            "Eigenen Desktop-Dienst stoppen",
            "Detener el servicio de escritorio gestionado",
            "管理対象のデスクトップサービスを停止",
            "停止受管桌面服务",
            "Остановить собственную службу рабочего стола",
        ],
        "enable" => [
            "Enable explicitly; never enabled implicitly",
            "Activer explicitement ; jamais implicitement",
            "Ausdrücklich aktivieren; nie automatisch",
            "Activar explícitamente; nunca implícitamente",
            "明示的に有効化。自動では有効にしません",
            "明确启用，不隐式启用",
            "Включить явно; никогда не включается неявно",
        ],
        "disable" => [
            "Disable the owned configuration",
            "Désactiver la configuration gérée",
            "Eigene Konfiguration deaktivieren",
            "Desactivar la configuración gestionada",
            "管理対象の設定を無効化",
            "禁用受管配置",
            "Отключить собственную настройку",
        ],
        "allow" => [
            "Restore only MCW-owned camera changes",
            "Restaurer seulement les changements caméra de MCW",
            "Nur eigene Kamer Änderungen wiederherstellen",
            "Restaurar solo los cambios de cámara de MCW",
            "MCW によるカメラ変更のみ復元",
            "仅恢复 MCW 自有摄像头更改",
            "Восстановить только изменения камеры, сделанные MCW",
        ],
        "block" => [
            "Request camera restriction within the supported scope",
            "Demander une restriction caméra dans la portée prise en charge",
            "Kamerabeschränkung im unterstützten Umfang anfordern",
            "Solicitar restricción de cámara dentro del alcance admitido",
            "対応範囲内のカメラ制限を要求",
            "请求支持范围内的摄像头限制",
            "Запросить ограничение камеры в поддерживаемой области",
        ],
        "toggle" => [
            "Toggle according to effective state",
            "Basculer selon l'état effectif",
            "Nach tatsächlichem Zustand umschalten",
            "Alternar según el estado efectivo",
            "実際の状態に応じて切り替え",
            "根据实际状态切换",
            "Переключить по фактическому состоянию",
        ],
        "pause" => [
            "Pause notifications for a bounded duration",
            "Suspendre les notifications pour une durée bornée",
            "Benachrichtigungen zeitlich begrenzt pausieren",
            "Pausar notificaciones durante un tiempo limitado",
            "通知を一定時間停止",
            "暂停通知一段有限时间",
            "Приостановить уведомления на ограниченное время",
        ],
        "resume" => [
            "Resume notifications",
            "Reprendre les notifications",
            "Benachrichtigungen fortsetzen",
            "Reanudar notificaciones",
            "通知を再開",
            "恢复通知",
            "Возобновить уведомления",
        ],
        "path" => [
            "Print the owned data/configuration path",
            "Afficher le chemin des données ou de la configuration",
            "Eigenen Daten-/Konfigurationspfad anzeigen",
            "Mostrar ruta de datos o configuración",
            "データ・設定のパスを表示",
            "显示数据或配置路径",
            "Показать путь данных или конфигурации",
        ],
        "settings-path" => [
            "Print the preferences path",
            "Afficher le chemin des préférences",
            "Einstellungspfad anzeigen",
            "Mostrar ruta de preferencias",
            "設定パスを表示",
            "显示偏好设置路径",
            "Показать путь настроек",
        ],
        "clear" => [
            "Clear local activity history",
            "Effacer l'historique local",
            "Lokalen Verlauf löschen",
            "Borrar historial local",
            "ローカル履歴を消去",
            "清除本地活动历史",
            "Очистить локальную историю",
        ],
        "validate" => [
            "Parse and validate the selected policy",
            "Lire et valider la politique sélectionnée",
            "Gewählte Richtlinie lesen und prüfen",
            "Leer y validar la política seleccionada",
            "選択したポリシーを解析・検証",
            "解析并验证所选策略",
            "Разобрать и проверить выбранную политику",
        ],
        "help" => [
            "Print help for a command",
            "Afficher l'aide d'une commande",
            "Hilfe zu einem Befehl anzeigen",
            "Mostrar ayuda de un comando",
            "コマンドのヘルプを表示",
            "显示命令帮助",
            "Показать справку по команде",
        ],
        "version" => [
            "Print version",
            "Afficher la version",
            "Version anzeigen",
            "Mostrar versión",
            "バージョンを表示",
            "显示版本",
            "Показать версию",
        ],
        _ => return None,
    };
    Some(help_text(lang, values))
}

fn argument_help(name: &str, lang: Language) -> Option<&'static str> {
    if matches!(name, "help" | "version") {
        return command_help(name, false, lang);
    }
    let values = match name {
        "config" => [
            "TOML policy file",
            "Fichier de politique TOML",
            "TOML-Richtliniendatei",
            "Archivo de política TOML",
            "TOML ポリシーファイル",
            "TOML 策略文件",
            "Файл политики TOML",
        ],
        "lang" => [
            "UI/help language: en fr de es ja zh ru",
            "Langue de l'interface/aide : en fr de es ja zh ru",
            "Sprache für Oberfläche/Hilfe: en fr de es ja zh ru",
            "Idioma de interfaz/ayuda: en fr de es ja zh ru",
            "表示・ヘルプ言語: en fr de es ja zh ru",
            "界面及帮助语言：en fr de es ja zh ru",
            "Язык интерфейса/справки: en fr de es ja zh ru",
        ],
        "microphone" => [
            "Microphone only",
            "Microphone uniquement",
            "Nur Mikrofon",
            "Solo micrófono",
            "マイクのみ",
            "仅麦克风",
            "Только микрофон",
        ],
        "camera" => [
            "Camera only",
            "Caméra uniquement",
            "Nur Kamera",
            "Solo cámara",
            "カメラのみ",
            "仅摄像头",
            "Только камера",
        ],
        "risk" => [
            "Minimum risk: expected unexplained suspicious blocked",
            "Risque minimal : expected unexplained suspicious blocked",
            "Mindestrisiko: expected unexplained suspicious blocked",
            "Riesgo mínimo: expected unexplained suspicious blocked",
            "最小リスク: expected unexplained suspicious blocked",
            "最低风险：expected unexplained suspicious blocked",
            "Минимальный риск: expected unexplained suspicious blocked",
        ],
        "include_ready" => [
            "Include Ready pipelines; frame flow remains unproven",
            "Inclure les pipelines Ready ; flux d'images non prouvé",
            "Ready-Pipelines einschließen; Bildfluss unbewiesen",
            "Incluir canales Ready; flujo de imágenes no demostrado",
            "Ready パイプラインを含む。フレームの流れは未証明",
            "包含 Ready 管线；尚未证明帧流",
            "Включить Ready; поток кадров не доказан",
        ],
        "json" => [
            "Emit versioned JSON/JSONL",
            "Produire du JSON/JSONL versionné",
            "Versioniertes JSON/JSONL ausgeben",
            "Emitir JSON/JSONL versionado",
            "バージョン付き JSON/JSONL を出力",
            "输出带版本的 JSON/JSONL",
            "Вывести версионированный JSON/JSONL",
        ],
        "no_color" => [
            "Disable terminal colors",
            "Désactiver les couleurs",
            "Terminalfarben deaktivieren",
            "Desactivar colores",
            "端末の色を無効化",
            "禁用终端颜色",
            "Отключить цвета терминала",
        ],
        "interval" => [
            "Polling interval in milliseconds (minimum 100)",
            "Intervalle en millisecondes (minimum 100)",
            "Abfrageintervall in Millisekunden (mindestens 100)",
            "Intervalo en milisegundos (mínimo 100)",
            "ポーリング間隔（ミリ秒、最小 100）",
            "轮询间隔（毫秒，最少 100）",
            "Интервал опроса в миллисекундах (минимум 100)",
        ],
        "notify" => [
            "Deliver native desktop notifications on access events",
            "Notifier les événements via le bureau natif",
            "Native Desktop-Benachrichtigungen bei Zugriffsereignissen",
            "Notificar eventos mediante el escritorio nativo",
            "アクセスイベントをネイティブ通知",
            "通过原生桌面通知访问事件",
            "Нативные уведомления о событиях доступа",
        ],
        "sound" => [
            "Play a native chime when capture starts",
            "Jouer un son natif au début d'une capture",
            "Nativen Ton bei Aufnahmestart abspielen",
            "Reproducir sonido nativo al iniciar captura",
            "キャプチャ開始時にネイティブ音を再生",
            "采集开始时播放原生提示音",
            "Нативный звук при начале захвата",
        ],
        "log" => [
            "Append event JSONL to this file",
            "Ajouter les événements JSONL à ce fichier",
            "JSONL-Ereignisse an diese Datei anhängen",
            "Añadir eventos JSONL a este archivo",
            "このファイルに JSONL イベントを追記",
            "将 JSONL 事件追加到该文件",
            "Добавлять события JSONL в этот файл",
        ],
        "eventlog" => [
            "Deliver events to the native system journal",
            "Envoyer les événements au journal système natif",
            "Ereignisse an das native Systemprotokoll senden",
            "Enviar eventos al registro nativo del sistema",
            "イベントをネイティブシステムログに送信",
            "将事件发送至原生系统日志",
            "Отправлять события в нативный системный журнал",
        ],
        "kill_unauthorized" => [
            "Terminate only explicitly denied Active processes after two validated observations (opt-in)",
            "Terminer uniquement les processus Active explicitement refusés après deux observations validées (opt-in)",
            "Nur ausdrücklich abgelehnte Active-Prozesse nach zwei geprüften Beobachtungen beenden (optional)",
            "Terminar solo procesos Active explícitamente denegados tras dos observaciones validadas (optativo)",
            "明示的に拒否された Active プロセスのみ、検証済みの 2 回の観測後に終了（任意）",
            "仅在两次已验证观测后终止被明确拒绝的 Active 进程（需主动启用）",
            "Завершать только явно запрещённые Active-процессы после двух проверенных наблюдений (опционально)",
        ],
        "no_kill" => [
            "Disable automatic termination, overriding policy",
            "Désactiver l'arrêt automatique malgré la politique",
            "Automatisches Beenden trotz Richtlinie deaktivieren",
            "Desactivar terminación automática, anulando la política",
            "ポリシーに優先して自動終了を無効化",
            "禁用自动终止，覆盖策略",
            "Отключить автоматическое завершение независимо от политики",
        ],
        "toggle" => [
            "Toggle supported input mute state",
            "Basculer le mute des entrées prises en charge",
            "Unterstützte Eingangsstummschaltung umschalten",
            "Alternar silencio de entradas compatibles",
            "対応入力のミュートを切り替え",
            "切换支持输入的静音状态",
            "Переключить звук поддерживаемых входов",
        ],
        "status" => [
            "Query effective mute state without changing it",
            "Interroger le mute effectif sans le modifier",
            "Tatsächlichen Stummzustand ohne Änderung abfragen",
            "Consultar silencio efectivo sin modificarlo",
            "変更せず実際のミュート状態を確認",
            "查询实际静音状态，不进行更改",
            "Проверить фактическое отключение звука без изменений",
        ],
        "pid" => [
            "Process ID to inspect",
            "PID à inspecter",
            "Zu prüfende Prozess-ID",
            "PID que consultar",
            "確認するプロセス ID",
            "要查询的进程 ID",
            "PID для проверки",
        ],
        "minutes" => [
            "Pause duration in minutes (1–10080)",
            "Durée de pause en minutes (1–10080)",
            "Pausendauer in Minuten (1–10080)",
            "Duración en minutos (1–10080)",
            "停止時間（分、1–10080）",
            "暂停分钟数（1–10080）",
            "Продолжительность паузы в минутах (1–10080)",
        ],
        "profile" => [
            "Privacy profile",
            "Profil de confidentialité",
            "Datenschutzprofil",
            "Perfil de privacidad",
            "プライバシープロファイル",
            "隐私配置",
            "Профиль конфиденциальности",
        ],
        _ => return None,
    };
    Some(help_text(lang, values))
}
