//! Explicit, non-operational development entry: the real collector and tray,
//! without control workers, history/settings writes, automatic policies or restore.
use super::{
    IconGlyph, MutexGuard, WM_OPEN_DESKTOP, create_status_icon, enable_dpi_awareness, format_wide,
    notify_icon_data,
};
use crate::{
    config::Policy,
    frontends::{
        cli::Filter,
        desktop::{DesktopSnapshot, windows::Supervisor},
    },
    i18n::Language,
    model::{Activity, CollectorState, Snapshot},
    platform::{MicrophoneProtectionStatus, PlatformMonitor},
    privacy::CameraControlObservation,
};
use anyhow::{Context, Result, ensure};
use parking_lot::Mutex;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicIsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use windows::{
    Win32::{
        Foundation::{
            CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HWND, LPARAM, LRESULT, WPARAM,
        },
        System::Threading::{
            CreateMutexW, GetCurrentProcessId, OpenProcess, PROCESS_NAME_WIN32,
            PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
        },
        UI::{
            Shell::{NIM_ADD, NIM_DELETE, NIM_MODIFY, Shell_NotifyIconW},
            WindowsAndMessaging::{
                AppendMenuW, CREATESTRUCTW, CreatePopupMenu, CreateWindowExW, DefWindowProcW,
                DestroyIcon, DestroyMenu, DestroyWindow, DispatchMessageW, FindWindowW,
                GWLP_USERDATA, GetCursorPos, GetMessageW, GetSystemMetrics, GetWindowLongPtrW,
                GetWindowThreadProcessId, HICON, KillTimer, MF_GRAYED, MF_SEPARATOR, MF_STRING,
                MSG, PostMessageW, PostQuitMessage, RegisterClassW, SM_CXSMICON, SendMessageW,
                SetForegroundWindow, SetTimer, SetWindowLongPtrW, TPM_RETURNCMD, TPM_RIGHTBUTTON,
                TrackPopupMenu, TranslateMessage, WINDOW_EX_STYLE, WM_APP, WM_CLOSE, WM_COMMAND,
                WM_DESTROY, WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_NCCREATE, WM_RBUTTONUP, WM_TIMER,
                WNDCLASSW, WS_OVERLAPPED,
            },
        },
    },
    core::{PCWSTR, PWSTR},
};
const CLASS: &str = "MicCamWatchReadOnlyDevelopmentTray";
const CALLBACK: u32 = WM_APP + 60;
const REFRESH: u32 = WM_APP + 61;
const OPEN: usize = 1;
const EXIT: usize = 2;
const TIMER: usize = 61;

