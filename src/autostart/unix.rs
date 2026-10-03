use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
#[cfg(target_os = "macos")]
use std::process::{Command, Output};
use std::{
    ffi::{CStr, CString, OsStr, OsString},
    fs::File,
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{
            ffi::{OsStrExt, OsStringExt},
            fs::MetadataExt,
        },
    },
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const RECORD: &str = "autostart.json";
const MAX_FILE: u64 = 64 * 1024;
#[cfg(target_os = "macos")]
const LABEL: &str = "com.roman-cuisset.miccamwatch.tray";
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AutostartState {
    Enabled,
    Disabled,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Registration {
    executable: PathBuf,
    version: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    format: u8,
    path: PathBuf,
    // Two candidates journal a replacement. Neither authorizes arbitrary bytes.
    registrations: Vec<Registration>,
}

struct Store {
    state: Directory,
    target: Directory,
    path: PathBuf,
    name: String,
    _lock: File,
}

struct RegistrationSnapshot {
    bytes: Option<Vec<u8>>,
    record: Option<Vec<u8>>,
    registration: Option<Registration>,
}

impl Store {
    fn open() -> Result<Self> {
        ordinary_user()?;
        let config = crate::settings::config_dir()?;
        let state = Directory::open(&config.join("autostart"), true)?;
        state.require_private()?;
        let lock = state.lock("autostart.lock")?;
        #[cfg(target_os = "linux")]
        let path = config
            .parent()
            .context("invalid XDG config directory")?
            .join("autostart/com.roman-cuisset.miccamwatch.desktop");
        #[cfg(target_os = "macos")]
        let path = absolute_home()?
            .join("Library/LaunchAgents")
            .join(format!("{LABEL}.plist"));
        let target = Directory::open(path.parent().context("invalid autostart path")?, true)?;
        let name = path
            .file_name()
            .and_then(OsStr::to_str)
            .context("autostart filename is not UTF-8")?
            .to_owned();
        Ok(Self {
            state,
            target,
            path,
            name,
            _lock: lock,
        })
    }

    fn snapshot(&self) -> Result<RegistrationSnapshot> {
        let record = self.state.read(RECORD)?;
        let receipt = record
            .as_deref()
            .map(serde_json::from_slice::<Receipt>)
            .transpose()
            .context("invalid autostart ownership receipt; registration was preserved")?;
        if let Some(receipt) = &receipt {
            if receipt.format != 1
                || receipt.path != self.path
                || receipt.registrations.is_empty()
                || receipt.registrations.len() > 2
            {
                bail!("invalid autostart ownership receipt; registration was preserved");
            }
            for entry in &receipt.registrations {
                registration_bytes(entry)?;
            }
        }
        let bytes = self.target.read(&self.name)?;
        let registered = match &bytes {
            Some(bytes) => {
                let receipt = receipt
                    .as_ref()
                    .context("autostart registration is unmanaged; it was preserved")?;
                Some(
                    receipt
                        .registrations
                        .iter()
                        .find(|entry| {
                            registration_bytes(entry).is_ok_and(|expected| expected == *bytes)
                        })
                        .cloned()
                        .context("autostart registration was manually changed; it was preserved")?,
                )
            }
            None => None,
        };
        Ok(RegistrationSnapshot {
            bytes,
            record,
            registration: registered,
        })
    }

    fn save_receipt(&self, entries: Vec<Registration>, expected: Option<&[u8]>) -> Result<Vec<u8>> {
        let receipt = Receipt {
            format: 1,
            path: self.path.clone(),
            registrations: entries,
        };
        let encoded = serde_json::to_vec(&receipt)?;
        self.state.replace(RECORD, expected, Some(&encoded))?;
        Ok(encoded)
    }
}

pub fn state() -> Result<AutostartState> {
    let store = Store::open()?;
    let registration = store.snapshot()?.registration;
    #[cfg(target_os = "macos")]
    {
        let native = Native::open()?;
        match registration {
            Some(entry) => {
                native.owned_loaded(&store.path, &entry)?;
                if native.disabled()? {
                    return Ok(AutostartState::Disabled);
                }
                if !native.owned_loaded(&store.path, &entry)? {
                    bail!(
                        "autostart plist is configured but its LaunchAgent is not loaded in {}; run autostart enable to repair it",
                        native.domain
                    );
                }
                return Ok(AutostartState::Enabled);
            }
            None => {
                if native.service()?.is_some() {
                    bail!("LaunchAgent exists without an unchanged owned plist; it was preserved");
                }
            }
        }
    }
    #[cfg(target_os = "linux")]
    if registration.is_some() {
        return Ok(AutostartState::Enabled);
    }
    Ok(AutostartState::Disabled)
}

pub fn enable() -> Result<()> {
    let store = Store::open()?;
    enable_in(&store, env!("CARGO_PKG_VERSION"))
}

/// Recheck the opt-in under the same lock used by enable/disable. Updating an
/// executable must never resurrect a registration the user has since removed.
pub fn refresh_if_enabled(version: &str) -> Result<()> {
    let store = Store::open()?;
    let registration = store.snapshot()?.registration;
    if registration.is_none() {
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    if Native::open()?.disabled()? {
        return Ok(());
    }
    enable_in(&store, version)
}

fn enable_in(store: &Store, version: &str) -> Result<()> {
    if version != env!("CARGO_PKG_VERSION") {
        bail!("autostart refresh must be performed by the matching mcw {version} executable");
    }
    let executable = std::env::current_exe().context("cannot locate mcw executable")?;
    let executable_dir = Directory::open(
        executable.parent().context("mcw has no parent directory")?,
        false,
    )?;
    let name = executable.file_name().context("mcw has no filename")?;
    let file = executable_dir
        .file(name)?
        .context("mcw executable disappeared")?;
    let metadata = file.metadata()?;
    if metadata.mode() & 0o111 == 0 {
        bail!("mcw executable is not executable");
    }
    let new = Registration {
        executable,
        version: version.to_owned(),
    };
    let new_bytes = registration_bytes(&new)?;
    let RegistrationSnapshot {
        bytes: old_bytes,
        record: old_record,
        registration: old,
    } = store.snapshot()?;
    #[cfg(target_os = "macos")]
    let native = Native::open()?;
    #[cfg(target_os = "macos")]
    let was_loaded = match &old {
        Some(entry) => native.owned_loaded(&store.path, entry)?,
        None => {
            if native.service()?.is_some() {
                bail!("refusing to replace an unmanaged LaunchAgent");
            }
            false
        }
    };
    #[cfg(target_os = "macos")]
    let was_disabled = native.disabled()?;
    let mut journal = old.clone().into_iter().collect::<Vec<_>>();
    if !journal.contains(&new) {
        journal.push(new.clone());
    }
    let journal_bytes = store.save_receipt(journal, old_record.as_deref())?;
    let result = (|| -> Result<()> {
        #[cfg(target_os = "macos")]
        if was_loaded {
            native.bootout(&store.path, old.as_ref().unwrap())?;
        }
        store
            .target
            .replace(&store.name, old_bytes.as_deref(), Some(&new_bytes))?;
        #[cfg(target_os = "macos")]
        {
            if was_disabled {
                native.set_disabled(false)?;
            }
            native.bootstrap(&store.path, &new)?;
        }
        store.save_receipt(vec![new.clone()], Some(&journal_bytes))?;
        Ok(())
    })();
    if let Err(error) = result {
        let rollback = (|| -> Result<()> {
            let current = store.target.read(&store.name)?;
            #[cfg(target_os = "macos")]
            if native.service()?.is_some() {
                let still_old = current == old_bytes
                    && old.as_ref().is_some_and(|entry| {
                        native.owned_loaded(&store.path, entry).unwrap_or(false)
                    });
                if !still_old {
                    native.bootout(&store.path, &new)?;
                }
            }
            if current != old_bytes {
                if current.as_deref() != Some(new_bytes.as_slice()) {
                    bail!("registration changed during rollback; it was preserved");
                }
                store
                    .target
                    .replace(&store.name, current.as_deref(), old_bytes.as_deref())?;
            }
            #[cfg(target_os = "macos")]
            {
                if was_disabled {
                    native.set_disabled(true)?;
                }
                if was_loaded {
                    native.bootstrap(&store.path, old.as_ref().unwrap())?;
                }
            }
            store
                .state
                .replace(RECORD, Some(&journal_bytes), old_record.as_deref())?;
            Ok(())
        })();
        return match rollback {
            Ok(()) => Err(error).context("autostart enable failed; previous registration restored"),
            Err(rollback) => Err(error).context(format!("autostart enable failed; rollback also failed: {rollback:#}; ownership journal retained")),
        };
    }
    Ok(())
}

pub fn disable() -> Result<()> {
    let store = Store::open()?;
    let RegistrationSnapshot {
        bytes,
        record,
        registration,
    } = store.snapshot()?;
    #[cfg(target_os = "macos")]
    let native = Native::open()?;
    #[cfg(target_os = "macos")]
    let was_loaded = match &registration {
        Some(entry) => native.owned_loaded(&store.path, entry)?,
        None => {
            if native.service()?.is_some() {
                bail!("LaunchAgent exists without an unchanged owned plist; it was preserved");
            }
            false
        }
    };
    let result = (|| -> Result<()> {
        #[cfg(target_os = "macos")]
        if was_loaded {
            native.bootout(&store.path, registration.as_ref().unwrap())?;
        }
        store.target.replace(&store.name, bytes.as_deref(), None)?;
        store.state.replace(RECORD, record.as_deref(), None)?;
        Ok(())
    })();
    if let Err(error) = result {
        let rollback = (|| -> Result<()> {
            let current = store.target.read(&store.name)?;
            if current != bytes {
                if current.is_some() {
                    bail!("registration changed during rollback; it was preserved");
                }
                store.target.replace(&store.name, None, bytes.as_deref())?;
            }
            #[cfg(target_os = "macos")]
            if was_loaded {
                native.bootstrap(&store.path, registration.as_ref().unwrap())?;
            }
            Ok(())
        })();
        return match rollback {
            Ok(()) => {
                Err(error).context("autostart disable failed; previous registration restored")
            }
            Err(rollback) => Err(error).context(format!(
                "autostart disable failed; rollback also failed: {rollback:#}"
            )),
        };
    }
    #[cfg(target_os = "linux")]
    let _ = registration;
    Ok(())
}

fn registration_bytes(entry: &Registration) -> Result<Vec<u8>> {
    let executable = entry
        .executable
        .to_str()
        .context("autostart executable must be UTF-8")?;
    if !entry.executable.is_absolute()
        || executable.chars().any(char::is_control)
        || entry.version.is_empty()
        || entry
            .version
            .chars()
            .any(|c| !c.is_ascii_alphanumeric() && !".-+".contains(c))
    {
        bail!("unsafe autostart executable or version");
    }
    #[cfg(target_os = "linux")]
    return Ok(format!("[Desktop Entry]\nType=Application\nName=MicCamWatch\nComment=MicCamWatch {} desktop-session tray\nExec={} tray run\nTerminal=false\nX-MicCamWatch-Managed=1\n", entry.version, desktop_argument(executable)).into_bytes());
    #[cfg(target_os = "macos")]
    return Ok(format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict>\n<key>Label</key><string>{LABEL}</string>\n<key>ProgramArguments</key><array><string>{}</string><string>tray</string><string>run</string></array>\n<key>RunAtLoad</key><true/>\n<key>LimitLoadToSessionType</key><string>Aqua</string>\n<key>ProcessType</key><string>Interactive</string>\n<key>MicCamWatchVersion</key><string>{}</string>\n</dict></plist>\n", xml(executable), xml(&entry.version)).into_bytes());
}

#[cfg(target_os = "linux")]
fn desktop_argument(argument: &str) -> String {
    // Exec has TWO escape layers: Desktop Entry string decoding, then argv
    // quoting. Field-code expansion is independent of quoting; %% is literal %.
    let mut encoded = String::with_capacity(argument.len() + 2);
    encoded.push('"');
    for character in argument.chars() {
        match character {
            '\\' => encoded.push_str("\\\\\\\\"),
            '"' => encoded.push_str("\\\\\""),
            '$' => encoded.push_str("\\\\$"),
            '`' => encoded.push_str("\\\\`"),
            '%' => encoded.push_str("%%"),
            _ => encoded.push(character),
        }
    }
    encoded.push('"');
    encoded
}

#[cfg(target_os = "macos")]
fn xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub(crate) fn ordinary_user() -> Result<()> {
    let uid = unsafe { libc::getuid() };
    if uid == 0 || unsafe { libc::geteuid() } != uid {
        bail!("run MicCamWatch user services as your ordinary user, without root, sudo or setuid");
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn absolute_home() -> Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .context("HOME is unavailable")?;
    if !home.is_absolute() {
        bail!("HOME must be absolute");
    }
    let directory = Directory::open(&home, false)?;
    if directory.0.metadata()?.uid() != unsafe { libc::getuid() } {
        bail!("HOME is not owned by the current user");
    }
    Ok(home)
}

/// Descriptor-relative filesystem access prevents a symlink in ANY ancestor,
/// not just the final filename, from redirecting an ownership operation.
pub(crate) struct Directory(File);

impl Directory {
    pub(crate) fn open(path: &Path, create: bool) -> Result<Self> {
        if !path.is_absolute() {
            bail!("directory must be absolute: {}", path.display());
        }
        let root = CString::new("/")?;
        let fd = unsafe {
            libc::open(
                root.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(std::io::Error::last_os_error()).context("cannot open filesystem root");
        }
        let mut current = Self(unsafe { File::from_raw_fd(fd) });
        for component in path.components() {
            let Component::Normal(name) = component else {
                if matches!(component, Component::RootDir) {
                    continue;
                }
                bail!(
                    "directory contains a non-normal component: {}",
                    path.display()
                );
            };
            let name = c_name(name)?;
            let mut fd = unsafe {
                libc::openat(
                    current.0.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            };
            if fd < 0
                && create
                && std::io::Error::last_os_error().kind() == std::io::ErrorKind::NotFound
            {
                let status = unsafe { libc::mkdirat(current.0.as_raw_fd(), name.as_ptr(), 0o700) };
                if status != 0
                    && std::io::Error::last_os_error().kind() != std::io::ErrorKind::AlreadyExists
                {
                    return Err(std::io::Error::last_os_error())
                        .context("cannot create user service directory");
                }
                fd = unsafe {
                    libc::openat(
                        current.0.as_raw_fd(),
                        name.as_ptr(),
                        libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                    )
                };
            }
            if fd < 0 {
                return Err(std::io::Error::last_os_error()).with_context(|| {
                    format!("cannot open {} without following symlinks", path.display())
                });
            }
            current = Self(unsafe { File::from_raw_fd(fd) });
            let metadata = current.0.metadata()?;
            let uid = unsafe { libc::getuid() };
            if metadata.uid() != uid && metadata.uid() != 0 {
                bail!(
                    "directory ancestor is owned by another user: {}",
                    path.display()
                );
            }
            if metadata.mode() & 0o022 != 0
                && !(metadata.uid() == 0 && metadata.mode() & 0o1000 != 0)
            {
                bail!(
                    "directory ancestor is writable by another user: {}",
                    path.display()
                );
            }
        }
        Ok(current)
    }

    fn require_private(&self) -> Result<()> {
        let metadata = self.0.metadata()?;
        if metadata.uid() != unsafe { libc::getuid() } || metadata.mode() & 0o077 != 0 {
            bail!("MicCamWatch service state directory must be owned by you with mode 0700");
        }
        Ok(())
    }

    pub(crate) fn create_directory(&self, name: &str) -> Result<Option<Self>> {
        let name = c_name(OsStr::new(name))?;
        if unsafe { libc::mkdirat(self.0.as_raw_fd(), name.as_ptr(), 0o700) } != 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                return Ok(None);
            }
            return Err(error).context("cannot create private updater staging directory");
        }
        let fd = unsafe {
            libc::openat(
                self.0.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(std::io::Error::last_os_error())
                .context("cannot open private updater staging directory");
        }
        let directory = Self(unsafe { File::from_raw_fd(fd) });
        directory.require_private()?;
        Ok(Some(directory))
    }

    pub(crate) fn create_file(&self, name: &str, bytes: &[u8]) -> Result<()> {
        let name = c_name(OsStr::new(name))?;
        let fd = unsafe {
            libc::openat(
                self.0.as_raw_fd(),
                name.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                0o600,
            )
        };
        if fd < 0 {
            return Err(std::io::Error::last_os_error()).context("cannot stage updater script");
        }
        let mut file = unsafe { File::from_raw_fd(fd) };
        file.write_all(bytes)?;
        file.sync_all()?;
        self.0.sync_all()?;
        Ok(())
    }

    pub(crate) fn remove_entry(&self, name: &str, directory: bool) -> Result<()> {
        let name = c_name(OsStr::new(name))?;
        let flags = if directory { libc::AT_REMOVEDIR } else { 0 };
        if unsafe { libc::unlinkat(self.0.as_raw_fd(), name.as_ptr(), flags) } != 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() != std::io::ErrorKind::NotFound {
                return Err(error).context("cannot remove updater staging entry");
            }
        }
        Ok(())
    }

    pub(crate) fn file(&self, name: &OsStr) -> Result<Option<File>> {
        let name = c_name(name)?;
        let fd = unsafe {
            libc::openat(
                self.0.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
            )
        };
        if fd < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::NotFound {
                return Ok(None);
            }
            return Err(error).context("cannot open owned file without following symlinks");
        }
        let file = unsafe { File::from_raw_fd(fd) };
        validate_file(&file)?;
        Ok(Some(file))
    }

    pub(crate) fn names(&self) -> Result<Vec<OsString>> {
        let fd = unsafe { libc::dup(self.0.as_raw_fd()) };
        if fd < 0 {
            return Err(std::io::Error::last_os_error())
                .context("cannot inspect ownership directory");
        }
        let stream = unsafe { libc::fdopendir(fd) };
        if stream.is_null() {
            let error = std::io::Error::last_os_error();
            unsafe {
                libc::close(fd);
            }
            return Err(error).context("cannot inspect ownership directory");
        }
        struct Stream(*mut libc::DIR);
        impl Drop for Stream {
            fn drop(&mut self) {
                unsafe {
                    libc::closedir(self.0);
                }
            }
        }
        let stream = Stream(stream);
        unsafe {
            libc::rewinddir(stream.0);
        }
        let mut names = Vec::new();
        loop {
            #[cfg(target_os = "linux")]
            let errno = unsafe { libc::__errno_location() };
            #[cfg(target_os = "macos")]
            let errno = unsafe { libc::__error() };
            unsafe {
                *errno = 0;
            }
            let entry = unsafe { libc::readdir(stream.0) };
            if entry.is_null() {
                let error = unsafe { *errno };
                if error != 0 {
                    return Err(std::io::Error::from_raw_os_error(error))
                        .context("cannot read ownership directory");
                }
                return Ok(names);
            }
            let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
            if name != b"." && name != b".." {
                names.push(OsString::from_vec(name.to_vec()));
            }
        }
    }

    pub(crate) fn read(&self, name: &str) -> Result<Option<Vec<u8>>> {
        let Some(file) = self.file(OsStr::new(name))? else {
            return Ok(None);
        };
        if file.metadata()?.len() > MAX_FILE {
            bail!("user service ownership file exceeds size limit");
        }
        let mut bytes = Vec::new();
        file.take(MAX_FILE + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_FILE {
            bail!("user service ownership file exceeds size limit");
        }
        Ok(Some(bytes))
    }

    fn lock(&self, name: &str) -> Result<File> {
        let name = c_name(OsStr::new(name))?;
        let fd = unsafe {
            libc::openat(
                self.0.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDWR
                    | libc::O_CREAT
                    | libc::O_NOFOLLOW
                    | libc::O_CLOEXEC
                    | libc::O_NONBLOCK,
                0o600,
            )
        };
        if fd < 0 {
            return Err(std::io::Error::last_os_error()).context("cannot open autostart lock");
        }
        let file = unsafe { File::from_raw_fd(fd) };
        validate_file(&file)?;
        if unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(std::io::Error::last_os_error())
                .context("another autostart operation is running");
        }
        Ok(file)
    }

    fn replace(
        &self,
        name: &str,
        expected: Option<&[u8]>,
        replacement: Option<&[u8]>,
    ) -> Result<()> {
        if self.read(name)?.as_deref() != expected {
            bail!("user service file changed concurrently; it was preserved");
        }
        let name_c = c_name(OsStr::new(name))?;
        let Some(bytes) = replacement else {
            if expected.is_some()
                && unsafe { libc::unlinkat(self.0.as_raw_fd(), name_c.as_ptr(), 0) } != 0
            {
                return Err(std::io::Error::last_os_error())
                    .context("cannot remove owned registration");
            }
            self.0.sync_all()?;
            return Ok(());
        };
        let temporary = format!(
            ".mcw-autostart-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        );
        let temporary_c = c_name(OsStr::new(&temporary))?;
        let fd = unsafe {
            libc::openat(
                self.0.as_raw_fd(),
                temporary_c.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                0o600,
            )
        };
        if fd < 0 {
            return Err(std::io::Error::last_os_error()).context("cannot stage user service file");
        }
        let mut file = unsafe { File::from_raw_fd(fd) };
        let result = (|| -> Result<()> {
            file.write_all(bytes)?;
            file.sync_all()?;
            if self.read(name)?.as_deref() != expected {
                bail!("user service file changed concurrently; it was preserved");
            }
            let status = if expected.is_none() {
                // linkat refuses an entry created concurrently; rename would overwrite it.
                unsafe {
                    libc::linkat(
                        self.0.as_raw_fd(),
                        temporary_c.as_ptr(),
                        self.0.as_raw_fd(),
                        name_c.as_ptr(),
                        0,
                    )
                }
            } else {
                unsafe {
                    libc::renameat(
                        self.0.as_raw_fd(),
                        temporary_c.as_ptr(),
                        self.0.as_raw_fd(),
                        name_c.as_ptr(),
                    )
                }
            };
            if status != 0 {
                return Err(std::io::Error::last_os_error())
                    .context("cannot commit owned registration");
            }
            self.0.sync_all()?;
            Ok(())
        })();
        unsafe {
            libc::unlinkat(self.0.as_raw_fd(), temporary_c.as_ptr(), 0);
        }
        result
    }
}

fn c_name(name: &OsStr) -> Result<CString> {
    let bytes = name.as_bytes();
    if bytes.is_empty() || bytes == b"." || bytes == b".." || bytes.contains(&b'/') {
        bail!("invalid relative user service filename");
    }
    CString::new(bytes).context("filename contains NUL")
}

fn validate_file(file: &File) -> Result<()> {
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.nlink() != 1
        || metadata.uid() != unsafe { libc::getuid() }
        || metadata.mode() & 0o022 != 0
    {
        bail!(
            "refusing nonregular, hardlinked, foreign-owned or writable-by-others user service file"
        );
    }
    Ok(())
}

#[cfg(target_os = "macos")]
struct Native {
    domain: String,
    service: String,
}

#[cfg(target_os = "macos")]
impl Native {
    fn open() -> Result<Self> {
        let domain = format!("gui/{}", unsafe { libc::getuid() });
        let output = launchctl(&["print", &domain])?;
        checked_output(
            output,
            "the current user's GUI launchd domain is unavailable; log into a desktop session",
        )?;
        Ok(Self {
            service: format!("{domain}/{LABEL}"),
            domain,
        })
    }

    fn service(&self) -> Result<Option<String>> {
        let output = launchctl(&["print", &self.service])?;
        if output.status.success() {
            return Ok(Some(
                String::from_utf8(output.stdout).context("invalid launchctl status encoding")?,
            ));
        }
        // ESRCH is launchctl's documented missing-service error, not a blanket
        // conversion of native failures to Disabled.
        if output.status.code() == Some(libc::ESRCH) || output.status.code() == Some(113) {
            return Ok(None);
        }
        checked_output(output, "cannot inspect LaunchAgent")?;
        unreachable!()
    }

    fn owned_loaded(&self, path: &Path, entry: &Registration) -> Result<bool> {
        let Some(status) = self.service()? else {
            return Ok(false);
        };
        let executable = entry.executable.to_str().context("non-UTF-8 executable")?;
        let path = path.to_str().context("non-UTF-8 LaunchAgent path")?;
        let field = |name: &str| {
            status
                .lines()
                .find_map(|line| line.trim().strip_prefix(name))
        };
        if field("path = ") != Some(path) || field("program = ") != Some(executable) {
            bail!("LaunchAgent label belongs to a different registration; it was preserved");
        }
        let mut lines = status.lines().map(str::trim);
        let arguments = if lines.any(|line| line == "arguments = {") {
            lines.take_while(|line| *line != "}").collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        if arguments != [executable, "tray", "run"] {
            bail!("loaded LaunchAgent arguments were manually changed; it was preserved");
        }
        Ok(true)
    }

    fn disabled(&self) -> Result<bool> {
        let output = launchctl(&["print-disabled", &self.domain])?;
        let output = checked_output(output, "cannot inspect disabled LaunchAgents")?;
        let text = String::from_utf8(output.stdout)
            .context("invalid launchctl disabled-state encoding")?;
        let key = format!("\"{LABEL}\" => ");
        for line in text.lines() {
            if let Some(value) = line.trim().strip_prefix(&key) {
                return match value.trim_end_matches([',', ';']).trim() {
                    "true" => Ok(true),
                    "false" => Ok(false),
                    _ => bail!("unrecognized LaunchAgent disabled-state response"),
                };
            }
        }
        Ok(false)
    }

    fn set_disabled(&self, disabled: bool) -> Result<()> {
        let operation = if disabled { "disable" } else { "enable" };
        checked_output(
            launchctl(&[operation, &self.service])?,
            "cannot change LaunchAgent disabled state",
        )?;
        if self.disabled()? != disabled {
            bail!("LaunchAgent disabled-state readback failed");
        }
        Ok(())
    }

    fn bootout(&self, path: &Path, entry: &Registration) -> Result<()> {
        if !self.owned_loaded(path, entry)? {
            return Ok(());
        }
        checked_output(
            launchctl(&["bootout", &self.service])?,
            "cannot unload LaunchAgent",
        )?;
        if self.service()?.is_some() {
            bail!("LaunchAgent remains loaded after bootout");
        }
        Ok(())
    }

    fn bootstrap(&self, path: &Path, entry: &Registration) -> Result<()> {
        if self.owned_loaded(path, entry)? {
            return Ok(());
        }
        let path_text = path.to_str().context("non-UTF-8 LaunchAgent path")?;
        checked_output(
            launchctl(&["bootstrap", &self.domain, path_text])?,
            "cannot bootstrap LaunchAgent",
        )?;
        if !self.owned_loaded(path, entry)? {
            bail!("LaunchAgent was not loaded after bootstrap");
        }
        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn launchctl(arguments: &[&str]) -> Result<Output> {
    Command::new("/bin/launchctl")
        .args(arguments)
        .output()
        .context("cannot execute launchctl")
}

#[cfg(target_os = "macos")]
fn checked_output(output: Output, operation: &str) -> Result<Output> {
    if !output.status.success() {
        bail!(
            "{operation}: {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};

    struct Temporary(PathBuf);
    impl Temporary {
        fn new() -> Result<Self> {
            let path = std::env::temp_dir().canonicalize()?.join(format!(
                "mcw-autostart-test-{}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&path)?;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))?;
            Ok(Self(path))
        }
    }
    impl Drop for Temporary {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn desktop_exec_preserves_reserved_characters_and_literal_field_codes() {
        assert_eq!(
            desktop_argument("/home/a b/\"$`%f\\mcw"),
            r#""/home/a b/\\"\\$\\`%%f\\\\mcw""#
        );
        assert_eq!(
            desktop_argument("/home/O'Brien/(mcw);&"),
            "\"/home/O'Brien/(mcw);&\""
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn plist_argv_preserves_xml_metacharacters_without_shell_interpretation() -> Result<()> {
        let registration = Registration {
            executable: PathBuf::from("/Users/A & B/<$'\"%>/mcw"),
            version: "1.2.3".into(),
        };
        let bytes = registration_bytes(&registration)?;
        let text = String::from_utf8(bytes)?;
        assert!(text.contains("<string>/Users/A &amp; B/&lt;$&apos;&quot;%&gt;/mcw</string><string>tray</string><string>run</string>"));
        Ok(())
    }

    #[test]
    fn ownership_refuses_modified_file_symlink_and_hardlink() -> Result<()> {
        let temporary = Temporary::new()?;
        let directory = Directory::open(&temporary.0, false)?;
        directory.replace("entry", None, Some(b"owned"))?;
        std::fs::write(temporary.0.join("entry"), b"manual change")?;
        assert!(directory.replace("entry", Some(b"owned"), None).is_err());
        assert_eq!(std::fs::read(temporary.0.join("entry"))?, b"manual change");
        symlink("entry", temporary.0.join("link"))?;
        assert!(directory.read("link").is_err());
        std::fs::hard_link(temporary.0.join("entry"), temporary.0.join("hardlink"))?;
        assert!(directory.read("entry").is_err());
        symlink(&temporary.0, temporary.0.join("directory-link"))?;
        assert!(Directory::open(&temporary.0.join("directory-link"), false).is_err());
        Ok(())
    }

    #[test]
    fn creation_does_not_claim_an_existing_unmanaged_registration() -> Result<()> {
        let temporary = Temporary::new()?;
        let directory = Directory::open(&temporary.0, false)?;
        directory.replace("entry", None, Some(b"unmanaged"))?;
        assert!(directory.replace("entry", None, Some(b"ours")).is_err());
        assert_eq!(directory.read("entry")?.unwrap(), b"unmanaged");
        Ok(())
    }
}
