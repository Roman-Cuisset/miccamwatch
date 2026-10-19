use super::*;
use ksni::Tray;

fn fixture_view(lang: Language) -> View {
    let text = words(lang);
    let payload = if matches!(lang, Language::Zh) {
        "运行时摄像头错误_原始完整诊断<&>\n第二行\t控制字符\r".repeat(180)
    } else {
        "runtime camera error_ full original diagnostic<&>\nsecond line\tcontrol\r".repeat(180)
    };
    let diagnostic = format!(
        "{}: BEGIN-LINUX-DIAGNOSTIC {payload} END-LINUX-DIAGNOSTIC",
        text.degraded
    );
    make_view(
        (
            format!(
                "capture-client: MIC ACTIVE — {}\n\n{diagnostic}",
                text.degraded
            ),
            "partial",
        ),
        MicrophoneMuteState::Unmuted,
        (false, Some(&diagnostic)),
        &Settings::default(),
        lang,
        &None,
        MenuContext {
            lock: SessionLockState::Unknown,
            explicit_policy: false,
            inspection: &CameraInspectionUi::default(),
        },
    )
}

fn assert_menu_bounds(menu: &[ksni::MenuItem<LinuxTray>]) {
    for item in menu {
        let label = match item {
            ksni::MenuItem::Standard(item) => &item.label,
            ksni::MenuItem::Checkmark(item) => &item.label,
            ksni::MenuItem::SubMenu(item) => {
                assert_menu_bounds(&item.submenu);
                &item.label
            }
            _ => panic!("unexpected native menu item type"),
        };
        assert!(!label.chars().any(char::is_control));
        let visible = label.replace("__", "_");
        assert!(ratatui::text::Span::raw(&visible).width() <= 48);
        assert!(visible.chars().count() <= 97);
    }
}

fn require_isolated_desktop() -> Result<()> {
    for (name, expected) in [
        ("GITHUB_ACTIONS", "true"),
        ("RUNNER_ENVIRONMENT", "github-hosted"),
        ("MCW_NATIVE_LINUX_ISOLATED", "1"),
    ] {
        ensure!(
            std::env::var(name).as_deref() == Ok(expected),
            "{name} must be {expected}"
        );
    }
    let root = PathBuf::from(std::env::var("MCW_NATIVE_LINUX_ROOT")?);
    ensure!(
        root.is_absolute() && root.is_dir(),
        "missing isolated native root"
    );
    for name in [
        "HOME",
        "XDG_CONFIG_HOME",
        "XDG_DATA_HOME",
        "XDG_STATE_HOME",
        "XDG_RUNTIME_DIR",
        "XDG_CACHE_HOME",
    ] {
        ensure!(
            PathBuf::from(std::env::var(name)?).starts_with(&root),
            "{name} escapes private root"
        );
    }
    ensure!(
        std::env::var("PIPEWIRE_REMOTE").as_deref() == Ok("mcw-native-no-hardware"),
        "native fixture must not connect to the host PipeWire server"
    );
    Ok(())
}

#[test]
fn native_linux_menu_bounds_preserve_complete_diagnostics() {
    for lang in [
        Language::En,
        Language::Fr,
        Language::De,
        Language::Es,
        Language::Ja,
        Language::Zh,
        Language::Ru,
    ] {
        let view = fixture_view(lang);
        let original = view.clone();
        let (sender, _receiver) = mpsc::sync_channel(16);
        let tray = LinuxTray {
            view,
            actions: sender,
            ready: Arc::new(AtomicBool::new(false)),
        };
        let menu = tray.menu();
        assert!(menu.len() <= 16);
        assert_menu_bounds(&menu);
        let document = diagnostic_document(&original);
        for entry in &original.items {
            assert!(document.contains(entry.detail.as_deref().unwrap_or(&entry.label)));
        }
    }
    assert_eq!(
        linux_menu_label("日本語 — 摄像头_状态"),
        "日本語 — 摄像头__状态"
    );
    assert_eq!(
        linux_menu_label("one\ntwo\tthree\u{0007}"),
        "one two three "
    );
    assert!(linux_menu_label(&"界".repeat(500)).ends_with('…'));
    assert!(linux_menu_label(&"\u{0301}".repeat(500)).chars().count() <= 97);
}

