use crate::{
    collector::{CaptureCollector, CaptureScope},
    config::Policy,
    model::{
        Access, Activity, CollectorHealth, CollectorState, Confidence, Device, DiagnosticCheck,
        DiagnosticStatus, EnforcementDecision, Evidence, EvidenceKind, ProcessContext, Resource,
        Risk, Snapshot,
    },
};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, de::DeserializeOwned};
use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    os::unix::{
        fs::{DirBuilderExt, OpenOptionsExt},
        process::CommandExt,
    },
    path::{Component, Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        LazyLock, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub(crate) mod control;
pub(crate) mod signature;

const HELPER: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/MicCamWatchHelper.app/Contents/MacOS/MicCamWatchHelper"
));
const HELPER_PLIST: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/MicCamWatchHelper.app/Contents/Info.plist"
));
const HELPER_RESOURCES: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/MicCamWatchHelper.app/Contents/_CodeSignature/CodeResources"
));
const MAX_OUTPUT: u64 = 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(5);
const MAX_ARGUMENTS: usize = 65_536;
static BINARY: LazyLock<Result<Helper, String>> =
    LazyLock::new(|| Helper::install().map_err(|error| format!("{error:#}")));
static SCAN_ID: AtomicU64 = AtomicU64::new(0);

pub struct PlatformMonitor {
    policy: Policy,
    camera_inventory: Mutex<HashSet<String>>,
}

impl PlatformMonitor {
    pub fn new(policy: Policy) -> Result<Self> {
        Ok(Self {
            policy,
            camera_inventory: Mutex::new(HashSet::new()),
        })
    }

    pub(crate) fn set_policy(&mut self, policy: Policy) {
        // CoreAudio control originals live in the durable journal, not in policy.
        self.policy = policy;
    }

    pub fn snapshot(&self, scope: CaptureScope) -> Result<Snapshot> {
        // Neither passive API proves readiness; include_ready never synthesizes
        // capture sessions or an inactive camera/microphone observation.
        let mut snapshot = Snapshot {
            collectors: Vec::new(),
            accesses: Vec::new(),
            observation_gaps: Vec::new(),
        };
        if !scope.microphone && !scope.camera {
            return Ok(snapshot);
        }
        let mode = match (scope.microphone, scope.camera) {
            (true, true) => "both",
            (true, false) => "audio",
            (false, true) => "video",
            (false, false) => unreachable!(),
        };
        match observe(mode) {
            Ok(observation) => {
                if scope.microphone {
                    let audio = observation
                        .audio
                        .context("helper omitted CoreAudio result")?;
                    if !audio.available || audio.error.is_some() {
                        snapshot.observation_gaps.push(Resource::Microphone);
                    }
                    let mut detail = audio.error.or_else(|| {
                        (!audio.available)
                            .then(|| "CoreAudio process enumeration unavailable".to_owned())
                    });
                    let state = if !audio.available {
                        CollectorState::Unavailable
                    } else if detail.is_some() {
                        CollectorState::Degraded
                    } else {
                        CollectorState::Healthy
                    };
                    if audio.available {
                        let first = snapshot.accesses.len();
                        for (index, process) in audio.processes.into_iter().enumerate() {
                            if !process.has_identity() {
                                detail.get_or_insert_with(|| "active CoreAudio input client has no stable process identity".to_owned());
                            }
                            snapshot.accesses.push(microphone_access(process, index));
                        }
                        if let Some(error) =
                            self.assess_microphones(&mut snapshot.accesses[first..])
                        {
                            detail.get_or_insert(error);
                        }
                    }
                    snapshot.collectors.push(health(
                        "coreaudio_input",
                        if state == CollectorState::Healthy && detail.is_some() {
                            CollectorState::Degraded
                        } else {
                            state
                        },
                        detail,
                    ));
                }
                if scope.camera {
                    let video = observation
                        .video
                        .context("helper omitted CoreMediaIO camera result")?;
                    append_camera_observation_with_inventory(
                        &mut snapshot,
                        video,
                        &mut self
                            .camera_inventory
                            .lock()
                            .unwrap_or_else(|e| e.into_inner()),
                    );
                }
            }
            Err(error) => {
                let detail = format!("macOS capture helper unavailable: {error:#}");
                if scope.microphone {
                    snapshot.collectors.push(health(
                        "coreaudio_input",
                        CollectorState::Unavailable,
                        Some(detail.clone()),
                    ));
                }
                if scope.camera {
                    snapshot.collectors.push(health(
                        "coremediaio_video",
                        CollectorState::Unavailable,
                        Some(detail),
                    ));
                }
            }
        }
        Ok(snapshot)
    }

    pub fn devices(&self) -> Result<Vec<Device>> {
        let observation = observe("devices")?;
        let video = observation
            .video
            .context("helper omitted AVFoundation inventory")?;
        if let Some(error) = video.error {
            bail!("AVFoundation inventory failed: {error}");
        }
        let mut devices = observation
            .microphones
            .context("helper omitted microphone inventory")?
            .into_iter()
            .map(|device| Device {
                resource: Resource::Microphone,
                id: device.id,
                name: device.name,
            })
            .collect::<Vec<_>>();
        devices.extend(video.devices.into_iter().map(|device| Device {
            resource: Resource::Camera,
            id: device.id,
            name: device.name,
        }));
        Ok(devices)
    }