struct Reading {
    snapshot: Result<Snapshot, String>,
    microphone: Result<MicrophoneProtectionStatus, String>,
    camera: Result<CameraControlObservation, String>,
    observed_at: chrono::DateTime<chrono::Utc>,
    sequence: u64,
}
struct Observer {
    hwnd: HWND,
    lang: Language,
    theme: String,
    ui: Option<Supervisor>,
    current: Option<Reading>,
    pending: Arc<Mutex<Option<Reading>>>,
    alive: Arc<AtomicBool>,
    window: Arc<AtomicIsize>,
    icon: HICON,
    summary: String,
}
impl Observer {
    fn frame(&self) -> DesktopSnapshot {
        let (sequence, observed_at, snapshot) = self.current.as_ref().map_or(
            (0, chrono::Utc::now(), Err("Initial observation pending")),
            |reading| {
                (
                    reading.sequence,
                    reading.observed_at,
                    reading.snapshot.as_ref().map_err(String::as_str),
                )
            },
        );
        let mut frame = DesktopSnapshot::from_snapshot(
            sequence,
            observed_at,
            self.lang.code(),
            snapshot,
            "read_only_development",
        );
        if let Some(reading) = &self.current {
            frame.set_protection(
                reading.microphone.as_ref().map_err(String::as_str),
                reading.camera.as_ref().map_err(String::as_str),
            );
        }
        frame
    }
    fn open(&mut self) -> Result<()> {
        self.refresh()?;
        let frame = self.frame();
        if self.ui.is_none() {
            self.ui = Some(Supervisor::new()?);
        }
        self.ui
            .as_mut()
            .expect("initialized supervisor")
            .open(&frame, &self.theme)
    }
    fn refresh(&mut self) -> Result<()> {
        if let Some(ui) = &mut self.ui {
            ui.poll();
        }
        let next = self.pending.lock().take();
        if let Some(reading) = next {
            let (active, partial) = reading.snapshot.as_ref().map_or((0, true), |s| {
                (
                    s.accesses
                        .iter()
                        .filter(|a| a.activity == Activity::Active)
                        .count(),
                    !s.observation_gaps.is_empty()
                        || s.collectors.is_empty()
                        || s.collectors
                            .iter()
                            .any(|c| c.state != CollectorState::Healthy),
                )
            });
            self.summary = format!(
                "MicCamWatch — READ ONLY DEV; {active} active; {}",
                if partial {
                    "observation incomplete"
                } else {
                    "live observations"
                }
            );
            self.current = Some(reading);
            let mut icon = notify_icon_data(self.hwnd, self.icon, &self.summary);
            icon.uCallbackMessage = CALLBACK;
            let _ = unsafe { Shell_NotifyIconW(NIM_MODIFY, &icon) };
            if self.ui.as_ref().is_some_and(Supervisor::is_running) {
                let frame = self.frame();
                self.ui
                    .as_mut()
                    .expect("running supervisor")
                    .publish(&frame)?;
            }
        }
        Ok(())
    }
    fn menu(&mut self) -> Result<()> {
        unsafe {
            let menu = CreatePopupMenu()?;
            AppendMenuW(
                menu,
                MF_STRING,
                OPEN,
                PCWSTR(format_wide("Open MicCamWatch — read only").as_ptr()),
            )?;
            AppendMenuW(
                menu,
                MF_STRING | MF_GRAYED,
                0,
                PCWSTR(format_wide(&self.summary).as_ptr()),
            )?;
            AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null())?;
            AppendMenuW(
                menu,
                MF_STRING,
                EXIT,
                PCWSTR(format_wide("Exit read-only development tray").as_ptr()),
            )?;
            let mut point = Default::default();
            GetCursorPos(&mut point)?;
            let _ = SetForegroundWindow(self.hwnd);
            let command = TrackPopupMenu(
                menu,
                TPM_RETURNCMD | TPM_RIGHTBUTTON,
                point.x,
                point.y,
                Some(0),
                self.hwnd,
                None,
            )
            .0 as usize;
            let _ = DestroyMenu(menu);
            match command {
                OPEN => {
                    eprintln!("desktop event=open_trigger source=context_menu");
                    self.open()?;
                }
                EXIT => {
                    PostMessageW(Some(self.hwnd), WM_CLOSE, WPARAM(0), LPARAM(0))?;
                }
                _ => {}
            }
        }
        Ok(())
    }
}
unsafe fn state(hwnd: HWND) -> Option<&'static mut Observer> {
    unsafe { (GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Observer).as_mut() }
}
unsafe extern "system" fn wnd_proc(hwnd: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if message == WM_NCCREATE {
        let create = unsafe { &*(l.0 as *const CREATESTRUCTW) };
        unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize) };
        return LRESULT(1);
    }
    let Some(observer) = (unsafe { state(hwnd) }) else {
        return unsafe { DefWindowProcW(hwnd, message, w, l) };
    };
    let result = match message {
        WM_OPEN_DESKTOP if w.0 == unsafe { GetCurrentProcessId() } as usize => {
            eprintln!("desktop event=open_trigger source=window_activation");
            observer.open()
        }
        REFRESH | WM_TIMER => observer.refresh(),
        CALLBACK => match l.0 as u32 {
            WM_LBUTTONDBLCLK => {
                eprintln!("desktop event=open_trigger source=tray_double_click");
                observer.open()
            }
            WM_LBUTTONUP | WM_RBUTTONUP => observer.menu(),
            _ => Ok(()),
        },
        WM_COMMAND => match w.0 & 0xffff {
            OPEN => {
                eprintln!("desktop event=open_trigger source=window_command");
                observer.open()
            }
            EXIT => unsafe { DestroyWindow(hwnd) }.map_err(Into::into),
            _ => Ok(()), // No mutation command dispatcher exists in this mode.
        },
        WM_CLOSE => unsafe { DestroyWindow(hwnd) }.map_err(Into::into),
        WM_DESTROY => {
            observer.alive.store(false, Ordering::Release);
            observer.window.store(0, Ordering::Release);
            unsafe { PostQuitMessage(0) };
            Ok(())
        }
        _ => return unsafe { DefWindowProcW(hwnd, message, w, l) },
    };
    if let Err(error) = result {
        eprintln!("mcw read-only desktop: {error:#}");
        return LRESULT(0);
    }
    LRESULT(1)
}

