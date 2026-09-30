use super::procfs::{BootTime, ProcessIdentity};
use crate::{
    collector::CaptureScope,
    model::{
        Access, Activity, Confidence, Device, EnforcementDecision, Evidence, EvidenceKind,
        ProcessContext, Resource, Risk,
    },
};
use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::{
    io::Read,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const MAX_DUMP_BYTES: u64 = 16 * 1024 * 1024;
const DUMP_TIMEOUT: Duration = Duration::from_secs(3);

pub(super) struct Graph {
    objects: Vec<Value>,
}

pub(super) struct Observation {
    pub(super) accesses: Vec<Access>,
    pub(super) audio_error: Option<String>,
    pub(super) video_error: Option<String>,
}

impl Graph {
    pub(super) fn read() -> Result<Self> {
        let output = dump_with_timeout()?;
        let objects: Vec<Value> = serde_json::from_slice(&output)
            .context("pw-dump returned invalid PipeWire graph JSON")?;
        Ok(Self { objects })
    }

    #[cfg(test)]
    pub(super) fn parse(bytes: &[u8]) -> Result<Self> {
        let objects = serde_json::from_slice(bytes).context("invalid PipeWire graph JSON")?;
        Ok(Self { objects })
    }

    fn nodes(&self) -> impl Iterator<Item = &Value> {
        self.objects.iter().filter(|object| kind(object, "Node"))
    }

    fn links(&self) -> impl Iterator<Item = &Value> {
        self.objects.iter().filter(|object| kind(object, "Link"))
    }

    pub(super) fn devices(&self) -> Vec<Device> {
        self.nodes()
            .filter_map(|node| {
                let resource = match property(node, "media.class")? {
                    "Audio/Source" if !is_monitor(node) => Resource::Microphone,
                    "Video/Source" => Resource::Camera,
                    _ => return None,
                };
                let id = property(node, "api.v4l2.path")
                    .filter(|path| path.starts_with("/dev/video"))
                    .or_else(|| property(node, "node.name"))?;
                Some(Device {
                    resource,
                    id: id.to_owned(),
                    name: property(node, "node.description")
                        .or_else(|| property(node, "node.nick"))
                        .unwrap_or(id)
                        .to_owned(),
                })
            })
            .collect()
    }

    // A WirePlumber capture link runs from an Audio/Source or Video/Source
    // output node to a Stream/Input/{Audio,Video} application input node.
    // Both node states must be running and the link active to prove flow.
    fn source_for(&self, stream_id: u64, class: &str) -> Option<(&Value, bool)> {
        let mut inactive = None;
        for link in self.links() {
            if number(link, &["info", "input-node-id"]) != Some(stream_id) {
                continue;
            }
            let Some(source_id) = number(link, &["info", "output-node-id"]) else {
                continue;
            };
            let Some(source) = self.nodes().find(|node| {
                number(node, &["id"]) == Some(source_id)
                    && property(node, "media.class") == Some(class)
                    && (class != "Audio/Source" || !is_monitor(node))
            }) else {
                continue;
            };
            if string(link, &["info", "state"]) == Some("active") {
                return Some((source, true));
            }
            inactive = Some((source, false));
        }
        inactive
    }

    fn process_id(&self, stream: &Value) -> Result<u32, String> {
        let props = stream
            .get("info")
            .and_then(|info| info.get("props"))
            .ok_or_else(|| "capture stream has no PipeWire properties".to_owned())?;
        let client_id = props
            .get("client.id")
            .and_then(value_u64)
            .ok_or_else(|| "capture stream has no owning client.id".to_owned())?;
        let client_props = self
            .objects
            .iter()
            .find(|client| kind(client, "Client") && number(client, &["id"]) == Some(client_id))
            .and_then(|client| client.get("info")?.get("props"))
            .ok_or_else(|| {
                format!("owning PipeWire client {client_id} has no Client properties")
            })?;
        // Only Client pipewire.* properties are security-safe. Node properties,
        // including a node's pipewire.sec.pid, are not client credentials.
        // https://docs.pipewire.org/page_man_pipewire-props_7.html#client-prop__pipewire_sec_pid
        // For PulseAudio, the secure PID belongs to pipewire-pulse; a different
        // application PID must remain unattributed rather than inherit its trust.
        let secure_pid = client_props
            .get("pipewire.sec.pid")
            .and_then(value_u32)
            .filter(|pid| *pid != 0)
            .ok_or_else(|| {
                format!(
                    "owning PipeWire client {client_id} has no valid protocol-set pipewire.sec.pid"
                )
            })?;
        let claimed_pid = props
            .get("application.process.id")
            .or_else(|| client_props.get("application.process.id"))
            .and_then(value_u32)
            .ok_or_else(|| {
                "capture client has no valid claimed application.process.id".to_owned()
            })?;
        if claimed_pid != secure_pid {
            return Err(format!(
                "claimed application.process.id {claimed_pid} does not match owning PipeWire client {client_id} protocol-set pipewire.sec.pid {secure_pid}"
            ));
        }
        Ok(secure_pid)
    }

    pub(super) fn observe(&self, scope: CaptureScope, boot: Option<&BootTime>) -> Observation {
        let mut result = Observation {
            accesses: Vec::new(),
            audio_error: None,
            video_error: None,
        };
        for stream in self.nodes() {
            let (resource, source_class, warning) = match property(stream, "media.class") {
                Some("Stream/Input/Audio") if scope.microphone => (
                    Resource::Microphone,
                    "Audio/Source",
                    &mut result.audio_error,
                ),
                Some("Stream/Input/Video") if scope.camera => {
                    (Resource::Camera, "Video/Source", &mut result.video_error)
                }
                _ => continue,
            };
            let Some(stream_id) = number(stream, &["id"]) else {
                *warning = Some("capture stream has no PipeWire node ID".to_owned());
                continue;
            };
            let state = string(stream, &["info", "state"]);
            if !matches!(state, Some("running" | "idle" | "suspended")) {
                *warning = Some(format!(
                    "capture node {stream_id} has unknown/error state: {state:?}"
                ));
                continue;
            }
            let source = self.source_for(stream_id, source_class);
            let confirmed = state == Some("running")
                && source.is_some_and(|(node, active)| {
                    active && string(node, &["info", "state"]) == Some("running")
                });
            if state == Some("running") && !confirmed {
                *warning = Some(format!(
                    "capture node {stream_id} is running but has no confirmed active link to {source_class}"
                ));
            }
            if !confirmed && !scope.include_ready {
                continue;
            }

            let authenticated_pid = self.process_id(stream);
            let identity = match (authenticated_pid.as_ref().ok().copied(), boot) {
                (Some(pid), Some(boot)) => ProcessIdentity::verify(pid, boot).ok(),
                _ => None,
            };
            if identity.is_none() {
                let reason = authenticated_pid.as_ref().err().map_or(
                    "authenticated PipeWire client PID could not be verified against /proc",
                    String::as_str,
                );
                warning.get_or_insert_with(|| format!("capture node {stream_id}: {reason}"));
            }
            let app = identity
                .as_ref()
                .map(|id| id.name.as_str())
                .unwrap_or("Unknown PipeWire capture client");
            let mut evidence = vec![Evidence::new(
                EvidenceKind::LiveApi,
                "pipewire_node",
                format!(
                    "PipeWire capture stream node {stream_id} state={}",
                    state.unwrap_or("unknown")
                ),
            )];
            evidence.push(Evidence::new(
                EvidenceKind::LiveApi,
                "pipewire_client",
                match &authenticated_pid {
                    Ok(pid) => format!(
                        "application.process.id {pid} matches owning Client protocol-set pipewire.sec.pid"
                    ),
                    Err(reason) => format!("process attribution rejected: {reason}"),
                },
            ));
            if let Some((source, active)) = source {
                evidence.push(Evidence::new(
                    EvidenceKind::LiveApi,
                    "pipewire_link",
                    format!(
                        "source {} link={} source_state={}",
                        property(source, "node.name").unwrap_or("unnamed"),
                        if active { "active" } else { "inactive" },
                        string(source, &["info", "state"]).unwrap_or("unknown")
                    ),
                ));
            }
            if let Some(identity) = &identity {
                evidence.push(Evidence::new(
                    EvidenceKind::ProcessLineage,
                    "procfs",
                    format!(
                        "PID {} executable {} verified with /proc starttime {}",
                        identity.pid, identity.executable, identity.created_at
                    ),
                ));
            } else {
                evidence.push(Evidence::new(
                    EvidenceKind::ProcessLineage,
                    "procfs",
                    "no authenticated capture client PID with verified /proc/pid/stat and /proc/pid/exe identity",
                ));
            }
            let source_id = source.and_then(|(node, _)| number(node, &["id"]));
            let instance = identity
                .as_ref()
                .map(|id| id.instance_id.as_str())
                .unwrap_or("unverified");
            result.accesses.push(Access {
                key: format!("linux:{resource}:{stream_id}:{source_id:?}:{instance}"),
                resource,
                activity: if confirmed {
                    Activity::Active
                } else {
                    Activity::Ready
                },
                risk: Risk::Unexplained,
                confidence: if confirmed && identity.is_some() {
                    Confidence::High
                } else {
                    Confidence::Low
                },
                enforcement: if identity.is_some() {
                    EnforcementDecision::Alert
                } else {
                    EnforcementDecision::Unknown
                },
                application: app.to_owned(),
                pid: identity.as_ref().map(|id| id.pid),
                parent_pid: identity.as_ref().and_then(|id| id.parent_pid),
                parent_name: None,
                executable: identity.as_ref().map(|id| id.executable.clone()),
                signature: None,
                device: source
                    .and_then(|(node, _)| {
                        property(node, "node.description").or_else(|| property(node, "node.name"))
                    })
                    .map(str::to_owned),
                started_at: None,
                modules: Vec::new(),
                evidence,
                process: identity.map(|id| ProcessContext {
                    instance_id: id.instance_id,
                    user: Some(id.uid.to_string()),
                    ..ProcessContext::default()
                }),
            });
        }
        result
    }
}
fn value_u64(value: &Value) -> Option<u64> {
    value.as_u64().or_else(|| value.as_str()?.parse().ok())
}

fn value_u32(value: &Value) -> Option<u32> {
    u32::try_from(value_u64(value)?).ok()
}

fn kind(value: &Value, kind: &str) -> bool {
    value
        .get("type")
        .and_then(Value::as_str)
        .and_then(|name| name.strip_prefix("PipeWire:Interface:"))
        .is_some_and(|name| name.split('/').next() == Some(kind))
}

fn property<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get("info")?.get("props")?.get(key)?.as_str()
}