    pub fn doctor(&self) -> Vec<DiagnosticCheck> {
        let mut checks: Vec<DiagnosticCheck> = match self.snapshot(CaptureScope::all()) {
            Ok(snapshot) => snapshot
                .collectors
                .into_iter()
                .map(|collector| DiagnosticCheck {
                    name: collector.collector,
                    status: match collector.state {
                        CollectorState::Healthy => DiagnosticStatus::Ok,
                        CollectorState::Degraded => DiagnosticStatus::Warning,
                        CollectorState::Unavailable => DiagnosticStatus::Error,
                    },
                    detail: collector
                        .detail
                        .unwrap_or_else(|| "CoreAudio input processes accessible".to_owned()),
                })
                .collect(),
            Err(error) => vec![DiagnosticCheck {
                name: "macos_collectors",
                status: DiagnosticStatus::Error,
                detail: format!("cannot inspect macOS capture collectors: {error:#}"),
            }],
        };
        checks.push(match crate::privacy::camera_inspection(false) {
            Ok(inspection) => DiagnosticCheck {
                name: "camera_profile_inventory",
                status: if inspection.state == "unknown" {
                    DiagnosticStatus::Warning
                } else { DiagnosticStatus::Ok },
                detail: if inspection.administrator_required {
                    format!("{} Explicit read-only proposal: mcw camera inspect --admin. No automatic elevation.", inspection.detail)
                } else { inspection.detail },
            },
            Err(error) => DiagnosticCheck {
                name: "camera_profile_inventory",
                status: DiagnosticStatus::Error,
                detail: format!("Cannot read camera profile metadata: {error:#}; no administrator requirement inferred"),
            },
        });
        checks.push(match crate::notify::native_status() {
            Ok(state) => DiagnosticCheck {
                name: "notification_authorization",
                status: if state == "authorized" {
                    DiagnosticStatus::Ok
                } else {
                    DiagnosticStatus::Warning
                },
                detail: format!("macos_authorization={state}; banner_delivery=unverified"),
            },
            Err(error) => DiagnosticCheck {
                name: "notification_authorization",
                status: DiagnosticStatus::Warning,
                detail: format!(
                    "macos_authorization=unknown; banner_delivery=unverified; {error:#}"
                ),
            },
        });
        checks.push(DiagnosticCheck {
            name: "session_lock_api",
            status: DiagnosticStatus::Warning,
            detail: "Public macOS session APIs do not establish current screen-lock state. Administrator rights do not supply this missing API; automatic lock controls remain unavailable.".into(),
        });
        checks
    }

    fn apply_policy(&self, access: &mut Access) {
        (access.risk, access.enforcement) = crate::policy::assess_policy(
            &self.policy,
            &access.application,
            access.executable.as_deref(),
            access.signature.as_ref(),
            &mut access.evidence,
        );
    }

    fn assess_microphones(&self, accesses: &mut [Access]) -> Option<String> {
        let mut attributed = false;
        for access in accesses
            .iter_mut()
            .filter(|access| access.process.is_some())
        {
            let Some(path) = access.executable.as_deref() else {
                continue;
            };
            attributed = true;
            match signature::verify_signature_with_policy(path, self.policy.trust_policy) {
                Ok(signature) => access.signature = Some(signature),
                Err(error) => access.evidence.push(Evidence::new(
                    EvidenceKind::Signature,
                    "security_framework",
                    format!("Native executable signature evidence unavailable: {error:#}"),
                )),
            }
        }
        if !attributed {
            return None;
        }
        // The signing helper verifies a file, not a PID. Re-read libproc identity
        // through the passive input collector before attaching its verdict or
        // permitting path/name rules to authorize enforcement.
        let current = observe("audio").and_then(|observation| {
            let audio = observation
                .audio
                .context("helper omitted identity revalidation")?;
            if !audio.available {
                bail!(
                    "{}",
                    audio
                        .error
                        .as_deref()
                        .unwrap_or("CoreAudio identity revalidation unavailable")
                );
            }
            Ok(audio.processes)
        });
        let mut unstable = false;
        for access in accesses
            .iter_mut()
            .filter(|access| access.process.is_some())
        {
            if current.as_ref().is_ok_and(|processes| {
                processes
                    .iter()
                    .any(|process| process.matches_access(access))
            }) {
                self.apply_policy(access);
            } else {
                unstable = true;
                access.signature = None;
                access.evidence.push(Evidence::new(
                    EvidenceKind::Signature,
                    "libproc",
                    "Process birth/executable identity could not be revalidated after signature verification; no policy decision is authorized.",
                ));
            }
        }
        if unstable {
            Some(match current {
                Ok(_) => {
                    "CoreAudio process identity changed or disappeared during policy assessment"
                        .to_owned()
                }
                Err(error) => format!("CoreAudio identity revalidation unavailable: {error:#}"),
            })
        } else {
            None
        }
    }
}

fn health(
    collector: &'static str,
    state: CollectorState,
    detail: Option<String>,
) -> CollectorHealth {
    CollectorHealth {
        collector,
        state,
        detail,
    }
}

#[derive(Deserialize)]
struct Observation {
    audio: Option<AudioResult>,
    video: Option<VideoResult>,
    microphones: Option<Vec<InventoryDevice>>,
}

#[derive(Deserialize)]
struct AudioResult {
    processes: Vec<AudioProcess>,
    error: Option<String>,
    available: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AudioProcess {
    pid: Option<u32>,
    start_seconds: Option<u64>,
    start_microseconds: Option<u64>,
    executable: Option<String>,
}

impl AudioProcess {
    fn has_identity(&self) -> bool {
        matches!((self.pid, self.start_seconds, self.start_microseconds, &self.executable),
            (Some(pid), Some(seconds), Some(micros), Some(path))
            if pid > 0 && pid <= i32::MAX as u32 && seconds > 0 && micros < 1_000_000
                && !path.contains('\0')
                && Path::new(path).is_absolute()
                && Path::new(path).file_name().is_some()
                && !Path::new(path).components().any(|part| part == Component::ParentDir))
    }

