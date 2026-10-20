use anyhow::{Context, Result, bail, ensure};
use std::{
    io,
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
};
use windows::{
    Win32::Storage::FileSystem::{
        MOVEFILE_WRITE_THROUGH, MoveFileExW, REPLACE_FILE_FLAGS, ReplaceFileW,
    },
    core::PCWSTR,
};

pub struct Replacement {
    pub staged: PathBuf,
    pub target: PathBuf,
    pub backup: PathBuf,
    pub had_original: bool,
    // Only the updater's own mapped CLI image, never a locked foreign tray.
    pub running_cli: bool,
}

/// The transaction only starts after all downloads and version checks have succeeded.
/// Tray swaps are atomic; the mapped CLI uses recoverable, same-volume renames.
pub trait Operations {
    fn reserve(&mut self) -> Result<()>;
    fn release(&mut self);
    fn stop(&mut self) -> Result<()>;
    fn install(&mut self, index: usize) -> Result<()>;
    fn rollback(&mut self, index: usize) -> Result<()>;
    fn restart(&mut self) -> Result<()>;
}

#[derive(Debug)]
struct IncompleteRecovery(anyhow::Error);
impl std::fmt::Display for IncompleteRecovery {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "interrupted replacement recovery failed: {:#}",
            self.0
        )
    }
}
impl std::error::Error for IncompleteRecovery {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.0.as_ref())
    }
}

pub fn apply(ops: &mut impl Operations, count: usize, was_running: bool) -> Result<()> {
    if count == 0 {
        return Ok(());
    }
    // Reserve before frontend shutdown: it can resolve automatic restoration
    // ownership, while manual protection remains independently owned.
    ops.reserve().context(
        "update refused; installed binaries and tray unchanged; protection was not released",
    )?;
    if was_running && let Err(error) = ops.stop() {
        ops.release();
        return Err(error);
    }
    let mut installed = 0;
    let mut reserved = true;
    let result: Result<()> = (|| {
        for index in 0..count {
            ops.install(index)?;
            installed += 1;
        }
        // Restart may resume requested controls. Never hold their locks across startup.
        ops.release();
        reserved = false;
        if was_running {
            ops.restart()?;
        }
        Ok(())
    })();
    if let Err(error) = result {
        let mut recovery_errors = Vec::new();
        if error.chain().any(|cause| cause.is::<IncompleteRecovery>()) {
            recovery_errors
                .push("interrupted replacement was not recovered; tray remains stopped".to_owned());
        }
        // A failed restart may already have activated a helper. Do not replace
        // either image again unless its ownership can be reserved anew.
        if !reserved {
            match ops.reserve() {
                Ok(()) => reserved = true,
                Err(reservation) => {
                    bail!(
                        "update restart failed: {error:#}; rollback refused because protection may have resumed: {reservation:#}; transaction backups retained; protection was not released"
                    );
                }
            }
        }
        for index in (0..installed).rev() {
            if let Err(rollback) = ops.rollback(index) {
                recovery_errors.push(format!("rollback: {rollback:#}"));
            }
        }
        if reserved {
            ops.release();
        }
        // Do not start an inconsistent pair after a failed rollback.
        if was_running
            && recovery_errors.is_empty()
            && let Err(restart) = ops.restart()
        {
            recovery_errors.push(format!("restart original tray: {restart:#}"));
        }
        if recovery_errors.is_empty() {
            return Err(error.context("update failed; previous installation preserved"));
        }
        bail!(
            "update failed: {error:#}; recovery needs attention: {}",
            recovery_errors.join("; ")
        );
    }
    Ok(())
}

struct ProtectionReservation {
    _microphone_request: crate::windows_control::ResourceLease,
    _camera_request: crate::windows_control::ResourceLease,
    _microphone: crate::windows_control::ResourceLease,
    _camera: crate::windows_control::ResourceLease,
}
impl ProtectionReservation {
    fn acquire() -> Result<Self> {
        let camera_request = crate::windows_control::acquire_request_lock("camera").context(
            "cannot reserve camera control requests; a pending operation may still own them",
        )?;
        let microphone_request = crate::windows_control::acquire_request_lock("microphone")
            .context("cannot reserve microphone control requests; a pending operation may still own them")?;
        let identity = crate::windows_control::current_identity()?;
        let microphone = crate::windows_control::acquire_resource_for("microphone", &identity)
            .context("cannot reserve native microphone ownership; a protection owner may still be active")?;
        let camera = crate::windows_control::acquire_resource_for("camera", &identity).context(
            "cannot reserve native camera ownership; a protection owner may still be active",
        )?;
        // Live owners are excluded globally, including foreign operational scopes.
        // Read dormant/manual/automatic intent only after locking out new owners.
        crate::platform::ensure_microphone_update_inactive()?;
        crate::privacy::ensure_camera_update_inactive()?;
        Ok(Self {
            _microphone_request: microphone_request,
            _camera_request: camera_request,
            _microphone: microphone,
            _camera: camera,
        })
    }
}

pub struct NativeOperations {
    pub replacements: Vec<Replacement>,
    pub running: Option<super::process::RunningTray>,
    reservation: Option<ProtectionReservation>,
}
impl NativeOperations {
    pub fn new(
        replacements: Vec<Replacement>,
        running: Option<super::process::RunningTray>,
    ) -> Self {
        Self {
            replacements,
            running,
            reservation: None,
        }
    }
}
impl Operations for NativeOperations {
    fn reserve(&mut self) -> Result<()> {
        ensure!(self.reservation.is_none(), "update already reserved");
        self.reservation = Some(ProtectionReservation::acquire()?);
        Ok(())
    }
    fn release(&mut self) {
        self.reservation = None;
    }
    fn stop(&mut self) -> Result<()> {
        self.running.as_ref().unwrap().stop()
    }
    fn install(&mut self, index: usize) -> Result<()> {
        ensure!(
            self.reservation.is_some(),
            "replacement requires protection reservation"
        );
        let replacement = &self.replacements[index];
        swap(
            &replacement.staged,
            &replacement.target,
            &replacement.backup,
            replacement.running_cli,
        )
    }
    fn rollback(&mut self, index: usize) -> Result<()> {
        let replacement = &self.replacements[index];
        if replacement.had_original && !replacement.backup.try_exists()? {
            bail!(
                "transaction backup {} disappeared; do not restore quarantine; inspect Windows Security > Protection history",
                replacement.backup.display()
            );
        }
        ensure!(
            self.reservation.is_some(),
            "rollback requires protection reservation"
        );
        if replacement.had_original {
            // Only move an extant transaction backup. Never recover quarantined bytes.
            let discarded = replacement.staged.with_extension("failed");
            swap(
                &replacement.backup,
                &replacement.target,
                &discarded,
                replacement.running_cli,
            )
        } else {
            match std::fs::remove_file(&replacement.target) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(io_error(
                    "roll back newly installed file",
                    &replacement.target,
                    error,
                )),
            }
        }
    }
    fn restart(&mut self) -> Result<()> {
        ensure!(
            self.reservation.is_none(),
            "tray restart requires released protection reservation"
        );
        self.running
            .as_ref()
            .context("cannot restart a tray that was not running before update")?
            .restart()
    }
}

fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

