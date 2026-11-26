use crate::frontends::desktop::package::{MANIFEST, verify_version};
use crate::uninstall::windows::{digest, regular_identity, safe_ancestors};
use anyhow::{Context, Result, ensure};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom},
    os::windows::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
};

pub(super) const LOCK: &str = ".mcw-install.lock";
const MAX_EXPANDED: u64 = 128 * 1024 * 1024;

fn expected() -> Result<BTreeSet<String>> {
    let layout: BTreeMap<String, Vec<String>> =
        serde_json::from_str(include_str!("../../installer/hybrid_layout.json"))?;
    Ok(layout["windows-x86_64"]
        .iter()
        .cloned()
        .chain([MANIFEST.into()])
        .collect())
}

/// Deny writes/renames while consuming a trusted input. A path to a junction,
/// symlink or hard link cannot stand in for an official manifest or payload.
pub(super) fn pin(path: &Path) -> Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .share_mode(1)
        .custom_flags(0x0020_0000)
        .open(path)
        .with_context(|| format!("cannot pin {}", path.display()))?;
    regular_identity(&file)?;
    Ok(file)
}

pub(super) fn verify(root: &Path, version: &str, runtime_lock: bool) -> Result<String> {
    safe_ancestors(root)?;
    let remaining = expected()?;
    let parents: BTreeSet<_> = remaining
        .iter()
        .flat_map(|name| Path::new(name).ancestors().skip(1))
        .filter(|path| !path.as_os_str().is_empty())
        .map(|path| root.join(path))
        .collect();
    let mut actual = BTreeSet::new();
    for directory in std::iter::once(root.to_owned()).chain(parents.iter().cloned()) {
        safe_ancestors(&directory)?;
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let path = entry.path();
            let relative = path
                .strip_prefix(root)?
                .to_str()
                .context("non-Unicode package path")?;
            let relative = relative.replace('\\', "/");
            let metadata = fs::symlink_metadata(&path)?;
            ensure!(
                metadata.file_attributes() & 0x400 == 0,
                "linked package entry: {relative}"
            );
            if metadata.is_dir() {
                ensure!(
                    parents.contains(&path),
                    "unexpected package directory: {relative}"
                );
            } else if runtime_lock && relative == LOCK {
                ensure!(
                    pin(&path)?.metadata()?.len() == 0,
                    "invalid native installation lock"
                );
            } else {
                ensure!(
                    metadata.is_file() && remaining.contains(&relative),
                    "unexpected package file: {relative}"
                );
                ensure!(actual.insert(relative), "duplicate package entry");
            }
        }
    }
    ensure!(actual == remaining, "incomplete hybrid package layout");
    let mut manifest = pin(&root.join(MANIFEST))?;
    let fingerprint = digest(&mut manifest)?;
    manifest.rewind()?;
    verify_version(&manifest, |name| pin(&root.join(name)), version)?;
    for name in [
        "mcw.exe",
        "mcw-tray.exe",
        "desktop/miccamwatch_desktop.exe",
        "desktop/flutter_windows.dll",
    ] {
        validate_x64_pe(pin(&root.join(name))?)
            .with_context(|| format!("invalid x64 binary: {name}"))?;
    }
    let mut aot = [0; 20];
    pin(&root.join("desktop/data/app.so"))?.read_exact(&mut aot)?;
    ensure!(
        &aot[..6] == b"\x7fELF\x02\x01" && aot[18..20] == [62, 0],
        "invalid x64 Flutter AOT ELF"
    );
    Ok(fingerprint)
}

fn validate_x64_pe(mut file: File) -> Result<()> {
    let mut dos = [0; 64];
    file.read_exact(&mut dos)?;
    ensure!(&dos[..2] == b"MZ", "invalid PE DOS header");
    let offset = u32::from_le_bytes(dos[60..64].try_into()?).into();
    ensure!(
        (64..=1_048_576).contains(&offset),
        "invalid PE header offset"
    );
    file.seek(SeekFrom::Start(offset))?;
    let mut header = [0; 26];
    file.read_exact(&mut header)?;
    ensure!(
        &header[..6] == b"PE\0\0\x64\x86" && header[24..26] == [11, 2],
        "expected x64 PE32+"
    );
    Ok(())
}

