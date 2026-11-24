use super::{bundle, download, msi, portable, transaction::ProtectionReservation};
use crate::uninstall::windows::{digest, ordinary_user, safe_ancestors};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    ffi::OsStr,
    fs::{self, File, OpenOptions},
    os::windows::fs::MetadataExt,
    path::{Path, PathBuf},
    time::Duration,
};

pub(super) const ARCHIVE: &str = "previous.msi";
pub(super) const LEGACY_VERSION: &str = "0.21.0";
const LEGACY_SHA256: &str = "4e6145a4e48bff47f08c8341c24c14e7fab563146e36b8338f46441f7f4da3df";
const LEGACY_MSI: &str = "miccamwatch-windows-x86_64.msi";
const HYBRID_MSI: &str = "miccamwatch-hybrid-windows-x86_64.msi";
const BACKUP: &str = "user-data-backup";

pub(super) struct Target<'a> {
    pub root: &'a Path,
    pub version: &'a str,
    pub code: &'a str,
    pub bundle_hash: &'a str,
    pub candidate_code: Option<&'a str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Action {
    InteractiveInstall,
    TargetedRemoval,
    CleanInstall,
}

pub(super) fn official_release(version: &str) -> Result<self_update::update::Release> {
    let tag = format!("v{version}");
    let release = self_update::backends::github::Update::configure()
        .repo_owner("Roman-Cuisset")
        .repo_name("miccamwatch")
        .bin_name("mcw")
        .current_version(env!("CARGO_PKG_VERSION"))
        .timeout(Duration::from_secs(20))
        .retries(0)
        .build()?
        .get_release_version(&tag)
        .context("cannot authenticate the recovery release; its retained copy was preserved")?;
    ensure!(
        release.version() == version
            && release.release_notes_url().and_then(|url| {
                url.strip_prefix("https://github.com/Roman-Cuisset/miccamwatch/releases/tag/")
            }) == Some(tag.as_str()),
        "recovery release identity mismatch"
    );
    let name = package_name(version);
    ensure!(
        release
            .assets()
            .iter()
            .filter(|asset| asset.name() == name || asset.name() == "SHA256SUMS")
            .all(|asset| super::hybrid::official_asset_url(asset.download_url())),
        "recovery asset origin mismatch"
    );
    Ok(release)
}

fn package_name(version: &str) -> &str {
    if version == LEGACY_VERSION {
        LEGACY_MSI
    } else {
        HYBRID_MSI
    }
}

pub(super) fn retain_official(job: &Path, target: &Target<'_>) -> Result<String> {
    let scratch = tempfile::tempdir_in(job)?;
    let release = official_release(target.version)?;
    let (path, hash) =
        download::download_verified(&release, package_name(target.version), scratch.path())?;
    ensure!(
        msi::inspect_recovery_package(&path, target.version)?.eq_ignore_ascii_case(target.code),
        "the authenticated release does not match the installed ProductCode"
    );
    fs::rename(path, job.join(ARCHIVE))?;
    Ok(hash)
}

pub(super) fn retain_legacy(source: &Path, job: &Path) -> Result<String> {
    safe_ancestors(source.parent().context("MSI source has no parent")?)?;
    let mut input = bundle::pin(source)?;
    ensure!(
        digest(&mut input)? == LEGACY_SHA256,
        "the recovery MSI is not the official v0.21.0 x64 artifact"
    );
    use std::io::Seek as _;
    input.rewind()?;
    let path = job.join(ARCHIVE);
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    std::io::copy(&mut input, &mut output)?;
    output.sync_all()?;
    drop(output);
    ensure!(
        msi::inspect_recovery_package(&path, LEGACY_VERSION)?
            .eq_ignore_ascii_case(msi::LEGACY_CODE),
        "unexpected v0.21.0 recovery ProductCode"
    );
    Ok(LEGACY_SHA256.to_owned())
}

fn pin_source(job: &Path, target: &Target<'_>, retained_hash: &str) -> Result<File> {
    let path = job.join(ARCHIVE);
    let mut file = bundle::pin(&path).context("the retained recovery MSI is unavailable")?;
    let expected = if target.version == LEGACY_VERSION {
        LEGACY_SHA256.to_owned()
    } else {
        let scratch = tempfile::tempdir_in(job)?;
        download::authenticated_sha256(
            &official_release(target.version)?,
            package_name(target.version),
            scratch.path(),
        )?
    };
    ensure!(
        expected == retained_hash
            && digest(&mut file)? == expected
            && msi::inspect_recovery_package(&path, target.version)?
                .eq_ignore_ascii_case(target.code),
        "recovery MSI digest or ProductCode mismatch"
    );
    Ok(file)
}

