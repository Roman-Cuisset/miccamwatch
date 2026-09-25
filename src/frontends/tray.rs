use crate::{
    frontends::cli::Filter,
    i18n::Language,
    model::{
        Access, AccessEvent, Action, Activity, CollectorState, MicrophoneMuteState, Resource,
        SCHEMA_VERSION, Snapshot, event_code,
    },
    platform::{PlatformMonitor, SessionLockState},
    privacy::CameraPrivacyState,
    settings::{PrivacyProfile, Settings},
};
use anyhow::{Context, Result};
use std::{
    collections::HashMap,
    ffi::OsStr,
    mem::size_of,
    os::windows::ffi::OsStrExt,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicIsize, Ordering},
        mpsc::{self, Receiver},
    },
    thread,
    time::{Duration, Instant},
};
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
                FindWindowW, GWLP_USERDATA, GetCursorPos, GetMessageW, GetSystemMetrics,
                GetWindowLongPtrW, HICON, ICONINFO, KillTimer, MF_DISABLED, MF_GRAYED,
                MF_SEPARATOR, MF_STRING, MSG, PostMessageW, PostQuitMessage, RegisterClassW,
                SM_CXSMICON, SetForegroundWindow, SetTimer, SetWindowLongPtrW, TPM_BOTTOMALIGN,
                TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu, TranslateMessage,
                WINDOW_EX_STYLE, WM_APP, WM_CLOSE, WM_COMMAND, WM_DESTROY, WM_LBUTTONDBLCLK,
                WM_LBUTTONUP, WM_NCCREATE, WM_NULL, WM_RBUTTONUP, WM_TIMER, WNDCLASSW,
                WS_OVERLAPPED,
            },
        },
    },
    core::PCWSTR,
};

#[link(name = "user32")]
unsafe extern "system" {
    fn SetProcessDpiAwarenessContext(context: isize) -> i32;
}

/// `DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2`.
const PER_MONITOR_AWARE_V2: isize = -4;

/// Without this the shell renders the popup menu at 96 DPI and bitmap-stretches it,
/// which is what makes the tray text look pixelated on a scaled display.
fn enable_dpi_awareness() {
    unsafe {
        // Fails harmlessly when the embedded manifest already set the context.
        let _ = SetProcessDpiAwarenessContext(PER_MONITOR_AWARE_V2);
    }
}

const WM_TRAY_CALLBACK: u32 = WM_APP + 1;
const WM_CAMERA_RESULT: u32 = WM_APP + 2;
const WM_TRAY_REFRESH: u32 = WM_APP + 3;
const WM_RESTORE_ARRIVED: u32 = WM_APP + 4;
const WM_RESTORE_RESULT: u32 = WM_APP + 5;
const TIMER_POLL_ID: usize = 1;
const CAMERA_POLL_INTERVAL: Duration = Duration::from_secs(5);
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

