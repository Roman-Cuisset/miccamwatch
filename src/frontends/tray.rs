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
        atomic::{AtomicBool, AtomicIsize, AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender, SyncSender},
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
const WM_RESTORE_RESULT: u32 = WM_APP + 4;
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
    camera_generation: u64,
    lock_state: SessionLockState,
}

#[derive(Clone, Copy, Debug)]
enum CameraRequest {
    Manual,
    Block {
        id: u64,
        previous: CameraPrivacyState,
        manual_generation: u64,
    },
    Restore {
        id: u64,
        previous: CameraPrivacyState,
    },
}

type CameraResult = (CameraRequest, Result<CameraPrivacyState, String>);
type CameraWorker = (Sender<CameraRequest>, Receiver<CameraResult>);

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
    camera_sequence: Arc<AtomicU64>,
    camera_generation: u64,
    hwnd_cell: Arc<AtomicIsize>,
    restore_results: Receiver<Result<usize, String>>,
    camera_requests: Sender<CameraRequest>,
    camera_results: Receiver<CameraResult>,
    camera_next_id: u64,
    manual_generation: u64,
    pending_manual: usize,
    deferred_lock_block: bool,
    pending_lock_block: Option<u64>,
    pending_lock_restore: Option<u64>,
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
    camera_sequence: Arc<AtomicU64>,
    restore_requests: SyncSender<()>,
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
                    camera_generation: 0,
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
        let mut camera_generation = camera_sequence.load(Ordering::Acquire);
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
            // Native device inventory is kept off the window thread. The
            // generation tags a poll so older queued frames cannot replace
            // a more recent camera operation result.
            let now = Instant::now();
            let camera_stale = last_camera_poll
                .is_none_or(|last| now.duration_since(last) >= CAMERA_POLL_INTERVAL);
            if camera_stale || camera_dirty.swap(false, Ordering::AcqRel) {
                let generation = camera_sequence.load(Ordering::Acquire);
                camera_state =
                    crate::privacy::camera_state().unwrap_or(CameraPrivacyState::SystemManaged);
                camera_generation = generation;
                last_camera_poll = Some(now);
            }
            if sender
                .send(TrayRefresh {
                    snapshot,
                    mute_state,
                    camera_state,
                    camera_generation,
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
            if !take_fresh_arrivals(
                crate::privacy::arrived_restore_targets().as_deref(),
                &mut restore_offered,
            )
            .is_empty()
            {
                let _ = restore_requests.try_send(());
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
    let camera_sequence = Arc::new(AtomicU64::new(0));
    let (restore_requests, restore_results) = start_restore_worker(Arc::clone(&hwnd_cell));
    let (camera_requests, camera_results) = start_camera_worker(Arc::clone(&hwnd_cell));
    let refreshes = start_refresh_worker(
        Arc::clone(&hwnd_cell),
        Arc::clone(&camera_dirty),
        Arc::clone(&camera_sequence),
        restore_requests,
        policy,
    );
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
        camera_sequence,
        camera_generation: 0,
        hwnd_cell: Arc::clone(&hwnd_cell),
        restore_results,
        restore_camera: None,
        camera_requests,
        camera_results,
        camera_next_id: 0,
        manual_generation: 0,
        pending_manual: 0,
        deferred_lock_block: false,
        pending_lock_block: None,
        pending_lock_restore: None,
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

    hwnd_cell.store(0, Ordering::Relaxed);
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
                // Timer fallback also consumes results if a worker's PostMessage failed.
                handle_worker_results(state);
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
        WM_CAMERA_RESULT | WM_RESTORE_RESULT => {
            if let Some(state) = state(hwnd) {
                handle_worker_results(state);
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
            if let Some(state) = state(hwnd) {
                state.hwnd_cell.store(0, Ordering::Relaxed);
            }
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

fn start_camera_worker(hwnd_cell: Arc<AtomicIsize>) -> CameraWorker {
    let (requests, pending) = mpsc::channel();
    let (results, delivered) = mpsc::channel();
    thread::spawn(move || {
        camera_worker(
            pending,
            results,
            |request| {
                match request {
                    CameraRequest::Manual => crate::privacy::toggle_camera(),
                    CameraRequest::Block { .. } => {
                        crate::privacy::set_camera_state(CameraPrivacyState::Blocked)
                            .and_then(|()| crate::privacy::camera_state())
                    }
                    CameraRequest::Restore { previous, .. } => {
                        crate::privacy::set_camera_state(previous)
                            .and_then(|()| crate::privacy::camera_state())
                    }
                }
                .map_err(|error| format!("{error:#}"))
            },
            || {
                let h = hwnd_cell.load(Ordering::Relaxed);
                if h != 0 {
                    unsafe {
                        let _ = PostMessageW(
                            Some(HWND(h as *mut _)),
                            WM_CAMERA_RESULT,
                            WPARAM(0),
                            LPARAM(0),
                        );
                    }
                }
            },
            || hwnd_cell.load(Ordering::Relaxed) != 0,
        );
    });
    (requests, delivered)
}

fn camera_worker(
    requests: Receiver<CameraRequest>,
    results: Sender<CameraResult>,
    mut execute: impl FnMut(CameraRequest) -> Result<CameraPrivacyState, String>,
    mut notify: impl FnMut(),
    mut alive: impl FnMut() -> bool,
) {
    while let Ok(request) = requests.recv() {
        if !alive() {
            break;
        }
        if results.send((request, execute(request))).is_err() {
            break;
        }
        notify();
    }
}

fn queue_lock_camera_restore(state: &mut TrayAppState, previous: CameraPrivacyState) {
    if state.pending_lock_restore.is_some() {
        return;
    }
    state.camera_next_id = state.camera_next_id.wrapping_add(1);
    let id = state.camera_next_id;
    if state
        .camera_requests
        .send(CameraRequest::Restore { id, previous })
        .is_ok()
    {
        state.pending_lock_restore = Some(id);
    }
}

fn queue_lock_camera_block(state: &mut TrayAppState, camera_state: CameraPrivacyState) {
    if state.pending_lock_block.is_some() {
        return;
    }
    state.camera_next_id = state.camera_next_id.wrapping_add(1);
    let id = state.camera_next_id;
    let previous = state.restore_camera.unwrap_or(camera_state);
    if state
        .camera_requests
        .send(CameraRequest::Block {
            id,
            previous,
            manual_generation: state.manual_generation,
        })
        .is_ok()
    {
        state.pending_lock_block = Some(id);
    }
}

fn apply_lock_policy(state: &mut TrayAppState, current: SessionLockState) {
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
            if state.pending_manual == 0 {
                queue_lock_camera_block(state, state.camera_state);
            } else {
                state.deferred_lock_block = true;
            }
        }
    } else {
        state.deferred_lock_block = false;
        if current == SessionLockState::Unlocked && state.settings.restore_on_unlock {
            if let Some(was_muted) = state.restore_mute.take() {
                let _ = state.monitor.set_microphone_mute(was_muted);
            }
            if state.pending_lock_block.is_none()
                && let Some(previous) = state.restore_camera
                && previous == CameraPrivacyState::Allowed
            {
                queue_lock_camera_restore(state, previous);
            }
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

fn current_camera_from_refresh(
    refresh_generation: u64,
    completed_generation: u64,
    refresh_state: CameraPrivacyState,
    completed_state: CameraPrivacyState,
) -> CameraPrivacyState {
    if refresh_generation == completed_generation {
        refresh_state
    } else {
        completed_state
    }
}

fn refresh_state(hwnd: HWND, state: &mut TrayAppState) {
    let mut latest = None;
    while let Ok(refresh) = state.refreshes.try_recv() {
        latest = Some(refresh);
    }
    let Some(refresh) = latest else { return };
    let camera_state = current_camera_from_refresh(
        refresh.camera_generation,
        state.camera_generation,
        refresh.camera_state,
        state.camera_state,
    );
    let old_camera_state = state.camera_state;
    state.camera_state = camera_state;
    apply_lock_policy(state, refresh.lock_state);
    let (visual, summary) = match refresh.snapshot {
        Ok(snapshot) => {
            let unhealthy = snapshot
                .collectors
                .iter()
                .any(|collector| collector.state != CollectorState::Healthy);
            record_history_changes(state, &snapshot);
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
    if visual == state.visual
        && summary == state.summary
        && mute_state == state.mute_state
        && camera_state == old_camera_state
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

fn record_history_changes(state: &mut TrayAppState, snapshot: &Snapshot) {
    if !state.settings.history_enabled {
        return;
    }
    let current = history_current(&state.previous_accesses, snapshot);
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

fn history_current(
    previous: &HashMap<String, Access>,
    snapshot: &Snapshot,
) -> HashMap<String, Access> {
    let mut unavailable_mic = false;
    let mut unavailable_camera = false;
    for collector in snapshot
        .collectors
        .iter()
        .filter(|collector| collector.state == CollectorState::Unavailable)
    {
        match collector.collector {
            "wasapi" => unavailable_mic = true,
            "privacy_store" | "module_scanner" => unavailable_camera = true,
            _ => {
                unavailable_mic = true;
                unavailable_camera = true;
            }
        }
    }
    let mut current = snapshot
        .accesses
        .iter()
        .map(|access| (access.key.clone(), access.clone()))
        .collect::<HashMap<_, _>>();
    for (key, access) in previous {
        if (access.resource == Resource::Microphone && unavailable_mic)
            || (access.resource == Resource::Camera && unavailable_camera)
        {
            current.entry(key.clone()).or_insert_with(|| access.clone());
        }
    }
    current
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
    let Some(state) = state(hwnd) else { return };
    if state.camera_requests.send(CameraRequest::Manual).is_ok() {
        state.manual_generation = state.manual_generation.wrapping_add(1);
        state.pending_manual += 1;
    }
}

/// Narrows reconnected cameras down to the ones not yet offered for restoration,
/// and remembers them. A failed probe is not evidence that any camera was
/// unplugged: retain all offers until a successful probe observes their absence.
fn take_fresh_arrivals<E>(arrived: Result<&[String], E>, offered: &mut Vec<String>) -> Vec<String> {
    let Ok(arrived) = arrived else {
        return Vec::new();
    };
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

// Only one automatic restore worker may enter the elevated privacy operation.
// A one-slot queue coalesces multiple arrivals while a UAC prompt is in flight.
fn start_restore_worker(
    hwnd_cell: Arc<AtomicIsize>,
) -> (SyncSender<()>, Receiver<Result<usize, String>>) {
    let (requests, pending) = mpsc::sync_channel(1);
    let (results, delivered) = mpsc::channel();
    thread::spawn(move || {
        restore_worker(
            pending,
            results,
            || hwnd_cell.load(Ordering::Relaxed) != 0,
            || crate::privacy::restore_arrived().map_err(|error| format!("{error:#}")),
            || {
                let h = hwnd_cell.load(Ordering::Relaxed);
                h != 0
                    && unsafe {
                        PostMessageW(
                            Some(HWND(h as *mut _)),
                            WM_RESTORE_RESULT,
                            WPARAM(0),
                            LPARAM(0),
                        )
                        .is_ok()
                    }
            },
        );
    });
    (requests, delivered)
}

fn restore_worker(
    requests: Receiver<()>,
    results: mpsc::Sender<Result<usize, String>>,
    alive: impl Fn() -> bool,
    mut restore: impl FnMut() -> Result<usize, String>,
    mut notify: impl FnMut() -> bool,
) {
    while requests.recv().is_ok() {
        if !alive() {
            break;
        }
        let result = restore();
        if result.is_err() {
            // A declined UAC prompt must not be raised again just because a
            // different camera arrived during that prompt.
            while requests.try_recv().is_ok() {}
        }
        if results.send(result).is_err() {
            break;
        }
        // A failed post leaves the owned result in the tray's channel; its
        // timer consumes it while the window is alive. A closed window ends work.
        if !notify() && !alive() {
            break;
        }
    }
}

fn handle_worker_results(state: &mut TrayAppState) {
    while let Ok((request, result)) = state.camera_results.try_recv() {
        handle_camera_operation_result(state, request, result);
    }
    while let Ok(result) = state.restore_results.try_recv() {
        handle_restore_result(state, result);
    }
}

fn restore_due_after_lock_result(
    pending: Option<u64>,
    id: u64,
    succeeded: bool,
    previous: CameraPrivacyState,
    lock_state: SessionLockState,
    restore_on_unlock: bool,
    same_manual_generation: bool,
) -> bool {
    pending == Some(id)
        && succeeded
        && previous == CameraPrivacyState::Allowed
        && lock_state == SessionLockState::Unlocked
        && restore_on_unlock
        && same_manual_generation
}

fn should_queue_deferred_block(
    pending_manual: usize,
    deferred_lock_block: bool,
    lock_state: SessionLockState,
    block_camera_on_lock: bool,
) -> bool {
    pending_manual == 0
        && deferred_lock_block
        && lock_state == SessionLockState::Locked
        && block_camera_on_lock
}

fn handle_camera_operation_result(
    state: &mut TrayAppState,
    request: CameraRequest,
    result: Result<CameraPrivacyState, String>,
) {
    state.camera_generation = state.camera_generation.wrapping_add(1);
    state
        .camera_sequence
        .store(state.camera_generation, Ordering::Release);
    state.camera_dirty.store(true, Ordering::Release);
    match request {
        CameraRequest::Manual => {
            state.pending_manual -= 1;
            handle_camera_result(state, result);
            let block_after_manual = should_queue_deferred_block(
                state.pending_manual,
                state.deferred_lock_block,
                state.lock_state,
                state.settings.block_camera_on_lock,
            );
            if state.pending_manual == 0 {
                state.deferred_lock_block = false;
            }
            if block_after_manual {
                queue_lock_camera_block(state, state.camera_state);
            }
        }
        CameraRequest::Block {
            id,
            previous,
            manual_generation,
        } => {
            let same_manual_generation = state.manual_generation == manual_generation;
            let restore_now = restore_due_after_lock_result(
                state.pending_lock_block,
                id,
                result.is_ok(),
                previous,
                state.lock_state,
                state.settings.restore_on_unlock,
                same_manual_generation,
            );
            if state.pending_lock_block != Some(id) {
                return;
            }
            state.pending_lock_block = None;
            match result {
                Ok(camera_state) => {
                    state.camera_state = camera_state;
                    if previous == CameraPrivacyState::Allowed {
                        state.restore_camera = Some(previous);
                        if restore_now {
                            queue_lock_camera_restore(state, previous);
                        }
                    }
                }
                Err(error) => {
                    let message = format!("Camera lock policy failed: {error}");
                    state.summary = format!("miccamwatch: {message}");
                    state.visual = TrayVisual::Error;
                    notify_async("MicCamWatch camera", &message);
                }
            }
        }
        CameraRequest::Restore { id, .. } => {
            if state.pending_lock_restore != Some(id) {
                return;
            }
            state.pending_lock_restore = None;
            match result {
                Ok(camera_state) => {
                    state.camera_state = camera_state;
                    if state.lock_state == SessionLockState::Unlocked
                        && state.pending_lock_block.is_none()
                    {
                        state.restore_camera = None;
                    }
                }
                Err(error) => {
                    let message = format!("Camera unlock policy failed: {error}");
                    state.summary = format!("miccamwatch: {message}");
                    state.visual = TrayVisual::Error;
                    notify_async("MicCamWatch camera", &message);
                }
            }
        }
    }
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

fn commit_manual_camera_state(
    camera_state: &mut CameraPrivacyState,
    restore_camera: &mut Option<CameraPrivacyState>,
    completed: CameraPrivacyState,
) {
    *camera_state = completed;
    *restore_camera = None;
}

fn handle_camera_result(state: &mut TrayAppState, result: Result<CameraPrivacyState, String>) {
    match result {
        Ok(camera_state) => {
            commit_manual_camera_state(
                &mut state.camera_state,
                &mut state.restore_camera,
                camera_state,
            );
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
    fn observed(resource: Resource, key: &str) -> Access {
        Access {
            key: key.to_owned(),
            resource,
            activity: Activity::Active,
            risk: crate::model::Risk::Expected,
            confidence: crate::model::Confidence::High,
            enforcement: crate::model::EnforcementDecision::Alert,
            application: "capture.exe".into(),
            pid: Some(123),
            parent_pid: None,
            parent_name: None,
            executable: None,
            signature: None,
            device: None,
            started_at: None,
            modules: vec![],
            evidence: vec![],
            process: None,
        }
    }

    #[test]
    fn stale_camera_refresh_cannot_revert_manual_completion() {
        assert_eq!(
            current_camera_from_refresh(
                4,
                5,
                CameraPrivacyState::Allowed,
                CameraPrivacyState::Blocked,
            ),
            CameraPrivacyState::Blocked
        );
        assert_eq!(
            current_camera_from_refresh(
                5,
                5,
                CameraPrivacyState::Allowed,
                CameraPrivacyState::Blocked,
            ),
            CameraPrivacyState::Allowed
        );
    }

    #[test]
    fn declined_manual_toggle_does_not_reprompt_pending_unlock_restore() {
        // The block finished after unlock; its original restore obligation is
        // retained, but a declined manual UAC must not issue another request.
        let mut restore_camera = Some(CameraPrivacyState::Allowed);
        let mut camera_state = CameraPrivacyState::Blocked;
        let declined: Result<CameraPrivacyState, String> = Err("approval declined".into());
        if let Ok(completed) = declined {
            commit_manual_camera_state(&mut camera_state, &mut restore_camera, completed);
        }
        assert_eq!(restore_camera, Some(CameraPrivacyState::Allowed));
        assert_eq!(camera_state, CameraPrivacyState::Blocked);
        assert!(!should_queue_deferred_block(
            0,
            false,
            SessionLockState::Unlocked,
            true,
        ));
    }

    #[test]
    fn lock_block_result_restores_only_successful_matching_unlocked_transition() {
        let restores = |pending, id, succeeded, previous, lock_state| {
            restore_due_after_lock_result(pending, id, succeeded, previous, lock_state, true, true)
        };
        assert!(!restores(
            Some(3),
            3,
            false,
            CameraPrivacyState::Allowed,
            SessionLockState::Unlocked
        ));
        assert!(!restores(
            Some(4),
            3,
            true,
            CameraPrivacyState::Allowed,
            SessionLockState::Unlocked
        ));
        assert!(!restores(
            Some(3),
            3,
            true,
            CameraPrivacyState::Allowed,
            SessionLockState::Locked
        ));
        assert!(!restores(
            Some(3),
            3,
            true,
            CameraPrivacyState::Blocked,
            SessionLockState::Unlocked
        ));
        assert!(restores(
            Some(3),
            3,
            true,
            CameraPrivacyState::Allowed,
            SessionLockState::Unlocked
        ));
        assert!(!restore_due_after_lock_result(
            Some(3),
            3,
            true,
            CameraPrivacyState::Allowed,
            SessionLockState::Unlocked,
            true,
            false,
        ));
    }

    #[test]
    fn manual_block_supersedes_failed_restore_on_next_lock() {
        let mut state = CameraPrivacyState::Allowed;
        let mut restore_camera = Some(CameraPrivacyState::Allowed);
        // A declined automatic restore leaves its intent outstanding.
        commit_manual_camera_state(&mut state, &mut restore_camera, CameraPrivacyState::Blocked);
        assert_eq!(state, CameraPrivacyState::Blocked);
        assert_eq!(restore_camera, None);
        let previous = restore_camera.unwrap_or(state);
        assert!(!restore_due_after_lock_result(
            Some(5),
            5,
            true,
            previous,
            SessionLockState::Unlocked,
            true,
            true,
        ));
    }

    #[test]
    fn serialized_camera_worker_applies_manual_block_after_unlock_restore() {
        let (requests, pending) = mpsc::channel();
        let (results, delivered) = mpsc::channel();
        requests
            .send(CameraRequest::Restore {
                id: 1,
                previous: CameraPrivacyState::Allowed,
            })
            .unwrap();
        requests.send(CameraRequest::Manual).unwrap();
        drop(requests);
        camera_worker(
            pending,
            results,
            |request| match request {
                CameraRequest::Restore { .. } => Ok(CameraPrivacyState::Allowed),
                CameraRequest::Manual => Ok(CameraPrivacyState::Blocked),
                CameraRequest::Block { .. } => unreachable!(),
            },
            || {},
            || true,
        );
        let mut actual = CameraPrivacyState::Blocked;
        let mut restore_camera = Some(CameraPrivacyState::Allowed);
        for (request, completed) in delivered {
            let completed = completed.unwrap();
            match request {
                CameraRequest::Restore { .. } => {
                    actual = completed;
                    restore_camera = None;
                }
                CameraRequest::Manual => {
                    commit_manual_camera_state(&mut actual, &mut restore_camera, completed);
                }
                CameraRequest::Block { .. } => unreachable!(),
            }
        }
        assert_eq!(actual, CameraPrivacyState::Blocked);
        assert_eq!(restore_camera, None);
    }

    #[test]
    fn partial_outage_preserves_mic_without_losing_new_camera_history() {
        let mic = observed(Resource::Microphone, "microphone:old");
        let camera = observed(Resource::Camera, "camera:new");
        let old = HashMap::from([(mic.key.clone(), mic)]);
        let snapshot = Snapshot {
            collectors: vec![
                crate::model::CollectorHealth {
                    collector: "wasapi",
                    state: CollectorState::Unavailable,
                    detail: None,
                },
                crate::model::CollectorHealth {
                    collector: "privacy_store",
                    state: CollectorState::Healthy,
                    detail: None,
                },
            ],
            accesses: vec![camera],
        };
        let current = history_current(&old, &snapshot);
        assert!(
            current.contains_key("microphone:old"),
            "outage is not a stop"
        );
        assert!(
            current.contains_key("camera:new"),
            "healthy camera start is kept"
        );
    }

    #[test]
    fn a_declined_restore_prompt_is_not_raised_again() {
        let logi = "USB\\CAMERA_LOGI".to_owned();
        let mut offered = Vec::new();

        // First sighting reports the camera and arms the bookkeeping.
        assert_eq!(
            take_fresh_arrivals(Ok::<_, ()>(std::slice::from_ref(&logi)), &mut offered),
            vec![logi.clone()]
        );
        // Still connected on the next poll: the prompt must not reappear.
        assert!(
            take_fresh_arrivals(Ok::<_, ()>(std::slice::from_ref(&logi)), &mut offered).is_empty()
        );
        assert_eq!(offered, vec![logi.clone()]);
    }

    #[test]
    fn unplugging_re_arms_the_restore_prompt() {
        let logi = "USB\\CAMERA_LOGI".to_owned();
        let mut offered = vec![logi.clone()];

        // The camera is gone, so the entry is forgotten.
        assert!(take_fresh_arrivals(Ok::<_, ()>(&[]), &mut offered).is_empty());
        assert!(offered.is_empty());

        // Plugged back in, it is offered again.
        assert_eq!(
            take_fresh_arrivals(Ok::<_, ()>(std::slice::from_ref(&logi)), &mut offered),
            vec![logi]
        );
    }

    #[test]
    fn only_newly_arrived_cameras_are_reported() {
        let builtin = "USB\\CAMERA_BUILTIN".to_owned();
        let logi = "USB\\CAMERA_LOGI".to_owned();
        let mut offered = vec![builtin.clone()];

        let fresh =
            take_fresh_arrivals(Ok::<_, ()>(&[builtin.clone(), logi.clone()]), &mut offered);
        assert_eq!(fresh, vec![logi]);
        // Case differences in PnP instance IDs must not defeat the bookkeeping.
        assert!(
            take_fresh_arrivals(Ok::<_, ()>(&[builtin.to_uppercase()]), &mut offered).is_empty()
        );
    }

    #[test]
    fn failed_arrival_probe_keeps_offered_ids_until_observed_absent() {
        let id = "USB\\CAMERA_LOGI".to_owned();
        let mut offered = Vec::new();
        assert_eq!(
            take_fresh_arrivals(Ok::<_, ()>(std::slice::from_ref(&id)), &mut offered),
            vec![id.clone()]
        );
        assert!(take_fresh_arrivals(Err::<&[String], _>(()), &mut offered).is_empty());
        assert_eq!(offered, vec![id.clone()]);
        assert!(
            take_fresh_arrivals(Ok::<_, ()>(std::slice::from_ref(&id)), &mut offered).is_empty()
        );
        assert!(take_fresh_arrivals(Ok::<_, ()>(&[]), &mut offered).is_empty());
        assert_eq!(
            take_fresh_arrivals(Ok::<_, ()>(std::slice::from_ref(&id)), &mut offered),
            vec![id]
        );
    }

    #[test]
    fn arrival_during_restore_waits_for_first_result_then_runs_once() {
        let (requests, pending) = mpsc::sync_channel(1);
        let (results, delivered) = mpsc::channel();
        let (started, entered) = mpsc::channel();
        let (release, resume) = mpsc::channel();
        let worker = thread::spawn(move || {
            let mut calls = 0;
            restore_worker(
                pending,
                results,
                || true,
                || {
                    calls += 1;
                    if calls == 1 {
                        started.send(()).unwrap();
                        resume.recv().unwrap();
                    }
                    Ok(calls)
                },
                || true,
            );
            calls
        });

        requests.try_send(()).unwrap();
        entered.recv_timeout(Duration::from_secs(2)).unwrap();
        requests.try_send(()).unwrap();
        assert!(matches!(
            delivered.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        release.send(()).unwrap();
        assert_eq!(
            delivered.recv_timeout(Duration::from_secs(2)).unwrap(),
            Ok(1)
        );
        assert_eq!(
            delivered.recv_timeout(Duration::from_secs(2)).unwrap(),
            Ok(2)
        );
        drop(requests);
        assert_eq!(worker.join().unwrap(), 2);
    }

    #[test]
    fn declined_prompt_discards_arrivals_queued_during_the_prompt() {
        let (requests, pending) = mpsc::sync_channel(1);
        let (results, delivered) = mpsc::channel();
        let (started, entered) = mpsc::channel();
        let (release, resume) = mpsc::channel();
        let worker = thread::spawn(move || {
            let mut calls = 0;
            restore_worker(
                pending,
                results,
                || true,
                || {
                    calls += 1;
                    if calls == 1 {
                        started.send(()).unwrap();
                        resume.recv().unwrap();
                    }
                    Err("declined".to_owned())
                },
                || true,
            );
            calls
        });
        requests.try_send(()).unwrap();
        entered.recv_timeout(Duration::from_secs(2)).unwrap();
        requests.try_send(()).unwrap();
        release.send(()).unwrap();
        assert_eq!(
            delivered.recv_timeout(Duration::from_secs(2)).unwrap(),
            Err("declined".to_owned())
        );
        drop(requests);
        assert_eq!(worker.join().unwrap(), 1);
        assert!(matches!(
            delivered.try_recv(),
            Err(mpsc::TryRecvError::Disconnected)
        ));
    }

    #[test]
    fn closed_result_receiver_stops_worker_before_queued_restore() {
        let (requests, pending) = mpsc::sync_channel(1);
        let (results, delivered) = mpsc::channel();
        let (started, entered) = mpsc::channel();
        let (release, resume) = mpsc::channel();
        let worker = thread::spawn(move || {
            let mut calls = 0;
            restore_worker(
                pending,
                results,
                || true,
                || {
                    calls += 1;
                    if calls == 1 {
                        started.send(()).unwrap();
                        resume.recv().unwrap();
                    }
                    Ok(calls)
                },
                || true,
            );
            calls
        });
        requests.try_send(()).unwrap();
        entered.recv_timeout(Duration::from_secs(2)).unwrap();
        requests.try_send(()).unwrap();
        drop(delivered);
        release.send(()).unwrap();
        assert_eq!(worker.join().unwrap(), 1);
        drop(requests);
    }

    #[test]
    fn failed_post_keeps_result_owned_and_worker_available() {
        let (requests, pending) = mpsc::sync_channel(1);
        let (results, delivered) = mpsc::channel();
        let (started, entered) = mpsc::channel();
        let (release, resume) = mpsc::channel();
        let worker = thread::spawn(move || {
            let mut calls = 0;
            restore_worker(
                pending,
                results,
                || true,
                || {
                    calls += 1;
                    if calls == 1 {
                        started.send(()).unwrap();
                        resume.recv().unwrap();
                    }
                    Ok(calls)
                },
                || false,
            );
            calls
        });
        requests.try_send(()).unwrap();
        entered.recv_timeout(Duration::from_secs(2)).unwrap();
        release.send(()).unwrap();
        assert_eq!(
            delivered.recv_timeout(Duration::from_secs(2)).unwrap(),
            Ok(1)
        );
        requests.try_send(()).unwrap();
        assert_eq!(
            delivered.recv_timeout(Duration::from_secs(2)).unwrap(),
            Ok(2)
        );
        drop(requests);
        assert_eq!(worker.join().unwrap(), 2);
    }

    #[test]
    fn closed_window_skips_pending_restore_without_invoking_privacy() {
        let (requests, pending) = mpsc::sync_channel(1);
        let (results, delivered) = mpsc::channel();
        let worker = thread::spawn(move || {
            let mut called = false;
            restore_worker(
                pending,
                results,
                || false,
                || {
                    called = true;
                    Ok(1)
                },
                || true,
            );
            called
        });
        requests.try_send(()).unwrap();
        assert!(!worker.join().unwrap());
        assert!(matches!(
            delivered.try_recv(),
            Err(mpsc::TryRecvError::Disconnected)
        ));
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