fn start_collector(observer: &Observer) -> thread::JoinHandle<()> {
    let pending = Arc::clone(&observer.pending);
    let alive = Arc::clone(&observer.alive);
    let window = Arc::clone(&observer.window);
    thread::spawn(move || {
        let monitor = PlatformMonitor::new(Policy::default()).map_err(|e| format!("{e:#}"));
        let scope = Filter {
            include_ready: true,
            ..Filter::default()
        };
        let mut sequence = 0u64;
        let mut camera = Err("Initial camera observation pending".to_owned());
        let mut camera_at = None;
        while alive.load(Ordering::Acquire) {
            let observed_at = chrono::Utc::now();
            let (snapshot, microphone) = match &monitor {
                Ok(monitor) => (
                    monitor
                        .snapshot((&scope).into())
                        .map_err(|e| format!("{e:#}")),
                    monitor
                        .microphone_protection_status()
                        .map_err(|e| format!("{e:#}")),
                ),
                Err(error) => (Err(error.clone()), Err(error.clone())),
            };
            if camera_at.is_none_or(|at: Instant| at.elapsed() >= Duration::from_secs(5)) {
                camera = crate::privacy::camera_observation().map_err(|e| format!("{e:#}"));
                camera_at = Some(Instant::now());
            }
            sequence = sequence.saturating_add(1);
            *pending.lock() = Some(Reading {
                snapshot,
                microphone,
                camera: camera.clone(),
                observed_at,
                sequence,
            });
            let hwnd = window.load(Ordering::Acquire);
            if hwnd != 0 {
                let _ = unsafe {
                    PostMessageW(Some(HWND(hwnd as *mut _)), REFRESH, WPARAM(0), LPARAM(0))
                };
            }
            if sequence == 1 || sequence.is_multiple_of(5) {
                eprintln!(
                    "{{\"event\":\"observer_tick\",\"sequence\":{sequence},\"pid\":{}}}",
                    unsafe { GetCurrentProcessId() }
                );
            }
            thread::park_timeout(Duration::from_millis(500));
        }
    })
}

fn reuse_read_only(initial_window: bool) -> Result<bool> {
    let class = format_wide(CLASS);
    let Ok(hwnd) = (unsafe { FindWindowW(PCWSTR(class.as_ptr()), PCWSTR::null()) }) else {
        return Ok(false);
    };
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }?;
    let mut path = vec![0u16; 32768];
    let mut length = path.len() as u32;
    let query = unsafe {
        QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(path.as_mut_ptr()),
            &mut length,
        )
    };
    unsafe {
        let _ = CloseHandle(handle);
    }
    query?;
    let peer = std::path::PathBuf::from(String::from_utf16_lossy(&path[..length as usize]))
        .canonicalize()?;
    ensure!(
        peer == std::env::current_exe()?.canonicalize()?,
        "another development build owns the read-only tray; it was preserved"
    );
    if initial_window {
        unsafe {
            SendMessageW(
                hwnd,
                WM_OPEN_DESKTOP,
                Some(WPARAM(pid as usize)),
                Some(LPARAM(0)),
            )
        };
    }
    Ok(true)
}

