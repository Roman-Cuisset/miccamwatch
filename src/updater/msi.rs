use super::{
    bundle, portable,
    system_library::{SystemLibrary, system_directory},
};
use crate::uninstall::windows::valid_product_code;
use anyhow::{Context, Result, bail, ensure};
use std::{ffi::OsStr, os::windows::ffi::OsStrExt, path::Path, process::Command};
use windows::{
    Win32::{
        Foundation::FILETIME,
        System::ApplicationInstallationAndServicing::{
            MSIHANDLE, MSIINSTALLCONTEXT, MSIINSTALLCONTEXT_MACHINE, MSIINSTALLCONTEXT_USERMANAGED,
            MSIINSTALLCONTEXT_USERUNMANAGED,
        },
    },
    core::{PCWSTR, PWSTR, w},
};

const UPGRADE_CODE: &str = "{8FC9317B-4517-4E83-9F52-CF3257584BF6}";
pub(super) const LEGACY_CODE: &str = "{86CDFED5-BA07-4C91-8BC7-8A856198DC33}";

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Registration {
    Absent,
    AdvertisedWithoutIndex { package_code: String },
    PresentWithoutIndex,
    Complete,
}

struct Installer {
    _library: SystemLibrary,
    close: unsafe extern "system" fn(MSIHANDLE) -> u32,
    database: unsafe extern "system" fn(PCWSTR, PCWSTR, *mut MSIHANDLE) -> u32,
    view: unsafe extern "system" fn(MSIHANDLE, PCWSTR, *mut MSIHANDLE) -> u32,
    execute: unsafe extern "system" fn(MSIHANDLE, MSIHANDLE) -> u32,
    fetch: unsafe extern "system" fn(MSIHANDLE, *mut MSIHANDLE) -> u32,
    record_string: unsafe extern "system" fn(MSIHANDLE, u32, PWSTR, *mut u32) -> u32,
    summary: unsafe extern "system" fn(MSIHANDLE, PCWSTR, u32, *mut MSIHANDLE) -> u32,
    summary_property: unsafe extern "system" fn(
        MSIHANDLE,
        u32,
        *mut u32,
        *mut i32,
        *mut FILETIME,
        PWSTR,
        *mut u32,
    ) -> u32,
    product_info: unsafe extern "system" fn(
        PCWSTR,
        PCWSTR,
        MSIINSTALLCONTEXT,
        PCWSTR,
        PWSTR,
        *mut u32,
    ) -> u32,
    enum_related: unsafe extern "system" fn(PCWSTR, u32, u32, PWSTR) -> u32,
}

impl Installer {
    fn load() -> Result<Self> {
        let library = SystemLibrary::load("msi.dll")?;
        // Match the msi.h ABI above and retain the System32 module until
        // all its database, record and summary handles have been closed.
        Ok(unsafe {
            Self {
                close: library.export(c"MsiCloseHandle")?,
                database: library.export(c"MsiOpenDatabaseW")?,
                view: library.export(c"MsiDatabaseOpenViewW")?,
                execute: library.export(c"MsiViewExecute")?,
                fetch: library.export(c"MsiViewFetch")?,
                record_string: library.export(c"MsiRecordGetStringW")?,
                summary: library.export(c"MsiGetSummaryInformationW")?,
                summary_property: library.export(c"MsiSummaryInfoGetPropertyW")?,
                product_info: library.export(c"MsiGetProductInfoExW")?,
                enum_related: library.export(c"MsiEnumRelatedProductsW")?,
                _library: library,
            }
        })
    }
}

struct Handle<'a>(MSIHANDLE, &'a Installer);

impl Drop for Handle<'_> {
    fn drop(&mut self) {
        if self.0.0 != 0 {
            unsafe { (self.1.close)(self.0) };
        }
    }
}

pub(super) fn wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain([0]).collect()
}

