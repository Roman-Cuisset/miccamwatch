mod control;
mod pipewire;
mod procfs;
mod signature;
mod v4l2;

use crate::{
    collector::{CaptureCollector, CaptureScope},
    config::Policy,
    model::{
        Access, CollectorHealth, CollectorState, Device, DiagnosticCheck, DiagnosticStatus,
        Evidence, EvidenceKind, MicrophoneMuteState, Resource, Snapshot,
    },
    policy::assess_policy,
};
use anyhow::{Context, Result, bail};
use pipewire::Graph;
use procfs::{BootTime, ProcessIdentity};

/// Linux captures are derived from PipeWire's live graph. Direct V4L2 FDs
/// provide only low-confidence readiness and never prove frame flow.
pub struct PlatformMonitor {
    policy: Policy,
    control: control::Control,
}

impl PlatformMonitor {
    pub fn new(policy: Policy) -> Result<Self> {
        Ok(Self {
            policy,
            control: control::Control::new(),
        })
    }

    pub(crate) fn set_policy(&mut self, policy: Policy) {
        self.policy = policy;
    }

    pub fn microphone_mute_state(&self) -> Result<MicrophoneMuteState> {
        self.control.microphone_mute_state()
    }

    pub fn set_microphone_mute(&self, muted: bool) -> Result<usize> {
        self.control.set_microphone_mute(muted)
    }

    pub fn toggle_microphone_mute(&self) -> Result<bool> {
        self.control.toggle_microphone_mute()
    }

    pub fn begin_lock_microphone_mute(&self) -> Result<()> {
        self.control.begin_lock_mute()
    }

    pub fn restore_lock_microphone_mute(&self) -> Result<()> {
        self.control.restore_lock_mute()
    }

    pub fn snapshot(&self, scope: CaptureScope) -> Result<Snapshot> {
        self.snapshot_with_graph(scope, Graph::read())
    }

    fn snapshot_with_graph(&self, scope: CaptureScope, graph: Result<Graph>) -> Result<Snapshot> {
        let boot = BootTime::read();
        let cameras = if scope.camera {
            Some(v4l2::devices())
        } else {
            None
        };
        let mut accesses = Vec::new();
        let mut collectors =
            Vec::with_capacity(usize::from(scope.microphone) + usize::from(scope.camera));

        let (audio, video) = match &graph {
            Ok(graph) => {
                let observation = graph.observe(scope, boot.as_ref().ok());
                accesses = observation.accesses;
                (observation.audio_error, observation.video_error)
            }
            Err(error) => {
                let detail = format!("{error:#}");
                (Some(detail.clone()), Some(detail))
            }
        };
        let proc_error = boot
            .as_ref()
            .err()
            .map(|error| format!("process attribution unavailable: {error:#}"));
        if scope.microphone {
            collectors.push(health(
                "pipewire_audio",
                if graph.is_err() {
                    CollectorState::Unavailable
                } else if audio.is_some() || proc_error.is_some() {
                    CollectorState::Degraded
                } else {
                    CollectorState::Healthy
                },
                audio.or_else(|| proc_error.clone()),
            ));
        }
        if scope.camera {
            let camera_result = cameras.expect("camera scope requested");
            let v4l2_error = camera_result
                .as_ref()
                .err()
                .map(|error| format!("V4L2 inventory unavailable: {error:#}"));
            let devices = camera_result.unwrap_or_default();
            let scan = if scope.include_ready {
                v4l2::scan(&devices, boot.as_ref().ok())
            } else {
                v4l2::Scan::default()
            };
            // When the same PID and camera are visible through PipeWire and /proc,
            // one stream is represented by its stronger graph evidence.
            for direct in scan.accesses {
                if !accesses.iter().any(|graph| {
                    graph.pid.is_some()
                        && graph.pid == direct.pid
                        && graph.device == direct.device
                        && graph.resource == direct.resource
                }) {
                    accesses.push(direct);
                }
            }
            let bypass = (!devices.is_empty()).then(|| {
                "direct V4L2 capture may bypass PipeWire; an open /dev/video* fd never proves streaming".to_owned()
            });
            let detail = video
                .or(v4l2_error)
                .or(scan.warning)
                .or(proc_error)
                .or(bypass);
            collectors.push(health(
                "pipewire_video",
                if graph.is_err() {
                    CollectorState::Unavailable
                } else if detail.is_some() {
                    CollectorState::Degraded
                } else {
                    CollectorState::Healthy
                },
                detail,
            ));
        }
        for access in &mut accesses {
            self.assess_access(access, boot.as_ref().ok());
        }
        let mut observation_gaps = Vec::new();
        if graph.is_err() {
            if scope.microphone {
                observation_gaps.push(Resource::Microphone);
            }
            if scope.camera {
                observation_gaps.push(Resource::Camera);
            }
        }
        Ok(Snapshot {
            collectors,
            accesses,
            observation_gaps,
        })
    }

