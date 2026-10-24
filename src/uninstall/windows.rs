use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Read, Seek, SeekFrom, Write},
    os::windows::{
        ffi::OsStringExt,
        fs::{MetadataExt, OpenOptionsExt},
        io::AsRawHandle,
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE},
        Storage::FileSystem::{
            BY_HANDLE_FILE_INFORMATION, FILE_DISPOSITION_INFO, FileDispositionInfo,
            GetFileInformationByHandle, SetFileInformationByHandle,
        },
        System::Threading::{
            GetCurrentProcess, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
            QueryFullProcessImageNameW,
        },
    },
    core::PWSTR,
};

pub(crate) const RECEIPT: &str = ".miccamwatch-portable.json";
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Receipt {
    pub format: u8,
    pub version: String,
    pub cli_sha256: String,
    pub tray_sha256: String,
}

pub(super) fn run() -> Result<u8> {
    ordinary_user()?;
    let current = std::env::current_exe()?;
    ensure!(
        current
            .file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case("mcw.exe")),
        "uninstall requires the installed mcw.exe, not a renamed copy"
    );
    if owner_guidance(&current)? {
        return Ok(2);
    }
    let _installation = crate::autostart::try_lock_installation(&current)?;
    let pair = Pair::inspect(&current)?;
    let stage = tempfile::Builder::new()
        .prefix(".mcw-uninstall-")
        .tempdir()?;
    let helper = stage.path().join("worker.exe");
    // Copy only our already verified CLI. The worker derives targets from its
    // actual kernel parent image; it accepts no user-supplied deletion path.
    fs::copy(&current, &helper)?;
    ensure!(
        digest(&mut File::open(&helper)?)? == pair.cli.hash,
        "uninstall worker copy differs from the verified CLI"
    );
    let log = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(stage.path().join("worker.log"))?;
    drop(pair);
    drop(_installation);
    let stage_path = stage.keep();
    let mut child = Command::new(&helper)
        .arg("__uninstall-worker")
        .arg(std::process::id().to_string())
        .arg(crate::windows_control::current_process_created()?.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(log)
        .creation_flags(0x0800_0000)
        .spawn()
        .context("cannot launch cooperative uninstall worker; installation preserved")?;
    // The worker performs the actual preflight, takes the existing native
    // reservations and safely stops the exact frontend before acknowledging.
    // No success is claimed for a worker that fails its preflight.
    let stdout = child
        .stdout
        .take()
        .context("worker acknowledgement pipe absent")?;
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout.take(256));
        let mut line = String::new();
        let result = reader.read_line(&mut line).map(|_| line);
        let _ = sender.send(result);
    });
    let acknowledgement = receiver.recv_timeout(Duration::from_secs(120)).context("uninstall worker did not confirm readiness; nothing was forced; inspect worker.log in the retained staging directory")??;
    ensure!(
        acknowledgement == "ready\n",
        "uninstall worker refused; installation/protection preserved; inspect {}",
        stage_path.join("worker.log").display()
    );
    println!(
        "Uninstall accepted for this exact portable installation. This CLI will exit normally; the authenticated worker retains protection reservations and then removes the verified pair. Completion is recorded in {} (not a claim that deferred deletion has already succeeded). Settings/history/policy are preserved. The private worker recovery directory can be removed after completion.",
        stage_path.join("result.json").display()
    );
    Ok(0)
}

pub(super) fn run_worker(parent_pid: u32, parent_created: u64) -> Result<u8> {
    ordinary_user()?;
    let helper = std::env::current_exe()?;
    ensure!(
        helper.file_name().is_some_and(|name| name == "worker.exe"),
        "uninstall bootstrap is not a staged worker"
    );
    let stage = helper.parent().context("worker has no staging directory")?;
    ensure!(
        stage
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with(".mcw-uninstall-")),
        "unrecognized worker staging directory"
    );
    safe_ancestors(stage)?;
    let result = worker_inner(parent_pid, parent_created, &helper);
    let record = match &result {
        Ok(()) => serde_json::json!({"status":"removed", "data_preserved":true}),
        Err(error) => {
            serde_json::json!({"status":"refused_or_incomplete", "error":format!("{error:#}"), "data_preserved":true})
        }
    };
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(stage.join("result.json"))?;
    serde_json::to_writer(&mut output, &record)?;
    output.write_all(b"\n")?;
    output.sync_all()?;
    result?;
    Ok(0)
}

