use crate::{
    cli::Filter,
    i18n::Language,
    model::{
        Access, AccessEvent, Action, Activity, CollectorState, MicrophoneMuteState, Resource,
        SCHEMA_VERSION, event_code,
    },
    platform::{PlatformMonitor, SessionLockState},
    privacy::CameraPrivacyState,
    settings::{PrivacyProfile, Settings},
};
use anyhow::{Context, Result};
use std::{collections::HashMap, ffi::OsStr, mem::size_of, os::windows::ffi::OsStrExt};
use windows::{
    Win32::{
        Foundation::{
            CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, HWND, LPARAM, LRESULT, POINT,
            WPARAM,
        },
        Graphics::Gdi::{
            BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateBitmap, CreateDIBSection, DIB_RGB_COLORS,
            DeleteObject,
        },
        System::Threading::CreateMutexW,
        UI::{
            Shell::{
                NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
                Shell_NotifyIconW,
            },
            WindowsAndMessaging::{
                AppendMenuW, CREATESTRUCTW, CreateIconIndirect, CreatePopupMenu, CreateWindowExW,
                DefWindowProcW, DestroyIcon, DestroyMenu, DestroyWindow, DispatchMessageW,
                FindWindowW, GWLP_USERDATA, GetCursorPos, GetMessageW, GetWindowLongPtrW, HICON,
                ICONINFO, KillTimer, MF_DISABLED, MF_GRAYED, MF_SEPARATOR, MF_STRING, MSG,
                PostMessageW, PostQuitMessage, RegisterClassW, SetForegroundWindow, SetTimer,
                SetWindowLongPtrW, TPM_BOTTOMALIGN, TPM_RIGHTBUTTON, TrackPopupMenu,
                TranslateMessage, WINDOW_EX_STYLE, WM_APP, WM_CLOSE, WM_COMMAND, WM_DESTROY,
                WM_LBUTTONDBLCLK, WM_NCCREATE, WM_RBUTTONUP, WM_TIMER, WNDCLASSW, WS_OVERLAPPED,
            },
        },
    },
    core::PCWSTR,
};

const WM_TRAY_CALLBACK: u32 = WM_APP + 1;
const TIMER_POLL_ID: usize = 1;
const CMD_TOGGLE_MUTE: usize = 101;
const CMD_TOGGLE_CAMERA: usize = 102;
const CMD_EXIT: usize = 103;
const CMD_TOGGLE_NOTIFICATIONS: usize = 104;
const CMD_TOGGLE_AUTOSTART: usize = 105;
const CMD_CYCLE_PROFILE: usize = 106;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TrayVisual {
    Idle,
    Ready,
    Active,
    Error,
}

#[derive(Clone, Copy)]
enum IconGlyph {
    Check,
    Ready,
    Active,
    Error,
}

struct MutexGuard(HANDLE);

impl Drop for MutexGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

struct TrayAppState {
    monitor: PlatformMonitor,
    lang: Language,
    visual: TrayVisual,
    summary: String,
    mute_state: MicrophoneMuteState,
    camera_state: CameraPrivacyState,
    autostart_state: crate::autostart::AutostartState,
    settings: Settings,
    lock_state: SessionLockState,
    restore_mute: Option<bool>,
    restore_camera: Option<CameraPrivacyState>,
    previous_accesses: HashMap<String, Access>,
    green_icon: HICON,
    yellow_icon: HICON,
    red_icon: HICON,
    gray_icon: HICON,
}

impl Drop for TrayAppState {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyIcon(self.green_icon);
            let _ = DestroyIcon(self.yellow_icon);
            let _ = DestroyIcon(self.red_icon);
            let _ = DestroyIcon(self.gray_icon);
        }
    }
}

