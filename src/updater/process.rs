use anyhow::{Context, Result, bail};
use std::{
    ffi::OsString,
    os::windows::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
};
use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE, HWND, LPARAM, WAIT_OBJECT_0, WAIT_TIMEOUT, WPARAM},
        System::Threading::{
            CREATE_NO_WINDOW, CREATE_UNICODE_ENVIRONMENT, CreateProcessW, DETACHED_PROCESS,
            OpenProcess, PROCESS_INFORMATION, PROCESS_NAME_WIN32,
            PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, PROCESS_VM_READ,
            QueryFullProcessImageNameW, STARTUPINFOW, WaitForSingleObject,
        },
        UI::WindowsAndMessaging::{
            AllowSetForegroundWindow, FindWindowW, GetWindowThreadProcessId, SMTO_ABORTIFHUNG,
            SMTO_BLOCK, SendMessageTimeoutW,
        },
    },
    core::{PCWSTR, PWSTR},
};

pub struct RunningTray {
    process: HANDLE,
    window: HWND,
    pid: u32,
    launch: Option<LaunchContext>,
}

#[derive(PartialEq, Eq)]
pub struct LaunchContext {
    target: PathBuf,
    command: Vec<u16>,
    directory: Option<PathBuf>,
    environment: Option<Vec<u16>>,
}
impl Drop for RunningTray {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.process);
        }
    }
}

fn wide(path: &std::ffi::OsStr) -> Vec<u16> {
    path.encode_wide().chain(Some(0)).collect()
}
fn environment_value<'a>(environment: &'a [u16], name: &str) -> Result<&'a [u16]> {
    // Windows variable names used here are ASCII; values remain raw UTF-16.
    let mut found = None;
    for entry in environment
        .split(|unit| *unit == 0)
        .filter(|entry| !entry.is_empty())
    {
        let Some(separator) = entry.iter().position(|unit| *unit == b'=' as u16) else {
            bail!("invalid running tray environment entry");
        };
        if entry[..separator].len() == name.len()
            && entry[..separator]
                .iter()
                .zip(name.bytes())
                .all(|(left, right)| {
                    ascii_environment_unit(*left) == ascii_environment_unit(right as u16)
                })
        {
            if found.is_some() {
                bail!("duplicate running tray environment variable");
            }
            found = Some(&entry[separator + 1..]);
        }
    }
    found.context("running tray application-data namespace is unavailable")
}

fn ascii_environment_unit(unit: u16) -> u16 {
    if (b'A' as u16..=b'Z' as u16).contains(&unit) {
        unit + 32
    } else {
        unit
    }
}

fn ensure_launch_namespace(environment: &[u16]) -> Result<()> {
    // Reservations inspect this updater's namespace. Never stop a frontend whose
    // dormant requests/preferences live elsewhere, even when its image matches.
    for name in ["APPDATA", "LOCALAPPDATA"] {
        let captured = environment_value(environment, name)?;
        let current =
            std::env::var_os(name).context("updater application-data namespace is unavailable")?;
        let captured = PathBuf::from(OsString::from_wide(captured));
        let current = PathBuf::from(current);
        if !captured.is_absolute()
            || !current.is_absolute()
            || crate::windows_control::scope_hash(&captured.join("MicCamWatch"))?
                != crate::windows_control::scope_hash(&current.join("MicCamWatch"))?
        {
            bail!(
                "running tray uses a different {name} namespace; installation unchanged; no process was stopped"
            );
        }
    }
    Ok(())
}

pub fn owned_running_tray(target: &Path) -> Result<Option<RunningTray>> {
    let class = wide(std::ffi::OsStr::new("MicCamWatchTrayClass"));
    let Ok(window) = (unsafe { FindWindowW(PCWSTR(class.as_ptr()), PCWSTR::null()) }) else {
        return Ok(None);
    };
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(window, Some(&mut pid));
    }
    if pid == 0 {
        bail!("could not identify the running tray; no process was stopped");
    }
    let process = unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE | PROCESS_VM_READ,
            false,
            pid,
        )
    }
    .context("cannot inspect running tray ownership; no process was stopped")?;
    let running = RunningTray {
        process,
        window,
        pid,
        launch: None,
    };
    let mut image = [0u16; 32768];
    let mut length = image.len() as u32;
    unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(image.as_mut_ptr()),
            &mut length,
        )
    }
    .context("cannot inspect running tray executable; no process was stopped")?;
    let image = std::path::PathBuf::from(OsString::from_wide(&image[..length as usize]));
    if !same_target(&image, target)? {
        return Ok(None);
    }
    // Recheck the HWND's PID after opening a stable process handle: HWNDs and PIDs are reusable.
    let mut current_pid = 0;
    unsafe {
        GetWindowThreadProcessId(window, Some(&mut current_pid));
    }
    if current_pid != pid || unsafe { WaitForSingleObject(process, 0) } != WAIT_TIMEOUT {
        bail!("running tray changed while inspecting it; retry the update");
    }
    Ok(Some(running))
}