    fn matches_access(&self, access: &Access) -> bool {
        if !self.has_identity() || self.pid != access.pid || self.executable != access.executable {
            return false;
        }
        let Some(context) = &access.process else {
            return false;
        };
        let Some(identity) = context.instance_id.strip_prefix("macos:microphone:") else {
            return false;
        };
        let mut parts = identity.split(':');
        parts.next().and_then(|part| part.parse::<u32>().ok()) == self.pid
            && parts.next().and_then(|part| part.parse::<u64>().ok()) == self.start_seconds
            && parts.next().and_then(|part| part.parse::<u64>().ok()) == self.start_microseconds
            && parts.next().is_none()
    }
}

#[derive(Deserialize)]
struct VideoResult {
    devices: Vec<CameraDevice>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct CameraDevice {
    id: String,
    name: String,
    #[serde(rename = "runningSomewhere")]
    running_somewhere: Option<bool>,
}

#[derive(Deserialize)]
struct InventoryDevice {
    id: String,
    name: String,
}

fn microphone_access(process: AudioProcess, index: usize) -> Access {
    let verified = process.has_identity();
    let instance = if verified {
        format!(
            "macos:microphone:{}:{}:{}",
            process.pid.unwrap(),
            process.start_seconds.unwrap(),
            process.start_microseconds.unwrap()
        )
    } else {
        format!("macos:microphone:unattributed:{index}")
    };
    let executable = verified.then_some(process.executable).flatten();
    let application = executable
        .as_deref()
        .and_then(|path| std::path::Path::new(path).file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Unknown CoreAudio input client".to_owned());
    Access {
        key: instance.clone(),
        resource: Resource::Microphone,
        activity: Activity::Active,
        risk: Risk::Unexplained,
        confidence: if verified {
            Confidence::High
        } else {
            Confidence::Medium
        },
        enforcement: EnforcementDecision::Unknown,
        application,
        pid: verified.then_some(process.pid).flatten(),
        parent_pid: None,
        parent_name: None,
        executable,
        signature: None,
        device: None,
        started_at: None,
        modules: Vec::new(),
        evidence: vec![Evidence::new(
            EvidenceKind::LiveApi,
            "coreaudio_input",
            "AudioHardwareProcess.isRunningInput=true",
        )],
        process: verified.then_some(ProcessContext {
            instance_id: instance,
            ..ProcessContext::default()
        }),
    }
}

fn append_camera_observation_with_inventory(
    snapshot: &mut Snapshot,
    video: VideoResult,
    previous: &mut HashSet<String>,
) {
    let current: HashSet<_> = video
        .devices
        .iter()
        .map(|device| device.id.clone())
        .collect();
    let disappeared = !previous.is_subset(&current);
    *previous = current;
    append_camera_observation(snapshot, video);
    if disappeared {
        if !snapshot.observation_gaps.contains(&Resource::Camera) {
            snapshot.observation_gaps.push(Resource::Camera);
        }
        if let Some(collector) = snapshot.collectors.last_mut() {
            collector.detail = Some("Camera inventory changed: a previously observed device disappeared; capture stop time is unknown".to_owned());
        }
    }
}

fn append_camera_observation(snapshot: &mut Snapshot, video: VideoResult) {
    let has_running_state = video
        .devices
        .iter()
        .any(|camera| camera.running_somewhere.is_some());
    let unknown_state = video
        .devices
        .iter()
        .any(|camera| camera.running_somewhere.is_none());
    if video.error.is_some() || video.devices.is_empty() || unknown_state {
        snapshot.observation_gaps.push(Resource::Camera);
    }
    let detail = video.error.unwrap_or_else(|| {
        if video.devices.is_empty() {
            "AVFoundation camera discovery returned no devices; camera activity is unknown; no camera permission requested".to_owned()
        } else if unknown_state {
            "CoreMediaIO camera running-state unavailable; camera activity is unknown; no camera permission requested".to_owned()
        } else {
            "CoreMediaIO reports device running-state without client identity or proof of frame flow; no camera permission requested".to_owned()
        }
    });
    for camera in video.devices {
        if camera.running_somewhere == Some(true) {
            snapshot.accesses.push(camera_access(&camera));
        }
    }
    // Attribution/frame-flow limitations do not invalidate known running-state.
    // Empty discovery, missing states and API errors independently suppress STOP.
    snapshot.collectors.push(health(
        "coremediaio_video",
        if has_running_state {
            CollectorState::Degraded
        } else {
            CollectorState::Unavailable
        },
        Some(detail),
    ));
}

fn camera_access(camera: &CameraDevice) -> Access {
    Access {
        key: format!("macos:camera:{}", camera.id),
        resource: Resource::Camera,
        activity: Activity::Active,
        risk: Risk::Unexplained,
        confidence: Confidence::Medium,
        enforcement: EnforcementDecision::Unknown,
        application: "Unknown application".to_owned(),
        pid: None,
        parent_pid: None,
        parent_name: None,
        executable: None,
        signature: None,
        device: Some(camera.name.clone()),
        started_at: None,
        modules: Vec::new(),
        process: None,
        evidence: vec![Evidence::new(
            EvidenceKind::LiveApi,
            "coremediaio_video",
            "kCMIODevicePropertyDeviceIsRunningSomewhere!=0; API does not identify the client or prove frame flow",
        )],
    }
}

struct Helper {
    directory: PathBuf,
    binary: PathBuf,
    notification_binary: Option<PathBuf>,
    desktop_children: Mutex<Vec<u32>>,
}

impl Helper {
    fn install() -> Result<Self> {
        for attempt in 0..10 {
            let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
            let directory = std::env::temp_dir()
                .join(format!("mcw-{}-{timestamp}-{attempt}", std::process::id()));
            match fs::DirBuilder::new().mode(0o700).create(&directory) {
                Ok(()) => {
                    let bundle = directory.join("MicCamWatchHelper.app");
                    let installation = Self {
                        binary: bundle.join("Contents/MacOS/MicCamWatchHelper"),
                        notification_binary: installed_notification_helper()?,
                        directory,
                        desktop_children: Mutex::new(Vec::new()),
                    };
                    for directory in [
                        &bundle,
                        &bundle.join("Contents"),
                        &bundle.join("Contents/MacOS"),
                        &bundle.join("Contents/_CodeSignature"),
                    ] {
                        fs::DirBuilder::new().mode(0o700).create(directory)?;
                    }
                    write_private(&installation.binary, HELPER, 0o700)?;
                    write_private(&bundle.join("Contents/Info.plist"), HELPER_PLIST, 0o600)?;
                    write_private(
                        &bundle.join("Contents/_CodeSignature/CodeResources"),
                        HELPER_RESOURCES,
                        0o600,
                    )?;
                    if unsafe { libc::atexit(remove_embedded_helper) } != 0 {
                        bail!("cannot register macOS helper cleanup");
                    }
                    return Ok(installation);
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        }
        bail!("cannot reserve private directory for embedded macOS capture helper")
    }

    fn request<T: DeserializeOwned>(
        &self,
        mode: &str,
        args: &[&str],
        timeout: Duration,
    ) -> Result<T> {
        let number = SCAN_ID.fetch_add(1, Ordering::Relaxed);
        let output_path = self.directory.join(format!("output-{number}"));
        let error_path = self.directory.join(format!("error-{number}"));
        let _files = ScanFiles {
            output: output_path.clone(),
            error: error_path.clone(),
        };
        let mut stdout = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&output_path)?;
        let mut stderr = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&error_path)?;
        let start = Instant::now();
        let mut child = RunningChild {
            child: Command::new(
                if matches!(
                    mode,
                    "notify" | "notification-identity" | "notification-status"
                ) {
                    self.notification_binary.as_ref().unwrap_or(&self.binary)
                } else {
                    &self.binary
                },
            )
            .arg(mode)
            .args(args)
            .process_group(0)
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout.try_clone()?))
            .stderr(Stdio::from(stderr.try_clone()?))
            .spawn()
            .context("cannot execute embedded macOS helper")?,
            reaped: false,
        };
        let result = (|| {
            let mut exit_info: libc::siginfo_t = unsafe { std::mem::zeroed() };
            let status = loop {
                if stdout.metadata()?.len() > MAX_OUTPUT || stderr.metadata()?.len() > MAX_OUTPUT {
                    bail!("macOS helper output exceeds {MAX_OUTPUT}-byte size limit");
                }
                // Observe exit without releasing the PID: residual utilities
                // must be contained before this private group can be reused.
                let observed = unsafe {
                    libc::waitid(
                        libc::P_PID,
                        child.child.id() as libc::id_t,
                        &mut exit_info,
                        libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
                    )
                };
                if observed == -1 {
                    let error = std::io::Error::last_os_error();
                    if error.raw_os_error() == Some(libc::ECHILD) {
                        child.reaped = true;
                    }
                    if error.kind() != std::io::ErrorKind::Interrupted {
                        return Err(error.into());
                    }
                } else if exit_info.si_pid != 0 {
                    let _ =
                        unsafe { libc::kill(-(child.child.id() as libc::pid_t), libc::SIGKILL) };
                    let status = match child.child.wait() {
                        Ok(status) => status,
                        Err(error) => {
                            if error.raw_os_error() == Some(libc::ECHILD) {
                                child.reaped = true;
                            }
                            return Err(error.into());
                        }
                    };
                    child.reaped = true;
                    break status;
                }
                if start.elapsed() >= timeout {
                    bail!("macOS helper exceeded {}ms timeout", timeout.as_millis());
                }
                thread::sleep(Duration::from_millis(20));
            };
            if !status.success() {
                bail!("macOS helper exited {status}");
            }
            // Check again after exit: the final write can race the polling check.
            if stdout.metadata()?.len() > MAX_OUTPUT || stderr.metadata()?.len() > MAX_OUTPUT {
                bail!("macOS helper output exceeds {MAX_OUTPUT}-byte size limit");
            }
            stdout.seek(SeekFrom::Start(0))?;
            serde_json::from_reader((&mut stdout).take(MAX_OUTPUT))
                .context("invalid macOS helper JSON response")
        })();
        // Every wait/metadata/decode failure terminates and reaps an unreaped
        // child before reading diagnostics or deleting its private output files.
        drop(child);
        result.with_context(|| format!("macOS helper {mode}: {}", helper_diagnostics(&mut stderr)))
    }