fn worker_inner(parent_pid: u32, parent_created: u64, helper: &Path) -> Result<()> {
    // Shorter than the caller's 120-second acknowledgement deadline. Even a
    // slow read-only preflight must recheck this before any frontend mutation.
    let preflight_deadline = Instant::now() + Duration::from_secs(90);
    // The kernel parent relationship is additional to SID/session/creation-time
    // authentication. A caller cannot merely name another live MCW PID.
    ensure!(
        kernel_parent()? == parent_pid,
        "uninstall bootstrap parent does not match its kernel parent"
    );
    let identity = crate::windows_control::current_identity()?;
    let parent = crate::windows_control::invoker_lease(&identity, parent_pid, parent_created)?;
    let current = process_image(parent_pid)?;
    ensure!(
        current
            .file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case("mcw.exe")),
        "parent is not an installed mcw.exe"
    );
    let stage_root = helper
        .parent()
        .and_then(Path::parent)
        .context("worker staging root missing")?;
    ensure!(
        stage_root.canonicalize()? == std::env::temp_dir().canonicalize()?,
        "worker is not in its inherited private temporary staging namespace"
    );
    ensure!(
        !owner_guidance(&current)?,
        "installation is manager owned; use its reported removal command"
    );
    let installation = crate::autostart::try_lock_installation(&current)?;
    let pair = Pair::inspect(&current)?;
    ensure!(
        digest(&mut File::open(helper)?)? == pair.cli.hash,
        "worker is not an exact copy of its authenticated parent image"
    );
    let preferences_path = crate::settings::settings_path()?.with_extension("lock");
    fs::create_dir_all(
        preferences_path
            .parent()
            .context("preferences have no directory")?,
    )?;
    safe_ancestors(
        preferences_path
            .parent()
            .context("preferences have no directory")?,
    )?;
    let preferences = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .share_mode(3)
        .custom_flags(0x0020_0000)
        .open(&preferences_path)?;
    validate_regular(&preferences)?;
    preferences
        .try_lock()
        .context("preferences are being changed; uninstall refused")?;
    super::ensure_preferences_inactive()?;
    let registration = crate::autostart::UninstallRegistration::prepare(&current, &installation)?;
    let running = crate::updater::process::installed_running_tray(
        &current,
        &current.with_file_name("mcw-tray.exe"),
    )?;
    let _reservation = crate::updater::transaction::ProtectionReservation::acquire().context("uninstall refused before frontend shutdown; explicitly restore microphone/camera and resolve pending or unknown restoration; no protection was released")?;
    pair.verify_paths()?;
    ensure!(
        Instant::now() < preflight_deadline && !parent.exited()?,
        "uninstall preflight expired or initiating parent departed; frontend and autostart were not changed"
    );
    if let Some(running) = running {
        running.stop()?;
    }
    ensure!(
        Instant::now() < preflight_deadline + Duration::from_secs(20) && !parent.exited()?,
        "uninstall authorization expired or initiating parent departed after frontend shutdown; autostart and payloads were preserved"
    );
    registration.remove()?;
    pair.verify_paths()?;
    // A disconnected caller is an ordinary recorded failure, never a println!
    // panic that skips result.json or authorizes later payload deletion.
    let mut acknowledgement = std::io::stdout().lock();
    acknowledgement.write_all(b"ready\n").context(
        "initiating caller disconnected before uninstall acknowledgement; payloads preserved",
    )?;
    acknowledgement
        .flush()
        .context("cannot acknowledge uninstall readiness; payloads preserved")?;
    let deadline = Instant::now() + Duration::from_secs(30);
    while !parent.exited()? {
        ensure!(
            Instant::now() < deadline,
            "parent did not exit normally; installed payloads retained; no forced kill was attempted"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    pair.verify_paths()?;
    // Delete by retained, no-write-sharing file handles, not by later-supplied
    // path strings. A renamed/replaced/linked path cannot authorize deletion.
    pair.tray.delete()?;
    pair.cli.delete()?;
    if let Some(receipt) = pair.receipt {
        receipt.delete()?;
    }
    Ok(())
}

struct Pair {
    cli: Snapshot,
    tray: Snapshot,
    receipt: Option<Snapshot>,
}
impl Pair {
    fn inspect(current: &Path) -> Result<Self> {
        safe_ancestors(current.parent().context("installation has no directory")?)?;
        let mut cli = Snapshot::open(current)?;
        let mut tray = Snapshot::open(&current.with_file_name("mcw-tray.exe"))?;
        let receipt_path = current.with_file_name(RECEIPT);
        let receipt = if fs::symlink_metadata(&receipt_path).is_ok() {
            let mut snapshot = Snapshot::open(&receipt_path)?;
            ensure!(
                snapshot.file.metadata()?.len() <= 4096,
                "portable ownership receipt is too large"
            );
            let record: Receipt = serde_json::from_reader(&mut snapshot.file)
                .context("invalid portable ownership receipt; pair preserved")?;
            ensure!(
                record.format == 1 && record.version == env!("CARGO_PKG_VERSION"),
                "portable receipt format/version does not match this CLI"
            );
            ensure!(
                record.cli_sha256 == cli.hash && record.tray_sha256 == tray.hash,
                "portable pair was modified outside its recorded installation; it was preserved"
            );
            Some(snapshot)
        } else {
            // Legacy ZIPs have no receipt: compare against the exact same
            // version's authenticated public package, never latest/version text.
            let stage = tempfile::tempdir()?;
            let releases = self_update::backends::github::ReleaseList::configure()
                .repo_owner("Roman-Cuisset")
                .repo_name("miccamwatch")
                .build()?
                .fetch()?;
            let release = releases.all().iter().find(|release| release.version() == env!("CARGO_PKG_VERSION")).context("no exact published package for this unreceipted pair; preserve it and remove only the reviewed exact pair manually after restoration")?;
            crate::updater::download::stage_package(release, stage.path())?;
            ensure!(
                cli.hash == digest(&mut File::open(stage.path().join("mcw.exe"))?)?
                    && tray.hash == digest(&mut File::open(stage.path().join("mcw-tray.exe"))?)?,
                "unreceipted pair differs from its exact verified public release; modified or foreign payloads were preserved"
            );
            None
        };
        cli.file.seek(SeekFrom::Start(0))?;
        tray.file.seek(SeekFrom::Start(0))?;
        Ok(Self { cli, tray, receipt })
    }
    fn verify_paths(&self) -> Result<()> {
        self.cli.verify_path()?;
        self.tray.verify_path()?;
        if let Some(receipt) = &self.receipt {
            receipt.verify_path()?;
        }
        Ok(())
    }
}

struct Snapshot {
    file: File,
    path: PathBuf,
    identity: (u32, u32, u32),
    hash: String,
}
impl Snapshot {
    fn open(path: &Path) -> Result<Self> {
        safe_ancestors(path.parent().context("payload has no directory")?)?;
        let mut file = OpenOptions::new()
            .read(true)
            .access_mode(0x8000_0000 | 0x0001_0000)
            .share_mode(1 | 4)
            .custom_flags(0x0020_0000)
            .open(path)
            .with_context(|| {
                format!(
                    "cannot reserve unchanged payload {} (no ACL/antivirus bypass attempted)",
                    path.display()
                )
            })?;
        let identity = validate_regular(&file)?;
        let hash = digest(&mut file)?;
        file.seek(SeekFrom::Start(0))?;
        Ok(Self {
            file,
            path: path.to_owned(),
            identity,
            hash,
        })
    }
    fn verify_path(&self) -> Result<()> {
        safe_ancestors(self.path.parent().context("payload has no directory")?)?;
        let file = OpenOptions::new()
            .read(true)
            .share_mode(1 | 4)
            .custom_flags(0x0020_0000)
            .open(&self.path)?;
        ensure!(
            validate_regular(&file)? == self.identity,
            "payload identity changed; uninstall refused"
        );
        Ok(())
    }
    fn delete(self) -> Result<()> {
        self.verify_path()?;
        let info = FILE_DISPOSITION_INFO { DeleteFile: true };
        unsafe { SetFileInformationByHandle(HANDLE(self.file.as_raw_handle()), FileDispositionInfo, (&info as *const FILE_DISPOSITION_INFO).cast(), std::mem::size_of_val(&info) as u32) }.with_context(|| format!("cannot remove verified payload {}; no forced kill, ACL change, quarantine recovery or reboot deletion was attempted", self.path.display()))?;
        Ok(())
    }
}

fn validate_regular(file: &File) -> Result<(u32, u32, u32)> {
    let metadata = file.metadata()?;
    ensure!(
        metadata.is_file() && metadata.file_attributes() & 0x400 == 0,
        "uninstall target is not a regular non-reparse file"
    );
    ensure_owned(file)?;
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    unsafe { GetFileInformationByHandle(HANDLE(file.as_raw_handle()), &mut info) }?;
    ensure!(
        info.nNumberOfLinks == 1,
        "hardlinked uninstall target was preserved"
    );
    Ok((
        info.dwVolumeSerialNumber,
        info.nFileIndexHigh,
        info.nFileIndexLow,
    ))
}

fn ensure_owned(file: &File) -> Result<()> {
    use windows::Win32::Security::{
        Authorization::{ConvertSidToStringSidW, GetSecurityInfo, SE_FILE_OBJECT},
        OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID,
    };
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LocalFree(memory: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    }
    let mut owner = PSID::default();
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    let status = unsafe {
        GetSecurityInfo(
            HANDLE(file.as_raw_handle()),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION,
            Some(&mut owner),
            None,
            None,
            None,
            Some(&mut descriptor),
        )
    };
    ensure!(status.0 == 0, "cannot establish uninstall file ownership");
    let mut text = PWSTR::null();
    let converted = unsafe { ConvertSidToStringSidW(owner, &mut text) };
    unsafe {
        LocalFree(descriptor.0);
    }
    converted?;
    let sid = unsafe { text.to_string() };
    unsafe {
        LocalFree(text.0.cast());
    }
    ensure!(
        sid? == crate::windows_control::current_identity()?.user,
        "uninstall file belongs to another user; it was preserved"
    );
    Ok(())
}

fn safe_ancestors(path: &Path) -> Result<()> {
    ensure!(path.is_absolute(), "installation path must be absolute");
    for ancestor in path.ancestors() {
        let metadata = fs::symlink_metadata(ancestor)?;
        ensure!(
            metadata.is_dir() && metadata.file_attributes() & 0x400 == 0,
            "linked/reparse installation ancestor was preserved"
        );
    }
    Ok(())
}

pub(crate) fn digest(file: &mut File) -> Result<String> {
    file.seek(SeekFrom::Start(0))?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    use std::fmt::Write as _;
    let mut hex = String::with_capacity(64);
    for byte in hash.finalize() {
        write!(&mut hex, "{byte:02x}")?;
    }
    Ok(hex)
}

fn process_image(pid: u32) -> Result<PathBuf> {
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }?;
    let mut buffer = [0u16; 32768];
    let mut length = buffer.len() as u32;
    let result = unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut length,
        )
    };
    unsafe {
        let _ = CloseHandle(process);
    }
    result?;
    Ok(PathBuf::from(OsString::from_wide(
        &buffer[..length as usize],
    )))
}

