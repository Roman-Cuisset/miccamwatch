//! Retained Win32 presentation. Only owner messages can change authoritative state.
use super::*;
use windows::Win32::UI::WindowsAndMessaging::{SW_HIDE, SW_RESTORE, WM_NCDESTROY};

const HEADER: i32 = 300;
const TAB: i32 = 310;
const OBSERVED: i32 = 320;
const MICROPHONE: i32 = 321;
const CAMERA: i32 = 322;
const APPS: i32 = 323;
const COVERAGE: i32 = 324;
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
const ERROR: i32 = 346;
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
    values[match lang { Language::En => 0, Language::Fr => 1, Language::De => 2,
        Language::Es => 3, Language::Ja => 4, Language::Zh => 5, Language::Ru => 6 }]
}
fn unknown(lang: Language) -> &'static str {
    words(lang, ["Unknown / unavailable", "Inconnu / indisponible", "Unbekannt / nicht verfügbar", "Desconocido / no disponible", "不明・利用不可", "未知／不可用", "Неизвестно / недоступно"])
}
fn active(lang: Language) -> &'static str {
    words(lang, ["Observed active", "Activité observée", "Aktivität beobachtet", "Actividad observada", "使用中を観測", "观测到使用中", "Наблюдается активность"])
}
fn ready(lang: Language) -> &'static str {
    words(lang, ["Ready pipeline, not proven capture", "Pipeline prêt, capture non prouvée", "Pipeline bereit, Aufnahme nicht belegt", "Canal listo, captura no demostrada", "準備済み・キャプチャ未確認", "管线就绪，未证实采集", "Канал готов, захват не подтверждён"])
}
fn empty(lang: Language) -> &'static str {
    words(lang, ["No access observed in this scan (not proof of safety)", "Aucun accès observé (ne prouve pas la sécurité)", "Kein Zugriff beobachtet (kein Sicherheitsnachweis)", "Sin acceso observado (no demuestra seguridad)", "今回アクセス観測なし（安全の証明ではありません）", "本次未观测到访问（不代表安全）", "Доступ не замечен (не доказательство безопасности)"])
}
fn coverage_label(lang: Language) -> &'static str {
    words(lang, ["Observation coverage", "Couverture d’observation", "Beobachtungsabdeckung", "Cobertura de observación", "観測範囲", "观测覆盖", "Охват наблюдения"])
}
pub(super) fn open_label(lang: Language) -> &'static str {
    words(lang, ["Open MicCamWatch…", "Ouvrir MicCamWatch…", "MicCamWatch öffnen…", "Abrir MicCamWatch…", "MicCamWatch を開く…", "打开 MicCamWatch…", "Открыть MicCamWatch…"])
}
fn check_labels(lang: Language) -> [&'static str; 9] {
    [
        words(lang, ["Notifications", "Notifications", "Benachrichtigungen", "Notificaciones", "通知", "通知", "Уведомления"]),
        words(lang, ["Notification sound", "Son des notifications", "Benachrichtigungston", "Sonido de notificación", "通知音", "通知声音", "Звук уведомлений"]),
        words(lang, ["Local history", "Historique local", "Lokaler Verlauf", "Historial local", "ローカル履歴", "本地历史", "Локальная история"]),
        words(lang, ["Pause notifications for 1 hour", "Suspendre les notifications 1 h", "Benachrichtigungen 1 Stunde pausieren", "Pausar notificaciones 1 hora", "通知を1時間停止", "暂停通知1小时", "Приостановить уведомления на час"]),
        words(lang, ["Start at sign-in", "Démarrer à la connexion", "Bei Anmeldung starten", "Iniciar al acceder", "サインイン時に起動", "登录时启动", "Запуск при входе"]),
        words(lang, ["Mute microphone on lock", "Couper le micro au verrouillage", "Mikrofon beim Sperren stummschalten", "Silenciar micrófono al bloquear", "ロック時にマイクをミュート", "锁屏时静音麦克风", "Отключать микрофон при блокировке"]),
        words(lang, ["Block cameras on lock", "Bloquer les caméras au verrouillage", "Kameras beim Sperren blockieren", "Bloquear cámaras al bloquear", "ロック時にカメラをブロック", "锁屏时阻止摄像头", "Блокировать камеры при блокировке"]),
        words(lang, ["Restore on unlock", "Restaurer au déverrouillage", "Beim Entsperren wiederherstellen", "Restaurar al desbloquear", "ロック解除時に復元", "解锁时恢复", "Восстанавливать при разблокировке"]),
        words(lang, ["Changes are saved immediately. Device controls are explicit and may request UAC.", "Modifications enregistrées immédiatement. Les commandes peuvent demander l’UAC.", "Änderungen werden sofort gespeichert. Gerätesteuerung kann UAC anfordern.", "Los cambios se guardan al instante. El control puede solicitar UAC.", "変更は即時保存されます。デバイス操作ではUAC確認が必要な場合があります。", "更改立即保存。设备控制可能请求 UAC。", "Изменения сохраняются сразу. Управление может запросить UAC."]),
    ]
}
impl Overview {
    pub(super) fn from_snapshot(snapshot: Result<&Snapshot, &str>, lang: Language) -> Self {
        let observed = Some(chrono::Utc::now());
        let Ok(snapshot) = snapshot else {
            return Self { observed, activity: [unknown(lang).into(), unknown(lang).into()], apps: unknown(lang).into(), coverage: format!("{}: {}\r\n{}", coverage_label(lang), unknown(lang), snapshot.unwrap_err()), };
        };
        let activity = [Resource::Microphone, Resource::Camera].map(|resource| {
            let count = snapshot.accesses.iter().filter(|a| a.resource == resource && a.activity == Activity::Active).count();
            let pipelines = snapshot.accesses.iter().filter(|a| a.resource == resource && a.activity == Activity::Ready).count();
            format!("{}: {count}; {}: {pipelines}", active(lang), ready(lang))
        });
        let mut apps = String::new();
        for access in snapshot.accesses.iter().take(8) {
            let name: String = access.application.chars().take(256).map(|c| if c.is_control() { ' ' } else { c }).collect();
            apps.push_str(&format!("{} — {} — {} — PID {}\r\n", lang.resource(access.resource), if access.activity == Activity::Active { active(lang) } else { ready(lang) }, name, access.pid.map_or_else(|| "?".into(), |p| p.to_string())));
        }
        if snapshot.accesses.is_empty() { apps.push_str(empty(lang)); }
        let total = snapshot.accesses.len();
        apps.push_str(&format!("\r\n{}: {} / {total}; {}: {}", words(lang, ["Shown", "Affichés", "Angezeigt", "Mostrados", "表示", "显示", "Показано"]), total.min(8), words(lang, ["Omitted", "Omis", "Ausgelassen", "Omitidos", "省略", "省略", "Пропущено"]), total.saturating_sub(8)));
        let mut coverage = format!("{}: {}\r\n", coverage_label(lang), if snapshot.collectors.is_empty() || !snapshot.observation_gaps.is_empty() || snapshot.collectors.iter().any(|c| c.state != CollectorState::Healthy) { unknown(lang) } else { words(lang, ["Current collectors responding; observation is not a guarantee", "Collecteurs disponibles ; aucune garantie", "Kollektoren antworten; keine Garantie", "Colectores responden; no es garantía", "収集元応答あり・保証ではありません", "收集器响应，不代表保证", "Источники отвечают; это не гарантия"]) });
        for collector in &snapshot.collectors {
            coverage.push_str(&format!("{}: {:?}{}\r\n", collector.collector, collector.state, collector.detail.as_deref().map_or(String::new(), |d| format!(" — {d}"))));
        }
        for resource in &snapshot.observation_gaps { coverage.push_str(&format!("{}: {} — {}\r\n", coverage_label(lang), lang.resource(*resource), unknown(lang))); }
        Self { observed, activity, apps, coverage }
    }
}