// Windows Installer still uses Win32 DOS paths in its source list. The
// updater canonicalizes paths for identity checks, then supplies the same
// local path without the extended-length prefix to Windows Installer.
pub(super) fn dos_path(path: &Path) -> Result<&Path> {
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

fn database<'a>(installer: &'a Installer, path: &Path) -> Result<Handle<'a>> {
    let mut result = Handle(MSIHANDLE(0), installer);
    checked(
        unsafe {
            (installer.database)(
                PCWSTR(wide(dos_path(path)?.as_os_str()).as_ptr()),
                PCWSTR::null(),
                &mut result.0,
            )
        },
        "open database",
    )?;
    Ok(result)
}

fn first_string(db: &Handle<'_>, query: &str) -> Result<Option<String>> {
    let installer = db.1;
    let mut view = Handle(MSIHANDLE(0), installer);
    checked(
        unsafe { (installer.view)(db.0, PCWSTR(wide(OsStr::new(query)).as_ptr()), &mut view.0) },
        "open property query",
    )?;
    checked(
        unsafe { (installer.execute)(view.0, MSIHANDLE(0)) },
        "execute property query",
    )?;
    let mut row = Handle(MSIHANDLE(0), installer);
    let status = unsafe { (installer.fetch)(view.0, &mut row.0) };
    if status == 259 {
        return Ok(None);
    }
    checked(status, "fetch property")?;
    let mut value = [0u16; 512];
    let mut length = value.len() as u32;
    checked(
        unsafe { (installer.record_string)(row.0, 1, PWSTR(value.as_mut_ptr()), &mut length) },
        "read property",
    )?;
    Ok(Some(String::from_utf16(&value[..length as usize])?))
}

fn property(db: &Handle<'_>, name: &str) -> Result<Option<String>> {
    first_string(
        db,
        &format!("SELECT `Value` FROM `Property` WHERE `Property`='{name}'"),
    )
}

pub(super) fn inspect_candidate(path: &Path, version: &str) -> Result<String> {
    inspect_package(path, version, false)
}

pub(super) fn inspect_recovery_package(path: &Path, version: &str) -> Result<String> {
    inspect_package(path, version, version == "0.21.0")
}

fn inspect_package(path: &Path, version: &str, legacy: bool) -> Result<String> {
    let installer = Installer::load()?;
    let db = database(&installer, path)?;
    let template = summary_string(&db, 7)?;
    ensure!(
        template.starts_with("x64;") || legacy && template.starts_with("Intel;"),
        "unexpected MSI architecture"
    );
    ensure!(
        property(&db, "ProductName")?.as_deref() == Some("MicCamWatch")
            && property(&db, "Manufacturer")?.as_deref() == Some("Roman Cuisset")
            && property(&db, "UpgradeCode")?
                .is_some_and(|value| value.eq_ignore_ascii_case(UPGRADE_CODE))
            && property(&db, "ProductVersion")?.as_deref() == Some(version)
            && (legacy
                || property(&db, "MCW_BUNDLE_IPC")?.as_deref()
                    == Some(&crate::frontends::desktop::PROTOCOL_VERSION.to_string()))
            && property(&db, "ALLUSERS")?.is_none_or(|value| value.is_empty()),
        "candidate MSI has incompatible product identity, version, scope or IPC"
    );
    let product = property(&db, "ProductCode")?.context("missing MSI ProductCode")?;
    ensure!(valid_product_code(&product), "invalid MSI ProductCode");
    ensure!(
        !legacy || product.eq_ignore_ascii_case(LEGACY_CODE),
        "unexpected legacy ProductCode"
    );
    let mut media = Handle(MSIHANDLE(0), &installer);
    checked(
        unsafe { (installer.view)(db.0, w!("SELECT `Cabinet` FROM `Media`"), &mut media.0) },
        "open cabinet query",
    )?;
    checked(
        unsafe { (installer.execute)(media.0, MSIHANDLE(0)) },
        "read cabinet table",
    )?;
    let mut count = 0;
    loop {
        let mut row = Handle(MSIHANDLE(0), &installer);
        let status = unsafe { (installer.fetch)(media.0, &mut row.0) };
        if status == 259 {
            break;
        }
        checked(status, "fetch cabinet")?;
        let mut value = [0u16; 256];
        let mut size = value.len() as u32;
        checked(
            unsafe { (installer.record_string)(row.0, 1, PWSTR(value.as_mut_ptr()), &mut size) },
            "read cabinet name",
        )?;
        ensure!(
            size > 1 && value[0] == u16::from(b'#'),
            "MSI needs external media"
        );
        let cabinet = String::from_utf16(&value[1..size as usize])?;
        ensure!(
            cabinet
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
                && first_string(
                    &db,
                    &format!("SELECT `Name` FROM `_Streams` WHERE `Name`='{cabinet}'")
                )?
                .as_deref()
                    == Some(cabinet.as_str()),
            "MSI embedded cabinet is missing or has an unsupported name"
        );
        count += 1;
    }
    ensure!(count > 0, "MSI lacks an embedded cabinet");
    Ok(product)
}