#[repr(C)]
#[derive(Default)]
struct BasicInformation {
    reserved1: usize,
    peb: usize,
    reserved2: [usize; 2],
    pid: usize,
    parent_pid: usize,
}
fn kernel_parent() -> Result<u32> {
    let mut information = BasicInformation::default();
    let status = unsafe {
        crate::platform::NtQueryInformationProcess(
            GetCurrentProcess(),
            0,
            (&mut information as *mut BasicInformation).cast(),
            std::mem::size_of_val(&information) as u32,
            std::ptr::null_mut(),
        )
    };
    ensure!(status.0 >= 0, "cannot authenticate worker's kernel parent");
    Ok(information.parent_pid.try_into()?)
}

fn ordinary_user() -> Result<()> {
    use windows::Win32::{
        Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation},
        System::Threading::OpenProcessToken,
    };
    let mut token = HANDLE::default();
    unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) }?;
    let mut elevation = TOKEN_ELEVATION::default();
    let mut returned = 0;
    let result = unsafe {
        GetTokenInformation(
            token,
            TokenElevation,
            Some((&mut elevation as *mut TOKEN_ELEVATION).cast()),
            std::mem::size_of_val(&elevation) as u32,
            &mut returned,
        )
    };
    unsafe {
        let _ = CloseHandle(token);
    }
    result?;
    ensure!(
        elevation.TokenIsElevated == 0,
        "uninstall refuses an elevated operation; run as your ordinary user after completing privileged camera operations"
    );
    Ok(())
}