pub fn run_tray(monitor: PlatformMonitor, lang: Language, settings: Settings) -> Result<()> {
    let mutex_name = format_wide("Local\\MicCamWatch.Tray");
    let _mutex = MutexGuard(unsafe { CreateMutexW(None, true, PCWSTR(mutex_name.as_ptr()))? });
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        anyhow::bail!("MicCamWatch tray is already running");
    }

    let state = Box::new(TrayAppState {
        mute_state: monitor
            .microphone_mute_state()
            .unwrap_or(MicrophoneMuteState::Unavailable),
        camera_state: crate::privacy::camera_state().unwrap_or(CameraPrivacyState::SystemManaged),
        autostart_state: crate::autostart::state()
            .unwrap_or(crate::autostart::AutostartState::Disabled),
        monitor,
        lang,
        visual: TrayVisual::Idle,
        summary: idle_text(lang).to_owned(),
        settings,
        lock_state: crate::platform::session_lock_state(),
        restore_mute: None,
        restore_camera: None,
        previous_accesses: HashMap::new(),
        green_icon: create_status_icon((34, 197, 94), IconGlyph::Check)?,
        yellow_icon: create_status_icon((245, 158, 11), IconGlyph::Ready)?,
        red_icon: create_status_icon((239, 68, 68), IconGlyph::Active)?,
        gray_icon: create_status_icon((107, 114, 128), IconGlyph::Error)?,
    });
    let state_ptr = Box::into_raw(state);

    let class_name = format_wide("MicCamWatchTrayClass");
    let wc = WNDCLASSW {
        lpfnWndProc: Some(tray_wnd_proc),
        lpszClassName: PCWSTR(class_name.as_ptr()),
        ..Default::default()
    };
    if unsafe { RegisterClassW(&wc) } == 0 {
        unsafe { drop(Box::from_raw(state_ptr)) };
        anyhow::bail!("failed to register tray window class");
    }

    let window_title = format_wide("MicCamWatchTray");
    let hwnd = match unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(class_name.as_ptr()),
            PCWSTR(window_title.as_ptr()),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            None,
            None,
            None,
            Some(state_ptr.cast()),
        )
    } {
        Ok(hwnd) => hwnd,
        Err(error) => {
            unsafe { drop(Box::from_raw(state_ptr)) };
            return Err(error).context("failed to create tray message window");
        }
    };

    let state = unsafe { &*state_ptr };
    let nid = notify_icon_data(hwnd, state.green_icon, &state.summary);
    if !unsafe { Shell_NotifyIconW(NIM_ADD, &nid) }.as_bool() {
        unsafe {
            let _ = DestroyWindow(hwnd);
            drop(Box::from_raw(state_ptr));
        }
        anyhow::bail!("failed to register system tray icon");
    }
    let timer = unsafe { SetTimer(Some(hwnd), TIMER_POLL_ID, 500, None) };
    if timer == 0 {
        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
            let _ = DestroyWindow(hwnd);
            drop(Box::from_raw(state_ptr));
        }
        anyhow::bail!("failed to start tray polling timer");
    }

    println!("miccamwatch tray running. Right-click the icon near the clock to control.");
    let mut message = MSG::default();
    let loop_result = loop {
        let result = unsafe { GetMessageW(&mut message, None, 0, 0) };
        if result.0 == -1 {
            break Err(anyhow::anyhow!("Windows tray message loop failed"));
        }
        if result.0 == 0 {
            break Ok(());
        }
        unsafe {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    };

    unsafe {
        let _ = KillTimer(Some(hwnd), TIMER_POLL_ID);
        let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        drop(Box::from_raw(state_ptr));
    }
    loop_result
}

pub fn is_running() -> bool {
    let class_name = format_wide("MicCamWatchTrayClass");
    unsafe { FindWindowW(PCWSTR(class_name.as_ptr()), PCWSTR::null()) }.is_ok()
}

pub fn stop_running() -> Result<bool> {
    let class_name = format_wide("MicCamWatchTrayClass");
    let Ok(hwnd) = (unsafe { FindWindowW(PCWSTR(class_name.as_ptr()), PCWSTR::null()) }) else {
        return Ok(false);
    };
    unsafe { PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0))? };
    Ok(true)
}

fn state(hwnd: HWND) -> Option<&'static mut TrayAppState> {
    let pointer = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut TrayAppState;
    unsafe { pointer.as_mut() }
}