// Runs only in an explicitly provisioned disposable native desktop. The Python
// harness opens the real panel/GTK menu; no production fault-injection flag exists.
#[test]
#[ignore = "requires real XFCE StatusNotifier, X11 and notification daemon"]
fn native_linux_tray_width() -> Result<()> {
    require_isolated_desktop()?;
    let proof = PathBuf::from(std::env::var("MCW_LINUX_MENU_PROOF")?);
    fs::create_dir_all(&proof)?;
    let lang = if std::env::var("MCW_LINUX_MENU_CASE")? == "cjk" {
        Language::Zh
    } else {
        Language::En
    };
    let view = fixture_view(lang);
    fs::write(proof.join("view.json"), serde_json::to_vec_pretty(&view)?)?;
    let document = diagnostic_document(&view);
    fs::write(proof.join("diagnostic.txt"), &document)?;
    let (sender, receiver) = mpsc::sync_channel(16);
    let ready = Arc::new(AtomicBool::new(false));
    let mut host = DesktopHost::start(sender, ready, view)?;
    fs::write(proof.join("ready"), b"actual ksni item registered\n")?;
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut delivered = false;
    while Instant::now() < deadline {
        host.check()?;
        match receiver.recv_timeout(Duration::from_millis(100)) {
            Ok(Command::Status) => {
                crate::notify::notify_message("MicCamWatch", &document)?;
                fs::write(
                    proof.join("status-delivered"),
                    b"actual native Status action delivered full notification body\n",
                )?;
                delivered = true;
            }
            Ok(Command::Exit) => {
                ensure!(delivered, "native details action was not exercised");
                host.shutdown()?;
                return Ok(());
            }
            Ok(command) => bail!("native proof selected unexpected command: {command:?}"),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(error) => return Err(error.into()),
        }
    }
    bail!("native panel proof did not select Details and Exit before its deadline")
}

fn profile_fixture_view(settings: &Settings, lang: Language, explicit_policy: bool) -> View {
    make_view(
        ("Isolated profile menu qualification".to_owned(), "idle"),
        MicrophoneMuteState::Unavailable,
        (false, None),
        settings,
        lang,
        &None,
        MenuContext {
            lock: SessionLockState::Unknown,
            explicit_policy,
            inspection: &CameraInspectionUi::default(),
        },
    )
}

#[test]
#[ignore = "requires explicitly isolated GitHub-hosted XFCE desktop"]
fn native_linux_tray_profiles() -> Result<()> {
    require_isolated_desktop()?;
    let proof = PathBuf::from(std::env::var("MCW_LINUX_MENU_PROOF")?);
    fs::create_dir_all(&proof)?;
    let lang = Language::from_code(&std::env::var("MCW_LINUX_PROFILE_LANGUAGE")?)
        .context("unsupported native proof language")?;
    let explicit = std::env::var_os("MCW_LINUX_EXPLICIT_CONFIG").map(PathBuf::from);
    if let Some(path) = &explicit {
        ensure!(
            path.starts_with(PathBuf::from(std::env::var("MCW_NATIVE_LINUX_ROOT")?)),
            "explicit policy must remain inside the private fixture root"
        );
        // Parse a real private policy, without creating a hardware monitor.
        Policy::load(Some(path.as_path()))?;
        let before = fs::read(crate::settings::settings_path()?)?;
        for command in [
            Command::ProfileBalanced,
            Command::ProfilePrivate,
            Command::ProfileMeeting,
            Command::ProfileDevelopment,
        ] {
            ensure!(
                select_profile(command, true, lang).is_err(),
                "explicit config accepted mutation"
            );
            ensure!(
                fs::read(crate::settings::settings_path()?)? == before,
                "refused action modified settings"
            );
        }
        fs::write(
            proof.join("backend-refused"),
            b"all four production profile actions refused\n",
        )?;
    }
    let mut settings = Settings::load()?;
    let view = profile_fixture_view(&settings, lang, explicit.is_some());
    fs::write(proof.join("view-0.json"), serde_json::to_vec_pretty(&view)?)?;
    let (sender, receiver) = mpsc::sync_channel(16);
    let ready = Arc::new(AtomicBool::new(false));
    let mut host = DesktopHost::start(sender, ready, view)?;
    let ready = proof.join("ready");
    let staging = proof.join("ready.tmp");
    fs::write(&staging, std::process::id().to_string())?;
    fs::rename(staging, ready)?;
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut selections = 0;
    while Instant::now() < deadline {
        host.check()?;
        match receiver.recv_timeout(Duration::from_millis(100)) {
            Ok(Command::Exit) => {
                host.shutdown()?;
                return Ok(());
            }
            Ok(
                command @ (Command::ProfileBalanced
                | Command::ProfilePrivate
                | Command::ProfileMeeting
                | Command::ProfileDevelopment),
            ) => {
                settings = select_profile(command, explicit.is_some(), lang)?;
                ensure!(
                    Settings::load()?.profile == settings.profile,
                    "profile was not persisted"
                );
                let view = profile_fixture_view(&settings, lang, explicit.is_some());
                host.update(view.clone())?;
                selections += 1;
                fs::write(
                    proof.join(format!("view-{selections}.json")),
                    serde_json::to_vec_pretty(&view)?,
                )?;
            }
            Ok(command) => {
                bail!("native profile proof selected unsafe/unexpected command: {command:?}")
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(error) => return Err(error.into()),
        }
    }
    bail!("native profile proof did not exit before its deadline")
}