fn summary_string(db: &Handle<'_>, property: u32) -> Result<String> {
    let installer = db.1;
    let mut summary = Handle(MSIHANDLE(0), installer);
    checked(
        unsafe { (installer.summary)(db.0, PCWSTR::null(), 0, &mut summary.0) },
        "open MSI summary",
    )?;
    let mut value = [0u16; 256];
    let mut length = value.len() as u32;
    let mut value_type = 0;
    checked(
        unsafe {
            (installer.summary_property)(
                summary.0,
                property,
                &mut value_type,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                PWSTR(value.as_mut_ptr()),
                &mut length,
            )
        },
        "read MSI summary property",
    )?;
    Ok(String::from_utf16(&value[..length as usize])?)
}

pub(super) fn package_code(path: &Path) -> Result<String> {
    let installer = Installer::load()?;
    let db = database(&installer, path)?;
    let value = summary_string(&db, 9)?;
    ensure!(valid_product_code(&value), "invalid MSI PackageCode");
    Ok(value)
}

fn installed_property(
    installer: &Installer,
    code: &str,
    property: PCWSTR,
) -> Result<Option<String>> {
    product_property(installer, code, MSIINSTALLCONTEXT_USERUNMANAGED, property)
}

fn product_property(
    installer: &Installer,
    code: &str,
    context: MSIINSTALLCONTEXT,
    property: PCWSTR,
) -> Result<Option<String>> {
    let mut buffer = [0u16; 32768];
    let mut length = buffer.len() as u32;
    let status = unsafe {
        (installer.product_info)(
            PCWSTR(wide(OsStr::new(code)).as_ptr()),
            PCWSTR::null(),
            context,
            property,
            PWSTR(buffer.as_mut_ptr()),
            &mut length,
        )
    };
    if status == 1605 {
        return Ok(None);
    }
    checked(status, "read installed per-user product")?;
    Ok(Some(String::from_utf16(&buffer[..length as usize])?))
}

pub(super) fn verify_installed(root: &Path, code: &str, version: &str) -> Result<()> {
    ensure!(
        registration(root, code, version)? == Registration::Complete,
        "MSI ProductCode or UpgradeCode index does not match this per-user product"
    );
    Ok(())
}

fn require_other_contexts_absent(installer: &Installer, code: &str) -> Result<()> {
    for context in [MSIINSTALLCONTEXT_MACHINE, MSIINSTALLCONTEXT_USERMANAGED] {
        ensure!(
            product_property(installer, code, context, w!("State"))?.is_none(),
            "a matching ProductCode exists in a different MSI context"
        );
    }
    Ok(())
}

fn related_product(installer: &Installer) -> Result<Option<String>> {
    let family = wide(OsStr::new(UPGRADE_CODE));
    let mut value = [0u16; 39];
    let code = unsafe {
        (installer.enum_related)(PCWSTR(family.as_ptr()), 0, 0, PWSTR(value.as_mut_ptr()))
    };
    if code == 259 {
        return Ok(None);
    }
    checked(code, "enumerate products by UpgradeCode")?;
    let product = String::from_utf16(&value[..38])?;
    ensure!(
        valid_product_code(&product) && value[38] == 0,
        "invalid related ProductCode"
    );
    let next = unsafe {
        (installer.enum_related)(PCWSTR(family.as_ptr()), 0, 1, PWSTR(value.as_mut_ptr()))
    };
    ensure!(
        next == 259,
        "multiple related products or incomplete MSI index enumeration"
    );
    Ok(Some(product))
}

