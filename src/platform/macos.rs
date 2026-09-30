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
use serde::Deserialize;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        LazyLock,
        atomic::{AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const HELPER: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/mcw-macos-capture"));
const MAX_OUTPUT: u64 = 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(5);
static BINARY: LazyLock<Result<Helper, String>> =
    LazyLock::new(|| Helper::install().map_err(|error| format!("{error:#}")));
static SCAN_ID: AtomicU64 = AtomicU64::new(0);

pub struct PlatformMonitor;

impl PlatformMonitor {
    pub fn new(_policy: Policy) -> Result<Self> {
        Ok(Self)
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
                        for (index, process) in audio.processes.into_iter().enumerate() {
                            if !process.has_identity() {
                                detail.get_or_insert_with(|| "active CoreAudio input client has no stable process identity".to_owned());
                            }
                            snapshot.accesses.push(microphone_access(process, index));
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
                        .context("helper omitted AVFoundation result")?;
                    if video.error.is_some() || video.devices.is_empty() {
                        snapshot.observation_gaps.push(Resource::Camera);
                    }
                    let detail = video.error.unwrap_or_else(|| {
                        if !video.interactive || video.devices.is_empty() {
                            "passive AVFoundation camera discovery is unverified in this session; no camera permission requested; own-app capture and client identity are not observable".to_owned()
                        } else {
                            "AVFoundation only reports capture by another application; own-app capture and client identity are not observable".to_owned()
                        }
                    });
                    for camera in video.devices {
                        if camera.in_use {
                            snapshot.accesses.push(camera_access(&camera));
                        }
                    }
                    // Coverage limitations alone do not invalidate device observations.
                    // Empty discovery and API errors are recorded separately as scan gaps.
                    snapshot.collectors.push(health(
                        "avfoundation_video",
                        CollectorState::Degraded,
                        Some(detail),
                    ));
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
                        "avfoundation_video",
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
        match self.snapshot(CaptureScope::all()) {
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
            if pid > 0 && seconds > 0 && micros < 1_000_000 && !path.is_empty())
    }
}

#[derive(Deserialize)]
struct VideoResult {
    devices: Vec<CameraDevice>,
    error: Option<String>,
    interactive: bool,
}

#[derive(Deserialize)]
struct CameraDevice {
    id: String,
    name: String,
    #[serde(rename = "inUse")]
    in_use: bool,
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
            "avfoundation_video",
            "AVCaptureDevice.isInUseByAnotherApplication=true; API does not identify the client",
        )],
    }
}

struct Helper {
    directory: PathBuf,
    binary: PathBuf,
}

impl Helper {
    fn install() -> Result<Self> {
        for attempt in 0..10 {
            let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
            let directory = std::env::temp_dir()
                .join(format!("mcw-{}-{timestamp}-{attempt}", std::process::id()));
            match fs::DirBuilder::new().mode(0o700).create(&directory) {
                Ok(()) => {
                    let installation = Self {
                        binary: directory.join("capture"),
                        directory,
                    };
                    let mut file = OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .mode(0o700)
                        .open(&installation.binary)?;
                    file.write_all(HELPER)?;
                    if unsafe { libc::atexit(remove_embedded_helper) } != 0 {
                        bail!("cannot register macOS capture helper cleanup");
                    }
                    return Ok(installation);
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        }
        bail!("cannot reserve private directory for embedded macOS capture helper")
    }

    fn run(&self, mode: &str) -> Result<Observation> {
        let number = SCAN_ID.fetch_add(1, Ordering::Relaxed);
        let output_path = self.directory.join(format!("output-{number}"));
        let error_path = self.directory.join(format!("error-{number}"));
        let _files = ScanFiles {
            output: output_path.clone(),
            error: error_path.clone(),
        };
        let stdout = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&output_path)?;
        let stderr = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&error_path)?;
        let mut child = Command::new(&self.binary)
            .arg(mode)
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .spawn()
            .context("cannot execute embedded macOS capture helper")?;
        let start = Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if start.elapsed() >= TIMEOUT {
                let _ = child.kill();
                let _ = child.wait();
                bail!(
                    "macOS capture helper exceeded {}s timeout",
                    TIMEOUT.as_secs()
                );
            }
            thread::sleep(Duration::from_millis(20));
        };
        if !status.success() {
            let mut stderr = String::new();
            File::open(&error_path)?
                .take(MAX_OUTPUT)
                .read_to_string(&mut stderr)?;
            bail!("macOS capture helper exited {status}: {}", stderr.trim());
        }
        let file = File::open(&output_path)?;
        if file.metadata()?.len() > MAX_OUTPUT {
            bail!("macOS capture helper response exceeds size limit");
        }
        let observation: Observation =
            serde_json::from_reader(file).context("invalid macOS capture helper response")?;
        validate_observation(mode, &observation)?;
        Ok(observation)
    }
}

impl Helper {
    fn remove_files(&self) {
        let _ = fs::remove_file(&self.binary);
        let _ = fs::remove_dir(&self.directory);
    }
}

impl Drop for Helper {
    fn drop(&mut self) {
        self.remove_files();
    }
}

extern "C" fn remove_embedded_helper() {
    // Statics are never dropped; remove this process's private helper on normal
    // CLI/watch exit. Abrupt termination can still leave an orphaned directory.
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
    match &*BINARY {
        Ok(helper) => helper.run(mode),
        Err(error) => bail!("cannot install embedded capture helper: {error}"),
    }
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
        for (seconds, micros, path) in [(0, 0, "/bin/app"), (1, 1_000_000, "/bin/app"), (1, 0, "")]
        {
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
    fn camera_activity_never_attributes_or_enforces_unknown_client() {
        let observation: Observation = serde_json::from_str(r#"{"video":{"devices":[{"id":"camera-1","name":"USB camera","inUse":true},{"id":"camera-2","name":"Idle camera","inUse":false}],"error":null,"interactive":false}}"#).unwrap();
        let video = observation.video.unwrap();
        let active: Vec<_> = video
            .devices
            .iter()
            .filter(|camera| camera.in_use)
            .map(camera_access)
            .collect();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].device.as_deref(), Some("USB camera"));
        assert_eq!(active[0].pid, None);
        assert_eq!(active[0].application, "Unknown application");
        assert_eq!(active[0].enforcement, EnforcementDecision::Unknown);
        assert!(!video.interactive);
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
                r#"{"video":{"devices":[{"id":"c","name":"cam"}],"interactive":true}}"#
            )
            .is_err()
        );
        let empty: Observation = serde_json::from_str("{}").unwrap();
        assert!(validate_observation("audio", &empty).is_err());
        assert!(validate_observation("video", &empty).is_err());
        assert!(validate_observation("devices", &empty).is_err());
    }
}