    fn desktop(&self, args: &[&str]) -> Result<Child> {
        let mut children = self
            .desktop_children
            .lock()
            .map_err(|_| anyhow::anyhow!("macOS desktop ownership lock poisoned"))?;
        // Never rewrite/reinstall this bundle beneath a running desktop host.
        let child = Command::new(&self.binary)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            // Keep stdout exclusively for desktop protocol frames. AppKit's
            // bounded launch/session diagnostics belong to the caller's stderr.
            .stderr(Stdio::inherit())
            .spawn()
            .context("cannot execute embedded AppKit desktop helper")?;
        children.retain(|pid| process_may_be_running(*pid));
        children.push(child.id());
        Ok(child)
    }
}

fn installed_notification_helper() -> Result<Option<PathBuf>> {
    let executable = std::env::current_exe()?.canonicalize()?;
    let Some(contents) = executable
        .parent()
        .and_then(Path::parent)
        .filter(|path| path.file_name().is_some_and(|name| name == "Contents"))
    else {
        return Ok(None);
    };
    let bundle = contents.join("Helpers/MicCamWatchHelper.app");
    // New packages always carry this helper. Old/development bundles keep the
    // embedded fallback and report their limited authorization persistence.
    if !bundle.try_exists()? {
        return Ok(None);
    }
    for (relative, expected) in [
        ("Contents/MacOS/MicCamWatchHelper", HELPER),
        ("Contents/Info.plist", HELPER_PLIST),
        ("Contents/_CodeSignature/CodeResources", HELPER_RESOURCES),
    ] {
        let path = bundle.join(relative);
        if path.canonicalize()? != path {
            bail!("installed notification helper path is redirected");
        }
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&path)?;
        let metadata = file.metadata()?;
        if !metadata.is_file() || metadata.len() != expected.len() as u64 {
            bail!("installed notification helper does not match this engine");
        }
        let mut actual = Vec::with_capacity(expected.len());
        file.read_to_end(&mut actual)?;
        if actual != expected {
            bail!("installed notification helper does not match this engine");
        }
    }
    Ok(Some(bundle.join("Contents/MacOS/MicCamWatchHelper")))
}

impl Helper {
    fn remove_files(&self) {
        // The returned Child belongs to the desktop host. On a normal shutdown
        // it is reaped first. If it still runs (or liveness is uncertain), retain
        // the bundle instead of removing resources out from under AppKit.
        let Ok(children) = self.desktop_children.lock() else {
            return;
        };
        if children.iter().any(|pid| process_may_be_running(*pid)) {
            return;
        }
        let _ = fs::remove_dir_all(&self.directory);
    }
}