fn string<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    path.iter()
        .try_fold(value, |node, key| node.get(*key))?
        .as_str()
}

fn number(value: &Value, path: &[&str]) -> Option<u64> {
    path.iter()
        .try_fold(value, |node, key| node.get(*key))?
        .as_u64()
}

fn is_monitor(node: &Value) -> bool {
    property(node, "node.name").is_some_and(|name| name.ends_with(".monitor"))
}

fn dump_with_timeout() -> Result<Vec<u8>> {
    let mut child = Command::new("pw-dump")
        .arg("--no-colors")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("cannot launch pw-dump; install the PipeWire client tools")?;
    let stdout = child.stdout.take().context("missing pw-dump stdout")?;
    let mut stderr = child.stderr.take().context("missing pw-dump stderr")?;
    let output = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.take(MAX_DUMP_BYTES + 1).read_to_end(&mut bytes)?;
        Ok::<_, std::io::Error>(bytes)
    });
    let errors = thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes)?;
        Ok::<_, std::io::Error>(bytes)
    });
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if start.elapsed() >= DUMP_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            let _ = output.join();
            let _ = errors.join();
            bail!("pw-dump timed out (PipeWire server unavailable or unresponsive)");
        }
        thread::sleep(Duration::from_millis(20));
    };
    let bytes = output
        .join()
        .map_err(|_| anyhow::anyhow!("pw-dump stdout reader failed"))??;
    let stderr = errors
        .join()
        .map_err(|_| anyhow::anyhow!("pw-dump stderr reader failed"))??;
    if bytes.len() as u64 > MAX_DUMP_BYTES {
        bail!("pw-dump graph exceeds {MAX_DUMP_BYTES} bytes");
    }
    if !status.success() {
        bail!(
            "pw-dump failed: {}",
            String::from_utf8_lossy(&stderr).trim()
        );
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_capture_requires_running_stream_running_source_and_active_link() {
        let raw = br#"[
            {"id":5,"type":"PipeWire:Interface:Node/3","info":{"state":"running","props":{"media.class":"Audio/Source","node.name":"mic"}}},
            {"id":6,"type":"PipeWire:Interface:Node/3","info":{"state":"running","props":{"media.class":"Video/Source","node.name":"camera","api.v4l2.path":"/dev/video2"}}},
            {"id":10,"type":"PipeWire:Interface:Node/3","info":{"state":"running","props":{"media.class":"Stream/Input/Audio","application.name":"Recorder"}}},
            {"id":11,"type":"PipeWire:Interface:Node/3","info":{"state":"running","props":{"media.class":"Stream/Input/Video","application.name":"Viewer"}}},
            {"id":20,"type":"PipeWire:Interface:Link/3","info":{"input-node-id":10,"output-node-id":5,"state":"active"}},
            {"id":21,"type":"PipeWire:Interface:Link/3","info":{"input-node-id":11,"output-node-id":6,"state":"active"}}
        ]"#;
        let graph = Graph::parse(raw).unwrap();
        let accesses = graph
            .observe(
                CaptureScope {
                    include_ready: true,
                    ..CaptureScope::all()
                },
                None,
            )
            .accesses;
        assert_eq!(accesses.len(), 2);
        assert!(
            accesses
                .iter()
                .all(|access| access.activity == Activity::Active)
        );
        assert!(accesses.iter().all(
            |access| access.pid.is_none() && access.enforcement == EnforcementDecision::Unknown
        ));
        assert_eq!(
            graph
                .devices()
                .iter()
                .find(|dev| dev.resource == Resource::Camera)
                .unwrap()
                .id,
            "/dev/video2"
        );
        let idle = String::from_utf8(raw.to_vec())
            .unwrap()
            .replace("\"state\":\"active\"", "\"state\":\"paused\"");
        let idle_graph = Graph::parse(idle.as_bytes()).unwrap();
        assert!(
            idle_graph
                .observe(CaptureScope::all(), None)
                .accesses
                .is_empty()
        );
        assert!(
            idle_graph
                .observe(
                    CaptureScope {
                        include_ready: true,
                        ..CaptureScope::all()
                    },
                    None
                )
                .accesses
                .iter()
                .all(|access| access.activity == Activity::Ready)
        );
    }
    #[test]
    fn failed_or_unlinked_pipewire_stream_cannot_claim_active_video() {
        let error = Graph::parse(br#"[
            {"id":6,"type":"PipeWire:Interface:Node/3","info":{"state":"running","props":{"media.class":"Video/Source","node.name":"camera"}}},
            {"id":11,"type":"PipeWire:Interface:Node/3","info":{"state":"error","error":"camera removed","props":{"media.class":"Stream/Input/Video"}}}
        ]"#).unwrap();
        let failed = error.observe(
            CaptureScope {
                include_ready: true,
                ..CaptureScope::all()
            },
            None,
        );
        assert!(failed.accesses.is_empty());
        assert!(failed.video_error.as_deref().unwrap().contains("error"));

        let unlinked = Graph::parse(br#"[
            {"id":6,"type":"PipeWire:Interface:Node/3","info":{"state":"running","props":{"media.class":"Video/Source","node.name":"camera"}}},
            {"id":11,"type":"PipeWire:Interface:Node/3","info":{"state":"running","props":{"media.class":"Stream/Input/Video"}}}
        ]"#).unwrap();
        let observation = unlinked.observe(
            CaptureScope {
                include_ready: true,
                ..CaptureScope::all()
            },
            None,
        );
        assert_eq!(observation.accesses.len(), 1);
        assert_eq!(observation.accesses[0].activity, Activity::Ready);
        assert!(
            observation
                .video_error
                .as_deref()
                .unwrap()
                .contains("no confirmed active link")
        );
        assert!(Graph::parse(b"not valid JSON").is_err());
    }

    #[test]
    fn client_process_property_is_used_only_when_it_matches_the_secure_client_pid() {
        let graph = Graph::parse(br#"[
            {"id":30,"type":"PipeWire:Interface:Client/3","info":{"props":{"application.process.id":321,"pipewire.sec.pid":"321"}}},
            {"id":31,"type":"PipeWire:Interface:Node/3","info":{"state":"idle","props":{"media.class":"Stream/Input/Audio","client.id":"30"}}}
        ]"#).unwrap();
        let node = graph.nodes().next().unwrap();
        assert_eq!(graph.process_id(node), Ok(321));
    }

    #[test]
    fn spoofed_stream_pid_cannot_override_authenticated_client_pid() {
        let graph = Graph::parse(br#"[
            {"id":5,"type":"PipeWire:Interface:Node/3","info":{"state":"running","props":{"media.class":"Audio/Source","node.name":"mic"}}},
            {"id":30,"type":"PipeWire:Interface:Client/3","info":{"props":{"application.process.id":654,"pipewire.sec.pid":654}}},
            {"id":31,"type":"PipeWire:Interface:Node/3","info":{"state":"running","props":{"media.class":"Stream/Input/Audio","client.id":30,"application.process.id":321,"application.name":"Spoofed app","pipewire.sec.pid":321}}},
            {"id":40,"type":"PipeWire:Interface:Link/3","info":{"input-node-id":31,"output-node-id":5,"state":"active"}}
        ]"#).unwrap();
        let stream = graph
            .nodes()
            .find(|node| number(node, &["id"]) == Some(31))
            .unwrap();
        let rejection = graph.process_id(stream).unwrap_err();
        assert!(rejection.contains("application.process.id 321"));
        assert!(rejection.contains("pipewire.sec.pid 654"));
        let observation = graph.observe(CaptureScope::all(), None);
        let access = &observation.accesses[0];
        assert_eq!(access.activity, Activity::Active);
        assert_eq!(access.confidence, Confidence::Low);
        assert_eq!(access.enforcement, EnforcementDecision::Unknown);
        assert_eq!(access.application, "Unknown PipeWire capture client");
        assert!(access.pid.is_none());
        assert!(access.executable.is_none());
        assert!(access.process.is_none());
        assert!(
            observation
                .audio_error
                .as_deref()
                .unwrap()
                .contains(&rejection)
        );
        assert!(
            access
                .evidence
                .iter()
                .any(|evidence| evidence.source == "pipewire_client"
                    && evidence.detail.contains(&rejection))
        );
    }

    #[test]
    fn node_secure_pid_cannot_replace_missing_client_secure_pid() {
        let graph = Graph::parse(br#"[
            {"id":30,"type":"PipeWire:Interface:Client/3","info":{"props":{"application.process.id":321}}},
            {"id":31,"type":"PipeWire:Interface:Node/3","info":{"state":"idle","props":{"media.class":"Stream/Input/Audio","client.id":30,"application.process.id":321,"pipewire.sec.pid":321}}}
        ]"#).unwrap();
        let node = graph.nodes().next().unwrap();
        assert!(
            graph
                .process_id(node)
                .unwrap_err()
                .contains("no valid protocol-set pipewire.sec.pid")
        );
    }
}