pub(super) struct View {
    lang: Language,
    observed: String,
    microphone: String,
    camera: String,
    apps: String,
    coverage: String,
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
        let microphone = state.microphone_protection.as_ref().map_or_else(|| lang.unknown_protection(true).to_owned(), |s| crate::output::microphone_protection_summary(lang, s));
        let camera = state.camera_observation.as_ref().map_or_else(|| lang.unknown_protection(false).to_owned(), |s| crate::output::camera_protection_summary(lang, s));
        let mut error = state.settings_error.clone().unwrap_or_default();
        for e in state.operation_errors.messages().chain(state.microphone_status_error.iter().map(String::as_str)).chain(state.camera_status_error.iter().map(String::as_str)) { error.push_str(e); error.push('\n'); }
        let mut diagnostics = state.summary.clone();
        if !error.is_empty() { diagnostics.push('\n'); diagnostics.push_str(&error); }
        Self {
            lang,
            observed: format!("{}: {}", words(lang, ["Observed at", "Observé à", "Beobachtet um", "Observado a las", "観測時刻", "观测时间", "Время наблюдения"]), state.overview.observed.map_or_else(|| unknown(lang).into(), |t| t.with_timezone(&chrono::Local).to_rfc3339())),
            microphone: format!("{} — {}\r\n{}", lang.resource(Resource::Microphone), if state.overview.observed.is_some() { state.overview.activity[0].as_str() } else { unknown(lang) }, microphone),
            camera: format!("{} — {}\r\n{}", lang.resource(Resource::Camera), if state.overview.observed.is_some() { state.overview.activity[1].as_str() } else { unknown(lang) }, camera),
            apps: if state.overview.observed.is_some() { state.overview.apps.clone() } else { unknown(lang).into() },
            coverage: if state.overview.observed.is_some() { state.overview.coverage.clone() } else { unknown(lang).into() },
            diagnostics, error,
            profile: PROFILE_CHOICES.iter().position(|(_, p)| *p == state.settings.profile).unwrap_or(0),
            explicit: state.explicit_policy,
            checks: [state.settings.notifications_enabled, state.settings.sound_enabled, state.settings.history_enabled, state.settings.notifications_paused(), state.autostart_state == crate::autostart::AutostartState::Enabled, state.settings.mute_on_lock, state.settings.block_camera_on_lock, state.settings.restore_on_unlock],
            mic_enabled: state.microphone_protection.is_some() && state.microphone_status_error.is_none() && state.pending_microphone == 0,
            cam_enabled: state.camera_observation.is_some() && state.camera_status_error.is_none() && state.pending_manual == 0 && state.pending_lock_block.is_none() && state.pending_lock_restore.is_none(),
            resume_enabled: state.camera_observation.as_ref().is_some_and(|c| c.desired_blocked && !c.helper_active) && state.pending_manual == 0,
            mic_action: state.microphone_protection.as_ref().map_or(lang.unknown_protection(true), |m| lang.protection_action(m.requested)).into(),
            cam_action: state.camera_observation.as_ref().map_or(lang.unknown_protection(false), |c| if c.desired_blocked { camera_allow_label(lang) } else { camera_block_label(lang) }).into(),
        }
    }
}

