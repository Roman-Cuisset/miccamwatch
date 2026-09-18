use crate::{cli::Filter, i18n::Language, model::Resource, platform::PlatformMonitor};
use anyhow::{Context, Result};
use std::{ffi::OsStr, os::windows::ffi::OsStrExt, ptr};
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
                AppendMenuW, CreateIconIndirect, CreatePopupMenu, CreateWindowExW, DefWindowProcW,
                DestroyIcon, DestroyMenu, DestroyWindow, DispatchMessageW, GetCursorPos,
                GetMessageW, HICON, ICONINFO, KillTimer, MF_DISABLED, MF_GRAYED, MF_SEPARATOR,
                MF_STRING, MSG, PostQuitMessage, RegisterClassW, SetForegroundWindow, SetTimer,
                TPM_BOTTOMALIGN, TPM_RIGHTBUTTON, TrackPopupMenu, TranslateMessage,
                WINDOW_EX_STYLE, WM_APP, WM_COMMAND, WM_DESTROY, WM_LBUTTONDBLCLK, WM_RBUTTONUP,
                WM_TIMER, WNDCLASSW, WS_OVERLAPPED,
            },
        },
    },
    core::PCWSTR,
};

const WM_TRAY_CALLBACK: u32 = WM_APP + 1;
const TIMER_POLL_ID: usize = 1;

const CMD_TOGGLE_MUTE: usize = 101;
const CMD_STATUS_TOAST: usize = 102;
const CMD_EXIT: usize = 103;

struct TrayAppState {
    monitor: PlatformMonitor,
    lang: Language,
    active: bool,
    summary: String,
    muted: bool,
    green_icon: HICON,
    red_icon: HICON,
}

static mut TRAY_STATE: Option<TrayAppState> = None;

pub fn run_tray(monitor: PlatformMonitor, lang: Language) -> Result<()> {
    let green_icon = create_circle_icon(34, 197, 94).context("failed to create green tray icon")?;
    let red_icon = create_circle_icon(239, 68, 68).context("failed to create red tray icon")?;
    let muted = monitor.get_microphone_mute().unwrap_or(false);

    let initial_state = TrayAppState {
        monitor,
        lang,
        active: false,
        summary: "miccamwatch: Idle".to_owned(),
        muted,
        green_icon,
        red_icon,
    };
    unsafe {
        TRAY_STATE = Some(initial_state);
    }

    let class_name: Vec<u16> = OsStr::new("MicCamWatchTrayClass")
        .encode_wide()
        .chain(Some(0))
        .collect();

    let wc = WNDCLASSW {
        lpfnWndProc: Some(tray_wnd_proc),
        lpszClassName: PCWSTR(class_name.as_ptr()),
        ..Default::default()
    };
    unsafe {
        let _ = RegisterClassW(&wc);
    }

    let window_title: Vec<u16> = OsStr::new("MicCamWatchTray")
        .encode_wide()
        .chain(Some(0))
        .collect();

    let hwnd = unsafe {
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
            None,
        )
    }
    .context("failed to create tray message window")?;

    let mut nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: 1,
        uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
        uCallbackMessage: WM_TRAY_CALLBACK,
        hIcon: green_icon,
        ..Default::default()
    };
    set_tooltip(&mut nid, "miccamwatch: Idle");

    let ok = unsafe { Shell_NotifyIconW(NIM_ADD, &nid) };
    if !ok.as_bool() {
        anyhow::bail!("failed to register system tray icon");
    }

    // Timer every 500ms on the main GUI thread - no cross-thread COM issues!
    unsafe {
        SetTimer(Some(hwnd), TIMER_POLL_ID, 500, None);
    }

    println!("miccamwatch tray running. Right-click the icon near the clock to control.");

    // Message loop
    let mut msg = MSG::default();
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    unsafe {
        let _ = KillTimer(Some(hwnd), TIMER_POLL_ID);
        let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
        let _ = DestroyIcon(green_icon);
        let _ = DestroyIcon(red_icon);
    }
    Ok(())
}

fn set_tooltip(nid: &mut NOTIFYICONDATAW, text: &str) {
    let wide: Vec<u16> = OsStr::new(text).encode_wide().collect();
    let max = nid.szTip.len().saturating_sub(1);
    let len = wide.len().min(max);
    nid.szTip[..len].copy_from_slice(&wide[..len]);
    nid.szTip[len] = 0;
}