pub fn installed_running_tray(cli: &Path, companion: &Path) -> Result<Option<RunningTray>> {
    // Both are supported frontend hosts. A missing companion must not hide an
    // installed CLI-hosted tray during same-version companion repair.
    for target in [companion, cli] {
        if target.try_exists()?
            && let Some(mut running) = owned_running_tray(target)?
        {
            running.launch = Some(running.launch_context(target)?);
            return Ok(Some(running));
        }
    }
    let class = wide(std::ffi::OsStr::new("MicCamWatchTrayClass"));
    if unsafe { FindWindowW(PCWSTR(class.as_ptr()), PCWSTR::null()) }.is_ok() {
        bail!(
            "running tray does not belong to this installed CLI or companion; installation unchanged; no process was stopped"
        );
    }
    Ok(None)
}

fn same_target(image: &Path, target: &Path) -> Result<bool> {
    let image = std::fs::canonicalize(image).context("cannot resolve running tray executable")?;
    let target =
        std::fs::canonicalize(target).context("cannot resolve installed tray executable")?;
    // Canonicalization resolves junctions/symlinks. Do not match a basename or a sibling installation.
    Ok(image == target)
}

impl RunningTray {
    pub(crate) fn open_window(&self) -> Result<()> {
        let mut pid = 0;
        unsafe { GetWindowThreadProcessId(self.window, Some(&mut pid)) };
        if pid != self.pid || unsafe { WaitForSingleObject(self.process, 0) } != WAIT_TIMEOUT {
            bail!("MicCamWatch owner changed before activation; no other window was opened");
        }
        // Only the explicit foreground launcher can grant this; failure never
        // authorizes stealing input queues or bypassing foreground restrictions.
        let _ = unsafe { AllowSetForegroundWindow(self.pid) };
        let mut reply = 0;
        let sent = unsafe {
            SendMessageTimeoutW(
                self.window,
                crate::frontends::tray::WM_OPEN_DESKTOP,
                WPARAM(self.pid as usize),
                LPARAM(0),
                SMTO_ABORTIFHUNG | SMTO_BLOCK,
                5000,
                Some(&mut reply),
            )
        };
        if sent.0 == 0 || reply != 1 {
            bail!(
                "MicCamWatch did not confirm opening its window; the existing owner was preserved"
            );
        }
        Ok(())
    }

    pub fn launch_context(&self, target: &Path) -> Result<LaunchContext> {
        let (directory, environment) =
            crate::platform::process_launch_directory_environment(self.process)
                .context("cannot preserve running tray launch context; installation unchanged")?;
        ensure_launch_namespace(&environment)?;
        Ok(LaunchContext {
            target: target.to_owned(),
            command: crate::platform::with_process_command_line(self.process, |command| {
                command.to_vec()
            })
            .context("cannot preserve running tray arguments; installation unchanged")?,
            directory: Some(directory),
            environment: Some(environment),
        })
    }

    pub fn restart(&self) -> Result<()> {
        self.launch
            .as_ref()
            .context("running tray launch context was not captured before shutdown")?
            .restart()
    }