pub(super) fn verify_flutter_version(root: &Path, version: &str) -> Result<()> {
    use std::ffi::c_void;
    use windows::{
        Win32::Storage::FileSystem::VS_FIXEDFILEINFO,
        core::{BOOL, PCWSTR, w},
    };
    let library = super::system_library::SystemLibrary::load("version.dll")?;
    // These types mirror winver.h. Keep the selected System32 DLL resident
    // while reading version data from the pinned Flutter executable.
    let info_size: unsafe extern "system" fn(PCWSTR, *mut u32) -> u32 =
        unsafe { library.export(c"GetFileVersionInfoSizeW") }?;
    let get_info: unsafe extern "system" fn(PCWSTR, u32, u32, *mut c_void) -> BOOL =
        unsafe { library.export(c"GetFileVersionInfoW") }?;
    let query: unsafe extern "system" fn(
        *const c_void,
        PCWSTR,
        *mut *mut c_void,
        *mut u32,
    ) -> BOOL = unsafe { library.export(c"VerQueryValueW") }?;
    let path = root.join("desktop/miccamwatch_desktop.exe");
    let _pinned = pin(&path)?;
    let path = super::msi::wide(path.as_os_str());
    let length = unsafe { info_size(PCWSTR(path.as_ptr()), std::ptr::null_mut()) };
    ensure!(
        (1..=65_536).contains(&length),
        "Flutter runner version resource is missing or oversized"
    );
    let mut resource = vec![0u8; length as usize];
    unsafe {
        get_info(
            PCWSTR(path.as_ptr()),
            0,
            length,
            resource.as_mut_ptr().cast(),
        )
    }
    .ok()?;
    let mut value = std::ptr::null_mut();
    let mut size = 0;
    ensure!(
        unsafe { query(resource.as_ptr().cast(), w!("\\"), &mut value, &mut size) }.as_bool()
            && !value.is_null()
            && size as usize >= std::mem::size_of::<VS_FIXEDFILEINFO>(),
        "cannot read Flutter runner version"
    );
    let info = unsafe { value.cast::<VS_FIXEDFILEINFO>().read_unaligned() };
    let actual = format!(
        "{}.{}.{}",
        info.dwProductVersionMS >> 16,
        info.dwProductVersionMS & 0xffff,
        info.dwProductVersionLS >> 16
    );
    ensure!(
        info.dwSignature == 0xfeef04bd && actual == version,
        "Flutter runner product version {actual} differs from bundle {version}"
    );
    Ok(())
}

