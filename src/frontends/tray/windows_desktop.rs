//! Retained Win32 presentation. Only owner messages can change authoritative state.
use super::*;
use std::cell::RefCell;
use windows::UI::ViewManagement::{UIColorType, UISettings};
use windows::Win32::Foundation::COLORREF;
use windows::Win32::Graphics::Gdi::{
    COLOR_BTNFACE, COLOR_GRAYTEXT, COLOR_HIGHLIGHT, COLOR_HIGHLIGHTTEXT, COLOR_WINDOWTEXT,
    CreateSolidBrush, DT_CALCRECT, DT_CENTER, DT_WORDBREAK, DrawFocusRect, DrawTextW, FillRect,
    FrameRect, GetSysColor, HBRUSH, HDC, RestoreDC, SaveDC, SetBkColor, SetBkMode, SetTextColor,
    TRANSPARENT,
};
use windows::Win32::UI::Controls::DRAWITEMSTRUCT;
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowTextW, HWND_BOTTOM, SW_HIDE, SW_RESTORE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SetWindowPos, WM_NCDESTROY, WS_VISIBLE,
};

const PAGE_TOP: i32 = 156;
const PAGE_VIEWPORT: i32 = 386;
const LOGO: i32 = 299;
const MIC_PROTECTION: i32 = 328;
const CAM_PROTECTION: i32 = 329;
const APPS_TITLE: i32 = 3500;
const FEEDBACK: i32 = 3501;
const CARD_MIC: i32 = 370;
const CARD_CAM: i32 = 371;
const LIMIT: i32 = 372;
const CONTROLS_TITLE: i32 = 373;
const PANEL_PROFILE: i32 = 380;
const PANEL_ALERTS: i32 = 381;
const PANEL_AUTOMATION: i32 = 382;
const PROFILE_TITLE: i32 = 383;
const ALERTS_TITLE: i32 = 384;
const AUTOMATION_TITLE: i32 = 385;

const HEADER: i32 = 300;
const TAB: i32 = 310;
const OBSERVED: i32 = 320;
const MICROPHONE: i32 = 321;
const CAMERA: i32 = 322;
const APPS: i32 = 323;
const MIC_ACTION: i32 = 325;
const CAM_ACTION: i32 = 326;
const CAM_RESUME: i32 = 327;
const PROFILE: i32 = 330;
pub(super) const NOTIFICATIONS: usize = 340;
pub(super) const SOUND: usize = 341;
pub(super) const HISTORY: usize = 342;
pub(super) const PAUSE: usize = 343;
pub(super) const AUTOSTART: usize = 344;
const POLICY: i32 = 345;
pub(super) const MUTE_LOCK: usize = 347;
pub(super) const CAMERA_LOCK: usize = 348;
pub(super) const RESTORE_UNLOCK: usize = 349;
const DIAGNOSTIC: i32 = 350;
const COPY: i32 = 351;
const ABOUT: i32 = 360;
const BM_GETCHECK: u32 = 0x00f0;
const BM_SETCHECK: u32 = 0x00f1;
const EM_GETSEL: u32 = 0x00b0;
const EM_SETSEL: u32 = 0x00b1;
const EM_GETFIRSTVISIBLELINE: u32 = 0x00ce;
const EM_LINESCROLL: u32 = 0x00b6;
const EM_SETLIMITTEXT: u32 = 0x00c5;
const WM_DPICHANGED: u32 = 0x02e0;
const WM_GETMINMAXINFO: u32 = 0x0024;
const WM_SETTINGCHANGE: u32 = 0x001a;
const WM_THEMECHANGED: u32 = 0x031a;
#[link(name = "user32")]
unsafe extern "system" {
    fn EnableWindow(hwnd: HWND, enabled: i32) -> i32;
    fn IsWindowVisible(hwnd: HWND) -> i32;
    fn IsDialogMessageW(hwnd: HWND, message: *const MSG) -> i32;
    fn GetAncestor(hwnd: HWND, flags: u32) -> HWND;
    fn SetFocus(hwnd: HWND) -> HWND;
    fn GetFocus() -> HWND;
    fn SetScrollInfo(hwnd: HWND, bar: i32, info: *const ScrollInfo, redraw: i32) -> i32;
    fn SystemParametersInfoW(
        action: u32,
        parameter: u32,
        value: *mut std::ffi::c_void,
        flags: u32,
    ) -> i32;
    fn GetWindowRect(hwnd: HWND, rect: *mut RECT) -> i32;
    fn ScreenToClient(hwnd: HWND, point: *mut POINT) -> i32;
    fn InvalidateRect(hwnd: HWND, rect: *const RECT, erase: i32) -> i32;
    fn GetWindowThreadProcessId(hwnd: HWND, pid: *mut u32) -> u32;
    fn GetKeyState(key: i32) -> i16;
    fn GetDlgCtrlID(hwnd: HWND) -> i32;
    fn GetParent(hwnd: HWND) -> HWND;
    fn GetNextDlgTabItem(hwnd: HWND, current: HWND, previous: i32) -> HWND;
}
#[link(name = "dwmapi")]
unsafe extern "system" {
    fn DwmSetWindowAttribute(hwnd: HWND, attribute: u32, value: *const i32, size: u32) -> i32;
}
#[link(name = "uxtheme")]
unsafe extern "system" {
    fn SetWindowTheme(hwnd: HWND, app: *const u16, id: *const u16) -> i32;
}
#[link(name = "runtimeobject")]
unsafe extern "system" {
    fn RoInitialize(kind: u32) -> i32;
    fn RoUninitialize();
}

// UISettings caches a WinRT activation factory. Releasing the apartment after
// each color query leaves that cache pointing into a torn-down apartment.
// Keep it and the settings object alive for the native UI thread's lifetime.
struct DesktopRuntime {
    initialized: bool,
    settings: Option<UISettings>,
}
impl DesktopRuntime {
    fn new() -> Self {
        let result = unsafe { RoInitialize(0) };
        // The monitor may already own an MTA. Match it and retain our own
        // initialization rather than relying on that caller's lifetime.
        let result = if result == windows::Win32::Foundation::RPC_E_CHANGED_MODE.0 {
            unsafe { RoInitialize(1) }
        } else {
            result
        };
        let initialized = result >= 0;
        Self {
            initialized,
            settings: if initialized {
                UISettings::new().ok()
            } else {
                None
            },
        }
    }
}
impl Drop for DesktopRuntime {
    fn drop(&mut self) {
        drop(self.settings.take());
        if self.initialized {
            unsafe { RoUninitialize() };
        }
    }
}
thread_local! {
    static DESKTOP_RUNTIME: DesktopRuntime = DesktopRuntime::new();
}
#[repr(C)]
struct ScrollInfo {
    size: u32,
    mask: u32,
    min: i32,
    max: i32,
    page: u32,
    pos: i32,
    track: i32,
}
#[repr(C)]
struct HighContrast {
    size: u32,
    flags: u32,
    scheme: *mut u16,
}

/// Current-frame projection, never a second history buffer.
#[derive(Default)]
pub(super) struct Overview {
    observed: Option<chrono::DateTime<chrono::Utc>>,
    activity: [String; 2],
    apps: String,
    coverage: String,
}