fn owner_guidance(current: &Path) -> Result<bool> {
    let parent = current.parent().context("executable has no directory")?;
    safe_ancestors(parent)?;
    if parent.file_name().is_some_and(|name| name == "bin") {
        let root = parent.parent().context("Cargo root missing")?;
        let manifest = root.join(".crates2.json");
        if let Ok(bytes) = fs::read(&manifest) {
            let value: serde_json::Value = serde_json::from_slice(&bytes)
                .context("invalid Cargo install metadata; installation preserved")?;
            if value
                .get("installs")
                .and_then(serde_json::Value::as_object)
                .is_some_and(|installs| {
                    installs.iter().any(|(package, install)| {
                        package.starts_with("miccamwatch ")
                            && install
                                .get("bins")
                                .and_then(serde_json::Value::as_array)
                                .is_some_and(|bins| {
                                    bins.iter()
                                        .any(|bin| matches!(bin.as_str(), Some("mcw" | "mcw.exe")))
                                })
                    })
                })
            {
                eprintln!(
                    "Cargo owns this exact install root. Explicitly restore protection and normally exit its frontends, then run: cargo uninstall --root \"{}\" miccamwatch. Cargo metadata has no historical binary hashes, so MCW does not delete or claim removal of a possibly modified Cargo payload.",
                    root.display()
                );
                return Ok(true);
            }
        }
    }
    match registered_owner(parent)? {
        Some(RegisteredOwner::Msi(product_code)) => {
            eprintln!(
                "Windows Installer registration matches this installation. Explicitly restore protection and exit its frontends, then run: msiexec.exe /x {product_code}. The MSI owns PATH/shortcuts/payload removal. No MSI operation, elevation or success was claimed."
            );
            Ok(true)
        }
        Some(RegisteredOwner::Managed) => {
            eprintln!(
                "A registered MicCamWatch installation matches this directory; use Windows Settings > Apps > Installed apps > MicCamWatch > Uninstall after explicit restoration and normal frontend exit. No registered command was executed or success claimed."
            );
            Ok(true)
        }
        None => Ok(false),
    }
}

