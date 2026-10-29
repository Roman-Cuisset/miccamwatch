//! Isolated development tray using real collectors. Device controls and
//! preference writes use separate opt-in modes and dedicated data paths.
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
use crate::notify::{
    NotificationIdentity,
    capture::{CaptureAlerts, NotificationWorker, submit_test_sequence},
};
use parking_lot::Mutex;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicIsize, AtomicU64, Ordering},
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
const TEST_NOTIFICATIONS: usize = 3;
const PROBE_MESSAGE: u32 = WM_APP + 62;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum DevelopmentMode {
    ReadOnly,
    Controlled,
    Settings,
    SettingsProbe,
}
impl DevelopmentMode {
    fn preferences_enabled(self) -> bool {
        self != Self::ReadOnly
    }
    fn wire_name(self) -> &'static str {
        match self {
            Self::ReadOnly => "read_only_development",
            Self::Controlled => "controlled_development",
            Self::Settings => "settings_development",
            Self::SettingsProbe => "settings_development",
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::ReadOnly => "READ ONLY",
            Self::Controlled => "CONTROLLED",
            Self::Settings => "PREFERENCES ONLY",
            Self::SettingsProbe => "TEST NOTIFICATIONS; REAL ALERTS DISABLED",
        }
    }
}

struct Reading {
    snapshot: Result<Snapshot, String>,
    microphone: Result<MicrophoneProtectionStatus, String>,
    camera: Result<CameraControlObservation, String>,
    observed_at: chrono::DateTime<chrono::Utc>,
    microphone_generation: u64,
    applied_profile: Option<crate::settings::PrivacyProfile>,
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
    controlled: bool,
    mode: DevelopmentMode,
    preference_worker: Option<crate::frontends::desktop::preferences::PreferenceWorker>,
    microphone_worker: Option<super::microphone_worker::Worker>,
    pending_microphone: usize,
    microphone_generation: u64,
    microphone_result: Option<Result<MicrophoneProtectionStatus, String>>,
    presentation_sequence: u64,
    control_used: bool,
    notifications: Option<NotificationWorker>,
    probe_alerts: CaptureAlerts,
    last_alert_health: Option<(u64, u64, u64, bool)>,
}
impl Observer {
    fn microphone(&self) -> Result<&MicrophoneProtectionStatus, &str> {
        if self.pending_microphone != 0 {
            return Err("Native microphone operation pending; effective state not yet confirmed");
        }
        if let Some(reading) = self.current.as_ref()
            && reading.microphone_generation == self.microphone_generation
        {
            return reading.microphone.as_ref().map_err(String::as_str);
        }
        self.microphone_result
            .as_ref()
            .map_or(Err("Microphone observation pending"), |result| {
                result.as_ref().map_err(String::as_str)
            })
    }
    fn can_exit(&self) -> bool {
        !self
            .preference_worker
            .as_ref()
            .is_some_and(crate::frontends::desktop::preferences::PreferenceWorker::is_pending)
            && (!self.controlled
                || (self.pending_microphone == 0
                    && (!self.control_used
                        || self.microphone().is_ok_and(|status| !status.requested))))
    }
    fn frame(&mut self) -> DesktopSnapshot {
        if self.current.is_some() {
            self.presentation_sequence = self.presentation_sequence.saturating_add(1);
        }
        let (observed_at, snapshot) = self.current.as_ref().map_or(
            (chrono::Utc::now(), Err("Initial observation pending")),
            |reading| {
                (
                    reading.observed_at,
                    reading.snapshot.as_ref().map_err(String::as_str),
                )
            },
        );
        let mode = self.mode.wire_name();
        let mut frame = DesktopSnapshot::from_snapshot(
            self.presentation_sequence,
            observed_at,
            self.lang.code(),
            snapshot,
            mode,
        );
        if self.mode == DevelopmentMode::SettingsProbe {
            let observed_detail = frame.engine.detail.take().unwrap_or_default();
            frame.engine.detail = Some(format!(
                "TEST native notification probe; real-source alerts disabled; dashboard observations remain real. {observed_detail}"
            ));
        }
        if let Some(reading) = &self.current {
            frame.set_protection(
                self.microphone(),
                reading.camera.as_ref().map_err(String::as_str),
            );
        }
        frame.controls.microphone = crate::frontends::desktop::microphone_capability(
            self.microphone(),
            self.microphone_generation,
            self.pending_microphone != 0,
            self.controlled,
        );
        frame.preferences = crate::frontends::desktop::observe_preferences(
            self.mode.preferences_enabled(),
            self.mode.preferences_enabled(),
            self.current
                .as_ref()
                .and_then(|reading| reading.applied_profile),
            crate::frontends::desktop::preferences::PreferenceEffects {
                capture_notifications_supported: self.notifications.is_some(),
                capture_sounds_supported: self.notifications.is_some(),
                ..Default::default()
            },
            self.preference_worker.as_ref(),
        );
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
            .open(frame, &self.theme)
    }
    fn refresh(&mut self) -> Result<()> {
        if let Some(ui) = &mut self.ui {
            ui.poll();
        }
        let mut changed = false;
        if let Some(completion) = self
            .preference_worker
            .as_mut()
            .and_then(crate::frontends::desktop::preferences::PreferenceWorker::try_complete)
        {
            crate::frontends::desktop::complete_preference(self.ui.as_mut(), &completion);
            changed = true;
        }
        if let Some(command) = self.ui.as_mut().and_then(Supervisor::take_preference) {
            let work = crate::frontends::desktop::preferences::PreferenceWork {
                ticket: command.ticket,
                request: command.request,
                profile_writable: self.mode.preferences_enabled(),
            };
            if let Err(error) = crate::frontends::desktop::preferences::PreferenceWorker::enqueue(
                &mut self.preference_worker,
                work,
            ) && let Some(ui) = self.ui.as_mut()
            {
                ui.complete_preference(
                    command.ticket,
                    crate::frontends::desktop::commands::CommandState::Rejected,
                    Some("preferences_worker"),
                    Some(format!("Preference request not queued: {error:#}")),
                );
            }
            changed = true;
        }
        if let Some(command) = self.ui.as_mut().and_then(Supervisor::take_command) {
            let request = super::microphone_worker::Request::Set {
                ticket: command.ticket,
                requested: command.requested,
                expected_requested: command.expected_requested,
                expected_generation: command.expected_generation,
            };
            if self.pending_microphone == 0
                && self
                    .microphone_worker
                    .as_ref()
                    .is_some_and(|worker| worker.requests.try_send(request).is_ok())
            {
                self.pending_microphone += 1;
                self.control_used = true;
            } else if let Some(ui) = self.ui.as_mut() {
                ui.complete(
                    command.ticket,
                    crate::frontends::desktop::commands::CommandState::Rejected,
                    Some("busy"),
                    Some("Native worker unavailable or busy; request was not applied".into()),
                );
            }
            changed = true;
        }
        loop {
            let result = self
                .microphone_worker
                .as_ref()
                .and_then(|worker| worker.results.try_recv().ok());
            let Some((request, generation, result)) = result else {
                break;
            };
            self.microphone_generation = generation;
            self.pending_microphone = self.pending_microphone.saturating_sub(1);
            if let super::microphone_worker::Request::Set {
                ticket, requested, ..
            } = request
                && let Some(ui) = self.ui.as_mut()
            {
                crate::frontends::desktop::complete_microphone(
                    ui,
                    ticket,
                    requested,
                    result.as_ref().map_err(String::as_str),
                );
            }
            self.microphone_result = Some(result);
            changed = true;
        }
        changed |= self
            .ui
            .as_mut()
            .is_some_and(Supervisor::take_controls_changed);
        if let Some(notifications) = &self.notifications {
            let health = &notifications.health;
            let current = (
                health.submitted.load(Ordering::Relaxed),
                health.suppressed.load(Ordering::Relaxed),
                health.undelivered.load(Ordering::Relaxed),
                health.unavailable.load(Ordering::Relaxed),
            );
            if self.last_alert_health != Some(current) {
                self.last_alert_health = Some(current);
                eprintln!("native alerts: submitted={} suppressed={} (disabled={} paused={} cooldown={}) failures={} unavailable={}",
                    health.submitted.load(Ordering::Relaxed),
                    health.suppressed.load(Ordering::Relaxed),
                    health.suppressed_disabled.load(Ordering::Relaxed),
                    health.suppressed_paused.load(Ordering::Relaxed),
                    health.suppressed_cooldown.load(Ordering::Relaxed),
                    health.undelivered.load(Ordering::Relaxed),
                    health.unavailable.load(Ordering::Relaxed));
            }
        }
        let next = self.pending.lock().take();
        if let Some(reading) = next {
            changed = true;
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
                "MicCamWatch — {} DEV; {active} active; {}",
                self.mode.label(),
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
        }
        if changed && self.ui.as_ref().is_some_and(Supervisor::is_running) {
            let frame = self.frame();
            self.ui
                .as_mut()
                .expect("running supervisor")
                .publish(frame)?;
        }
        Ok(())
    }
    fn notification_probe(&mut self) {
        if self.mode == DevelopmentMode::SettingsProbe
            && let Some(worker) = &self.notifications
        {
            submit_test_sequence(&mut self.probe_alerts, worker, self.lang);
            eprintln!("TEST native notification fixture submitted; no hardware or dashboard observations simulated");
        }
    }
    fn menu(&mut self) -> Result<()> {
        unsafe {
            let menu = CreatePopupMenu()?;
            AppendMenuW(
                menu,
                MF_STRING,
                OPEN,
                PCWSTR(
                    format_wide(match self.mode {
                        DevelopmentMode::Controlled => "Open MicCamWatch — controlled test",
                        DevelopmentMode::Settings => "Open MicCamWatch — isolated preferences",
                        DevelopmentMode::SettingsProbe => "Open MicCamWatch — TEST notifications",
                        DevelopmentMode::ReadOnly => "Open MicCamWatch — read only",
                    })
                    .as_ptr(),
                ),
            )?;
            AppendMenuW(
                menu,
                MF_STRING | MF_GRAYED,
                0,
                PCWSTR(format_wide(&self.summary).as_ptr()),
            )?;
            if self.mode == DevelopmentMode::SettingsProbe {
                AppendMenuW(menu, MF_STRING, TEST_NOTIFICATIONS,
                    PCWSTR(format_wide("TEST native notifications (30-second cooldown)").as_ptr()))?;
            }
            AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null())?;
            AppendMenuW(
                menu,
                if self.can_exit() {
                    MF_STRING
                } else {
                    MF_STRING | MF_GRAYED
                },
                EXIT,
                PCWSTR(format_wide("Exit development tray").as_ptr()),
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
                TEST_NOTIFICATIONS => self.notification_probe(),
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
    if message == PROBE_MESSAGE && w.0 == unsafe { GetCurrentProcessId() } as usize {
        if observer.mode == DevelopmentMode::SettingsProbe {
            observer.notification_probe();
            return LRESULT(1);
        }
        return LRESULT(0);
    }
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
            EXIT if observer.can_exit() => unsafe { DestroyWindow(hwnd) }.map_err(Into::into),
            EXIT => Err(anyhow::anyhow!(
                "Wait for pending preference writes and release controlled microphone requests before exiting"
            )),
            _ => Ok(()), // No mutation command dispatcher exists in this mode.
        },
        WM_CLOSE if observer.can_exit() => unsafe { DestroyWindow(hwnd) }.map_err(Into::into),
        WM_CLOSE => Err(anyhow::anyhow!(
            "Preference write pending or controlled microphone request active/unknown; wait for confirmation before exit"
        )),
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
    let preferences_enabled = observer.mode.preferences_enabled();
    let microphone_generation = observer.microphone_worker.as_ref().map_or_else(
        || Arc::new(AtomicU64::new(0)),
        |worker| Arc::clone(&worker.sequence),
    );
    let lang = observer.lang;
    let notifications = if observer.mode == DevelopmentMode::Settings {
        observer.notifications.clone()
    } else {
        None
    };
    thread::spawn(move || {
        let mut policy = Policy::default();
        let mut applied_profile = crate::settings::PrivacyProfile::Balanced;
        let mut monitor = PlatformMonitor::new(policy.clone()).map_err(|e| format!("{e:#}"));
        let scope = Filter {
            include_ready: true,
            ..Filter::default()
        };
        let mut sequence = 0u64;
        let mut camera = Err("Initial camera observation pending".to_owned());
        let mut camera_at = None;
        let mut capture_alerts = CaptureAlerts::default();
        while alive.load(Ordering::Acquire) {
            if preferences_enabled
                && let Ok(settings) = crate::settings::Settings::load()
                && settings.profile != applied_profile
                && let Ok(monitor) = monitor.as_mut()
            {
                policy.profile = settings.profile.assessment_profile();
                monitor.set_policy(policy.clone());
                applied_profile = settings.profile;
            }
            let observed_at = chrono::Utc::now();
            let microphone_generation = microphone_generation.load(Ordering::Acquire);
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
            // Observe before the UI's coalescing slot: an unattached/slow Flutter
            // client cannot discard capture edges or stop native alert delivery.
            if let (Ok(snapshot), Some(worker)) = (&snapshot, &notifications) {
                capture_alerts.observe(snapshot, worker, lang, false);
            }
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
                microphone_generation,
                applied_profile: monitor.is_ok().then_some(applied_profile),
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

fn reuse_read_only(initial_window: bool, probe: bool) -> Result<bool> {
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
    if probe {
        let accepted = unsafe { SendMessageW(
            hwnd, PROBE_MESSAGE, Some(WPARAM(pid as usize)), Some(LPARAM(0)),
        ) };
        ensure!(accepted.0 == 1,
            "existing development owner is not TEST notification mode; close it before starting the probe");
    }
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

pub(super) fn run(
    lang: Language,
    initial_window: bool,
    theme: &str,
    mode: DevelopmentMode,
) -> Result<()> {
    ensure!(matches!(theme, "light" | "dark"), "invalid desktop theme");
    let controlled = mode == DevelopmentMode::Controlled;
    if controlled {
        verify_controlled_scope()?;
    } else if matches!(mode, DevelopmentMode::Settings | DevelopmentMode::SettingsProbe) {
        verify_isolated_scope()?;
    }
    enable_dpi_awareness();
    let mutex = format_wide("Local\\MicCamWatch.Tray.ReadOnlyDevelopment");
    let _lease = MutexGuard(unsafe { CreateMutexW(None, true, PCWSTR(mutex.as_ptr()))? });
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if reuse_read_only(initial_window, mode == DevelopmentMode::SettingsProbe)? {
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
        controlled,
        mode,
        preference_worker: None,
        microphone_worker: None,
        pending_microphone: 0,
        microphone_generation: 0,
        microphone_result: None,
        presentation_sequence: 0,
        control_used: false,
        notifications: matches!(mode, DevelopmentMode::Settings | DevelopmentMode::SettingsProbe)
            .then(|| NotificationWorker::with_identity(NotificationIdentity::Qualification)),
        probe_alerts: CaptureAlerts::default(),
        last_alert_health: None,
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
    if controlled {
        observer.microphone_worker = Some(super::microphone_worker::Worker::start_controlled(
            Policy::default(),
            Arc::clone(&observer.window),
        ));
    }
    let worker = start_collector(&observer);
    if mode == DevelopmentMode::SettingsProbe {
        observer.notification_probe();
    }
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
            "{{\"event\":\"observer_started\",\"pid\":{},\"mode\":\"{}\"}}",
            unsafe { GetCurrentProcessId() },
            mode.wire_name()
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

fn verify_isolated_scope() -> Result<()> {
    use windows::Win32::{
        System::Com::CoTaskMemFree,
        UI::Shell::{FOLDERID_LocalAppData, KF_FLAG_DEFAULT, SHGetKnownFolderPath},
    };
    let folder = unsafe { SHGetKnownFolderPath(&FOLDERID_LocalAppData, KF_FLAG_DEFAULT, None) }?;
    let known = unsafe { folder.to_string() };
    unsafe {
        CoTaskMemFree(Some(folder.0.cast()));
    }
    let allowed = std::path::PathBuf::from(known?)
        .join("MicCamWatchQualification")
        .canonicalize()?;
    for path in [crate::settings::config_dir()?, crate::settings::data_dir()?] {
        let path = path
            .canonicalize()
            .context("isolated development data/config directories must exist")?;
        ensure!(
            path.starts_with(&allowed),
            "development writes require isolated APPDATA and LOCALAPPDATA under MicCamWatchQualification"
        );
    }
    Ok(())
}

fn verify_controlled_scope() -> Result<()> {
    verify_isolated_scope()?;
    ensure!(
        !crate::windows_control::resource_conflict("microphone")?,
        "another native microphone owner is active; controlled development refused without mutation"
    );
    Ok(())
}
