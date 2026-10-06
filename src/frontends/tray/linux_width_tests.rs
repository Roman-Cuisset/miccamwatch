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
        text.failed
    );
    make_view(
        (diagnostic.clone(), "error"),
        MicrophoneMuteState::Unmuted,
        (false, Some(&diagnostic)),
        &Settings::default(),
        lang,
        &Some(diagnostic.clone()),
        SessionLockState::Unknown,
    )
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
        for item in &menu {
            let ksni::MenuItem::Standard(item) = item else {
                panic!("unexpected native menu item type");
            };
            assert!(!item.label.chars().any(char::is_control));
            let visible = item.label.replace("__", "_");
            assert!(ratatui::text::Span::raw(&visible).width() <= 48);
            assert!(visible.chars().count() <= 97);
        }
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
