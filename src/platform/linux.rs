mod pipewire;
mod procfs;
mod v4l2;

use crate::{
    collector::{CaptureCollector, CaptureScope},
    config::Policy,
    model::{CollectorHealth, CollectorState, Device, DiagnosticCheck, DiagnosticStatus, Snapshot},
};
use anyhow::Result;
use pipewire::Graph;
use procfs::BootTime;

/// Linux captures are derived from PipeWire's live graph. Direct V4L2 FDs
/// provide only low-confidence readiness and never prove frame flow.
pub struct PlatformMonitor;

impl PlatformMonitor {
    pub fn new(_policy: Policy) -> Result<Self> {
        Ok(Self)
    }

    pub fn snapshot(&self, scope: CaptureScope) -> Result<Snapshot> {
        let graph = Graph::read();
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
        Ok(Snapshot {
            collectors,
            accesses,
        })
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
        match self.snapshot(CaptureScope {
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