unsafe extern "system" fn tray_wnd_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_NCCREATE {
        let create = unsafe { &*(lparam.0 as *const CREATESTRUCTW) };
        unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize) };
        return LRESULT(1);
    }
    match message {
        WM_TIMER => {
            if let Some(state) = state(hwnd) {
                refresh_state(hwnd, state);
            }
            LRESULT(0)
        }
        WM_TRAY_CALLBACK => {
            match lparam.0 as u32 {
                WM_RBUTTONUP => show_context_menu(hwnd),
                WM_LBUTTONDBLCLK => show_status_toast(hwnd),
                _ => {}
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            match wparam.0 & 0xffff {
                CMD_TOGGLE_MUTE => toggle_mute(hwnd),
                CMD_TOGGLE_CAMERA => toggle_camera(hwnd),
                CMD_TOGGLE_NOTIFICATIONS => toggle_notifications(hwnd),
                CMD_TOGGLE_AUTOSTART => toggle_autostart(hwnd),
                CMD_CYCLE_PROFILE => cycle_profile(hwnd),
                CMD_EXIT => unsafe {
                    let _ = DestroyWindow(hwnd);
                },
                _ => {}
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

fn apply_lock_policy(state: &mut TrayAppState) {
    let current = crate::platform::session_lock_state();
    if current == state.lock_state {
        return;
    }
    if current == SessionLockState::Locked {
        if state.settings.mute_on_lock {
            let was_muted = state.mute_state == MicrophoneMuteState::Muted;
            if state.monitor.set_microphone_mute(true).is_ok() {
                state.restore_mute = Some(was_muted);
            }
        }
        if state.settings.block_camera_on_lock {
            let previous = crate::privacy::camera_state().ok();
            if crate::privacy::set_camera_state(CameraPrivacyState::Blocked).is_ok() {
                state.restore_camera = previous;
            }
        }
    } else if current == SessionLockState::Unlocked && state.settings.restore_on_unlock {
        if let Some(was_muted) = state.restore_mute.take() {
            let _ = state.monitor.set_microphone_mute(was_muted);
        }
        if let Some(previous) = state.restore_camera.take()
            && previous != CameraPrivacyState::SystemManaged
        {
            let _ = crate::privacy::set_camera_state(previous);
        }
    }
    state.lock_state = current;
}

fn refresh_state(hwnd: HWND, state: &mut TrayAppState) {
    apply_lock_policy(state);
    let filter = Filter {
        include_ready: true,
        ..Filter::default()
    };
    let (visual, summary) = match state.monitor.snapshot(&filter) {
        Ok(snapshot) => {
            record_history_changes(state, &snapshot.accesses);
            let unhealthy = snapshot
                .collectors
                .iter()
                .any(|collector| collector.state != CollectorState::Healthy);
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
            if unhealthy {
                (
                    TrayVisual::Error,
                    "miccamwatch: telemetry degraded".to_owned(),
                )
            } else if !active.is_empty() {
                (TrayVisual::Active, summarize(&active))
            } else if !ready.is_empty() {
                (
                    TrayVisual::Ready,
                    format!(
                        "miccamwatch: camera-ready pipeline: {}",
                        ready[0].application
                    ),
                )
            } else {
                (TrayVisual::Idle, idle_text(state.lang).to_owned())
            }
        }
        Err(error) => (
            TrayVisual::Error,
            format!("miccamwatch: telemetry error: {error}"),
        ),
    };
    let mute_state = state
        .monitor
        .microphone_mute_state()
        .unwrap_or(MicrophoneMuteState::Unavailable);
    let camera_state = crate::privacy::camera_state().unwrap_or(CameraPrivacyState::SystemManaged);
    if visual == state.visual
        && summary == state.summary
        && mute_state == state.mute_state
        && camera_state == state.camera_state
    {
        return;
    }
    state.visual = visual;
    state.summary = summary;
    state.mute_state = mute_state;
    state.camera_state = camera_state;
    let icon = match visual {
        TrayVisual::Idle => state.green_icon,
        TrayVisual::Ready => state.yellow_icon,
        TrayVisual::Active => state.red_icon,
        TrayVisual::Error => state.gray_icon,
    };
    let nid = notify_icon_data(hwnd, icon, &state.summary);
    if !unsafe { Shell_NotifyIconW(NIM_MODIFY, &nid) }.as_bool() {
        state.visual = TrayVisual::Error;
        state.summary = "miccamwatch: failed to update tray icon".to_owned();
    }
}

fn record_history_changes(state: &mut TrayAppState, accesses: &[Access]) {
    if !state.settings.history_enabled {
        return;
    }
    let current = accesses
        .iter()
        .map(|access| (access.key.clone(), access.clone()))
        .collect::<HashMap<_, _>>();
    for (key, access) in &current {
        if !state.previous_accesses.contains_key(key) {
            append_history_event(access, Action::Start);
        }
    }
    for (key, access) in &state.previous_accesses {
        if !current.contains_key(key) {
            append_history_event(access, Action::Stop);
        }
    }
    state.previous_accesses = current;
}

fn append_history_event(access: &Access, action: Action) {
    let event = AccessEvent {
        schema_version: SCHEMA_VERSION,
        event_code: event_code(action),
        tool_version: env!("CARGO_PKG_VERSION"),
        action,
        observed_at: chrono::Utc::now(),
        access: access.clone(),
    };
    if let Err(error) = crate::history::append(&event) {
        eprintln!("history append failed: {error}");
    }
}

fn summarize(accesses: &[&crate::model::Access]) -> String {
    let items = accesses
        .iter()
        .map(|access| {
            let resource = match access.resource {
                Resource::Microphone => "Mic",
                Resource::Camera => "Cam",
            };
            format!("{}: {resource}", access.application)
        })
        .collect::<Vec<_>>();
    format!("miccamwatch: {}", items.join(", "))
}

fn toggle_mute(hwnd: HWND) {
    let Some(state) = state(hwnd) else { return };
    match state.monitor.toggle_microphone_mute() {
        Ok(muted) => {
            state.mute_state = if muted {
                MicrophoneMuteState::Muted
            } else {
                MicrophoneMuteState::Unmuted
            };
            let message = if muted {
                "Microphone muted"
            } else {
                "Microphone unmuted"
            };
            if let Err(error) = crate::notify::notify_message("miccamwatch", message) {
                eprintln!("notification failed: {error}");
            }
        }
        Err(error) => {
            state.summary = format!("miccamwatch: mute failed: {error}");
            state.visual = TrayVisual::Error;
        }
    }
}

fn toggle_camera(hwnd: HWND) {
    let Some(state) = state(hwnd) else { return };
    match crate::privacy::toggle_camera() {
        Ok(camera_state) => state.camera_state = camera_state,
        Err(error) => state.summary = format!("miccamwatch: camera privacy failed: {error}"),
    }
}

fn toggle_notifications(hwnd: HWND) {
    let Some(state) = state(hwnd) else { return };
    state.settings.pause_notifications_until = if state.settings.notifications_paused() {
        None
    } else {
        Some(chrono::Utc::now() + chrono::Duration::hours(1))
    };
    if let Err(error) = state.settings.save() {
        state.summary = format!("miccamwatch: settings save failed: {error}");
    }
}

fn toggle_autostart(hwnd: HWND) {
    let Some(state) = state(hwnd) else { return };
    let result = match state.autostart_state {
        crate::autostart::AutostartState::Enabled => crate::autostart::disable(),
        crate::autostart::AutostartState::Disabled => crate::autostart::enable(),
    };
    if let Err(error) = result {
        state.summary = format!("miccamwatch: autostart failed: {error}");
    }
    state.autostart_state =
        crate::autostart::state().unwrap_or(crate::autostart::AutostartState::Disabled);
}

fn cycle_profile(hwnd: HWND) {
    let Some(state) = state(hwnd) else { return };
    state.settings.profile = match state.settings.profile {
        PrivacyProfile::Balanced => PrivacyProfile::Private,
        PrivacyProfile::Private => PrivacyProfile::Meeting,
        PrivacyProfile::Meeting => PrivacyProfile::Development,
        PrivacyProfile::Development => PrivacyProfile::Balanced,
    };
    if let Err(error) = state.settings.save() {
        state.summary = format!("miccamwatch: settings save failed: {error}");
    }
}

fn show_context_menu(hwnd: HWND) {
    let Some(state) = state(hwnd) else { return };
    unsafe {
        let Ok(menu) = CreatePopupMenu() else { return };
        append_disabled(menu, &format!("miccamwatch v{}", env!("CARGO_PKG_VERSION")));
        append_disabled(menu, &state.summary);
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let mute_text = match state.mute_state {
            MicrophoneMuteState::Muted => "Unmute microphone",
            MicrophoneMuteState::Unmuted | MicrophoneMuteState::Mixed => "Mute microphone",
            MicrophoneMuteState::Unavailable => "Microphone unavailable",
        };
        let mute_wide = format_wide(mute_text);
        let mute_flags = if state.mute_state == MicrophoneMuteState::Unavailable {
            MF_STRING | MF_DISABLED | MF_GRAYED
        } else {
            MF_STRING
        };
        let _ = AppendMenuW(
            menu,
            mute_flags,
            CMD_TOGGLE_MUTE,
            PCWSTR(mute_wide.as_ptr()),
        );
        let camera_text = format_wide(match state.camera_state {
            CameraPrivacyState::Allowed => "Block camera",
            CameraPrivacyState::Blocked => "Allow camera",
            CameraPrivacyState::SystemManaged => "Allow camera (system managed)",
        });
        let _ = AppendMenuW(
            menu,
            MF_STRING,
            CMD_TOGGLE_CAMERA,
            PCWSTR(camera_text.as_ptr()),
        );
        let notifications = format_wide(if state.settings.notifications_paused() {
            "Resume notifications"
        } else {
            "Pause notifications for 1 hour"
        });
        let _ = AppendMenuW(
            menu,
            MF_STRING,
            CMD_TOGGLE_NOTIFICATIONS,
            PCWSTR(notifications.as_ptr()),
        );
        let profile = format_wide(&format!("Profile: {:?} (change)", state.settings.profile));
        let _ = AppendMenuW(menu, MF_STRING, CMD_CYCLE_PROFILE, PCWSTR(profile.as_ptr()));
        let autostart = format_wide(match state.autostart_state {
            crate::autostart::AutostartState::Enabled => "Disable autostart",
            crate::autostart::AutostartState::Disabled => "Enable autostart",
        });
        let _ = AppendMenuW(
            menu,
            MF_STRING,
            CMD_TOGGLE_AUTOSTART,
            PCWSTR(autostart.as_ptr()),
        );
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let exit = format_wide("Exit");
        let _ = AppendMenuW(menu, MF_STRING, CMD_EXIT, PCWSTR(exit.as_ptr()));
        let mut point = POINT::default();
        let _ = GetCursorPos(&mut point);
        let _ = SetForegroundWindow(hwnd);
        let _ = TrackPopupMenu(
            menu,
            TPM_RIGHTBUTTON | TPM_BOTTOMALIGN,
            point.x,
            point.y,
            None,
            hwnd,
            None,
        );
        let _ = DestroyMenu(menu);
    }
}

fn append_disabled(menu: windows::Win32::UI::WindowsAndMessaging::HMENU, text: &str) {
    let wide = format_wide(text);
    let _ = unsafe {
        AppendMenuW(
            menu,
            MF_STRING | MF_DISABLED | MF_GRAYED,
            0,
            PCWSTR(wide.as_ptr()),
        )
    };
}

fn show_status_toast(hwnd: HWND) {
    let Some(state) = state(hwnd) else { return };
    if let Err(error) = crate::notify::notify_message("miccamwatch", &state.summary) {
        eprintln!("notification failed: {error}");
    }
}

fn idle_text(lang: Language) -> &'static str {
    match lang {
        Language::Fr => "miccamwatch : aucun accès actif",
        Language::De => "miccamwatch: keine aktiven Zugriffe",
        Language::Es => "miccamwatch: sin accesos activos",
        Language::Ja => "miccamwatch: アクティブなアクセスなし",
        Language::Zh => "miccamwatch: 无活动访问",
        Language::Ru => "miccamwatch: нет активных доступов",
        Language::En => "miccamwatch: idle",
    }
}

fn notify_icon_data(hwnd: HWND, icon: HICON, tooltip: &str) -> NOTIFYICONDATAW {
    let mut data = NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: 1,
        uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
        uCallbackMessage: WM_TRAY_CALLBACK,
        hIcon: icon,
        ..Default::default()
    };
    let wide: Vec<u16> = OsStr::new(tooltip).encode_wide().collect();
    let length = wide.len().min(data.szTip.len().saturating_sub(1));
    data.szTip[..length].copy_from_slice(&wide[..length]);
    data.szTip[length] = 0;
    data
}

fn format_wide(value: &str) -> Vec<u16> {
    OsStr::new(value).encode_wide().chain(Some(0)).collect()
}

fn create_status_icon(color: (u8, u8, u8), glyph: IconGlyph) -> Result<HICON> {
    const SIZE: i32 = 32;
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: SIZE,
            biHeight: -SIZE,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits = std::ptr::null_mut();
    let color_bitmap =
        unsafe { CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0)? };
    if bits.is_null() {
        let _ = unsafe { DeleteObject(color_bitmap.into()) };
        anyhow::bail!("Windows returned no pixel buffer for tray icon");
    }
    let rendered = render_icon_pixels(color, glyph);
    let pixels =
        unsafe { std::slice::from_raw_parts_mut(bits.cast::<u32>(), (SIZE * SIZE) as usize) };
    pixels.copy_from_slice(&rendered);
    let mask = [0u8; 128];
    let mask_bitmap = unsafe { CreateBitmap(SIZE, SIZE, 1, 1, Some(mask.as_ptr().cast())) };
    let icon_info = ICONINFO {
        fIcon: true.into(),
        hbmMask: mask_bitmap,
        hbmColor: color_bitmap,
        ..Default::default()
    };
    let icon = unsafe { CreateIconIndirect(&icon_info) };
    let _ = unsafe { DeleteObject(color_bitmap.into()) };
    let _ = unsafe { DeleteObject(mask_bitmap.into()) };
    icon.context("failed to create alpha tray icon")
}

fn render_icon_pixels(color: (u8, u8, u8), glyph: IconGlyph) -> Vec<u32> {
    const SIZE: i32 = 32;
    let mut pixels = vec![0; (SIZE * SIZE) as usize];
    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as f32 - 15.5;
            let dy = y as f32 - 15.5;
            let distance = (dx * dx + dy * dy).sqrt();
            let alpha = ((15.0 - distance).clamp(0.0, 1.0) * 255.0) as u32;
            if alpha == 0 {
                continue;
            }
            let white = glyph_pixel(glyph, x, y);
            let (red, green, blue) = if white { (255, 255, 255) } else { color };
            pixels[(y * SIZE + x) as usize] = (alpha << 24)
                | ((red as u32 * alpha / 255) << 16)
                | ((green as u32 * alpha / 255) << 8)
                | (blue as u32 * alpha / 255);
        }
    }
    pixels
}

