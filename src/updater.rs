use anyhow::{Context, Result, bail};
use self_update::cargo_crate_version;
use std::{
    cmp::Ordering,
    env, fs,
    io::Read,
    os::windows::process::CommandExt,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

mod download;
mod process;
mod transaction;

pub(crate) fn stop_installed_tray() -> Result<bool> {
    let current = env::current_exe().context("failed to locate mcw executable")?;
    let companion = current.with_file_name("mcw-tray.exe");
    for target in [&companion, &current] {
        if target.try_exists()?
            && let Some(running) = process::owned_running_tray(target)?
        {
            running.stop()?;
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn update() -> Result<()> {
    let current = env::current_exe().context("failed to locate the installed mcw executable")?;
    let installation_lock = crate::autostart::lock_installation(&current)?;
    let tray = current.with_file_name("mcw-tray.exe");
    let releases = self_update::backends::github::Update::configure()
        .repo_owner("Roman-Cuisset")
        .repo_name("miccamwatch")
        .bin_name("mcw")
        .asset_identifier("windows-x86_64.zip")
        .current_version(cargo_crate_version!())
        .build()?
        .get_latest_release()
        .context("failed to check the latest release")?;
    let latest = releases
        .latest()
        .context("no MicCamWatch release is available")?;
    let version = latest.version();
    let comparison = self_update::version::cmp_versions(cargo_crate_version!(), version)
        .context("failed to compare MicCamWatch versions")?;
    let tray_is_current = tray.try_exists()?
        && crate::autostart::tray_matches_current_version(&tray)
            .map_err(|error| tray_inspection_error(&tray, error))?;
    if comparison == Ordering::Greater {
        if !tray_is_current {
            bail!(
                "mcw {} is newer than the latest release, but {} is missing or outdated; install matching binaries together",
                cargo_crate_version!(),
                tray.display()
            );
        }
        println!(
            "mcw {} and mcw-tray are already up to date.",
            cargo_crate_version!()
        );
        return Ok(());
    }
    if comparison == Ordering::Equal && tray_is_current {
        crate::autostart::refresh_if_enabled(version, &current, &installation_lock)?;
        println!("mcw {version} and mcw-tray are already up to date.");
        return Ok(());
    }
    // Stage beside the installation so all swaps and rollback moves are same-volume.
    // TempDir is exclusive; the single package download never touches installed bytes.
    let stage = tempfile::Builder::new()
        .prefix(".mcw-update-")
        .tempdir_in(
            current
                .parent()
                .context("mcw has no installation directory")?,
        )
        .map_err(|error| {
            transaction::io_error("create update staging directory", &current, error)
        })?;
    let staged_tray = stage.path().join("mcw-tray.exe");
    println!(
        "Preparing matching mcw/mcw-tray release {version}; installation remains unchanged until verification completes."
    );
    download::stage_package(latest, stage.path())?;
    validate_pe(&staged_tray)?;
    if !crate::autostart::tray_matches_version(&staged_tray, version)
        .map_err(|error| tray_inspection_error(&staged_tray, error))?
    {
        bail!("staged tray version does not match release {version}; installation unchanged");
    }
    let staged_cli = stage.path().join("mcw.exe");
    validate_pe(&staged_cli)?;
    validate_cli_version(&staged_cli, version, stage.path())?;
    let mut replacements = vec![transaction::Replacement {
        staged: staged_tray,
        target: tray.clone(),
        backup: stage.path().join("previous-mcw-tray.exe"),
        had_original: tray.try_exists()?,
        running_cli: false,
    }];
    if comparison == Ordering::Less {
        // Same-version repair verifies the paired package but leaves this CLI untouched.
        replacements.push(transaction::Replacement {
            staged: staged_cli,
            target: current.clone(),
            backup: stage.path().join("previous-mcw.exe"),
            had_original: true,
            running_cli: true,
        });
    }
    // Identify ownership after staging, then wait on that exact process handle.
    let running = if tray.try_exists()? {
        process::owned_running_tray(&tray)?
    } else {
        None
    };
    let was_running = running.is_some();
    let count = replacements.len();
    let mut operations = transaction::NativeOperations::new(replacements, running, tray);
    let outcome = transaction::apply(&mut operations, count, was_running);
    let recovery_backup_remains = outcome.is_err()
        && operations
            .replacements
            .iter()
            .any(|replacement| replacement.backup.try_exists().unwrap_or(true));
    if recovery_backup_remains {
        let retained = stage.keep();
        eprintln!(
            "Failed update staging/transaction backups retained at {}. Inspect the reported error before removing this directory; never restore quarantined files.",
            retained.display()
        );
    } else {
        // A mapped old CLI may remain until this updater exits. Do not launch a
        // self-deleting helper executable just to clean up its backup.
        let retained = stage.path().to_owned();
        if let Err(error) = stage.close() {
            eprintln!(
                "Update backup directory retained at {}: {error}. Remove it after this mcw process exits; do not recover quarantined files.",
                retained.display()
            );
        }
    }
    outcome?;
    crate::autostart::refresh_if_enabled(version, &current, &installation_lock)
        .context("binaries updated, but the owned autostart registration could not be repointed")?;
    if comparison == Ordering::Less {
        println!("mcw and mcw-tray updated to {version}.");
    } else {
        println!("mcw-tray repaired to match mcw {version}.");
    }
    Ok(())
}

fn tray_inspection_error(path: &Path, error: anyhow::Error) -> anyhow::Error {
    let code = error.chain().find_map(|cause| {
        cause
            .downcast_ref::<std::io::Error>()
            .and_then(std::io::Error::raw_os_error)
    });
    match code {
        Some(code) => transaction::io_error(
            "inspect tray executable version",
            path,
            std::io::Error::from_raw_os_error(code),
        ),
        None => error,
    }
}

fn validate_cli_version(path: &Path, version: &str, stage: &Path) -> Result<()> {
    struct ValidationChild(std::process::Child);
    impl Drop for ValidationChild {
        fn drop(&mut self) {
            if !matches!(self.0.try_wait(), Ok(Some(_))) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
    }
    let output_path = stage.join("cli-version-output");
    let output = fs::OpenOptions::new()
        .create_new(true)
        .read(true)
        .write(true)
        .open(&output_path)
        .map_err(|error| {
            transaction::io_error("create CLI validation output", &output_path, error)
        })?;
    // A file avoids an unbounded pipe read or a descendant retaining a pipe handle.
    let mut child = ValidationChild(
        Command::new(path)
            .arg("--version")
            .creation_flags(0x0800_0000)
            .stdout(Stdio::from(output.try_clone()?))
            .stderr(Stdio::from(output))
            .spawn()
            .map_err(|error| transaction::io_error("validate staged CLI version", path, error))?,
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(status) = child
            .0
            .try_wait()
            .map_err(|error| transaction::io_error("wait for staged CLI validation", path, error))?
        {
            break status;
        }
        if Instant::now() >= deadline || fs::metadata(&output_path)?.len() > 4096 {
            child.0.kill().map_err(|error| {
                transaction::io_error("stop staged CLI validation", path, error)
            })?;
            child.0.wait().map_err(|error| {
                transaction::io_error("wait for stopped CLI validation", path, error)
            })?;
            bail!(
                "staged CLI version validation exceeded its time/output bound; installation unchanged"
            );
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    if fs::metadata(&output_path)?.len() > 4096 {
        bail!("staged CLI version validation output is too large; installation unchanged");
    }
    let mut bytes = [0u8; 4097];
    let mut file = fs::File::open(&output_path)?;
    let mut length = 0;
    while length < bytes.len() {
        let count = file.read(&mut bytes[length..])?;
        if count == 0 {
            break;
        }
        length += count;
    }
    if length > 4096 {
        bail!("staged CLI version validation output is too large; installation unchanged");
    }
    let reported = std::str::from_utf8(&bytes[..length])
        .context("staged CLI version output is not valid UTF-8; installation unchanged")?;
    if !status.success() || reported.trim().strip_prefix("mcw ") != Some(version) {
        bail!(
            "staged CLI did not report expected version mcw {version} (status {status}, output {:?}); installation unchanged",
            reported.trim()
        );
    }
    Ok(())
}

fn validate_pe(path: &Path) -> Result<()> {
    let mut file = fs::File::open(path)
        .map_err(|error| transaction::io_error("read staged executable", path, error))?;
    let mut header = [0u8; 2];
    file.read_exact(&mut header)
        .map_err(|error| transaction::io_error("read staged executable header", path, error))?;
    if header != *b"MZ" {
        bail!(
            "staged file {} is not a Windows executable; installation unchanged",
            path.display()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_staged_binary_is_rejected_before_transaction() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("mcw.exe");
        fs::write(&path, b"not an executable")?;
        assert!(validate_pe(&path).is_err());
        Ok(())
    }
}
