use super::{bundle, msi::wide};
use crate::uninstall::windows::{ensure_owned, safe_ancestors, validate_regular};
use anyhow::{Context, Result, bail, ensure};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    os::windows::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};
use windows::{
    Win32::Storage::FileSystem::{MOVEFILE_WRITE_THROUGH, MoveFileExW},
    core::PCWSTR,
};

pub(super) fn open_owned_directory(path: &Path) -> Result<File> {
    let directory = open_directory(path, false)?;
    ensure_owned(&directory)?;
    Ok(directory)
}

pub(super) fn pin_parent(path: &Path) -> Result<File> {
    open_directory(path, false)
}

fn open_directory(path: &Path, set_owner: bool) -> Result<File> {
    safe_ancestors(path)?;
    let directory = OpenOptions::new()
        .access_mode(0x0002_0080 | if set_owner { 0x0008_0000 } else { 0 })
        .share_mode(3) // Keep the directory path stable, allowing normal children.
        .custom_flags(0x0200_0000 | 0x0020_0000) // BACKUP_SEMANTICS | OPEN_REPARSE_POINT
        .open(path)?;
    let metadata = directory.metadata()?;
    ensure!(
        metadata.is_dir() && metadata.file_attributes() & 0x400 == 0,
        "unsafe maintenance directory handle"
    );
    Ok(directory)
}

pub(super) fn private_job(base: &Path) -> Result<tempfile::TempDir> {
    let job = tempfile::Builder::new()
        .prefix("update-")
        .tempdir_in(base)?;
    let directory = open_directory(job.path(), true)?;
    crate::windows_control::assign_new_maintenance_owner(&directory)?;
    Ok(job)
}

fn known_folder(folder: &windows::core::GUID) -> Result<PathBuf> {
    use windows::Win32::{System::Com::CoTaskMemFree, UI::Shell::SHGetKnownFolderPath};
    let path = unsafe { SHGetKnownFolderPath(folder, Default::default(), None) }?;
    let value = unsafe { path.to_string() };
    unsafe { CoTaskMemFree(Some(path.0.cast())) };
    Ok(PathBuf::from(value?).canonicalize()?)
}

pub(super) fn local_app_data() -> Result<PathBuf> {
    known_folder(&windows::Win32::UI::Shell::FOLDERID_LocalAppData)
}

pub(super) fn require_real_profile() -> Result<()> {
    use windows::Win32::UI::Shell::{FOLDERID_LocalAppData, FOLDERID_RoamingAppData};
    for (variable, folder) in [
        ("LOCALAPPDATA", &FOLDERID_LocalAppData),
        ("APPDATA", &FOLDERID_RoamingAppData),
    ] {
        let inherited =
            std::env::var_os(variable).context("maintenance profile directory is missing")?;
        ensure!(
            Path::new(&inherited).canonicalize()? == known_folder(folder)?,
            "maintenance requires the real Windows {variable} profile for protection revalidation"
        );
    }
    Ok(())
}

pub(super) fn transaction_root(active: &Path) -> Result<Option<PathBuf>> {
    let local = local_app_data()?;
    let parent = active.parent().context("installation parent missing")?;
    let leaf = active
        .file_name()
        .context("installation directory missing")?;
    if !parent.starts_with(&local) {
        return Ok(None);
    }
    safe_ancestors(parent)?;
    let mut name = std::ffi::OsString::from(".");
    name.push(leaf);
    name.push(".mcw-maintenance");
    Ok(Some(parent.join(name)))
}

pub(super) fn lock(base: &Path, exclusive: bool) -> Result<File> {
    crate::windows_control::create_private_update_directory(base)?;
    let _directory = open_owned_directory(base)?;
    let path = base.join("lock");
    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(true)
        .share_mode(3)
        .custom_flags(0x0020_0000);
    let file = match options
        .clone()
        .create_new(true)
        .access_mode(0xc008_0000)
        .open(&path)
    {
        Ok(file) => {
            crate::windows_control::assign_new_maintenance_owner(&file)?;
            file
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => options.open(&path)?,
        Err(error) => return Err(error.into()),
    };
    validate_regular(&file)?;
    if exclusive {
        file.try_lock()
            .context("another process is using this installation; exit MicCamWatch and retry")?;
    } else {
        file.try_lock_shared()
            .context("maintenance owns this installation")?;
    }
    Ok(file)
}

fn move_entry(from: &Path, to: &Path) -> Result<()> {
    unsafe {
        MoveFileExW(
            PCWSTR(wide(from.as_os_str()).as_ptr()),
            PCWSTR(wide(to.as_os_str()).as_ptr()),
            MOVEFILE_WRITE_THROUGH,
        )
    }
    .with_context(|| format!("could not move {} to {}", from.display(), to.display()))
}

pub(super) struct Swap<'a> {
    pub active: &'a Path,
    pub job: &'a Path,
    pub old_version: &'a str,
    pub old_hash: &'a str,
    pub new_version: &'a str,
    pub new_hash: &'a str,
}

