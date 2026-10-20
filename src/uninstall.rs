//! Installation-scoped removal. Data and native restoration records are never purged.
use anyhow::{Context, Result, bail};
#[cfg(unix)]
use std::{fs::File, path::Path};

#[cfg(windows)]
#[path = "uninstall/windows.rs"]
pub(crate) mod windows;

/// Remove this installation, or report its exact owner's procedure without
/// claiming success. This branch must run before monitor/control startup.
pub fn run() -> Result<u8> {
    #[cfg(windows)]
    {
        windows::run()
    }
    #[cfg(unix)]
    {
        unix_run()
    }
}

#[cfg(windows)]
pub fn run_worker(parent_pid: u32, parent_created: u64) -> Result<u8> {
    windows::run_worker(parent_pid, parent_created)
}

/// Packaging-only producer; no settings or native controls are opened.
#[cfg(windows)]
pub fn portable_receipt() -> Result<String> {
    let current = std::env::current_exe()?;
    let bytes = windows::receipt_bytes(
        &current,
        &current.with_file_name("mcw-tray.exe"),
        env!("CARGO_PKG_VERSION"),
    )?;
    String::from_utf8(bytes).context("portable receipt is not UTF-8")
}

fn ensure_preferences_inactive() -> Result<()> {
    let settings = crate::settings::Settings::load()?;
    if settings.mute_on_lock || settings.block_camera_on_lock {
        bail!(
            "uninstall refused: automatic protection is requested; explicitly disable microphone/camera protection on lock and restore with mcw unmute / mcw camera allow before retrying; no protection was released"
        );
    }
    Ok(())
}

#[cfg(unix)]
fn unix_run() -> Result<u8> {
    use crate::autostart::Directory;
    use std::ffi::OsStr;
    crate::autostart::ordinary_user()?;
    let current = std::env::current_exe().context("cannot locate this installed executable")?;
    if unix_owner_guidance(&current)? {
        return Ok(2);
    }
    // Reuse receipt and checksum validation, including the Linux privileged
    // helper refusal, BEFORE stopping or changing any frontend/registration.
    let removal = crate::updater::prepare_managed_uninstall(&current)?;
    let configuration = Directory::open(&crate::settings::config_dir()?, true)?;
    let preferences = configuration.lock("settings.lock")?;
    let _ = configuration.file(OsStr::new("settings.toml"))?;
    ensure_preferences_inactive()?;
    let registration = crate::autostart::UninstallRegistration::prepare(&current)?;
    #[cfg(target_os = "linux")]
    let microphone = crate::platform::linux::control::reserve_uninstall()?;
    #[cfg(target_os = "macos")]
    let microphone = crate::platform::macos::control::reserve_uninstall()?;
    #[cfg(target_os = "macos")]
    let camera = crate::privacy::reserve_uninstall()?;
    crate::frontends::tray::stop_current_installation()?;
    registration.remove()?;
    // These are the actual native ownership locks, not new uninstall-only
    // locks. Keep them held while exec replaces our image and the installer
    // performs its final running-target/receipt/PATH checks and removal.
    inherit(&preferences)?;
    registration.inherit_across_exec()?;
    #[cfg(target_os = "linux")]
    inherit(&microphone)?;
    #[cfg(target_os = "macos")]
    {
        microphone.inherit_across_exec()?;
        camera.inherit_across_exec()?;
    }
    removal.execute()
}

#[cfg(unix)]
fn inherit(file: &File) -> Result<()> {
    use std::os::fd::AsRawFd;
    if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETFD, 0) } == -1 {
        return Err(std::io::Error::last_os_error())
            .context("cannot retain native ownership across uninstall handoff");
    }
    Ok(())
}