    pub fn stop(&self) -> Result<()> {
        if unsafe { WaitForSingleObject(self.process, 0) } == WAIT_OBJECT_0 {
            return Ok(());
        }
        let mut current_pid = 0;
        unsafe {
            GetWindowThreadProcessId(self.window, Some(&mut current_pid));
        }
        if current_pid != self.pid {
            bail!("tray window ownership changed; no process was stopped");
        }
        let mut reply = 0;
        let result = unsafe {
            SendMessageTimeoutW(
                self.window,
                crate::frontends::tray::WM_UPDATE_STOP,
                WPARAM(self.pid as usize),
                LPARAM(0),
                SMTO_ABORTIFHUNG | SMTO_BLOCK,
                5000,
                Some(&mut reply),
            )
        };
        if result.0 == 0 {
            bail!(
                "tray did not acknowledge safe shutdown; installation unchanged; finish pending camera operations and exit the tray manually, then retry"
            );
        }
        match reply {
            crate::frontends::tray::UPDATE_STOPPED => {}
            crate::frontends::tray::UPDATE_BUSY => bail!(
                "tray has pending camera operations (possibly a UAC prompt); installation unchanged; finish them and retry; nothing was cancelled"
            ),
            _ => bail!(
                "this tray does not support safe updater shutdown; installation unchanged; finish pending camera operations and exit the tray manually, then retry"
            ),
        }
        match unsafe { WaitForSingleObject(self.process, 15000) } {
            WAIT_OBJECT_0 => Ok(()),
            WAIT_TIMEOUT => bail!(
                "tray acknowledged shutdown but has not terminated; installation unchanged; wait for it to exit and retry"
            ),
            _ => bail!("failed waiting on the tray process handle; installation unchanged"),
        }
    }
}

#[cfg(test)]
pub fn restart(target: &Path) -> Result<()> {
    LaunchContext {
        target: target.to_owned(),
        command: std::iter::once(b'"' as u16)
            .chain(target.as_os_str().encode_wide())
            .chain([b'"' as u16])
            .collect(),
        directory: None,
        environment: None,
    }
    .restart()
}