struct TrayRefresh {
    snapshot: Result<Snapshot, String>,
    mute_state: MicrophoneMuteState,
    camera_state: CameraPrivacyState,
    lock_state: SessionLockState,
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
    refreshes: Receiver<TrayRefresh>,
    camera_dirty: Arc<AtomicBool>,
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

fn start_refresh_worker(
    hwnd_cell: Arc<AtomicIsize>,
    camera_dirty: Arc<AtomicBool>,
    policy: crate::config::Policy,
) -> Receiver<TrayRefresh> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let monitor = match PlatformMonitor::new(policy) {
            Ok(monitor) => monitor,
            Err(error) => {
                let _ = sender.send(TrayRefresh {
                    snapshot: Err(format!("{error:#}")),
                    mute_state: MicrophoneMuteState::Unavailable,
                    camera_state: CameraPrivacyState::SystemManaged,
                    lock_state: SessionLockState::Unknown,
                });
                let h = hwnd_cell.load(Ordering::Relaxed);
                if h != 0 {
                    unsafe {
                        let _ = PostMessageW(
                            Some(HWND(h as *mut _)),
                            WM_TRAY_REFRESH,
                            WPARAM(0),
                            LPARAM(0),
                        );
                    }
                }
                return;
            }
        };
        let filter = Filter {
            include_ready: true,
            ..Filter::default()
        };
        let mut camera_state =
            crate::privacy::camera_state().unwrap_or(CameraPrivacyState::SystemManaged);
        let mut last_camera_poll = Some(Instant::now());
        // Camera instance IDs already offered for restoration. Dropping an entry when
        // the device disappears is what re-arms the prompt after a replug, and it keeps
        // a declined UAC prompt from being raised again for the same arrival.
        let mut restore_offered: Vec<String> = Vec::new();
        loop {
            let snapshot = monitor
                .snapshot((&filter).into())
                .map_err(|error| format!("{error:#}"));
            let mute_state = monitor
                .microphone_mute_state()
                .unwrap_or(MicrophoneMuteState::Unavailable);
            // Session probes cross into the input desktop, so they stay off the window
            // thread or the popup menu paints late.
            let lock_state = crate::platform::session_lock_state();
            // `camera_state` shells out to pnputil; running it every cycle would spawn a
            // process twice a second forever. The tray updates it directly after a toggle,
            // so a slow poll is only there to notice out-of-band changes.
            let now = Instant::now();
            let camera_stale = last_camera_poll
                .is_none_or(|last| now.duration_since(last) >= CAMERA_POLL_INTERVAL);
            if camera_stale || camera_dirty.swap(false, Ordering::Relaxed) {
                camera_state =
                    crate::privacy::camera_state().unwrap_or(CameraPrivacyState::SystemManaged);
                last_camera_poll = Some(now);
            }
            if sender
                .send(TrayRefresh {
                    snapshot,
                    mute_state,
                    camera_state,
                    lock_state,
                })
                .is_err()
            {
                break;
            }
            let h = hwnd_cell.load(Ordering::Relaxed);
            if h == 0 {
                thread::sleep(Duration::from_millis(500));
                continue;
            }
            unsafe {
                let _ = PostMessageW(
                    Some(HWND(h as *mut _)),
                    WM_TRAY_REFRESH,
                    WPARAM(0),
                    LPARAM(0),
                );
            }
            // A camera the user already asked to restore is plugged back in, so finish
            // that request instead of waiting for another `mcw camera allow`. These
            // probes only touch cfgmgr32 and the record file, never the device tree.
            let arrived = crate::privacy::arrived_restore_targets().unwrap_or_default();
            let fresh = take_fresh_arrivals(&arrived, &mut restore_offered);
            if !fresh.is_empty() {
                unsafe {
                    let _ = PostMessageW(
                        Some(HWND(h as *mut _)),
                        WM_RESTORE_ARRIVED,
                        WPARAM(0),
                        LPARAM(0),
                    );
                }
            }
            thread::sleep(Duration::from_millis(500));
        }
    });
    receiver
}