unsafe extern "system" fn tray_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_TIMER => {
            unsafe {
                if let Some(state) = &mut *ptr::addr_of_mut!(TRAY_STATE) {
                    let filter = Filter::default();
                    if let Ok(snapshot) = state.monitor.snapshot(&filter) {
                        let is_active = !snapshot.accesses.is_empty();
                        let summary = if is_active {
                            let apps: Vec<String> = snapshot
                                .accesses
                                .iter()
                                .map(|a| {
                                    let res = match a.resource {
                                        Resource::Microphone => "Mic",
                                        Resource::Camera => "Cam",
                                    };
                                    format!("{}: {}", a.application, res)
                                })
                                .collect();
                            format!("miccamwatch: {}", apps.join(", "))
                        } else {
                            match state.lang {
                                Language::Fr => "miccamwatch : Aucun accès actif".to_owned(),
                                Language::De => "miccamwatch: Keine aktiven Zugriffe".to_owned(),
                                Language::Es => "miccamwatch: Sin accesos activos".to_owned(),
                                Language::Ja => "miccamwatch: アクティブなアクセスなし".to_owned(),
                                Language::Zh => "miccamwatch: 无活动访问".to_owned(),
                                Language::Ru => "miccamwatch: Нет активных доступов".to_owned(),
                                Language::En => "miccamwatch: Idle".to_owned(),
                            }
                        };
                        let muted = state.monitor.get_microphone_mute().unwrap_or(false);

                        let changed = state.active != is_active
                            || state.summary != summary
                            || state.muted != muted;
                        state.active = is_active;
                        state.summary = summary;
                        state.muted = muted;

                        if changed {
                            let mut nid = NOTIFYICONDATAW {
                                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                                hWnd: hwnd,
                                uID: 1,
                                uFlags: NIF_ICON | NIF_TIP,
                                hIcon: if state.active {
                                    state.red_icon
                                } else {
                                    state.green_icon
                                },
                                ..Default::default()
                            };
                            set_tooltip(&mut nid, &state.summary);
                            let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
                        }
                    }
                }
            }
            LRESULT(0)
        }
        WM_TRAY_CALLBACK => {
            let event = lparam.0 as u32;
            match event {
                WM_RBUTTONUP => {
                    show_context_menu(hwnd);
                }
                WM_LBUTTONDBLCLK => {
                    show_status_toast();
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            let id = wparam.0 & 0xffff;
            match id {
                CMD_TOGGLE_MUTE => unsafe {
                    if let Some(state) = &mut *ptr::addr_of_mut!(TRAY_STATE)
                        && let Ok(new_mute) = state.monitor.toggle_microphone_mute()
                    {
                        state.muted = new_mute;
                        let title = "miccamwatch";
                        let msg = if new_mute {
                            "Microphone MUTED"
                        } else {
                            "Microphone UNMUTED"
                        };
                        crate::notify::notify_access(
                            &crate::model::Access {
                                key: "mute:toggle".into(),
                                resource: Resource::Microphone,
                                activity: crate::model::Activity::Active,
                                risk: crate::model::Risk::Expected,
                                confidence: crate::model::Confidence::High,
                                application: "System".into(),
                                pid: None,
                                parent_pid: None,
                                parent_name: None,
                                executable: None,
                                signature: None,
                                device: None,
                                started_at: None,
                                modules: vec![],
                                evidence: vec![],
                                process: None,
                            },
                            crate::model::Action::Update,
                            Language::En,
                        );
                        println!("{title}: {msg}");
                    }
                },
                CMD_STATUS_TOAST => {
                    show_status_toast();
                }
                CMD_EXIT => unsafe {
                    let _ = DestroyWindow(hwnd);
                    PostQuitMessage(0);
                },
                _ => {}
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            unsafe {
                PostQuitMessage(0);
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn show_context_menu(hwnd: HWND) {
    let (lang, summary, muted) = unsafe {
        if let Some(state) = &*ptr::addr_of!(TRAY_STATE) {
            (state.lang, state.summary.clone(), state.muted)
        } else {
            return;
        }
    };

    unsafe {
        let menu = match CreatePopupMenu() {
            Ok(m) => m,
            Err(_) => return,
        };

        let title_str = format_wide("miccamwatch v0.9.0");
        let _ = AppendMenuW(
            menu,
            MF_STRING | MF_DISABLED | MF_GRAYED,
            0,
            PCWSTR(title_str.as_ptr()),
        );

        let summary_str = format_wide(&summary);
        let _ = AppendMenuW(
            menu,
            MF_STRING | MF_DISABLED | MF_GRAYED,
            0,
            PCWSTR(summary_str.as_ptr()),
        );

        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR(ptr::null()));

        let mute_text = if muted {
            match lang {
                Language::Fr => "Activer le micro (Unmute)",
                Language::De => "Mikrofon einschalten (Unmute)",
                Language::Es => "Activar micrófono (Unmute)",
                Language::Ja => "マイクのミュート解除 (Unmute)",
                Language::Zh => "取消静音麦克风 (Unmute)",
                Language::Ru => "Включить микрофон (Unmute)",
                Language::En => "Unmute microphone",
            }
        } else {
            match lang {
                Language::Fr => "Couper le micro (Mute)",
                Language::De => "Mikrofon stummschalten (Mute)",
                Language::Es => "Silenciar micrófono (Mute)",
                Language::Ja => "マイクをミュート (Mute)",
                Language::Zh => "静音麦克风 (Mute)",
                Language::Ru => "Заглушить микрофон (Mute)",
                Language::En => "Mute microphone",
            }
        };
        let mute_str = format_wide(mute_text);
        let _ = AppendMenuW(menu, MF_STRING, CMD_TOGGLE_MUTE, PCWSTR(mute_str.as_ptr()));

        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR(ptr::null()));

        let exit_text = match lang {
            Language::Fr => "Quitter",
            Language::De => "Beenden",
            Language::Es => "Salir",
            Language::Ja => "終了",
            Language::Zh => "退出",
            Language::Ru => "Выход",
            Language::En => "Exit",
        };
        let exit_str = format_wide(exit_text);
        let _ = AppendMenuW(menu, MF_STRING, CMD_EXIT, PCWSTR(exit_str.as_ptr()));

        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let _ = SetForegroundWindow(hwnd);
        let _ = TrackPopupMenu(
            menu,
            TPM_RIGHTBUTTON | TPM_BOTTOMALIGN,
            pt.x,
            pt.y,
            None,
            hwnd,
            None,
        );
        let _ = DestroyMenu(menu);
    }
}

fn show_status_toast() {
    let (summary, lang) = unsafe {
        if let Some(state) = &*ptr::addr_of!(TRAY_STATE) {
            (state.summary.clone(), state.lang)
        } else {
            return;
        }
    };

    let access = crate::model::Access {
        key: "tray:status".into(),
        resource: Resource::Camera,
        activity: crate::model::Activity::Active,
        risk: crate::model::Risk::Expected,
        confidence: crate::model::Confidence::High,
        application: summary,
        pid: None,
        parent_pid: None,
        parent_name: None,
        executable: None,
        signature: None,
        device: None,
        started_at: None,
        modules: vec![],
        evidence: vec![],
        process: None,
    };
    crate::notify::notify_access(&access, crate::model::Action::Update, lang);
}

fn format_wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(Some(0)).collect()
}

fn create_circle_icon(r: u8, g: u8, b: u8) -> Result<HICON> {
    unsafe {
        let dc = CreateCompatibleDC(None);
        if dc.is_invalid() {
            anyhow::bail!("failed to create compatible DC for tray icon");
        }

        let hbm_color = CreateCompatibleBitmap(dc, 16, 16);
        let mask_bits = [0xffu8; 32];
        let hbm_mask = CreateBitmap(16, 16, 1, 1, Some(mask_bits.as_ptr() as _));

        let old_bmp = SelectObject(dc, hbm_color.into());

        let bg_brush = CreateSolidBrush(COLORREF(0));
        let brush = CreateSolidBrush(COLORREF(r as u32 | ((g as u32) << 8) | ((b as u32) << 16)));

        let old_brush = SelectObject(dc, brush.into());
        let _ = Ellipse(dc, 1, 1, 15, 15);

        let _ = SelectObject(dc, old_brush);
        let _ = SelectObject(dc, old_bmp);
        let _ = DeleteObject(brush.into());
        let _ = DeleteObject(bg_brush.into());
        let _ = DeleteDC(dc);

        let info = ICONINFO {
            fIcon: true.into(),
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: hbm_mask,
            hbmColor: hbm_color,
        };

        let icon = CreateIconIndirect(&info);
        let _ = DeleteObject(hbm_color.into());
        let _ = DeleteObject(hbm_mask.into());

        icon.context("failed to create icon from indirect descriptor")
    }
}
