use super::{bundle, portable};
use crate::uninstall::windows::valid_product_code;
use anyhow::{Context, Result, bail, ensure};
use std::{
    ffi::OsStr,
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
    process::Command,
};
use windows::{
    Win32::System::{
        ApplicationInstallationAndServicing::*, SystemInformation::GetSystemDirectoryW,
    },
    core::{PCWSTR, PWSTR, w},
};

const UPGRADE_CODE: &str = "{8FC9317B-4517-4E83-9F52-CF3257584BF6}";

struct Handle(MSIHANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        unsafe { MsiCloseHandle(self.0) };
    }
}

pub(super) fn wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain([0]).collect()
}

// Windows Installer still uses Win32 DOS paths in its source list. The
// updater canonicalizes paths for identity checks, then supplies the same
// local path without the extended-length prefix to Windows Installer.
fn dos_path(path: &Path) -> Result<&Path> {
    let value = path.to_str().context("MSI path is not Unicode")?;
    let value = value.strip_prefix(r"\\?\").unwrap_or(value);
    ensure!(
        value.as_bytes().get(1..3) == Some(b":\\") && value.encode_utf16().count() < 260,
        "MSI requires a local path shorter than MAX_PATH"
    );
    Ok(Path::new(value))
}

fn checked(status: u32, action: &str) -> Result<()> {
    ensure!(status == 0, "Windows Installer {action} returned {status}");
    Ok(())
}

fn database(path: &Path) -> Result<Handle> {
    let mut result = Handle(MSIHANDLE(0));
    checked(
        unsafe {
            MsiOpenDatabaseW(
                PCWSTR(wide(dos_path(path)?.as_os_str()).as_ptr()),
                PCWSTR::null(),
                &mut result.0,
            )
        },
        "open database",
    )?;
    Ok(result)
}

fn property(db: &Handle, name: &str) -> Result<Option<String>> {
    let query = format!("SELECT `Value` FROM `Property` WHERE `Property`='{name}'");
    let mut view = Handle(MSIHANDLE(0));
    checked(
        unsafe {
            MsiDatabaseOpenViewW(db.0, PCWSTR(wide(OsStr::new(&query)).as_ptr()), &mut view.0)
        },
        "open property query",
    )?;
    checked(
        unsafe { MsiViewExecute(view.0, MSIHANDLE(0)) },
        "execute property query",
    )?;
    let mut row = Handle(MSIHANDLE(0));
    let status = unsafe { MsiViewFetch(view.0, &mut row.0) };
    if status == 259 {
        return Ok(None);
    }
    checked(status, "fetch property")?;
    let mut value = [0u16; 512];
    let mut length = value.len() as u32;
    checked(
        unsafe {
            MsiRecordGetStringW(row.0, 1, Some(PWSTR(value.as_mut_ptr())), Some(&mut length))
        },
        "read property",
    )?;
    Ok(Some(String::from_utf16(&value[..length as usize])?))
}

pub(super) fn inspect_candidate(path: &Path, version: &str) -> Result<String> {
    let db = database(path)?;
    let mut summary = Handle(MSIHANDLE(0));
    checked(
        unsafe { MsiGetSummaryInformationW(db.0, PCWSTR::null(), 0, &mut summary.0) },
        "open summary",
    )?;
    let mut template = [0u16; 256];
    let mut length = template.len() as u32;
    let mut value_type = 0;
    checked(
        unsafe {
            MsiSummaryInfoGetPropertyW(
                summary.0,
                7,
                &mut value_type,
                std::ptr::null_mut(),
                None,
                Some(PWSTR(template.as_mut_ptr())),
                Some(&mut length),
            )
        },
        "read x64 template",
    )?;
    let template = String::from_utf16(&template[..length as usize])?;
    ensure!(
        template.starts_with("x64;"),
        "candidate MSI architecture is not x64"
    );
    ensure!(
        property(&db, "ProductName")?.as_deref() == Some("MicCamWatch")
            && property(&db, "Manufacturer")?.as_deref() == Some("Roman Cuisset")
            && property(&db, "UpgradeCode")?
                .is_some_and(|value| value.eq_ignore_ascii_case(UPGRADE_CODE))
            && property(&db, "ProductVersion")?.as_deref() == Some(version)
            && property(&db, "MCW_BUNDLE_IPC")?.as_deref()
                == Some(&crate::frontends::desktop::PROTOCOL_VERSION.to_string())
            && property(&db, "ALLUSERS")?.is_none_or(|value| value.is_empty()),
        "candidate MSI has incompatible product identity, version, scope or IPC"
    );
    let product = property(&db, "ProductCode")?.context("missing MSI ProductCode")?;
    ensure!(valid_product_code(&product), "invalid MSI ProductCode");
    Ok(product)
}

fn installed_property(code: &str, property: PCWSTR) -> Result<Option<String>> {
    let mut buffer = [0u16; 32768];
    let mut length = buffer.len() as u32;
    let status = unsafe {
        MsiGetProductInfoExW(
            PCWSTR(wide(OsStr::new(code)).as_ptr()),
            PCWSTR::null(),
            MSIINSTALLCONTEXT_USERUNMANAGED,
            property,
            Some(PWSTR(buffer.as_mut_ptr())),
            Some(&mut length),
        )
    };
    if status == 1605 {
        return Ok(None);
    }
    checked(status, "read installed per-user product")?;
    Ok(Some(String::from_utf16(&buffer[..length as usize])?))
}

pub(super) fn verify_installed(root: &Path, code: &str, version: &str) -> Result<()> {
    ensure!(valid_product_code(code), "invalid installed ProductCode");
    ensure!(
        installed_property(code, w!("State"))?.as_deref() == Some("5")
            && installed_property(code, w!("VersionString"))?.as_deref() == Some(version)
            && installed_property(code, w!("InstallLocation"))?.is_some_and(|value| !value
                .is_empty()
                && Path::new(&value)
                    .canonicalize()
                    .is_ok_and(|path| path == root)),
        "MSI registration does not match this installed per-user product"
    );
    let cache =
        installed_property(code, w!("LocalPackage"))?.context("MSI package cache missing")?;
    let db = database(Path::new(&cache))?;
    ensure!(
        property(&db, "UpgradeCode")?.is_some_and(|value| value.eq_ignore_ascii_case(UPGRADE_CODE))
            && property(&db, "ProductCode")?.is_some_and(|value| value.eq_ignore_ascii_case(code))
            && property(&db, "ProductName")?.as_deref() == Some("MicCamWatch"),
        "cached MSI does not belong to the official MicCamWatch upgrade family"
    );
    Ok(())
}

pub(super) fn require_new_product(code: &str) -> Result<()> {
    ensure!(
        valid_product_code(code) && installed_property(code, w!("State"))?.is_none(),
        "the candidate ProductCode already exists in this Windows Installer context"
    );
    Ok(())
}

pub(super) fn install(
    path: &Path,
    stage: &Path,
    root: &Path,
    old: &str,
    new: &str,
    old_version: &str,
    new_version: &str,
) -> Result<()> {
    let mut buffer = [0u16; 32768];
    let length = unsafe { GetSystemDirectoryW(Some(&mut buffer)) } as usize;
    ensure!(
        length > 0 && length < buffer.len(),
        "cannot locate Windows Installer system directory"
    );
    let executable = PathBuf::from(String::from_utf16(&buffer[..length])?).join("msiexec.exe");
    let package = dos_path(path)?;
    let log = stage.join("msi.log");
    let log = dos_path(&log)?;
    portable::write_record(stage, "msi-started", &boot_time()?.to_le_bytes())?;
    // Windows Installer handles any elevation or policy refusal itself.
    // Keep reboots and restart-manager shutdown/restart out of this invocation.
    let status = Command::new(executable)
        .arg("/i")
        .arg(package)
        .args([
            "/passive",
            "/norestart",
            "MSIRESTARTMANAGERCONTROL=Disable",
            "/l*v",
        ])
        .arg(log)
        .current_dir(stage)
        .status()
        .context("cannot start Windows Installer")?;
    let code = status
        .code()
        .context("Windows Installer did not provide an exit code")?;
    portable::write_record(stage, "msi-status", &code.to_le_bytes())?;
    match code {
        0 => {
            verify_installed(root, new, new_version)?;
            ensure!(
                installed_property(old, w!("State"))?.is_none(),
                "old MSI registration still exists after installation"
            );
            bundle::verify(root, new_version, true)?;
            Ok(())
        }
        1641 | 3010 => {
            bail!("Windows Installer requires a restart ({status}); completion is unverified");
        }
        _ => {
            let reason = rejection(code);
            verify_installed(root, old, old_version)
                .and_then(|()| bundle::verify(root, old_version, true).map(|_| ()))
                .with_context(|| format!("{reason} ({code}); previous MSI registration and bundle could not be revalidated"))?;
            bail!("{reason} ({code}); the verified previous bundle is still installed");
        }
    }
}

fn rejection(code: i32) -> &'static str {
    match code {
        1601 => "Windows Installer service is unavailable",
        1223 | 1602 => "Windows Installer was cancelled",
        1618 => "another Windows Installer transaction is active",
        1625 => "Windows Installer policy refused the transaction",
        _ => "Windows Installer failed",
    }
}