impl Swap<'_> {
    fn check(&self, root: &Path, version: &str, fingerprint: &str) -> Result<()> {
        ensure!(
            bundle::verify(root, version, true)? == fingerprint,
            "manifest changed at {}; preserve it for review",
            root.display()
        );
        Ok(())
    }

    pub(super) fn apply(&self, verify_started: impl FnOnce() -> Result<()>) -> Result<()> {
        let before = self.job.join("previous");
        let candidate = self.job.join("candidate");
        self.check(self.active, self.old_version, self.old_hash)?;
        self.check(&candidate, self.new_version, self.new_hash)?;
        move_entry(self.active, &before)?;
        let result = (|| {
            move_entry(&candidate, self.active)?;
            self.check(self.active, self.new_version, self.new_hash)?;
            verify_started()?;
            self.finish("committed")
        })();
        if let Err(error) = result {
            self.recover().with_context(|| {
                format!("update failed ({error:#}) and rollback remains incomplete")
            })?;
            return Err(error.context("update failed and the previous bundle was restored"));
        }
        Ok(())
    }

    pub(super) fn recover(&self) -> Result<()> {
        let before = self.job.join("previous");
        let rejected = self.job.join("rejected");
        if before.try_exists()? {
            self.check(&before, self.old_version, self.old_hash)?;
            if self.active.try_exists()? {
                // Preserve the complete known candidate for diagnosis. An
                // unknown tree is evidence of interference and stays in place.
                self.check(self.active, self.new_version, self.new_hash)?;
                move_entry(self.active, &rejected)?;
            }
            move_entry(&before, self.active)?;
        }
        self.check(self.active, self.old_version, self.old_hash)?;
        self.finish("rolled-back")
    }

    pub(super) fn remove(&self) -> Result<()> {
        let before = self.job.join("previous");
        self.check(self.active, self.old_version, self.old_hash)?;
        move_entry(self.active, &before)?;
        if let Err(error) = self
            .check(&before, self.old_version, self.old_hash)
            .and_then(|()| self.finish("committed"))
        {
            self.recover().with_context(|| {
                format!("removal failed ({error:#}) and rollback remains incomplete")
            })?;
            return Err(error.context("removal failed and the previous bundle was restored"));
        }
        // The live path is absent and the removal is committed. Clean only
        // this transaction's previously verified directory in the private job.
        if let Err(error) = fs::remove_dir_all(&before) {
            eprintln!("Retained removal files in {}: {error}", before.display());
        }
        Ok(())
    }

    fn finish(&self, name: &str) -> Result<()> {
        finish(self.job, name)
    }
}

pub(super) fn finish(job: &Path, name: &str) -> Result<()> {
    write_record(job, name, &[])
}

pub(super) fn write_record(job: &Path, name: &str, body: &[u8]) -> Result<()> {
    let mut temporary = tempfile::NamedTempFile::new_in(job)?;
    let owner_handle = OpenOptions::new()
        .access_mode(0x000a_0080)
        .share_mode(3)
        .custom_flags(0x0020_0000)
        .open(temporary.path())?;
    crate::windows_control::assign_new_maintenance_owner(&owner_handle)?;
    drop(owner_handle);
    temporary.write_all(body)?;
    temporary.as_file().sync_all()?;
    let path = temporary.into_temp_path();
    move_entry(&path, &job.join(name))
}