pub fn run_tray(
    monitor: PlatformMonitor,
    policy: crate::config::Policy,
    lang: Language,
    settings: Settings,
) -> Result<()> {
    enable_dpi_awareness();
    let mutex_name = format_wide("Local\\MicCamWatch.Tray");
    let _mutex = MutexGuard(unsafe { CreateMutexW(None, true, PCWSTR(mutex_name.as_ptr()))? });
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        anyhow::bail!("MicCamWatch tray is already running");
    }
    let hwnd_cell = Arc::new(AtomicIsize::new(0));
    let camera_dirty = Arc::new(AtomicBool::new(false));
    let refreshes = start_refresh_worker(Arc::clone(&hwnd_cell), Arc::clone(&camera_dirty), policy);
    // The shell downsamples anything larger than the small-icon metric, which is what
    // made the icons look muddy. Render at the size the notification area will use.
    let icon_size = unsafe { GetSystemMetrics(SM_CXSMICON) }.max(16);

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
        refreshes,
        camera_dirty,
        restore_camera: None,
        previous_accesses: HashMap::new(),
        green_icon: create_status_icon((34, 197, 94), IconGlyph::Check, icon_size)?,
        yellow_icon: create_status_icon((245, 158, 11), IconGlyph::Ready, icon_size)?,
        red_icon: create_status_icon((239, 68, 68), IconGlyph::Active, icon_size)?,
        gray_icon: create_status_icon((107, 114, 128), IconGlyph::Error, icon_size)?,
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
        Ok(hwnd) => {
            hwnd_cell.store(hwnd.0 as isize, Ordering::Relaxed);
            hwnd
        }
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

    println!("miccamwatch tray running. Click the icon near the clock to open the menu.");
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
        WM_TRAY_REFRESH | WM_TIMER => {
            if let Some(state) = state(hwnd) {
                refresh_state(hwnd, state);
            }
            LRESULT(0)
        }
        WM_TRAY_CALLBACK => {
            match lparam.0 as u32 {
                // A plain left click opens the menu, matching what users expect from
                // other tray applications. Double clicks keep the status toast.
                WM_LBUTTONUP | WM_RBUTTONUP => show_context_menu(hwnd),
                WM_LBUTTONDBLCLK => show_status_toast(hwnd),
                _ => {}
            }
            LRESULT(0)
        }
        WM_CAMERA_RESULT => {
            let result =
                unsafe { Box::from_raw(lparam.0 as *mut Result<CameraPrivacyState, String>) };
            if let Some(state) = state(hwnd) {
                handle_camera_result(state, *result);
            }
            LRESULT(0)
        }
        WM_RESTORE_ARRIVED => {
            restore_arrived_cameras(hwnd);
            LRESULT(0)
        }
        WM_RESTORE_RESULT => {
            let result = unsafe { Box::from_raw(lparam.0 as *mut Result<usize, String>) };
            if let Some(state) = state(hwnd) {
                handle_restore_result(state, *result);
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

fn apply_lock_policy(
    state: &mut TrayAppState,
    current: SessionLockState,
    camera_state: CameraPrivacyState,
) {
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
        if state.settings.block_camera_on_lock
            && crate::privacy::set_camera_state(CameraPrivacyState::Blocked).is_ok()
        {
            state.restore_camera = Some(camera_state);
            state.camera_dirty.store(true, Ordering::Relaxed);
        }
    } else if current == SessionLockState::Unlocked && state.settings.restore_on_unlock {
        if let Some(was_muted) = state.restore_mute.take() {
            let _ = state.monitor.set_microphone_mute(was_muted);
        }
        if let Some(previous) = state.restore_camera.take()
            && previous != CameraPrivacyState::SystemManaged
            && crate::privacy::set_camera_state(previous).is_ok()
        {
            state.camera_dirty.store(true, Ordering::Relaxed);
        }
    }
    state.lock_state = current;
}

/// WinRT toast delivery round-trips through the shell and can take hundreds of
/// milliseconds; running it inline would stall the window thread mid-menu.
fn notify_async(title: &str, message: &str) {
    let title = title.to_owned();
    let message = message.to_owned();
    thread::spawn(move || {
        if let Err(error) = crate::notify::notify_message(&title, &message) {
            eprintln!("notification failed: {error}");
        }
    });
}

fn refresh_state(hwnd: HWND, state: &mut TrayAppState) {
    let mut latest = None;
    while let Ok(refresh) = state.refreshes.try_recv() {
        latest = Some(refresh);
    }
    let Some(refresh) = latest else { return };
    apply_lock_policy(state, refresh.lock_state, refresh.camera_state);
    let (visual, summary) = match refresh.snapshot {
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
    let mute_state = refresh.mute_state;
    let camera_state = refresh.camera_state;
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
            notify_async("miccamwatch", message);
        }
        Err(error) => {
            state.summary = format!("miccamwatch: mute failed: {error}");
            state.visual = TrayVisual::Error;
        }
    }
}

fn toggle_camera(hwnd: HWND) {
    // Run the elevated pnputil call on a background thread so the UI stays responsive
    // during the UAC prompt. The result is posted back via WM_CAMERA_RESULT.
    let hwnd_raw = hwnd.0 as usize;
    thread::spawn(move || {
        let result = crate::privacy::toggle_camera().map_err(|error| format!("{error:#}"));
        let boxed = Box::into_raw(Box::new(result));
        let hwnd = HWND(hwnd_raw as *mut _);
        unsafe {
            let _ = PostMessageW(
                Some(hwnd),
                WM_CAMERA_RESULT,
                WPARAM(0),
                LPARAM(boxed as isize),
            );
        }
    });
}

/// Narrows reconnected cameras down to the ones not yet offered for restoration,
/// and remembers them.
///
/// Keeping the bookkeeping here means a declined administrator prompt is not
/// raised again for the same arrival. An entry that leaves `arrived` is forgotten,
/// which is what re-arms the prompt once the camera is unplugged and connected
/// again.
fn take_fresh_arrivals(arrived: &[String], offered: &mut Vec<String>) -> Vec<String> {
    offered.retain(|id| {
        arrived
            .iter()
            .any(|arrival| arrival.eq_ignore_ascii_case(id))
    });
    let fresh = arrived
        .iter()
        .filter(|id| !offered.iter().any(|seen| seen.eq_ignore_ascii_case(id)))
        .cloned()
        .collect::<Vec<_>>();
    offered.extend(fresh.iter().cloned());
    fresh
}