pub(super) fn words(lang: Language, values: [&'static str; 7]) -> &'static str {
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
fn unknown(lang: Language) -> &'static str {
    words(
        lang,
        [
            "Unknown / unavailable",
            "Inconnu / indisponible",
            "Unbekannt / nicht verfügbar",
            "Desconocido / no disponible",
            "不明・利用不可",
            "未知／不可用",
            "Неизвестно / недоступно",
        ],
    )
}
fn active(lang: Language) -> &'static str {
    words(
        lang,
        [
            "Observed active",
            "Activité observée",
            "Aktivität beobachtet",
            "Actividad observada",
            "使用中を観測",
            "观测到使用中",
            "Наблюдается активность",
        ],
    )
}
fn ready(lang: Language) -> &'static str {
    words(
        lang,
        [
            "Ready, capture unconfirmed",
            "Prêt, capture non confirmée",
            "Bereit, Aufnahme unbestätigt",
            "Listo, captura sin confirmar",
            "準備済み・収録未確認",
            "管线就绪，采集未确认",
            "Готово, захват не подтверждён",
        ],
    )
}
fn empty(lang: Language) -> &'static str {
    words(
        lang,
        [
            "No access observed in this scan (not proof of safety)",
            "Aucun accès observé (ne prouve pas la sécurité)",
            "Kein Zugriff beobachtet (kein Sicherheitsnachweis)",
            "Sin acceso observado (no demuestra seguridad)",
            "今回アクセス観測なし（安全の証明ではありません）",
            "本次未观测到访问（不代表安全）",
            "Доступ не замечен (не доказательство безопасности)",
        ],
    )
}
fn coverage_label(lang: Language) -> &'static str {
    words(
        lang,
        [
            "Observation coverage",
            "Couverture d’observation",
            "Beobachtungsabdeckung",
            "Cobertura de observación",
            "観測範囲",
            "观测覆盖",
            "Охват наблюдения",
        ],
    )
}
pub(super) fn open_label(lang: Language) -> &'static str {
    words(
        lang,
        [
            "Open MicCamWatch…",
            "Ouvrir MicCamWatch…",
            "MicCamWatch öffnen…",
            "Abrir MicCamWatch…",
            "MicCamWatch を開く…",
            "打开 MicCamWatch…",
            "Открыть MicCamWatch…",
        ],
    )
}
fn check_labels(lang: Language) -> [&'static str; 9] {
    [
        words(
            lang,
            [
                "Notifications",
                "Notifications",
                "Benachrichtigungen",
                "Notificaciones",
                "通知",
                "通知",
                "Уведомления",
            ],
        ),
        words(
            lang,
            [
                "Notification sound",
                "Son des notifications",
                "Benachrichtigungston",
                "Sonido de notificación",
                "通知音",
                "通知声音",
                "Звук уведомлений",
            ],
        ),
        words(
            lang,
            [
                "Local history",
                "Historique local",
                "Lokaler Verlauf",
                "Historial local",
                "ローカル履歴",
                "本地历史",
                "Локальная история",
            ],
        ),
        words(
            lang,
            [
                "Pause notifications for 1 hour",
                "Suspendre les notifications 1 h",
                "Benachrichtigungen 1 Stunde pausieren",
                "Pausar notificaciones 1 hora",
                "通知を1時間停止",
                "暂停通知1小时",
                "Приостановить уведомления на час",
            ],
        ),
        words(
            lang,
            [
                "Start at sign-in",
                "Démarrer à la connexion",
                "Bei Anmeldung starten",
                "Iniciar al acceder",
                "サインイン時に起動",
                "登录时启动",
                "Запуск при входе",
            ],
        ),
        words(
            lang,
            [
                "Mute microphone on lock",
                "Couper le micro au verrouillage",
                "Mikrofon beim Sperren stummschalten",
                "Silenciar micrófono al bloquear",
                "ロック時にマイクをミュート",
                "锁屏时静音麦克风",
                "Отключать микрофон при блокировке",
            ],
        ),
        words(
            lang,
            [
                "Block cameras on lock",
                "Bloquer les caméras au verrouillage",
                "Kameras beim Sperren blockieren",
                "Bloquear cámaras al bloquear",
                "ロック時にカメラをブロック",
                "锁屏时阻止摄像头",
                "Блокировать камеры при блокировке",
            ],
        ),
        words(
            lang,
            [
                "Restore on unlock",
                "Restaurer au déverrouillage",
                "Beim Entsperren wiederherstellen",
                "Restaurar al desbloquear",
                "ロック解除時に復元",
                "解锁时恢复",
                "Восстанавливать при разблокировке",
            ],
        ),
        words(
            lang,
            [
                "Changes are saved immediately. Device controls are explicit and may request UAC.",
                "Modifications enregistrées immédiatement. Les commandes peuvent demander l’UAC.",
                "Änderungen werden sofort gespeichert. Gerätesteuerung kann UAC anfordern.",
                "Los cambios se guardan al instante. El control puede solicitar UAC.",
                "変更は即時保存されます。デバイス操作ではUAC確認が必要な場合があります。",
                "更改立即保存。设备控制可能请求 UAC。",
                "Изменения сохраняются сразу. Управление может запросить UAC.",
            ],
        ),
    ]
}
fn resource_covered(snapshot: &Snapshot, resource: Resource) -> bool {
    let required: &[&str] = match resource {
        Resource::Microphone => &["wasapi"],
        Resource::Camera => &["privacy_store", "module_scanner", "sensor_activity"],
    };
    !snapshot.observation_gaps.contains(&resource)
        && required.iter().all(|name| {
            snapshot
                .collectors
                .iter()
                .find(|collector| collector.collector == *name)
                .is_some_and(|collector| collector.state == CollectorState::Healthy)
        })
}

fn last_scan_attempt(lang: Language) -> &'static str {
    words(
        lang,
        [
            "Last scan attempt",
            "Dernière tentative d’analyse",
            "Letzter Scanversuch",
            "Último intento de análisis",
            "最終スキャン試行",
            "上次扫描尝试",
            "Последняя попытка сканирования",
        ],
    )
}

fn request_labels(lang: Language, requested: bool) -> (&'static str, &'static str) {
    (
        words(
            lang,
            [
                "Requested",
                "Demandé",
                "Angefordert",
                "Solicitado",
                "要求",
                "请求",
                "Запрошено",
            ],
        ),
        if requested {
            words(
                lang,
                ["On", "Activé", "Ein", "Activado", "オン", "开启", "Вкл"],
            )
        } else {
            words(
                lang,
                [
                    "Off",
                    "Désactivé",
                    "Aus",
                    "Desactivado",
                    "オフ",
                    "关闭",
                    "Выкл",
                ],
            )
        },
    )
}

fn mute_readback(lang: Language, state: MicrophoneMuteState) -> &'static str {
    match state {
        MicrophoneMuteState::Muted => words(
            lang,
            [
                "Muted",
                "Muet",
                "Stumm",
                "Silenciado",
                "ミュート",
                "已静音",
                "Заглушён",
            ],
        ),
        MicrophoneMuteState::Unmuted => words(
            lang,
            [
                "Not muted",
                "Non muet",
                "Nicht stumm",
                "Sin silenciar",
                "ミュート解除",
                "未静音",
                "Не заглушён",
            ],
        ),
        MicrophoneMuteState::Mixed => words(
            lang,
            [
                "Mixed",
                "Mixte",
                "Gemischt",
                "Mixto",
                "混在",
                "混合",
                "Смешанное",
            ],
        ),
        MicrophoneMuteState::Unavailable => unknown(lang),
    }
}

fn microphone_card(lang: Language, status: Option<&MicrophoneProtectionStatus>) -> String {
    match status {
        Some(status) => {
            let (requested, value) = request_labels(lang, status.requested);
            let actual = if status.endpoint_count == 0 {
                MicrophoneMuteState::Unavailable
            } else {
                status.mute_state
            };
            format!(
                "{requested}: {value} · SDK: {}\r\n{}",
                mute_readback(lang, actual),
                lang.service_state(status.service_active),
            )
        }
        None => lang.unknown_protection(true).into(),
    }
}

fn camera_readback(lang: Language, status: &CameraControlObservation) -> &'static str {
    if status.present_total == 0 || status.unknown_devices > 0 {
        return unknown(lang);
    }
    match status.state {
        CameraPrivacyState::Blocked => words(
            lang,
            [
                "Blocked",
                "Bloquées",
                "Blockiert",
                "Bloqueadas",
                "ブロック済み",
                "已阻止",
                "Заблокированы",
            ],
        ),
        CameraPrivacyState::Allowed => words(
            lang,
            [
                "Allowed",
                "Autorisées",
                "Zugelassen",
                "Permitidas",
                "許可",
                "已允许",
                "Разрешены",
            ],
        ),
        CameraPrivacyState::SystemManaged => words(
            lang,
            [
                "System-managed / mixed",
                "Gérées par le système / mixtes",
                "Systemverwaltet / gemischt",
                "Del sistema / mixtas",
                "システム管理・混在",
                "系统管理／混合",
                "Системное / смешанное",
            ],
        ),
    }
}

fn camera_card(lang: Language, status: Option<&CameraControlObservation>) -> String {
    match status {
        Some(status) => {
            let (requested, value) = request_labels(lang, status.desired_blocked);
            let actual = words(
                lang,
                [
                    "Devices",
                    "Périphériques",
                    "Geräte",
                    "Dispositivos",
                    "デバイス",
                    "设备",
                    "Устройства",
                ],
            );
            format!(
                "{requested}: {value} · {actual}: {}\r\n{}",
                camera_readback(lang, status),
                lang.service_state(status.helper_active)
            )
        }
        None => lang.unknown_protection(false).into(),
    }
}