fn swap_running_cli(staged: &Path, target: &Path, backup: &Path) -> Result<()> {
    let source_w = wide(staged);
    let target_w = wide(target);
    let backup_w = wide(backup);
    // ReplaceFile cannot replace a mapped PE. Rename our own image first;
    // neither move replaces an existing destination or launches a helper.
    unsafe {
        MoveFileExW(
            PCWSTR(target_w.as_ptr()),
            PCWSTR(backup_w.as_ptr()),
            MOVEFILE_WRITE_THROUGH,
        )
    }
    .map_err(|error| {
        io_error(
            "retain running CLI transaction backup",
            target,
            io::Error::from_raw_os_error(error.code().0 & 0xffff),
        )
    })?;
    if let Err(error) = unsafe {
        MoveFileExW(
            PCWSTR(source_w.as_ptr()),
            PCWSTR(target_w.as_ptr()),
            MOVEFILE_WRITE_THROUGH,
        )
    } {
        let original = io_error(
            "install staged CLI",
            target,
            io::Error::from_raw_os_error(error.code().0 & 0xffff),
        );
        if let Err(recovery) = unsafe {
            MoveFileExW(
                PCWSTR(backup_w.as_ptr()),
                PCWSTR(target_w.as_ptr()),
                MOVEFILE_WRITE_THROUGH,
            )
        } {
            return Err(anyhow::Error::new(IncompleteRecovery(io_error(
                "restore running CLI transaction backup",
                target,
                io::Error::from_raw_os_error(recovery.code().0 & 0xffff),
            )))
            .context(original));
        }
        return Err(original);
    }
    Ok(())
}

fn swap(staged: &Path, target: &Path, backup: &Path, running_cli: bool) -> Result<()> {
    if running_cli {
        return swap_running_cli(staged, target, backup);
    }
    let source_w = wide(staged);
    let target_w = wide(target);
    let backup_w = wide(backup);
    let exists = target
        .try_exists()
        .map_err(|error| io_error("inspect replacement target", target, error))?;
    let result = unsafe {
        if exists {
            ReplaceFileW(
                PCWSTR(target_w.as_ptr()),
                PCWSTR(source_w.as_ptr()),
                PCWSTR(backup_w.as_ptr()),
                REPLACE_FILE_FLAGS(0),
                None,
                None,
            )
        } else {
            MoveFileExW(
                PCWSTR(source_w.as_ptr()),
                PCWSTR(target_w.as_ptr()),
                MOVEFILE_WRITE_THROUGH,
            )
        }
    };
    if let Err(error) = result {
        let original = io_error(
            "atomically replace executable",
            target,
            io::Error::from_raw_os_error(error.code().0 & 0xffff),
        );
        // ReplaceFile may report a partial failure after moving the original to the backup.
        // Reconcile that documented state before returning, without reading quarantine.
        let recovery: Result<()> = (|| {
            if backup.try_exists()? {
                let restored = unsafe {
                    if target.try_exists()? {
                        ReplaceFileW(
                            PCWSTR(target_w.as_ptr()),
                            PCWSTR(backup_w.as_ptr()),
                            PCWSTR::null(),
                            REPLACE_FILE_FLAGS(0),
                            None,
                            None,
                        )
                    } else {
                        MoveFileExW(
                            PCWSTR(backup_w.as_ptr()),
                            PCWSTR(target_w.as_ptr()),
                            MOVEFILE_WRITE_THROUGH,
                        )
                    }
                };
                restored.map_err(|restore| {
                    io_error(
                        "recover interrupted atomic replacement",
                        target,
                        io::Error::from_raw_os_error(restore.code().0 & 0xffff),
                    )
                })?;
            } else if exists && !target.try_exists()? {
                bail!(
                    "original executable disappeared with no transaction backup; inspect Protection history, never restore quarantine"
                );
            }
            Ok(())
        })();
        if let Err(recovery) = recovery {
            return Err(anyhow::Error::new(IncompleteRecovery(recovery)).context(original));
        }
        return Err(original);
    }
    Ok(())
}