pub(super) fn verify_payload(target: &Target<'_>) -> Result<()> {
    if target.version != LEGACY_VERSION {
        ensure!(
            bundle::verify(target.root, target.version, true)? == target.bundle_hash,
            "previous hybrid bundle differs"
        );
        return Ok(());
    }
    let _directory = portable::open_owned_directory(target.root)?;
    for (name, hash) in [
        (
            "mcw.exe",
            "55753140f5f73743fe49399c3fa7a126234ea860029b7f71d673ce80a3af8fe8",
        ),
        (
            "mcw-tray.exe",
            "5a0253ea90a77bf7478f6a9f1826290f6a6055723cb12bdf4975b89bdfb1b6fe",
        ),
    ] {
        ensure!(
            digest(&mut bundle::pin(&target.root.join(name))?)? == hash,
            "legacy MSI payload differs: {name}"
        );
    }
    for entry in fs::read_dir(target.root)? {
        let name = entry?.file_name();
        ensure!(
            name == "mcw.exe"
                || name == "mcw-tray.exe"
                || name == bundle::LOCK
                    && bundle::pin(&target.root.join(&name))?.metadata()?.len() == 0,
            "unexpected data in the legacy installation directory"
        );
    }
    Ok(())
}

fn verified_empty(path: &Path) -> Result<bool> {
    if !path.try_exists()? {
        return Ok(true);
    }
    let _directory = portable::open_owned_directory(path)?;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        // Keep the permanent, zero-length native lock if it already exists.
        if entry.file_name() != bundle::LOCK || bundle::pin(&entry.path())?.metadata()?.len() != 0 {
            return Ok(false);
        }
    }
    Ok(true)
}

fn observe(target: &Target<'_>) -> Result<msi::Registration> {
    msi::require_idle()?;
    if let Some(code) = target.candidate_code {
        msi::require_new_product(code)?;
    }
    msi::require_family_owner(target.code)?;
    if target.root.try_exists()? {
        match crate::uninstall::windows::registered_owner(target.root)? {
            None => {}
            Some(crate::uninstall::windows::RegisteredOwner::Msi(code))
                if code.eq_ignore_ascii_case(target.code) => {}
            _ => anyhow::bail!("another installation owns the recovery path"),
        }
    }
    msi::registration(target.root, target.code, target.version)
}

pub(super) fn confirmed_removal(job: &Path) -> Result<bool> {
    let mut removed = false;
    for entry in fs::read_dir(job)? {
        let entry = entry?;
        if !entry.file_name().to_string_lossy().starts_with("repair-") {
            continue;
        }
        let path = entry.path();
        let _directory = portable::open_owned_directory(&path)?;
        msi::require_restart_completed(&path)?;
        let intent = path.join("intent.json");
        if !intent.try_exists()? {
            continue;
        }
        let action: Action = serde_json::from_reader(bundle::pin(&intent)?)?;
        removed |= action == Action::TargetedRemoval && msi::exit_code(&path)? == Some(0);
    }
    Ok(removed)
}

fn next_action(
    registration: msi::Registration,
    intact: bool,
    empty: bool,
    removed: bool,
) -> Result<Option<Action>> {
    use msi::Registration;
    match (registration, intact, empty, removed) {
        (Registration::Complete, true, _, _) => Ok(None),
        (Registration::Absent, true, _, false) => Ok(Some(Action::InteractiveInstall)),
        (Registration::PresentWithoutIndex, true, _, _) => Ok(Some(Action::TargetedRemoval)),
        (Registration::Absent, false, true, true) => Ok(Some(Action::CleanInstall)),
        _ => anyhow::bail!("intervention_required: MSI registration or files are ambiguous"),
    }
}