pub(super) fn registration(root: &Path, code: &str, version: &str) -> Result<Registration> {
    let installer = Installer::load()?;
    ensure!(valid_product_code(code), "invalid installed ProductCode");
    require_other_contexts_absent(&installer, code)?;
    let related = related_product(&installer)?;
    let state = installed_property(&installer, code, w!("State"))?;
    if state.is_none() {
        ensure!(
            !related.is_some_and(|product| product.eq_ignore_ascii_case(code)),
            "UpgradeCode finds a ProductCode absent from its installation context"
        );
        return Ok(Registration::Absent);
    }
    if state.as_deref() == Some("1") {
        ensure!(
            related.is_none(),
            "advertised MSI is indexed; recovery context is ambiguous"
        );
        let package_code = installed_property(&installer, code, w!("PackageCode"))?
            .context("advertised MSI has no PackageCode")?;
        ensure!(
            valid_product_code(&package_code),
            "invalid advertised MSI PackageCode"
        );
        return Ok(Registration::AdvertisedWithoutIndex { package_code });
    }
    ensure!(
        state.as_deref() == Some("5")
            && installed_property(&installer, code, w!("VersionString"))?.as_deref()
                == Some(version)
            && installed_property(&installer, code, w!("InstallLocation"))?.is_some_and(|value| {
                !value.is_empty()
                    && Path::new(&value)
                        .canonicalize()
                        .is_ok_and(|path| path == root)
            }),
        "MSI registration does not match this installed per-user product"
    );
    let cache = installed_property(&installer, code, w!("LocalPackage"))?
        .context("MSI package cache missing")?;
    let cache = Path::new(&cache);
    let system = system_directory()?;
    let installer_cache = system
        .parent()
        .context("missing Windows directory")?
        .join("Installer");
    ensure!(
        cache.parent().is_some_and(|parent| parent
            .as_os_str()
            .eq_ignore_ascii_case(installer_cache.as_os_str()))
            && cache
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("msi")),
        "MSI cache is outside the Windows Installer directory"
    );
    crate::uninstall::windows::safe_ancestors(&installer_cache)?;
    let _cache_file = bundle::pin(cache)?;
    let db = database(&installer, cache)?;
    ensure!(
        property(&db, "UpgradeCode")?.is_some_and(|value| value.eq_ignore_ascii_case(UPGRADE_CODE))
            && property(&db, "ProductCode")?.is_some_and(|value| value.eq_ignore_ascii_case(code))
            && property(&db, "ProductName")?.as_deref() == Some("MicCamWatch")
            && property(&db, "ProductVersion")?.as_deref() == Some(version)
            && property(&db, "Manufacturer")?.as_deref() == Some("Roman Cuisset"),
        "cached MSI does not belong to the official MicCamWatch upgrade family"
    );
    match related {
        Some(found) if found.eq_ignore_ascii_case(code) => Ok(Registration::Complete),
        None => Ok(Registration::PresentWithoutIndex),
        Some(_) => bail!("UpgradeCode resolves a different installed product"),
    }
}

pub(super) fn require_new_product(code: &str) -> Result<()> {
    let installer = Installer::load()?;
    ensure!(valid_product_code(code), "invalid candidate ProductCode");
    require_other_contexts_absent(&installer, code)?;
    ensure!(
        installed_property(&installer, code, w!("State"))?.is_none()
            && !related_product(&installer)?
                .is_some_and(|product| product.eq_ignore_ascii_case(code)),
        "the candidate ProductCode already exists in this Windows Installer context"
    );
    Ok(())
}

pub(super) fn require_family_owner(code: &str) -> Result<()> {
    ensure!(
        related_product(&Installer::load()?)?.is_none_or(|found| found.eq_ignore_ascii_case(code)),
        "another product is discoverable in the MicCamWatch MSI family"
    );
    Ok(())
}