fn write_private(path: &Path, bytes: &[u8], mode: u32) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)?;
    file.write_all(bytes)?;
    Ok(())
}

fn process_may_be_running(pid: u32) -> bool {
    // Signal zero checks liveness only, never terminates a PID. A reused PID or
    // an unexpected credential error conservatively retains the private bundle.
    let status = unsafe { libc::kill(pid as libc::pid_t, 0) };
    status == 0 || std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
}

struct RunningChild {
    child: Child,
    reaped: bool,
}
impl Drop for RunningChild {
    fn drop(&mut self) {
        if !self.reaped {
            // The helper PID has not been reaped, so its private process group
            // cannot be reused. Include nonprivileged utility descendants.
            let _ = unsafe { libc::kill(-(self.child.id() as libc::pid_t), libc::SIGKILL) };
            let _ = self.child.wait();
        }
    }
}

fn helper_diagnostics(file: &mut File) -> String {
    let mut bytes = Vec::new();
    if file
        .seek(SeekFrom::Start(0))
        .and_then(|_| file.take(MAX_OUTPUT).read_to_end(&mut bytes))
        .is_err()
    {
        return "helper diagnostics unavailable".to_owned();
    }
    let text = String::from_utf8_lossy(&bytes);
    if text.trim().is_empty() {
        "no helper diagnostics".to_owned()
    } else {
        text.trim().to_owned()
    }
}

fn helper() -> Result<&'static Helper> {
    match &*BINARY {
        Ok(helper) => Ok(helper),
        Err(error) => bail!("cannot install embedded macOS helper: {error}"),
    }
}

pub(crate) fn native_request<T: DeserializeOwned>(
    mode: &str,
    args: &[&str],
    timeout: Duration,
) -> Result<T> {
    validate_request(mode, args)?;
    if timeout.is_zero() || timeout > Duration::from_secs(120) {
        bail!("macOS helper timeout must be positive and at most 120 seconds");
    }
    helper()?.request(mode, args, timeout)
}

pub(crate) fn desktop_process(args: &[&str]) -> Result<Child> {
    if args != ["desktop"] {
        bail!("persistent macOS helper requires exactly the desktop mode");
    }
    helper()?.desktop(args)
}

fn validate_request(mode: &str, args: &[&str]) -> Result<()> {
    let valid = match mode {
        "audio"
        | "video"
        | "both"
        | "devices"
        | "locale"
        | "session-lock"
        | "notification-identity"
        | "notification-status"
        | "sound" => args.is_empty(),
        "control" | "log" => args.len() == 1,
        "terminate" | "close-application" | "notify" => args.len() == 2,
        "signature" => args.len() == 2 && matches!(args[1], "offline" | "online"),
        _ => false,
    };
    if !valid {
        bail!("invalid macOS helper mode or argument count");
    }
    let bytes = args
        .iter()
        .fold(mode.len(), |total, arg| total.saturating_add(arg.len()));
    if bytes > MAX_ARGUMENTS || mode.contains('\0') || args.iter().any(|arg| arg.contains('\0')) {
        bail!("macOS helper arguments contain NUL or exceed the size limit");
    }
    Ok(())
}

impl Drop for Helper {
    fn drop(&mut self) {
        self.remove_files();
    }
}

extern "C" fn remove_embedded_helper() {
    // Statics are never dropped. Abrupt termination or a still-running owned
    // desktop can leave a private directory; cleanup never kills the desktop.
    if let Ok(helper) = &*BINARY {
        helper.remove_files();
    }
}
struct ScanFiles {
    output: PathBuf,
    error: PathBuf,
}
impl Drop for ScanFiles {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.output);
        let _ = fs::remove_file(&self.error);
    }
}

fn validate_observation(mode: &str, observation: &Observation) -> Result<()> {
    if matches!(mode, "audio" | "both") && observation.audio.is_none()
        || matches!(mode, "video" | "both" | "devices") && observation.video.is_none()
        || mode == "devices" && observation.microphones.is_none()
    {
        bail!("macOS capture helper omitted a requested collector response");
    }
    Ok(())
}

fn observe(mode: &str) -> Result<Observation> {
    let observation = native_request(mode, &[], TIMEOUT)?;
    validate_observation(mode, &observation)?;
    Ok(observation)
}