pub(super) struct DesktopWindow { hwnd: HWND }
struct WindowState {
    owner: HWND,
    font: Option<HFONT>,
    tab: usize,
    controls: Vec<(i32, HWND, usize)>,
    texts: HashMap<i32, String>,
    view: View,
}
impl DesktopWindow {
    pub(super) fn create(owner: HWND, view: View, icon: HICON) -> Result<Self> {
        let class = format_wide("MicCamWatchDesktopClass");
        let title = format_wide("MicCamWatch");
        unsafe {
            let _ = RegisterClassW(&WNDCLASSW { hbrBackground: GetSysColorBrush(COLOR_WINDOW), lpfnWndProc: Some(window_proc), lpszClassName: PCWSTR(class.as_ptr()), hIcon: icon, ..Default::default() });
        }
        let data = Box::into_raw(Box::new(WindowState { owner, font: None, tab: 0, controls: Vec::new(), texts: HashMap::new(), view }));
        let work = monitor_work_area(owner);
        let dpi = unsafe { GetDpiForWindow(owner) }.max(96) as i32;
        let width = (920 * dpi / 96).min(work.right - work.left);
        let height = (780 * dpi / 96).min(work.bottom - work.top);
        let hwnd = unsafe { CreateWindowExW(WINDOW_EX_STYLE(0), PCWSTR(class.as_ptr()), PCWSTR(title.as_ptr()), WS_OVERLAPPEDWINDOW, work.left + (work.right - work.left - width) / 2, work.top + (work.bottom - work.top - height) / 2, width, height, Some(owner), None, None, Some(data.cast())) };
        let hwnd = match hwnd { Ok(h) => h, Err(e) => { unsafe { drop(Box::from_raw(data)); } return Err(e.into()); } };
        let result = unsafe { &mut *data }.create_controls(hwnd);
        if let Err(e) = result { let _ = unsafe { DestroyWindow(hwnd) }; return Err(e); }
        Ok(Self { hwnd })
    }
    pub(super) fn show(&self) {
        unsafe { let _ = ShowWindow(self.hwnd, SW_RESTORE); let _ = SetForegroundWindow(self.hwnd); }
        if let Some(s) = window_state(self.hwnd) { s.render(self.hwnd); }
    }
    pub(super) fn update(&mut self, view: View) {
        if let Some(s) = window_state(self.hwnd) {
            s.view = view;
            if unsafe { IsWindowVisible(self.hwnd) } != 0 { s.render(self.hwnd); }
        }
    }
    pub(super) fn destroy(self) { let _ = unsafe { DestroyWindow(self.hwnd) }; }
}
pub(super) fn translate_message(message: &MSG) -> bool {
    if message.hwnd.is_invalid() { return false; }
    let root = unsafe { GetAncestor(message.hwnd, 2) };
    let class = format_wide("MicCamWatchDesktopClass");
    // FindWindow is class-scoped; never consume keyboard input for a foreign window.
    if unsafe { FindWindowW(PCWSTR(class.as_ptr()), PCWSTR::null()) }.ok() != Some(root) { return false; }
    unsafe { IsDialogMessageW(root, message) != 0 }
}
fn window_state(hwnd: HWND) -> Option<&'static mut WindowState> {
    unsafe { (GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState).as_mut() }
}
impl WindowState {
    fn add(&mut self, hwnd: HWND, id: i32, class: &str, caption: &str, style: WINDOW_STYLE, page: usize) -> Result<()> {
        let class = format_wide(class); let caption = format_wide(caption);
        let child = unsafe { CreateWindowExW(WINDOW_EX_STYLE(0), PCWSTR(class.as_ptr()), PCWSTR(caption.as_ptr()), WS_CHILD | style, 0, 0, 0, 0, Some(hwnd), Some(HMENU(id as usize as *mut _)), None, None)? };
        self.controls.push((id, child, page));
        if id == DIAGNOSTIC || id == APPS || id == COVERAGE || id == ERROR {
            unsafe { SendMessageW(child, EM_SETLIMITTEXT, Some(WPARAM(0x7fff_fffe)), None); }
        }
        Ok(())
    }
    fn create_controls(&mut self, hwnd: HWND) -> Result<()> {
        let lang = self.view.lang;
        self.add(hwnd, HEADER, "STATIC", &format!("MicCamWatch   v{}", env!("CARGO_PKG_VERSION")), WINDOW_STYLE(0), 4)?;
        let tabs = [words(lang, ["&Overview", "&Vue d’ensemble", "&Übersicht", "&Resumen", "概要(&O)", "概览(&O)", "&Обзор"]), words(lang, ["&Settings", "&Réglages", "&Einstellungen", "&Ajustes", "設定(&S)", "设置(&S)", "&Настройки"]), words(lang, ["&Diagnostics", "&Diagnostic", "&Diagnose", "&Diagnóstico", "診断(&D)", "诊断(&D)", "&Диагностика"]), words(lang, ["&About", "À &propos", "&Info", "&Acerca de", "情報(&A)", "关于(&A)", "О &программе"])];
        for (index, text) in tabs.iter().enumerate() { self.add(hwnd, TAB + index as i32, "BUTTON", text, WS_TABSTOP, 4)?; }
        for id in [OBSERVED, MICROPHONE, CAMERA] { self.add(hwnd, id, "STATIC", "", WINDOW_STYLE(0x80), 0)?; }
        let read = WS_TABSTOP | WS_VSCROLL | WINDOW_STYLE((ES_MULTILINE | ES_READONLY | ES_AUTOVSCROLL) as u32);
        for id in [APPS, COVERAGE] { self.add(hwnd, id, "EDIT", "", read, 0)?; }
        for (id, text) in [(MIC_ACTION, ""), (CAM_ACTION, ""), (CAM_RESUME, camera_resume_label(lang))] { self.add(hwnd, id, "BUTTON", text, WS_TABSTOP, 0)?; }
        for (index, text) in profile_words(lang).1.iter().enumerate() { self.add(hwnd, PROFILE + index as i32, "BUTTON", text, WS_TABSTOP | WINDOW_STYLE(4), 1)?; }
        let labels = check_labels(lang);
        for (index, id) in [NOTIFICATIONS, SOUND, HISTORY, PAUSE, AUTOSTART, MUTE_LOCK, CAMERA_LOCK, RESTORE_UNLOCK].iter().enumerate() { self.add(hwnd, *id as i32, "BUTTON", labels[index], WS_TABSTOP | WINDOW_STYLE(3), 1)?; }
        self.add(hwnd, POLICY, "STATIC", "", WINDOW_STYLE(0x80), 1)?;
        self.add(hwnd, ERROR, "EDIT", "", read, 1)?;
        self.add(hwnd, DIAGNOSTIC, "EDIT", "", read, 2)?;
        self.add(hwnd, COPY, "BUTTON", words(lang, ["Copy &all", "Tout &copier", "Alles &kopieren", "&Copiar todo", "すべてコピー(&C)", "全部复制(&C)", "&Копировать всё"]), WS_TABSTOP, 2)?;
        self.add(hwnd, ABOUT, "STATIC", &format!("MicCamWatch v{}\r\n\r\n{}\r\n\r\n{}", env!("CARGO_PKG_VERSION"), words(lang, ["Local microphone and camera activity observation and explicit privacy controls.", "Observation locale du micro et des caméras, commandes explicites de confidentialité.", "Lokale Beobachtung von Mikrofon und Kamera mit expliziter Datenschutzsteuerung.", "Observación local de micrófono y cámaras y controles explícitos de privacidad.", "マイクとカメラのローカル観測と明示的なプライバシー操作。", "本地麦克风与摄像头活动观测及明确的隐私控制。", "Локальное наблюдение микрофона и камер и явное управление приватностью."]), words(lang, ["No automatic uploads or telemetry. Closing this window leaves the tray running. Unknown observations never mean safe or protected.", "Aucun envoi automatique ni télémétrie. Fermer la fenêtre laisse le tray actif. Inconnu ne signifie pas protégé.", "Keine automatischen Uploads oder Telemetrie. Schließen lässt das Tray aktiv. Unbekannt bedeutet nicht geschützt.", "Sin envíos automáticos ni telemetría. Cerrar conserva la bandeja. Desconocido no significa protegido.", "自動アップロード・テレメトリなし。閉じてもトレイは動作します。不明は安全や保護済みを意味しません。", "无自动上传或遥测。关闭窗口后托盘继续运行。未知不代表安全或已保护。", "Нет автоматической отправки или телеметрии. Закрытие оставляет трей. Неизвестно не означает защищено."])), WINDOW_STYLE(0x80), 3)?;
        self.font(hwnd); self.render(hwnd); self.layout(hwnd);
        Ok(())
    }
    fn font(&mut self, hwnd: HWND) {
        let Some(font) = native_menu_font(hwnd) else { return; };
        for (_, child, _) in &self.controls { unsafe { SendMessageW(*child, WM_SETFONT, Some(WPARAM(font.0 as usize)), Some(LPARAM(1))); } }
        if let Some(old) = self.font.replace(font) { let _ = unsafe { DeleteObject(old.into()) }; }
    }
    fn text(&mut self, id: i32, value: String) {
        if self.texts.get(&id) == Some(&value) { return; }
        if let Some((_, child, _)) = self.controls.iter().find(|(i, _, _)| *i == id) {
            // Keep native EDIT selection and scroll on accepted live updates.
            let mut start = 0u32; let mut end = 0u32;
            let first = unsafe { SendMessageW(*child, EM_GETFIRSTVISIBLELINE, None, None) }.0;
            unsafe { SendMessageW(*child, EM_GETSEL, Some(WPARAM((&mut start as *mut u32) as usize)), Some(LPARAM((&mut end as *mut u32) as isize))); }
            let text = format_wide(&diagnostic_document(&value));
            let _ = unsafe { SetWindowTextW(*child, PCWSTR(text.as_ptr())) };
            unsafe { SendMessageW(*child, EM_SETSEL, Some(WPARAM(start as usize)), Some(LPARAM(end as isize))); let now = SendMessageW(*child, EM_GETFIRSTVISIBLELINE, None, None).0; SendMessageW(*child, EM_LINESCROLL, None, Some(LPARAM(first - now))); }
        }
        self.texts.insert(id, value);
    }
    fn render(&mut self, hwnd: HWND) {
        self.text(OBSERVED, self.view.observed.clone());
        self.text(MICROPHONE, self.view.microphone.clone()); self.text(CAMERA, self.view.camera.clone());
        self.text(APPS, self.view.apps.clone()); self.text(COVERAGE, self.view.coverage.clone());
        self.text(DIAGNOSTIC, self.view.diagnostics.clone()); self.text(ERROR, self.view.error.clone());
        self.text(MIC_ACTION, self.view.mic_action.clone()); self.text(CAM_ACTION, self.view.cam_action.clone());
        let policy = if self.view.explicit { profile_words(self.view.lang).2 } else { check_labels(self.view.lang)[8] };
        self.text(POLICY, policy.into());
        for (index, id) in [NOTIFICATIONS, SOUND, HISTORY, PAUSE, AUTOSTART, MUTE_LOCK, CAMERA_LOCK, RESTORE_UNLOCK].iter().enumerate() {
            if let Ok(child) = unsafe { GetDlgItem(Some(hwnd), *id as i32) } { unsafe { SendMessageW(child, BM_SETCHECK, Some(WPARAM(usize::from(self.view.checks[index]))), None); } }
        }
        for index in 0..4 { if let Ok(child) = unsafe { GetDlgItem(Some(hwnd), PROFILE + index) } { unsafe { SendMessageW(child, BM_SETCHECK, Some(WPARAM(usize::from(self.view.profile == index as usize && !self.view.explicit))), None); EnableWindow(child, i32::from(!self.view.explicit)); } } }
        for (id, enabled) in [(MIC_ACTION, self.view.mic_enabled), (CAM_ACTION, self.view.cam_enabled), (CAM_RESUME, self.view.resume_enabled)] { if let Ok(child) = unsafe { GetDlgItem(Some(hwnd), id) } { unsafe { EnableWindow(child, i32::from(enabled)); } } }
    }
    fn layout(&self, hwnd: HWND) {
        let mut r = RECT::default(); let _ = unsafe { GetClientRect(hwnd, &mut r) };
        let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96) as i32;
        let u = |n: i32| n * dpi / 96;
        let width = (r.right / (dpi.max(1)) * 96).max(1);
        let height = r.bottom * 96 / dpi;
        for (id, child, page) in &self.controls {
            let visible = *page == 4 || *page == self.tab;
            unsafe { let _ = ShowWindow(*child, if visible { SW_SHOW } else { SW_HIDE }); }
            if !visible { continue; }
            let (x,y,w,h) = match *id {
                HEADER => (20, 14, width - 40, 28),
                i if (TAB..TAB+4).contains(&i) => (20 + (i-TAB)*(width-40)/4, 50, (width-40)/4 - 4, 34),
                OBSERVED => (20, 100, width-40, 24),
                MICROPHONE => (20, 130, width-40, 76), CAMERA => (20, 210, width-40, 76),
                MIC_ACTION => (20, 290, (width-44)/2, 36), CAM_ACTION => (24+(width-44)/2, 290, (width-44)/2, 36),
                CAM_RESUME => (20, 332, width-40, 32),
                APPS => (20, 374, width-40, ((height-398)*3/5).max(44)),
                COVERAGE => (20, 384+((height-398)*3/5).max(44), width-40, ((height-398)*2/5-10).max(32)),
                i if (PROFILE..PROFILE+4).contains(&i) => (20+(i-PROFILE)*(width-40)/4, 104, (width-40)/4-2, 32),
                POLICY => (20, 142, width-40, 52),
                i if (NOTIFICATIONS as i32..=AUTOSTART as i32).contains(&i) => (20, 202+(i-NOTIFICATIONS as i32)*34, width-40, 30),
                i if (MUTE_LOCK as i32..=RESTORE_UNLOCK as i32).contains(&i) => (20, 372+(i-MUTE_LOCK as i32)*34, width-40, 30),
                ERROR => (20, 484, width-40, (height-504).max(36)),
                DIAGNOSTIC => (20, 104, width-40, (height-170).max(40)), COPY => (20, height-52, 190, 34),
                ABOUT => (20, 110, width-40, (height-130).max(60)), _ => continue,
            };
            let _ = unsafe { MoveWindow(*child, u(x), u(y), u(w.max(1)), u(h.max(1)), true) };
        }
    }
}
unsafe extern "system" fn window_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if message == WM_NCCREATE { let c = unsafe { &*(lparam.0 as *const CREATESTRUCTW) }; unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, c.lpCreateParams as isize); } return LRESULT(1); }
    if message == WM_NCDESTROY {
        let p = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut WindowState;
        unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0); }
        if !p.is_null() { let mut s = unsafe { Box::from_raw(p) }; if let Some(f) = s.font.take() { let _ = unsafe { DeleteObject(f.into()) }; } }
        return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
    }
    let Some(s) = window_state(hwnd) else { return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }; };
    match message {
        WM_CLOSE => { unsafe { let _ = ShowWindow(hwnd, SW_HIDE); } LRESULT(0) }
        WM_SIZE => { s.layout(hwnd); LRESULT(0) }
        WM_DPICHANGED => {
            let r = unsafe { &*(lparam.0 as *const RECT) };
            let _ = unsafe { MoveWindow(hwnd, r.left, r.top, r.right-r.left, r.bottom-r.top, true) };
            s.font(hwnd); s.layout(hwnd); LRESULT(0)
        }
        WM_SETTINGCHANGE | WM_THEMECHANGED => { s.font(hwnd); s.layout(hwnd); LRESULT(0) }
        WM_GETMINMAXINFO => {
            // MINMAXINFO contains five POINTs; enforce usable keyboard/DPI layout.
            let points = unsafe { &mut *(lparam.0 as *mut [POINT; 5]) };
            let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96) as i32;
            points[3] = POINT { x: 640*dpi/96, y: 680*dpi/96 }; LRESULT(0)
        }
        WM_COMMAND => {
            let id = (wparam.0 & 0xffff) as i32;
            if (TAB..TAB+4).contains(&id) {
                s.tab = (id-TAB) as usize; s.layout(hwnd);
                // Keep keyboard focus on the selected navigation control.
                if let Ok(c) = unsafe { GetDlgItem(Some(hwnd), id) } { unsafe { SetFocus(c); } }
            } else if (PROFILE..PROFILE+4).contains(&id) && !s.view.explicit {
                let _ = unsafe { PostMessageW(Some(s.owner), WM_COMMAND, WPARAM(PROFILE_CHOICES[(id-PROFILE) as usize].0), LPARAM(0)) };
            } else if let Some(index) = [NOTIFICATIONS, SOUND, HISTORY, PAUSE, AUTOSTART, MUTE_LOCK, CAMERA_LOCK, RESTORE_UNLOCK].iter().position(|i| *i == id as usize) {
                // BS_AUTOCHECKBOX already contains the desired value. Never invert
                // a stale frame or issue toggles during refresh.
                if let Ok(c) = unsafe { GetDlgItem(Some(hwnd), id) } {
                    let desired = unsafe { SendMessageW(c, BM_GETCHECK, None, None) }.0 == 1;
                    let _ = index;
                    let _ = unsafe { PostMessageW(Some(s.owner), WM_DESKTOP_SETTING, WPARAM(id as usize), LPARAM(isize::from(desired))) };
                }
            } else if id == COPY {
                if let Ok(c) = unsafe { GetDlgItem(Some(hwnd), DIAGNOSTIC) } { unsafe { SendMessageW(c, EM_SETSEL, Some(WPARAM(0)), Some(LPARAM(-1))); SendMessageW(c, WM_COPY, None, None); } }
            } else {
                let action = match id { MIC_ACTION if s.view.mic_enabled => Some(CMD_TOGGLE_MUTE), CAM_ACTION if s.view.cam_enabled => Some(CMD_TOGGLE_CAMERA), CAM_RESUME if s.view.resume_enabled => Some(CMD_RESUME_CAMERA), _ => None };
                if let Some(command) = action { let _ = unsafe { PostMessageW(Some(s.owner), WM_COMMAND, WPARAM(command), LPARAM(0)) }; }
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}