impl Overview {
    pub(super) fn from_snapshot(
        snapshot: Result<&Snapshot, &str>,
        lang: Language,
        observed_at: chrono::DateTime<chrono::Utc>,
    ) -> Self {
        let observed = Some(observed_at);
        let Ok(snapshot) = snapshot else {
            return Self {
                observed,
                activity: [unknown(lang).into(), unknown(lang).into()],
                apps: unknown(lang).into(),
                coverage: format!(
                    "{}: {}\r\n{}",
                    coverage_label(lang),
                    unknown(lang),
                    snapshot.unwrap_err()
                ),
            };
        };
        let activity = [Resource::Microphone, Resource::Camera].map(|resource| {
            let count = snapshot
                .accesses
                .iter()
                .filter(|a| a.resource == resource && a.activity == Activity::Active)
                .count();
            let pipelines = snapshot
                .accesses
                .iter()
                .filter(|a| a.resource == resource && a.activity == Activity::Ready)
                .count();
            if count > 0 {
                format!("{} · {count}", active(lang))
            } else if pipelines > 0 {
                format!("{} · {pipelines}", ready(lang))
            } else if !resource_covered(snapshot, resource) {
                unknown(lang).into()
            } else {
                words(
                    lang,
                    [
                        "No activity observed",
                        "Aucune activité observée",
                        "Keine Aktivität beobachtet",
                        "Sin actividad observada",
                        "活動の観測なし",
                        "未观测到活动",
                        "Активность не замечена",
                    ],
                )
                .into()
            }
        });
        let mut apps = String::new();
        for access in snapshot.accesses.iter().take(8) {
            let mut name = String::with_capacity(access.application.len().min(256));
            for c in access.application.chars() {
                let c = if c.is_control()
                    || matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
                {
                    ' '
                } else {
                    c
                };
                if name.len() + c.len_utf8() > 256 {
                    break;
                }
                name.push(c);
            }
            apps.push_str(&format!(
                "{} — {} — {} — PID {}\r\n",
                lang.resource(access.resource),
                if access.activity == Activity::Active {
                    active(lang)
                } else {
                    ready(lang)
                },
                name,
                access.pid.map_or_else(|| "?".into(), |p| p.to_string())
            ));
        }
        if snapshot.accesses.is_empty() {
            apps.push_str(
                if ![Resource::Microphone, Resource::Camera]
                    .into_iter()
                    .all(|resource| resource_covered(snapshot, resource))
                {
                    unknown(lang)
                } else {
                    empty(lang)
                },
            );
        }
        let total = snapshot.accesses.len();
        apps.push_str(&format!(
            "\r\n{}: {} / {total}; {}: {}",
            words(
                lang,
                [
                    "Shown",
                    "Affichés",
                    "Angezeigt",
                    "Mostrados",
                    "表示",
                    "显示",
                    "Показано"
                ]
            ),
            total.min(8),
            words(
                lang,
                [
                    "Omitted",
                    "Omis",
                    "Ausgelassen",
                    "Omitidos",
                    "省略",
                    "省略",
                    "Пропущено"
                ]
            ),
            total.saturating_sub(8)
        ));
        let mut coverage = format!(
            "{}: {}\r\n",
            coverage_label(lang),
            if ![Resource::Microphone, Resource::Camera]
                .into_iter()
                .all(|resource| resource_covered(snapshot, resource))
                || snapshot
                    .collectors
                    .iter()
                    .any(|c| c.state != CollectorState::Healthy)
            {
                unknown(lang)
            } else {
                words(
                    lang,
                    [
                        "Current collectors responding; observation is not a guarantee",
                        "Collecteurs disponibles ; aucune garantie",
                        "Kollektoren antworten; keine Garantie",
                        "Colectores responden; no es garantía",
                        "収集元応答あり・保証ではありません",
                        "收集器响应，不代表保证",
                        "Источники отвечают; это не гарантия",
                    ],
                )
            }
        );
        for collector in &snapshot.collectors {
            coverage.push_str(&format!(
                "{}: {:?}{}\r\n",
                collector.collector,
                collector.state,
                collector
                    .detail
                    .as_deref()
                    .map_or(String::new(), |d| format!(" — {d}"))
            ));
        }
        for resource in &snapshot.observation_gaps {
            coverage.push_str(&format!(
                "{}: {} — {}\r\n",
                coverage_label(lang),
                lang.resource(*resource),
                unknown(lang)
            ));
        }
        Self {
            observed,
            activity,
            apps,
            coverage,
        }
    }
}

#[derive(Clone)]
pub(super) struct View {
    lang: Language,
    observed: String,
    microphone: String,
    camera: String,
    mic_protection: String,
    cam_protection: String,
    apps: String,
    diagnostics: String,
    error: String,
    profile: usize,
    explicit: bool,
    checks: [bool; 8],
    mic_enabled: bool,
    cam_enabled: bool,
    resume_enabled: bool,
    mic_action: String,
    cam_action: String,
}
impl View {
    pub(super) fn from_state(state: &TrayAppState) -> Self {
        let lang = state.lang;
        let microphone = microphone_card(
            lang,
            state
                .microphone_protection
                .as_ref()
                .filter(|_| state.microphone_status_error.is_none()),
        );
        let camera = camera_card(
            lang,
            state
                .camera_observation
                .as_ref()
                .filter(|_| state.camera_status_error.is_none()),
        );
        let mut error = state.settings_error.clone().unwrap_or_default();
        for e in state
            .operation_errors
            .messages()
            .chain(state.microphone_status_error.iter().map(String::as_str))
            .chain(state.camera_status_error.iter().map(String::as_str))
        {
            error.push_str(e);
            error.push('\n');
        }
        let mut diagnostics = state.summary.clone();
        diagnostics.push('\n');
        diagnostics.push_str(&state.overview.coverage);
        if !error.is_empty() {
            diagnostics.push('\n');
            diagnostics.push_str(&error);
        }
        if error.len() > 2048 {
            let mut end = 2048;
            while !error.is_char_boundary(end) {
                end -= 1;
            }
            error.truncate(end);
        }
        Self {
            lang,
            observed: format!(
                "{}: {}",
                last_scan_attempt(lang),
                state.overview.observed.map_or_else(
                    || unknown(lang).into(),
                    |t| t.with_timezone(&chrono::Local).to_rfc3339()
                )
            ),
            microphone: format!(
                "{}\r\n{}",
                lang.resource(Resource::Microphone),
                if state.overview.observed.is_some() {
                    state.overview.activity[0].as_str()
                } else {
                    unknown(lang)
                }
            ),
            camera: format!(
                "{}\r\n{}",
                lang.resource(Resource::Camera),
                if state.overview.observed.is_some() {
                    state.overview.activity[1].as_str()
                } else {
                    unknown(lang)
                }
            ),
            mic_protection: microphone,
            cam_protection: camera,
            apps: if state.overview.observed.is_some() {
                state.overview.apps.clone()
            } else {
                unknown(lang).into()
            },
            diagnostics,
            error,
            profile: PROFILE_CHOICES
                .iter()
                .position(|(_, p)| *p == state.settings.profile)
                .unwrap_or(0),
            explicit: state.explicit_policy,
            checks: [
                state.settings.notifications_enabled,
                state.settings.sound_enabled,
                state.settings.history_enabled,
                state.settings.notifications_paused(),
                state.autostart_state == crate::autostart::AutostartState::Enabled,
                state.settings.mute_on_lock,
                state.settings.block_camera_on_lock,
                state.settings.restore_on_unlock,
            ],
            mic_enabled: state.microphone_protection.is_some()
                && state.microphone_status_error.is_none()
                && state.pending_microphone == 0,
            cam_enabled: state.camera_observation.is_some()
                && state.camera_status_error.is_none()
                && state.pending_manual == 0
                && state.pending_lock_block.is_none()
                && state.pending_lock_restore.is_none(),
            resume_enabled: state
                .camera_observation
                .as_ref()
                .is_some_and(|c| c.desired_blocked && !c.helper_active)
                && state.camera_status_error.is_none()
                && state.pending_manual == 0
                && state.pending_lock_block.is_none()
                && state.pending_lock_restore.is_none(),
            mic_action: state
                .microphone_protection
                .as_ref()
                .map_or(lang.unknown_protection(true), |m| {
                    lang.protection_action(m.requested)
                })
                .into(),
            cam_action: state
                .camera_observation
                .as_ref()
                .map_or(lang.unknown_protection(false), |c| {
                    if c.desired_blocked {
                        camera_allow_label(lang)
                    } else {
                        camera_block_label(lang)
                    }
                })
                .into(),
        }
    }
}

