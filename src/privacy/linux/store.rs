use super::{HELPER, VERSION, usb::DeviceIdentity};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::{
        fd::AsRawFd,
        unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    },
    path::{Component, Path},
    sync::atomic::{AtomicU64, Ordering},
};

const DIRECTORY: &str = "/var/lib/miccamwatch";
const JOURNAL: &str = "journal.json";
const CACHE: &str = "status.json";
const LOCK: &str = "operation.lock";
pub(super) const MAX_BYTES: u64 = 1024 * 1024;
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Intent {
    Block,
    Restore,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Entry {
    pub uid: u32,
    pub intent: Intent,
    pub device: DeviceIdentity,
    /// Exact complete original video binding set; partly-bound originals are refused.
    pub original_bound: Vec<String>,
}

#[derive(Clone)]
pub(super) struct Journal {
    pub format: u32,
    pub entries: Vec<Entry>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope<E> {
    format: u32,
    digest: String,
    entries: E,
}

struct HashWriter(Sha256);
impl Write for HashWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn entries_digest(entries: &[Entry]) -> Result<String> {
    let mut writer = HashWriter(Sha256::new());
    serde_json::to_writer(&mut writer, entries)?;
    Ok(format!("{:x}", writer.0.finalize()))
}

fn decode_journal(bytes: &[u8]) -> Result<Journal> {
    let envelope: Envelope<Vec<Entry>> =
        serde_json::from_slice(bytes).context("damaged camera journal; refusing mutation")?;
    if envelope.format != 1 || envelope.digest != entries_digest(&envelope.entries)? {
        bail!("damaged camera journal format/integrity; refusing mutation and retaining evidence");
    }
    validate_entries(&envelope.entries)?;
    Ok(Journal {
        format: 1,
        entries: envelope.entries,
    })
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Cache {
    pub format: u32,
    pub version: String,
    pub boot_id: String,
    pub digest: String,
    pub entries: Vec<Entry>,
}

pub(super) struct Store {
    directory: File,
    _lock: File,
}

pub(super) fn safe_directory(path: &Path) -> Result<()> {
    if !path.is_absolute() {
        bail!("protected path is not absolute");
    }
    let mut current = std::path::PathBuf::from("/");
    check_directory(&current)?;
    for component in path.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(name) => {
                current.push(name);
                check_directory(&current)?;
            }
            _ => bail!("invalid protected path component"),
        }
    }
    Ok(())
}

fn check_directory(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("cannot inspect protected directory {}", path.display()))?;
    if !metadata.is_dir() || metadata.uid() != 0 || metadata.mode() & 0o022 != 0 {
        bail!(
            "unsafe protected directory {}; require root ownership, no symlinks or group/other write",
            path.display()
        );
    }
    Ok(())
}

pub(super) fn trusted_executable(path: &str) -> Result<()> {
    let path = Path::new(path);
    safe_directory(path.parent().context("executable has no parent")?)?;
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.uid() != 0
        || metadata.nlink() != 1
        || metadata.mode() & 0o022 != 0
        || metadata.mode() & 0o111 == 0
        || (path == Path::new(HELPER) && metadata.mode() & 0o6000 != 0)
    {
        bail!("unsafe privileged executable {}", path.display());
    }
    Ok(())
}

fn check_file(file: &File, mode: u32) -> Result<()> {
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.uid() != 0
        || metadata.nlink() != 1
        || metadata.mode() & 0o7777 != mode
        || metadata.len() > MAX_BYTES
    {
        bail!("unsafe root camera state file (ownership, mode, links or size)");
    }
    Ok(())
}

fn read_file(name: &str, mode: u32) -> Result<Option<Vec<u8>>> {
    let path = Path::new(DIRECTORY).join(name);
    let mut file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
        .open(&path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| format!("cannot safely open {}", path.display()));
        }
    };
    check_file(&file, mode)?;
    let mut bytes = Vec::new();
    (&mut file).take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_BYTES {
        bail!("oversized root camera state file");
    }
    Ok(Some(bytes))
}