impl CaptureCollector for PlatformMonitor {
    fn snapshot(&self, scope: CaptureScope) -> Result<Snapshot> {
        PlatformMonitor::snapshot(self, scope)
    }
    fn devices(&self) -> Result<Vec<Device>> {
        PlatformMonitor::devices(self)
    }
    fn diagnostics(&self) -> Vec<DiagnosticCheck> {
        self.doctor()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires an explicitly disposable hosted Mac and its fixture-only sudo authority"]
    fn native_other_user_process_is_refused_and_remains_alive() -> Result<()> {
        use std::{
            io::{BufRead, BufReader},
            os::unix::fs::PermissionsExt,
        };
        anyhow::ensure!(
            std::env::var("RUNNER_ENVIRONMENT").as_deref() == Ok("github-hosted"),
            "cross-user fixture is admitted only on a disposable hosted runner"
        );
        let temporary = tempfile::Builder::new()
            .prefix("mcw-other-user-")
            .tempdir_in("/private/tmp")?;
        fs::set_permissions(temporary.path(), fs::Permissions::from_mode(0o755))?;
        let binary = temporary.path().join("fixture");
        ensure_native_success(
            Command::new("/usr/bin/cc")
                .args(["native/fixtures/other_user_process.c", "-o"])
                .arg(&binary)
                .status()?,
        )?;
        let mut child = Command::new("/usr/bin/sudo")
            .args(["-n", "-u", "nobody"])
            .arg(&binary)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;
        let result = (|| -> Result<()> {
            let mut line = String::new();
            BufReader::new(child.stdout.take().context("fixture stdout missing")?)
                .read_line(&mut line)?;
            let pid: i32 = line
                .trim()
                .parse()
                .context("cross-user fixture did not report its PID")?;
            let read_identity = || -> Result<libc::proc_bsdinfo> {
                let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
                let size = std::mem::size_of_val(&info) as i32;
                anyhow::ensure!(
                    unsafe {
                        libc::proc_pidinfo(
                            pid,
                            libc::PROC_PIDTBSDINFO,
                            0,
                            (&mut info as *mut libc::proc_bsdinfo).cast(),
                            size,
                        )
                    } == size,
                    "cross-user identity unavailable"
                );
                Ok(info)
            };
            let before = read_identity()?;
            anyhow::ensure!(
                before.pbi_uid != unsafe { libc::getuid() },
                "fixture unexpectedly belongs to the caller"
            );
            let instance = format!(
                "macos:microphone:{pid}:{}:{}",
                before.pbi_start_tvsec, before.pbi_start_tvusec
            );
            for mode in ["terminate", "close-application"] {
                let reply: serde_json::Value = native_request(
                    mode,
                    &[&pid.to_string(), &instance],
                    Duration::from_secs(20),
                )?;
                assert_eq!(reply["effect"]["ok"], false, "{reply}");
                let after = read_identity()?;
                assert_eq!(
                    (after.pbi_start_tvsec, after.pbi_start_tvusec),
                    (before.pbi_start_tvsec, before.pbi_start_tvusec)
                );
            }
            Ok(())
        })();
        // EOF belongs to the retained fixture, not to a numeric PID. Never ask
        // the product to kill it during cleanup or escalate the rejected action.
        drop(child.stdin.take());
        let deadline = Instant::now() + Duration::from_secs(5);
        while child.try_wait()?.is_none() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        anyhow::ensure!(
            child.try_wait()?.is_some(),
            "cross-user fixture did not exit after owned stdin closed"
        );
        let _ = child.wait();
        result
    }

    #[test]
    fn native_close_application_observes_a_hardened_fixture_exit() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let app = temporary.path().join("MCWCloseFixture.app");
        let contents = app.join("Contents");
        fs::create_dir_all(contents.join("MacOS"))?;
        fs::write(
            contents.join("Info.plist"),
            r#"<?xml version="1.0"?><plist version="1.0"><dict>
            <key>CFBundleIdentifier</key><string>dev.miccamwatch.close-fixture</string>
            <key>CFBundleExecutable</key><string>fixture</string>
            <key>CFBundlePackageType</key><string>APPL</string>
            <key>LSUIElement</key><true/>
            </dict></plist>"#,
        )?;
        let binary = contents.join("MacOS/fixture");
        ensure_native_success(
            Command::new("swiftc")
                .args(["native/fixtures/macos_close_app.swift", "-o"])
                .arg(&binary)
                .status()?,
        )?;
        ensure_native_success(
            Command::new("codesign")
                .args([
                    "--force",
                    "--sign",
                    "-",
                    "--options",
                    "runtime",
                    "--timestamp=none",
                ])
                .arg(&app)
                .status()?,
        )?;
        let pid_path = temporary.path().join("pid");
        let mut launcher = Command::new("open")
            .args(["-n", "-W", "-a"])
            .arg(&app)
            .arg("--args")
            .arg(&pid_path)
            .arg("refuse-once")
            .spawn()?;
        let result = (|| -> Result<()> {
            let deadline = Instant::now() + Duration::from_secs(10);
            while !pid_path.exists() && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(50));
            }
            let pid: i32 = fs::read_to_string(&pid_path)?.parse()?;
            let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
            let size = std::mem::size_of_val(&info) as i32;
            let read = unsafe {
                libc::proc_pidinfo(
                    pid,
                    libc::PROC_PIDTBSDINFO,
                    0,
                    (&mut info as *mut libc::proc_bsdinfo).cast(),
                    size,
                )
            };
            if read != size {
                bail!("fixture birth identity unavailable");
            }
            let instance = format!(
                "macos:microphone:{pid}:{}:{}",
                info.pbi_start_tvsec, info.pbi_start_tvusec
            );
            let stale = format!("macos:microphone:{pid}:{}:0", info.pbi_start_tvsec + 1);
            for (target, identity) in [(pid, stale.as_str()), (1, instance.as_str())] {
                let refused: serde_json::Value = native_request(
                    "close-application",
                    &[&target.to_string(), identity],
                    Duration::from_secs(20),
                )?;
                assert_eq!(refused["effect"]["ok"], false, "{refused}");
            }
            let refused: serde_json::Value = native_request(
                "close-application",
                &[&pid.to_string(), &instance],
                Duration::from_secs(20),
            )?;
            assert_eq!(refused["effect"]["ok"], false, "{refused}");
            assert!(
                refused["effect"]["error"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("unconfirmed"),
                "{refused}"
            );
            assert!(
                launcher.try_wait()?.is_none(),
                "refused close must leave the application alive"
            );
            let reply: serde_json::Value = native_request(
                "close-application",
                &[&pid.to_string(), &instance],
                Duration::from_secs(20),
            )?;
            assert_eq!(reply["effect"]["ok"], true, "{reply}");
            assert_eq!(reply["effect"]["state"], "application_exited");
            let deadline = Instant::now() + Duration::from_secs(5);
            while launcher.try_wait()?.is_none() && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(50));
            }
            assert!(launcher.try_wait()?.is_some());
            Ok(())
        })();
        // Only the fixture launcher belongs to this test. A failed application
        // close is retained as a failure and is never escalated by PID.
        if launcher.try_wait()?.is_none() {
            let _ = launcher.kill();
        }
        let _ = launcher.wait();
        result
    }

    fn ensure_native_success(status: std::process::ExitStatus) -> Result<()> {
        if !status.success() {
            bail!("native fixture setup failed: {status}");
        }
        Ok(())
    }

    #[test]
    fn native_termination_rejects_parent_and_stale_observation() -> Result<()> {
        let request = |pid: u32| {
            native_request::<serde_json::Value>(
                "terminate",
                &[&pid.to_string(), "stale-observation"],
                Duration::from_secs(5),
            )
        };
        let parent = request(std::process::id())?;
        assert_eq!(parent["effect"]["ok"], false);
        assert!(
            parent["effect"]["error"]
                .as_str()
                .is_some_and(|detail| detail.contains("protected"))
        );

        let mut child = Command::new("/bin/sleep").arg("20").spawn()?;
        let result = (|| -> Result<_> {
            let reply = request(child.id())?;
            Ok((reply, child.try_wait()?.is_none()))
        })();
        // The fixture owns this child handle and is responsible for reaping it.
        let _ = child.kill();
        child.wait()?;
        let (reply, still_running) = result?;
        assert!(still_running);
        assert_eq!(reply["effect"]["ok"], false);
        assert!(
            reply["effect"]["error"]
                .as_str()
                .is_some_and(|detail| detail.contains("birth identity changed"))
        );
        Ok(())
    }

    #[test]
    fn active_input_requires_verified_process_identity_for_pid() {
        let observation: Observation = serde_json::from_str(r#"{"audio":{"available":true,"processes":[{"pid":51,"startSeconds":1710000000,"startMicroseconds":3,"executable":"/Applications/Recorder.app/Contents/MacOS/Recorder"},{"pid":51,"startSeconds":null,"startMicroseconds":null,"executable":null}],"error":null}}"#).unwrap();
        let processes = observation.audio.unwrap().processes;
        let first = microphone_access(processes.into_iter().next().unwrap(), 0);
        assert_eq!(first.pid, Some(51));
        assert_eq!(first.application, "Recorder");
        assert_eq!(first.activity, Activity::Active);
        assert!(first.key.contains("1710000000:3"));
        let unknown = microphone_access(
            AudioProcess {
                pid: Some(51),
                start_seconds: None,
                start_microseconds: None,
                executable: None,
            },
            1,
        );
        assert_eq!(unknown.pid, None);
        assert!(unknown.process.is_none());
        assert_eq!(unknown.enforcement, EnforcementDecision::Unknown);
    }

    #[test]
    fn rejects_partial_or_invalid_birth_identity() {
        for (seconds, micros, path) in [
            (0, 0, "/bin/app"),
            (1, 1_000_000, "/bin/app"),
            (1, 0, ""),
            (1, 0, "bin/app"),
            (1, 0, "/bin/../app"),
        ] {
            let process = AudioProcess {
                pid: Some(42),
                start_seconds: Some(seconds),
                start_microseconds: Some(micros),
                executable: Some(path.to_owned()),
            };
            assert!(!process.has_identity());
            assert_eq!(microphone_access(process, 0).pid, None);
        }
    }

    #[test]
    fn birth_or_executable_changes_invalidate_signature_attribution() {
        let access = microphone_access(
            AudioProcess {
                pid: Some(51),
                start_seconds: Some(10),
                start_microseconds: Some(2),
                executable: Some("/Applications/Recorder.app/Contents/MacOS/Recorder".into()),
            },
            0,
        );
        let mut current = AudioProcess {
            pid: Some(51),
            start_seconds: Some(10),
            start_microseconds: Some(2),
            executable: access.executable.clone(),
        };
        assert!(current.matches_access(&access));
        current.start_microseconds = Some(3);
        assert!(!current.matches_access(&access));
        current.start_microseconds = Some(2);
        current.executable = Some("/other/Recorder".into());
        assert!(!current.matches_access(&access));
        current.executable = access.executable.clone();
        current.pid = Some(52);
        assert!(!current.matches_access(&access));
    }

    #[test]
    fn changed_policy_affects_future_assessments_without_native_side_effects() {
        let mut monitor = PlatformMonitor::new(Policy::default()).unwrap();
        let mut access = microphone_access(
            AudioProcess {
                pid: Some(51),
                start_seconds: Some(10),
                start_microseconds: Some(2),
                executable: Some("/Applications/Recorder.app/Contents/MacOS/Recorder".into()),
            },
            0,
        );
        monitor.apply_policy(&mut access);
        assert_eq!(access.enforcement, EnforcementDecision::Alert);
        let instance = access.process.clone();
        monitor.set_policy(Policy {
            applications: vec![crate::config::ApplicationRule {
                executable: "Recorder".into(),
                paths: vec!["/Applications/Recorder.app".into()],
                publishers: Vec::new(),
            }],
            ..Policy::default()
        });
        monitor.apply_policy(&mut access);
        assert_eq!(access.enforcement, EnforcementDecision::Allow);
        assert_eq!(access.process, instance);
        monitor.set_policy(Policy {
            applications: vec![crate::config::ApplicationRule {
                executable: "Recorder".into(),
                paths: vec!["/other".into()],
                publishers: vec!["Trusted publisher".into()],
            }],
            ..Policy::default()
        });
        monitor.apply_policy(&mut access);
        assert_eq!(access.enforcement, EnforcementDecision::Unknown);
        access.signature = Some(crate::model::SignatureInfo {
            verified: false,
            signer: None,
            error: Some("Unsigned executable".into()),
        });
        monitor.apply_policy(&mut access);
        assert_eq!(access.enforcement, EnforcementDecision::Deny);
    }

    fn camera_snapshot(document: &str) -> Snapshot {
        let mut snapshot = Snapshot {
            collectors: Vec::new(),
            accesses: Vec::new(),
            observation_gaps: Vec::new(),
        };
        append_camera_observation(&mut snapshot, serde_json::from_str(document).unwrap());
        snapshot
    }

    #[test]
    fn camera_disappearance_with_another_idle_device_does_not_invent_stop() {
        let mut previous = HashSet::new();
        let mut tracker = crate::watcher::TransitionTracker::default();
        let mut events = Vec::new();
        for document in [
            r#"{"devices":[{"id":"usb","name":"USB","runningSomewhere":true},{"id":"built-in","name":"Built-in","runningSomewhere":false}]}"#,
            r#"{"devices":[{"id":"built-in","name":"Built-in","runningSomewhere":false}]}"#,
            r#"{"devices":[{"id":"built-in","name":"Built-in","runningSomewhere":false}]}"#,
            r#"{"devices":[{"id":"usb","name":"USB","runningSomewhere":true},{"id":"built-in","name":"Built-in","runningSomewhere":false}]}"#,
            r#"{"devices":[{"id":"usb","name":"USB","runningSomewhere":false},{"id":"built-in","name":"Built-in","runningSomewhere":false}]}"#,
        ] {
            let mut snapshot = Snapshot {
                collectors: Vec::new(),
                accesses: Vec::new(),
                observation_gaps: Vec::new(),
            };
            append_camera_observation_with_inventory(
                &mut snapshot,
                serde_json::from_str(document).unwrap(),
                &mut previous,
            );
            tracker
                .reconcile(&snapshot, |_, action| {
                    events.push(action);
                    Ok(())
                })
                .unwrap();
        }
        assert!(matches!(
            events.as_slice(),
            [
                crate::model::Action::Start,
                crate::model::Action::Start,
                crate::model::Action::Stop
            ]
        ));
    }

    #[test]
    fn camera_activity_never_attributes_or_enforces_unknown_client() {
        let snapshot = camera_snapshot(
            r#"{"devices":[{"id":"camera-1","name":"USB camera","runningSomewhere":true},{"id":"camera-2","name":"Idle camera","runningSomewhere":false}]}"#,
        );
        assert_eq!(snapshot.accesses.len(), 1);
        let active = &snapshot.accesses[0];
        assert_eq!(active.device.as_deref(), Some("USB camera"));
        assert_eq!(active.activity, Activity::Active);
        assert_eq!(active.confidence, Confidence::Medium);
        assert_eq!(active.pid, None);
        assert_eq!(active.application, "Unknown application");
        assert_eq!(active.enforcement, EnforcementDecision::Unknown);
        assert!(active.executable.is_none());
        assert!(active.signature.is_none());
        assert!(active.process.is_none());
        assert!(snapshot.observation_gaps.is_empty());
        assert_eq!(snapshot.collectors[0].state, CollectorState::Degraded);
    }

    #[test]
    fn camera_known_idle_is_complete_but_not_healthy_or_ready() {
        let snapshot = camera_snapshot(
            r#"{"devices":[{"id":"camera-1","name":"USB camera","runningSomewhere":false}]}"#,
        );
        assert!(snapshot.accesses.is_empty());
        assert!(snapshot.observation_gaps.is_empty());
        assert_eq!(snapshot.collectors[0].state, CollectorState::Degraded);
    }

    #[test]
    fn camera_unknown_null_missing_empty_and_error_are_observation_gaps() {
        for (document, state) in [
            (
                r#"{"devices":[{"id":"c","name":"cam","runningSomewhere":null}]}"#,
                CollectorState::Unavailable,
            ),
            (
                r#"{"devices":[{"id":"c","name":"cam"}]}"#,
                CollectorState::Unavailable,
            ),
            (r#"{"devices":[]}"#, CollectorState::Unavailable),
            (
                r#"{"devices":[{"id":"c","name":"cam","runningSomewhere":false}],"error":"CoreMediaIO lookup failed"}"#,
                CollectorState::Degraded,
            ),
        ] {
            let snapshot = camera_snapshot(document);
            assert!(snapshot.accesses.is_empty());
            assert_eq!(snapshot.observation_gaps, vec![Resource::Camera]);
            assert_eq!(snapshot.collectors[0].state, state);
        }
    }

    #[test]
    fn camera_partial_error_preserves_known_activity_but_suppresses_stop() {
        for document in [
            r#"{"devices":[{"id":"active","name":"USB camera","runningSomewhere":true},{"id":"unknown","name":"Other camera","runningSomewhere":null}],"error":"CoreMediaIO running-state read failed"}"#,
            r#"{"devices":[{"id":"active","name":"USB camera","runningSomewhere":true},{"id":"unknown","name":"Other camera"}]}"#,
        ] {
            let snapshot = camera_snapshot(document);
            assert_eq!(snapshot.accesses.len(), 1);
            assert_eq!(snapshot.accesses[0].key, "macos:camera:active");
            assert_eq!(
                snapshot.accesses[0].enforcement,
                EnforcementDecision::Unknown
            );
            assert_eq!(snapshot.observation_gaps, vec![Resource::Camera]);
            assert_eq!(snapshot.collectors[0].state, CollectorState::Degraded);
        }
    }

    #[test]
    fn camera_inventory_accepts_unknown_activity_without_observing_it() {
        let observation: Observation = serde_json::from_str(
            r#"{"video":{"devices":[{"id":"camera-1","name":"USB camera"}]},"microphones":[]}"#,
        )
        .unwrap();
        assert!(validate_observation("devices", &observation).is_ok());
        let camera = &observation.video.as_ref().unwrap().devices[0];
        assert_eq!(camera.id, "camera-1");
        assert_eq!(camera.running_somewhere, None);
    }

    #[test]
    fn malformed_native_response_is_not_a_healthy_empty_scan() {
        assert!(
            serde_json::from_str::<Observation>(
                r#"{"audio":{"processes":[{"pid":"oops"}],"available":true}}"#
            )
            .is_err()
        );
        assert!(
            serde_json::from_str::<Observation>(
                r#"{"video":{"devices":[{"id":"c","name":"cam","runningSomewhere":"false"}]}}"#
            )
            .is_err()
        );
        let empty: Observation = serde_json::from_str("{}").unwrap();
        assert!(validate_observation("audio", &empty).is_err());
        assert!(validate_observation("video", &empty).is_err());
        assert!(validate_observation("devices", &empty).is_err());
        assert!(validate_request("desktop", &[]).is_err());
        assert!(validate_request("signature", &["/bin/app", "unchecked"]).is_err());
        assert!(validate_request("control", &["{}\0"]).is_err());
        assert!(validate_request("log", &[&"x".repeat(MAX_ARGUMENTS)]).is_err());
        assert!(validate_request("control", &["{}"]).is_ok());
    }
}