/// Owns both HWND and callback storage. Win32 never owns/frees the Rust allocation.
pub(super) struct DesktopWindow {
    hwnd: HWND,
    data: Box<RefCell<WindowState>>,
}
fn rgb(red: u8, green: u8, blue: u8) -> COLORREF {
    COLORREF(u32::from(red) | u32::from(green) << 8 | u32::from(blue) << 16)
}
struct Surfaces {
    colors: [COLORREF; 4],
    brushes: [HBRUSH; 4], // card, button, selected, border
    muted: COLORREF,
    selected_text: COLORREF,
}
impl Surfaces {
    fn new(colors: [COLORREF; 4], muted: COLORREF, selected_text: COLORREF) -> Option<Self> {
        let brushes = colors.map(|color| unsafe { CreateSolidBrush(color) });
        if brushes.iter().any(|brush| brush.is_invalid()) {
            for brush in brushes {
                if !brush.is_invalid() {
                    let _ = unsafe { DeleteObject(brush.into()) };
                }
            }
            return None;
        }
        Some(Self {
            colors,
            brushes,
            muted,
            selected_text,
        })
    }
}
impl Drop for Surfaces {
    fn drop(&mut self) {
        for brush in self.brushes {
            let _ = unsafe { DeleteObject(brush.into()) };
        }
    }
}
struct WindowState {
    owner: HWND,
    font: Option<HFONT>,
    heading_font: Option<HFONT>,
    logo: Option<HICON>,
    background: COLORREF,
    foreground: COLORREF,
    brush: Option<HBRUSH>,
    surfaces: Option<Surfaces>,
    tab: usize,
    scroll: [i32; 4],
    controls: Vec<(i32, HWND, usize)>,
    texts: HashMap<i32, String>,
    view: View,
}
impl Drop for WindowState {
    fn drop(&mut self) {
        for font in [self.font.take(), self.heading_font.take()]
            .into_iter()
            .flatten()
        {
            let _ = unsafe { DeleteObject(font.into()) };
        }
        if let Some(icon) = self.logo.take() {
            let _ = unsafe { DestroyIcon(icon) };
        }
        if let Some(brush) = self.brush.take() {
            let _ = unsafe { DeleteObject(brush.into()) };
        }
    }
}
impl Drop for DesktopWindow {
    fn drop(&mut self) {
        let _ = unsafe { DestroyWindow(self.hwnd) };
    }
}
impl DesktopWindow {
    pub(super) fn create(owner: HWND, view: View) -> Result<Self> {
        let class = format_wide("MicCamWatchDesktopClass");
        let title = format_wide("MicCamWatch");
        unsafe {
            let _ = RegisterClassW(&WNDCLASSW {
                hbrBackground: GetSysColorBrush(COLOR_WINDOW),
                lpfnWndProc: Some(window_proc),
                lpszClassName: PCWSTR(class.as_ptr()),
                ..Default::default()
            });
        }
        let data = Box::new(RefCell::new(WindowState {
            owner,
            font: None,
            heading_font: None,
            logo: None,
            background: COLORREF(unsafe { GetSysColor(COLOR_WINDOW) }),
            foreground: COLORREF(unsafe { GetSysColor(COLOR_WINDOWTEXT) }),
            brush: None,
            surfaces: None,
            tab: 0,
            scroll: [0; 4],
            controls: Vec::new(),
            texts: HashMap::new(),
            view,
        }));
        let work = monitor_work_area(owner);
        let dpi = unsafe { GetDpiForWindow(owner) }.max(96) as i32;
        let width = (920 * dpi / 96).min(work.right - work.left);
        let height = (800 * dpi / 96).min(work.bottom - work.top);
        let pointer = (&*data as *const RefCell<WindowState>).cast::<std::ffi::c_void>();
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0x00010000),
                PCWSTR(class.as_ptr()),
                PCWSTR(title.as_ptr()),
                WS_OVERLAPPEDWINDOW | WS_VSCROLL | WINDOW_STYLE(0x02000000),
                work.left + (work.right - work.left - width) / 2,
                work.top + (work.bottom - work.top - height) / 2,
                width,
                height,
                Some(owner),
                None,
                None,
                Some(pointer),
            )?
        };
        let window = Self { hwnd, data };
        window.data.borrow_mut().create_controls(hwnd)?;
        Ok(window)
    }
    pub(super) fn is_visible(&self) -> bool {
        unsafe { IsWindowVisible(self.hwnd) != 0 }
    }
    pub(super) fn show(&self) {
        self.data.borrow_mut().render(self.hwnd);
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_RESTORE);
            let _ = SetForegroundWindow(self.hwnd);
        }
    }
    pub(super) fn update(&mut self, view: View) {
        let mut data = self.data.borrow_mut();
        data.view = view;
        if self.is_visible() {
            data.render(self.hwnd);
        }
    }
    pub(super) fn destroy(self) {
        drop(self);
    }
}
pub(super) fn translate_message(message: &MSG) -> bool {
    if message.hwnd.is_invalid() {
        return false;
    }
    let root = unsafe { GetAncestor(message.hwnd, 2) };
    let mut pid = 0u32;
    unsafe {
        GetWindowThreadProcessId(root, &mut pid);
    }
    if pid != unsafe { GetCurrentProcessId() } {
        return false;
    }
    use windows::Win32::UI::WindowsAndMessaging::GWLP_WNDPROC;
    // Match our own procedure, not a global FindWindow result: an installed tray
    // in another process may legitimately have the same class during local proof.
    if unsafe { GetWindowLongPtrW(root, GWLP_WNDPROC) } != window_proc as *const () as isize {
        return false;
    }
    // Use Win32's nested tab order without allocating a parallel control list.
    // Alt/Ctrl combinations keep their native meaning (e.g. Ctrl+Tab).
    if message.message == 0x0100
        && message.wParam.0 == 9
        && unsafe { GetKeyState(0x11) } >= 0
        && unsafe { GetKeyState(0x12) } >= 0
    {
        let next = unsafe { GetNextDlgTabItem(root, GetFocus(), i32::from(GetKeyState(0x10) < 0)) };
        if !next.is_invalid() {
            unsafe { SetFocus(next) };
            return true;
        }
    }
    unsafe { IsDialogMessageW(root, message) != 0 }
}
fn control(hwnd: HWND, id: i32) -> windows::core::Result<HWND> {
    unsafe { GetDlgItem(Some(hwnd), id) }.or_else(|_| {
        let viewport = unsafe { GetDlgItem(Some(hwnd), PAGE_VIEWPORT) }?;
        unsafe { GetDlgItem(Some(viewport), id) }
    })
}
// SendMessage/MoveWindow synchronously reenter the procedure. A shared RefCell
// with a scoped try_borrow prevents aliased mutable Rust references; notifications
// caused by our own presentation work require no second state transition.
fn with_window_state<T>(hwnd: HWND, f: impl FnOnce(&mut WindowState) -> T) -> Option<T> {
    let pointer = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *const RefCell<WindowState>;
    let data = unsafe { pointer.as_ref() }?;
    let mut state = data.try_borrow_mut().ok()?;
    Some(f(&mut state))
}
impl WindowState {
    fn add(
        &mut self,
        hwnd: HWND,
        id: i32,
        class: &str,
        caption: &str,
        style: WINDOW_STYLE,
        page: usize,
    ) -> Result<()> {
        let wrap = if class == "BUTTON" {
            WINDOW_STYLE(0x6000)
        } else {
            WINDOW_STYLE(0)
        };
        let class = format_wide(class);
        let caption = format_wide(caption);
        let parent = if page == 4 {
            hwnd
        } else {
            control(hwnd, PAGE_VIEWPORT)?
        };
        let child = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                PCWSTR(class.as_ptr()),
                PCWSTR(caption.as_ptr()),
                WS_CHILD | style | wrap | WINDOW_STYLE(0x04000000),
                0,
                0,
                0,
                0,
                Some(parent),
                Some(HMENU(id as usize as *mut _)),
                None,
                None,
            )?
        };
        self.controls.push((id, child, page));
        if id == DIAGNOSTIC || id == APPS {
            unsafe {
                SendMessageW(child, EM_SETLIMITTEXT, Some(WPARAM(0x7fff_fffe)), None);
            }
        }
        Ok(())
    }
    fn create_controls(&mut self, hwnd: HWND) -> Result<()> {
        let class = windows::core::w!("MicCamWatchDesktopViewport");
        unsafe {
            let _ = RegisterClassW(&WNDCLASSW {
                lpfnWndProc: Some(viewport_proc),
                lpszClassName: class,
                ..Default::default()
            });
            CreateWindowExW(
                WINDOW_EX_STYLE(0x0001_0000),
                class,
                windows::core::w!(""),
                WS_CHILD | WS_VISIBLE | WINDOW_STYLE(0x0200_0000),
                0,
                0,
                0,
                0,
                Some(hwnd),
                Some(HMENU(PAGE_VIEWPORT as usize as *mut _)),
                None,
                None,
            )?;
        }
        let lang = self.view.lang;
        self.add(
            hwnd,
            HEADER,
            "STATIC",
            &format!("MicCamWatch   v{}", env!("CARGO_PKG_VERSION")),
            WINDOW_STYLE(0),
            4,
        )?;
        self.add(hwnd, LOGO, "STATIC", "MicCamWatch", WINDOW_STYLE(3), 4)?;
        self.add(hwnd, FEEDBACK, "STATIC", "", WINDOW_STYLE(0x80), 4)?;
        for (id, page) in [
            (CARD_MIC, 0),
            (CARD_CAM, 0),
            (PANEL_PROFILE, 1),
            (PANEL_ALERTS, 1),
            (PANEL_AUTOMATION, 1),
        ] {
            self.add(hwnd, id, "STATIC", "", WINDOW_STYLE(0x0d), page)?;
        }
        self.add(
            hwnd,
            LIMIT,
            "STATIC",
            lang.protection_limit(),
            WINDOW_STYLE(0x80),
            0,
        )?;
        self.add(
            hwnd,
            CONTROLS_TITLE,
            "STATIC",
            words(
                lang,
                [
                    "Device controls",
                    "Commandes des appareils",
                    "Gerätesteuerung",
                    "Controles de dispositivos",
                    "デバイス操作",
                    "设备控制",
                    "Управление устройствами",
                ],
            ),
            WINDOW_STYLE(0x80),
            0,
        )?;
        for (id, text) in [
            (PROFILE_TITLE, profile_words(lang).0),
            (
                ALERTS_TITLE,
                words(
                    lang,
                    [
                        "Alerts and history",
                        "Alertes et historique",
                        "Hinweise und Verlauf",
                        "Avisos e historial",
                        "通知と履歴",
                        "通知与历史",
                        "Уведомления и история",
                    ],
                ),
            ),
            (
                AUTOMATION_TITLE,
                words(
                    lang,
                    [
                        "Startup and automation",
                        "Démarrage et automatisation",
                        "Start und Automatisierung",
                        "Inicio y automatización",
                        "起動と自動化",
                        "启动与自动化",
                        "Запуск и автоматизация",
                    ],
                ),
            ),
        ] {
            self.add(hwnd, id, "STATIC", text, WINDOW_STYLE(0x80), 1)?;
        }
        let tabs = [
            words(
                lang,
                [
                    "&Overview",
                    "&Vue d’ensemble",
                    "&Übersicht",
                    "&Resumen",
                    "概要(&O)",
                    "概览(&O)",
                    "&Обзор",
                ],
            ),
            words(
                lang,
                [
                    "&Settings",
                    "&Réglages",
                    "&Einstellungen",
                    "&Ajustes",
                    "設定(&S)",
                    "设置(&S)",
                    "&Настройки",
                ],
            ),
            words(
                lang,
                [
                    "&Diagnostics",
                    "&Diagnostic",
                    "&Diagnose",
                    "&Diagnóstico",
                    "診断(&D)",
                    "诊断(&D)",
                    "&Диагностика",
                ],
            ),
            words(
                lang,
                [
                    "&About",
                    "À &propos",
                    "&Info",
                    "&Acerca de",
                    "情報(&A)",
                    "关于(&A)",
                    "О &программе",
                ],
            ),
        ];
        for (index, text) in tabs.iter().enumerate() {
            self.add(
                hwnd,
                TAB + index as i32,
                "BUTTON",
                text,
                WS_TABSTOP | WINDOW_STYLE(0x0b),
                4,
            )?;
        }
        for id in [
            OBSERVED,
            MICROPHONE,
            CAMERA,
            MIC_PROTECTION,
            CAM_PROTECTION,
            APPS_TITLE,
        ] {
            self.add(hwnd, id, "STATIC", "", WINDOW_STYLE(0x80), 0)?;
        }
        let read = WS_TABSTOP
            | WS_VSCROLL
            | WINDOW_STYLE((ES_MULTILINE | ES_READONLY | ES_AUTOVSCROLL) as u32);
        self.add(hwnd, APPS, "EDIT", "", read, 0)?;
        for (id, text) in [
            (MIC_ACTION, ""),
            (CAM_ACTION, ""),
            (CAM_RESUME, camera_resume_label(lang)),
        ] {
            self.add(hwnd, id, "BUTTON", text, WS_TABSTOP | WINDOW_STYLE(0x0b), 0)?;
        }
        for (index, text) in profile_words(lang).1.iter().enumerate() {
            self.add(
                hwnd,
                PROFILE + index as i32,
                "BUTTON",
                text,
                WS_TABSTOP | WINDOW_STYLE(4),
                1,
            )?;
        }
        let labels = check_labels(lang);
        for (index, id) in [
            NOTIFICATIONS,
            SOUND,
            HISTORY,
            PAUSE,
            AUTOSTART,
            MUTE_LOCK,
            CAMERA_LOCK,
            RESTORE_UNLOCK,
        ]
        .iter()
        .enumerate()
        {
            self.add(
                hwnd,
                *id as i32,
                "BUTTON",
                labels[index],
                WS_TABSTOP | WINDOW_STYLE(3),
                1,
            )?;
        }
        self.add(hwnd, POLICY, "STATIC", "", WINDOW_STYLE(0x80), 1)?;
        self.add(hwnd, DIAGNOSTIC, "EDIT", "", read, 2)?;
        self.add(
            hwnd,
            COPY,
            "BUTTON",
            words(
                lang,
                [
                    "Copy &all",
                    "Tout &copier",
                    "Alles &kopieren",
                    "&Copiar todo",
                    "すべてコピー(&C)",
                    "全部复制(&C)",
                    "&Копировать всё",
                ],
            ),
            WS_TABSTOP | WINDOW_STYLE(0x0b),
            2,
        )?;
        self.add(hwnd, ABOUT, "STATIC", &format!("MicCamWatch v{}\r\n\r\n{}\r\n\r\n{}", env!("CARGO_PKG_VERSION"), words(lang, ["Local microphone and camera activity observation and explicit privacy controls.", "Observation locale du micro et des caméras, commandes explicites de confidentialité.", "Lokale Beobachtung von Mikrofon und Kamera mit expliziter Datenschutzsteuerung.", "Observación local de micrófono y cámaras y controles explícitos de privacidad.", "マイクとカメラのローカル観測と明示的なプライバシー操作。", "本地麦克风与摄像头活动观测及明确的隐私控制。", "Локальное наблюдение микрофона и камер и явное управление приватностью."]), words(lang, ["No automatic uploads or telemetry. Closing this window leaves the tray running. Unknown observations never mean safe or protected.", "Aucun envoi automatique ni télémétrie. Fermer la fenêtre laisse le tray actif. Inconnu ne signifie pas protégé.", "Keine automatischen Uploads oder Telemetrie. Schließen lässt das Tray aktiv. Unbekannt bedeutet nicht geschützt.", "Sin envíos automáticos ni telemetría. Cerrar conserva la bandeja. Desconocido no significa protegido.", "自動アップロード・テレメトリなし。閉じてもトレイは動作します。不明は安全や保護済みを意味しません。", "无自动上传或遥测。关闭窗口后托盘继续运行。未知不代表安全或已保护。", "Нет автоматической отправки или телеметрии. Закрытие оставляет трей. Неизвестно не означает защищено."])), WINDOW_STYLE(0x80), 3)?;
        // Newly created children follow older siblings in Win32 z-order.
        // Put decorative surfaces behind their labels and interactive controls.
        for (id, child, _) in &self.controls {
            if matches!(
                *id,
                CARD_MIC | CARD_CAM | PANEL_PROFILE | PANEL_ALERTS | PANEL_AUTOMATION
            ) {
                unsafe {
                    SetWindowPos(
                        *child,
                        Some(HWND_BOTTOM),
                        0,
                        0,
                        0,
                        0,
                        SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
                    )?;
                }
            }
        }
        self.font(hwnd);
        self.theme(hwnd);
        self.render(hwnd);
        self.layout(hwnd);
        Ok(())
    }
    fn font(&mut self, hwnd: HWND) {
        let Some(font) = native_menu_font(hwnd) else {
            return;
        };
        let mut metrics = NONCLIENTMETRICSW {
            cbSize: size_of::<NONCLIENTMETRICSW>() as u32,
            ..Default::default()
        };
        let heading = unsafe {
            if SystemParametersInfoForDpi(
                SPI_GETNONCLIENTMETRICS.0,
                metrics.cbSize,
                &mut metrics,
                0,
                GetDpiForWindow(hwnd).max(96),
            ) != 0
            {
                metrics.lfMessageFont.lfHeight = metrics.lfMessageFont.lfHeight * 3 / 2;
                metrics.lfMessageFont.lfWeight = 600;
                let f = CreateFontIndirectW(&metrics.lfMessageFont);
                (!f.is_invalid()).then_some(f)
            } else {
                None
            }
        };
        for (id, child, _) in &self.controls {
            let selected = if [
                HEADER,
                MICROPHONE,
                CAMERA,
                PROFILE_TITLE,
                ALERTS_TITLE,
                AUTOMATION_TITLE,
                APPS_TITLE,
                CONTROLS_TITLE,
            ]
            .contains(id)
            {
                heading.unwrap_or(font)
            } else {
                font
            };
            unsafe {
                SendMessageW(
                    *child,
                    WM_SETFONT,
                    Some(WPARAM(selected.0 as usize)),
                    Some(LPARAM(1)),
                );
            }
        }
        if let Some(old) = self.font.replace(font) {
            let _ = unsafe { DeleteObject(old.into()) };
        }
        if let Some(old) = std::mem::replace(&mut self.heading_font, heading) {
            let _ = unsafe { DeleteObject(old.into()) };
        }
        // Decode the shipped ICO, including in test binaries without .rc resources.
        let bytes = include_bytes!("../../../installer/miccamwatch.ico");
        let count = u16::from_le_bytes([bytes[4], bytes[5]]) as usize;
        let desired = 48 * unsafe { GetDpiForWindow(hwnd) }.max(96) as i32 / 96;
        let entry = (0..count)
            .map(|i| 6 + i * 16)
            .filter(|i| i + 16 <= bytes.len())
            .min_by_key(|i| {
                ((if bytes[*i] == 0 {
                    256
                } else {
                    i32::from(bytes[*i])
                }) - desired)
                    .abs()
            });
        if let Some(i) = entry {
            let size = u32::from_le_bytes(bytes[i + 8..i + 12].try_into().unwrap()) as usize;
            let offset = u32::from_le_bytes(bytes[i + 12..i + 16].try_into().unwrap()) as usize;
            if let Some(image) = offset
                .checked_add(size)
                .and_then(|end| bytes.get(offset..end))
            {
                use windows::Win32::UI::WindowsAndMessaging::CreateIconFromResourceEx;
                if let Ok(icon) = unsafe {
                    CreateIconFromResourceEx(
                        image,
                        true,
                        0x00030000,
                        desired,
                        desired,
                        Default::default(),
                    )
                } {
                    if let Ok(child) = unsafe { GetDlgItem(Some(hwnd), LOGO) } {
                        unsafe {
                            SendMessageW(child, 0x0170, Some(WPARAM(icon.0 as usize)), None);
                        }
                    }
                    unsafe {
                        SendMessageW(hwnd, 0x0080, Some(WPARAM(1)), Some(LPARAM(icon.0 as isize)));
                    }
                    if let Some(old) = self.logo.replace(icon) {
                        let _ = unsafe { DestroyIcon(old) };
                    }
                }
            }
        }
    }
    fn theme(&mut self, hwnd: HWND) {
        let mut hc = HighContrast {
            size: size_of::<HighContrast>() as u32,
            flags: 0,
            scheme: std::ptr::null_mut(),
        };
        unsafe {
            SystemParametersInfoW(0x0042, hc.size, (&mut hc as *mut HighContrast).cast(), 0);
        }
        let mut background = COLORREF(unsafe { GetSysColor(COLOR_WINDOW) });
        let mut foreground = COLORREF(unsafe { GetSysColor(COLOR_WINDOWTEXT) });
        if hc.flags & 1 == 0 {
            DESKTOP_RUNTIME.with(|runtime| {
                if let Some(settings) = runtime.settings.as_ref()
                    && let (Ok(bg), Ok(fg)) = (
                        settings.GetColorValue(UIColorType::Background),
                        settings.GetColorValue(UIColorType::Foreground),
                    )
                {
                    background =
                        COLORREF(u32::from(bg.R) | u32::from(bg.G) << 8 | u32::from(bg.B) << 16);
                    foreground =
                        COLORREF(u32::from(fg.R) | u32::from(fg.G) << 8 | u32::from(fg.B) << 16);
                }
            });
        }
        self.palette(hwnd, background, foreground, hc.flags & 1 != 0);
    }
    fn palette(
        &mut self,
        hwnd: HWND,
        background: COLORREF,
        foreground: COLORREF,
        high_contrast: bool,
    ) {
        let luminance =
            (background.0 & 255) + ((background.0 >> 8) & 255) + ((background.0 >> 16) & 255);
        let dark = !high_contrast && luminance < 384;
        let (background, foreground, colors, muted, selected_text) = if high_contrast {
            (
                background,
                foreground,
                [
                    background,
                    COLORREF(unsafe { GetSysColor(COLOR_BTNFACE) }),
                    COLORREF(unsafe { GetSysColor(COLOR_HIGHLIGHT) }),
                    foreground,
                ],
                COLORREF(unsafe { GetSysColor(COLOR_GRAYTEXT) }),
                COLORREF(unsafe { GetSysColor(COLOR_HIGHLIGHTTEXT) }),
            )
        } else if dark {
            (
                rgb(23, 29, 26),
                rgb(230, 239, 233),
                [
                    rgb(32, 41, 35),
                    rgb(44, 56, 48),
                    rgb(41, 76, 58),
                    rgb(67, 86, 74),
                ],
                rgb(174, 193, 180),
                rgb(230, 239, 233),
            )
        } else {
            (
                rgb(244, 247, 245),
                rgb(31, 45, 37),
                [
                    rgb(255, 255, 255),
                    rgb(232, 239, 235),
                    rgb(201, 230, 214),
                    rgb(196, 211, 202),
                ],
                rgb(78, 100, 86),
                rgb(25, 49, 37),
            )
        };
        if self.surfaces.as_ref().is_none_or(|surface| {
            surface.colors != colors
                || surface.muted != muted
                || surface.selected_text != selected_text
        }) && let Some(surface) = Surfaces::new(colors, muted, selected_text)
        {
            self.surfaces = Some(surface);
        }
        if self.brush.is_none() || self.background != background {
            let brush = unsafe { CreateSolidBrush(background) };
            if !brush.is_invalid()
                && let Some(old) = self.brush.replace(brush)
            {
                let _ = unsafe { DeleteObject(old.into()) };
            }
        }
        self.background = background;
        self.foreground = foreground;
        for (id, child, _) in &self.controls {
            if (PROFILE..PROFILE + 4).contains(id)
                || [
                    NOTIFICATIONS as i32,
                    SOUND as i32,
                    HISTORY as i32,
                    PAUSE as i32,
                    AUTOSTART as i32,
                    MUTE_LOCK as i32,
                    CAMERA_LOCK as i32,
                    RESTORE_UNLOCK as i32,
                ]
                .contains(id)
            {
                // Classic checkboxes honor WM_CTLCOLORSTATIC; retain native
                // semantics, toggle state and accessibility, including HC.
                let empty = [0u16];
                unsafe {
                    SetWindowTheme(
                        *child,
                        if dark {
                            empty.as_ptr()
                        } else {
                            std::ptr::null()
                        },
                        if dark {
                            empty.as_ptr()
                        } else {
                            std::ptr::null()
                        },
                    );
                }
            }
        }
        let value = i32::from(dark);
        unsafe {
            let _ = DwmSetWindowAttribute(hwnd, 20, &value, size_of::<i32>() as u32);
        }
    }
    fn on_surface(id: i32) -> bool {
        [
            MICROPHONE,
            CAMERA,
            MIC_PROTECTION,
            CAM_PROTECTION,
            APPS,
            DIAGNOSTIC,
            PROFILE_TITLE,
            ALERTS_TITLE,
            AUTOMATION_TITLE,
        ]
        .contains(&id)
            || (PROFILE..PROFILE + 4).contains(&id)
            || [
                NOTIFICATIONS,
                SOUND,
                HISTORY,
                PAUSE,
                AUTOSTART,
                MUTE_LOCK,
                CAMERA_LOCK,
                RESTORE_UNLOCK,
            ]
            .contains(&(id as usize))
    }
    fn draw_item(&self, item: &DRAWITEMSTRUCT) -> LRESULT {
        let Some(surface) = &self.surfaces else {
            return LRESULT(0);
        };
        let id = item.CtlID as i32;
        if [
            CARD_MIC,
            CARD_CAM,
            PANEL_PROFILE,
            PANEL_ALERTS,
            PANEL_AUTOMATION,
        ]
        .contains(&id)
        {
            unsafe {
                FillRect(item.hDC, &item.rcItem, surface.brushes[0]);
                FrameRect(item.hDC, &item.rcItem, surface.brushes[3]);
            }
            return LRESULT(1);
        }
        let selected = (TAB..TAB + 4).contains(&id) && self.tab == (id - TAB) as usize;
        let pressed = item.itemState.0 & 1 != 0;
        let disabled = item.itemState.0 & 4 != 0;
        let saved = unsafe { SaveDC(item.hDC) };
        unsafe {
            FillRect(
                item.hDC,
                &item.rcItem,
                surface.brushes[if selected || pressed { 2 } else { 1 }],
            );
            FrameRect(item.hDC, &item.rcItem, surface.brushes[3]);
            SetBkMode(item.hDC, TRANSPARENT);
            SetTextColor(
                item.hDC,
                if disabled {
                    surface.muted
                } else if selected || pressed {
                    surface.selected_text
                } else {
                    self.foreground
                },
            );
            if let Some(font) = self.font {
                SelectObject(item.hDC, font.into());
            }
        }
        let mut caption = [0u16; 512];
        let length = unsafe { GetWindowTextW(item.hwndItem, &mut caption) }.max(0) as usize;
        let dpi = unsafe { GetDpiForWindow(item.hwndItem) }.max(96) as i32;
        let pad = 8 * dpi / 96;
        let mut rect = item.rcItem;
        rect.left += pad;
        rect.right -= pad;
        let mut measured = RECT {
            left: rect.left,
            right: rect.right,
            top: 0,
            bottom: 0,
        };
        unsafe {
            DrawTextW(
                item.hDC,
                &mut caption[..length],
                &mut measured,
                DT_CENTER | DT_WORDBREAK | DT_CALCRECT,
            );
        }
        let text_height = measured.bottom - measured.top;
        rect.top += ((rect.bottom - rect.top - text_height) / 2).max(0);
        unsafe {
            DrawTextW(
                item.hDC,
                &mut caption[..length],
                &mut rect,
                DT_CENTER | DT_WORDBREAK,
            );
        }
        if item.itemState.0 & 0x10 != 0 && item.itemState.0 & 0x200 == 0 {
            let mut focus = item.rcItem;
            let inset = 3 * dpi / 96;
            focus.left += inset;
            focus.right -= inset;
            focus.top += inset;
            focus.bottom -= inset;
            unsafe {
                let _ = DrawFocusRect(item.hDC, &focus);
            }
        }
        if saved != 0 {
            let _ = unsafe { RestoreDC(item.hDC, saved) };
        }
        LRESULT(1)
    }
    fn text(&mut self, id: i32, value: String) {
        if self.texts.get(&id) == Some(&value) {
            return;
        }
        if let Some((_, child, _)) = self.controls.iter().find(|(i, _, _)| *i == id) {
            // Keep native EDIT selection and scroll on accepted live updates.
            let mut start = 0u32;
            let mut end = 0u32;
            let first = unsafe { SendMessageW(*child, EM_GETFIRSTVISIBLELINE, None, None) }.0;
            unsafe {
                SendMessageW(
                    *child,
                    EM_GETSEL,
                    Some(WPARAM((&mut start as *mut u32) as usize)),
                    Some(LPARAM((&mut end as *mut u32) as isize)),
                );
            }
            let text = format_wide(&diagnostic_document(&value));
            let _ = unsafe { SetWindowTextW(*child, PCWSTR(text.as_ptr())) };
            unsafe {
                SendMessageW(
                    *child,
                    EM_SETSEL,
                    Some(WPARAM(start as usize)),
                    Some(LPARAM(end as isize)),
                );
                let now = SendMessageW(*child, EM_GETFIRSTVISIBLELINE, None, None).0;
                SendMessageW(*child, EM_LINESCROLL, None, Some(LPARAM(first - now)));
            }
        }
        self.texts.insert(id, value);
    }
    fn render(&mut self, hwnd: HWND) {
        self.text(OBSERVED, self.view.observed.clone());
        self.text(MICROPHONE, self.view.microphone.clone());
        self.text(CAMERA, self.view.camera.clone());
        self.text(MIC_PROTECTION, self.view.mic_protection.clone());
        self.text(CAM_PROTECTION, self.view.cam_protection.clone());
        self.text(
            APPS_TITLE,
            words(
                self.view.lang,
                [
                    "Current applications",
                    "Applications actuelles",
                    "Aktuelle Anwendungen",
                    "Aplicaciones actuales",
                    "現在のアプリ",
                    "当前应用",
                    "Текущие приложения",
                ],
            )
            .into(),
        );
        let feedback = if self.view.error.is_empty() {
            ""
        } else {
            words(
                self.view.lang,
                [
                    "Attention needed — see Diagnostics for details",
                    "Attention requise — voir Diagnostic",
                    "Bitte Diagnose prüfen",
                    "Requiere atención — consulte Diagnóstico",
                    "確認が必要です — 診断を参照",
                    "需要注意 — 请查看诊断",
                    "Требуется внимание — см. диагностику",
                ],
            )
        };
        self.text(FEEDBACK, feedback.into());
        self.text(APPS, self.view.apps.clone());
        self.text(DIAGNOSTIC, self.view.diagnostics.clone());
        self.text(
            MIC_ACTION,
            format!(
                "{}: {}",
                self.view.lang.resource(Resource::Microphone),
                self.view.mic_action
            ),
        );
        self.text(CAM_ACTION, self.view.cam_action.clone());
        let policy = if self.view.explicit {
            profile_words(self.view.lang).2
        } else {
            check_labels(self.view.lang)[8]
        };
        self.text(POLICY, policy.into());
        for (index, id) in [
            NOTIFICATIONS,
            SOUND,
            HISTORY,
            PAUSE,
            AUTOSTART,
            MUTE_LOCK,
            CAMERA_LOCK,
            RESTORE_UNLOCK,
        ]
        .iter()
        .enumerate()
        {
            if let Ok(child) = control(hwnd, *id as i32) {
                unsafe {
                    SendMessageW(
                        child,
                        BM_SETCHECK,
                        Some(WPARAM(usize::from(self.view.checks[index]))),
                        None,
                    );
                }
            }
        }
        for index in 0..4 {
            if let Ok(child) = control(hwnd, PROFILE + index) {
                unsafe {
                    SendMessageW(
                        child,
                        BM_SETCHECK,
                        Some(WPARAM(usize::from(
                            self.view.profile == index as usize && !self.view.explicit,
                        ))),
                        None,
                    );
                    EnableWindow(child, i32::from(!self.view.explicit));
                }
            }
        }
        for index in 0..4 {
            if let Ok(child) = unsafe { GetDlgItem(Some(hwnd), TAB + index) } {
                unsafe {
                    // Selection belongs to the retained view model, not the
                    // native transient "button held down" mouse state.
                    InvalidateRect(child, std::ptr::null(), 1);
                }
            }
        }
        for (id, enabled) in [
            (MIC_ACTION, self.view.mic_enabled),
            (CAM_ACTION, self.view.cam_enabled),
            (CAM_RESUME, self.view.resume_enabled),
        ] {
            if let Ok(child) = control(hwnd, id) {
                unsafe {
                    EnableWindow(child, i32::from(enabled));
                }
            }
        }
        // Render never moves focus or scrolls a page.
    }
    fn layout(&mut self, hwnd: HWND) {
        let mut r = RECT::default();
        let _ = unsafe { GetClientRect(hwnd, &mut r) };
        let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96) as i32;
        let u = |n: i32| n * dpi / 96;
        let width = (r.right * 96 / dpi).max(1);
        let height = (r.bottom * 96 / dpi - 156).max(1);
        if let Ok(viewport) = control(hwnd, PAGE_VIEWPORT) {
            let _ = unsafe { MoveWindow(viewport, 0, u(PAGE_TOP), r.right, u(height), true) };
        }
        let wide = width >= 740;
        let extra = if wide { 0 } else { 172 };
        let half = (width - 56) / 2;
        let content = match self.tab {
            0 => 604 + extra,
            1 => {
                if wide {
                    456
                } else {
                    680
                }
            }
            _ => height,
        };
        self.scroll[self.tab] = self.scroll[self.tab].clamp(0, (content - height).max(0));
        let scroll = self.scroll[self.tab];
        let info = ScrollInfo {
            size: size_of::<ScrollInfo>() as u32,
            mask: 0x0f,
            min: 0,
            max: content - 1,
            page: height as u32,
            pos: scroll,
            track: 0,
        };
        unsafe {
            SetScrollInfo(hwnd, 1, &info, 1);
        }
        for (id, child, page) in &self.controls {
            let visible = *page == 4 || *page == self.tab;
            unsafe {
                let _ = ShowWindow(*child, if visible { SW_SHOW } else { SW_HIDE });
            }
            if !visible {
                continue;
            }
            let column = if wide { half } else { width - 48 };
            let camera_x = if wide { 32 + half } else { 24 };
            let auto_y = if wide { 224 } else { 448 };
            let auto_x = if wide { 32 + half } else { 24 };
            let (x, y, w, h) = match *id {
                LOGO => (24, 20, 48, 48),
                HEADER => (88, 24, width - 112, 40),
                i if (TAB..TAB + 4).contains(&i) => (
                    24 + (i - TAB) * (width - 48) / 4,
                    78,
                    (width - 48) / 4 - 4,
                    36,
                ),
                FEEDBACK => (24, 124, width - 48, 28),
                OBSERVED => (24, 4, width - 48, 28),
                CARD_MIC => (24, 44, column, 160),
                CARD_CAM => (camera_x, 44 + extra, column, 160),
                MICROPHONE => (40, 60, column - 32, 52),
                CAMERA => (camera_x + 16, 60 + extra, column - 32, 52),
                MIC_PROTECTION => (40, 116, column - 32, 72),
                CAM_PROTECTION => (camera_x + 16, 116 + extra, column - 32, 72),
                LIMIT => (24, 216 + extra, width - 48, 48),
                APPS_TITLE => (24, 276 + extra, width - 48, 28),
                APPS => (24, 308 + extra, width - 48, 136),
                CONTROLS_TITLE => (24, 464 + extra, width - 48, 28),
                MIC_ACTION => (24, 500 + extra, half, 36),
                CAM_ACTION => (32 + half, 500 + extra, half, 36),
                CAM_RESUME => (24, 544 + extra, width - 48, 36),
                PANEL_PROFILE => (24, 8, width - 48, 136),
                PROFILE_TITLE => (40, 20, width - 80, 28),
                i if (PROFILE..PROFILE + 4).contains(&i) => (
                    40 + (i - PROFILE) % 2 * (width - 80) / 2,
                    56 + (i - PROFILE) / 2 * 40,
                    (width - 80) / 2 - 8,
                    32,
                ),
                POLICY => (24, 156, width - 48, 52),
                PANEL_ALERTS => (24, 224, column, 208),
                PANEL_AUTOMATION => (auto_x, auto_y, column, 208),
                ALERTS_TITLE => (40, 236, column - 32, 40),
                AUTOMATION_TITLE => (auto_x + 16, auto_y + 12, column - 32, 40),
                i if (NOTIFICATIONS as i32..=PAUSE as i32).contains(&i) => {
                    (40, 284 + (i - NOTIFICATIONS as i32) * 32, column - 32, 32)
                }
                i if i == AUTOSTART as i32 => (auto_x + 16, auto_y + 60, column - 32, 32),
                i if (MUTE_LOCK as i32..=RESTORE_UNLOCK as i32).contains(&i) => (
                    auto_x + 16,
                    auto_y + 92 + (i - MUTE_LOCK as i32) * 32,
                    column - 32,
                    32,
                ),
                DIAGNOSTIC => (24, 8, width - 48, (height - 64).max(60)),
                COPY => (24, height - 44, 192.min(width - 48), 36),
                ABOUT => (24, 12, width - 48, height - 24),
                _ => continue,
            };
            // Native child clipping keeps partially scrolled controls out of
            // the fixed header without hiding keyboard-reachable page content.
            let y = if *page == 4 { y } else { y - scroll };
            let (w, h) = (w.max(1), h.max(1));
            let _ = unsafe { MoveWindow(*child, u(x), u(y), u(w), u(h), false) };
            unsafe {
                InvalidateRect(*child, std::ptr::null(), 1);
            }
        }
        unsafe {
            InvalidateRect(hwnd, std::ptr::null(), 1);
        }
    }
}
unsafe extern "system" fn viewport_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if matches!(
        message,
        WM_COMMAND | 0x002b | 0x0138 | 0x0133 | 0x0135 | 0x0014 | 0x020a
    ) {
        let parent = unsafe { GetParent(hwnd) };
        if !parent.is_invalid() {
            return unsafe { SendMessageW(parent, message, Some(wparam), Some(lparam)) };
        }
    }
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}
unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_NCCREATE {
        let c = unsafe { &*(lparam.0 as *const CREATESTRUCTW) };
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, c.lpCreateParams as isize);
        }
        return LRESULT(1);
    }
    if message == WM_NCDESTROY {
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        }
        return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
    }
    if message == WM_CLOSE {
        unsafe {
            let _ = ShowWindow(hwnd, SW_HIDE);
        }
        return LRESULT(0);
    }
    // Default painting/printing must run without a state borrow: DefWindowProc
    // synchronously asks us for background/control colors while painting.
    if !matches!(
        message,
        0x0014
            | 0x0138
            | 0x0133
            | 0x0135
            | 0x002b
            | 0x0115
            | 0x020a
            | WM_SIZE
            | WM_DPICHANGED
            | WM_SETTINGCHANGE
            | WM_THEMECHANGED
            | WM_GETMINMAXINFO
            | WM_COMMAND
    ) {
        return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
    }
    with_window_state(hwnd, |s| match message {
        0x002b => {
            if let Some(item) = unsafe { (lparam.0 as *const DRAWITEMSTRUCT).as_ref() } {
                s.draw_item(item)
            } else {
                LRESULT(0)
            }
        }
        0x0014 => {
            // WM_ERASEBKGND
            let mut rect = RECT::default();
            unsafe {
                let _ = GetClientRect(hwnd, &mut rect);
                FillRect(
                    HDC(wparam.0 as *mut _),
                    &rect,
                    s.brush.unwrap_or_else(|| GetSysColorBrush(COLOR_WINDOW)),
                );
            }
            LRESULT(1)
        }
        0x0138 | 0x0133 | 0x0135 => {
            // STATIC / EDIT / BUTTON
            let dc = HDC(wparam.0 as *mut _);
            let id = unsafe { GetDlgCtrlID(HWND(lparam.0 as *mut _)) };
            let surface = s.surfaces.as_ref().filter(|_| WindowState::on_surface(id));
            let brush = surface.map_or_else(
                || {
                    s.brush
                        .unwrap_or_else(|| unsafe { GetSysColorBrush(COLOR_WINDOW) })
                },
                |surface| surface.brushes[0],
            );
            let background = surface.map_or(s.background, |surface| surface.colors[0]);
            let foreground = if [OBSERVED, LIMIT, POLICY, FEEDBACK].contains(&id) {
                s.surfaces
                    .as_ref()
                    .map_or(s.foreground, |surface| surface.muted)
            } else {
                s.foreground
            };
            unsafe {
                SetTextColor(dc, foreground);
                SetBkColor(dc, background);
            }
            LRESULT(brush.0 as isize)
        }
        0x0115 | 0x020a => {
            // WM_VSCROLL / WM_MOUSEWHEEL
            let current = s.scroll[s.tab];
            let value = if message == 0x020a {
                current - (wparam.0 >> 16) as u16 as i16 as i32 / 120 * 64
            } else {
                match wparam.0 & 0xffff {
                    0 => current - 32,
                    1 => current + 32,
                    2 => current - 240,
                    3 => current + 240,
                    4 | 5 => (wparam.0 >> 16) as i32,
                    6 => 0,
                    7 => i32::MAX,
                    _ => current,
                }
            };
            s.scroll[s.tab] = value.max(0);
            s.layout(hwnd);
            LRESULT(0)
        }
        WM_SIZE => {
            s.layout(hwnd);
            LRESULT(0)
        }
        WM_DPICHANGED => {
            let r = unsafe { &*(lparam.0 as *const RECT) };
            let _ = unsafe {
                MoveWindow(
                    hwnd,
                    r.left,
                    r.top,
                    r.right - r.left,
                    r.bottom - r.top,
                    true,
                )
            };
            s.font(hwnd);
            s.layout(hwnd);
            LRESULT(0)
        }
        WM_SETTINGCHANGE => {
            if wparam.0 == 0x002a {
                s.font(hwnd);
            }
            s.theme(hwnd);
            s.layout(hwnd);
            LRESULT(0)
        }
        WM_THEMECHANGED => {
            s.theme(hwnd);
            s.layout(hwnd);
            LRESULT(0)
        }
        WM_GETMINMAXINFO => {
            // MINMAXINFO contains five POINTs; enforce usable keyboard/DPI layout.
            let points = unsafe { &mut *(lparam.0 as *mut [POINT; 5]) };
            let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96) as i32;
            points[3] = POINT {
                x: 560 * dpi / 96,
                y: 480 * dpi / 96,
            };
            LRESULT(0)
        }
        WM_COMMAND => {
            let id = (wparam.0 & 0xffff) as i32;
            let notification = wparam.0 >> 16;
            if notification == 6 || notification == 0x0100 {
                let focus = unsafe { GetFocus() };
                // Only reveal page content; header/navigation never scroll.
                let page_control = s
                    .controls
                    .iter()
                    .any(|(_, child, page)| *child == focus && *page == s.tab);
                if !page_control {
                    return LRESULT(0);
                }
                let viewport = control(hwnd, PAGE_VIEWPORT).unwrap_or(hwnd);
                let mut rect = RECT::default();
                let mut client = RECT::default();
                unsafe {
                    GetWindowRect(focus, &mut rect);
                    let _ = GetClientRect(viewport, &mut client);
                }
                let mut top = POINT {
                    x: rect.left,
                    y: rect.top,
                };
                unsafe {
                    ScreenToClient(viewport, &mut top);
                }
                let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96) as i32;
                let bottom = top.y + rect.bottom - rect.top;
                let delta = if top.y < 0 {
                    top.y
                } else {
                    (bottom - client.bottom).max(0)
                };
                s.scroll[s.tab] = (s.scroll[s.tab] + delta * 96 / dpi).max(0);
                s.layout(hwnd);
                return LRESULT(0);
            }
            if notification != 0 {
                return LRESULT(0);
            }
            if (TAB..TAB + 4).contains(&id) {
                s.tab = (id - TAB) as usize;
                s.scroll[s.tab] = 0;
                s.render(hwnd);
                s.layout(hwnd);
                // Keep keyboard focus on the selected navigation control.
                if let Ok(c) = unsafe { GetDlgItem(Some(hwnd), id) } {
                    unsafe {
                        SetFocus(c);
                    }
                }
            } else if (PROFILE..PROFILE + 4).contains(&id) && !s.view.explicit {
                let _ = unsafe {
                    PostMessageW(
                        Some(s.owner),
                        WM_COMMAND,
                        WPARAM(PROFILE_CHOICES[(id - PROFILE) as usize].0),
                        LPARAM(0),
                    )
                };
            } else if [
                NOTIFICATIONS,
                SOUND,
                HISTORY,
                PAUSE,
                AUTOSTART,
                MUTE_LOCK,
                CAMERA_LOCK,
                RESTORE_UNLOCK,
            ]
            .contains(&(id as usize))
            {
                // BS_AUTOCHECKBOX already contains the desired value. Never invert
                // a stale frame or issue toggles during refresh.
                if let Ok(c) = control(hwnd, id) {
                    let desired = unsafe { SendMessageW(c, BM_GETCHECK, None, None) }.0 == 1;
                    let _ = unsafe {
                        PostMessageW(
                            Some(s.owner),
                            WM_DESKTOP_SETTING,
                            WPARAM(id as usize),
                            LPARAM(isize::from(desired)),
                        )
                    };
                }
            } else if id == COPY {
                if let Ok(c) = control(hwnd, DIAGNOSTIC) {
                    unsafe {
                        SendMessageW(c, EM_SETSEL, Some(WPARAM(0)), Some(LPARAM(-1)));
                        SendMessageW(c, WM_COPY, None, None);
                    }
                }
            } else {
                let action = match id {
                    MIC_ACTION if s.view.mic_enabled => Some(CMD_TOGGLE_MUTE),
                    CAM_ACTION if s.view.cam_enabled => Some(CMD_TOGGLE_CAMERA),
                    CAM_RESUME if s.view.resume_enabled => Some(CMD_RESUME_CAMERA),
                    _ => None,
                };
                if let Some(command) = action {
                    let _ = unsafe {
                        PostMessageW(Some(s.owner), WM_COMMAND, WPARAM(command), LPARAM(0))
                    };
                }
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    })
    .unwrap_or_else(|| unsafe { DefWindowProcW(hwnd, message, wparam, lparam) })
}

#[cfg(test)]
#[path = "windows_desktop_proof.rs"]
mod tests;
