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
    // Reserve before stopping a frontend: tray exit never releases a background guard.
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
        let camera_request = crate::windows_control::acquire_request_lock("camera")?;
        let microphone_request = crate::windows_control::acquire_request_lock("microphone")?;
        let identity = crate::windows_control::current_identity()?;
        let microphone = crate::windows_control::acquire_resource_for("microphone", &identity)?;
        let camera = crate::windows_control::acquire_resource_for("camera", &identity)?;
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
    pub tray: PathBuf,
    reservation: Option<ProtectionReservation>,
}
impl NativeOperations {
    pub fn new(
        replacements: Vec<Replacement>,
        running: Option<super::process::RunningTray>,
        tray: PathBuf,
    ) -> Self {
        Self {
            replacements,
            running,
            tray,
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
        super::process::restart(&self.tray)
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
    fn native_dormant_or_unknown_protection_preserves_installed_files() -> Result<()> {
        if std::env::var_os("MCW_TEST_UPDATE_REFUSAL").is_none() {
            let isolated = tempfile::tempdir()?;
            let output = std::process::Command::new(std::env::current_exe()?)
                .args([
                    "--exact",
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
                target.clone(),
            );
            let error = apply(&mut ops, 1, false).unwrap_err();
            ensure!(format!("{error:#}").contains("microphone"), "{error:#}");
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
                target.clone(),
            );
            let error = apply(&mut ops, 1, false).unwrap_err();
            ensure!(format!("{error:#}").contains("camera"), "{error:#}");
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
            target.clone(),
        );
        apply(&mut ops, 1, false)?;
        assert_eq!(std::fs::read(&target)?, b"new");
        assert_eq!(std::fs::read(&backup)?, b"old");
        assert_eq!(std::fs::read(&camera)?, legacy);
        Ok(())
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
            tray: target.clone(),
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