pub(super) fn validate_entries(entries: &[Entry]) -> Result<()> {
    if entries.len() > 256 {
        bail!("too many owned camera records");
    }
    for (index, entry) in entries.iter().enumerate() {
        entry.device.validate()?;
        if entry.original_bound.is_empty()
            || entry.original_bound.len() > entry.device.interfaces.len()
            || entry.original_bound.iter().enumerate().any(|(i, name)| {
                !entry
                    .device
                    .interfaces
                    .iter()
                    .any(|interface| &interface.name == name)
                    || entry.original_bound[..i].contains(name)
            })
            || entries[..index]
                .iter()
                .any(|other| other.device.topology == entry.device.topology)
        {
            bail!("damaged camera journal ownership/identity; evidence was preserved");
        }
    }
    Ok(())
}

pub(super) fn read_cache() -> Result<Option<Cache>> {
    match fs::symlink_metadata(DIRECTORY) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            safe_directory(Path::new("/var/lib"))?;
            return Ok(None);
        }
        Err(error) => return Err(error.into()),
        Ok(_) => safe_directory(Path::new(DIRECTORY))?,
    }
    let path = Path::new(DIRECTORY).join(CACHE);
    let mut file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
        .open(&path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).context("cannot safely open camera status cache"),
    };
    check_file(&file, 0o644)?;
    let opened = file.metadata()?;
    let mut bytes = Vec::new();
    (&mut file).take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_BYTES {
        bail!("oversized camera status cache");
    }
    let cache: Cache = serde_json::from_slice(&bytes).context("damaged camera status cache")?;
    if cache.format != 1
        || cache.version != VERSION
        || !super::usb::valid_boot_id(&cache.boot_id)
        || cache.digest != entries_digest(&cache.entries)?
    {
        bail!(
            "camera status cache version/identity/integrity mismatch; administrator recovery is required"
        );
    }
    validate_entries(&cache.entries)?;
    // An observer may have opened the previous snapshot just before a privileged
    // ownership transition invalidated it. Never publish that retired evidence.
    if !cache_still_published(&file, &path, &opened)? {
        return Ok(None);
    }
    check_file(&file, 0o644)?;
    Ok(Some(cache))
}

fn cache_still_published(file: &File, path: &Path, opened: &fs::Metadata) -> Result<bool> {
    let current = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    Ok(file.metadata()?.nlink() == 1
        && current.dev() == opened.dev()
        && current.ino() == opened.ino())
}