fn classify(job: &Path, target: &Target<'_>) -> Result<Option<Action>> {
    let removed = confirmed_removal(job)?;
    let registration = observe(target)?;
    let intact = verify_payload(target).is_ok();
    next_action(
        registration,
        intact,
        !intact && verified_empty(target.root)?,
        removed,
    )
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
struct ProfileSnapshot {
    files: BTreeMap<PathBuf, String>,
    autostart: serde_json::Value,
}

fn inspect_tree(
    source: &Path,
    relative: &Path,
    backup: Option<&Path>,
    remaining: &mut u64,
    files: &mut BTreeMap<PathBuf, String>,
) -> Result<()> {
    let _directory = portable::open_owned_directory(source)?;
    ensure!(
        relative.components().count() <= 64,
        "user data nesting exceeds the backup limit"
    );
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        let relative = relative.join(entry.file_name());
        let metadata = fs::symlink_metadata(&path)?;
        ensure!(
            metadata.file_attributes() & 0x400 == 0,
            "data backup encountered a reparse point"
        );
        if metadata.is_dir() {
            inspect_tree(&path, &relative, backup, remaining, files)?;
            continue;
        }
        let mut file = bundle::pin(&path)?;
        *remaining = remaining
            .checked_sub(file.metadata()?.len())
            .context("user data exceeds the recovery backup budget")?;
        ensure!(
            files.len() < 8192,
            "too many files for a bounded recovery backup"
        );
        let hash = digest(&mut file)?;
        if let Some(backup) = backup {
            let to = backup.join(&relative);
            fs::create_dir_all(to.parent().context("invalid backup file path")?)?;
            let mut copy = OpenOptions::new().write(true).create_new(true).open(&to)?;
            use std::io::Seek as _;
            file.rewind()?;
            std::io::copy(&mut file, &mut copy)?;
            copy.sync_all()?;
        }
        files.insert(relative, hash);
    }
    Ok(())
}

fn snapshot(backup: Option<&Path>) -> Result<ProfileSnapshot> {
    let mut files = BTreeMap::new();
    let mut remaining = 1024 * 1024 * 1024;
    for (name, source) in [
        ("roaming", crate::settings::config_dir()?),
        ("local", crate::settings::data_dir()?),
    ] {
        if source.try_exists()? {
            inspect_tree(&source, Path::new(name), backup, &mut remaining, &mut files)?;
        }
    }
    Ok(ProfileSnapshot {
        files,
        autostart: crate::autostart::maintenance_snapshot()?,
    })
}

pub(super) fn verify_backup(job: &Path) -> Result<()> {
    let backup = job.join(BACKUP);
    let _directory = portable::open_owned_directory(&backup)?;
    let expected: ProfileSnapshot =
        serde_json::from_reader(bundle::pin(&backup.join("snapshot.json"))?)?;
    ensure!(
        snapshot(None)? == expected,
        "user data or autostart changed; its backup was preserved"
    );
    for (path, hash) in &expected.files {
        ensure!(
            path.components()
                .all(|part| matches!(part, std::path::Component::Normal(_))),
            "invalid backup path"
        );
        let file = backup.join(path);
        safe_ancestors(file.parent().context("backup file has no parent")?)?;
        ensure!(
            digest(&mut bundle::pin(&file)?)? == *hash,
            "user data backup verification failed"
        );
    }
    Ok(())
}

pub(super) fn capture_backup(job: &Path) -> Result<()> {
    if !job.join(BACKUP).try_exists()? {
        let staging = tempfile::tempdir_in(job)?;
        let before = snapshot(Some(staging.path()))?;
        portable::write_record(
            staging.path(),
            "snapshot.json",
            &serde_json::to_vec(&before)?,
        )?;
        fs::rename(staging.path(), job.join(BACKUP))?;
    }
    verify_backup(job)
}

fn finish(job: &Path) -> Result<()> {
    if !job.join("recovery-result.json").try_exists()? {
        portable::write_record(
            job,
            "recovery-result.json",
            br#"{"status":"previous_bundle_verified","msi_registration":"complete"}"#,
        )?;
    }
    portable::finish(job, "rolled-back")
}

fn verify_runtime(job: &Path, target: &Target<'_>) -> Result<()> {
    if target.version != LEGACY_VERSION {
        return super::hybrid::binaries(job, target.root, target.version);
    }
    // The historical tray has no --version command. Its pinned payload hash
    // is checked separately without launching a new monitoring process.
    let scratch = tempfile::tempdir_in(job)?;
    super::validate_reported_version(&target.root.join("mcw.exe"), "mcw 0.21.0", scratch.path())
}