pub(super) fn require_restart_completed(stage: &Path) -> Result<()> {
    let begun = stage.join("msi-started");
    if begun.try_exists()? && exit_code(stage)?.is_none_or(|code| matches!(code, 1641 | 3010)) {
        ensure!(
            u64::from_le_bytes(fixed_record(&begun)?) != boot_time()?,
            "Windows Installer was interrupted or requires a restart; reboot before verifying the result"
        );
    }
    Ok(())
}

pub(super) fn exit_code(stage: &Path) -> Result<Option<i32>> {
    let path = stage.join("msi-status");
    if !path.try_exists()? {
        return Ok(None);
    }
    Ok(Some(i32::from_le_bytes(fixed_record(&path)?)))
}

fn fixed_record<const N: usize>(path: &Path) -> Result<[u8; N]> {
    use std::io::Read;
    let mut file = bundle::pin(path)?;
    ensure!(
        file.metadata()?.len() == N as u64,
        "invalid Windows Installer receipt"
    );
    let mut bytes = [0u8; N];
    file.read_exact(&mut bytes)?;
    Ok(bytes)
}

fn boot_time() -> Result<u64> {
    use std::{ffi::c_void, mem::size_of};
    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn NtQuerySystemInformation(
            class: u32,
            buffer: *mut c_void,
            size: u32,
            used: *mut u32,
        ) -> i32;
    }
    // SystemTimeOfDayInformation begins with immutable BootTime as a
    // LARGE_INTEGER. Keep an aligned buffer for the entire native reply.
    let mut data = [0u64; 6];
    let mut used = 0;
    let status = unsafe {
        NtQuerySystemInformation(
            3,
            data.as_mut_ptr().cast(),
            size_of::<[u64; 6]>() as u32,
            &mut used,
        )
    };
    ensure!(
        status >= 0 && used >= 8 && data[0] != 0,
        "could not verify Windows boot time"
    );
    Ok(data[0])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_restart_marker_blocks_verification_during_the_same_boot() -> Result<()> {
        let directory = tempfile::tempdir()?;
        assert!(require_restart_completed(directory.path()).is_ok());
        portable::write_record(directory.path(), "msi-started", &boot_time()?.to_le_bytes())?;
        assert!(require_restart_completed(directory.path()).is_err());
        portable::write_record(directory.path(), "msi-status", &3010i32.to_le_bytes())?;
        assert!(require_restart_completed(directory.path()).is_err());
        Ok(())
    }

    #[test]
    #[ignore = "requires exact WiX artifact; reads metadata without invoking installation"]
    fn inspect_exact_wix_artifact() -> Result<()> {
        let path =
            PathBuf::from(std::env::var_os("MCW_TEST_HYBRID_MSI").context("WiX fixture missing")?)
                .canonicalize()?;
        ensure!(
            valid_product_code(&inspect_candidate(&path, env!("CARGO_PKG_VERSION"))?),
            "candidate product code is invalid"
        );
        assert!(inspect_candidate(&path, "255.255.65535").is_err());
        assert!(
            inspect_candidate(&path.with_extension("absent"), env!("CARGO_PKG_VERSION")).is_err()
        );
        Ok(())
    }

    #[test]
    #[ignore = "installs an exact fixture on a disposable GitHub Windows runner only"]
    fn install_exact_wix_artifact_on_hosted_fixture() -> Result<()> {
        use crate::uninstall::windows::{RegisteredOwner, digest, registered_owner};
        use std::env;
        ensure!(
            env::var("GITHUB_ACTIONS")?.as_str() == "true"
                && env::var("RUNNER_ENVIRONMENT")?.as_str() == "github-hosted",
            "disposable GitHub-hosted Windows runner required"
        );
        let sandbox = PathBuf::from(env::var_os("RUNNER_TEMP").context("runner temp missing")?)
            .canonicalize()?;
        let artifact =
            PathBuf::from(env::var_os("MCW_TEST_HYBRID_MSI").context("MSI fixture missing")?)
                .canonicalize()?;
        let stage =
            PathBuf::from(env::var_os("MCW_TEST_MSI_PROOF").context("MSI proof path missing")?)
                .canonicalize()?;
        ensure!(
            artifact.starts_with(&sandbox) && stage.starts_with(&sandbox),
            "fixture must be under actual runner temp"
        );
        let mut package = bundle::pin(&artifact)?;
        ensure!(
            digest(&mut package)? == env::var("MCW_TEST_MSI_SHA256")?,
            "MSI fixture differs"
        );
        let root = portable::local_app_data()?
            .join("Programs")
            .join("MicCamWatch")
            .canonicalize()?;
        let Some(RegisteredOwner::Msi(old)) = registered_owner(&root)? else {
            bail!("expected exact per-user MSI baseline");
        };
        verify_installed(&root, &old, "0.21.0")?;
        let new = inspect_candidate(&artifact, env!("CARGO_PKG_VERSION"))?;
        install(
            &artifact,
            &stage,
            &root,
            &old,
            &new,
            "0.21.0",
            env!("CARGO_PKG_VERSION"),
        )?;
        bundle::verify_flutter_version(&root, env!("CARGO_PKG_VERSION"))?;
        Ok(())
    }
}