pub(super) fn run(lang: Language, initial_window: bool, theme: &str) -> Result<()> {
    ensure!(matches!(theme, "light" | "dark"), "invalid desktop theme");
    enable_dpi_awareness();
    let mutex = format_wide("Local\\MicCamWatch.Tray.ReadOnlyDevelopment");
    let _lease = MutexGuard(unsafe { CreateMutexW(None, true, PCWSTR(mutex.as_ptr()))? });
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if reuse_read_only(initial_window)? {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(50));
        }
        anyhow::bail!("read-only development tray owner unavailable; preserved existing owner");
    }
    let class = format_wide(CLASS);
    let wc = WNDCLASSW {
        lpfnWndProc: Some(wnd_proc),
        lpszClassName: PCWSTR(class.as_ptr()),
        ..Default::default()
    };
    ensure!(
        unsafe { RegisterClassW(&wc) } != 0,
        "cannot register read-only tray class"
    );
    let icon = create_status_icon(
        (107, 114, 128),
        IconGlyph::Error,
        unsafe { GetSystemMetrics(SM_CXSMICON) }.max(16),
    )?;
    let mut observer = Box::new(Observer {
        hwnd: HWND::default(),
        lang,
        theme: theme.to_owned(),
        ui: None,
        current: None,
        pending: Arc::new(Mutex::new(None)),
        alive: Arc::new(AtomicBool::new(true)),
        window: Arc::new(AtomicIsize::new(0)),
        icon,
        summary: "MicCamWatch — READ ONLY DEVELOPMENT; observation pending".to_owned(),
    });
    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(class.as_ptr()),
            PCWSTR(class.as_ptr()),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            None,
            None,
            None,
            Some((&mut *observer as *mut Observer).cast()),
        )
    }
    .context("create read-only tray window")?;
    observer.hwnd = hwnd;
    observer.window.store(hwnd.0 as isize, Ordering::Release);
    let nid = notify_icon_data(hwnd, icon, &observer.summary);
    let mut nid = nid;
    nid.uCallbackMessage = CALLBACK;
    let added = unsafe { Shell_NotifyIconW(NIM_ADD, &nid) }.as_bool();
    if !added {
        unsafe {
            let _ = DestroyWindow(hwnd);
            let _ = DestroyIcon(icon);
        }
        anyhow::bail!("cannot register read-only development tray icon");
    }
    let worker = start_collector(&observer);
    let timer = unsafe { SetTimer(Some(hwnd), TIMER, 250, None) };
    let mut result = if timer == 0 {
        Err(anyhow::anyhow!("cannot start read-only tray timer"))
    } else {
        Ok(())
    };
    if result.is_ok()
        && initial_window
        && let Err(error) = observer.open()
    {
        // A failed disposable window must never stop the collector/tray.
        eprintln!("mcw read-only desktop launch failed; monitoring continues: {error:#}");
    }
    if result.is_ok() {
        eprintln!(
            "{{\"event\":\"observer_started\",\"pid\":{},\"mode\":\"read_only_development\"}}",
            unsafe { GetCurrentProcessId() }
        );
        let mut message = MSG::default();
        loop {
            let next = unsafe { GetMessageW(&mut message, None, 0, 0) };
            if next.0 == -1 {
                result = Err(anyhow::anyhow!("read-only tray message loop failed"));
                break;
            }
            if next.0 == 0 {
                break;
            }
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
    observer.alive.store(false, Ordering::Release);
    observer.window.store(0, Ordering::Release);
    observer.ui.take();
    worker.thread().unpark();
    let _ = worker.join();
    unsafe {
        let _ = KillTimer(Some(hwnd), TIMER);
        let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        let _ = DestroyWindow(hwnd);
        let _ = DestroyIcon(icon);
    }
    eprintln!("{{\"event\":\"observer_stopped\",\"pid\":{}}}", unsafe {
        GetCurrentProcessId()
    });
    result
}
