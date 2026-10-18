use crate::{
    frontends::cli::Filter,
    i18n::Language,
    model::{
        Access, AccessEvent, Action, Activity, CollectorState, MicrophoneMuteState, Resource,
        SCHEMA_VERSION, Snapshot, event_code,
    },
    platform::{MicrophoneProtectionStatus, PlatformMonitor, SessionLockState},
    privacy::{CameraControlObservation, CameraPrivacyState},
    settings::{PrivacyProfile, Settings},
};
use anyhow::{Context, Result};
use parking_lot::Mutex;
use std::{
    collections::HashMap,
    ffi::OsStr,
    mem::size_of,
    os::windows::ffi::OsStrExt,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicIsize, AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread,
    time::{Duration, Instant},
};
use windows::{
    Win32::{
        Foundation::{
            CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, HWND, LPARAM, LRESULT, POINT,
            RECT, SIZE, WPARAM,
        },
        Graphics::Gdi::{
            BI_RGB, BITMAPINFO, BITMAPINFOHEADER, COLOR_WINDOW, CreateBitmap, CreateDIBSection,
            CreateFontIndirectW, DIB_RGB_COLORS, DeleteObject, GetDC, GetMonitorInfoW,
            GetSysColorBrush, GetTextExtentPoint32W, HFONT, MONITOR_DEFAULTTONEAREST, MONITORINFO,
            MonitorFromWindow, ReleaseDC, SelectObject,
        },
        System::Threading::{CreateMutexW, GetCurrentProcessId},
        UI::{
            Shell::{
                NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
                Shell_NotifyIconW,
            },
            WindowsAndMessaging::{
                AppendMenuW, CREATESTRUCTW, CreateIconIndirect, CreatePopupMenu, CreateWindowExW,
                DefWindowProcW, DestroyIcon, DestroyMenu, DestroyWindow, DispatchMessageW,
                ES_AUTOVSCROLL, ES_MULTILINE, ES_READONLY, FindWindowW, GWLP_USERDATA,
                GetClientRect, GetCursorPos, GetDlgItem, GetMessageW, GetSystemMetrics,
                GetWindowLongPtrW, HICON, HMENU, ICONINFO, KillTimer, MF_DISABLED, MF_GRAYED,
                MF_SEPARATOR, MF_STRING, MSG, MoveWindow, NONCLIENTMETRICSW, PostMessageW,
                PostQuitMessage, RegisterClassW, SM_CXSMICON, SPI_GETNONCLIENTMETRICS, SW_SHOW,
                SendMessageW, SetForegroundWindow, SetTimer, SetWindowLongPtrW, SetWindowTextW,
                ShowWindow, TPM_BOTTOMALIGN, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON,
                TrackPopupMenu, TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP, WM_CLOSE,
                WM_COMMAND, WM_COPY, WM_DESTROY, WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_NCCREATE,
                WM_NULL, WM_RBUTTONUP, WM_SETFONT, WM_SIZE, WM_TIMER, WNDCLASSW, WS_CHILD,
                WS_EX_CLIENTEDGE, WS_OVERLAPPED, WS_OVERLAPPEDWINDOW, WS_TABSTOP, WS_VISIBLE,
                WS_VSCROLL,
            },
        },
    },
    core::PCWSTR,
};

#[path = "tray/windows_audio.rs"]
mod microphone_worker;

