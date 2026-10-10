//! Explicit intent outlives frontends. Only this MTA owner performs native mute writes.
use super::{
    PlatformMonitor,
    microphone_lock::{GuardEndpoints, NativeLockJournal},
};
use crate::{model::MicrophoneMuteState, windows_control};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::windows::{ffi::OsStrExt, process::CommandExt},
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, SyncSender},
    },
    thread,
    time::{Duration, Instant},
};
use windows::{
    Win32::{
        Media::Audio::{DEVICE_STATE_ACTIVE, Endpoints::IAudioEndpointVolume, eCapture},
        Storage::FileSystem::{MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW},
        System::Com::CLSCTX_ALL,
    },
    core::PCWSTR,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MicrophoneProtectionStatus {
    pub requested: bool,
    pub service_active: bool,
    pub mute_state: MicrophoneMuteState,
    pub endpoint_count: usize,
    pub hardware_mute_count: usize,
    pub corrections: u64,
    pub detail: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Intent {
    format: u32,
    generation: u64,
    requested: bool,
    release_pending: bool,
    next_lock: u64,
    #[serde(default)]
    automatic_token: u64,
    #[serde(default)]
    automatic_generation: u64,
    #[serde(default)]
    automatic_restore_pending: bool,
}
impl Default for Intent {
    fn default() -> Self {
        Self {
            format: 1,
            generation: 0,
            requested: false,
            release_pending: false,
            next_lock: 0,
            automatic_token: 0,
            automatic_generation: 0,
            automatic_restore_pending: false,
        }
    }
}
impl Intent {
    fn change(&mut self, requested: bool) -> Result<()> {
        self.generation = self
            .generation
            .checked_add(1)
            .context("microphone intent generation exhausted")?;
        self.requested = requested;
        self.release_pending = !requested;
        self.clear_automatic();
        Ok(())
    }
    fn permits(&self, generation: u64) -> bool {
        self.requested && self.generation == generation
    }
    fn permits_restore(&self, generation: u64, owned: u64, requested: u64) -> bool {
        !self.requested
            && !self.release_pending
            && self.generation == generation
            && owned != 0
            && owned == requested
    }
    fn clear_automatic(&mut self) {
        self.automatic_token = 0;
        self.automatic_generation = 0;
        self.automatic_restore_pending = false;
    }
    fn owns_automatic(&self, token: u64) -> bool {
        !self.requested
            && !self.release_pending
            && token != 0
            && self.automatic_token == token
            && self.automatic_generation == self.generation
    }
    fn handoff(&mut self, token: u64) -> bool {
        if !self.owns_automatic(token) {
            return false;
        }
        self.automatic_restore_pending = true;
        true
    }
}
struct Store {
    _lock: File,
    path: PathBuf,
}
fn intent_directory(
    root: &std::path::Path,
    identity: &windows_control::ControlIdentity,
) -> PathBuf {
    root.join("microphone-protection").join(format!(
        "{}-{}-{}",
        identity.user, identity.session, identity.scope
    ))
}
fn directory() -> Result<PathBuf> {
    Ok(intent_directory(
        &crate::settings::data_dir()?,
        &windows_control::current_identity()?,
    ))
}
fn bounded_lock(file: &File) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(()),
            Err(std::fs::TryLockError::WouldBlock) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(10))
            }
            Err(error) => return Err(error).context("microphone ownership lock unavailable"),
        }
    }
}
impl Store {
    fn open() -> Result<Self> {
        Self::open_at(directory()?)
    }
    fn open_at(directory: PathBuf) -> Result<Self> {
        fs::create_dir_all(&directory)?;
        // Rust filesystem calls support long paths; native MoveFileExW requires the
        // canonical verbatim form once SID/session/scope extend a deep data root.
        let directory = fs::canonicalize(directory)
            .context("canonical microphone intent directory unavailable")?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(directory.join("intent.lock"))?;
        bounded_lock(&lock)?;
        Ok(Self {
            _lock: lock,
            path: directory.join("intent.json"),
        })
    }
    fn load(&self) -> Result<Intent> {
        load_intent(&self.path)
    }
    fn save(&self, value: &Intent) -> Result<()> {
        static NONCE: AtomicU64 = AtomicU64::new(0);
        let temporary = self.path.with_extension(format!(
            "tmp-{}-{}",
            std::process::id(),
            NONCE.fetch_add(1, Ordering::Relaxed)
        ));
        let result = (|| -> Result<()> {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            file.write_all(&serde_json::to_vec(value)?)?;
            file.sync_all()?;
            drop(file);
            let from: Vec<u16> = temporary.as_os_str().encode_wide().chain(Some(0)).collect();
            let to: Vec<u16> = self.path.as_os_str().encode_wide().chain(Some(0)).collect();
            unsafe {
                MoveFileExW(
                    PCWSTR(from.as_ptr()),
                    PCWSTR(to.as_ptr()),
                    MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
                )?;
            }
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }
}
fn load_intent(path: &std::path::Path) -> Result<Intent> {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Intent::default()),
        Err(error) => return Err(error).context("microphone operational intent unreadable"),
    };
    ensure!(
        file.metadata()?.len() <= 4096,
        "microphone intent exceeds bounded record size"
    );
    let mut bytes = Vec::new();
    Read::by_ref(&mut file).take(4097).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 4096,
        "microphone intent grew beyond bounded record size"
    );
    let intent: Intent = serde_json::from_slice(&bytes)
        .context("unknown/corrupt microphone intent; no safe state can be asserted")?;
    ensure!(intent.format == 1, "unsupported microphone intent format");
    Ok(intent)
}
/// Read only after the updater reserves the native microphone resource.
/// A dormant request or unfinished restoration is still protection ownership.
pub(crate) fn ensure_update_inactive() -> Result<()> {
    let intent = load_intent(&directory()?.join("intent.json"))?;
    ensure!(
        !intent.requested
            && !intent.release_pending
            && intent.automatic_token == 0
            && intent.automatic_generation == 0
            && !intent.automatic_restore_pending,
        "microphone protection is requested or restoration is pending; update refused without releasing protection"
    );
    Ok(())
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum Operation {
    Status,
    Set {
        requested: bool,
    },
    Toggle,
    Apply {
        generation: u64,
    },
    Lock {
        token: u64,
        generation: u64,
        caller_pid: u32,
        caller_created: u64,
    },
    Restore {
        token: u64,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    version: String,
    operation: Operation,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Response {
    status: MicrophoneProtectionStatus,
    changed: usize,
    token: u64,
}
fn call(operation: Operation) -> Result<Response> {
    windows_control::request(
        "microphone",
        &Request {
            version: env!("CARGO_PKG_VERSION").into(),
            operation,
        },
    )
}
fn prepare_request(mut request: Request) -> Result<Request> {
    ensure!(
        request.version == env!("CARGO_PKG_VERSION"),
        "microphone owner version mismatch"
    );
    if matches!(request.operation, Operation::Set { .. } | Operation::Toggle) {
        // This runs only inside the broker, whose native-resource HANDLE lease remains held
        // until this transport worker joins. A foreign scope cannot commit this owner's intent.
        let store = Store::open()?;
        let mut intent = store.load()?;
        let requested = match request.operation {
            Operation::Set { requested } => requested,
            Operation::Toggle => !intent.requested,
            _ => unreachable!(),
        };
        intent.change(requested)?;
        store.save(&intent)?;
        request.operation = Operation::Apply {
            generation: intent.generation,
        };
    }
    Ok(request)
}
fn spawn_owner() -> Result<()> {
    let mut executable = std::env::current_exe()?;
    if executable
        .file_stem()
        .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("mcw-tray"))
    {
        executable.set_file_name("mcw.exe");
    }
    ensure!(
        executable.is_file(),
        "paired current-version mcw.exe is unavailable"
    );
    Command::new(executable)
        .arg("__microphone-guard")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(0x08000000 | 0x00000008)
        .spawn()
        .context("start temporary microphone guard")?;
    Ok(())
}
fn connect_owner() -> Result<()> {
    // Each caller holds the request lock through startup AND its intent RPC.
    if call(Operation::Status).is_ok() {
        return Ok(());
    }
    ensure!(
        !windows_control::resource_conflict("microphone")?,
        "another operational namespace owns microphone hardware; protection activation/release cannot mutate it"
    );
    spawn_owner()?;
    let deadline = Instant::now() + Duration::from_secs(4);
    let mut last;
    loop {
        match call(Operation::Status) {
            Ok(_) => return Ok(()),
            Err(error) => last = error,
        }
        if Instant::now() >= deadline {
            return Err(last)
                .context("microphone intent retained, but guard activation could not be verified");
        }
        thread::sleep(Duration::from_millis(50));
    }
}
pub(super) fn set_requested(requested: bool) -> Result<usize> {
    let _transaction = windows_control::acquire_request_lock("microphone")?;
    connect_owner()?;
    Ok(call(Operation::Set { requested })?.changed)
}
pub(super) fn toggle_requested() -> Result<bool> {
    let _transaction = windows_control::acquire_request_lock("microphone")?;
    connect_owner()?;
    Ok(call(Operation::Toggle)?.status.requested)
}
pub fn resume_requested_microphone_protection() -> Result<()> {
    let _transaction = windows_control::acquire_request_lock("microphone")?;
    let intent = load_intent(&directory()?.join("intent.json"))?;
    if intent.requested || intent.release_pending || intent.automatic_restore_pending {
        connect_owner()?;
        if intent.automatic_restore_pending && !intent.requested && !intent.release_pending {
            call(Operation::Restore {
                token: intent.automatic_token,
            })?;
        } else {
            call(Operation::Apply {
                generation: intent.generation,
            })?;
        }
    }
    Ok(())
}
fn append_observation_detail(detail: &mut String, message: std::fmt::Arguments<'_>) {
    use std::fmt::Write as _;
    if detail.len() >= 8192 {
        return;
    }
    if !detail.is_empty() {
        detail.push_str("; ");
    }
    let _ = detail.write_fmt(message);
    windows_control::truncate_detail(detail, 8192);
}

fn observation(
    monitor: &PlatformMonitor,
    requested: bool,
    active: bool,
    corrections: u64,
    error: Option<String>,
) -> MicrophoneProtectionStatus {
    let mut status = MicrophoneProtectionStatus {
        requested,
        service_active: active,
        mute_state: MicrophoneMuteState::Unavailable,
        endpoint_count: 0,
        hardware_mute_count: 0,
        corrections,
        detail: error,
    };
    let mut detail = status.detail.take().unwrap_or_default();
    windows_control::truncate_detail(&mut detail, 8192);
    let mut enumeration_known = false;
    let mut hardware_seen = 0;
    let observed = (|| -> Result<()> {
        let collection = unsafe {
            monitor
                .enumerator
                .EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE)?
        };
        let count = unsafe { collection.GetCount()? };
        enumeration_known = true;
        status.endpoint_count = count as usize;
        ensure!(
            count <= 256,
            "microphone endpoint observation overload: {count}"
        );
        let mut muted = 0;
        let mut read_failed = false;
        for index in 0..count {
            let endpoint = (|| -> Result<IAudioEndpointVolume> {
                let device = unsafe { collection.Item(index)? };
                Ok(unsafe { device.Activate(CLSCTX_ALL, None)? })
            })();
            let volume = match endpoint {
                Ok(volume) => volume,
                Err(error) => {
                    read_failed = true;
                    append_observation_detail(
                        &mut detail,
                        format_args!(
                            "endpoint {index}: activation/observation unsupported or unknown: {error:#}"
                        ),
                    );
                    continue;
                }
            };
            match unsafe { volume.GetMute() } {
                Ok(value) => muted += usize::from(value.as_bool()),
                Err(error) => {
                    read_failed = true;
                    append_observation_detail(
                        &mut detail,
                        format_args!("endpoint {index}: mute observation unknown: {error}"),
                    );
                }
            }
            match unsafe { volume.QueryHardwareSupport() } {
                Ok(bits) => {
                    hardware_seen += 1;
                    status.hardware_mute_count += usize::from(bits & 2 != 0);
                }
                Err(error) => append_observation_detail(
                    &mut detail,
                    format_args!("endpoint {index}: hardware mute capability unknown: {error}"),
                ),
            }
        }
        if !read_failed && count != 0 {
            status.mute_state = if muted == count as usize {
                MicrophoneMuteState::Muted
            } else if muted == 0 {
                MicrophoneMuteState::Unmuted
            } else {
                MicrophoneMuteState::Mixed
            };
        }
        Ok(())
    })();
    if let Err(error) = observed {
        append_observation_detail(
            &mut detail,
            format_args!("microphone observation unknown/partial: {error:#}"),
        );
    }
    if requested && !active {
        append_observation_detail(
            &mut detail,
            format_args!(
                "Protection requested but service unavailable; protection health is not provable"
            ),
        );
    }
    if requested && !matches!(status.mute_state, MicrophoneMuteState::Muted) {
        append_observation_detail(
            &mut detail,
            format_args!("Desired protection is not verified on all active endpoints"),
        );
    }
    if !enumeration_known {
        append_observation_detail(
            &mut detail,
            format_args!("Active endpoint count is unknown; zero is not a proven absence"),
        );
    } else if status.endpoint_count == 0 {
        append_observation_detail(
            &mut detail,
            format_args!(
                "No active capture endpoints observed; future compatible inputs follow requested protection"
            ),
        );
    }
    let coverage = if !enumeration_known || hardware_seen != status.endpoint_count {
        " Hardware capabilities of some inputs are unknown; unverified inputs have software-only or unknown coverage;"
    } else if status.hardware_mute_count < status.endpoint_count {
        " Remaining observed inputs are software-mute only;"
    } else {
        ""
    };
    append_observation_detail(
        &mut detail,
        format_args!(
            "{} of {} observed inputs advertise hardware mute;{coverage} Endpoint mute is not capture permission; exclusive/unsupported capture paths may bypass software mute",
            status.hardware_mute_count, status.endpoint_count
        ),
    );
    status.detail = Some(detail);
    status
}
pub(super) fn status(monitor: &PlatformMonitor) -> Result<MicrophoneProtectionStatus> {
    // Never spawn a service or change hardware from a status/doctor query.
    match call(Operation::Status) {
        Ok(response) => Ok(response.status),
        Err(service_error) => {
            let conflict = match windows_control::resource_conflict("microphone") {
                Ok(true) => "; another operational namespace owns microphone hardware; this namespace cannot adopt/control that guard".to_owned(),
                Ok(false) => String::new(),
                Err(error) => format!("; native hardware ownership unknown: {error:#}"),
            };
            match load_intent(&directory()?.join("intent.json")) {
                Ok(intent) => {
                    let automatic = if intent.automatic_token != 0 {
                        "; automatic owned restoration is unresolved; original state cannot be proven while owner is unavailable"
                    } else {
                        ""
                    };
                    let endpoint_absent = service_error
                        .downcast_ref::<std::io::Error>()
                        .is_some_and(|error| error.raw_os_error() == Some(2));
                    let detail = if !intent.requested
                        && intent.automatic_token == 0
                        && conflict.is_empty()
                        && endpoint_absent
                    {
                        // A released intention needs no resident owner. Its absent
                        // endpoint is normal, not degraded protection health.
                        None
                    } else {
                        Some(format!(
                            "Guard unavailable: {service_error:#}{conflict}{automatic}"
                        ))
                    };
                    Ok(observation(monitor, intent.requested, false, 0, detail))
                }
                Err(error) => Ok(observation(
                    monitor,
                    true,
                    false,
                    0,
                    Some(format!(
                        "Desired intent unknown, not safe: {error:#}; guard unavailable: {service_error:#}{conflict}"
                    )),
                )),
            }
        }
    }
}
#[derive(Default)]
pub(crate) struct MicrophoneLockJournal {
    token: u64,
}
impl MicrophoneLockJournal {
    pub(crate) fn invalidate_manual(&mut self) {
        self.token = 0;
    }
}
impl Drop for MicrophoneLockJournal {
    fn drop(&mut self) {
        if self.token != 0
            && let Err(error) = handoff_token(self.token)
        {
            eprintln!(
                "automatic microphone ownership handoff failed; token {} unresolved: {error:#}",
                self.token
            );
        }
    }
}
fn handoff_token(token: u64) -> Result<bool> {
    let store = Store::open()?;
    let mut intent = store.load()?;
    if !intent.handoff(token) {
        return Ok(false);
    }
    store.save(&intent)?;
    Ok(true)
}
impl PlatformMonitor {
    pub(crate) fn mute_for_lock(&self, journal: &mut MicrophoneLockJournal) -> Result<usize> {
        let _transaction = windows_control::acquire_request_lock("microphone")?;
        ensure!(
            journal.token == 0,
            "previous microphone policy ownership is unresolved"
        );
        connect_owner()?;
        let (token, generation) = {
            let store = Store::open()?;
            let mut intent = store.load()?;
            if intent.requested {
                return Ok(0);
            }
            ensure!(!intent.release_pending, "explicit release is unresolved");
            intent.next_lock = intent
                .next_lock
                .checked_add(1)
                .context("microphone lock token exhausted")?;
            store.save(&intent)?;
            (intent.next_lock, intent.generation)
        };
        journal.token = token;
        let response = call(Operation::Lock {
            token,
            generation,
            caller_pid: std::process::id(),
            caller_created: windows_control::current_process_created()?,
        })?;
        journal.token = response.token;
        Ok(response.changed)
    }
    pub(crate) fn restore_after_lock(&self, journal: &mut MicrophoneLockJournal) -> Result<usize> {
        if journal.token == 0 {
            return Ok(0);
        }
        let response = match call(Operation::Restore {
            token: journal.token,
        }) {
            Ok(response) => response,
            Err(error) => {
                // Relinquishment can legitimately report an error after ending ownership.
                // Clear only a positively disproven token; unreadable/unknown ledgers retain it.
                if let Ok(path) = directory()
                    && let Ok(intent) = load_intent(&path.join("intent.json"))
                    && !intent.owns_automatic(journal.token)
                {
                    journal.token = 0;
                }
                return Err(error);
            }
        };
        journal.token = response.token;
        Ok(response.changed)
    }
    pub(crate) fn finish_after_lock(&self, journal: &mut MicrophoneLockJournal) -> Result<()> {
        if journal.token == 0 {
            return Ok(());
        }
        let handoff = handoff_token(journal.token);
        if matches!(handoff, Ok(false)) {
            journal.token = 0;
            return Ok(());
        }
        // Even a failed durable save still gets a matching broker restore attempt.
        match self.restore_after_lock(journal) {
            Ok(_) => Ok(()),
            Err(error) => match handoff {
                Ok(true) => {
                    Err(error.context("automatic restore remains durably pending in the broker"))
                }
                Err(handoff) => Err(anyhow::anyhow!(
                    "automatic restore failed: {error:#}; durable handoff also failed: {handoff:#}; ownership token retained"
                )),
                Ok(false) => unreachable!(),
            },
        }
    }
}
struct Job {
    request: Request,
    reply: SyncSender<Result<Response>>,
}
#[derive(Default)]
struct ReconcileBudget {
    generation: Option<u64>,
    failures: u8,
    next_attempt: Option<Instant>,
}
impl ReconcileBudget {
    fn ready(&mut self, generation: u64, wake: bool, now: Instant) -> bool {
        if self.generation != Some(generation) || wake {
            self.generation = Some(generation);
            self.failures = 0;
            self.next_attempt = None;
        }
        self.next_attempt.is_none_or(|deadline| now >= deadline)
    }
    fn failed(&mut self, now: Instant) {
        self.failures = self.failures.saturating_add(1);
        self.next_attempt = Some(now + Duration::from_secs((1u64 << self.failures.min(5)).min(30)));
    }
    fn succeeded(&mut self) {
        self.failures = 0;
        self.next_attempt = None;
    }
}
struct Owner {
    monitor: PlatformMonitor,
    guard: Option<GuardEndpoints>,
    wake: SyncSender<()>,
    journal: NativeLockJournal,
    token: u64,
    auto_generation: u64,
    automatic_lease: Option<windows_control::ProcessLease>,
    corrections: u64,
    detail: Option<String>,
    retries: ReconcileBudget,
}
impl Drop for Owner {
    fn drop(&mut self) {
        self.guard = None;
        self.journal.invalidate_manual();
        self.automatic_lease = None;
    }
}
impl Owner {
    fn guard(&mut self) -> Result<&mut GuardEndpoints> {
        if self.guard.is_none() {
            self.guard = Some(GuardEndpoints::new(&self.monitor, self.wake.clone())?);
        }
        Ok(self.guard.as_mut().unwrap())
    }
    fn apply(&mut self, store: &Store, intent: &mut Intent, generation: u64) -> Result<usize> {
        ensure!(
            intent.generation == generation,
            "microphone request superseded by newer explicit intent"
        );
        if intent.permits(generation) {
            self.journal.invalidate_manual();
            self.token = 0;
            self.automatic_lease = None;
            let (changed, result) = self.guard()?.enforce();
            self.corrections = self.corrections.saturating_add(changed as u64);
            result?;
            Ok(changed)
        } else if intent.release_pending {
            self.journal.invalidate_manual();
            self.token = 0;
            self.automatic_lease = None;
            let changed = self.guard()?.explicit_release()?;
            intent.release_pending = false;
            store.save(intent)?;
            self.guard = None;
            Ok(changed)
        } else {
            Ok(0)
        }
    }
    fn response(&self, intent: &Intent, changed: usize) -> Response {
        let mut detail = self.detail.clone();
        if intent.release_pending {
            let pending = "Explicit release remains pending; SDK release success is not proven";
            detail = Some(match detail {
                Some(detail) => format!("{detail}; {pending}"),
                None => pending.to_owned(),
            });
        }
        if intent.automatic_token != 0 {
            let ownership = if self.token != intent.automatic_token {
                "Automatic originals unavailable after owner loss; restoration cannot be proven"
            } else if intent.automatic_restore_pending {
                "Matching automatic restoration remains pending in the native owner"
            } else {
                "Automatic policy journal retains original per-endpoint state"
            };
            detail = Some(match detail {
                Some(detail) => format!("{detail}; {ownership}"),
                None => ownership.to_owned(),
            });
        }
        Response {
            status: observation(
                &self.monitor,
                intent.requested,
                true,
                self.corrections,
                detail,
            ),
            changed,
            token: self.token,
        }
    }
    fn handle(&mut self, request: Request) -> Result<Response> {
        ensure!(
            request.version == env!("CARGO_PKG_VERSION"),
            "microphone owner version mismatch"
        );
        let store = Store::open()?;
        let mut intent = store.load()?;
        let ownership_before = self.token;
        let result = (|| -> Result<usize> {
            match request.operation {
                Operation::Status => Ok(0),
                Operation::Set { .. } | Operation::Toggle => {
                    anyhow::bail!("uncommitted microphone intent request")
                }
                Operation::Apply { generation } => {
                    self.retries = ReconcileBudget::default();
                    self.apply(&store, &mut intent, generation)
                }
                Operation::Lock { .. } if intent.requested => {
                    self.journal.invalidate_manual();
                    self.token = 0;
                    self.automatic_lease = None;
                    Ok(0)
                }
                Operation::Lock {
                    token,
                    generation,
                    caller_pid,
                    caller_created,
                } => {
                    ensure!(
                        !intent.release_pending,
                        "explicit microphone release remains unresolved"
                    );
                    ensure!(
                        self.token == 0,
                        "another microphone automatic lock journal owns changes"
                    );
                    ensure!(
                        intent.automatic_token == 0,
                        "previous automatic original state remains unresolved"
                    );
                    let lifetime = windows_control::invoker_lease(
                        &windows_control::current_identity()?,
                        caller_pid,
                        caller_created,
                    )?;
                    ensure!(
                        intent.generation == generation && token > 0 && token <= intent.next_lock,
                        "automatic lock superseded by newer microphone intent"
                    );
                    // Caller already retained the token, including on partial response failure.
                    self.token = token;
                    self.auto_generation = generation;
                    self.automatic_lease = Some(lifetime);
                    intent.automatic_token = token;
                    intent.automatic_generation = generation;
                    intent.automatic_restore_pending = false;
                    store.save(&intent)?;
                    let result = self.monitor.native_mute_for_lock(&mut self.journal);
                    if self.journal.is_empty() {
                        self.token = 0;
                        self.automatic_lease = None;
                        intent.clear_automatic();
                        store.save(&intent)?;
                    }
                    result
                }
                Operation::Restore { token } => {
                    if intent.owns_automatic(token) {
                        self.retries = ReconcileBudget::default();
                        self.restore_automatic(&store, &mut intent, token)
                    } else {
                        if intent.requested
                            || intent.release_pending
                            || intent.generation != self.auto_generation
                        {
                            self.journal.invalidate_manual();
                            self.token = 0;
                            self.automatic_lease = None;
                        }
                        Ok(0)
                    }
                }
            }
        })();
        match result {
            Ok(changed) => {
                let completed = matches!(request.operation, Operation::Apply { .. })
                    || matches!(request.operation, Operation::Lock { .. } if !intent.requested)
                    || matches!(request.operation, Operation::Restore { token } if token != 0 && token == ownership_before
                        && !intent.requested && !intent.release_pending);
                if completed {
                    self.detail = None;
                }
                let mut response = self.response(&intent, changed);
                if matches!(request.operation, Operation::Status)
                    || matches!(request.operation, Operation::Restore { token } if token != self.token)
                {
                    // A stale consumer must not adopt another policy consumer's journal.
                    response.token = 0;
                }
                Ok(response)
            }
            Err(error) => {
                self.detail = Some(format!("{error:#}"));
                Err(error)
            }
        }
    }
    fn restore_automatic(
        &mut self,
        store: &Store,
        intent: &mut Intent,
        token: u64,
    ) -> Result<usize> {
        ensure!(
            intent.handoff(token),
            "automatic restore token/generation no longer owns changes"
        );
        store.save(intent)?;
        ensure!(
            intent.permits_restore(self.auto_generation, self.token, token)
                && !self.journal.is_empty(),
            "automatic originals unavailable after owner loss; restoration cannot be proven"
        );
        let result = self.monitor.native_restore_after_lock(&mut self.journal);
        if self.journal.is_empty() {
            self.token = 0;
            self.automatic_lease = None;
            intent.clear_automatic();
            store.save(intent)?;
        }
        result
    }
    fn reconcile(&mut self, wake: bool) -> Result<bool> {
        let store = Store::open()?;
        let mut intent = store.load()?;
        if !intent.requested && !intent.release_pending {
            if let Some(lease) = &self.automatic_lease
                && self.token != 0
                && lease.exited()?
            {
                ensure!(
                    intent.handoff(self.token),
                    "exited automatic consumer no longer owns its journal"
                );
                store.save(&intent)?;
            }
            ensure!(
                intent.automatic_token == 0 || self.token == intent.automatic_token,
                "automatic originals unavailable after owner loss; no blind restoration is allowed"
            );
        }
        if intent.requested || intent.release_pending || intent.automatic_restore_pending {
            let generation = intent.generation;
            if !self.retries.ready(generation, wake, Instant::now()) {
                return Ok(true);
            }
            let result = if intent.requested || intent.release_pending {
                self.apply(&store, &mut intent, generation).map(|_| ())
            } else {
                let token = intent.automatic_token;
                self.restore_automatic(&store, &mut intent, token)
                    .map(|_| ())
            };
            match result {
                Ok(_) => {
                    self.retries.succeeded();
                    self.detail = None;
                }
                Err(error) => {
                    self.retries.failed(Instant::now());
                    return Err(error);
                }
            }
        } else {
            self.guard = None;
        }
        Ok(intent.requested
            || intent.release_pending
            || intent.automatic_restore_pending
            || self.token != 0)
    }
}
pub fn run_microphone_protection_service() -> Result<()> {
    let directory = directory()?;
    fs::create_dir_all(&directory)?;
    let lease_name = format!("owner-{}.lock", windows_control::session_id()?);
    let lease = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(directory.join(lease_name))?;
    match lease.try_lock() {
        Ok(()) => {}
        Err(std::fs::TryLockError::WouldBlock) => return Ok(()),
        Err(error) => return Err(error).context("microphone service ownership unavailable"),
    }
    let identity = windows_control::current_identity()?;
    let _hardware_ownership = windows_control::acquire_resource_for("microphone", &identity)?;
    let (wake, wakes) = mpsc::sync_channel(1);
    let (jobs, pending) = mpsc::sync_channel::<Job>(16);
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = Arc::clone(&stop);
    let (ended, worker_result) = mpsc::sync_channel(1);
    let worker = thread::spawn(move || {
        let result = windows_control::serve("microphone", &worker_stop, |request| {
            let request = prepare_request(request)?;
            let (reply, result) = mpsc::sync_channel(1);
            jobs.try_send(Job { request, reply })
                .context("microphone command queue full/stopped")?;
            result
                .recv_timeout(Duration::from_secs(2))
                .context("microphone owner response deadline")?
        });
        worker_stop.store(true, Ordering::Release);
        let _ = ended.send(result);
    });
    // The sole SDK apartment owns registrations, native endpoint references and automatic originals.
    let mut owner = match PlatformMonitor::new_microphone_owner() {
        Ok(monitor) => Owner {
            monitor,
            guard: None,
            wake,
            journal: NativeLockJournal::default(),
            token: 0,
            auto_generation: 0,
            automatic_lease: None,
            corrections: 0,
            detail: None,
            retries: ReconcileBudget::default(),
        },
        Err(error) => {
            stop.store(true, Ordering::Release);
            let _ = worker.join();
            return Err(error);
        }
    };
    let started = Instant::now();
    let mut next_retry = Instant::now();
    while !stop.load(Ordering::Acquire) {
        match pending.recv_timeout(Duration::from_millis(100)) {
            Ok(job) => {
                let result = owner.handle(job.request);
                let _ = job.reply.send(result);
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        let dirty = wakes.try_recv().is_ok();
        if dirty || Instant::now() >= next_retry {
            match owner.reconcile(dirty) {
                Ok(active) => {
                    if !active && started.elapsed() > Duration::from_secs(2) {
                        break;
                    }
                    next_retry = Instant::now() + Duration::from_secs(1);
                }
                Err(error) => {
                    let mut detail =
                        format!("Protection reconciliation partial/unknown: {error:#}");
                    detail.push_str("; bounded slow SDK retry remains scheduled; desired/owned restore intent retained");
                    windows_control::truncate_detail(&mut detail, 8192);
                    owner.detail = Some(detail);
                    next_retry = Instant::now() + Duration::from_secs(1);
                }
            }
        }
    }
    stop.store(true, Ordering::Release);
    drop(owner);
    worker
        .join()
        .map_err(|_| anyhow::anyhow!("microphone transport worker panicked"))?;
    worker_result
        .recv()
        .context("microphone transport worker result missing")??;
    drop(lease);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn desired_toggle_ignores_foreign_false_and_release_invalidates_queued_generation() {
        let mut intent = Intent::default();
        intent.change(true).unwrap();
        let queued = intent.generation;
        assert!(intent.permits(queued));
        let desired_toggle = !intent.requested;
        assert!(!desired_toggle);
        intent.change(desired_toggle).unwrap();
        assert!(!intent.permits(queued));
        assert!(intent.release_pending);
    }
    #[test]
    fn operational_ledger_rejects_unknown_formats_and_never_defaults_corruption_to_safe() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("intent.json");
        fs::write(&path, b"{}").unwrap();
        assert!(load_intent(&path).is_err());
        fs::write(&path, br#"{"format":0,"generation":0,"requested":false,"release_pending":false,"next_lock":0}"#).unwrap();
        assert!(load_intent(&path).is_err());
        let mut intent = Intent::default();
        intent.change(true).unwrap();
        fs::write(&path, serde_json::to_vec(&intent).unwrap()).unwrap();
        assert!(load_intent(&path).unwrap().requested);
    }
    #[test]
    fn generation_cannot_wrap_into_stale_authority() {
        let mut intent = Intent {
            generation: u64::MAX,
            ..Intent::default()
        };
        assert!(intent.change(true).is_err());
        assert!(!intent.requested);
    }
    #[test]
    fn automatic_restore_cannot_release_manual_protection_or_newer_release_generation() {
        let mut intent = Intent::default();
        assert!(intent.permits_restore(0, 5, 5));
        assert!(!intent.permits_restore(0, 5, 4));
        intent.change(true).unwrap();
        assert!(!intent.permits_restore(0, 5, 5));
        intent.change(false).unwrap();
        assert!(!intent.permits_restore(0, 5, 5));
        intent.release_pending = false;
        assert!(!intent.permits_restore(0, 5, 5));
        assert!(intent.permits_restore(intent.generation, 6, 6));
    }
    #[test]
    fn atomic_operational_release_is_visible_before_queued_protection_can_write() {
        let directory = tempfile::tempdir().unwrap();
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(directory.path().join("intent.lock"))
            .unwrap();
        bounded_lock(&lock).unwrap();
        let competing = OpenOptions::new()
            .read(true)
            .write(true)
            .open(directory.path().join("intent.lock"))
            .unwrap();
        assert!(matches!(
            competing.try_lock(),
            Err(std::fs::TryLockError::WouldBlock)
        ));
        let store = Store {
            _lock: lock,
            path: directory.path().join("intent.json"),
        };
        let mut intent = Intent::default();
        intent.change(true).unwrap();
        store.save(&intent).unwrap();
        let queued_generation = store.load().unwrap().generation;
        intent.change(false).unwrap();
        store.save(&intent).unwrap();
        let release = store.load().unwrap();
        assert!(!release.permits(queued_generation));
        assert!(!release.requested);
        assert!(release.release_pending);
        drop(store);
        competing.try_lock().unwrap();
    }
    #[test]
    fn failed_reconciliation_keeps_slow_recovery_without_native_callbacks() {
        let mut retry = ReconcileBudget::default();
        let now = Instant::now();
        assert!(retry.ready(4, false, now));
        retry.failed(now);
        assert!(!retry.ready(4, false, now + Duration::from_secs(1)));
        assert!(retry.ready(4, false, now + Duration::from_secs(2)));
        retry.failed(now + Duration::from_secs(2));
        assert!(!retry.ready(4, false, now + Duration::from_secs(3)));
        assert!(retry.ready(4, false, now + Duration::from_secs(6)));
        assert!(retry.ready(5, false, now + Duration::from_secs(3)));
        for _ in 0..100 {
            retry.failed(now);
        }
        assert!(retry.ready(5, false, now + Duration::from_secs(30)));
    }
    #[test]
    fn operational_intents_are_isolated_by_verified_user_session_and_data_scope() {
        let root = tempfile::tempdir().unwrap();
        let a = windows_control::ControlIdentity {
            user: "S-1-5-21-1".into(),
            session: 1,
            scope: "a".repeat(64),
        };
        let mut b = a.clone();
        b.session = 2;
        let a_directory = intent_directory(root.path(), &a);
        let b_directory = intent_directory(root.path(), &b);
        assert_ne!(a_directory, b_directory);
        fs::create_dir_all(&a_directory).unwrap();
        fs::create_dir_all(&b_directory).unwrap();
        let open = |directory: &std::path::Path| {
            let lock = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(directory.join("intent.lock"))
                .unwrap();
            bounded_lock(&lock).unwrap();
            Store {
                _lock: lock,
                path: directory.join("intent.json"),
            }
        };
        let a_store = open(&a_directory);
        let b_store = open(&b_directory);
        let mut a_intent = Intent::default();
        a_intent.change(true).unwrap();
        a_store.save(&a_intent).unwrap();
        let mut b_intent = Intent::default();
        b_intent.change(false).unwrap();
        b_store.save(&b_intent).unwrap();
        assert!(a_store.load().unwrap().requested);
        assert!(!b_store.load().unwrap().requested);
        b = a.clone();
        b.scope = "b".repeat(64);
        assert_ne!(a_directory, intent_directory(root.path(), &b));
    }
    #[test]
    fn matching_shutdown_handoff_survives_reopen_but_cannot_release_new_manual_intent() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("intent.json");
        let lock_path = directory.path().join("intent.lock");
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .unwrap();
        bounded_lock(&lock).unwrap();
        let store = Store {
            _lock: lock,
            path: path.clone(),
        };
        let mut intent = Intent {
            generation: 7,
            next_lock: 12,
            automatic_token: 12,
            automatic_generation: 7,
            ..Intent::default()
        };
        store.save(&intent).unwrap();
        assert!(!intent.handoff(11));
        assert!(intent.handoff(12));
        store.save(&intent).unwrap();
        drop(store);
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&lock_path)
            .unwrap();
        bounded_lock(&lock).unwrap();
        let reopened = Store { _lock: lock, path };
        let mut recovered = reopened.load().unwrap();
        assert!(recovered.automatic_restore_pending);
        assert_eq!(recovered.automatic_token, 12);
        assert!(recovered.permits_restore(7, 12, 12));
        recovered.change(true).unwrap();
        reopened.save(&recovered).unwrap();
        assert!(!recovered.handoff(12));
        assert!(!recovered.permits_restore(7, 12, 12));
        assert!(reopened.load().unwrap().requested);
        assert_eq!(reopened.load().unwrap().automatic_token, 0);
    }
    #[test]
    fn atomic_intent_roundtrip_uses_verbatim_native_paths_beyond_max_path() {
        let root = tempfile::tempdir().unwrap();
        let mut directory = root.path().to_owned();
        for _ in 0..8 {
            directory.push("long-operational-data-directory");
        }
        assert!(directory.as_os_str().encode_wide().count() > 260);
        let store = Store::open_at(directory).unwrap();
        assert!(
            store
                .path
                .as_os_str()
                .to_string_lossy()
                .starts_with(r"\\?\")
        );
        let mut intent = Intent::default();
        intent.change(true).unwrap();
        store.save(&intent).unwrap();
        assert!(store.load().unwrap().requested);
        intent.change(false).unwrap();
        store.save(&intent).unwrap();
        let released = store.load().unwrap();
        assert!(!released.requested);
        assert!(released.release_pending);
        assert_eq!(released.generation, 2);
    }
}