    fn assess_access(&self, access: &mut Access, boot: Option<&BootTime>) {
        let identity = (|| {
            let boot = boot.context("Linux boot time is unavailable")?;
            let before = validated_access_identity(access, boot)?;
            let signature = signature::verify_signature_with_policy(
                access
                    .executable
                    .as_deref()
                    .context("executable is unavailable")?,
                self.policy.trust_policy,
            );
            // A long-running verifier must not grant policy trust after PID reuse
            // or exec. The signature authenticates the on-disk file, not pages
            // already loaded by the process.
            let after = validated_access_identity(access, boot)?;
            if !procfs::same_executable(&before.executable_file, &after.executable_file) {
                bail!("running executable changed during detached signature verification");
            }
            Ok::<_, anyhow::Error>(signature)
        })();
        let executable = match identity {
            Ok(signature) => {
                access.signature = match signature {
                    Ok(signature) => {
                        access.evidence.push(Evidence::new(
                            EvidenceKind::Signature,
                            "openpgp",
                            if signature.verified {
                                format!(
                                    "Detached executable signature verified for {}; this does not attest loaded process pages",
                                    signature.signer.as_deref().unwrap_or("unknown signer")
                                )
                            } else {
                                signature.error.clone().unwrap_or_else(|| {
                                    "Detached executable signature did not verify".to_owned()
                                })
                            },
                        ));
                        Some(signature)
                    }
                    Err(error) => {
                        access.evidence.push(Evidence::new(
                            EvidenceKind::Signature,
                            "openpgp",
                            format!(
                                "Detached executable signature evidence unavailable: {error:#}"
                            ),
                        ));
                        None
                    }
                };
                access.executable.as_deref()
            }
            Err(error) => {
                access.signature = None;
                access.evidence.push(Evidence::new(
                    EvidenceKind::ProcessLineage,
                    "procfs",
                    format!("Policy executable identity unavailable: {error:#}"),
                ));
                None
            }
        };
        (access.risk, access.enforcement) = assess_policy(
            &self.policy,
            &access.application,
            executable,
            access.signature.as_ref(),
            &mut access.evidence,
        );
    }

    pub fn devices(&self) -> Result<Vec<Device>> {
        let mut devices = Graph::read()?.devices();
        for camera in v4l2::devices()? {
            if !devices
                .iter()
                .any(|node| node.resource == camera.resource && node.id == camera.id)
            {
                devices.push(camera);
            }
        }
        Ok(devices)
    }