pub(super) fn recover(job: &Path, target: &Target<'_>, retained_hash: &str) -> Result<()> {
    ordinary_user()?;
    ensure!(
        !std::env::current_dir()?
            .canonicalize()?
            .starts_with(target.root),
        "open the terminal outside the installation directory"
    );
    super::hybrid::require_idle(target.root)?;
    let _protection = ProtectionReservation::acquire()?;
    let Some(action) = classify(job, target)? else {
        if job.join(BACKUP).try_exists()? {
            verify_backup(job)?;
        }
        verify_runtime(job, target)?;
        finish(job)?;
        return Ok(());
    };
    let package = pin_source(job, target, retained_hash)?;
    msi::require_space(
        job,
        package.metadata()?.len().saturating_mul(4) + 128 * 1024 * 1024,
    )?;
    capture_backup(job)?;
    let (prompt, keyword) = match action {
        Action::InteractiveInstall => (
            "Restore the previous ProductCode interactively using the verified original MSI. Windows may request UAC.",
            "restore",
        ),
        Action::TargetedRemoval => (
            "The previous ProductCode is registered but its UpgradeCode index is incomplete. Remove that exact product? A separate reinstall will be needed.",
            "remove",
        ),
        Action::CleanInstall => (
            "No conflicting MSI registration or payload remains. Reinstall the verified original MSI?",
            "reinstall",
        ),
    };
    println!(
        "MicCamWatch monitoring may be unavailable while maintenance runs. Data backup: {}",
        job.join(BACKUP).display()
    );
    let step = tempfile::Builder::new()
        .prefix("repair-")
        .tempdir_in(job)?
        .keep();
    if let Err(error) = super::hybrid::confirm(prompt, keyword) {
        portable::write_record(
            &step,
            "result.json",
            br#"{"status":"cancelled_before_msi"}"#,
        )?;
        return Err(error);
    }
    super::hybrid::require_idle(target.root)?;
    ensure!(
        classify(job, target)? == Some(action),
        "MSI state changed after confirmation"
    );
    verify_backup(job)?;
    portable::write_record(&step, "intent.json", &serde_json::to_vec(&action)?)?;
    let path = job.join(ARCHIVE);
    let (operation, argument, interactive) = match action {
        Action::InteractiveInstall => ("/i", msi::dos_path(&path)?.as_os_str(), true),
        Action::TargetedRemoval => ("/x", OsStr::new(target.code), false),
        Action::CleanInstall => ("/i", msi::dos_path(&path)?.as_os_str(), false),
    };
    if let Err(error) = msi::run_recovery(&step, operation, argument, interactive) {
        let code = msi::exit_code(&step)?;
        let status = match code {
            Some(1223 | 1602) => "msi_cancelled",
            Some(1618) => "msi_busy",
            Some(1641 | 3010) => "restart_required",
            _ => "intervention_required",
        };
        portable::write_record(
            &step,
            "result.json",
            &serde_json::to_vec(&serde_json::json!({"status":status,"msi_exit_code":code}))?,
        )?;
        return Err(error)
            .with_context(|| format!("{status}; log: {}", step.join("msi.log").display()));
    }
    verify_backup(job)?;
    let next = classify(job, target)?;
    portable::write_record(&step, "observed.json", &serde_json::to_vec(&next)?)?;
    if let Some(next) = next {
        anyhow::bail!(
            "intervention_required: {next:?}; run recover-update --repair again to review and authorize the next step"
        );
    }
    verify_runtime(job, target)?;
    finish(job)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Action, msi::Registration::*, next_action};

    #[test]
    fn loss_requires_interactive_registration_before_removal() {
        assert_eq!(
            next_action(Absent, true, false, false).unwrap(),
            Some(Action::InteractiveInstall)
        );
        assert_eq!(
            next_action(PresentWithoutIndex, true, false, false).unwrap(),
            Some(Action::TargetedRemoval)
        );
        assert_eq!(
            next_action(Absent, false, true, true).unwrap(),
            Some(Action::CleanInstall)
        );
        assert_eq!(next_action(Complete, true, false, true).unwrap(), None);
    }

    #[test]
    fn incomplete_or_mixed_states_never_authorize_a_removal() {
        assert!(next_action(Absent, false, true, false).is_err());
        assert!(next_action(Absent, true, false, true).is_err());
        assert!(next_action(PresentWithoutIndex, false, false, false).is_err());
        assert!(next_action(Complete, false, false, true).is_err());
    }

    #[test]
    fn unavailable_or_substituted_legacy_msi_is_not_staged() -> anyhow::Result<()> {
        let directory = tempfile::tempdir()?;
        let job = tempfile::tempdir()?;
        let path = directory.path().join("previous.msi");
        assert!(super::retain_legacy(&path, job.path()).is_err());
        std::fs::write(&path, b"inert fixture")?;
        assert!(super::retain_legacy(&path, job.path()).is_err());
        assert!(!job.path().join(super::ARCHIVE).exists());
        Ok(())
    }
}
