//! Installation ownership for the managed Linux hybrid distribution.
use anyhow::{Context, Result, ensure};
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::Read,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

use crate::{autostart::Directory, frontends::desktop::package};

const MARKER: &str = ".miccamwatch-managed-v1";
const LEASE: &str = ".bundle.lock";
const ACTIVE: &str = "bundle";

fn file(path: &Path) -> Result<File> {
    Directory::open(path.parent().context("missing package directory")?, false)?
        .file(path.file_name().context("missing package file name")?)?
        .with_context(|| format!("missing package file {}", path.display()))
}

fn managed_root(executable: &Path) -> Result<Option<PathBuf>> {
    // The only admissible managed image is <root>/bundle/mcw. In particular a
    // process mapped just before a swap cannot start from a retained revision.
    for root in executable.ancestors().skip(2).take(3) {
        match fs::symlink_metadata(root.join(MARKER)) {
            Ok(_) => {
                let mut marker = Vec::new();
                file(&root.join(MARKER))?
                    .take(64)
                    .read_to_end(&mut marker)?;
                ensure!(
                    marker == b"MicCamWatch Linux bundle 1\n",
                    "invalid managed MicCamWatch root"
                );
                ensure!(
                    executable == root.join(ACTIVE).join("mcw"),
                    "this is a retained or staged bundle; launch the active MicCamWatch installation"
                );
                return Ok(Some(root.to_owned()));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).context("cannot determine Linux bundle ownership"),
        }
    }
    Ok(None)
}

fn locked_root(exclusive: bool) -> Result<Option<(PathBuf, File)>> {
    let current = std::env::current_exe()?;
    let Some(root) = managed_root(&current)? else {
        return Ok(None);
    };
    let lease = file(&root.join(LEASE))?;
    if exclusive {
        lease
            .try_lock()
            .context("MicCamWatch is still running; quit the owned tray before uninstalling")?;
    } else {
        lease
            .try_lock_shared()
            .context("MicCamWatch package transaction in progress; try again after it completes")?;
    }
    let running = fs::metadata("/proc/self/exe")?;
    let active = file(&root.join(ACTIVE).join("mcw"))?.metadata()?;
    ensure!(
        running.dev() == active.dev() && running.ino() == active.ino(),
        "MicCamWatch bundle changed during startup; reopen the active installation"
    );
    Ok(Some((root, lease)))
}

pub fn hold_process_lease() -> Result<Option<File>> {
    Ok(locked_root(false)?.map(|(_, lease)| lease))
}

fn validate_tree(bundle: &Path, expected: &[String]) -> Result<BTreeSet<PathBuf>> {
    let mut allowed: BTreeSet<_> = expected.iter().map(|name| bundle.join(name)).collect();
    allowed.insert(bundle.join(package::MANIFEST));
    let directories: BTreeSet<_> = allowed
        .iter()
        .flat_map(|path| {
            path.ancestors()
                .skip(1)
                .take_while(|parent| *parent != bundle)
                .map(Path::to_owned)
        })
        .collect();
    let mut found = BTreeSet::new();
    for directory in std::iter::once(bundle).chain(directories.iter().map(PathBuf::as_path)) {
        let scan = Directory::open(directory, false)?;
        for name in scan.names()? {
            let item = directory.join(name);
            if directories.contains(&item) {
                ensure!(
                    fs::symlink_metadata(&item)?.is_dir(),
                    "unexpected package directory type"
                );
            } else {
                ensure!(
                    allowed.contains(&item),
                    "foreign file in managed bundle: {}",
                    item.display()
                );
                found.insert(item);
            }
        }
    }
    ensure!(found == allowed, "incomplete managed package");
    Ok(directories)
}

pub(super) fn remove() -> Result<Option<u8>> {
    crate::autostart::ordinary_user()?;
    let Some((root, _lease)) = locked_root(true)? else {
        return Ok(None);
    };
    let bundle = root.join(ACTIVE);
    let current = bundle.join("mcw");
    let manifest_path = bundle.join(package::MANIFEST);
    let expected = package::verify(file(&manifest_path)?, |name| file(&bundle.join(name)))?;
    let directories = validate_tree(&bundle, &expected)?;
    let tombstone = root.join(".removed-bundle");
    ensure!(
        !tombstone.try_exists()?,
        "inspect the retained .removed-bundle before uninstalling again"
    );

    // Use the native protection and registration owners before changing the
    // package. Preserve recordings and user settings outside the payload.
    crate::updater::root_camera_installation_absent()?;
    let configuration = Directory::open(&crate::settings::config_dir()?, true)?;
    let _preferences = configuration.lock("settings.lock")?;
    let _settings = configuration.file(std::ffi::OsStr::new("settings.toml"))?;
    super::ensure_preferences_inactive()?;
    let registration = crate::autostart::UninstallRegistration::prepare(&current)?;
    let _microphone = crate::platform::linux::control::reserve_uninstall()?;
    fs::rename(&bundle, &tombstone).context("cannot retire the complete Linux bundle")?;
    if let Err(error) = registration.remove() {
        fs::rename(&tombstone, &bundle).with_context(|| {
            format!(
                "autostart removal failed ({error:#}); bundle retained at {}",
                tombstone.display()
            )
        })?;
        return Err(error).context("package rollback completed after autostart removal failed");
    }
    let retired = |path: &Path| -> PathBuf {
        tombstone.join(path.strip_prefix(&bundle).expect("validated managed path"))
    };
    for name in &expected {
        fs::remove_file(tombstone.join(name)).with_context(|| {
            format!("retired bundle requires cleanup at {}", tombstone.display())
        })?;
    }
    fs::remove_file(retired(&manifest_path))?;
    let mut by_depth: Vec<_> = directories.iter().collect();
    by_depth.sort_by_key(|directory| std::cmp::Reverse(directory.components().count()));
    for directory in by_depth {
        fs::remove_dir(retired(directory))?;
    }
    fs::remove_dir(&tombstone)?;
    println!(
        "Managed MicCamWatch payload removed. Preferences, history and recovery packages are retained."
    );
    Ok(Some(0))
}