#[link(name = "user32")]
unsafe extern "system" {
    fn SetProcessDpiAwarenessContext(context: isize) -> i32;
    fn GetDpiForWindow(hwnd: HWND) -> u32;
    fn SystemParametersInfoForDpi(
        action: u32,
        size: u32,
        data: *mut NONCLIENTMETRICSW,
        flags: u32,
        dpi: u32,
    ) -> i32;
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
/// Cooperative updater shutdown. Older trays do not acknowledge this message.
pub(crate) const WM_UPDATE_STOP: u32 = WM_APP + 41;
pub(crate) const UPDATE_STOPPED: usize = 0x4d435731;
pub(crate) const UPDATE_BUSY: usize = 0x4d435732;
const TIMER_POLL_ID: usize = 1;
const CAMERA_POLL_INTERVAL: Duration = Duration::from_secs(5);
const CMD_TOGGLE_MUTE: usize = 101;
const CMD_TOGGLE_CAMERA: usize = 102;
const CMD_EXIT: usize = 103;
const CMD_TOGGLE_NOTIFICATIONS: usize = 104;
const CMD_TOGGLE_AUTOSTART: usize = 105;
const CMD_CYCLE_PROFILE: usize = 106;
const CMD_DETAILS: usize = 107;
const DETAILS_EDIT: i32 = 201;
const DETAILS_COPY: usize = 202;
// Standard EDIT messages (no common-controls dependency).
const EM_SETSEL: u32 = 0x00b1;
const EM_SETLIMITTEXT: u32 = 0x00c5;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TrayVisual {
    Idle,
    Ready,
    Active,
    Partial,
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
    microphone: Result<MicrophoneProtectionStatus, String>,
    microphone_generation: u64,
    camera: Result<CameraControlObservation, String>,
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

type CameraResult = (CameraRequest, Result<CameraControlObservation, String>);
type CameraWorker = (SyncSender<CameraRequest>, Receiver<CameraResult>);

#[derive(Debug, Default)]
struct RestoreActivity {
    pending: usize,
    closing: bool,
}

#[derive(Debug)]
struct RestorePermit(Arc<Mutex<RestoreActivity>>);

impl Drop for RestorePermit {
    fn drop(&mut self) {
        self.0.lock().pending -= 1;
    }
}

type RestoreResult = (RestorePermit, Result<usize, String>);

struct CameraRefreshSignals {
    dirty: Arc<AtomicBool>,
    sequence: Arc<AtomicU64>,
}

#[derive(Default)]
struct PrivacyOperationErrors {
    microphone: [Option<String>; 2],
    camera: [Option<String>; 2],
}

impl PrivacyOperationErrors {
    fn slots(&mut self, resource: Resource) -> &mut [Option<String>; 2] {
        match resource {
            Resource::Microphone => &mut self.microphone,
            Resource::Camera => &mut self.camera,
        }
    }

    fn failed(&mut self, resource: Resource, manual: bool, message: String) {
        self.slots(resource)[usize::from(manual)] = Some(message);
    }

    fn completed(&mut self, resource: Resource, manual: bool) {
        let slots = self.slots(resource);
        // An explicit successful action supersedes automatic policy; automatic
        // success cannot acknowledge a failed explicit request or another device.
        slots[0] = None;
        if manual {
            slots[1] = None;
        }
    }

    fn messages(&self) -> impl Iterator<Item = &str> {
        self.microphone
            .iter()
            .chain(&self.camera)
            .filter_map(Option::as_deref)
    }
}

struct TrayAppState {
    lang: Language,
    visual: TrayVisual,
    summary: String,
    mute_state: MicrophoneMuteState,
    camera_state: CameraPrivacyState,
    microphone_protection: Option<MicrophoneProtectionStatus>,
    camera_observation: Option<CameraControlObservation>,
    microphone_status_error: Option<String>,
    camera_status_error: Option<String>,
    autostart_state: crate::autostart::AutostartState,
    settings: Settings,
    lock_state: SessionLockState,
    microphone_worker: microphone_worker::Worker,
    pending_microphone: usize,
    microphone_generation: u64,
    operation_errors: PrivacyOperationErrors,
    restore_camera: Option<CameraPrivacyState>,
    previous_accesses: HashMap<String, Access>,
    green_icon: HICON,
    yellow_icon: HICON,
    red_icon: HICON,
    // Amber activity glyph distinguishes partial coverage from Ready pause bars.
    partial_icon: HICON,
    gray_icon: HICON,
    refreshes: Receiver<TrayRefresh>,
    camera_dirty: Arc<AtomicBool>,
    camera_sequence: Arc<AtomicU64>,
    camera_generation: u64,
    hwnd_cell: Arc<AtomicIsize>,
    restore_results: Receiver<RestoreResult>,
    camera_requests: SyncSender<CameraRequest>,
    camera_results: Receiver<CameraResult>,
    camera_next_id: u64,
    manual_generation: u64,
    pending_manual: usize,
    deferred_lock_block: bool,
    pending_lock_block: Option<u64>,
    pending_lock_restore: Option<u64>,
    restore_activity: Arc<Mutex<RestoreActivity>>,
    workers_alive: Arc<AtomicBool>,
    notifications: NotificationWorker,
}

impl Drop for TrayAppState {
    fn drop(&mut self) {
        self.workers_alive.store(false, Ordering::Release);
        unsafe {
            let _ = DestroyIcon(self.green_icon);
            let _ = DestroyIcon(self.yellow_icon);
            let _ = DestroyIcon(self.red_icon);
            let _ = DestroyIcon(self.partial_icon);
            let _ = DestroyIcon(self.gray_icon);
        }
    }
}

fn start_refresh_worker(
    hwnd_cell: Arc<AtomicIsize>,
    workers_alive: Arc<AtomicBool>,
    camera: CameraRefreshSignals,
    microphone_sequence: Arc<AtomicU64>,
    restore_requests: SyncSender<RestorePermit>,
    restore_activity: Arc<Mutex<RestoreActivity>>,
    policy: crate::config::Policy,
) -> Receiver<TrayRefresh> {
    let CameraRefreshSignals {
        dirty: camera_dirty,
        sequence: camera_sequence,
    } = camera;
    // Blocking is confined to this collector worker. Preserve every observed
    // access transition and lock edge; coalescing arbitrary snapshots loses both.
    let (sender, receiver) = mpsc::sync_channel(2);
    thread::spawn(move || {
        let resume_error = crate::platform::resume_requested_microphone_protection()
            .err()
            .map(|error| format!("{error:#}"));
        let monitor = match PlatformMonitor::new(policy) {
            Ok(monitor) => monitor,
            Err(error) => {
                let _ = sender.send(TrayRefresh {
                    snapshot: Err(format!("{error:#}")),
                    microphone: Err(format!("{error:#}")),
                    microphone_generation: 0,
                    camera: Err("Camera observation unavailable".to_owned()),
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
        let mut camera = crate::privacy::camera_observation().map_err(|error| format!("{error:#}"));
        let mut last_camera_poll = Some(Instant::now());
        // Camera instance IDs already offered for restoration. Dropping an entry when
        // the device disappears is what re-arms the prompt after a replug, and it keeps
        // a declined UAC prompt from being raised again for the same arrival.
        let mut restore_offered: Vec<String> = Vec::new();
        while workers_alive.load(Ordering::Acquire) {
            let snapshot = monitor
                .snapshot((&filter).into())
                .map_err(|error| format!("{error:#}"));
            let microphone_generation = microphone_sequence.load(Ordering::Acquire);
            let microphone = monitor
                .microphone_protection_status()
                .map_err(|error| format!("{error:#}"))
                .map(|mut status| {
                    if let Some(error) = &resume_error {
                        status.detail = Some(format!(
                            "Resume failed: {error}; {}",
                            status.detail.as_deref().unwrap_or("")
                        ));
                    }
                    status
                });
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
                camera = crate::privacy::camera_observation().map_err(|error| format!("{error:#}"));
                camera_generation = generation;
                last_camera_poll = Some(now);
            }
            if enqueue_refresh(
                &sender,
                TrayRefresh {
                    snapshot,
                    microphone,
                    microphone_generation,
                    camera: camera.clone(),
                    camera_generation,
                    lock_state,
                },
            )
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
                let mut activity = restore_activity.lock();
                if !activity.closing {
                    activity.pending += 1;
                    drop(activity);
                    let _ = restore_requests.try_send(RestorePermit(Arc::clone(&restore_activity)));
                }
            }
            thread::sleep(Duration::from_millis(500));
        }
    });
    receiver
}

fn enqueue_refresh<T>(sender: &SyncSender<T>, refresh: T) -> Result<(), mpsc::SendError<T>> {
    match sender.try_send(refresh) {
        Ok(()) => Ok(()),
        Err(mpsc::TrySendError::Full(refresh)) => {
            eprintln!(
                "tray refresh queue saturated; collector backpressure delays observation cadence (retained lock/access transitions are not discarded)"
            );
            sender.send(refresh)
        }
        Err(mpsc::TrySendError::Disconnected(refresh)) => Err(mpsc::SendError(refresh)),
    }
}

fn next_refresh<T>(receiver: &Receiver<T>, pending_microphone: usize) -> Option<T> {
    if pending_microphone >= 8 {
        None
    } else {
        receiver.try_recv().ok()
    }
}

pub fn run_tray(
    _monitor: PlatformMonitor,
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
    let restore_activity = Arc::new(Mutex::new(RestoreActivity::default()));
    let workers_alive = Arc::new(AtomicBool::new(true));
    let (restore_requests, restore_results) = start_restore_worker(Arc::clone(&hwnd_cell));
    let (camera_requests, camera_results) = start_camera_worker(Arc::clone(&hwnd_cell));
    let microphone_worker =
        microphone_worker::Worker::start(policy.clone(), Arc::clone(&hwnd_cell));
    let refreshes = start_refresh_worker(
        Arc::clone(&hwnd_cell),
        Arc::clone(&workers_alive),
        CameraRefreshSignals {
            dirty: Arc::clone(&camera_dirty),
            sequence: Arc::clone(&camera_sequence),
        },
        Arc::clone(&microphone_worker.sequence),
        restore_requests,
        Arc::clone(&restore_activity),
        policy,
    );
    // The shell downsamples anything larger than the small-icon metric, which is what
    // made the icons look muddy. Render at the size the notification area will use.
    let icon_size = unsafe { GetSystemMetrics(SM_CXSMICON) }.max(16);

    let state = Box::new(TrayAppState {
        mute_state: MicrophoneMuteState::Unavailable,
        camera_state: CameraPrivacyState::SystemManaged,
        microphone_protection: None,
        camera_observation: None,
        microphone_status_error: None,
        camera_status_error: None,
        autostart_state: crate::autostart::state()
            .unwrap_or(crate::autostart::AutostartState::Disabled),
        lang,
        // NIM_ADD can become Explorer's cached accessible icon identity. Before
        // the first observation, brand neutrally rather than asserting inactivity.
        visual: TrayVisual::Error,
        summary: "miccamwatch".to_owned(),
        settings,
        lock_state: crate::platform::session_lock_state(),
        microphone_worker,
        pending_microphone: 0,
        microphone_generation: 0,
        operation_errors: PrivacyOperationErrors::default(),
        refreshes,
        camera_dirty,
        camera_sequence,
        camera_generation: 0,
        hwnd_cell: Arc::clone(&hwnd_cell),
        restore_results,
        restore_activity,
        restore_camera: None,
        camera_requests,
        camera_results,
        camera_next_id: 0,
        manual_generation: 0,
        pending_manual: 0,
        deferred_lock_block: false,
        pending_lock_block: None,
        pending_lock_restore: None,
        workers_alive,
        notifications: NotificationWorker::start(),
        previous_accesses: HashMap::new(),
        green_icon: create_status_icon((34, 197, 94), IconGlyph::Check, icon_size)?,
        yellow_icon: create_status_icon((245, 158, 11), IconGlyph::Ready, icon_size)?,
        red_icon: create_status_icon((239, 68, 68), IconGlyph::Active, icon_size)?,
        partial_icon: create_status_icon((245, 158, 11), IconGlyph::Active, icon_size)?,
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
    let nid = notify_icon_data(hwnd, state.gray_icon, &state.summary);
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
    crate::updater::stop_installed_tray()
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
        WM_UPDATE_STOP => {
            if wparam.0 != unsafe { GetCurrentProcessId() } as usize {
                return LRESULT(0);
            }
            LRESULT(request_safe_close(hwnd) as isize)
        }
        WM_CLOSE => LRESULT(request_safe_close(hwnd) as isize),
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
                CMD_DETAILS => {
                    if let Some(state) = state(hwnd)
                        && let Err(error) = show_diagnostic_window(hwnd, &state.summary)
                    {
                        notify_async(state, "MicCamWatch details", &format!("{error:#}"));
                    }
                }
                CMD_EXIT if request_safe_close(hwnd) == UPDATE_BUSY => {
                    if let Some(state) = state(hwnd) {
                        notify_async(
                            state,
                            "MicCamWatch",
                            "Finish pending privacy operations or UAC prompts before exiting. Nothing was cancelled.",
                        );
                    }
                }
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

fn request_safe_close(hwnd: HWND) -> usize {
    let Some(state) = state(hwnd) else {
        return UPDATE_BUSY;
    };
    let mut activity = state.restore_activity.lock();
    if state.pending_microphone != 0
        || !can_close_tray(
            state.pending_manual,
            state.pending_lock_block.is_some(),
            state.pending_lock_restore.is_some(),
            activity.pending,
        )
    {
        return UPDATE_BUSY;
    }
    activity.closing = true;
    drop(activity);
    unsafe {
        let _ = DestroyWindow(hwnd);
    }
    UPDATE_STOPPED
}

fn can_close_tray(manual: usize, block: bool, restore: bool, arrivals: usize) -> bool {
    manual == 0 && !block && !restore && arrivals == 0
}

fn start_camera_worker(hwnd_cell: Arc<AtomicIsize>) -> CameraWorker {
    let (requests, pending) = mpsc::sync_channel(8);
    // Results are effects, not snapshots. Backpressure never discards ownership.
    let (results, delivered) = mpsc::sync_channel(8);
    thread::spawn(move || {
        camera_worker(
            pending,
            results,
            |request| {
                (|| {
                    match request {
                        CameraRequest::Manual => {
                            crate::privacy::toggle_camera()?;
                        }
                        CameraRequest::Block { .. } => {
                            crate::privacy::set_camera_state(CameraPrivacyState::Blocked)?;
                        }
                        CameraRequest::Restore { previous, .. } => {
                            crate::privacy::set_camera_state(previous)?;
                        }
                    }
                    crate::privacy::camera_observation()
                })()
                .map_err(|error: anyhow::Error| format!("{error:#}"))
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

fn camera_worker<T>(
    requests: Receiver<CameraRequest>,
    results: SyncSender<(CameraRequest, Result<T, String>)>,
    mut execute: impl FnMut(CameraRequest) -> Result<T, String>,
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
        .try_send(CameraRequest::Restore { id, previous })
        .is_ok()
    {
        state.pending_lock_restore = Some(id);
    } else {
        camera_queue_error(state);
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
        .try_send(CameraRequest::Block {
            id,
            previous,
            manual_generation: state.manual_generation,
        })
        .is_ok()
    {
        state.pending_lock_block = Some(id);
    } else {
        camera_queue_error(state);
    }
}

fn apply_lock_policy(state: &mut TrayAppState, current: SessionLockState) {
    if current == state.lock_state {
        return;
    }
    if current == SessionLockState::Locked {
        if state.settings.mute_on_lock {
            queue_microphone(state, microphone_worker::Request::Lock);
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
            queue_microphone(state, microphone_worker::Request::Unlock);
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

const NOTIFICATION_QUEUE_CAPACITY: usize = 32;
#[derive(Default)]
struct NotificationHealth {
    undelivered: AtomicU64,
    unavailable: AtomicBool,
}
impl NotificationHealth {
    fn failed(&self) {
        self.undelivered.fetch_add(1, Ordering::Relaxed);
        self.unavailable.store(true, Ordering::Relaxed);
    }
    fn delivered(&self) {
        self.unavailable.store(false, Ordering::Relaxed);
    }
}
struct NotificationWorker {
    sender: SyncSender<(String, String)>,
    health: Arc<NotificationHealth>,
}
impl NotificationWorker {
    fn start() -> Self {
        let (sender, pending) = mpsc::sync_channel::<(String, String)>(NOTIFICATION_QUEUE_CAPACITY);
        let health = Arc::new(NotificationHealth::default());
        let delivery = Arc::clone(&health);
        thread::spawn(move || {
            // A single shell round-trip at a time. Dropping the sender ends this
            // finite worker after accepted toasts are drained.
            while let Ok((title, message)) = pending.recv() {
                if let Err(error) = crate::notify::notify_message(&title, &message) {
                    delivery.failed();
                    eprintln!("notification failed: {error}");
                } else {
                    // This is positive delivery evidence, not deletion of loss history.
                    delivery.delivered();
                }
            }
        });
        Self { sender, health }
    }
    fn submit(&self, title: &str, message: &str) -> bool {
        if self
            .sender
            .try_send((title.to_owned(), message.to_owned()))
            .is_err()
        {
            self.health.failed();
            eprintln!(
                "notification queue overloaded/unavailable; toast not delivered: {title}: {message}"
            );
            false
        } else {
            true
        }
    }
}
fn notify_async(state: &mut TrayAppState, title: &str, message: &str) {
    if !state.notifications.submit(title, message) {
        state.visual = TrayVisual::Error;
        state.summary = format!(
            "miccamwatch: notification queue overloaded/unavailable ({} undelivered toasts)",
            state
                .notifications
                .health
                .undelivered
                .load(Ordering::Relaxed)
        );
    }
}

fn accepts_control_refresh(
    refresh_generation: u64,
    completed_generation: u64,
    pending: usize,
) -> bool {
    refresh_generation == completed_generation && pending == 0
}

fn refresh_state(hwnd: HWND, state: &mut TrayAppState) {
    // Consume each retained observation, then display the newest. Intermediate
    // lock/unlock and capture transitions carry effects and cannot be skipped.
    while let Some(refresh) = next_refresh(&state.refreshes, state.pending_microphone) {
        apply_refresh(hwnd, state, refresh);
    }
}

fn apply_refresh(hwnd: HWND, state: &mut TrayAppState, refresh: TrayRefresh) {
    let old_camera_state = state.camera_state;
    if accepts_control_refresh(
        refresh.camera_generation,
        state.camera_generation,
        state.pending_manual
            + usize::from(state.pending_lock_block.is_some())
            + usize::from(state.pending_lock_restore.is_some()),
    ) {
        match refresh.camera {
            Ok(camera) => {
                state.camera_state = if camera.desired_blocked {
                    CameraPrivacyState::Blocked
                } else {
                    CameraPrivacyState::Allowed
                };
                state.camera_observation = Some(camera);
                state.camera_status_error = None;
            }
            Err(error) => state.camera_status_error = Some(error),
        }
    }
    let camera_state = state.camera_state;
    if accepts_control_refresh(
        refresh.microphone_generation,
        state.microphone_generation,
        state.pending_microphone,
    ) {
        match refresh.microphone {
            Ok(status) => {
                state.mute_state = status.mute_state;
                state.microphone_protection = Some(status);
                state.microphone_status_error = None;
            }
            Err(error) => state.microphone_status_error = Some(error),
        }
    }
    apply_lock_policy(state, refresh.lock_state);
    if let Ok(snapshot) = &refresh.snapshot {
        record_history_changes(state, snapshot);
    }
    let (visual, summary) = refreshed_status(
        refresh.snapshot.as_ref().map_err(String::as_str),
        TrayStatus {
            lang: state.lang,
            microphone: state.microphone_protection.as_ref(),
            camera: state.camera_observation.as_ref(),
            observation_errors: [
                state.microphone_status_error.as_deref(),
                state.camera_status_error.as_deref(),
            ],
            operation_errors: &state.operation_errors,
            notifications: &state.notifications.health,
        },
    );
    let mute_state = state.mute_state;
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
        TrayVisual::Partial => state.partial_icon,
        TrayVisual::Error => state.gray_icon,
    };
    let nid = notify_icon_data(hwnd, icon, &state.summary);
    if !unsafe { Shell_NotifyIconW(NIM_MODIFY, &nid) }.as_bool() {
        state.visual = TrayVisual::Error;
        state
            .summary
            .insert_str(0, "miccamwatch: failed to update tray icon\n");
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

struct TrayStatus<'a> {
    lang: Language,
    microphone: Option<&'a MicrophoneProtectionStatus>,
    camera: Option<&'a CameraControlObservation>,
    observation_errors: [Option<&'a str>; 2],
    operation_errors: &'a PrivacyOperationErrors,
    notifications: &'a NotificationHealth,
}

fn refreshed_status(
    snapshot: Result<&Snapshot, &str>,
    status: TrayStatus<'_>,
) -> (TrayVisual, String) {
    use std::fmt::Write;

    let notification_unavailable = status.notifications.unavailable.load(Ordering::Relaxed);
    let protection_failed = status.microphone.is_none_or(|microphone| {
        microphone.mute_state == MicrophoneMuteState::Unavailable
            || (microphone.requested
                && (!microphone.service_active
                    || microphone.mute_state != MicrophoneMuteState::Muted))
    }) || status.camera.is_none_or(|camera| {
        (camera.desired_blocked && !camera.helper_active) || camera.unknown_devices != 0
    }) || status.observation_errors.iter().any(Option::is_some)
        || status.operation_errors.messages().next().is_some()
        || notification_unavailable;
    let (mut visual, mut summary) = match snapshot {
        Ok(snapshot) => {
            let mut active_mic = 0;
            let mut active_camera = 0;
            let mut ready = 0;
            for access in &snapshot.accesses {
                match (access.activity, access.resource) {
                    (Activity::Active, Resource::Microphone) => active_mic += 1,
                    (Activity::Active, Resource::Camera) => active_camera += 1,
                    (Activity::Ready, _) => ready += 1,
                }
            }
            let active = active_mic + active_camera;
            let unavailable = snapshot
                .collectors
                .iter()
                .any(|collector| collector.state == CollectorState::Unavailable);
            let partial = !snapshot.observation_gaps.is_empty()
                || snapshot
                    .collectors
                    .iter()
                    .any(|collector| collector.state == CollectorState::Degraded);
            let visual = if unavailable || protection_failed {
                TrayVisual::Error
            } else if partial {
                TrayVisual::Partial
            } else if active != 0 {
                TrayVisual::Active
            } else if ready != 0 {
                TrayVisual::Ready
            } else {
                TrayVisual::Idle
            };
            // Keep the popup/tooltip first line independent of application name length.
            // An incomplete scan reports observed counts, never a claim of global idle.
            let mut summary =
                if active != 0 || ready != 0 || unavailable || partial || protection_failed {
                    format!(
                        "{}{active} active ({active_mic} Mic, {active_camera} Cam); {ready} ready",
                        if unavailable || protection_failed {
                            "Status error — "
                        } else if partial {
                            "Partial — "
                        } else {
                            ""
                        },
                    )
                } else {
                    idle_text(status.lang).to_owned()
                };
            for access in &snapshot.accesses {
                let activity = match access.activity {
                    Activity::Active => "Active",
                    Activity::Ready => "Ready (pipeline, not proven capture)",
                };
                let resource = match access.resource {
                    Resource::Microphone => "Microphone",
                    Resource::Camera => "Camera",
                };
                let _ = write!(summary, "\n{activity} {resource}: {}", access.application);
                match access.pid {
                    Some(pid) => {
                        let _ = write!(summary, "; PID {pid}");
                    }
                    None => summary.push_str("; PID unknown"),
                }
            }
            for collector in snapshot
                .collectors
                .iter()
                .filter(|collector| collector.state != CollectorState::Healthy)
            {
                let health = match collector.state {
                    CollectorState::Degraded => "degraded",
                    CollectorState::Unavailable => "unavailable",
                    CollectorState::Healthy => unreachable!(),
                };
                let _ = write!(
                    summary,
                    "\nCollector {} {health}: {}",
                    collector.collector,
                    collector.detail.as_deref().unwrap_or("reason not supplied"),
                );
            }
            for resource in &snapshot.observation_gaps {
                let _ = write!(
                    summary,
                    "\nObservation gap: {resource:?}; missing accesses cannot be confirmed stopped"
                );
            }
            (visual, summary)
        }
        Err(error) => (
            TrayVisual::Error,
            format!("miccamwatch: telemetry unavailable\nTelemetry error: {error}"),
        ),
    };
    if let Some(microphone) = status.microphone {
        let _ = write!(
            summary,
            "\n{}",
            crate::output::microphone_protection_summary(status.lang, microphone),
        );
        if microphone.mute_state == MicrophoneMuteState::Unavailable
            || (microphone.requested
                && (!microphone.service_active
                    || microphone.mute_state != MicrophoneMuteState::Muted))
        {
            visual = TrayVisual::Error;
        }
    } else {
        visual = TrayVisual::Error;
        let _ = write!(summary, "\n{}", status.lang.unknown_protection(true));
    }
    if let Some(camera) = status.camera {
        let _ = write!(
            summary,
            "\n{}",
            crate::output::camera_protection_summary(status.lang, camera),
        );
        if (camera.desired_blocked && !camera.helper_active) || camera.unknown_devices != 0 {
            visual = TrayVisual::Error;
        }
    } else {
        visual = TrayVisual::Error;
        let _ = write!(summary, "\n{}", status.lang.unknown_protection(false));
    }
    let _ = write!(summary, "\n{}", status.lang.protection_limit());
    for error in status.observation_errors.into_iter().flatten() {
        visual = TrayVisual::Error;
        let _ = write!(summary, "\nProtection observation failed: {error}");
    }
    for error in status.operation_errors.messages() {
        visual = TrayVisual::Error;
        let _ = write!(summary, "\nPrivacy operation failed/partial: {error}");
    }
    let undelivered = status.notifications.undelivered.load(Ordering::Relaxed);
    if undelivered != 0 {
        let _ = write!(
            summary,
            "\nNotification history: {undelivered} undelivered toasts"
        );
    }
    if notification_unavailable {
        visual = TrayVisual::Error;
        summary.push_str("\nNotification delivery currently overloaded/unavailable");
    }
    (visual, summary)
}

fn toggle_mute(hwnd: HWND) {
    let Some(state) = state(hwnd) else { return };
    // Reserve two accepted slots for lock policy. Rejected manual clicks have
    // no effects and are reported, rather than blocking the menu or disappearing.
    if state.pending_microphone < 6 {
        queue_microphone(state, microphone_worker::Request::Manual);
    } else {
        operation_error(state, Resource::Microphone, true, "Microphone operation queue busy; request not accepted, retry after pending operations finish".to_owned());
    }
}

fn queue_microphone(state: &mut TrayAppState, request: microphone_worker::Request) {
    if state.microphone_worker.requests.try_send(request).is_ok() {
        state.pending_microphone += 1;
    } else {
        operation_error(
            state,
            Resource::Microphone,
            matches!(request, microphone_worker::Request::Manual),
            "Microphone worker unavailable; request not accepted".to_owned(),
        );
    }
}

fn operation_error(state: &mut TrayAppState, resource: Resource, manual: bool, message: String) {
    state
        .operation_errors
        .failed(resource, manual, message.clone());
    state.summary = format!("miccamwatch: {message}");
    state.visual = TrayVisual::Error;
    notify_async(state, "MicCamWatch privacy operation", &message);
}

fn toggle_camera(hwnd: HWND) {
    let Some(state) = state(hwnd) else { return };
    if state.pending_manual < 6
        && state
            .camera_requests
            .try_send(CameraRequest::Manual)
            .is_ok()
    {
        state.manual_generation = state.manual_generation.wrapping_add(1);
        state.pending_manual += 1;
    } else {
        camera_queue_error(state);
    }
}

fn camera_queue_error(state: &mut TrayAppState) {
    operation_error(state, Resource::Camera, true, "Camera operation queue busy/unavailable; request not accepted, retry after pending operations finish".to_owned());
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
) -> (SyncSender<RestorePermit>, Receiver<RestoreResult>) {
    let (requests, pending) = mpsc::sync_channel(1);
    let (results, delivered) = mpsc::sync_channel(2);
    thread::spawn(move || {
        restore_worker(
            pending,
            results,
            || hwnd_cell.load(Ordering::Relaxed) != 0,
            // The request's RAII permit counts queued and active UAC operations.
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

fn restore_worker<T>(
    requests: Receiver<T>,
    results: SyncSender<(T, Result<usize, String>)>,
    alive: impl Fn() -> bool,
    mut restore: impl FnMut() -> Result<usize, String>,
    mut notify: impl FnMut() -> bool,
) {
    while let Ok(request) = requests.recv() {
        if !alive() {
            break;
        }
        let result = restore();
        if result.is_err() {
            // A declined UAC prompt must not be raised again just because a
            // different camera arrived during that prompt.
            while requests.try_recv().is_ok() {}
        }
        // Ownership spans queued work, active UAC and the unconsumed completion.
        if results.send((request, result)).is_err() {
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
    while let Ok((request, generation, result)) = state.microphone_worker.results.try_recv() {
        state.microphone_generation = generation;
        state.pending_microphone = state.pending_microphone.saturating_sub(1);
        match result {
            Ok(actual) => {
                state.operation_errors.completed(
                    Resource::Microphone,
                    matches!(request, microphone_worker::Request::Manual),
                );
                state.mute_state = actual.mute_state;
                let message = crate::output::microphone_protection_summary(state.lang, &actual);
                state.microphone_protection = Some(actual);
                state.microphone_status_error = None;
                if matches!(request, microphone_worker::Request::Manual) {
                    notify_async(state, "MicCamWatch", &message);
                }
            }
            Err(error) => operation_error(
                state,
                Resource::Microphone,
                matches!(request, microphone_worker::Request::Manual),
                format!("microphone {request:?} failed/partial: {error}"),
            ),
        }
    }
    while let Ok((request, result)) = state.camera_results.try_recv() {
        handle_camera_operation_result(state, request, result);
    }
    while let Ok((_permit, result)) = state.restore_results.try_recv() {
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
    result: Result<CameraControlObservation, String>,
) {
    state.camera_generation = state.camera_generation.wrapping_add(1);
    state
        .camera_sequence
        .store(state.camera_generation, Ordering::Release);
    state.camera_dirty.store(true, Ordering::Release);
    let result = result.map(|observation| {
        let desired = if observation.desired_blocked {
            CameraPrivacyState::Blocked
        } else {
            CameraPrivacyState::Allowed
        };
        state.camera_observation = Some(observation);
        state
            .operation_errors
            .completed(Resource::Camera, matches!(request, CameraRequest::Manual));
        state.camera_status_error = None;
        desired
    });
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
                    state
                        .operation_errors
                        .failed(Resource::Camera, false, message.clone());
                    state.summary = format!("miccamwatch: {message}");
                    state.visual = TrayVisual::Error;
                    notify_async(state, "MicCamWatch camera", &message);
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
                    state
                        .operation_errors
                        .failed(Resource::Camera, false, message.clone());
                    state.summary = format!("miccamwatch: {message}");
                    state.visual = TrayVisual::Error;
                    notify_async(state, "MicCamWatch camera", &message);
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
            if restored != 0 {
                state.operation_errors.completed(Resource::Camera, false);
            }
            let message = format!(
                "Reconnected camera restored ({restored} device{}).",
                if restored == 1 { "" } else { "s" }
            );
            notify_async(state, "MicCamWatch camera", &message);
        }
        Err(error) => {
            // Staying silent after a declined prompt matters more than reporting it,
            // so the failure only surfaces in the tray summary and tooltip.
            state.summary = format!("miccamwatch: camera restore failed: {error}");
            state.operation_errors.failed(
                Resource::Camera,
                false,
                format!("Camera restore failed: {error}"),
            );
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
            let message = state
                .camera_observation
                .as_ref()
                .map(|status| crate::output::camera_protection_summary(state.lang, status))
                .unwrap_or_else(|| "Camera protection observation unknown".to_owned());
            notify_async(state, "MicCamWatch camera", &message);
        }
        Err(error) => {
            let message = format!("Camera control failed: {error}");
            state
                .operation_errors
                .failed(Resource::Camera, true, message.clone());
            state.summary = format!("miccamwatch: {message}");
            state.visual = TrayVisual::Error;
            notify_async(state, "MicCamWatch camera", &message);
        }
    }
}

fn toggle_notifications(hwnd: HWND) {
    let Some(state) = state(hwnd) else { return };
    match Settings::update(|settings| {
        settings.pause_notifications_until = if settings.notifications_paused() {
            None
        } else {
            Some(chrono::Utc::now() + chrono::Duration::hours(1))
        };
    }) {
        Ok(settings) => state.settings = settings,
        Err(error) => {
            state.summary = format!("miccamwatch: settings update failed: {error:#}");
            notify_async(state, "MicCamWatch settings", &state.summary.clone());
        }
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
    match Settings::update(|settings| {
        settings.profile = match settings.profile {
            PrivacyProfile::Balanced => PrivacyProfile::Private,
            PrivacyProfile::Private => PrivacyProfile::Meeting,
            PrivacyProfile::Meeting => PrivacyProfile::Development,
            PrivacyProfile::Development => PrivacyProfile::Balanced,
        };
    }) {
        Ok(settings) => state.settings = settings,
        Err(error) => {
            state.summary = format!("miccamwatch: settings update failed: {error:#}");
            notify_async(state, "MicCamWatch settings", &state.summary.clone());
        }
    }
}

fn monitor_work_area(hwnd: HWND) -> RECT {
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    unsafe {
        let _ = GetMonitorInfoW(MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST), &mut info);
    }
    info.rcWork
}

fn native_menu_font(hwnd: HWND) -> Option<HFONT> {
    let mut metrics = NONCLIENTMETRICSW {
        cbSize: size_of::<NONCLIENTMETRICSW>() as u32,
        ..Default::default()
    };
    unsafe {
        if SystemParametersInfoForDpi(
            SPI_GETNONCLIENTMETRICS.0,
            metrics.cbSize,
            &mut metrics,
            0,
            GetDpiForWindow(hwnd).max(96),
        ) == 0
        {
            return None;
        }
        let font = CreateFontIndirectW(&metrics.lfMenuFont);
        (!font.is_invalid()).then_some(font)
    }
}

fn menu_summary_width(hwnd: HWND) -> i32 {
    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96) as i32;
    let work = monitor_work_area(hwnd);
    // Reserve native checkmark, submenu and border gutters separately from text.
    (200 * dpi / 96)
        .min((work.right - work.left) / 2 - 80 * dpi / 96)
        .max(1)
}

fn menu_plain_text(summary: &str) -> (String, bool) {
    // Protection diagnostics remain in Details, not flattened into a wide menu row.
    let mut chars = summary.lines().next().unwrap_or_default().chars();
    let text = chars.by_ref().take(256).map(|c| {
        if c.is_control() || c.is_whitespace()
            || matches!(c, '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        {
            ' '
        } else {
            c
        }
    }).collect();
    (text, chars.next().is_some())
}

fn compact_menu_summary(hwnd: HWND, summary: &str) -> String {
    let (plain, already_truncated) = menu_plain_text(summary);
    let Some(font) = native_menu_font(hwnd) else {
        return "Status details…".into();
    };
    let mut wide: Vec<u16> = plain.encode_utf16().collect();
    let limit = menu_summary_width(hwnd);
    let compact = unsafe {
        let dc = GetDC(Some(hwnd));
        if dc.is_invalid() {
            let _ = DeleteObject(font.into());
            return "Status details…".into();
        }
        let previous = SelectObject(dc, font.into());
        let measure = |text: &[u16]| {
            let mut size = SIZE::default();
            GetTextExtentPoint32W(dc, text, &mut size)
                .as_bool()
                .then_some(size.cx)
        };
        let result = if !already_truncated && measure(&wide).is_some_and(|width| width <= limit) {
            plain
        } else {
            wide.push('…' as u16);
            let mut boundaries = vec![0];
            let mut units = 0;
            for c in plain.chars() {
                units += c.len_utf16();
                boundaries.push(units);
            }
            let mut low = 0;
            let mut high = boundaries.len() - 1;
            while low < high {
                let middle = (low + high).div_ceil(2);
                let end = boundaries[middle];
                let saved = wide[end];
                wide[end] = '…' as u16;
                let fits = measure(&wide[..=end]).is_some_and(|width| width <= limit);
                wide[end] = saved;
                if fits {
                    low = middle;
                } else {
                    high = middle - 1;
                }
            }
            format!("{}…", String::from_utf16_lossy(&wide[..boundaries[low]]))
        };
        let _ = SelectObject(dc, previous);
        let _ = ReleaseDC(Some(hwnd), dc);
        let _ = DeleteObject(font.into());
        result
    };
    // AppendMenu interprets ampersands as mnemonics; double them only after measuring.
    compact.replace('&', "&&")
}

fn diagnostic_document(summary: &str) -> String {
    // Windows EDIT expects CRLF. NUL cannot be represented by its text API, so
    // expose it explicitly rather than silently discarding everything after it.
    let mut document = String::with_capacity(summary.len());
    let mut chars = summary.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\0' => document.push_str("\\0"),
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                document.push_str("\r\n");
            }
            '\n' => document.push_str("\r\n"),
            c => document.push(c),
        }
    }
    document
}

fn show_diagnostic_window(owner: HWND, summary: &str) -> Result<HWND> {
    let class = format_wide("MicCamWatchDetailsClass");
    let title = format_wide("MicCamWatch — status details");
    unsafe {
        let _ = RegisterClassW(&WNDCLASSW {
            hbrBackground: GetSysColorBrush(COLOR_WINDOW),
            lpfnWndProc: Some(details_wnd_proc),
            lpszClassName: PCWSTR(class.as_ptr()),
            ..Default::default()
        });
        let work = monitor_work_area(owner);
        let dpi = GetDpiForWindow(owner).max(96) as i32;
        let width = (640 * dpi / 96).min(work.right - work.left);
        let height = (420 * dpi / 96).min(work.bottom - work.top);
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(class.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_OVERLAPPEDWINDOW,
            work.left + (work.right - work.left - width) / 2,
            work.top + (work.bottom - work.top - height) / 2,
            width,
            height,
            Some(owner),
            None,
            None,
            None,
        )
        .context("create status details window")?;
        let result = (|| -> Result<()> {
            let font = native_menu_font(owner).context("load native status font")?;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, font.0 as isize);
            let edit_class = format_wide("EDIT");
            let edit = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                PCWSTR(edit_class.as_ptr()),
                PCWSTR::null(),
                WS_CHILD
                    | WS_VISIBLE
                    | WS_VSCROLL
                    | WS_TABSTOP
                    | WINDOW_STYLE((ES_MULTILINE | ES_READONLY | ES_AUTOVSCROLL) as u32),
                0,
                0,
                0,
                0,
                Some(hwnd),
                Some(HMENU(DETAILS_EDIT as *mut _)),
                None,
                None,
            )?;
            SendMessageW(edit, EM_SETLIMITTEXT, Some(WPARAM(0x7fff_fffe)), None);
            let document = format_wide(&diagnostic_document(summary));
            SetWindowTextW(edit, PCWSTR(document.as_ptr()))?;
            SendMessageW(
                edit,
                WM_SETFONT,
                Some(WPARAM(font.0 as usize)),
                Some(LPARAM(1)),
            );
            let button_class = format_wide("BUTTON");
            let copy_text = format_wide("Copy all");
            let copy = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                PCWSTR(button_class.as_ptr()),
                PCWSTR(copy_text.as_ptr()),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                0,
                0,
                0,
                0,
                Some(hwnd),
                Some(HMENU(DETAILS_COPY as *mut _)),
                None,
                None,
            )?;
            SendMessageW(
                copy,
                WM_SETFONT,
                Some(WPARAM(font.0 as usize)),
                Some(LPARAM(1)),
            );
            layout_details(hwnd);
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetForegroundWindow(hwnd);
            Ok(())
        })();
        if let Err(error) = result {
            let _ = DestroyWindow(hwnd);
            return Err(error).context("create status details controls");
        }
        Ok(hwnd)
    }
}

fn layout_details(hwnd: HWND) {
    unsafe {
        let mut rect = RECT::default();
        let _ = GetClientRect(hwnd, &mut rect);
        let unit = GetDpiForWindow(hwnd).max(96) as i32;
        let margin = 12 * unit / 96;
        let button_height = 30 * unit / 96;
        if let Ok(edit) = GetDlgItem(Some(hwnd), DETAILS_EDIT) {
            let _ = MoveWindow(
                edit,
                margin,
                margin,
                (rect.right - 2 * margin).max(1),
                (rect.bottom - 3 * margin - button_height).max(1),
                true,
            );
        }
        if let Ok(copy) = GetDlgItem(Some(hwnd), DETAILS_COPY as i32) {
            let _ = MoveWindow(
                copy,
                margin,
                (rect.bottom - margin - button_height).max(0),
                110 * unit / 96,
                button_height,
                true,
            );
        }
    }
}

unsafe extern "system" fn details_wnd_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_SIZE => {
            layout_details(hwnd);
            LRESULT(0)
        }
        WM_COMMAND if wparam.0 & 0xffff == DETAILS_COPY => {
            if let Ok(edit) = unsafe { GetDlgItem(Some(hwnd), DETAILS_EDIT) } {
                unsafe {
                    SendMessageW(edit, EM_SETSEL, Some(WPARAM(0)), Some(LPARAM(-1)));
                    SendMessageW(edit, WM_COPY, None, None);
                }
            }
            LRESULT(0)
        }
        // This modeless owned window must never quit/block the tray event loop:
        // pending camera results and updater shutdown safety continue unchanged.
        windows::Win32::UI::WindowsAndMessaging::WM_NCDESTROY => {
            let font = HFONT(unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut _);
            if !font.is_invalid() {
                let _ = unsafe { DeleteObject(font.into()) };
            }
            unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

fn camera_allow_label(lang: Language) -> &'static str {
    match lang {
        Language::En => "Allow all cameras",
        Language::Fr => "Autoriser toutes les caméras",
        Language::De => "Alle Kameras freigeben",
        Language::Es => "Permitir todas las cámaras",
        Language::Ja => "全カメラを許可",
        Language::Zh => "允许全部摄像头",
        Language::Ru => "Разрешить все камеры",
    }
}

fn camera_block_label(lang: Language) -> &'static str {
    match lang {
        Language::En => "Block all cameras",
        Language::Fr => "Bloquer toutes les caméras",
        Language::De => "Alle Kameras sperren",
        Language::Es => "Bloquear todas las cámaras",
        Language::Ja => "全カメラをブロック",
        Language::Zh => "阻止全部摄像头",
        Language::Ru => "Блокировать все камеры",
    }
}

fn build_context_menu(
    hwnd: HWND,
    summary: &str,
    lang: Language,
    microphone_requested: Option<bool>,
    camera_blocked: Option<bool>,
    settings: &Settings,
    autostart_state: crate::autostart::AutostartState,
) -> Result<HMENU> {
    unsafe {
        let menu = CreatePopupMenu()?;
        append_disabled(menu, &format!("miccamwatch v{}", env!("CARGO_PKG_VERSION")));
        append_disabled(menu, &compact_menu_summary(hwnd, summary));
        let details = format_wide("Status details…");
        let _ = AppendMenuW(menu, MF_STRING, CMD_DETAILS, PCWSTR(details.as_ptr()));
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let mute_text = microphone_requested.map_or(lang.unknown_protection(true), |requested| {
            lang.protection_action(requested)
        });
        let mute_wide = format_wide(&format!(
            "{}: {mute_text}",
            lang.resource(Resource::Microphone)
        ));
        let mute_flags = MF_STRING;
        let _ = AppendMenuW(
            menu,
            mute_flags,
            CMD_TOGGLE_MUTE,
            PCWSTR(mute_wide.as_ptr()),
        );
        let camera_text = format_wide(match camera_blocked {
            Some(true) => camera_allow_label(lang),
            Some(false) => camera_block_label(lang),
            None => lang.unknown_protection(false),
        });
        let _ = AppendMenuW(
            menu,
            MF_STRING,
            CMD_TOGGLE_CAMERA,
            PCWSTR(camera_text.as_ptr()),
        );
        let notifications = format_wide(if settings.notifications_paused() {
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
        let profile = format_wide(&format!("Profile: {:?} (change)", settings.profile));
        let _ = AppendMenuW(menu, MF_STRING, CMD_CYCLE_PROFILE, PCWSTR(profile.as_ptr()));
        let autostart = format_wide(match autostart_state {
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
        Ok(menu)
    }
}

fn show_context_menu(hwnd: HWND) {
    let Some(state) = state(hwnd) else { return };
    let mut point = POINT::default();
    unsafe {
        let _ = GetCursorPos(&mut point);
        // Align the hidden owner with the popup monitor before reading its DPI/font.
        let _ = MoveWindow(hwnd, point.x, point.y, 0, 0, false);
    }
    let Ok(menu) = build_context_menu(
        hwnd,
        &state.summary,
        state.lang,
        state
            .microphone_protection
            .as_ref()
            .map(|status| status.requested),
        state
            .camera_observation
            .as_ref()
            .map(|status| status.desired_blocked),
        &state.settings,
        state.autostart_state,
    ) else {
        return;
    };
    unsafe {
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
    notify_async(state, "miccamwatch", &state.summary.clone());
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
    // Only the current bounded headline belongs in the shell tooltip; full
    // application/collector/protection diagnostics remain in Status details.
    // Leave one UTF-16 unit for NUL and never copy half a surrogate pair.
    let mut length = 0;
    let headline = tooltip.lines().next().unwrap_or_default();
    let brand = if headline.starts_with("miccamwatch") {
        ""
    } else {
        "miccamwatch: "
    };
    for character in brand.chars().chain(headline.chars()) {
        if character == '\0' || length + character.len_utf16() >= data.szTip.len() {
            break;
        }
        let mut encoded = [0u16; 2];
        let units = character.encode_utf16(&mut encoded);
        data.szTip[length..length + units.len()].copy_from_slice(units);
        length += units.len();
    }
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

    struct StatusFixture {
        microphone: MicrophoneProtectionStatus,
        camera: CameraControlObservation,
        errors: PrivacyOperationErrors,
        notifications: NotificationHealth,
    }

    impl StatusFixture {
        fn healthy() -> Self {
            Self {
                microphone: MicrophoneProtectionStatus {
                    requested: false,
                    service_active: false,
                    mute_state: MicrophoneMuteState::Unmuted,
                    endpoint_count: 1,
                    hardware_mute_count: 0,
                    corrections: 0,
                    detail: Some("SDK reports software mute only".into()),
                },
                camera: CameraControlObservation {
                    desired_blocked: false,
                    state: CameraPrivacyState::Allowed,
                    present_total: 1,
                    owned_blocked_present: 0,
                    pending_restore: 0,
                    absent_owned: 0,
                    unknown_devices: 0,
                    helper_active: false,
                    detail: None,
                },
                errors: PrivacyOperationErrors::default(),
                notifications: NotificationHealth::default(),
            }
        }

        fn status(&self) -> TrayStatus<'_> {
            TrayStatus {
                lang: Language::En,
                microphone: Some(&self.microphone),
                camera: Some(&self.camera),
                observation_errors: [None, None],
                operation_errors: &self.errors,
                notifications: &self.notifications,
            }
        }
    }

    fn partial_activity_snapshot() -> Snapshot {
        let mut microphone = observed(Resource::Microphone, "microphone:zoom");
        microphone.application = "Zoom.exe".into();
        let mut camera = observed(Resource::Camera, "camera:unknown");
        camera.application = "unknown camera client".into();
        camera.pid = None;
        let mut ready = observed(Resource::Camera, "camera:ready");
        ready.application = "ready-only-client".repeat(100);
        ready.activity = Activity::Ready;
        Snapshot {
            collectors: vec![crate::model::CollectorHealth {
                collector: "sensor_activity",
                state: CollectorState::Degraded,
                detail: Some("streaming camera; native process identity unavailable".into()),
            }],
            accesses: vec![microphone, camera, ready],
            observation_gaps: vec![Resource::Camera],
        }
    }

    fn native_tip_text(data: &NOTIFYICONDATAW) -> String {
        assert_eq!(data.szTip.len(), 128);
        let end = data.szTip.iter().position(|unit| *unit == 0).unwrap();
        assert!(end <= 127);
        assert!(data.szTip[end..].iter().all(|unit| *unit == 0));
        String::from_utf16(&data.szTip[..end]).expect("shell tooltip must be valid UTF-16")
    }

    #[test]
    fn native_tooltip_reports_current_activity_health_and_idle_in_every_language() {
        let fixture = StatusFixture::healthy();
        for lang in [
            Language::En,
            Language::Fr,
            Language::De,
            Language::Es,
            Language::Ja,
            Language::Zh,
            Language::Ru,
        ] {
            let mut snapshot = partial_activity_snapshot();
            for expected in [
                TrayVisual::Partial,
                TrayVisual::Error,
                TrayVisual::Active,
                TrayVisual::Ready,
                TrayVisual::Idle,
            ] {
                match expected {
                    TrayVisual::Error => snapshot.collectors[0].state = CollectorState::Unavailable,
                    TrayVisual::Active => {
                        snapshot.collectors[0].state = CollectorState::Healthy;
                        snapshot.collectors[0].detail = None;
                        snapshot.observation_gaps.clear();
                    }
                    TrayVisual::Ready => {
                        snapshot.accesses.retain(|a| a.activity == Activity::Ready)
                    }
                    TrayVisual::Idle => snapshot.accesses.clear(),
                    TrayVisual::Partial => {}
                }
                let mut status = fixture.status();
                status.lang = lang;
                let (visual, summary) = refreshed_status(Ok(&snapshot), status);
                assert_eq!(visual, expected);
                let tooltip = native_tip_text(&notify_icon_data(
                    HWND::default(),
                    HICON::default(),
                    &summary,
                ));
                assert!(tooltip.starts_with("miccamwatch"));
                assert!(!tooltip.contains('\n'));
                assert!(!tooltip.contains("SDK reports"));
                if expected == TrayVisual::Idle {
                    assert_eq!(tooltip, idle_text(lang));
                } else {
                    assert!(!tooltip.contains(idle_text(lang)));
                    assert!(tooltip.contains(if expected == TrayVisual::Ready {
                        "0 active"
                    } else {
                        "2 active"
                    }));
                    if expected != TrayVisual::Ready {
                        assert!(tooltip.contains("1 Mic, 1 Cam"));
                    }
                }
                if expected == TrayVisual::Partial {
                    assert!(tooltip.contains("Partial"));
                } else if expected == TrayVisual::Error {
                    assert!(tooltip.contains("Status error"));
                } else {
                    assert!(!tooltip.contains("Partial"));
                    assert!(!tooltip.contains("Status error"));
                }
            }
            let mut status = fixture.status();
            status.lang = lang;
            let (_, summary) = refreshed_status(Err("worker unavailable"), status);
            let tooltip = native_tip_text(&notify_icon_data(
                HWND::default(),
                HICON::default(),
                &summary,
            ));
            assert_eq!(tooltip, "miccamwatch: telemetry unavailable");
            assert!(!tooltip.contains(idle_text(lang)));
            assert!(diagnostic_document(&summary).contains("worker unavailable"));
        }
        let startup = native_tip_text(&notify_icon_data(
            HWND::default(),
            HICON::default(),
            "miccamwatch",
        ));
        assert_eq!(startup, "miccamwatch");
    }

    #[test]
    fn native_tooltip_utf16_boundary_never_splits_surrogates_or_loses_terminator() {
        for prefix_units in [125, 126, 127, 128] {
            let prefix = format!("miccamwatch: {}", "a".repeat(prefix_units - 13));
            let input = format!("{prefix}🛡suffix\nprivate diagnostic");
            let tooltip =
                native_tip_text(&notify_icon_data(HWND::default(), HICON::default(), &input));
            let expected = if prefix_units <= 125 {
                format!("{prefix}🛡")
            } else {
                prefix.chars().take(127).collect()
            };
            assert_eq!(tooltip, expected);
        }
        let tooltip = native_tip_text(&notify_icon_data(
            HWND::default(),
            HICON::default(),
            "2 active\0false idle",
        ));
        assert_eq!(tooltip, "miccamwatch: 2 active");
    }

    #[test]
    fn partial_activity_details_keep_zoom_unknown_camera_ready_and_coverage() {
        let fixture = StatusFixture::healthy();
        let snapshot = partial_activity_snapshot();
        let (visual, summary) = refreshed_status(Ok(&snapshot), fixture.status());
        assert_eq!(visual, TrayVisual::Partial);
        let first_line = summary.lines().next().unwrap();
        assert!(first_line.len() < 128);
        assert!(first_line.contains("2 active"));
        assert!(first_line.contains("1 Mic, 1 Cam"));
        assert!(first_line.contains("1 ready"));
        let document = diagnostic_document(&summary);
        assert!(document.contains("Active Microphone: Zoom.exe; PID 123"));
        assert!(document.contains("Active Camera: unknown camera client; PID unknown"));
        assert!(!document.contains("Active Camera: Zoom"));
        assert!(document.contains("Ready (pipeline, not proven capture) Camera:"));
        assert!(document.contains(&snapshot.accesses[2].application));
        assert!(document.contains(snapshot.collectors[0].detail.as_deref().unwrap()));
        assert!(document.contains("sensor_activity degraded"));
        assert!(document.contains("Observation gap: Camera"));
        assert!(document.contains(fixture.microphone.detail.as_deref().unwrap()));
        assert!(document.contains(fixture.status().lang.protection_limit()));
    }

    #[test]
    fn unavailable_telemetry_never_reports_idle_and_keeps_other_known_accesses() {
        let fixture = StatusFixture::healthy();
        let mut snapshot = partial_activity_snapshot();
        snapshot.collectors.push(crate::model::CollectorHealth {
            collector: "wasapi",
            state: CollectorState::Unavailable,
            detail: Some("audio session enumeration denied".into()),
        });
        let (visual, summary) = refreshed_status(Ok(&snapshot), fixture.status());
        assert_eq!(visual, TrayVisual::Error);
        assert!(summary.contains("Zoom.exe; PID 123"));
        for collector in &snapshot.collectors {
            assert!(summary.contains(collector.detail.as_deref().unwrap()));
        }
        snapshot.accesses.clear();
        let (visual, summary) = refreshed_status(Ok(&snapshot), fixture.status());
        assert_eq!(visual, TrayVisual::Error);
        assert!(!summary.contains(idle_text(Language::En)));
        let (visual, summary) =
            refreshed_status(Err("snapshot worker disconnected"), fixture.status());
        assert_eq!(visual, TrayVisual::Error);
        assert!(summary.contains("snapshot worker disconnected"));
        assert!(!summary.contains(idle_text(Language::En)));
    }

    #[test]
    fn healthy_refresh_recovers_partial_warning_without_acknowledging_error_history() {
        let mut fixture = StatusFixture::healthy();
        fixture.notifications.failed();
        fixture.notifications.delivered();
        let mut snapshot = partial_activity_snapshot();
        let (visual, _) = refreshed_status(Ok(&snapshot), fixture.status());
        assert_eq!(visual, TrayVisual::Partial);
        snapshot.collectors[0].state = CollectorState::Healthy;
        let old_reason = snapshot.collectors[0].detail.as_deref().unwrap().to_owned();
        snapshot.collectors[0].detail = None;
        snapshot.observation_gaps.clear();
        let (visual, summary) = refreshed_status(Ok(&snapshot), fixture.status());
        assert_eq!(visual, TrayVisual::Active);
        assert!(!summary.contains(&old_reason));
        assert!(!summary.contains("sensor_activity degraded"));
        assert!(summary.contains("1 undelivered toasts"));
        fixture.errors.failed(
            Resource::Camera,
            true,
            "explicit camera operation failed".into(),
        );
        let (visual, summary) = refreshed_status(Ok(&snapshot), fixture.status());
        assert_eq!(visual, TrayVisual::Error);
        assert!(summary.contains("explicit camera operation failed"));
        assert_eq!(fixture.errors.messages().count(), 1);
        fixture.errors.completed(Resource::Camera, true);
        snapshot
            .accesses
            .retain(|access| access.activity == Activity::Ready);
        assert_eq!(
            refreshed_status(Ok(&snapshot), fixture.status()).0,
            TrayVisual::Ready
        );
        snapshot.accesses.clear();
        assert_eq!(
            refreshed_status(Ok(&snapshot), fixture.status()).0,
            TrayVisual::Idle
        );
        assert_eq!(fixture.notifications.undelivered.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn control_and_notification_failures_outrank_partial_activity_without_erasing_details() {
        let mut fixture = StatusFixture::healthy();
        let snapshot = partial_activity_snapshot();
        for failure in 0..6 {
            if failure == 4 {
                fixture
                    .errors
                    .failed(Resource::Microphone, true, "mute command denied".into());
            }
            if failure == 5 {
                fixture.errors.completed(Resource::Microphone, true);
                fixture.notifications.failed();
            }
            let mut status = fixture.status();
            if failure == 0 {
                status.microphone = None;
            }
            if failure == 1 {
                status.camera = None;
            }
            if failure == 2 {
                status.observation_errors[0] = Some("microphone observation failed");
            }
            if failure == 3 {
                status.observation_errors[1] = Some("camera observation failed");
            }
            let (visual, summary) = refreshed_status(Ok(&snapshot), status);
            assert_eq!(visual, TrayVisual::Error);
            assert!(summary.lines().next().unwrap().starts_with("Status error"));
            assert!(summary.lines().next().unwrap().contains("2 active"));
            assert!(summary.contains("Zoom.exe; PID 123"));
            assert!(summary.contains("unknown camera client; PID unknown"));
            assert!(summary.contains(snapshot.collectors[0].detail.as_deref().unwrap()));
            let reason = match failure {
                0 => Language::En.unknown_protection(true),
                1 => Language::En.unknown_protection(false),
                2 => "microphone observation failed",
                3 => "camera observation failed",
                4 => "mute command denied",
                _ => "Notification delivery currently overloaded/unavailable",
            };
            assert!(summary.contains(reason));
        }
        fixture.notifications.delivered();
        fixture.microphone.requested = true;
        let (visual, summary) = refreshed_status(Ok(&snapshot), fixture.status());
        assert_eq!(visual, TrayVisual::Error);
        assert!(summary.contains(fixture.microphone.detail.as_deref().unwrap()));
        fixture.microphone.requested = false;
        fixture.camera.desired_blocked = true;
        assert_eq!(
            refreshed_status(Ok(&snapshot), fixture.status()).0,
            TrayVisual::Error
        );
        fixture.camera.desired_blocked = false;
        fixture.camera.unknown_devices = 1;
        assert_eq!(
            refreshed_status(Ok(&snapshot), fixture.status()).0,
            TrayVisual::Error
        );
        fixture.camera.unknown_devices = 0;
        fixture.microphone.mute_state = MicrophoneMuteState::Unavailable;
        assert_eq!(
            refreshed_status(Ok(&snapshot), fixture.status()).0,
            TrayVisual::Error
        );
    }

    #[test]
    fn bounded_refresh_consumes_lock_edges_then_latest_snapshot() {
        let (sender, receiver) = mpsc::sync_channel(2);
        sender.try_send((SessionLockState::Locked, 1)).unwrap();
        sender.try_send((SessionLockState::Unlocked, 2)).unwrap();
        assert!(next_refresh(&receiver, 8).is_none());
        assert!(matches!(
            sender.try_send((SessionLockState::Locked, 3)),
            Err(mpsc::TrySendError::Full(_))
        ));
        let first = next_refresh(&receiver, 0).unwrap();
        sender.try_send((SessionLockState::Locked, 3)).unwrap();
        let second = next_refresh(&receiver, 1).unwrap();
        let latest = next_refresh(&receiver, 2).unwrap();
        assert_eq!(
            [first.0, second.0, latest.0],
            [
                SessionLockState::Locked,
                SessionLockState::Unlocked,
                SessionLockState::Locked
            ]
        );
        assert_eq!(latest.1, 3);
        assert!(next_refresh(&receiver, 3).is_none());
    }

    #[test]
    fn saturated_refresh_backpressure_never_drops_an_observed_transition() {
        let (sender, receiver) = mpsc::sync_channel(2);
        enqueue_refresh(&sender, 0).unwrap();
        enqueue_refresh(&sender, 1).unwrap();
        let producer = thread::spawn(move || {
            for value in 2..1000 {
                enqueue_refresh(&sender, value).unwrap();
            }
        });
        let values: Vec<_> = receiver.into_iter().collect();
        producer.join().unwrap();
        assert_eq!(values, (0..1000).collect::<Vec<_>>());
    }

    #[test]
    fn notification_saturation_is_visible_and_preserves_accepted_toasts() {
        let (sender, receiver) = mpsc::sync_channel(NOTIFICATION_QUEUE_CAPACITY);
        let worker = NotificationWorker {
            sender,
            health: Arc::new(NotificationHealth::default()),
        };
        for value in 0..NOTIFICATION_QUEUE_CAPACITY {
            assert!(worker.submit("title", &value.to_string()));
        }
        assert!(!worker.submit("title", "overflow"));
        assert_eq!(worker.health.undelivered.load(Ordering::Relaxed), 1);
        assert!(worker.health.unavailable.load(Ordering::Relaxed));
        for value in 0..NOTIFICATION_QUEUE_CAPACITY {
            assert_eq!(
                receiver.try_recv().unwrap(),
                ("title".to_owned(), value.to_string())
            );
        }
        assert!(worker.submit("title", "next"));
        assert_eq!(receiver.try_recv().unwrap().1, "next");
        // Queue admission alone is not evidence that native toast delivery recovered.
        assert!(worker.health.unavailable.load(Ordering::Relaxed));
        assert_eq!(worker.health.undelivered.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn notification_delivery_recovery_retains_loss_history() {
        let health = NotificationHealth::default();
        health.failed();
        health.failed();
        assert!(health.unavailable.load(Ordering::Relaxed));
        health.delivered();
        assert!(!health.unavailable.load(Ordering::Relaxed));
        assert_eq!(health.undelivered.load(Ordering::Relaxed), 2);
        health.failed();
        assert!(health.unavailable.load(Ordering::Relaxed));
        assert_eq!(health.undelivered.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn bounded_camera_completion_delivery_keeps_every_accepted_effect() {
        let (requests, pending) = mpsc::sync_channel(8);
        let (results, delivered) = mpsc::sync_channel(1);
        for _ in 0..8 {
            requests.try_send(CameraRequest::Manual).unwrap();
        }
        assert!(matches!(
            requests.try_send(CameraRequest::Manual),
            Err(mpsc::TrySendError::Full(_))
        ));
        drop(requests);
        let worker = thread::spawn(move || {
            camera_worker(
                pending,
                results,
                |_| Ok(CameraPrivacyState::Blocked),
                || {},
                || true,
            );
        });
        let completions: Vec<_> = delivered.into_iter().collect();
        worker.join().unwrap();
        assert_eq!(completions.len(), 8);
        assert!(completions.iter().all(|(request, result)| matches!(
            request,
            CameraRequest::Manual
        ) && result
            == &Ok(CameraPrivacyState::Blocked)));
    }

    static SHELL_CALLBACK_COUNT: AtomicU64 = AtomicU64::new(0);
    static SHELL_CALLBACK_EVENT: AtomicU64 = AtomicU64::new(0);

    unsafe extern "system" fn native_test_wnd_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if message == WM_TRAY_CALLBACK {
            SHELL_CALLBACK_COUNT.fetch_add(1, Ordering::Relaxed);
            SHELL_CALLBACK_EVENT.store(lparam.0 as u64, Ordering::Relaxed);
        }
        unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
    }

    fn native_test_owner() -> HWND {
        enable_dpi_awareness();
        let class = format_wide("MicCamWatchNativeProofClass");
        let title = format_wide("MicCamWatch native menu proof");
        unsafe {
            let _ = RegisterClassW(&WNDCLASSW {
                lpfnWndProc: Some(native_test_wnd_proc),
                lpszClassName: PCWSTR(class.as_ptr()),
                ..Default::default()
            });
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                PCWSTR(class.as_ptr()),
                PCWSTR(title.as_ptr()),
                WS_OVERLAPPED,
                0,
                0,
                0,
                0,
                None,
                None,
                None,
                None,
            )
            .unwrap()
        }
    }

    fn assert_native_menu(menu: HMENU, owner: HWND) {
        use windows::Win32::UI::WindowsAndMessaging::{
            GetMenuItemID, GetMenuState, GetMenuStringW, MF_BYPOSITION,
        };
        unsafe {
            let mut label = [0u16; 1024];
            let len = GetMenuStringW(menu, 1, Some(&mut label), MF_BYPOSITION);
            assert!(len > 0);
            let label = String::from_utf16(&label[..len as usize]).unwrap();
            assert!(!label.chars().any(char::is_control));
            let displayed: Vec<u16> = label.replace("&&", "&").encode_utf16().collect();
            let font = native_menu_font(owner).expect("native menu font required for this proof");
            let dc = GetDC(Some(owner));
            assert!(!dc.is_invalid());
            let previous = SelectObject(dc, font.into());
            let mut size = SIZE::default();
            assert!(GetTextExtentPoint32W(dc, &displayed, &mut size).as_bool());
            assert!(
                size.cx <= menu_summary_width(owner),
                "{} > {}",
                size.cx,
                menu_summary_width(owner)
            );
            let _ = SelectObject(dc, previous);
            let _ = ReleaseDC(Some(owner), dc);
            let _ = DeleteObject(font.into());
            for (position, command) in [
                (2, CMD_DETAILS),
                (4, CMD_TOGGLE_MUTE),
                (5, CMD_TOGGLE_CAMERA),
                (6, CMD_TOGGLE_NOTIFICATIONS),
                (7, CMD_CYCLE_PROFILE),
                (8, CMD_TOGGLE_AUTOSTART),
                (10, CMD_EXIT),
            ] {
                assert_eq!(GetMenuItemID(menu, position), command as u32);
            }
            let flags = GetMenuState(
                menu,
                CMD_TOGGLE_MUTE as u32,
                windows::Win32::UI::WindowsAndMessaging::MF_BYCOMMAND,
            );
            assert_eq!(flags & (MF_DISABLED.0 | MF_GRAYED.0), 0);
        }
    }

    #[test]
    fn privacy_error_recovery_keeps_other_resources_and_manual_failures() {
        let mut errors = PrivacyOperationErrors::default();
        errors.failed(Resource::Microphone, true, "microphone failure".into());
        errors.failed(Resource::Camera, true, "manual camera failure".into());
        errors.failed(Resource::Camera, false, "automatic camera failure".into());

        errors.completed(Resource::Camera, false);
        assert_eq!(
            errors.messages().collect::<Vec<_>>(),
            ["microphone failure", "manual camera failure"]
        );
        errors.completed(Resource::Microphone, true);
        assert_eq!(
            errors.messages().collect::<Vec<_>>(),
            ["manual camera failure"]
        );
        errors.completed(Resource::Camera, true);
        assert!(errors.messages().next().is_none());
    }

    #[test]
    fn native_menu_consumer_bounds_unicode_and_preserves_actions() {
        let owner = native_test_owner();
        let fixture = StatusFixture::healthy();
        let snapshot = partial_activity_snapshot();
        let (_, partial_summary) = refreshed_status(Ok(&snapshot), fixture.status());
        for requested in [Some(true), Some(false), None] {
            for summary in [
                format!("miccamwatch: collection failed: {}", "W".repeat(16 * 1024)),
                format!(
                    "miccamwatch: degraded: {}",
                    "界🛡️\t&Exit\n\r\u{202e}\0".repeat(1000)
                ),
                partial_summary.clone(),
            ] {
                let menu = build_context_menu(
                    owner,
                    &summary,
                    Language::En,
                    requested,
                    None,
                    &Settings::default(),
                    crate::autostart::AutostartState::Disabled,
                )
                .unwrap();
                assert_native_menu(menu, owner);
                let _ = unsafe { DestroyMenu(menu) };
            }
        }
        let _ = unsafe { DestroyWindow(owner) };
    }

    struct NativeProofShellIcon(NOTIFYICONDATAW);

    impl Drop for NativeProofShellIcon {
        fn drop(&mut self) {
            let _ = unsafe { Shell_NotifyIconW(NIM_DELETE, &self.0) };
        }
    }

    fn qualify_native_shell_tip(directory: &std::path::Path, name: &str, data: &NOTIFYICONDATAW) {
        use windows::Win32::UI::WindowsAndMessaging::{PM_REMOVE, PeekMessageW};
        std::fs::write(
            directory.join(format!("{name}-shell.json")),
            serde_json::to_vec(&serde_json::json!({
                "owner": data.hWnd.0 as usize,
                "id": data.uID,
                "tooltip": native_tip_text(data),
                "utf16_units": data.szTip.iter().position(|unit| *unit == 0).unwrap(),
                "capacity": data.szTip.len(),
            }))
            .unwrap(),
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(90);
        let checked = directory.join(format!("{name}-shell-checked"));
        let inspect = directory.join(format!("{name}-shell-inspect"));
        let input = directory.join(format!("{name}-shell-input.json"));
        let mut input_recorded = false;
        while !checked.exists() {
            assert!(
                Instant::now() < deadline,
                "isolated driver did not qualify actual shell tooltip"
            );
            if !input_recorded && inspect.exists() {
                std::fs::write(
                    &input,
                    serde_json::to_vec(&serde_json::json!({
                        "callback_count": SHELL_CALLBACK_COUNT.load(Ordering::Relaxed),
                        "last_callback": SHELL_CALLBACK_EVENT.load(Ordering::Relaxed),
                    }))
                    .unwrap(),
                )
                .unwrap();
                input_recorded = true;
            }
            let mut message = MSG::default();
            if unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.as_bool() {
                unsafe {
                    let _ = TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            } else {
                thread::sleep(Duration::from_millis(10));
            }
        }
    }

    #[test]
    #[ignore = "requires interactive Windows desktop; run installer/tests/windows-tray-native.ps1"]
    fn native_windows_tray_visual_smoke() {
        use windows::Win32::UI::WindowsAndMessaging::{
            ES_AUTOHSCROLL, GWL_STYLE, GetWindowTextLengthW, GetWindowTextW, IsWindow, PM_REMOVE,
            PeekMessageW,
        };
        let directory = std::path::PathBuf::from(
            std::env::var_os("MCW_WINDOWS_NATIVE_PROOF_DIR").expect("proof output directory"),
        );
        assert_eq!(std::env::var("GITHUB_ACTIONS").as_deref(), Ok("true"));
        assert_eq!(
            std::env::var("RUNNER_ENVIRONMENT").as_deref(),
            Ok("github-hosted")
        );
        std::fs::create_dir_all(&directory).unwrap();
        let owner = native_test_owner();
        let initial_icon = create_status_icon((107, 114, 128), IconGlyph::Error, unsafe {
            GetSystemMetrics(SM_CXSMICON)
        })
        .unwrap();
        let mut shell_icon =
            NativeProofShellIcon(notify_icon_data(owner, initial_icon, "miccamwatch"));
        assert!(unsafe { Shell_NotifyIconW(NIM_ADD, &shell_icon.0) }.as_bool());
        qualify_native_shell_tip(&directory, "startup", &shell_icon.0);
        let fixture = StatusFixture::healthy();
        let mut partial = partial_activity_snapshot();
        let partial_summary = refreshed_status(Ok(&partial), fixture.status()).1;
        partial.collectors[0].state = CollectorState::Unavailable;
        let error_summary = refreshed_status(Ok(&partial), fixture.status()).1;
        partial.collectors[0].state = CollectorState::Healthy;
        partial.collectors[0].detail = None;
        partial.observation_gaps.clear();
        let recovered_summary = refreshed_status(Ok(&partial), fixture.status()).1;
        for (name, visual, summary) in [
            ("partial-active", TrayVisual::Partial, partial_summary),
            ("error-active", TrayVisual::Error, error_summary),
            ("recovered-active", TrayVisual::Active, recovered_summary),
            (
                "long-error",
                TrayVisual::Error,
                format!("miccamwatch: collection failed: {}", "W".repeat(16 * 1024)),
            ),
            (
                "unicode-controls",
                TrayVisual::Partial,
                format!(
                    "miccamwatch: degraded: {}",
                    "界🛡️\t&Exit\n\r\u{202e}\0".repeat(1000)
                ),
            ),
            (
                "idle-protected",
                TrayVisual::Idle,
                format!(
                    "{}\n{}\n{}",
                    idle_text(Language::Fr),
                    "Protection micro demandée; garde active; SDK coupé",
                    "Protection caméra demandée; garde active; aucune capture observée"
                ),
            ),
        ] {
            let (color, glyph) = match visual {
                TrayVisual::Idle => ((34, 197, 94), IconGlyph::Check),
                TrayVisual::Ready => ((245, 158, 11), IconGlyph::Ready),
                TrayVisual::Active => ((239, 68, 68), IconGlyph::Active),
                TrayVisual::Partial => ((245, 158, 11), IconGlyph::Active),
                TrayVisual::Error => ((107, 114, 128), IconGlyph::Error),
            };
            let icon =
                create_status_icon(color, glyph, unsafe { GetSystemMetrics(SM_CXSMICON) }).unwrap();
            shell_icon.0 = notify_icon_data(owner, icon, &summary);
            assert!(unsafe { Shell_NotifyIconW(NIM_MODIFY, &shell_icon.0) }.as_bool());
            qualify_native_shell_tip(&directory, name, &shell_icon.0);
            let menu = build_context_menu(
                owner,
                &summary,
                Language::En,
                Some(true),
                Some(false),
                &Settings::default(),
                crate::autostart::AutostartState::Disabled,
            )
            .unwrap();
            assert_native_menu(menu, owner);
            let work = monitor_work_area(owner);
            std::fs::write(
                directory.join(format!("{name}-menu.json")),
                serde_json::to_vec(&serde_json::json!({
                    "owner": owner.0 as usize, "menu": menu.0 as usize,
                    "text_width_limit": menu_summary_width(owner),
                    "work_width": work.right - work.left,
                    "work_height": work.bottom - work.top,
                    "dpi": unsafe { GetDpiForWindow(owner) },
                    "icon": icon.0 as usize,
                }))
                .unwrap(),
            )
            .unwrap();
            unsafe {
                let _ = SetForegroundWindow(owner);
                let command = TrackPopupMenu(
                    menu,
                    TPM_RIGHTBUTTON | TPM_BOTTOMALIGN | TPM_RETURNCMD | TPM_NONOTIFY,
                    work.right - 30,
                    work.bottom - 30,
                    None,
                    owner,
                    None,
                );
                assert_eq!(
                    command.0, 0,
                    "proof driver cancels without invoking privacy actions"
                );
                let _ = DestroyMenu(menu);
            }
            let document = diagnostic_document(&summary);
            std::fs::write(directory.join(format!("{name}-diagnostic.txt")), &document).unwrap();
            let details = show_diagnostic_window(owner, &summary).unwrap();
            unsafe {
                let edit = GetDlgItem(Some(details), DETAILS_EDIT).unwrap();
                let style = GetWindowLongPtrW(edit, GWL_STYLE) as u32;
                assert_eq!(
                    style & ES_AUTOHSCROLL as u32,
                    0,
                    "diagnostic must word-wrap"
                );
                assert_ne!(
                    style & ES_READONLY as u32,
                    0,
                    "diagnostic must be copyable, not editable"
                );
                let mut text = vec![0u16; GetWindowTextLengthW(edit) as usize + 1];
                let length = GetWindowTextW(edit, &mut text);
                assert_eq!(
                    String::from_utf16(&text[..length as usize]).unwrap(),
                    document
                );
                std::fs::write(
                    directory.join(format!("{name}-details.json")),
                    serde_json::to_vec(&serde_json::json!({
                        "window": details.0 as usize, "edit": edit.0 as usize,
                        "document_utf16_units": document.encode_utf16().count(),
                    }))
                    .unwrap(),
                )
                .unwrap();
                let deadline = Instant::now() + Duration::from_secs(90);
                while IsWindow(Some(details)).as_bool() {
                    assert!(
                        Instant::now() < deadline,
                        "native proof driver did not close details"
                    );
                    let mut message = MSG::default();
                    if PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                        let _ = TranslateMessage(&message);
                        DispatchMessageW(&message);
                    } else {
                        thread::sleep(Duration::from_millis(10));
                    }
                }
            }
            let _ = unsafe { DestroyIcon(icon) };
        }
        drop(shell_icon);
        let _ = unsafe { DestroyIcon(initial_icon) };
        let _ = unsafe { DestroyWindow(owner) };
        std::fs::write(
            directory.join("complete"),
            "actual shell tooltip/UIA, native HMENU geometry and full EDIT content checked",
        )
        .unwrap();
    }

    #[test]
    fn safe_close_refuses_every_pending_camera_operation() {
        assert!(can_close_tray(0, false, false, 0));
        assert!(!can_close_tray(1, false, false, 0));
        assert!(!can_close_tray(0, true, false, 0));
        assert!(!can_close_tray(0, false, true, 0));
        assert!(!can_close_tray(0, false, false, 1));
    }

    #[test]
    fn restore_permits_cover_queued_and_active_operations() {
        let activity = Arc::new(Mutex::new(RestoreActivity::default()));
        activity.lock().pending += 1;
        let permit = RestorePermit(Arc::clone(&activity));
        let (sender, receiver) = mpsc::channel();
        sender.send(permit).unwrap();
        assert_eq!(activity.lock().pending, 1);
        let active = receiver.recv().unwrap();
        assert_eq!(activity.lock().pending, 1);
        drop(active);
        assert_eq!(activity.lock().pending, 0);
    }

    #[test]
    fn declined_restore_drains_permits_without_cancelling_active_work() {
        let activity = Arc::new(Mutex::new(RestoreActivity::default()));
        let (sender, receiver) = mpsc::channel();
        for _ in 0..2 {
            activity.lock().pending += 1;
            sender.send(RestorePermit(Arc::clone(&activity))).unwrap();
        }
        drop(sender);
        let (results, delivered) = mpsc::sync_channel(2);
        let mut calls = 0;
        restore_worker(
            receiver,
            results,
            || true,
            || {
                assert_eq!(activity.lock().pending, 2);
                calls += 1;
                Err("UAC declined".into())
            },
            || true,
        );
        assert_eq!(calls, 1);
        assert_eq!(activity.lock().pending, 1);
        let (permit, result) = delivered.recv().unwrap();
        assert!(result.is_err());
        assert_eq!(activity.lock().pending, 1);
        drop(permit);
        assert_eq!(activity.lock().pending, 0);
    }
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
    fn stale_or_inflight_control_refresh_cannot_revert_completed_intent() {
        assert!(!accepts_control_refresh(4, 5, 0));
        assert!(!accepts_control_refresh(5, 5, 1));
        assert!(accepts_control_refresh(5, 5, 0));
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
        let (results, delivered) = mpsc::sync_channel(8);
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
            observation_gaps: Vec::new(),
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
        let (results, delivered) = mpsc::sync_channel(2);
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
            delivered.recv_timeout(Duration::from_secs(2)).unwrap().1,
            Ok(1)
        );
        assert_eq!(
            delivered.recv_timeout(Duration::from_secs(2)).unwrap().1,
            Ok(2)
        );
        drop(requests);
        assert_eq!(worker.join().unwrap(), 2);
    }

    #[test]
    fn declined_prompt_discards_arrivals_queued_during_the_prompt() {
        let (requests, pending) = mpsc::sync_channel(1);
        let (results, delivered) = mpsc::sync_channel(2);
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
            delivered.recv_timeout(Duration::from_secs(2)).unwrap().1,
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
        let (results, delivered) = mpsc::sync_channel(2);
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
        let (results, delivered) = mpsc::sync_channel(2);
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
            delivered.recv_timeout(Duration::from_secs(2)).unwrap().1,
            Ok(1)
        );
        requests.try_send(()).unwrap();
        assert_eq!(
            delivered.recv_timeout(Duration::from_secs(2)).unwrap().1,
            Ok(2)
        );
        drop(requests);
        assert_eq!(worker.join().unwrap(), 2);
    }

    #[test]
    fn closed_window_skips_pending_restore_without_invoking_privacy() {
        let (requests, pending) = mpsc::sync_channel(1);
        let (results, delivered) = mpsc::sync_channel(2);
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
