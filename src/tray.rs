use crate::{
    cli::Filter,
    i18n::Language,
    model::{Activity, CollectorState, MicrophoneMuteState, Resource},
    platform::PlatformMonitor,
};
use anyhow::{Context, Result};
use std::{ffi::OsStr, os::windows::ffi::OsStrExt};
use windows::{
    Win32::{
        Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, WPARAM},
        Graphics::Gdi::{
            CreateBitmap, CreateCompatibleBitmap, CreateCompatibleDC, CreateSolidBrush, DeleteDC,
            DeleteObject, Ellipse, SelectObject,
        },
        UI::{
            Shell::{
                NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
                Shell_NotifyIconW,
            },
            WindowsAndMessaging::{
                AppendMenuW, CREATESTRUCTW, CreateIconIndirect, CreatePopupMenu, CreateWindowExW,
                DefWindowProcW, DestroyIcon, DestroyMenu, DestroyWindow, DispatchMessageW,
                GWLP_USERDATA, GetCursorPos, GetMessageW, GetWindowLongPtrW, HICON, ICONINFO,
                KillTimer, MF_DISABLED, MF_GRAYED, MF_SEPARATOR, MF_STRING, MSG, PostQuitMessage,
                RegisterClassW, SetForegroundWindow, SetTimer, SetWindowLongPtrW, TPM_BOTTOMALIGN,
                TPM_RIGHTBUTTON, TrackPopupMenu, TranslateMessage, WINDOW_EX_STYLE, WM_APP,
                WM_COMMAND, WM_DESTROY, WM_LBUTTONDBLCLK, WM_NCCREATE, WM_RBUTTONUP, WM_TIMER,
                WNDCLASSW, WS_OVERLAPPED,
            },
        },
    },
    core::PCWSTR,
};

const WM_TRAY_CALLBACK: u32 = WM_APP + 1;
const TIMER_POLL_ID: usize = 1;
const CMD_TOGGLE_MUTE: usize = 101;
const CMD_EXIT: usize = 103;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TrayVisual {
    Idle,
    Ready,
    Active,
    Error,
}

struct TrayAppState {
    monitor: PlatformMonitor,
    lang: Language,
    visual: TrayVisual,
    summary: String,
    mute_state: MicrophoneMuteState,
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

pub fn run_tray(monitor: PlatformMonitor, lang: Language) -> Result<()> {
    let state = Box::new(TrayAppState {
        mute_state: monitor
            .microphone_mute_state()
            .unwrap_or(MicrophoneMuteState::Unavailable),
        monitor,
        lang,
        visual: TrayVisual::Idle,
        summary: idle_text(lang).to_owned(),
        green_icon: create_circle_icon(34, 197, 94)?,
        yellow_icon: create_circle_icon(245, 158, 11)?,
        red_icon: create_circle_icon(239, 68, 68)?,
        gray_icon: create_circle_icon(107, 114, 128)?,
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

fn refresh_state(hwnd: HWND, state: &mut TrayAppState) {
    let filter = Filter {
        include_ready: true,
        ..Filter::default()
    };
    let (visual, summary) = match state.monitor.snapshot(&filter) {
        Ok(snapshot) => {
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
    if visual == state.visual && summary == state.summary && mute_state == state.mute_state {
        return;
    }
    state.visual = visual;
    state.summary = summary;
    state.mute_state = mute_state;
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

fn create_circle_icon(red: u8, green: u8, blue: u8) -> Result<HICON> {
    unsafe {
        let dc = CreateCompatibleDC(None);
        if dc.is_invalid() {
            anyhow::bail!("failed to create compatible DC for tray icon");
        }
        let color_bitmap = CreateCompatibleBitmap(dc, 16, 16);
        let mask_bits = [0u8; 32];
        let mask_bitmap = CreateBitmap(16, 16, 1, 1, Some(mask_bits.as_ptr().cast()));
        let old_bitmap = SelectObject(dc, color_bitmap.into());
        let brush = CreateSolidBrush(COLORREF(
            red as u32 | ((green as u32) << 8) | ((blue as u32) << 16),
        ));
        let old_brush = SelectObject(dc, brush.into());
        let _ = Ellipse(dc, 1, 1, 15, 15);
        let _ = SelectObject(dc, old_brush);
        let _ = SelectObject(dc, old_bitmap);
        let _ = DeleteObject(brush.into());
        let _ = DeleteDC(dc);
        let info = ICONINFO {
            fIcon: true.into(),
            hbmMask: mask_bitmap,
            hbmColor: color_bitmap,
            ..Default::default()
        };
        let icon = CreateIconIndirect(&info);
        let _ = DeleteObject(color_bitmap.into());
        let _ = DeleteObject(mask_bitmap.into());
        icon.context("failed to create tray icon")
    }
}

use std::mem::size_of;