pub(super) fn extract(archive: File, root: &Path, version: &str) -> Result<String> {
    let mut zip = zip::ZipArchive::new(archive)?;
    let mut files = expected()?;
    ensure!(zip.len() == files.len(), "unexpected ZIP entry count");
    let mut total = 0u64;
    fs::create_dir(root)?;
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index)?;
        let name = std::str::from_utf8(entry.name_raw())?;
        ensure!(
            entry.is_file()
                && !entry.is_symlink()
                && entry
                    .unix_mode()
                    .is_none_or(|mode| matches!(mode & 0o170000, 0 | 0o100000))
                && files.remove(name),
            "unexpected ZIP entry: {name}"
        );
        let size = entry.size();
        total = total.checked_add(size).context("oversized ZIP")?;
        ensure!(
            size > 0 && total <= MAX_EXPANDED,
            "oversized or empty ZIP entry"
        );
        let output = root.join(name);
        fs::create_dir_all(output.parent().context("ZIP entry has no parent")?)?;
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(output)?;
        ensure!(
            std::io::copy(&mut entry.by_ref().take(size + 1), &mut file)? == size,
            "ZIP entry length disagrees with header"
        );
        file.sync_all()?;
    }
    ensure!(files.is_empty(), "incomplete ZIP payload");
    verify(root, version, false)
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    #[test]
    #[ignore = "requires the exact release Flutter ZIP; extracts files without launching the application"]
    fn exact_archive_and_flutter_version_are_read_without_launching_it() -> Result<()> {
        let zip = std::env::var_os("MCW_TEST_HYBRID_ZIP").context("ZIP fixture missing")?;
        let temp = tempfile::tempdir()?;
        let expand = temp.path().join("expanded");
        let version = env!("CARGO_PKG_VERSION");
        extract(File::open(zip)?, &expand, version)?;
        verify_flutter_version(&expand, version)?;
        assert!(verify_flutter_version(&expand, "255.255.65535").is_err());
        Ok(())
    }

    pub(crate) fn fixture(root: &Path, version: &str) -> Result<String> {
        fs::create_dir(root)?;
        let mut files = BTreeMap::new();
        for name in expected()? {
            if name == MANIFEST {
                continue;
            }
            let path = root.join(&name);
            fs::create_dir_all(path.parent().unwrap())?;
            let mut bytes = vec![0; 90];
            if name.ends_with(".exe") || name.ends_with(".dll") {
                bytes[..2].copy_from_slice(b"MZ");
                bytes[60] = 64;
                bytes[64..70].copy_from_slice(b"PE\0\0\x64\x86");
                bytes[88..90].copy_from_slice(&[11, 2]);
            } else if name.ends_with("/app.so") {
                bytes[..6].copy_from_slice(b"\x7fELF\x02\x01");
                bytes[18] = 62;
            } else {
                bytes = version.as_bytes().to_vec();
            }
            fs::write(&path, bytes)?;
            files.insert(name, digest(&mut File::open(path)?)?);
        }
        let manifest = serde_json::json!({
            "schema": 1, "version": version, "ipc": 10, "target": "windows-x86_64",
            "revision": "0011223344556677889900112233445566778899", "files": files
        });
        fs::write(root.join(MANIFEST), serde_json::to_vec(&manifest)?)?;
        verify(root, version, false)
    }

    #[test]
    fn archive_rejects_extra_duplicate_traversal_and_symlink_entries() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let root = temp.path().join("fixture");
        let hash = fixture(&root, "0.22.0")?;
        let names = expected()?;
        for (index, invalid) in [
            None,
            Some("../escape"),
            Some("mcw.exe"),
            Some("link"),
            Some("unknown"),
        ]
        .into_iter()
        .enumerate()
        {
            let archive = temp.path().join(format!("package-{index}.zip"));
            let mut writer = zip::ZipWriter::new(File::create(&archive)?);
            for name in &names {
                if invalid.is_some() && name == "LICENSE" {
                    continue;
                }
                writer.start_file(name, SimpleFileOptions::default())?;
                writer.write_all(&fs::read(root.join(name))?)?;
            }
            if let Some(invalid) = invalid {
                if invalid == "link" {
                    writer.add_symlink("LICENSE", "mcw.exe", SimpleFileOptions::default())?;
                } else {
                    let name = if invalid == "mcw.exe" {
                        "fake000"
                    } else {
                        invalid
                    };
                    writer.start_file(name, SimpleFileOptions::default())?;
                    writer.write_all(b"bad entry")?;
                }
            }
            writer.finish()?;
            if invalid == Some("mcw.exe") {
                // The writer rejects duplicate names. Inject equal-length
                // local and central directory names to exercise the reader.
                let mut bytes = fs::read(&archive)?;
                let offsets: Vec<_> = bytes
                    .windows(7)
                    .enumerate()
                    .filter_map(|(index, name)| (name == b"fake000").then_some(index))
                    .collect();
                assert_eq!(offsets.len(), 2);
                for index in offsets {
                    bytes[index..index + 7].copy_from_slice(b"mcw.exe");
                }
                fs::write(&archive, bytes)?;
            }
            let actual = extract(
                File::open(&archive)?,
                &temp.path().join(format!("expanded-{index}")),
                "0.22.0",
            );
            if invalid.is_some() {
                assert!(actual.is_err(), "{invalid:?}");
            } else {
                assert_eq!(actual?, hash);
            }
            assert!(!temp.path().join("escape").exists());
        }
        Ok(())
    }

    #[test]
    fn installed_tree_rejects_changed_resource_missing_runner_and_wrong_version() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let root = temp.path().join("fixture");
        fixture(&root, "0.22.0")?;
        assert!(verify(&root, "0.22.1", false).is_err());
        let runner = root.join("desktop/miccamwatch_desktop.exe");
        let saved = fs::read(&runner)?;
        fs::remove_file(&runner)?;
        assert!(verify(&root, "0.22.0", false).is_err());
        fs::write(runner, saved)?;
        fs::write(root.join("desktop/data/icudtl.dat"), b"changed resource")?;
        assert!(verify(&root, "0.22.0", false).is_err());
        Ok(())
    }
}