impl LaunchContext {
    fn restart(&self) -> Result<()> {
        // Command with NUL stdio still inherits unrelated inheritable handles,
        // including the updater caller's original pipes. Match spawn_owner: none.
        let target = &self.target;
        let application = wide(target.as_os_str());
        let directory = self.directory.as_ref().map(|path| wide(path.as_os_str()));
        let mut command = self.command.clone();
        command.push(0);
        if application[..application.len() - 1]
            .iter()
            .any(|unit| *unit == 0 || *unit == b'"' as u16)
            || self.command.contains(&0)
        {
            return Err(super::transaction::io_error(
                "restart previously running tray",
                target,
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid executable path"),
            ));
        }
        let startup = STARTUPINFOW {
            cb: std::mem::size_of::<STARTUPINFOW>() as u32,
            ..Default::default()
        };
        let mut process = PROCESS_INFORMATION::default();
        unsafe {
            if CreateProcessW(
                PCWSTR(application.as_ptr()),
                Some(PWSTR(command.as_mut_ptr())),
                None,
                None,
                false,
                DETACHED_PROCESS | CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT,
                self.environment.as_ref().map(|block| block.as_ptr().cast()),
                directory
                    .as_ref()
                    .map_or(PCWSTR::null(), |path| PCWSTR(path.as_ptr())),
                &startup,
                &mut process,
            )
            .is_err()
            {
                return Err(super::transaction::io_error(
                    "restart previously running tray",
                    target,
                    std::io::Error::last_os_error(),
                ));
            }
            let _ = CloseHandle(process.hThread);
            let _ = CloseHandle(process.hProcess);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        os::windows::process::CommandExt,
        path::PathBuf,
        process::{Child, Command, ExitStatus, Stdio},
        sync::mpsc,
        thread,
        time::{Duration, Instant},
    };

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn native_launch_snapshot_matches_process_context() -> Result<()> {
        windows::core::link!("kernel32.dll" "system" fn GetCommandLineW() -> PCWSTR);
        let process = unsafe { windows::Win32::System::Threading::GetCurrentProcess() };
        let (directory, environment) =
            crate::platform::process_launch_directory_environment(process)?;
        anyhow::ensure!(
            directory.canonicalize()? == std::env::current_dir()?.canonicalize()?,
            "native working-directory snapshot differs from the current process"
        );
        for name in ["APPDATA", "LOCALAPPDATA"] {
            let expected =
                std::env::var_os(name).context("test application-data namespace absent")?;
            anyhow::ensure!(
                environment_value(&environment, name)?
                    .iter()
                    .copied()
                    .eq(expected.encode_wide()),
                "native namespace snapshot differs from the current process"
            );
        }
        let native_command = unsafe { GetCommandLineW() };
        let arguments_match = crate::platform::with_process_command_line(process, |observed| {
            observed == unsafe { native_command.as_wide() }
        })?;
        anyhow::ensure!(
            arguments_match,
            "native command-line snapshot differs from GetCommandLineW"
        );
        Ok(())
    }

    // This is deliberately not a tray: it never opens a window, settings, controls,
    // devices, or the network. Its standard handles remain open until cooperative
    // stop, so inherited stdout/stderr really would prevent consumer EOF.
    const SUCCESSOR_SOURCE: &str = r#"
use std::{io::Read, path::PathBuf, thread, time::{Duration, Instant}};
fn main() {
    let root = PathBuf::from(std::env::var_os("MCW_TEST_RESTART_ROOT").unwrap());
    std::fs::write(root.join("ready.tmp"), std::process::id().to_string()).unwrap();
    std::fs::rename(root.join("ready.tmp"), root.join("ready")).unwrap();
    let input_root = root.clone();
    thread::spawn(move || {
        let mut byte = [0];
        let state = match std::io::stdin().read(&mut byte) {
            Ok(0) => "eof",
            Ok(_) => "inherited",
            Err(_) => "error",
        };
        std::fs::write(input_root.join("stdin.tmp"), state).unwrap();
        std::fs::rename(input_root.join("stdin.tmp"), input_root.join("stdin")).unwrap();
    });
    let deadline = Instant::now() + Duration::from_secs(60);
    while !root.join("stop").exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
}
"#;

    struct RestartFixture {
        directory: tempfile::TempDir,
        caller: Option<Child>,
        successor: Option<HANDLE>,
        product_tray: bool,
    }

    impl RestartFixture {
        fn root(&self) -> &Path {
            self.directory.path()
        }

        fn retain_successor(&mut self) -> Result<bool> {
            let ready = self.root().join("ready");
            if !ready.try_exists()? {
                return Ok(false);
            }
            let pid = std::fs::read_to_string(ready)?.parse()?;
            let process = unsafe {
                OpenProcess(
                    PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                    false,
                    pid,
                )
            }?;
            // Own the handle before any further fallible work, including the
            // exact-image check. Never clean up a process by its PID/name.
            self.successor = Some(process);
            let mut image = [0u16; 32768];
            let mut length = image.len() as u32;
            unsafe {
                QueryFullProcessImageNameW(
                    process,
                    PROCESS_NAME_WIN32,
                    PWSTR(image.as_mut_ptr()),
                    &mut length,
                )
            }?;
            let image = PathBuf::from(OsString::from_wide(&image[..length as usize]));
            anyhow::ensure!(
                same_target(&image, &self.root().join("successor.exe"))?,
                "successor process does not own the fixture image"
            );
            Ok(true)
        }
    }

    impl Drop for RestartFixture {
        fn drop(&mut self) {
            // Closing caller stdin also unblocks a regressed inheriting child.
            if let Some(caller) = &mut self.caller {
                drop(caller.stdin.take());
            }
            let _ = std::fs::write(self.root().join("stop"), b"stop");
            if let Some(caller) = &mut self.caller {
                // This retained child is only our libtest caller, never MCW.
                if wait_for_caller(caller, Duration::from_secs(5)).is_err() {
                    let _ = caller.kill();
                    let _ = caller.wait();
                }
            }
            if self.caller.is_some() && self.successor.is_none() {
                // A failed assertion can race the successor's readiness write.
                // Keep its directory/stop request alive through its lifetime cap
                // even if startup failed before publishing a process handle.
                let deadline = Instant::now() + Duration::from_secs(65);
                while Instant::now() < deadline {
                    match self.retain_successor() {
                        Ok(true) | Err(_) => break,
                        Ok(false) => thread::sleep(Duration::from_millis(10)),
                    }
                }
            }
            if self.product_tray
                && let Ok(Some(tray)) = owned_running_tray(&self.root().join("successor.exe"))
            {
                let _ = tray.stop();
            }
            if let Some(process) = self.successor.take() {
                unsafe {
                    // Cooperative stop, with the fixture's own 60-second expiry
                    // as a fail-safe; no termination API targets the successor.
                    let _ = WaitForSingleObject(process, 65000);
                    let _ = CloseHandle(process);
                }
            }
        }
    }

    fn wait_for_caller(caller: &mut Child, timeout: Duration) -> Result<ExitStatus> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(status) = caller.try_wait()? {
                return Ok(status);
            }
            anyhow::ensure!(Instant::now() < deadline, "restart caller did not exit");
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn capture_until_eof(
        mut pipe: impl Read + Send + 'static,
    ) -> mpsc::Receiver<std::io::Result<Vec<u8>>> {
        let (send, receive) = mpsc::channel();
        thread::spawn(move || {
            let mut bytes = Vec::new();
            let result = pipe.read_to_end(&mut bytes).map(|_| bytes);
            let _ = send.send(result);
        });
        receive
    }

    #[test]
    #[ignore = "native lifecycle proof; product mode requires a disposable hosted Windows desktop"]
    fn native_restart_releases_captured_streams_while_successor_lives() -> Result<()> {
        let product = std::env::var_os("MCW_TEST_RESTART_PRODUCT");
        if product.is_some() {
            anyhow::ensure!(
                std::env::var("GITHUB_ACTIONS").as_deref() == Ok("true")
                    && std::env::var("RUNNER_ENVIRONMENT").as_deref() == Ok("github-hosted"),
                "real tray restart proof requires a disposable hosted Windows runner"
            );
        }
        if let Some(root) = std::env::var_os("MCW_TEST_RESTART_ROOT") {
            let root = PathBuf::from(root);
            let deadline = Instant::now() + Duration::from_secs(5);
            while !root.join("launch").try_exists()? {
                anyhow::ensure!(Instant::now() < deadline, "caller input was not prepared");
                thread::sleep(Duration::from_millis(10));
            }
            if product.is_some() {
                crate::settings::Settings::update(|settings| {
                    settings.notifications_enabled = false;
                    settings.history_enabled = false;
                    settings.mute_on_lock = false;
                    settings.block_camera_on_lock = false;
                    settings.restore_on_unlock = false;
                })?;
            }
            restart(&root.join("successor.exe"))?;
            if product.is_some() {
                let deadline = Instant::now() + Duration::from_secs(20);
                let tray = loop {
                    if let Some(tray) = owned_running_tray(&root.join("successor.exe"))? {
                        break tray;
                    }
                    anyhow::ensure!(Instant::now() < deadline, "real tray did not become ready");
                    thread::sleep(Duration::from_millis(10));
                };
                std::fs::write(root.join("ready.tmp"), tray.pid.to_string())?;
                std::fs::rename(root.join("ready.tmp"), root.join("ready"))?;
            }
            println!("restart caller completed");
            eprintln!("restart caller stderr completed");
            return Ok(());
        }
        let mut fixture = RestartFixture {
            directory: tempfile::Builder::new()
                .prefix("mcw restart Кириллица ")
                .tempdir()?,
            caller: None,
            successor: None,
            product_tray: product.is_some(),
        };
        if let Some(product) = product {
            std::fs::copy(product, fixture.root().join("successor.exe"))?;
        } else {
            let source = fixture.root().join("successor.rs");
            let target = fixture.root().join("successor.exe");
            std::fs::write(&source, SUCCESSOR_SOURCE)?;
            let compiled = Command::new("rustc")
                .args(["--edition=2024", "--crate-name=mcw_restart_successor"])
                .arg(&source)
                .arg("-o")
                .arg(&target)
                .output()?;
            anyhow::ensure!(
                compiled.status.success(),
                "cannot compile controlled successor: {}",
                String::from_utf8_lossy(&compiled.stderr)
            );
        }
        fixture.caller = Some(
            Command::new(std::env::current_exe()?)
                .args([
                    "--exact",
                    "--ignored",
                    "updater::process::tests::native_restart_releases_captured_streams_while_successor_lives",
                    "--test-threads=1",
                    "--nocapture",
                ])
                .env("MCW_TEST_RESTART_ROOT", fixture.root())
                .env("APPDATA", fixture.root())
                .env("LOCALAPPDATA", fixture.root())
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .creation_flags(0x0800_0000)
                .spawn()?,
        );
        let caller = fixture.caller.as_mut().unwrap();
        let stdout = capture_until_eof(caller.stdout.take().unwrap());
        let stderr = capture_until_eof(caller.stderr.take().unwrap());
        // Keep this pipe OPEN, with readable input. An inheriting successor
        // reports "inherited", not EOF; closing stdin here would hide that bug.
        caller.stdin.as_mut().unwrap().write_all(b"not inherited")?;
        caller.stdin.as_mut().unwrap().flush()?;
        std::fs::write(fixture.directory.path().join("launch"), b"launch")?;
        let status = wait_for_caller(caller, Duration::from_secs(30))?;
        anyhow::ensure!(status.success(), "restart caller failed: {status}");
        let deadline = Instant::now() + Duration::from_secs(5);
        while !fixture.retain_successor()? {
            anyhow::ensure!(Instant::now() < deadline, "successor did not become ready");
            thread::sleep(Duration::from_millis(10));
        }
        let successor = fixture.successor.unwrap();
        assert_eq!(unsafe { WaitForSingleObject(successor, 0) }, WAIT_TIMEOUT);
        let stdout = stdout
            .recv_timeout(Duration::from_secs(5))
            .context("stdout consumer did not receive EOF while successor was alive")??;
        let stderr = stderr
            .recv_timeout(Duration::from_secs(5))
            .context("stderr consumer did not receive EOF while successor was alive")??;
        assert!(String::from_utf8_lossy(&stdout).contains("restart caller completed"));
        assert!(String::from_utf8_lossy(&stderr).contains("restart caller stderr completed"));
        if !fixture.product_tray {
            let deadline = Instant::now() + Duration::from_secs(5);
            let input = fixture.root().join("stdin");
            while !input.try_exists()? {
                anyhow::ensure!(
                    Instant::now() < deadline,
                    "successor stdin read did not finish"
                );
                thread::sleep(Duration::from_millis(10));
            }
            assert_eq!(std::fs::read_to_string(input)?, "eof");
        }
        assert_eq!(unsafe { WaitForSingleObject(successor, 0) }, WAIT_TIMEOUT);
        // Verify cooperative cleanup on the success path as well as in Drop.
        if fixture.product_tray {
            owned_running_tray(&fixture.root().join("successor.exe"))?
                .context("real tray disappeared before cooperative shutdown")?
                .stop()?;
        } else {
            std::fs::write(fixture.root().join("stop"), b"stop")?;
        }
        assert_eq!(
            unsafe { WaitForSingleObject(successor, 5000) },
            WAIT_OBJECT_0
        );
        Ok(())
    }
    #[test]
    fn ownership_requires_exact_installed_target() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let installed_root = temp.path().join("installed");
        let foreign_root = temp.path().join("foreign");
        let backup_root = installed_root.join(".mcw-update-test");
        std::fs::create_dir_all(&backup_root)?;
        std::fs::create_dir(&foreign_root)?;
        for name in ["mcw.exe", "mcw-tray.exe"] {
            let installed = installed_root.join(name);
            let foreign = foreign_root.join(name);
            std::fs::write(&installed, b"owned")?;
            std::fs::write(&foreign, b"other")?;
            assert!(same_target(&installed, &installed)?);
            assert!(!same_target(&foreign, &installed)?);
            let previous = backup_root.join(format!("previous-{name}"));
            std::fs::rename(&installed, &previous)?;
            std::fs::write(&installed, b"successor")?;
            assert!(!same_target(&previous, &installed)?);
        }
        Ok(())
    }