impl Store {
    pub fn open() -> Result<Self> {
        safe_directory(Path::new("/var/lib"))?;
        match fs::symlink_metadata(DIRECTORY) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::DirBuilder::new().mode(0o755).create(DIRECTORY)?;
                fs::set_permissions(DIRECTORY, fs::Permissions::from_mode(0o755))?;
                File::open("/var/lib")?.sync_all()?;
            }
            Err(error) => return Err(error.into()),
            Ok(_) => {}
        }
        safe_directory(Path::new(DIRECTORY))?;
        let directory = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(DIRECTORY)?;
        let lock_path = Path::new(DIRECTORY).join(LOCK);
        let lock = match OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&lock_path)
        {
            Ok(file) => {
                file.set_permissions(fs::Permissions::from_mode(0o600))?;
                file.sync_all()?;
                directory.sync_all()?;
                file
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => OpenOptions::new()
                .read(true)
                .write(true)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
                .open(&lock_path)?,
            Err(error) => return Err(error.into()),
        };
        check_file(&lock, 0o600)?;
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            bail!(
                "another camera operation/administrative install holds the global lock; retry later"
            );
        }
        // Reject an unsafe destination before any kernel operation, not afterwards.
        read_file(JOURNAL, 0o600)?;
        read_file(CACHE, 0o644)?;
        Ok(Self {
            directory,
            _lock: lock,
        })
    }

    pub fn load(&self) -> Result<Journal> {
        let bytes = read_file(JOURNAL, 0o600)?;
        let journal = match bytes {
            Some(bytes) => decode_journal(&bytes)?,
            None => {
                if read_cache()?.is_some_and(|cache| !cache.entries.is_empty()) {
                    bail!(
                        "camera restoration journal is missing but ownership evidence remains; refusing mutation"
                    );
                }
                Journal {
                    format: 1,
                    entries: Vec::new(),
                }
            }
        };
        if journal.format != 1 {
            bail!("unsupported camera journal format; refusing mutation");
        }
        validate_entries(&journal.entries)?;
        Ok(journal)
    }

    fn atomic<T: Serialize>(&self, name: &str, value: &T, mode: u32) -> Result<()> {
        let bytes = serde_json::to_vec(value)?;
        if bytes.len() as u64 > MAX_BYTES {
            bail!("camera journal exceeds its fixed size limit");
        }
        read_file(name, mode)?;
        let temp = Path::new(DIRECTORY).join(format!(
            ".{}-{}-{}",
            name,
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let result = (|| -> Result<()> {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(mode)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open(&temp)?;
            file.set_permissions(fs::Permissions::from_mode(mode))?;
            check_file(&file, mode)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            fs::rename(&temp, Path::new(DIRECTORY).join(name))?;
            self.directory.sync_all()?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result
    }

    pub fn save(&self, journal: &Journal) -> Result<()> {
        validate_entries(&journal.entries)?;
        // Retire readable ownership durably BEFORE committing any journal change.
        // A crash/cache failure can hide owned evidence, never resurrect retired
        // ownership. Observation without a cache reports uncertainty, not Blocked.
        read_file(CACHE, 0o644)?;
        match fs::remove_file(Path::new(DIRECTORY).join(CACHE)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).context("cannot invalidate readable camera ownership evidence");
            }
        }
        self.directory.sync_all()?;
        self.atomic(
            JOURNAL,
            &Envelope {
                format: journal.format,
                digest: entries_digest(&journal.entries)?,
                entries: journal.entries.as_slice(),
            },
            0o600,
        )?;
        // Persist ownership immediately, before unbind. Readers independently inspect sysfs.
        self.cache(journal)
    }

    pub fn cache(&self, journal: &Journal) -> Result<()> {
        #[derive(Serialize)]
        struct CacheView<'a> {
            format: u32,
            version: &'static str,
            boot_id: String,
            digest: String,
            entries: &'a [Entry],
        }
        self.atomic(
            CACHE,
            &CacheView {
                format: 1,
                version: VERSION,
                boot_id: super::usb::boot_id()?,
                digest: entries_digest(&journal.entries)?,
                entries: &journal.entries,
            },
            0o644,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry() -> Entry {
        let device = super::super::usb::test_device();
        Entry {
            uid: 1000,
            intent: Intent::Block,
            device: device.identity,
            original_bound: device.bindings.keys().cloned().collect(),
        }
    }

    #[test]
    fn damaged_parseable_original_binding_evidence_is_rejected() {
        let entries = vec![entry()];
        let mut envelope = Envelope {
            format: 1,
            digest: entries_digest(&entries).unwrap(),
            entries,
        };
        let encoded = serde_json::to_vec(&envelope).unwrap();
        assert_eq!(
            decode_journal(&encoded).unwrap().entries[0]
                .original_bound
                .len(),
            2
        );
        envelope.entries[0].original_bound.pop();
        assert!(decode_journal(&serde_json::to_vec(&envelope).unwrap()).is_err());
    }

    #[test]
    fn duplicate_or_foreign_original_interfaces_never_authorize_restoration() {
        let mut original = entry();
        original
            .original_bound
            .push(original.original_bound[0].clone());
        assert!(validate_entries(&[original]).is_err());
        let mut original = entry();
        original.original_bound[0] = "1-9:1.0".to_owned();
        assert!(validate_entries(&[original]).is_err());
    }

    #[test]
    fn two_callers_cannot_hold_conflicting_original_states_for_one_device() {
        let first = entry();
        let mut second = first.clone();
        second.uid = 1001;
        assert!(validate_entries(&[first, second]).is_err());
    }

    #[test]
    fn empty_restoration_record_has_installer_verifiable_integrity() {
        assert_eq!(
            entries_digest(&[]).unwrap(),
            "4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
        );
    }

    #[test]
    fn opened_ownership_snapshot_is_rejected_after_atomic_replacement_or_invalidation() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("status.json");
        fs::write(&path, b"old ownership").unwrap();
        let file = File::open(&path).unwrap();
        let opened = file.metadata().unwrap();
        assert!(cache_still_published(&file, &path, &opened).unwrap());
        let replacement = directory.path().join("replacement");
        fs::write(&replacement, b"new ownership").unwrap();
        fs::rename(&replacement, &path).unwrap();
        assert!(!cache_still_published(&file, &path, &opened).unwrap());
        let current = File::open(&path).unwrap();
        let current_metadata = current.metadata().unwrap();
        fs::remove_file(&path).unwrap();
        assert!(!cache_still_published(&current, &path, &current_metadata).unwrap());
    }
}