fn glyph_pixel(glyph: IconGlyph, x: i32, y: i32) -> bool {
    match glyph {
        IconGlyph::Check => {
            line_distance(x, y, 8, 16, 13, 21) <= 1.7 || line_distance(x, y, 13, 21, 24, 10) <= 1.7
        }
        IconGlyph::Ready => {
            (9..=22).contains(&y) && ((10..=12).contains(&x) || (19..=21).contains(&x))
        }
        IconGlyph::Active => {
            ((13..=18).contains(&x) && (7..=19).contains(&y))
                || ((13..=18).contains(&x) && (23..=27).contains(&y))
        }
        IconGlyph::Error => {
            line_distance(x, y, 9, 9, 22, 22) <= 1.8 || line_distance(x, y, 22, 9, 9, 22) <= 1.8
        }
    }
}

fn line_distance(x: i32, y: i32, x1: i32, y1: i32, x2: i32, y2: i32) -> f32 {
    let (px, py) = (x as f32, y as f32);
    let (ax, ay) = (x1 as f32, y1 as f32);
    let (bx, by) = (x2 as f32, y2 as f32);
    let (vx, vy) = (bx - ax, by - ay);
    let length_squared = vx * vx + vy * vy;
    let t = (((px - ax) * vx + (py - ay) * vy) / length_squared).clamp(0.0, 1.0);
    ((px - (ax + t * vx)).powi(2) + (py - (ay + t * vy)).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_icons_have_alpha_color_and_distinct_glyphs() {
        let idle = render_icon_pixels((34, 197, 94), IconGlyph::Check);
        let ready = render_icon_pixels((245, 158, 11), IconGlyph::Ready);
        let active = render_icon_pixels((239, 68, 68), IconGlyph::Active);
        let error = render_icon_pixels((107, 114, 128), IconGlyph::Error);

        assert_eq!(idle.len(), 32 * 32);
        assert_eq!(idle[0] >> 24, 0);
        assert_eq!(idle[16 * 32 + 16] >> 24, 255);
        assert_ne!(idle, ready);
        assert_ne!(ready, active);
        assert_ne!(active, error);
    }
}