enum RegisteredOwner {
    Msi(String),
    Managed,
}

fn registered_owner(parent: &Path) -> Result<Option<RegisteredOwner>> {
    use winreg::{
        RegKey,
        enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE},
    };
    let parent = parent
        .canonicalize()
        .context("cannot resolve the installed MicCamWatch directory")?;
    for hive in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        let root = RegKey::predef(hive);
        for view in [
            r"Software\Microsoft\Windows\CurrentVersion\Uninstall",
            r"Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
        ] {
            let Ok(uninstall) = root.open_subkey(view) else {
                continue;
            };
            for key in uninstall.enum_keys() {
                let key = key?;
                let product = uninstall.open_subkey(&key)?;
                if product
                    .get_value::<String, _>("DisplayName")
                    .ok()
                    .as_deref()
                    != Some("MicCamWatch")
                {
                    continue;
                }
                let location = product
                    .get_value::<String, _>("InstallLocation")
                    .ok()
                    .map(PathBuf::from);
                let standard = std::env::var_os("LOCALAPPDATA")
                    .map(PathBuf::from)
                    .map(|path| path.join("Programs/MicCamWatch"));
                let matches = location
                    .as_ref()
                    .or(standard.as_ref())
                    .is_some_and(|location| {
                        location
                            .canonicalize()
                            .is_ok_and(|location| location == parent)
                    });
                if matches {
                    if valid_product_code(&key)
                        && product.get_value::<u32, _>("WindowsInstaller").ok() == Some(1)
                    {
                        return Ok(Some(RegisteredOwner::Msi(key)));
                    }
                    return Ok(Some(RegisteredOwner::Managed));
                }
            }
        }
    }
    Ok(None)
}