#[cfg(unix)]
fn unix_owner_guidance(current: &Path) -> Result<bool> {
    use std::{ffi::OsStr, process::Command};
    let physical = current
        .canonicalize()
        .context("cannot resolve this executable's provenance")?;
    #[cfg(target_os = "macos")]
    if let Some(bundle) = physical.ancestors().find(|path| {
        path.extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("app"))
    }) {
        // Never invalidate a seal by deleting only Contents/MacOS/mcw. A whole
        // app has no public-installer receipt and may be managed externally.
        eprintln!(
            "macOS application bundle preserved: {}. Explicitly restore using this app's mcw unmute and mcw camera allow; finish any Device Management approval, disable owned autostart with this app's mcw autostart disable, then quit MicCamWatch. In Finder, move this exact whole app to Trash (or use its administrator/MDM owner). Settings/history/policy remain. No automatic app-bundle deletion or success is claimed.",
            bundle.display()
        );
        return Ok(true);
    }
    // Cargo is itself an installation manager. Its metadata does not retain
    // historical payload hashes, so do not invent an unchanged-file receipt.
    if current.parent().and_then(Path::file_name) == Some(OsStr::new("bin")) {
        let root = current
            .parent()
            .and_then(Path::parent)
            .context("invalid bin prefix")?;
        if let Ok(directory) = crate::autostart::Directory::open(root, false)
            && let Some(bytes) = directory.read(".crates2.json")?
        {
            let metadata: serde_json::Value = serde_json::from_slice(&bytes)
                .context("invalid Cargo installation metadata; files preserved")?;
            if metadata
                .get("installs")
                .and_then(serde_json::Value::as_object)
                .is_some_and(|installs| {
                    installs.iter().any(|(package, install)| {
                        package.starts_with("miccamwatch ")
                            && install
                                .get("bins")
                                .and_then(serde_json::Value::as_array)
                                .is_some_and(|bins| {
                                    bins.iter().any(|bin| bin.as_str() == Some("mcw"))
                                })
                    })
                })
            {
                eprintln!(
                    "Cargo-owned installation preserved. After explicit restoration and normal frontend exit, use: cargo uninstall --root {} miccamwatch. Cargo controls its own manifest and payload removal; MCW does not claim success.",
                    shell_quote(root)?
                );
                return Ok(true);
            }
        }
    }
    if physical
        .components()
        .any(|component| component.as_os_str() == "Cellar")
    {
        let output = Command::new("brew")
            .args(["--prefix", "--installed", "miccamwatch"])
            .output();
        if let Ok(output) = output
            && output.status.success()
        {
            let prefix =
                String::from_utf8(output.stdout).context("Homebrew prefix is not UTF-8")?;
            if Path::new(prefix.trim())
                .join("bin/mcw")
                .canonicalize()
                .is_ok_and(|target| target == physical)
            {
                eprintln!(
                    "Homebrew-owned installation preserved. Explicitly restore and quit its frontend, then use: brew uninstall --formula miccamwatch. Settings/history/policy remain; no uninstall success is claimed."
                );
                return Ok(true);
            }
        }
        bail!(
            "Cellar installation provenance could not be verified; use its Homebrew owner; nothing was stopped or removed"
        );
    }
    #[cfg(target_os = "linux")]
    if let Ok(output) = Command::new("pacman")
        .args([OsStr::new("-Qoq"), OsStr::new("--"), physical.as_os_str()])
        .output()
        && output.status.success()
    {
        let package = String::from_utf8(output.stdout).context("package owner is not UTF-8")?;
        let package = package.trim();
        if matches!(package, "miccamwatch" | "miccamwatch-bin") {
            eprintln!(
                "Arch package-manager installation preserved. Explicitly restore and quit its frontend, then use: sudo pacman -R -- {package}. The manager owns removal; no elevation or success is claimed."
            );
            return Ok(true);
        }
        bail!(
            "executable is owned by another Arch package; use that package owner's removal procedure; files preserved"
        );
    }
    #[cfg(target_os = "macos")]
    if current.parent().and_then(Path::file_name) != Some(OsStr::new("bin")) {
        let directory = crate::autostart::Directory::open(
            current.parent().context("executable has no directory")?,
            false,
        )?;
        let _ = directory
            .file(current.file_name().context("executable has no filename")?)?
            .context("standalone executable disappeared")?;
        eprintln!(
            "Standalone macOS executable preserved (no installer ownership receipt). Explicitly restore with this exact mcw unmute / mcw camera allow, disable its owned autostart, and quit its frontends. Then remove only this exact file: rm -- {}. Settings/history/policy remain; no automatic removal or success is claimed.",
            shell_quote(current)?
        );
        return Ok(true);
    }
    Ok(false)
}

#[cfg(unix)]
fn shell_quote(path: &Path) -> Result<String> {
    let text = path.to_str().context("installation path is not UTF-8")?;
    if text.chars().any(char::is_control) {
        bail!("installation path contains control characters; no shell command emitted");
    }
    Ok(format!("'{}'", text.replace('\'', "'\\''")))
}