fn restore_arrived_cameras(hwnd: HWND) {
    // The enable needs administrator approval, so it runs off the window thread and
    // reports back through WM_RESTORE_RESULT. The worker only raises this once per
    // arrival, so a declined prompt is not repeated.
    let hwnd_raw = hwnd.0 as usize;
    thread::spawn(move || {
        let result = crate::privacy::restore_arrived().map_err(|error| format!("{error:#}"));
        let boxed = Box::into_raw(Box::new(result));
        unsafe {
            let _ = PostMessageW(
                Some(HWND(hwnd_raw as *mut _)),
                WM_RESTORE_RESULT,
                WPARAM(0),
                LPARAM(boxed as isize),
            );
        }
    });
}

fn handle_restore_result(state: &mut TrayAppState, result: Result<usize, String>) {
    // Let the poller re-read privacy state rather than guessing it here.
    state.camera_dirty.store(true, Ordering::Relaxed);
    match result {
        Ok(restored) => {
            let message = format!(
                "Reconnected camera restored ({restored} device{}).",
                if restored == 1 { "" } else { "s" }
            );
            notify_async("MicCamWatch camera", &message);
        }
        Err(error) => {
            // Staying silent after a declined prompt matters more than reporting it,
            // so the failure only surfaces in the tray summary and tooltip.
            state.summary = format!("miccamwatch: camera restore failed: {error}");
            state.visual = TrayVisual::Error;
        }
    }
}