pub(super) fn require_idle() -> Result<()> {
    use windows::Win32::{
        Foundation::{CloseHandle, ERROR_FILE_NOT_FOUND, WAIT_OBJECT_0},
        System::Threading::{
            OpenMutexW, ReleaseMutex, SYNCHRONIZATION_ACCESS_RIGHTS, WaitForSingleObject,
        },
    };
    for name in [w!("Global\\_MSIExecute"), w!("_MSIExecute")] {
        let handle = match unsafe {
            OpenMutexW(SYNCHRONIZATION_ACCESS_RIGHTS(0x0010_0001), false, name)
        } {
            Ok(handle) => handle,
            Err(error) if error.code() == ERROR_FILE_NOT_FOUND.to_hresult() => continue,
            Err(error) => return Err(error).context("cannot verify Windows Installer activity"),
        };
        let wait = unsafe { WaitForSingleObject(handle, 0) };
        let released = if matches!(wait.0, 0 | 0x80) {
            unsafe { ReleaseMutex(handle) }
        } else {
            Ok(())
        };
        unsafe { CloseHandle(handle) }?;
        released?;
        ensure!(
            wait == WAIT_OBJECT_0,
            "Windows Installer transaction or ambiguous mutex state"
        );
    }
    Ok(())
}

pub(super) fn require_space(directory: &Path, bytes: u64) -> Result<()> {
    use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let system = system_directory()?;
    for directory in [directory, system.as_path()] {
        let mut available = 0;
        unsafe {
            GetDiskFreeSpaceExW(
                PCWSTR(wide(directory.as_os_str()).as_ptr()),
                Some(&mut available),
                None,
                None,
            )
        }?;
        ensure!(
            available >= bytes,
            "insufficient space on an installation or Windows Installer volume"
        );
    }
    Ok(())
}

pub(super) fn run_recovery(
    step: &Path,
    operation: &str,
    target: &OsStr,
    interactive: bool,
) -> Result<()> {
    require_idle()?;
    let arguments = if interactive { &[][..] } else { &["/qn"][..] };
    let code = run_installer(step, operation, target, arguments)?;
    ensure!(
        code == 0,
        "{} ({code}); inspect the recovery receipt before continuing",
        rejection(code)
    );
    Ok(())
}

fn run_installer(stage: &Path, operation: &str, target: &OsStr, ui: &[&str]) -> Result<i32> {
    let executable = system_directory()?.join("msiexec.exe");
    let log = stage.join("msi.log");
    let log = dos_path(&log)?;
    portable::write_record(stage, "msi-started", &boot_time()?.to_le_bytes())?;
    let status = Command::new(executable)
        .arg(operation)
        .arg(target)
        .args(ui)
        .args(["/norestart", "MSIRESTARTMANAGERCONTROL=Disable", "/l*v"])
        .arg(log)
        .current_dir(stage)
        .status()
        .context("cannot start Windows Installer")?;
    let code = status
        .code()
        .context("Windows Installer did not provide an exit code")?;
    portable::write_record(stage, "msi-status", &code.to_le_bytes())?;
    Ok(code)
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
    let package = dos_path(path)?;
    // Windows Installer handles any elevation or policy refusal itself.
    // Keep reboots and restart-manager shutdown/restart out of this invocation.
    require_idle()?;
    let code = run_installer(stage, "/i", package.as_os_str(), &["/passive"])?;
    match code {
        0 => {
            verify_installed(root, new, new_version)?;
            require_new_product(old)
                .context("old MSI registration still exists after installation")?;
            bundle::verify(root, new_version, true)?;
            Ok(())
        }
        1641 | 3010 => {
            bail!("Windows Installer requires a restart ({code}); completion is unverified");
        }
        _ => {
            let reason = rejection(code);
            require_new_product(new).and_then(|()| verify_installed(root, old, old_version))
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
        1641 | 3010 => "Windows Installer requires a restart",
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
    use std::path::PathBuf;

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
        let qualification = portable::local_app_data()?
            .join("MicCamWatchQualification")
            .join("flutter-b5-ci");
        ensure!(
            [&sandbox, &qualification]
                .iter()
                .any(|directory| artifact.starts_with(directory) && stage.starts_with(directory)),
            "fixture must be under the exact hosted qualification directory"
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