    pub fn doctor(&self) -> Vec<DiagnosticCheck> {
        let mut checks = match self.snapshot(CaptureScope {
            include_ready: true,
            ..CaptureScope::all()
        }) {
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
                        .unwrap_or_else(|| "PipeWire graph accessible".to_owned()),
                })
                .collect(),
            Err(error) => vec![DiagnosticCheck {
                name: "linux_collectors",
                status: DiagnosticStatus::Error,
                detail: format!("cannot inspect Linux capture collectors: {error:#}"),
            }],
        };
        checks.push(DiagnosticCheck {
            name: "linux_control_scope",
            status: DiagnosticStatus::Warning,
            detail: "Signal mute applies only to writable capture sources in the connected PipeWire session, including virtual and monitor sources when supported. It does not deny microphone access or control direct ALSA, other PipeWire sessions, V4L2 cameras, or physical hardware.".to_owned(),
        });
        match self.control.capabilities() {
            Ok(report) => {
                let writable = report.sources.iter().filter(|source| source.writable).count();
                checks.push(DiagnosticCheck {
                    name: "pipewire_source_mute",
                    status: if writable == 0 { DiagnosticStatus::Warning } else { DiagnosticStatus::Ok },
                    detail: format!(
                        "{writable}/{} session capture sources have writable mute controls; {} original mute states retained, {} restorations pending; lock active={}, suppressed by manual intent={}",
                        report.sources.len(), report.retained_original_states,
                        report.pending_restorations.len(), report.lock_active,
                        report.lock_suppressed_by_manual_intent,
                    ),
                });
                for source in report.sources.iter().filter(|source| !source.writable) {
                    checks.push(DiagnosticCheck {
                        name: "pipewire_unsupported_source",
                        status: DiagnosticStatus::Warning,
                        detail: format!(
                            "{} ({:?}): {}",
                            source.name, source.kind,
                            source.unsupported_reason.as_deref().unwrap_or("writable mute control unavailable"),
                        ),
                    });
                }
                for pending in report.pending_restorations {
                    checks.push(DiagnosticCheck {
                        name: "pipewire_pending_restoration",
                        status: DiagnosticStatus::Warning,
                        detail: format!("{}: {}", pending.name, pending.reason),
                    });
                }
            }
            Err(error) => checks.push(DiagnosticCheck {
                name: "pipewire_source_mute",
                status: DiagnosticStatus::Error,
                detail: format!("Cannot inspect session mute controls or retained restoration records: {error:#}"),
            }),
        }
        checks
    }
}

/// The existing procfs instance identifier is also the pidfd authority token.
pub(crate) fn verified_process_identity(pid: u32) -> Result<(String, String, u32)> {
    let identity = ProcessIdentity::verify(pid, &BootTime::read()?)?;
    Ok((identity.instance_id, identity.executable, identity.uid))
}

fn validated_access_identity(access: &Access, boot: &BootTime) -> Result<ProcessIdentity> {
    let pid = access
        .pid
        .context("authenticated capture PID is unavailable")?;
    let observed = access
        .process
        .as_ref()
        .context("capture process instance is unavailable")?;
    let identity = ProcessIdentity::verify(pid, boot)?;
    if observed.instance_id != identity.instance_id
        || access.executable.as_deref() != Some(identity.executable.as_str())
        || access.application != identity.name
        || observed
            .user
            .as_deref()
            .and_then(|uid| uid.parse::<u32>().ok())
            != Some(identity.uid)
    {
        bail!("capture process or executable no longer matches its validated observation");
    }
    Ok(identity)
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
    fn failed_graph_read_is_unavailable_and_cannot_prove_capture_stopped() {
        let monitor = PlatformMonitor::new(Policy::default()).unwrap();
        let scope = CaptureScope {
            microphone: true,
            camera: false,
            include_ready: false,
        };
        let retained = r#"{"type":"PipeWire:Interface:Client/3"}"#;
        let overloaded = format!("[{}]", vec![retained; 4097].join(","));
        for graph in [
            Err(anyhow::anyhow!("injected PipeWire reader failure")),
            Graph::parse(b"malformed graph"),
            Graph::parse(overloaded.as_bytes()),
        ] {
            let snapshot = monitor.snapshot_with_graph(scope, graph).unwrap();
            assert!(snapshot.accesses.is_empty());
            assert_eq!(snapshot.collectors.len(), 1);
            assert_eq!(snapshot.collectors[0].state, CollectorState::Unavailable);
            assert!(snapshot.collectors[0].detail.is_some());
            // The watcher retains prior microphone observations across this
            // gap instead of interpreting an empty access list as STOP.
            assert_eq!(snapshot.observation_gaps, vec![Resource::Microphone]);
        }
    }
}