pub(super) fn check_pending(base: &Path) -> Result<()> {
    for entry in fs::read_dir(base)? {
        let entry = entry?;
        if entry.file_name() == "lock" {
            continue;
        }
        let job = entry.path();
        let _directory = open_owned_directory(&job)?;
        if !job.join("plan.json").try_exists()? {
            // Interrupted download/extraction has never authorized a rename.
            continue;
        }
        if !job.join("committed").try_exists()? && !job.join("rolled-back").try_exists()? {
            bail!(
                "a previous update requires recovery: run \"{}\" recover-update",
                job.join("maintenance.exe").display()
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn success_and_removal_commit_a_whole_tree_without_touching_user_data() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let active = directory.path().join("active");
        let job = directory.path().join("upgrade");
        let removal = directory.path().join("uninstall");
        fs::create_dir(&job)?;
        fs::create_dir(&removal)?;
        let settings = directory.path().join("existing-settings.toml");
        fs::write(&settings, "preserve existing settings")?;
        let old_hash = bundle::tests::fixture(&active, "0.22.0")?;
        let new_hash = bundle::tests::fixture(&job.join("candidate"), "0.22.1")?;
        Swap {
            active: &active,
            job: &job,
            old_version: "0.22.0",
            old_hash: &old_hash,
            new_version: "0.22.1",
            new_hash: &new_hash,
        }
        .apply(|| Ok(()))?;
        assert_eq!(bundle::verify(&active, "0.22.1", false)?, new_hash);
        assert_eq!(
            bundle::verify(&job.join("previous"), "0.22.0", false)?,
            old_hash
        );
        assert!(job.join("committed").is_file());
        Swap {
            active: &active,
            job: &removal,
            old_version: "0.22.1",
            old_hash: &new_hash,
            new_version: "0.22.1",
            new_hash: "",
        }
        .remove()?;
        assert!(!active.exists());
        assert!(!removal.join("previous").exists());
        assert!(removal.join("committed").is_file());
        assert_eq!(fs::read_to_string(settings)?, "preserve existing settings");
        Ok(())
    }

    #[test]
    fn private_journal_blocks_a_second_transaction_but_incomplete_downloads_do_not() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let base = temporary.path().join("maintenance");
        let reader = lock(&base, false)?;
        assert!(lock(&base, true).is_err());
        drop(reader);
        let _writer = lock(&base, true)?;
        let active = temporary.path().join("active");
        let original = bundle::tests::fixture(&active, "0.22.0")?;
        let temporary_job = private_job(&base)?;
        let job = temporary_job.path();
        fs::write(job.join("partial.zip"), b"simulated interrupted download")?;
        assert!(check_pending(&base).is_ok());
        fs::create_dir(job.join("candidate"))?;
        fs::write(
            job.join("candidate/partial-file"),
            b"simulated interrupted extraction",
        )?;
        assert!(check_pending(&base).is_ok());
        write_record(job, "plan.json", b"checked fixture journal")?;
        assert!(check_pending(&base).is_err());
        assert_eq!(bundle::verify(&active, "0.22.0", false)?, original);
        finish(job, "rolled-back")?;
        assert!(check_pending(&base).is_ok());
        Ok(())
    }

    #[test]
    fn interruption_recovery_verifies_the_previous_whole_tree_at_each_move() -> Result<()> {
        for interruption in 0..4 {
            let temporary = tempfile::tempdir()?;
            let active = temporary.path().join("active");
            let job = temporary.path().join("job");
            fs::create_dir(&job)?;
            let candidate = job.join("candidate");
            let old_hash = bundle::tests::fixture(&active, "0.22.0")?;
            let new_hash = bundle::tests::fixture(&candidate, "0.22.1")?;
            let swap = Swap {
                active: &active,
                job: &job,
                old_version: "0.22.0",
                old_hash: &old_hash,
                new_version: "0.22.1",
                new_hash: &new_hash,
            };
            if interruption >= 1 {
                move_entry(&active, &job.join("previous"))?;
            }
            if interruption >= 2 {
                move_entry(&candidate, &active)?;
            }
            if interruption == 3 {
                move_entry(&active, &job.join("rejected"))?;
            }
            swap.recover()?;
            assert_eq!(bundle::verify(&active, "0.22.0", false)?, old_hash);
            assert!(job.join("rolled-back").is_file());
        }
        Ok(())
    }

    #[test]
    fn verification_failure_rolls_back_but_unknown_interference_is_preserved() -> Result<()> {
        for changed in [false, true] {
            let temporary = tempfile::tempdir()?;
            let active = temporary.path().join("active");
            let job = temporary.path().join("job");
            fs::create_dir(&job)?;
            let old_hash = bundle::tests::fixture(&active, "0.22.0")?;
            let new_hash = bundle::tests::fixture(&job.join("candidate"), "0.22.1")?;
            let swap = Swap {
                active: &active,
                job: &job,
                old_version: "0.22.0",
                old_hash: &old_hash,
                new_version: "0.22.1",
                new_hash: &new_hash,
            };
            let result = swap.apply(|| {
                if changed {
                    fs::write(active.join("foreign.txt"), "do not delete")?;
                }
                bail!("injected post-switch failure");
            });
            assert!(result.is_err());
            if changed {
                assert!(active.join("foreign.txt").exists());
                assert_eq!(
                    bundle::verify(&job.join("previous"), "0.22.0", true)?,
                    old_hash
                );
                assert!(!job.join("rolled-back").exists());
            } else {
                assert!(job.join("rolled-back").is_file());
                assert_eq!(bundle::verify(&active, "0.22.0", true)?, old_hash);
            }
        }
        Ok(())
    }
}