pub(crate) fn ensure_binary_update_allowed(current: &Path) -> Result<()> {
    let parent = current.parent().context("executable has no directory")?;
    match registered_owner(parent)? {
        Some(RegisteredOwner::Msi(product_code)) => bail!(
            "mcw update refuses binary-only replacement of this Windows Installer-managed installation (product {product_code}). Upgrade using the official MicCamWatch MSI from https://github.com/Roman-Cuisset/miccamwatch/releases/latest; run that MSI to upgrade this installed product so Windows Installer updates its version, repair metadata, PATH and shortcuts together with mcw/mcw-tray. No download, staging, frontend shutdown or installation changes were performed; no automatic MSI upgrade or elevation was attempted."
        ),
        Some(RegisteredOwner::Managed) => bail!(
            "mcw update refuses binary-only replacement of this registered MicCamWatch installation. Use its registered installer/manager to upgrade; for official MicCamWatch MSI installations, run the official MSI from https://github.com/Roman-Cuisset/miccamwatch/releases/latest so the manager retains ownership of the binaries, version, repair metadata, PATH and shortcuts. No download, staging, frontend shutdown or installation changes were performed; no automatic MSI upgrade or elevation was attempted."
        ),
        None => Ok(()),
    }
}
fn valid_product_code(value: &str) -> bool {
    value.len() == 38
        && value.starts_with('{')
        && value.ends_with('}')
        && value[1..37].bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

pub(crate) fn receipt_bytes(cli: &Path, tray: &Path, version: &str) -> Result<Vec<u8>> {
    serde_json::to_vec(&Receipt {
        format: 1,
        version: version.to_owned(),
        cli_sha256: digest(&mut File::open(cli)?)?,
        tray_sha256: digest(&mut File::open(tray)?)?,
    })
    .context("cannot encode portable installation receipt")
}

pub(crate) fn validate_update_receipt(current: &Path, tray: &Path) -> Result<()> {
    let path = current.with_file_name(RECEIPT);
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
        Ok(_) => {}
    }
    let mut snapshot = Snapshot::open(&path)?;
    ensure!(
        snapshot.file.metadata()?.len() <= 4096,
        "portable ownership receipt is too large"
    );
    let receipt: Receipt = serde_json::from_reader(&mut snapshot.file)?;
    let tray_matches = match File::open(tray) {
        Ok(mut file) => receipt.tray_sha256 == digest(&mut file)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
        Err(error) => return Err(error.into()),
    };
    ensure!(
        receipt.format == 1
            && receipt.version == env!("CARGO_PKG_VERSION")
            && receipt.cli_sha256 == digest(&mut File::open(current)?)?
            && tray_matches,
        "portable receipt or installed pair was modified; update refused before shutdown"
    );
    Ok(())
}