pub fn io_error(operation: &str, path: &Path, error: io::Error) -> anyhow::Error {
    let remedy = match error.raw_os_error() {
        Some(225 | 226) => {
            "Windows antivirus blocked or quarantined this file. Open Windows Security > Protection history and record the detection, file path and release hash; report them to the project/vendor. Do not disable protection, add an exclusion, or restore quarantined bytes"
        }
        Some(32 | 33) => {
            "The executable is locked by another process. Exit only the tray using this installed path after pending camera operations finish, then retry; elevation cannot release a mapped executable"
        }
        Some(5) => {
            "Windows denied access. This does not establish a file lock: inspect this path's ACL and Windows Security > Protection history for a block/quarantine; do not retry with sudo or bypass protection"
        }
        Some(2 | 3) => {
            "The file/path disappeared. Check Windows Security > Protection history for quarantine and confirm the installation path; do not restore quarantined bytes"
        }
        _ => {
            "Keep the previous installation and investigate the reported Windows error before retrying"
        }
    };
    anyhow::anyhow!("{operation} {} failed: {error}; {remedy}", path.display())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Fake {
        fail_install: Option<usize>,
        fail_restart: bool,
        fail_stop: bool,
        fail_rollback: bool,
        incomplete_install: bool,
        guard_state: Option<&'static str>,
        reserved: bool,
        resume_on_failed_restart: bool,
        bytes: [u8; 2],
        running: bool,
        launched: Vec<[u8; 2]>,
    }
    impl Operations for Fake {
        fn reserve(&mut self) -> Result<()> {
            if let Some(state) = self.guard_state {
                bail!("{state}");
            }
            self.reserved = true;
            Ok(())
        }
        fn release(&mut self) {
            self.reserved = false;
        }
        fn stop(&mut self) -> Result<()> {
            if self.fail_stop {
                bail!("busy camera");
            }
            self.running = false;
            Ok(())
        }
        fn install(&mut self, i: usize) -> Result<()> {
            assert!(
                self.reserved,
                "installed bytes may change only while reserved"
            );
            if self.fail_install == Some(i) {
                if self.incomplete_install {
                    self.bytes[i] = 2;
                    return Err(anyhow::Error::new(IncompleteRecovery(anyhow::anyhow!(
                        "original missing"
                    ))));
                }
                bail!("blocked");
            }
            self.bytes[i] = 1;
            Ok(())
        }
        fn rollback(&mut self, i: usize) -> Result<()> {
            assert!(
                self.reserved,
                "rollback bytes may change only while reserved"
            );
            if self.fail_rollback {
                bail!("backup missing");
            }
            self.bytes[i] = 0;
            Ok(())
        }
        fn restart(&mut self) -> Result<()> {
            assert!(
                !self.reserved,
                "restart must be able to resume requested controls"
            );
            if std::mem::take(&mut self.fail_restart) {
                if self.resume_on_failed_restart {
                    self.guard_state = Some("helper resumed during failed restart");
                }
                bail!("antivirus");
            }
            self.running = true;
            self.launched.push(self.bytes);
            Ok(())
        }
    }
    #[test]
    fn replacing_update_refusal_keeps_protection_tray_and_binaries() {
        for state in [
            "active owner",
            "manual intent requested",
            "ownership unknown",
        ] {
            let mut fake = Fake {
                guard_state: Some(state),
                running: true,
                ..Default::default()
            };
            assert!(apply(&mut fake, 2, true).is_err());
            assert!(fake.running);
            assert_eq!(fake.bytes, [0, 0]);
            assert_eq!(fake.guard_state, Some(state));
            assert!(fake.launched.is_empty());
        }
    }
    #[test]
    fn no_replacement_leaves_guarded_tray_running() {
        let mut fake = Fake {
            guard_state: Some("active owner"),
            running: true,
            ..Default::default()
        };
        apply(&mut fake, 0, true).unwrap();
        assert!(fake.running);
        assert_eq!(fake.bytes, [0, 0]);
        assert_eq!(fake.guard_state, Some("active owner"));
    }
    #[test]
    fn resumed_protection_prevents_rollback_after_failed_restart() {
        let mut fake = Fake {
            fail_restart: true,
            resume_on_failed_restart: true,
            running: true,
            ..Default::default()
        };
        assert!(apply(&mut fake, 2, true).is_err());
        assert_eq!(fake.bytes, [1, 1]);
        assert_eq!(
            fake.guard_state,
            Some("helper resumed during failed restart")
        );
        assert!(fake.launched.is_empty());
    }

    #[test]
    #[ignore = "requires free shared native protection ownership; exercised on disposable Windows CI"]
    fn native_dormant_or_unknown_protection_preserves_installed_files() -> Result<()> {
        if std::env::var_os("MCW_TEST_UPDATE_REFUSAL").is_none() {
            let isolated = tempfile::tempdir()?;
            let output = std::process::Command::new(std::env::current_exe()?)
                .args([
                    "--exact",
                    "--ignored",
                    "updater::transaction::tests::native_dormant_or_unknown_protection_preserves_installed_files",
                    "--test-threads=1",
                    "--nocapture",
                ])
                .env("MCW_TEST_UPDATE_REFUSAL", "1")
                .env("LOCALAPPDATA", isolated.path())
                .env("APPDATA", isolated.path())
                .output()?;
            ensure!(
                output.status.success(),
                "isolated update refusal failed: {}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return Ok(());
        }
        let data = crate::settings::data_dir()?;
        std::fs::create_dir_all(&data)?;
        let identity = crate::windows_control::current_identity()?;
        let intent_directory = data.join("microphone-protection").join(format!(
            "{}-{}-{}",
            identity.user, identity.session, identity.scope
        ));
        std::fs::create_dir_all(&intent_directory)?;
        let intent = intent_directory.join("intent.json");
        let camera = data.join("blocked-camera-devices.json");
        let target = data.join("installed");
        let staged = data.join("staged");
        let backup = data.join("backup");
        std::fs::write(&target, b"old")?;
        std::fs::write(&staged, b"new")?;
        let inactive = serde_json::json!({
            "format": 1, "generation": 0, "requested": false,
            "release_pending": false, "next_lock": 0, "automatic_token": 0,
            "automatic_generation": 0, "automatic_restore_pending": false
        });
        let cases = [
            ("requested", serde_json::json!(true)),
            ("release_pending", serde_json::json!(true)),
            ("automatic_token", serde_json::json!(1)),
            ("automatic_restore_pending", serde_json::json!(true)),
            ("format", serde_json::json!(2)),
        ];
        for (field, value) in cases {
            let mut record = inactive.clone();
            record[field] = value;
            let bytes = serde_json::to_vec(&record)?;
            std::fs::write(&intent, &bytes)?;
            let mut ops = NativeOperations::new(
                vec![Replacement {
                    staged: staged.clone(),
                    target: target.clone(),
                    backup: backup.clone(),
                    had_original: true,
                    running_cli: false,
                }],
                None,
            );
            apply(&mut ops, 1, false).unwrap_err();
            assert_eq!(std::fs::read(&target)?, b"old");
            assert_eq!(std::fs::read(&staged)?, b"new");
            assert_eq!(std::fs::read(&intent)?, bytes);
            assert!(!backup.try_exists()?);
        }
        std::fs::write(&intent, serde_json::to_vec(&inactive)?)?;
        for bytes in [
            br#"{"blocked":[],"restore_on_arrival":[],"desired_blocked":true,"intent_generation":1}"#.as_slice(),
            b"corrupt camera operational record".as_slice(),
        ] {
            std::fs::write(&camera, bytes)?;
            let mut ops = NativeOperations::new(
                vec![Replacement {
                    staged: staged.clone(),
                    target: target.clone(),
                    backup: backup.clone(),
                    had_original: true,
                    running_cli: false,
                }],
                None,
            );
            apply(&mut ops, 1, false).unwrap_err();
            assert_eq!(std::fs::read(&target)?, b"old");
            assert_eq!(std::fs::read(&staged)?, b"new");
            assert_eq!(std::fs::read(&camera)?, bytes);
            assert!(!backup.try_exists()?);
        }
        // Unsigned legacy restoration inventory must not become active-owner authority.
        let legacy = br#"{"blocked":[],"restore_on_arrival":["USB\\synthetic-camera-instance"]}"#;
        std::fs::write(&camera, legacy)?;
        let mut ops = NativeOperations::new(
            vec![Replacement {
                staged: staged.clone(),
                target: target.clone(),
                backup: backup.clone(),
                had_original: true,
                running_cli: false,
            }],
            None,
        );
        apply(&mut ops, 1, false)?;
        assert_eq!(std::fs::read(&target)?, b"new");
        assert_eq!(std::fs::read(&backup)?, b"old");
        assert_eq!(std::fs::read(&camera)?, legacy);
        Ok(())
    }

    #[test]
    #[ignore = "real old/candidate tray transaction proof; disposable GitHub-hosted Windows only"]
    fn native_active_tray_replacement_preserves_inactive_state() -> Result<()> {
        use std::{
            collections::BTreeMap,
            fs,
            os::windows::{
                ffi::OsStringExt,
                fs::OpenOptionsExt,
                io::{AsRawHandle, FromRawHandle, OwnedHandle},
                process::CommandExt,
            },
            process::{Child, Command, ExitStatus, Stdio},
            sync::mpsc,
            thread,
            time::{Duration, Instant},
        };
        use windows::{
            Win32::{
                Foundation::{HANDLE, LPARAM, WAIT_OBJECT_0, WAIT_TIMEOUT, WPARAM},
                System::Threading::{
                    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
                    PROCESS_SYNCHRONIZE, PROCESS_TERMINATE, QueryFullProcessImageNameW,
                    TerminateProcess, WaitForSingleObject,
                },
                UI::WindowsAndMessaging::{
                    FindWindowW, GetWindowThreadProcessId, SMTO_ABORTIFHUNG, SMTO_BLOCK,
                    SendMessageTimeoutW, WM_NULL,
                },
            },
            core::PWSTR,
        };

        // This gate precedes even version probes, settings access and native identity lookup.
        ensure!(
            std::env::var("GITHUB_ACTIONS").as_deref() == Ok("true")
                && std::env::var("RUNNER_ENVIRONMENT").as_deref() == Ok("github-hosted"),
            "active tray replacement proof requires a disposable GitHub-hosted Windows runner"
        );
        fn absolute_env(name: &str) -> Result<PathBuf> {
            let path =
                PathBuf::from(std::env::var_os(name).with_context(|| format!("{name} required"))?);
            ensure!(path.is_absolute(), "{name} must be absolute");
            ensure!(
                !path.components().any(|component| matches!(
                    component,
                    std::path::Component::ParentDir | std::path::Component::CurDir
                )),
                "{name} must not contain relative components"
            );
            Ok(path)
        }
        let previous = fs::canonicalize(absolute_env("MCW_TEST_UPDATE_PREVIOUS")?)?;
        let candidate = fs::canonicalize(absolute_env("MCW_TEST_UPDATE_CANDIDATE")?)?;
        let requested_root = absolute_env("MCW_TEST_UPDATE_PROOF")?;
        let appdata = absolute_env("APPDATA")?;
        let localappdata = absolute_env("LOCALAPPDATA")?;
        ensure!(
            appdata == requested_root.join("appdata")
                && localappdata == requested_root.join("localappdata"),
            "APPDATA and LOCALAPPDATA must be the private proof-root children"
        );
        let parent = fs::canonicalize(
            requested_root
                .parent()
                .context("proof root has no parent")?,
        )?;
        let root = parent.join(
            requested_root
                .file_name()
                .context("proof root has no name")?,
        );
        ensure!(
            previous != candidate
                && !previous.starts_with(&root)
                && !root.starts_with(&previous)
                && !candidate.starts_with(&root)
                && !root.starts_with(&candidate),
            "proof root and genuine artifact directories must be disjoint"
        );
        if requested_root.try_exists()? {
            ensure!(
                fs::canonicalize(&requested_root)? == root
                    && fs::read_dir(&requested_root)?.next().is_none(),
                "proof root must be an empty, non-redirected private directory"
            );
        } else {
            fs::create_dir(&requested_root)?;
        }
        ensure!(
            fs::canonicalize(&requested_root)? == root,
            "redirected proof root"
        );
        fs::create_dir(&appdata)?;
        fs::create_dir(&localappdata)?;
        ensure!(
            fs::canonicalize(&appdata)? == root.join("appdata")
                && fs::canonicalize(&localappdata)? == root.join("localappdata"),
            "private application roots must not escape the proof root"
        );
        for directory in [&previous, &candidate] {
            for name in ["mcw.exe", "mcw-tray.exe"] {
                let image = directory.join(name);
                ensure!(
                    fs::canonicalize(&image)? == image && image.is_file(),
                    "input image must be an exact regular file: {}",
                    image.display()
                );
                super::super::validate_pe(&image)?;
            }
        }

        fn wait_child(child: &mut Child, seconds: u64) -> Result<ExitStatus> {
            let deadline = Instant::now() + Duration::from_secs(seconds);
            loop {
                if let Some(status) = child.try_wait()? {
                    return Ok(status);
                }
                ensure!(
                    Instant::now() < deadline,
                    "owned child {} did not exit",
                    child.id()
                );
                thread::sleep(Duration::from_millis(25));
            }
        }
        fn version(image: &Path, root: &Path, label: &str) -> Result<String> {
            let stdout = root.join(format!("{label}-version.stdout"));
            let stderr = root.join(format!("{label}-version.stderr"));
            let mut child = Command::new(image)
                .arg("--version")
                .stdin(Stdio::null())
                .stdout(fs::File::create(&stdout)?)
                .stderr(fs::File::create(&stderr)?)
                .creation_flags(0x0800_0000)
                .spawn()?;
            let status = match wait_child(&mut child, 10) {
                Ok(status) => status,
                Err(error) => {
                    child
                        .kill()
                        .context("terminate only the timed-out owned version child")?;
                    wait_child(&mut child, 5)?;
                    return Err(error);
                }
            };
            ensure!(
                status.success(),
                "{label} --version failed: {}",
                fs::read_to_string(stderr)?
            );
            let output = fs::read_to_string(stdout)?;
            let value = output
                .trim()
                .strip_prefix("mcw ")
                .context("unexpected genuine CLI --version output")?;
            ensure!(
                !value.contains(char::is_whitespace),
                "invalid CLI version output"
            );
            Ok(value.to_owned())
        }
        fn snapshot(root: &Path) -> Result<BTreeMap<PathBuf, Vec<u8>>> {
            fn visit(
                root: &Path,
                path: &Path,
                files: &mut BTreeMap<PathBuf, Vec<u8>>,
            ) -> Result<()> {
                for entry in fs::read_dir(path)? {
                    let entry = entry?;
                    let kind = entry.file_type()?;
                    ensure!(
                        !kind.is_symlink(),
                        "private state contains a redirected entry"
                    );
                    if kind.is_dir() {
                        visit(root, &entry.path(), files)?;
                    } else {
                        ensure!(kind.is_file(), "private state contains a non-file");
                        files.insert(
                            entry.path().strip_prefix(root)?.to_owned(),
                            fs::read(entry.path())?,
                        );
                    }
                }
                Ok(())
            }
            let mut files = BTreeMap::new();
            for name in ["appdata", "localappdata"] {
                visit(root, &root.join(name), &mut files)?;
            }
            Ok(files)
        }

        struct ExactTray {
            running: super::super::process::RunningTray,
            process: OwnedHandle,
            pid: u32,
        }
        impl ExactTray {
            fn handle(&self) -> HANDLE {
                HANDLE(self.process.as_raw_handle())
            }
            fn alive(&self) -> Result<bool> {
                match unsafe { WaitForSingleObject(self.handle(), 0) } {
                    WAIT_TIMEOUT => Ok(true),
                    WAIT_OBJECT_0 => Ok(false),
                    _ => bail!("cannot inspect retained exact tray process"),
                }
            }
            fn acquire(target: &Path, expected_pid: Option<u32>) -> Result<Option<Self>> {
                let Some(running) = super::super::process::owned_running_tray(target)? else {
                    return Ok(None);
                };
                let class = wide(Path::new("MicCamWatchTrayClass"));
                let window = unsafe { FindWindowW(PCWSTR(class.as_ptr()), PCWSTR::null()) }?;
                let mut pid = 0;
                unsafe { GetWindowThreadProcessId(window, Some(&mut pid)) };
                ensure!(
                    pid != 0 && expected_pid.is_none_or(|expected| pid == expected),
                    "tray is not the fixture-owned launched child"
                );
                let handle = unsafe {
                    OpenProcess(
                        PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE | PROCESS_TERMINATE,
                        false,
                        pid,
                    )
                }?;
                let process = unsafe { OwnedHandle::from_raw_handle(handle.0) };
                let mut image = [0u16; 32768];
                let mut length = image.len() as u32;
                unsafe {
                    QueryFullProcessImageNameW(
                        handle,
                        PROCESS_NAME_WIN32,
                        PWSTR(image.as_mut_ptr()),
                        &mut length,
                    )
                }?;
                let image = PathBuf::from(std::ffi::OsString::from_wide(&image[..length as usize]));
                ensure!(
                    fs::canonicalize(image)? == fs::canonicalize(target)?,
                    "retained tray handle does not own the exact fixture image"
                );
                let mut current_pid = 0;
                unsafe { GetWindowThreadProcessId(window, Some(&mut current_pid)) };
                ensure!(
                    current_pid == pid,
                    "tray changed during exact-handle acquisition"
                );
                let tray = Self {
                    running,
                    process,
                    pid,
                };
                ensure!(tray.alive()?, "tray exited during handle acquisition");
                Ok(Some(tray))
            }
            fn ready(&self) -> Result<()> {
                // A console-subsystem CLI host can own a responsive GUI without
                // supporting process-wide WaitForInputIdle. Authenticate the HWND
                // and require its message loop to answer without a mutating probe.
                let class = wide(Path::new("MicCamWatchTrayClass"));
                let window = unsafe { FindWindowW(PCWSTR(class.as_ptr()), PCWSTR::null()) }?;
                let mut pid = 0;
                unsafe { GetWindowThreadProcessId(window, Some(&mut pid)) };
                ensure!(
                    pid == self.pid && self.alive()?,
                    "ready tray ownership changed"
                );
                let mut reply = 0;
                ensure!(
                    unsafe {
                        SendMessageTimeoutW(
                            window,
                            WM_NULL,
                            WPARAM(0),
                            LPARAM(0),
                            SMTO_ABORTIFHUNG | SMTO_BLOCK,
                            5000,
                            Some(&mut reply),
                        )
                    }
                    .0 != 0,
                    "exact tray message loop is not responsive"
                );
                Ok(())
            }
        }
        struct Fixture {
            target: PathBuf,
            child: Option<Child>,
            tray: Option<ExactTray>,
            restart_possible: bool,
            cleaned: bool,
        }
        impl Fixture {
            fn await_tray(&mut self, expected_pid: Option<u32>) -> Result<()> {
                let deadline = Instant::now() + Duration::from_secs(30);
                loop {
                    if let Some(tray) = ExactTray::acquire(&self.target, expected_pid)? {
                        // Retain ownership before the next fallible readiness check.
                        self.tray = Some(tray);
                        break;
                    }
                    if let (Some(child), Some(_)) = (&mut self.child, expected_pid) {
                        ensure!(
                            child.try_wait()?.is_none(),
                            "old tray exited before readiness"
                        );
                    }
                    ensure!(Instant::now() < deadline, "exact-image tray did not appear");
                    thread::sleep(Duration::from_millis(25));
                }
                let tray = self.tray.as_ref().context("tray handle missing")?;
                tray.ready()?;
                // Observe a real running loop, rather than racing window creation and
                // stopping before startup workers have had a chance to take ownership.
                let deadline = Instant::now() + Duration::from_secs(2);
                while Instant::now() < deadline {
                    tray.ready()?;
                    thread::sleep(Duration::from_millis(100));
                }
                Ok(())
            }
            fn cleanup(&mut self) -> Result<()> {
                let mut errors = Vec::new();
                // A restart can succeed before a later assertion fails. Adopt only an
                // exact-image successor, never a PID/name-selected foreign process.
                if self
                    .tray
                    .as_ref()
                    .is_none_or(|tray| matches!(tray.alive(), Ok(false)))
                {
                    let deadline = Instant::now() + Duration::from_secs(30);
                    loop {
                        match ExactTray::acquire(&self.target, None) {
                            Ok(Some(tray)) => {
                                self.tray = Some(tray);
                                break;
                            }
                            Ok(None) if self.restart_possible && Instant::now() < deadline => {
                                thread::sleep(Duration::from_millis(25));
                            }
                            Ok(None) => {
                                if self.restart_possible {
                                    errors.push("possible successor never exposed exact owned cleanup handles before deadline".to_owned());
                                }
                                break;
                            }
                            Err(error) => {
                                errors.push(format!("locate owned cleanup tray: {error:#}"));
                                break;
                            }
                        }
                    }
                }
                if let Some(tray) = &self.tray {
                    match tray.alive() {
                        Ok(true) => {
                            if let Err(error) = tray.ready().and_then(|()| tray.running.stop()) {
                                errors.push(format!("cooperative cleanup: {error:#}"));
                                // Last-resort cleanup is a failure, never positive proof.
                                // This retained, exact-image handle is exclusively ours.
                                match unsafe { TerminateProcess(tray.handle(), 1) } {
                                    Ok(()) => {
                                        if unsafe { WaitForSingleObject(tray.handle(), 5000) }
                                            != WAIT_OBJECT_0
                                        {
                                            errors.push(
                                                "owned tray termination timed out".to_owned(),
                                            );
                                        }
                                    }
                                    Err(error) => {
                                        errors.push(format!("owned tray cleanup failed: {error}"))
                                    }
                                }
                            }
                        }
                        Ok(false) => {}
                        Err(error) => errors.push(format!("inspect cleanup tray: {error:#}")),
                    }
                }
                if let Some(child) = &mut self.child {
                    match wait_child(child, 5) {
                        Ok(_) => {}
                        Err(error) => {
                            errors.push(format!("owned child cleanup: {error:#}"));
                            match child.kill() {
                                Ok(()) => {
                                    if let Err(error) = wait_child(child, 5) {
                                        errors.push(format!("reap owned child: {error:#}"));
                                    }
                                }
                                Err(error) => {
                                    errors.push(format!("terminate owned child: {error}"))
                                }
                            }
                        }
                    }
                }
                self.cleaned = true;
                ensure!(errors.is_empty(), "{}", errors.join("; "));
                Ok(())
            }
        }
        impl Drop for Fixture {
            fn drop(&mut self) {
                if !self.cleaned
                    && let Err(error) = self.cleanup()
                {
                    eprintln!("active transaction fixture cleanup failed: {error:#}");
                }
            }
        }
        struct HeldLease {
            release: Option<mpsc::Sender<()>>,
            worker: Option<thread::JoinHandle<Result<()>>>,
        }
        impl HeldLease {
            fn acquire() -> Result<Self> {
                let (ready_send, ready_receive) = mpsc::sync_channel(1);
                let (release, released) = mpsc::channel();
                let worker = thread::spawn(move || -> Result<()> {
                    let acquired = (|| {
                        let identity = crate::windows_control::current_identity()?;
                        crate::windows_control::acquire_resource_for("microphone", &identity)
                    })();
                    match acquired {
                        Ok(lease) => {
                            ready_send
                                .send(Ok(()))
                                .context("lease readiness receiver closed")?;
                            let result = released
                                .recv_timeout(Duration::from_secs(60))
                                .context("fixture lease release deadline expired");
                            drop(lease); // Windows mutex release must run on its owning thread.
                            result?;
                        }
                        Err(error) => {
                            ready_send
                                .send(Err(format!("{error:#}")))
                                .context("lease failure receiver closed")?;
                        }
                    }
                    Ok(())
                });
                let mut held = Self {
                    release: Some(release),
                    worker: Some(worker),
                };
                let ready = ready_receive
                    .recv_timeout(Duration::from_secs(15))
                    .context("native fixture lease did not report readiness");
                match ready {
                    Ok(Ok(())) => Ok(held),
                    error => {
                        let cleanup = held.release();
                        bail!(
                            "cannot hold fixture-only resource lease: {error:?}; cleanup: {cleanup:?}"
                        );
                    }
                }
            }
            fn release(&mut self) -> Result<()> {
                if let Some(release) = self.release.take() {
                    // A disconnected receiver means the worker already released/failed;
                    // joining below reports that outcome, rather than ignoring it.
                    let sent = release.send(());
                    if let Some(worker) = self.worker.take() {
                        worker
                            .join()
                            .map_err(|_| anyhow::anyhow!("fixture lease thread panicked"))??;
                    }
                    sent.context("fixture lease worker exited before release")?;
                }
                Ok(())
            }
        }
        impl Drop for HeldLease {
            fn drop(&mut self) {
                if let Err(error) = self.release() {
                    eprintln!("fixture-only native lease cleanup failed: {error:#}");
                }
            }
        }

        let mut proof = serde_json::json!({
            "format": 1,
            "kind": "native-active-tray-transaction",
            "public_candidate_acquisition_proven": false,
            "previous": previous, "candidate": candidate,
            "readiness": "exact owned_running_tray, authenticated responsive WM_NULL, two-second live observation",
            "lease_fixture": "real global microphone resource lease on a different owned thread; not a genuine protection helper",
            "limits": [
                "No candidate public release acquisition or mcw update routing is exercised",
                "The test driver is not the mapped installed CLI",
                "UI-loop readiness does not assert asynchronous collector completion",
                "No active protection, restoration or UAC request is created or released"
            ],
            "layouts": []
        });
        let result = (|| -> Result<()> {
            let old_version = version(&previous.join("mcw.exe"), &root, "previous")?;
            let candidate_version = version(&candidate.join("mcw.exe"), &root, "candidate")?;
            proof["previous_version"] = serde_json::json!(old_version);
            proof["candidate_version"] = serde_json::json!(candidate_version);
            ensure!(
                old_version != candidate_version,
                "old and candidate CLI versions must genuinely differ"
            );
            ensure!(
                candidate_version == env!("CARGO_PKG_VERSION"),
                "candidate is not this actual source version"
            );
            ensure!(
                crate::autostart::tray_matches_version(
                    &previous.join("mcw-tray.exe"),
                    &old_version
                )? && crate::autostart::tray_matches_version(
                    &candidate.join("mcw-tray.exe"),
                    &candidate_version
                )?,
                "genuine input companions do not match their CLI versions"
            );
            crate::settings::Settings::update(|settings| {
                settings.notifications_enabled = false;
                settings.sound_enabled = false;
                settings.show_ready = false;
                settings.history_enabled = false;
                settings.mute_on_lock = false;
                settings.block_camera_on_lock = false;
                settings.restore_on_unlock = false;
            })?;
            let data = crate::settings::data_dir()?;
            fs::create_dir_all(&data)?;
            let identity = crate::windows_control::current_identity()?;
            let intent = data.join("microphone-protection").join(format!(
                "{}-{}-{}",
                identity.user, identity.session, identity.scope
            ));
            fs::create_dir_all(&intent)?;
            fs::write(
                intent.join("intent.json"),
                serde_json::to_vec(&serde_json::json!({
                    "format": 1, "generation": 0, "requested": false, "release_pending": false,
                    "next_lock": 0, "automatic_token": 0, "automatic_generation": 0,
                    "automatic_restore_pending": false
                }))?,
            )?;
            fs::write(intent.join("intent.lock"), b"")?;
            fs::write(data.join("blocked-camera-devices.json"), br#"{"blocked":[],"restore_on_arrival":[],"desired_blocked":false,"intent_generation":0}"#)?;
            let initial_state = snapshot(&root)?;
            let mut layout_errors = Vec::new();
            for (label, cli_source, cli_version, cli_host, missing_companion) in [
                (
                    "old-pair-to-candidate",
                    &previous,
                    &old_version,
                    false,
                    false,
                ),
                (
                    "candidate-cli-old-tray-repair",
                    &candidate,
                    &candidate_version,
                    false,
                    false,
                ),
                (
                    "old-cli-host-to-candidate",
                    &previous,
                    &old_version,
                    true,
                    false,
                ),
                (
                    "candidate-cli-host-old-tray-repair",
                    &candidate,
                    &candidate_version,
                    true,
                    false,
                ),
                (
                    "candidate-cli-host-missing-tray-repair",
                    &candidate,
                    &candidate_version,
                    true,
                    true,
                ),
            ] {
                let layout = root.join(label);
                let installed = layout.join("installed");
                let stage = layout.join("stage");
                fs::create_dir_all(&installed)?;
                fs::create_dir(&stage)?;
                let launch_directory = layout.join("launch cwd");
                fs::create_dir_all(launch_directory.join("policy files"))?;
                let relative_policy = Path::new("policy files").join("custom 日本.toml");
                let policy_bytes = b"profile = \"strict\"\nlanguage = \"de\"\ntrust_policy = \"offline\"\naction = \"alert\"\n";
                fs::write(launch_directory.join(&relative_policy), policy_bytes)?;
                ensure!(
                    fs::canonicalize(&launch_directory)?
                        != fs::canonicalize(std::env::current_dir()?)?,
                    "launch and updater working directories must differ"
                );
                fs::copy(cli_source.join("mcw.exe"), installed.join("mcw.exe"))?;
                let companion = installed.join("mcw-tray.exe");
                if !missing_companion {
                    fs::copy(previous.join("mcw-tray.exe"), &companion)?;
                }
                for name in ["mcw.exe", "mcw-tray.exe"] {
                    fs::copy(candidate.join(name), stage.join(name))?;
                }
                let target = if cli_host {
                    installed.join("mcw.exe")
                } else {
                    companion.clone()
                };
                let repair = cli_version == &candidate_version;
                let mut fixture = Fixture {
                    target: target.clone(),
                    child: None,
                    tray: None,
                    restart_possible: false,
                    cleaned: false,
                };
                let mut evidence = serde_json::json!({
                    "layout": label, "installed": installed,
                    "initial_cli_version": cli_version,
                    "initial_tray_version": if missing_companion { None } else { Some(&old_version) },
                    "frontend_host": if cli_host { "cli" } else { "companion" },
                    "launch_directory": launch_directory, "updater_directory": std::env::current_dir()?,
                    "target_version": candidate_version, "result": "failed"
                });
                let attempt = (|| -> Result<()> {
                    ensure!(
                        snapshot(&root)? == initial_state,
                        "private state changed before fresh layout"
                    );
                    let mut command = Command::new(&target);
                    if cli_host {
                        command
                            .arg("--config")
                            .arg(&relative_policy)
                            .args(["--lang", "fr", "tray"]);
                    }
                    let child = command
                        .current_dir(&launch_directory)
                        .stdin(Stdio::null())
                        .stdout(fs::File::create(layout.join("old-tray.stdout"))?)
                        .stderr(fs::File::create(layout.join("old-tray.stderr"))?)
                        .creation_flags(0x0800_0000)
                        .spawn()?;
                    let pid = child.id();
                    fixture.child = Some(child);
                    fixture.await_tray(Some(pid))?;
                    evidence["old_pid"] = serde_json::json!(pid);
                    evidence["old_ready"] = serde_json::json!(true);
                    ensure!(
                        snapshot(&root)? == initial_state,
                        "old tray changed private durable state"
                    );
                    ensure!(
                        super::super::process::installed_running_tray(
                            &stage.join("mcw.exe"),
                            &stage.join("mcw-tray.exe"),
                        )
                        .is_err(),
                        "another installation adopted this fixture's tray owner"
                    );
                    ensure!(
                        fixture.tray.as_ref().context("old tray missing")?.alive()?,
                        "foreign installation inspection stopped the owner"
                    );
                    evidence["foreign_installation_refused_without_stop"] = serde_json::json!(true);
                    let running = super::super::process::installed_running_tray(
                        &installed.join("mcw.exe"),
                        &companion,
                    )?
                    .context("ready frontend was not accepted as an installed tray host")?;
                    let original_context = running.launch_context(&target)?;
                    let mut ops = NativeOperations::new(
                        ["mcw-tray.exe", "mcw.exe"]
                            .into_iter()
                            .filter(|name| !repair || *name != "mcw.exe")
                            .map(|name| Replacement {
                                staged: stage.join(name),
                                target: installed.join(name),
                                backup: stage.join(format!("previous-{name}")),
                                had_original: name != "mcw-tray.exe" || !missing_companion,
                                running_cli: false,
                            })
                            .collect(),
                        Some(running),
                    );
                    let count = ops.replacements.len();
                    let original_cli = fs::read(installed.join("mcw.exe"))?;
                    let original_tray = if missing_companion {
                        None
                    } else {
                        Some(fs::read(&companion)?)
                    };
                    let mut held = HeldLease::acquire()?;
                    ensure!(
                        fixture
                            .tray
                            .as_ref()
                            .context("old tray handle missing")?
                            .alive()?,
                        "old tray died before refusal attempt"
                    );
                    fixture.restart_possible = true;
                    let refusal = match apply(&mut ops, count, true) {
                        Err(error) => error,
                        Ok(()) => {
                            bail!("real other-thread resource lease did not refuse replacement")
                        }
                    };
                    evidence["lease_refusal"] = serde_json::json!(format!("{refusal:#}"));
                    ensure!(
                        fixture
                            .child
                            .as_mut()
                            .context("old child missing")?
                            .try_wait()?
                            .is_none(),
                        "reservation refusal stopped the old child"
                    );
                    ensure!(
                        fixture.tray.as_ref().context("old tray missing")?.alive()?,
                        "reservation refusal killed the retained old tray"
                    );
                    ensure!(
                        super::super::process::installed_running_tray(
                            &installed.join("mcw.exe"),
                            &companion
                        )?
                        .is_some(),
                        "refusal lost exact tray ownership"
                    );
                    ensure!(
                        fs::read(installed.join("mcw.exe"))? == original_cli
                            && if missing_companion {
                                !companion.try_exists()?
                            } else {
                                Some(fs::read(&companion)?) == original_tray
                            },
                        "refusal changed installed bytes"
                    );
                    for replacement in &ops.replacements {
                        ensure!(
                            !replacement.backup.try_exists()?,
                            "refusal created a transaction backup"
                        );
                        ensure!(
                            fs::read(&replacement.staged)?
                                == fs::read(
                                    candidate.join(
                                        replacement
                                            .target
                                            .file_name()
                                            .context("missing target name")?
                                    )
                                )?,
                            "refusal changed staged bytes"
                        );
                    }
                    ensure!(
                        snapshot(&root)? == initial_state,
                        "refusal changed private durable state"
                    );
                    evidence["refusal_preserved_files_tray_state"] = serde_json::json!(true);
                    held.release()?;
                    if cli_host && !repair {
                        // Deny deletion of only our genuine staged CLI. The tray
                        // swap must roll back, then restart the original CLI host
                        // with its original launch context before a later apply.
                        let staged_cli = fs::OpenOptions::new()
                            .read(true)
                            .share_mode(1)
                            .open(stage.join("mcw.exe"))?;
                        let rollback = apply(&mut ops, count, true);
                        drop(staged_cli);
                        ensure!(
                            rollback.is_err(),
                            "locked staged CLI did not force rollback"
                        );
                        ensure!(
                            wait_child(fixture.child.as_mut().context("old child missing")?, 5)?
                                .success()
                                && !fixture.tray.as_ref().context("old tray missing")?.alive()?,
                            "rollback did not retire the exact old frontend cooperatively"
                        );
                        ensure!(
                            fs::read(installed.join("mcw.exe"))? == original_cli
                                && Some(fs::read(&companion)?) == original_tray
                                && snapshot(&root)? == initial_state,
                            "native rollback did not preserve the original pair and preferences"
                        );
                        fixture.tray = None;
                        fixture.await_tray(None)?;
                        let restored = fixture
                            .tray
                            .as_ref()
                            .context("rollback successor missing")?;
                        ensure!(
                            restored.running.launch_context(&target)? == original_context,
                            "rollback restarted a different frontend launch context"
                        );
                        evidence["rollback_successor_pid"] = serde_json::json!(restored.pid);
                        evidence["native_rollback_preserved_pair_context_state"] =
                            serde_json::json!(true);
                        ops.running = super::super::process::installed_running_tray(
                            &installed.join("mcw.exe"),
                            &companion,
                        )?;
                        ensure!(ops.running.is_some(), "rollback successor was not retained");
                        fs::copy(candidate.join("mcw-tray.exe"), stage.join("mcw-tray.exe"))?;
                    }
                    ensure!(
                        fixture.tray.as_ref().context("old tray missing")?.alive()?,
                        "old tray must still be alive immediately before positive apply"
                    );
                    apply(&mut ops, count, true)?;
                    ensure!(
                        wait_child(fixture.child.as_mut().context("old child missing")?, 5)?
                            .success(),
                        "old tray did not exit successfully after cooperative updater stop"
                    );
                    ensure!(
                        !fixture.tray.as_ref().context("old tray missing")?.alive()?,
                        "old retained process survived replacement"
                    );
                    evidence["old_cooperatively_exited"] = serde_json::json!(true);
                    fixture.tray = None;
                    fixture.await_tray(None)?;
                    let successor = fixture.tray.as_ref().context("successor handle missing")?;
                    evidence["successor_pid"] = serde_json::json!(successor.pid);
                    evidence["successor_exact_image_ready"] = serde_json::json!(true);
                    ensure!(
                        successor.running.launch_context(&target)? == original_context,
                        "successor changed frontend arguments, working directory, or environment"
                    );
                    ensure!(
                        fs::read(launch_directory.join(&relative_policy))? == policy_bytes,
                        "update changed the explicit custom policy"
                    );
                    evidence["launch_context_preserved"] = serde_json::json!(true);
                    for name in ["mcw.exe", "mcw-tray.exe"] {
                        ensure!(
                            fs::read(installed.join(name))? == fs::read(candidate.join(name))?,
                            "installed {name} differs from genuine candidate input"
                        );
                    }
                    ensure!(
                        if repair {
                            !stage.join("previous-mcw.exe").try_exists()?
                        } else {
                            fs::read(stage.join("previous-mcw.exe"))? == original_cli
                        },
                        "CLI backup does not reflect actual replacement"
                    );
                    ensure!(
                        match &original_tray {
                            Some(bytes) => fs::read(stage.join("previous-mcw-tray.exe"))? == *bytes,
                            None => !stage.join("previous-mcw-tray.exe").try_exists()?,
                        },
                        "companion backup does not preserve the actual original installation"
                    );
                    ensure!(
                        snapshot(&root)? == initial_state,
                        "replacement changed private durable state"
                    );
                    evidence["installed_pair_byte_equal"] = serde_json::json!(true);
                    successor.running.stop()?;
                    ensure!(!successor.alive()?, "successor did not cooperatively close");
                    fixture.restart_possible = false;
                    ensure!(
                        super::super::process::owned_running_tray(&target)?.is_none(),
                        "fixture tray remained registered after cooperative close"
                    );
                    ensure!(
                        snapshot(&root)? == initial_state,
                        "successor shutdown changed private state"
                    );
                    evidence["successor_cooperatively_exited"] = serde_json::json!(true);
                    evidence["private_preferences_and_operational_state_unchanged"] =
                        serde_json::json!(true);
                    evidence["result"] = serde_json::json!("passed");
                    Ok(())
                })();
                let cleanup = fixture.cleanup();
                if let Err(error) = &attempt {
                    evidence["error"] = serde_json::json!(format!("{error:#}"));
                }
                if let Err(error) = &cleanup {
                    evidence["cleanup_error"] = serde_json::json!(format!("{error:#}"));
                    evidence["result"] = serde_json::json!("failed");
                }
                proof["layouts"]
                    .as_array_mut()
                    .context("proof layouts missing")?
                    .push(evidence);
                match (attempt, cleanup) {
                    (Ok(()), Ok(())) => {}
                    (Err(error), Ok(())) => layout_errors.push(format!("{label}: {error:#}")),
                    (Ok(()), Err(cleanup)) => return Err(cleanup),
                    (Err(error), Err(cleanup)) => bail!("{error:#}; cleanup: {cleanup:#}"),
                }
            }
            ensure!(layout_errors.is_empty(), "{}", layout_errors.join("; "));
            Ok(())
        })();
        proof["result"] = serde_json::json!(if result.is_ok() { "passed" } else { "failed" });
        if let Err(error) = &result {
            proof["error"] = serde_json::json!(format!("{error:#}"));
        }
        fs::write(
            root.join("native-active-tray-transaction.json"),
            serde_json::to_vec_pretty(&proof)?,
        )?;
        result
    }

    #[test]
    fn tray_failure_preserves_cli_and_restarts_original() {
        let mut fake = Fake {
            fail_install: Some(0),
            ..Default::default()
        };
        assert!(apply(&mut fake, 2, true).is_err());
        assert_eq!(fake.bytes, [0, 0]);
        assert!(fake.running);
        assert_eq!(fake.launched, [[0, 0]]);
    }
    #[test]
    fn cli_failure_rolls_back_tray_before_restart() {
        let mut fake = Fake {
            fail_install: Some(1),
            ..Default::default()
        };
        assert!(apply(&mut fake, 2, true).is_err());
        assert_eq!(fake.bytes, [0, 0]);
        assert!(fake.running);
        assert_eq!(fake.launched, [[0, 0]]);
    }
    #[test]
    fn restart_security_failure_rolls_back_entire_pair() {
        let mut fake = Fake {
            fail_restart: true,
            ..Default::default()
        };
        assert!(apply(&mut fake, 2, true).is_err());
        assert_eq!(fake.bytes, [0, 0]);
        assert!(fake.running);
        assert_eq!(fake.launched, [[0, 0]]);
    }
    #[test]
    fn stopped_tray_is_never_started() {
        let mut fake = Fake::default();
        apply(&mut fake, 2, false).unwrap();
        assert_eq!(fake.bytes, [1, 1]);
        assert!(!fake.running);
        assert!(fake.launched.is_empty());
    }
    #[test]
    fn pending_camera_operations_prevent_all_writes() {
        let mut fake = Fake {
            fail_stop: true,
            running: true,
            ..Default::default()
        };
        assert!(apply(&mut fake, 2, true).is_err());
        assert!(fake.running);
        assert!(fake.launched.is_empty());
        assert_eq!(fake.bytes, [0, 0]);
    }
    #[test]
    fn failed_rollback_does_not_launch_inconsistent_pair() {
        let mut fake = Fake {
            fail_install: Some(1),
            fail_rollback: true,
            ..Default::default()
        };
        assert!(apply(&mut fake, 2, true).is_err());
        assert_eq!(fake.bytes, [1, 0]);
        assert!(!fake.running);
        assert!(fake.launched.is_empty());
    }
    #[test]
    fn interrupted_install_recovery_keeps_the_tray_stopped() {
        let mut fake = Fake {
            fail_install: Some(1),
            incomplete_install: true,
            running: true,
            ..Default::default()
        };
        assert!(apply(&mut fake, 2, true).is_err());
        assert_eq!(fake.bytes, [0, 2]);
        assert!(!fake.running);
        assert!(fake.launched.is_empty());
    }
    #[test]
    fn missing_original_backup_is_not_quarantine_recovery() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let target = temp.path().join("mcw-tray.exe");
        std::fs::write(&target, b"new")?;
        let mut ops = NativeOperations {
            running: None,
            reservation: None,
            replacements: vec![Replacement {
                staged: temp.path().join("staged"),
                target: target.clone(),
                backup: temp.path().join("missing-backup"),
                had_original: true,
                running_cli: false,
            }],
        };
        assert!(
            ops.rollback(0)
                .unwrap_err()
                .to_string()
                .contains("disappeared")
        );
        assert_eq!(std::fs::read(&target)?, b"new");
        Ok(())
    }
    #[test]
    fn locked_target_keeps_original_and_staged_bytes() -> Result<()> {
        use std::os::windows::fs::OpenOptionsExt;
        let temp = tempfile::tempdir()?;
        let target = temp.path().join("target");
        let staged = temp.path().join("stage");
        let backup = temp.path().join("backup");
        std::fs::write(&target, b"old")?;
        std::fs::write(&staged, b"new")?;
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&target)?;
        assert!(swap(&staged, &target, &backup, false).is_err());
        drop(lock);
        assert_eq!(std::fs::read(&target)?, b"old");
        assert_eq!(std::fs::read(&staged)?, b"new");
        assert!(!backup.try_exists()?);
        Ok(())
    }
    #[test]
    fn atomic_swap_retains_backup_and_rolls_back() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let staged = temp.path().join("stage");
        let target = temp.path().join("target");
        let backup = temp.path().join("backup");
        std::fs::write(&staged, b"new")?;
        std::fs::write(&target, b"old")?;
        swap(&staged, &target, &backup, false)?;
        assert_eq!(std::fs::read(&target)?, b"new");
        assert_eq!(std::fs::read(&backup)?, b"old");
        swap(&backup, &target, &staged, false)?;
        assert_eq!(std::fs::read(&target)?, b"old");
        Ok(())
    }

    #[test]
    fn running_cli_replacement_and_rollback_preserve_mapped_image() -> Result<()> {
        let current = std::env::current_exe()?;
        if let Some(expected) = std::env::var_os("MCW_TEST_MAPPED_CLI") {
            assert_eq!(current, PathBuf::from(expected));
            let directory = current.parent().unwrap();
            let original = std::fs::read(&current)?;
            let staged = directory.join("staged.exe");
            let backup = directory.join("previous.exe");
            let mut replacement = original.clone();
            replacement.extend_from_slice(b"mapped CLI regression");
            std::fs::write(&staged, &replacement)?;
            swap(&staged, &current, &backup, true)?;
            assert_eq!(std::fs::read(&current)?, replacement);
            assert_eq!(std::fs::read(&backup)?, original);
            swap(&backup, &current, &staged, true)?;
            assert_eq!(std::fs::read(&current)?, original);
            assert!(swap(&directory.join("missing.exe"), &current, &backup, true).is_err());
            assert_eq!(std::fs::read(&current)?, original);
            assert!(!backup.try_exists()?);
            return Ok(());
        }
        let isolated = tempfile::tempdir()?;
        let child_image = isolated.path().join("mapped-cli-child.exe");
        std::fs::copy(&current, &child_image)?;
        let output = std::process::Command::new(&child_image)
            .args([
                "--exact",
                "updater::transaction::tests::running_cli_replacement_and_rollback_preserve_mapped_image",
                "--test-threads=1",
                "--nocapture",
            ])
            .env("MCW_TEST_MAPPED_CLI", &child_image)
            .output()?;
        assert!(
            output.status.success(),
            "mapped-image child failed: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(())
    }
}