    #[test]
    fn namespace_uses_canonical_identity_and_refuses_a_different_data_root() -> Result<()> {
        fn environment(appdata: &Path, localappdata: &Path) -> Vec<u16> {
            let mut block = Vec::new();
            for (name, path) in [("APPDATA=", appdata), ("LOCALAPPDATA=", localappdata)] {
                block.extend(name.encode_utf16());
                block.extend(path.as_os_str().encode_wide());
                block.push(0);
            }
            block.push(0);
            block
        }
        let appdata = PathBuf::from(std::env::var_os("APPDATA").context("APPDATA required")?);
        let localappdata =
            PathBuf::from(std::env::var_os("LOCALAPPDATA").context("LOCALAPPDATA required")?);
        ensure_launch_namespace(&environment(&appdata.join("."), &localappdata.join(".")))?;
        let different = tempfile::tempdir()?;
        assert!(ensure_launch_namespace(&environment(&appdata, different.path())).is_err());
        Ok(())
    }

    #[test]
    fn environment_lookup_is_lossless_and_rejects_ambiguous_names() -> Result<()> {
        let mut block: Vec<u16> = "=C:=C:\\work\0localappdata=".encode_utf16().collect();
        block.extend([0xd800, 0, 0]);
        assert_eq!(environment_value(&block, "LOCALAPPDATA")?, &[0xd800]);
        block.pop();
        block.extend("LOCALAPPDATA=other\0\0".encode_utf16());
        assert!(environment_value(&block, "LOCALAPPDATA").is_err());
        Ok(())
    }
}