fn handle_camera_result(state: &mut TrayAppState, result: Result<CameraPrivacyState, String>) {
    match result {
        Ok(camera_state) => {
            state.camera_state = camera_state;
            // Force the poller to re-read privacy state instead of overwriting this
            // with a value captured before the toggle.
            state.camera_dirty.store(true, Ordering::Relaxed);
            let message = match camera_state {
                CameraPrivacyState::Allowed => {
                    "Connected cameras are allowed. Plugged-in cameras that are still blocked will be restored automatically."
                }
                CameraPrivacyState::Blocked => "Connected cameras are blocked.",
                CameraPrivacyState::SystemManaged => {
                    "A blocked camera is still unplugged; it will be restored automatically when reconnected."
                }
            };
            notify_async("MicCamWatch camera", message);
        }
        Err(error) => {
            let message = format!("Camera control failed: {error}");
            state.summary = format!("miccamwatch: {message}");
            state.visual = TrayVisual::Error;
            notify_async("MicCamWatch camera", &message);
        }
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
            CameraPrivacyState::SystemManaged => "Allow camera (blocked camera unplugged)",
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
        let command = TrackPopupMenu(
            menu,
            TPM_RIGHTBUTTON | TPM_BOTTOMALIGN | TPM_RETURNCMD | TPM_NONOTIFY,
            point.x,
            point.y,
            None,
            hwnd,
            None,
        )
        .0;
        let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
        if command != 0 {
            let _ = PostMessageW(Some(hwnd), WM_COMMAND, WPARAM(command as usize), LPARAM(0));
        }
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
    notify_async("miccamwatch", &state.summary);
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

fn create_status_icon(color: (u8, u8, u8), glyph: IconGlyph, size: i32) -> Result<HICON> {
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: size,
            biHeight: -size,
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
    let rendered = render_icon_pixels(color, glyph, size);
    let pixels =
        unsafe { std::slice::from_raw_parts_mut(bits.cast::<u32>(), (size * size) as usize) };
    pixels.copy_from_slice(&rendered);
    let mask = vec![0u8; ((size * size) / 8) as usize];
    let mask_bitmap = unsafe { CreateBitmap(size, size, 1, 1, Some(mask.as_ptr().cast())) };
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

fn render_icon_pixels(color: (u8, u8, u8), glyph: IconGlyph, size: i32) -> Vec<u32> {
    let mut pixels = vec![0; (size * size) as usize];
    let scale = size as f32 / 32.0;
    for y in 0..size {
        for x in 0..size {
            // Sample the 32px glyph geometry in destination space so every icon size
            // keeps the same proportions instead of being stretched by the shell.
            let sx = (x as f32 + 0.5) / scale - 0.5;
            let sy = (y as f32 + 0.5) / scale - 0.5;
            let dx = sx - 15.5;
            let dy = sy - 15.5;
            let distance = (dx * dx + dy * dy).sqrt();
            let alpha = ((15.0 - distance).clamp(0.0, 1.0) * 255.0) as u32;
            if alpha == 0 {
                continue;
            }
            let white = glyph_pixel(glyph, sx, sy);
            let (red, green, blue) = if white { (255, 255, 255) } else { color };
            pixels[(y * size + x) as usize] = (alpha << 24)
                | ((red as u32 * alpha / 255) << 16)
                | ((green as u32 * alpha / 255) << 8)
                | (blue as u32 * alpha / 255);
        }
    }
    pixels
}

fn glyph_pixel(glyph: IconGlyph, x: f32, y: f32) -> bool {
    match glyph {
        IconGlyph::Check => {
            line_distance(x, y, 8.0, 16.0, 13.0, 21.0) <= 1.7
                || line_distance(x, y, 13.0, 21.0, 24.0, 10.0) <= 1.7
        }
        IconGlyph::Ready => {
            (9.0..=22.0).contains(&y) && ((10.0..=12.0).contains(&x) || (19.0..=21.0).contains(&x))
        }
        IconGlyph::Active => {
            ((13.0..=18.0).contains(&x) && (7.0..=19.0).contains(&y))
                || ((13.0..=18.0).contains(&x) && (23.0..=27.0).contains(&y))
        }
        IconGlyph::Error => {
            line_distance(x, y, 9.0, 9.0, 22.0, 22.0) <= 1.8
                || line_distance(x, y, 22.0, 9.0, 9.0, 22.0) <= 1.8
        }
    }
}

fn line_distance(x: f32, y: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    let (vx, vy) = (x2 - x1, y2 - y1);
    let length_squared = vx * vx + vy * vy;
    let t = (((x - x1) * vx + (y - y1) * vy) / length_squared).clamp(0.0, 1.0);
    ((x - (x1 + t * vx)).powi(2) + (y - (y1 + t * vy)).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_declined_restore_prompt_is_not_raised_again() {
        let logi = "USB\\CAMERA_LOGI".to_owned();
        let mut offered = Vec::new();

        // First sighting reports the camera and arms the bookkeeping.
        assert_eq!(
            take_fresh_arrivals(std::slice::from_ref(&logi), &mut offered),
            vec![logi.clone()]
        );
        // Still connected on the next poll: the prompt must not reappear.
        assert!(take_fresh_arrivals(std::slice::from_ref(&logi), &mut offered).is_empty());
        assert_eq!(offered, vec![logi.clone()]);
    }

    #[test]
    fn unplugging_re_arms_the_restore_prompt() {
        let logi = "USB\\CAMERA_LOGI".to_owned();
        let mut offered = vec![logi.clone()];

        // The camera is gone, so the entry is forgotten.
        assert!(take_fresh_arrivals(&[], &mut offered).is_empty());
        assert!(offered.is_empty());

        // Plugged back in, it is offered again.
        assert_eq!(
            take_fresh_arrivals(std::slice::from_ref(&logi), &mut offered),
            vec![logi]
        );
    }

    #[test]
    fn only_newly_arrived_cameras_are_reported() {
        let builtin = "USB\\CAMERA_BUILTIN".to_owned();
        let logi = "USB\\CAMERA_LOGI".to_owned();
        let mut offered = vec![builtin.clone()];

        let fresh = take_fresh_arrivals(&[builtin.clone(), logi.clone()], &mut offered);
        assert_eq!(fresh, vec![logi]);
        // Case differences in PnP instance IDs must not defeat the bookkeeping.
        assert!(take_fresh_arrivals(&[builtin.to_uppercase()], &mut offered).is_empty());
    }

    #[test]
    fn tray_icons_have_alpha_color_and_distinct_glyphs() {
        let idle = render_icon_pixels((34, 197, 94), IconGlyph::Check, 32);
        let ready = render_icon_pixels((245, 158, 11), IconGlyph::Ready, 32);
        let active = render_icon_pixels((239, 68, 68), IconGlyph::Active, 32);
        let error = render_icon_pixels((107, 114, 128), IconGlyph::Error, 32);

        assert_eq!(idle.len(), 32 * 32);
        assert_eq!(idle[0] >> 24, 0);
        assert_eq!(idle[16 * 32 + 16] >> 24, 255);
        assert_ne!(idle, ready);
        assert_ne!(ready, active);
        assert_ne!(active, error);
    }

    #[test]
    fn every_supported_icon_size_renders_an_opaque_centre() {
        for size in [16, 20, 24, 32, 40] {
            let pixels = render_icon_pixels((34, 197, 94), IconGlyph::Check, size);
            assert_eq!(pixels.len(), (size * size) as usize);
            assert_eq!(
                pixels[0] >> 24,
                0,
                "corner must stay transparent at {size}px"
            );
            let centre = pixels[((size / 2) * size + size / 2) as usize];
            assert_eq!(centre >> 24, 255, "centre must be opaque at {size}px");
        }
    }
}
