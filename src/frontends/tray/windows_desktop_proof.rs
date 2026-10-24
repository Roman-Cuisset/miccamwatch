//! Interactive proof of the production HWND; simulated observations, never hardware.
use super::*;
use std::path::{Path, PathBuf};
use windows::Win32::UI::WindowsAndMessaging::{
    GWL_STYLE, GetWindowTextLengthW, GetWindowTextW, IsWindow, PM_REMOVE, PeekMessageW, WS_DISABLED,
};
#[link(name = "user32")]
unsafe extern "system" {
    fn SetForegroundWindow(hwnd: HWND) -> i32;
    fn SetActiveWindow(hwnd: HWND) -> HWND;
    fn IsWindowVisible(hwnd: HWND) -> i32;
    fn IsWindowEnabled(hwnd: HWND) -> i32;
}
/// Activate only this fixture; never attach another application's input queue.
fn activate(window: HWND) {
    unsafe {
        let _ = SetForegroundWindow(window);
        let _ = SetActiveWindow(window);
    }
}

#[derive(Default)]
struct ProofOwner {
    commands: Vec<usize>,
    preferences: Vec<(usize, bool)>,
    error: Option<String>,
}
unsafe extern "system" fn proof_owner(hwnd: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if message == WM_NCCREATE {
        let create = unsafe { &*(l.0 as *const CREATESTRUCTW) };
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
        }
        return LRESULT(1);
    }
    let pointer = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *const RefCell<ProofOwner>;
    if let Some(data) = unsafe { pointer.as_ref() }
        && let Ok(mut data) = data.try_borrow_mut()
    {
        match message {
            WM_COMMAND => {
                data.commands.push(w.0);
                return LRESULT(0);
            }
            WM_DESKTOP_SETTING => {
                data.preferences.push((w.0, l.0 != 0));
                // Autostart deliberately has no implementation in this inert sink.
                // Every other preference uses the real transactional persistence.
                if w.0 != AUTOSTART
                    && let Err(error) = update_desktop_preferences(w.0, l.0 != 0)
                {
                    data.error = Some(error.to_string());
                }
                return LRESULT(0);
            }
            _ => {}
        }
    }
    unsafe { DefWindowProcW(hwnd, message, w, l) }
}
fn pump() {
    let mut message = MSG::default();
    while unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.as_bool() {
        if !translate_message(&message) {
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
}
fn text(hwnd: HWND) -> String {
    let mut text = vec![0u16; unsafe { GetWindowTextLengthW(hwnd) } as usize + 1];
    let length = unsafe { GetWindowTextW(hwnd, &mut text) };
    String::from_utf16(&text[..length as usize]).unwrap()
}
fn command(window: &DesktopWindow, id: i32) {
    unsafe {
        SendMessageW(window.hwnd, WM_COMMAND, Some(WPARAM(id as usize)), None);
    }
    pump();
}
fn snapshot_fixture() -> Snapshot {
    Snapshot {
        collectors: vec![crate::model::CollectorHealth {
            collector: "simulated proof collector",
            state: CollectorState::Degraded,
            detail: Some("SIMULATED: camera coverage unavailable; no devices queried".into()),
        }],
        accesses: vec![Access {
            key: "simulated-microphone".into(),
            resource: Resource::Microphone,
            activity: Activity::Active,
            risk: crate::model::Risk::Expected,
            confidence: crate::model::Confidence::High,
            enforcement: crate::model::EnforcementDecision::Alert,
            application: "SIMULATED · Meeting app / Réunion / 会議 / 会议 / Совещание".into(),
            pid: Some(4242),
            parent_pid: None,
            parent_name: None,
            executable: None,
            signature: None,
            device: None,
            started_at: None,
            modules: Vec::new(),
            evidence: Vec::new(),
            process: None,
        }],
        observation_gaps: vec![Resource::Camera],
    }
}
fn microphone_status_fixture(
    requested: bool,
    actual: MicrophoneMuteState,
    guard: bool,
) -> MicrophoneProtectionStatus {
    MicrophoneProtectionStatus {
        requested,
        service_active: guard,
        mute_state: actual,
        endpoint_count: 1,
        hardware_mute_count: 0,
        corrections: 0,
        detail: Some("SIMULATED readback; no endpoints queried".into()),
    }
}
fn camera_status_fixture(
    requested: bool,
    actual: CameraPrivacyState,
    guard: bool,
) -> CameraControlObservation {
    CameraControlObservation {
        desired_blocked: requested,
        state: actual,
        present_total: 1,
        owned_blocked_present: usize::from(actual == CameraPrivacyState::Blocked),
        pending_restore: 0,
        absent_owned: 0,
        unknown_devices: 0,
        helper_active: guard,
        detail: Some("SIMULATED readback; no devices queried".into()),
    }
}

fn fixture(lang: Language) -> View {
    let snapshot = snapshot_fixture();
    let overview = Overview::from_snapshot(Ok(&snapshot), lang, chrono::Utc::now());
    View {
        lang, observed: format!("{}: {} · SIMULATED", last_scan_attempt(lang), overview.observed.unwrap().to_rfc3339()),
        microphone: format!("{}\r\n{}", lang.resource(Resource::Microphone), overview.activity[0]),
        camera: format!("{}\r\n{}", lang.resource(Resource::Camera), overview.activity[1]),
        mic_protection: microphone_card(lang, Some(&microphone_status_fixture(false, MicrophoneMuteState::Muted, false))), cam_protection: camera_card(lang, Some(&camera_status_fixture(true, CameraPrivacyState::Blocked, false))),
        apps: overview.apps,
        diagnostics: format!("SIMULATED native production-window proof; not hardware validation.\n{}\n{}", overview.coverage, "Evidence line — é 界 🛡\n".repeat(160)),
        error: "SIMULATED: camera observation unavailable. No hardware or autostart actions are performed.".into(),
        profile: 2, explicit: false, checks: [true, true, false, false, false, false, false, false],
        mic_enabled: false, cam_enabled: false, resume_enabled: false,
        mic_action: lang.protection_action(false).into(), cam_action: camera_block_label(lang).into(),
    }
}
fn capture(
    directory: &Path,
    step: &mut usize,
    name: &str,
    window: &DesktopWindow,
    simulation: &str,
) {
    pump();
    let data = window.data.borrow();
    let record = serde_json::json!({ "step": *step, "name": name, "window": window.hwnd.0 as usize,
        "pid": std::process::id(), "dpi": unsafe { GetDpiForWindow(window.hwnd) }, "tab": data.tab,
        "scroll": data.scroll[data.tab], "background": data.background.0, "foreground": data.foreground.0,
        "simulation": simulation, "hardware_validation": false,
        "theme_limit": "Native BUTTON ownerdraw uses semantic light/dark or system high-contrast colors; checkbox/radio accessibility and scrollbar glyphs stay native. No global theme or DPI settings changed." });
    drop(data);
    let stem = format!("stage-{:03}", *step);
    std::fs::write(
        directory.join(format!("{stem}.json")),
        serde_json::to_vec_pretty(&record).unwrap(),
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(90);
    while !directory.join(format!("{stem}.captured")).exists() {
        assert!(
            Instant::now() < deadline,
            "native capture driver did not acknowledge {name}"
        );
        pump();
        thread::sleep(Duration::from_millis(10));
    }
    *step += 1;
}

#[test]
fn observation_projection_preserves_unknown_and_positive_activity() {
    let now = chrono::Utc::now();
    let empty = Snapshot {
        collectors: Vec::new(),
        accesses: Vec::new(),
        observation_gaps: Vec::new(),
    };
    let view = Overview::from_snapshot(Ok(&empty), Language::En, now);
    assert_eq!(view.observed, Some(now));
    assert_eq!(
        view.activity,
        [unknown(Language::En), unknown(Language::En)]
    );
    assert!(view.apps.starts_with(unknown(Language::En)));
    let failed = Overview::from_snapshot(Err("collector failed"), Language::En, now);
    assert_eq!(failed.activity, view.activity);
    assert!(failed.coverage.contains("collector failed"));
    let projected = fixture(Language::En);
    assert!(projected.microphone.contains(active(Language::En)));
    assert!(projected.camera.contains(unknown(Language::En)));
}

#[test]
fn overview_bounds_application_rows_and_utf8_names() {
    let mut snapshot = snapshot_fixture();
    let row = snapshot.accesses[0].clone();
    snapshot.accesses = (0..9)
        .map(|i| {
            let mut row = row.clone();
            row.application = if i == 0 {
                "界".repeat(300)
            } else {
                format!("unique-visible-app-{i}")
            };
            row
        })
        .collect();
    let overview = Overview::from_snapshot(Ok(&snapshot), Language::En, chrono::Utc::now());
    assert!(overview.apps.contains("unique-visible-app-7"));
    assert!(!overview.apps.contains("unique-visible-app-8"));
    assert!(
        !overview.apps.contains(&"界".repeat(86)),
        "names are bounded by UTF-8 bytes, not character count"
    );
    assert!(overview.activity[0].contains(active(Language::En)));
}

#[test]
fn observation_requires_resource_collectors_and_preserves_positive_evidence() {
    let now = chrono::Utc::now();
    let mut snapshot = snapshot_fixture();
    snapshot.accesses.clear();
    snapshot.observation_gaps.clear();
    snapshot.collectors = vec![crate::model::CollectorHealth {
        collector: "wasapi",
        state: CollectorState::Healthy,
        detail: None,
    }];
    let audio_only = Overview::from_snapshot(Ok(&snapshot), Language::En, now);
    assert_ne!(audio_only.activity[0], unknown(Language::En));
    assert_eq!(audio_only.activity[1], unknown(Language::En));
    assert!(audio_only.apps.starts_with(unknown(Language::En)));
    for collector in ["privacy_store", "module_scanner", "sensor_activity"] {
        snapshot.collectors.push(crate::model::CollectorHealth {
            collector,
            state: CollectorState::Healthy,
            detail: None,
        });
    }
    let covered = Overview::from_snapshot(Ok(&snapshot), Language::En, now);
    assert_ne!(covered.activity[1], unknown(Language::En));
    assert!(covered.apps.starts_with(empty(Language::En)));
    snapshot.collectors[0].state = CollectorState::Unavailable;
    let camera_only = Overview::from_snapshot(Ok(&snapshot), Language::En, now);
    assert_eq!(camera_only.activity[0], unknown(Language::En));
    assert_ne!(camera_only.activity[1], unknown(Language::En));
    snapshot
        .collectors
        .retain(|collector| collector.collector != "sensor_activity");
    assert_eq!(
        Overview::from_snapshot(Ok(&snapshot), Language::En, now).activity[1],
        unknown(Language::En)
    );
    let mut access = snapshot_fixture().accesses.remove(0);
    access.resource = Resource::Camera;
    snapshot.accesses.push(access);
    snapshot.observation_gaps.push(Resource::Camera);
    assert!(
        Overview::from_snapshot(Ok(&snapshot), Language::En, now).activity[1]
            .contains(active(Language::En))
    );
    snapshot.accesses[0].activity = Activity::Ready;
    assert!(
        Overview::from_snapshot(Ok(&snapshot), Language::En, now).activity[1]
            .contains(ready(Language::En))
    );
    let failed = Overview::from_snapshot(Err("SIMULATED scan failure"), Language::En, now);
    assert_eq!(
        failed.observed,
        Some(now),
        "timestamp denotes last scan attempt, including failures"
    );
    assert_eq!(
        failed.activity,
        [unknown(Language::En), unknown(Language::En)]
    );
}

#[test]
fn protection_readback_does_not_follow_request_and_reports_inactive_guards() {
    for lang in [
        Language::En,
        Language::Fr,
        Language::De,
        Language::Es,
        Language::Ja,
        Language::Zh,
        Language::Ru,
    ] {
        let mut microphone = microphone_status_fixture(false, MicrophoneMuteState::Muted, false);
        let muted = microphone_card(lang, Some(&microphone));
        assert!(muted.contains(mute_readback(lang, MicrophoneMuteState::Muted)));
        assert!(muted.contains(lang.service_state(false)));
        microphone.requested = true;
        let requested_muted = microphone_card(lang, Some(&microphone));
        assert!(requested_muted.contains(mute_readback(lang, MicrophoneMuteState::Muted)));
        assert!(requested_muted.contains(lang.service_state(false)));
        for actual in [
            MicrophoneMuteState::Unmuted,
            MicrophoneMuteState::Mixed,
            MicrophoneMuteState::Unavailable,
        ] {
            microphone.mute_state = actual;
            assert!(microphone_card(lang, Some(&microphone)).contains(mute_readback(lang, actual)));
        }
        let mut camera = camera_status_fixture(false, CameraPrivacyState::Blocked, false);
        let blocked = camera_card(lang, Some(&camera));
        assert!(blocked.contains(camera_readback(lang, &camera)));
        assert!(blocked.contains(lang.service_state(false)));
        camera.desired_blocked = true;
        assert!(camera_card(lang, Some(&camera)).contains(camera_readback(lang, &camera)));
        assert!(camera_card(lang, Some(&camera)).contains(lang.service_state(false)));
        camera.state = CameraPrivacyState::Allowed;
        assert_ne!(blocked, camera_card(lang, Some(&camera)));
        camera.unknown_devices = 1;
        assert!(camera_card(lang, Some(&camera)).contains(unknown(lang)));
        assert!(microphone_card(lang, None).contains(lang.unknown_protection(true)));
        assert!(camera_card(lang, None).contains(lang.unknown_protection(false)));
    }
}

#[test]
#[ignore = "interactive production HWND: installer/tests/windows-desktop-native.ps1"]
fn native_windows_desktop() {
    let scope =
        PathBuf::from(std::env::var_os("MCW_WINDOWS_DESKTOP_SCOPE").expect("isolated scope"));
    let directory = PathBuf::from(
        std::env::var_os("MCW_WINDOWS_DESKTOP_PROOF_DIR").expect("capture directory"),
    );
    assert!(scope.is_absolute() && directory.starts_with(&scope));
    assert_eq!(
        std::env::var_os("APPDATA").map(PathBuf::from),
        Some(scope.join("roaming"))
    );
    assert_eq!(
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from),
        Some(scope.join("local"))
    );
    assert_eq!(
        crate::settings::settings_path().unwrap(),
        scope.join("roaming/MicCamWatch/settings.toml")
    );
    enable_dpi_awareness();
    // The real PlatformMonitor caller already owns an MTA initialization.
    // The GUI runtime must retain its own reference after that caller ends.
    assert!(unsafe { RoInitialize(1) } >= 0);
    DESKTOP_RUNTIME.with(|_| ());
    let data = Box::new(RefCell::new(ProofOwner::default()));
    let class = format_wide("MicCamWatchIsolatedDesktopProofOwner");
    unsafe {
        let _ = RegisterClassW(&WNDCLASSW {
            lpfnWndProc: Some(proof_owner),
            lpszClassName: PCWSTR(class.as_ptr()),
            ..Default::default()
        });
    }
    let owner = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(class.as_ptr()),
            PCWSTR::null(),
            WS_OVERLAPPED,
            0,
            0,
            1,
            1,
            None,
            None,
            None,
            Some((&*data as *const RefCell<ProofOwner>).cast()),
        )
    }
    .unwrap();
    let mut step = 0;
    for (index, language) in ["en", "fr", "de", "es", "ja", "zh", "ru"]
        .into_iter()
        .enumerate()
    {
        let lang = Language::from_code(language).unwrap();
        let original = fixture(lang);
        let mut window = DesktopWindow::create(owner, original.clone()).unwrap();
        if index == 0 {
            unsafe { RoUninitialize() };
            let (mut apartment, mut qualifier) = (Default::default(), Default::default());
            unsafe {
                windows::Win32::System::Com::CoGetApartmentType(&mut apartment, &mut qualifier)
            }
            .expect("GUI must retain COM after the caller releases its initialization");
            assert_eq!(apartment, windows::Win32::System::Com::APTTYPE_MTA);
            assert_eq!(
                qualifier,
                windows::Win32::System::Com::APTTYPEQUALIFIER_NONE
            );
        }
        window.show();
        pump();
        assert_eq!(
            text(control(window.hwnd, LIMIT).unwrap()),
            diagnostic_document(lang.protection_limit())
        );
        let before = data.borrow().commands.len();
        // Injected commands must still respect model capability gates.
        for id in [MIC_ACTION, CAM_ACTION, CAM_RESUME] {
            let child = control(window.hwnd, id).unwrap();
            assert_ne!(
                unsafe { GetWindowLongPtrW(child, GWL_STYLE) } as u32 & WS_DISABLED.0,
                0
            );
            command(&window, id);
        }
        assert_eq!(data.borrow().commands.len(), before);
        for tab in 0..4 {
            command(&window, TAB + tab);
            capture(
                &directory,
                &mut step,
                &format!("{language}-native-tab-{tab}"),
                &window,
                "observations only; native system theme and actual window DPI",
            );
        }
        command(&window, TAB + 2);
        let diagnostic = control(window.hwnd, DIAGNOSTIC).unwrap();
        activate(window.hwnd);
        unsafe {
            SetFocus(diagnostic);
            assert_eq!(
                GetFocus(),
                diagnostic,
                "native editor accepts keyboard focus"
            );
            SendMessageW(diagnostic, EM_SETSEL, Some(WPARAM(12)), Some(LPARAM(36)));
            SendMessageW(diagnostic, EM_LINESCROLL, None, Some(LPARAM(8)));
        }
        let first = unsafe { SendMessageW(diagnostic, EM_GETFIRSTVISIBLELINE, None, None) }.0;
        let mut updated = original.clone();
        updated.diagnostics.push_str("\nNew accepted frame");
        window.update(updated);
        pump();
        let mut start = 0u32;
        let mut end = 0u32;
        unsafe {
            SendMessageW(
                diagnostic,
                EM_GETSEL,
                Some(WPARAM((&mut start as *mut u32) as usize)),
                Some(LPARAM((&mut end as *mut u32) as isize)),
            );
        }
        assert_eq!((start, end), (12, 36));
        assert_eq!(
            unsafe { GetFocus() },
            diagnostic,
            "accepted refresh preserves native keyboard focus"
        );
        assert_eq!(
            unsafe { SendMessageW(diagnostic, EM_GETFIRSTVISIBLELINE, None, None) }.0,
            first
        );
        assert_eq!(window.data.borrow().tab, 2);
        assert!(text(diagnostic).ends_with("New accepted frame"));
        let keyboard = MSG {
            hwnd: diagnostic,
            message: 0x0100,
            wParam: WPARAM(9),
            ..Default::default()
        };
        assert!(
            translate_message(&keyboard),
            "native Tab navigation must be handled for this owned HWND"
        );
        let stop = unsafe { GetFocus() };
        assert_ne!(stop, diagnostic, "Tab must leave the diagnostics editor");
        assert_eq!(unsafe { GetAncestor(stop, 2) }, window.hwnd);
        assert_ne!(unsafe { IsWindowVisible(stop) }, 0);
        assert_ne!(unsafe { IsWindowEnabled(stop) }, 0);
        let selected_tab = window.data.borrow().tab;
        unsafe {
            SendMessageW(window.hwnd, WM_CLOSE, None, None);
        }
        assert!(!window.is_visible());
        assert!(unsafe { IsWindow(Some(window.hwnd)) }.as_bool());
        let hidden_text = text(diagnostic);
        window.update(original.clone());
        assert_eq!(
            text(diagnostic),
            hidden_text,
            "hidden refresh must defer native text updates"
        );
        window.show();
        pump();
        assert_eq!(text(diagnostic), diagnostic_document(&original.diagnostics));
        assert!(window.is_visible());
        assert_eq!(window.data.borrow().tab, selected_tab);
        assert_eq!(
            data.borrow().commands.len(),
            before,
            "refresh/show/hide may not issue device actions"
        );
        // Production preference handler receives desired booleans, not stale toggles.
        command(&window, TAB + 1);
        let checkbox = control(window.hwnd, SOUND as i32).unwrap();
        unsafe {
            SendMessageW(checkbox, 0x00f5, None, None);
        } // BM_CLICK uses native auto-checkbox behavior.
        pump();
        command(&window, SOUND as i32); // Same desired false again must remain false.
        assert!(!Settings::load().unwrap().sound_enabled);
        assert!(Settings::load().unwrap().notifications_enabled);
        assert_eq!(
            data.borrow().preferences[data.borrow().preferences.len() - 2..],
            [(SOUND, false), (SOUND, false)]
        );
        for id in [
            NOTIFICATIONS,
            HISTORY,
            MUTE_LOCK,
            CAMERA_LOCK,
            RESTORE_UNLOCK,
        ] {
            let check = control(window.hwnd, id as i32).unwrap();
            unsafe {
                SendMessageW(check, BM_SETCHECK, Some(WPARAM(1)), None);
            }
            command(&window, id as i32);
            command(&window, id as i32);
        }
        let saved = Settings::load().unwrap();
        assert!(
            saved.notifications_enabled
                && saved.history_enabled
                && saved.mute_on_lock
                && saved.block_camera_on_lock
                && saved.restore_on_unlock
        );
        assert!(
            !saved.sound_enabled,
            "unrelated preference must survive every update"
        );
        let pause = control(window.hwnd, PAUSE as i32).unwrap();
        unsafe {
            SendMessageW(pause, BM_SETCHECK, Some(WPARAM(1)), None);
        }
        command(&window, PAUSE as i32);
        let until = Settings::load().unwrap().pause_notifications_until;
        command(&window, PAUSE as i32);
        assert_eq!(
            Settings::load().unwrap().pause_notifications_until,
            until,
            "repeated desired pause must not extend its deadline"
        );
        unsafe {
            SendMessageW(pause, BM_SETCHECK, Some(WPARAM(0)), None);
        }
        command(&window, PAUSE as i32);
        assert!(
            Settings::load()
                .unwrap()
                .pause_notifications_until
                .is_none()
        );
        assert!(data.borrow().error.is_none());
        let mut locked = original.clone();
        locked.explicit = true;
        window.update(locked);
        command(&window, PROFILE);
        assert_eq!(
            data.borrow().commands.len(),
            before,
            "explicit config must reject profiles"
        );
        window.update(original.clone());
        command(&window, PROFILE + 1);
        assert_eq!(data.borrow().commands.last(), Some(&CMD_PROFILE_PRIVATE));
        let mut enabled = original.clone();
        enabled.mic_enabled = true;
        enabled.cam_enabled = true;
        enabled.resume_enabled = true;
        window.update(enabled);
        for (id, expected) in [
            (MIC_ACTION, CMD_TOGGLE_MUTE),
            (CAM_ACTION, CMD_TOGGLE_CAMERA),
            (CAM_RESUME, CMD_RESUME_CAMERA),
        ] {
            command(&window, id);
            assert_eq!(data.borrow().commands.last(), Some(&expected));
        }
        window.update(original);
        let dpi = unsafe { GetDpiForWindow(window.hwnd) }.max(96) as i32;
        let work = monitor_work_area(window.hwnd);
        unsafe {
            let _ = MoveWindow(
                window.hwnd,
                work.left,
                work.top,
                560 * dpi / 96,
                480 * dpi / 96,
                false,
            );
        }
        for tab in 0..2 {
            command(&window, TAB + tab);
            let nav = control(window.hwnd, TAB + tab).unwrap();
            let mut header_before = RECT::default();
            unsafe {
                GetWindowRect(nav, &mut header_before);
            }
            capture(
                &directory,
                &mut step,
                &format!("{language}-minimum-tab-{tab}-top"),
                &window,
                "observations only; actual DPI; minimum size",
            );
            if tab == 1 {
                let setting = control(window.hwnd, RESTORE_UNLOCK as i32).unwrap();
                let viewport = control(window.hwnd, PAGE_VIEWPORT).unwrap();
                let mut page = RECT::default();
                let mut bounds = RECT::default();
                unsafe {
                    assert_ne!(GetWindowRect(viewport, &mut page), 0);
                    assert_ne!(GetWindowRect(setting, &mut bounds), 0);
                }
                assert!(
                    bounds.bottom > page.bottom,
                    "fixture must exercise an offscreen setting"
                );
                unsafe { SetFocus(setting) };
                pump();
                assert_eq!(unsafe { GetFocus() }, setting);
                assert_ne!(unsafe { GetWindowRect(setting, &mut bounds) }, 0);
                assert!(
                    bounds.top >= page.top
                        && bounds.bottom <= page.bottom
                        && bounds.left >= page.left
                        && bounds.right <= page.right,
                    "keyboard focus must reveal the entire offscreen setting"
                );
            }
            unsafe {
                SendMessageW(window.hwnd, 0x0115, Some(WPARAM(7)), None);
            }
            let mut header_after = RECT::default();
            unsafe {
                GetWindowRect(nav, &mut header_after);
            }
            assert_eq!(
                header_before, header_after,
                "page scrolling must not move navigation"
            );
            let scroll = window.data.borrow().scroll[tab as usize];
            let focus = unsafe { GetFocus() };
            let unchanged = window.data.borrow().view.clone();
            window.update(unchanged);
            assert_eq!(window.data.borrow().scroll[tab as usize], scroll);
            assert_eq!(unsafe { GetFocus() }, focus);
            capture(
                &directory,
                &mut step,
                &format!("{language}-minimum-tab-{tab}-bottom"),
                &window,
                "observations only; actual DPI; scrolled minimum size",
            );
        }
        if language == "en" {
            unsafe {
                let _ = MoveWindow(
                    window.hwnd,
                    work.left,
                    work.top,
                    (920 * dpi / 96).min(work.right - work.left),
                    (800 * dpi / 96).min(work.bottom - work.top),
                    false,
                );
            }
            for (name, bg, fg) in [
                ("light", COLORREF(0xffffff), COLORREF(0x202020)),
                ("dark", COLORREF(0x202020), COLORREF(0xf0f0f0)),
            ] {
                window.data.borrow_mut().palette(window.hwnd, bg, fg, false);
                for tab in 0..3 {
                    window.data.borrow_mut().scroll[tab] = 0;
                    command(&window, TAB + tab as i32);
                    capture(
                        &directory,
                        &mut step,
                        &format!("en-{name}-palette-tab-{tab}"),
                        &window,
                        "simulated light/dark semantic palette, not an OS theme switch; actual DPI",
                    );
                }
            }
        }
        let hwnd = window.hwnd;
        window.destroy();
        pump();
        assert!(!unsafe { IsWindow(Some(hwnd)) }.as_bool());
    }
    let _ = unsafe { DestroyWindow(owner) };
    std::fs::write(directory.join("complete.json"), serde_json::to_vec_pretty(&serde_json::json!({ "captures": step, "languages": 7, "hardware_validation": false, "settings_scope": scope, "device_actions_executed": 0, "autostart_actions_executed": 0, "keyboard_focus_verified": true, "checks": ["production render/update", "unknown and positive observation", "capability gates", "desired preference persistence/idempotence", "explicit profile lock", "native keyboard focus/selection/scroll stability", "retained hide/reopen", "native theme and actual DPI", "simulated light/dark semantic client palette"] })).unwrap()).unwrap();
}
