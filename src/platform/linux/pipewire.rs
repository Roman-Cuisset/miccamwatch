use super::procfs::{BootTime, ProcessIdentity};
use crate::{
    collector::CaptureScope,
    model::{
        Access, Activity, Confidence, Device, EnforcementDecision, Evidence, EvidenceKind,
        ProcessContext, Resource, Risk,
    },
};
use anyhow::{Context, Result, bail};
use serde::{
    Deserialize,
    de::{self, IgnoredAny, SeqAccess, Visitor},
};
use serde_json::{Map, Value};
use std::{
    fmt,
    io::{self, Read},
    os::unix::io::AsRawFd,
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

const MAX_DUMP_BYTES: usize = 16 * 1024 * 1024;
const MAX_STDERR_BYTES: usize = 1024 * 1024;
const STDERR_RETAIN_BYTES: usize = 8 * 1024;
const MAX_JSON_STRING_BYTES: usize = 64 * 1024;
const MAX_FIELD_BYTES: usize = 4096;
const MAX_GRAPH_OBJECTS: usize = 65_536;
const MAX_RETAINED_OBJECTS: usize = 4096;
const MAX_RETAINED_TEXT_BYTES: usize = 512 * 1024;
const DUMP_TIMEOUT: Duration = Duration::from_secs(3);

pub(super) struct Graph {
    objects: Vec<Value>,
}

pub(super) struct CaptureSource {
    pub(super) id: Option<u32>,
    pub(super) serial: Option<u64>,
    pub(super) name: String,
    pub(super) virtual_source: bool,
    pub(super) monitor: bool,
}

pub(super) struct Observation {
    pub(super) accesses: Vec<Access>,
    pub(super) audio_error: Option<String>,
    pub(super) video_error: Option<String>,
}

impl Graph {
    pub(super) fn read() -> Result<Self> {
        let mut command = Command::new("pw-dump");
        command.arg("--no-colors");
        read_command(&mut command, DUMP_TIMEOUT)
    }

    fn from_reader(reader: impl Read) -> Result<Self> {
        let mut decoder = serde_json::Deserializer::from_reader(BoundedJson::new(reader));
        let graph = Graph::deserialize(&mut decoder)
            .context("pw-dump returned invalid or over-limit PipeWire graph JSON")?;
        decoder.end().context("pw-dump graph did not end cleanly")?;
        Ok(graph)
    }

    #[cfg(test)]
    pub(super) fn parse(bytes: &[u8]) -> Result<Self> {
        Self::from_reader(bytes)
    }

    fn nodes(&self) -> impl Iterator<Item = &Value> {
        self.objects.iter().filter(|object| kind(object, "Node"))
    }

    fn links(&self) -> impl Iterator<Item = &Value> {
        self.objects.iter().filter(|object| kind(object, "Link"))
    }

    pub(super) fn core_cookie(&self) -> Option<u32> {
        self.objects
            .iter()
            .find(|object| kind(object, "Core"))
            .and_then(|object| object.get("info")?.get("cookie"))
            .and_then(value_u32)
    }

    pub(super) fn capture_sources(&self) -> Vec<CaptureSource> {
        self.nodes()
            .filter(|node| {
                matches!(
                    property(node, "media.class"),
                    Some("Audio/Source" | "Audio/Source/Virtual")
                )
            })
            .map(|node| CaptureSource {
                id: number(node, &["id"]).and_then(|id| u32::try_from(id).ok()),
                serial: node
                    .get("info")
                    .and_then(|info| info.get("props"))
                    .and_then(|props| props.get("object.serial"))
                    .and_then(value_u64),
                name: property(node, "node.description")
                    .or_else(|| property(node, "node.nick"))
                    .or_else(|| property(node, "node.name"))
                    .unwrap_or("Unnamed PipeWire source")
                    .to_owned(),
                virtual_source: property(node, "media.class") == Some("Audio/Source/Virtual")
                    || property(node, "node.virtual") == Some("true"),
                monitor: is_monitor(node) || property(node, "stream.monitor") == Some("true"),
            })
            .collect()
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
    interface_kind(value.get("type").unwrap_or(&Value::Null), kind)
}

fn interface_kind(value: &Value, kind: &str) -> bool {
    value
        .as_str()
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

// Unknown fields use serde's IgnoredAny path, not a temporary Value tree.
#[derive(Default)]
struct Scalar {
    value: Value,
    present: bool,
}

impl<'de> Deserialize<'de> for Scalar {
    fn deserialize<D: de::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        struct ScalarVisitor;
        impl<'de> Visitor<'de> for ScalarVisitor {
            type Value = Scalar;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a bounded PipeWire property")
            }
            fn visit_str<E: de::Error>(self, value: &str) -> Result<Scalar, E> {
                if value.len() > MAX_FIELD_BYTES {
                    return Err(E::custom("PipeWire retained field exceeds 4096 bytes"));
                }
                Ok(Scalar {
                    value: Value::String(value.to_owned()),
                    present: true,
                })
            }
            fn visit_u64<E: de::Error>(self, value: u64) -> Result<Scalar, E> {
                Ok(Scalar {
                    value: value.into(),
                    present: true,
                })
            }
            fn visit_i64<E: de::Error>(self, value: i64) -> Result<Scalar, E> {
                Ok(Scalar {
                    value: value.into(),
                    present: true,
                })
            }
            fn visit_f64<E: de::Error>(self, value: f64) -> Result<Scalar, E> {
                Ok(Scalar {
                    value: Value::from(value),
                    present: true,
                })
            }
            fn visit_bool<E: de::Error>(self, value: bool) -> Result<Scalar, E> {
                Ok(Scalar {
                    value: value.into(),
                    present: true,
                })
            }
            fn visit_unit<E: de::Error>(self) -> Result<Scalar, E> {
                Ok(Scalar {
                    value: Value::Null,
                    present: true,
                })
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Scalar, A::Error> {
                while sequence.next_element::<IgnoredAny>()?.is_some() {}
                self.visit_unit()
            }
            fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<Scalar, A::Error> {
                while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
                self.visit_unit()
            }
        }
        decoder.deserialize_any(ScalarVisitor)
    }
}

impl Scalar {
    fn insert(self, map: &mut Map<String, Value>, key: &str) {
        if self.present {
            map.insert(key.to_owned(), self.value);
        }
    }
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct Props {
    #[serde(rename = "media.class")]
    media_class: Scalar,
    #[serde(rename = "node.description")]
    description: Scalar,
    #[serde(rename = "node.nick")]
    nick: Scalar,
    #[serde(rename = "node.name")]
    name: Scalar,
    #[serde(rename = "node.virtual")]
    virtual_source: Scalar,
    #[serde(rename = "stream.monitor")]
    monitor: Scalar,
    #[serde(rename = "api.v4l2.path")]
    video_path: Scalar,
    #[serde(rename = "object.serial")]
    serial: Scalar,
    #[serde(rename = "client.id")]
    client_id: Scalar,
    #[serde(rename = "pipewire.sec.pid")]
    secure_pid: Scalar,
    #[serde(rename = "application.process.id")]
    claimed_pid: Scalar,
}

impl Props {
    fn into_value(self) -> Value {
        let mut map = Map::new();
        self.media_class.insert(&mut map, "media.class");
        self.description.insert(&mut map, "node.description");
        self.nick.insert(&mut map, "node.nick");
        self.name.insert(&mut map, "node.name");
        self.virtual_source.insert(&mut map, "node.virtual");
        self.monitor.insert(&mut map, "stream.monitor");
        self.video_path.insert(&mut map, "api.v4l2.path");
        self.serial.insert(&mut map, "object.serial");
        self.client_id.insert(&mut map, "client.id");
        self.secure_pid.insert(&mut map, "pipewire.sec.pid");
        self.claimed_pid.insert(&mut map, "application.process.id");
        Value::Object(map)
    }
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct Info {
    cookie: Scalar,
    state: Scalar,
    #[serde(rename = "input-node-id")]
    input: Scalar,
    #[serde(rename = "output-node-id")]
    output: Scalar,
    props: Option<Props>,
}

#[derive(Deserialize)]
struct ProjectedObject {
    #[serde(default)]
    id: Scalar,
    #[serde(rename = "type", default)]
    kind: Scalar,
    #[serde(default)]
    info: Option<Info>,
}

impl ProjectedObject {
    fn relevant(&self) -> bool {
        interface_kind(&self.kind.value, "Core")
            || interface_kind(&self.kind.value, "Client")
            || interface_kind(&self.kind.value, "Link")
            || (interface_kind(&self.kind.value, "Node")
                && matches!(
                    self.info
                        .as_ref()
                        .and_then(|info| info.props.as_ref())
                        .and_then(|props| props.media_class.value.as_str()),
                    Some(
                        "Audio/Source"
                            | "Audio/Source/Virtual"
                            | "Video/Source"
                            | "Stream/Input/Audio"
                            | "Stream/Input/Video"
                    )
                ))
    }

    fn into_value(self) -> Value {
        let mut map = Map::new();
        self.id.insert(&mut map, "id");
        self.kind.insert(&mut map, "type");
        if let Some(info) = self.info {
            let mut fields = Map::new();
            info.cookie.insert(&mut fields, "cookie");
            info.state.insert(&mut fields, "state");
            info.input.insert(&mut fields, "input-node-id");
            info.output.insert(&mut fields, "output-node-id");
            if let Some(props) = info.props {
                fields.insert("props".to_owned(), props.into_value());
            }
            map.insert("info".to_owned(), Value::Object(fields));
        }
        Value::Object(map)
    }
}

fn text_bytes(value: &Value) -> usize {
    match value {
        Value::String(text) => text.len(),
        Value::Object(map) => map
            .iter()
            .map(|(key, value)| key.len() + text_bytes(value))
            .sum(),
        _ => 0,
    }
}

impl<'de> Deserialize<'de> for Graph {
    fn deserialize<D: de::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        struct GraphVisitor;
        impl<'de> Visitor<'de> for GraphVisitor {
            type Value = Graph;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a complete bounded PipeWire graph array")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Graph, A::Error> {
                let mut objects = Vec::new();
                let mut count = 0;
                let mut retained_text = 0;
                while let Some(object) = sequence.next_element::<ProjectedObject>()? {
                    count += 1;
                    if count > MAX_GRAPH_OBJECTS {
                        return Err(de::Error::custom(
                            "PipeWire graph object count exceeds 65536",
                        ));
                    }
                    if !object.relevant() {
                        continue;
                    }
                    if objects.len() == MAX_RETAINED_OBJECTS {
                        return Err(de::Error::custom(
                            "PipeWire relevant object count exceeds 4096",
                        ));
                    }
                    let object = object.into_value();
                    retained_text += text_bytes(&object);
                    if retained_text > MAX_RETAINED_TEXT_BYTES {
                        return Err(de::Error::custom(
                            "PipeWire retained graph text exceeds 524288 bytes",
                        ));
                    }
                    objects.push(object);
                }
                Ok(Graph { objects })
            }
        }
        decoder.deserialize_seq(GraphVisitor)
    }
}

// Bound decoder scratch space, including unknown keys and strings. Large
// ignored arrays/maps stream without retaining their contents. Every limit
// rejects the entire observation rather than silently truncating the graph.
struct BoundedJson<R> {
    reader: R,
    bytes: usize,
    in_string: bool,
    escaped: bool,
    string_bytes: usize,
    depth: usize,
}

impl<R> BoundedJson<R> {
    fn new(reader: R) -> Self {
        Self {
            reader,
            bytes: 0,
            in_string: false,
            escaped: false,
            string_bytes: 0,
            depth: 0,
        }
    }
}

impl<R: Read> Read for BoundedJson<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let count = self.reader.read(buffer)?;
        self.bytes += count;
        if self.bytes > MAX_DUMP_BYTES {
            return Err(io::Error::other("pw-dump graph exceeds 16777216 bytes"));
        }
        for &byte in &buffer[..count] {
            if self.in_string {
                if !self.escaped && byte == b'"' {
                    self.in_string = false;
                    continue;
                }
                self.string_bytes += 1;
                if self.string_bytes > MAX_JSON_STRING_BYTES {
                    return Err(io::Error::other(
                        "pw-dump JSON string exceeds 65536 encoded bytes",
                    ));
                }
                if self.escaped {
                    self.escaped = false;
                } else if byte == b'\\' {
                    self.escaped = true;
                }
            } else if byte == b'"' {
                self.in_string = true;
                self.string_bytes = 0;
            } else if matches!(byte, b'[' | b'{') {
                self.depth += 1;
                if self.depth > 128 {
                    return Err(io::Error::other("pw-dump JSON nesting exceeds 128"));
                }
            } else if matches!(byte, b']' | b'}') {
                self.depth = self.depth.saturating_sub(1);
            }
        }
        Ok(count)
    }
}

fn nonblocking(pipe: &impl AsRawFd) -> io::Result<()> {
    let fd = pipe.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags == -1 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

// waitid(WNOWAIT) observes completion without releasing the direct child's
// PID. That unreaped child pins its dedicated PGID until group cleanup has
// finished; no group signal is ever sent after the final wait/reap.
struct PinnedDumpChild {
    child: Child,
    owns_pid: bool,
}

impl PinnedDumpChild {
    fn completion(&mut self) -> io::Result<Option<ExitStatus>> {
        use std::os::unix::process::ExitStatusExt;
        let mut info = unsafe { std::mem::zeroed::<libc::siginfo_t>() };
        let result = unsafe {
            libc::waitid(
                libc::P_PID,
                self.child.id(),
                &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        if result == -1 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::ECHILD) {
                // An external reaper/auto-reap configuration removed our
                // ownership pin. Fail explicitly, never signal a recycled ID.
                self.owns_pid = false;
            }
            if error.kind() == io::ErrorKind::Interrupted {
                return Ok(None);
            }
            return Err(error);
        }
        if unsafe { info.si_pid() } == 0 {
            return Ok(None);
        }
        let code = unsafe { info.si_status() };
        let raw_status = match info.si_code {
            libc::CLD_EXITED => code << 8,
            libc::CLD_KILLED => code,
            libc::CLD_DUMPED => code | 0x80,
            _ => {
                return Err(io::Error::other(
                    "pw-dump has unexpected waitid completion state",
                ));
            }
        };
        Ok(Some(ExitStatus::from_raw(raw_status)))
    }

    fn cleanup(&mut self) -> io::Result<()> {
        if !self.owns_pid {
            return Ok(());
        }
        let group_result = unsafe { libc::kill(-(self.child.id() as i32), libc::SIGKILL) };
        let group_error = (group_result == -1)
            .then(io::Error::last_os_error)
            .filter(|error| error.raw_os_error() != Some(libc::ESRCH));
        // Also terminate the immediate child if group cleanup failed.
        let child_result = self.child.kill();
        let wait_result = self.child.wait();
        if wait_result.is_ok()
            || wait_result
                .as_ref()
                .err()
                .is_some_and(|error| error.raw_os_error() == Some(libc::ECHILD))
        {
            self.owns_pid = false;
        }
        if let Some(error) = group_error {
            return Err(error);
        }
        if let Err(error) = child_result {
            // A pidfd signal may return ESRCH for an already-exited child.
            // The wait below still proves that the owned PID was reaped.
            if error.raw_os_error() != Some(libc::ESRCH) {
                return Err(error);
            }
        }
        wait_result?;
        Ok(())
    }
}

impl Drop for PinnedDumpChild {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

struct DumpReader {
    child: PinnedDumpChild,
    stdout: std::process::ChildStdout,
    stderr: std::process::ChildStderr,
    deadline: Instant,
    status: Option<ExitStatus>,
    stdout_eof: bool,
    stderr_eof: bool,
    stderr_bytes: usize,
    diagnostic: Vec<u8>,
    buffer: [u8; 8192],
    offset: usize,
    filled: usize,
}

impl DumpReader {
    fn drain_stderr(&mut self) -> io::Result<()> {
        let mut buffer = [0u8; 8192];
        // A continuously writing stderr cannot starve stdout or status checks.
        // Retain a diagnostic prefix but count every byte.
        for _ in 0..8 {
            if self.stderr_eof {
                break;
            }
            match self.stderr.read(&mut buffer) {
                Ok(0) => {
                    self.stderr_eof = true;
                    break;
                }
                Ok(count) => {
                    self.stderr_bytes += count;
                    let keep = count.min(STDERR_RETAIN_BYTES - self.diagnostic.len());
                    self.diagnostic.extend_from_slice(&buffer[..keep]);
                    if self.stderr_bytes > MAX_STDERR_BYTES {
                        return Err(io::Error::other("pw-dump stderr exceeds 1048576 bytes"));
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    fn diagnostic(&self) -> String {
        let suffix = if self.stderr_bytes > self.diagnostic.len() {
            " [stderr diagnostic truncated; complete graph required]"
        } else {
            ""
        };
        format!(
            "{}{}",
            String::from_utf8_lossy(&self.diagnostic).trim(),
            suffix
        )
    }
}

impl Read for DumpReader {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        while self.offset == self.filled {
            if Instant::now() >= self.deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "pw-dump timed out before complete output and child exit",
                ));
            }
            self.drain_stderr()?;
            if !self.stdout_eof {
                match self.stdout.read(&mut self.buffer) {
                    Ok(0) => self.stdout_eof = true,
                    Ok(count) => {
                        self.offset = 0;
                        self.filled = count;
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(error) => return Err(error),
                }
            }
            if self.status.is_none() {
                self.status = self.child.completion()?;
            }
            if self.offset != self.filled {
                break;
            }
            if self.stdout_eof && self.stderr_eof && self.status.is_some() {
                return Ok(0);
            }
            thread::sleep(Duration::from_millis(5));
        }
        let count = output.len().min(self.filled - self.offset);
        output[..count].copy_from_slice(&self.buffer[self.offset..self.offset + count]);
        self.offset += count;
        Ok(count)
    }
}

fn read_command(command: &mut Command, timeout: Duration) -> Result<Graph> {
    use std::os::unix::process::CommandExt;
    let deadline = Instant::now() + timeout;
    let mut child = PinnedDumpChild {
        child: command
            .process_group(0)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("cannot launch pw-dump; install the PipeWire client tools")?,
        owns_pid: true,
    };
    let stdout = child
        .child
        .stdout
        .take()
        .context("missing pw-dump stdout")?;
    let stderr = child
        .child
        .stderr
        .take()
        .context("missing pw-dump stderr")?;
    nonblocking(&stdout)?;
    nonblocking(&stderr)?;
    let mut reader = DumpReader {
        child,
        stdout,
        stderr,
        deadline,
        status: None,
        stdout_eof: false,
        stderr_eof: false,
        stderr_bytes: 0,
        diagnostic: Vec::with_capacity(STDERR_RETAIN_BYTES),
        buffer: [0; 8192],
        offset: 0,
        filled: 0,
    };
    let graph = Graph::from_reader(&mut reader)
        .with_context(|| format!("pw-dump capture failed: {}", reader.diagnostic()))?;
    if !reader.status.is_some_and(|status| status.success()) {
        bail!(
            "pw-dump failed ({:?}): {}",
            reader.status,
            reader.diagnostic()
        );
    }
    reader
        .child
        .cleanup()
        .context("cannot clean up and reap owned pw-dump helper")?;
    if Instant::now() >= deadline {
        bail!("pw-dump timed out before completion and cleanup");
    }
    Ok(graph)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn shell(script: &str, timeout: Duration) -> Result<Graph> {
        let mut command = Command::new("sh");
        command.args(["-c", script]);
        read_command(&mut command, timeout)
    }

    fn repeated_graph(object: &str, count: usize) -> String {
        format!("[{}]", vec![object; count].join(","))
    }

    #[test]
    fn child_completion_keeps_pid_pinned_until_owned_group_cleanup_and_reap() {
        use std::os::unix::process::CommandExt;
        for (script, expected_code, inherited_pipe) in [
            ("exit 0", 0, false),
            ("exit 7", 7, false),
            ("sleep 30 & exit 7", 7, true),
        ] {
            let mut command = Command::new("sh");
            command
                .args(["-c", script])
                .process_group(0)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            let mut child = PinnedDumpChild {
                child: command.spawn().unwrap(),
                owns_pid: true,
            };
            let pid = child.child.id();
            let mut stdout = child.child.stdout.take().unwrap();
            nonblocking(&stdout).unwrap();
            let deadline = Instant::now() + Duration::from_secs(3);
            let status = loop {
                if let Some(status) = child.completion().unwrap() {
                    break status;
                }
                assert!(Instant::now() < deadline, "helper did not exit");
                thread::sleep(Duration::from_millis(5));
            };
            assert_eq!(status.code(), Some(expected_code));

            // Independent kernel observation proves completion did not reap
            // the child, including nonzero exit and descriptor-orphan cases.
            let mut info = unsafe { std::mem::zeroed::<libc::siginfo_t>() };
            let result = unsafe {
                libc::waitid(
                    libc::P_PID,
                    pid,
                    &mut info,
                    libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
                )
            };
            assert_eq!(result, 0);
            assert_eq!(unsafe { info.si_pid() }, pid as i32);
            assert_eq!(unsafe { info.si_status() }, expected_code);
            if inherited_pipe {
                let error = stdout.read(&mut [0u8; 1]).unwrap_err();
                assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
            }

            child.cleanup().unwrap();
            let result = unsafe {
                libc::waitid(
                    libc::P_PID,
                    pid,
                    &mut info,
                    libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
                )
            };
            assert_eq!(result, -1);
            assert_eq!(
                io::Error::last_os_error().raw_os_error(),
                Some(libc::ECHILD)
            );
            // Group cleanup must also close the orphan's inherited pipe.
            loop {
                match stdout.read(&mut [0u8; 1]) {
                    Ok(0) => break,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        assert!(
                            Instant::now() < deadline,
                            "owned descendant retained stdout"
                        );
                        thread::sleep(Duration::from_millis(5));
                    }
                    other => panic!("unexpected helper stdout after cleanup: {other:?}"),
                }
            }
            // Dropping after explicit cleanup is a no-op; it cannot signal
            // the now-released numeric PID/PGID.
            drop(child);
        }
    }

    #[test]
    fn projection_streams_high_cardinality_and_large_irrelevant_values() {
        let object = r#"{"type":"PipeWire:Interface:Port/3","params":{"ignored":[1,2,3]}}"#;
        let graph = Graph::parse(repeated_graph(object, 16_384).as_bytes()).unwrap();
        assert!(graph.objects.is_empty());
        let irrelevant = "0,".repeat(250_000);
        let raw = format!(
            r#"[{{"id":1,"type":"PipeWire:Interface:Core/3","info":{{"cookie":"42","params":[{irrelevant}0]}}}},
            {{"id":5,"type":"PipeWire:Interface:Node/3","info":{{"state":"running","props":{{"media.class":"Audio/Source","object.serial":"123","node.name":"mic","node.virtual":"true","stream.monitor":"true","unused":"{}"}}}}}},
            {{"id":6,"type":"PipeWire:Interface:Node/3","info":{{"props":{{"media.class":"Audio/Source/Virtual","node.name":"virtual"}}}}}},
            {{"id":7,"type":"PipeWire:Interface:Node/3","info":{{"props":{{"media.class":"Audio/Source","node.name":"sink.monitor"}}}}}},
            {{"id":8,"type":"PipeWire:Interface:Node/3","info":{{"props":{{"media.class":"Audio/Sink","node.name":"speaker"}}}}}}]"#,
            "x".repeat(32_768)
        );
        let graph = Graph::parse(raw.as_bytes()).unwrap();
        assert_eq!(graph.core_cookie(), Some(42));
        assert_eq!(graph.objects.len(), 4);
        let sources = graph.capture_sources();
        assert_eq!(sources.len(), 3);
        assert_eq!(sources[0].id, Some(5));
        assert_eq!(sources[0].serial, Some(123));
        assert!(sources[0].virtual_source && sources[0].monitor);
        assert!(sources[1].virtual_source);
        assert!(sources[2].monitor);
        assert_eq!(graph.devices().len(), 1);
        assert!(graph.objects[1]["info"].get("params").is_none());
        assert!(graph.objects[1]["info"]["props"].get("unused").is_none());
    }

    #[test]
    fn graph_limits_reject_whole_observation_instead_of_partial_absence() {
        let retained = r#"{"type":"PipeWire:Interface:Client/3"}"#;
        let irrelevant = r#"{"type":"PipeWire:Interface:Port/3"}"#;
        assert!(Graph::parse(repeated_graph(retained, MAX_RETAINED_OBJECTS).as_bytes()).is_ok());
        let error = Graph::parse(repeated_graph(retained, MAX_RETAINED_OBJECTS + 1).as_bytes())
            .err()
            .unwrap();
        assert!(format!("{error:#}").contains("relevant object count"));
        let error = Graph::parse(repeated_graph(irrelevant, MAX_GRAPH_OBJECTS + 1).as_bytes())
            .err()
            .unwrap();
        assert!(format!("{error:#}").contains("graph object count"));
        let object = format!(
            r#"{{"type":"PipeWire:Interface:Client/3","info":{{"props":{{"node.name":"{}"}}}}}}"#,
            "x".repeat(MAX_FIELD_BYTES)
        );
        let error = Graph::parse(repeated_graph(&object, 129).as_bytes())
            .err()
            .unwrap();
        assert!(format!("{error:#}").contains("retained graph text"));
        let too_long = object.replace(
            &"x".repeat(MAX_FIELD_BYTES),
            &"x".repeat(MAX_FIELD_BYTES + 1),
        );
        assert!(Graph::parse(format!("[{too_long}]").as_bytes()).is_err());
        let raw = format!(
            r#"[{{"unused":"{}"}}]"#,
            "x".repeat(MAX_JSON_STRING_BYTES + 1)
        );
        assert!(
            format!("{:#}", Graph::parse(raw.as_bytes()).err().unwrap()).contains("JSON string")
        );
        let raw = format!(r#"[{{"unused":{}0{}}}]"#, "[".repeat(128), "]".repeat(128));
        assert!(format!("{:#}", Graph::parse(raw.as_bytes()).err().unwrap()).contains("nesting"));
        let error = Graph::from_reader(io::repeat(b' ').take((MAX_DUMP_BYTES + 1) as u64))
            .err()
            .unwrap();
        assert!(format!("{error:#}").contains("16777216"));
        assert!(Graph::parse(b"[] trailing junk").is_err());
        assert!(Graph::parse(b"[{\"type\":\"PipeWire:Interface:Core/3\"}").is_err());
    }

    #[test]
    fn explicit_null_claim_does_not_inherit_client_claim() {
        let graph = Graph::parse(br#"[
            {"id":30,"type":"PipeWire:Interface:Client/3","info":{"props":{"application.process.id":321,"pipewire.sec.pid":321}}},
            {"id":31,"type":"PipeWire:Interface:Node/3","info":{"state":"idle","props":{"media.class":"Stream/Input/Audio","client.id":30,"application.process.id":null}}}
        ]"#).unwrap();
        let rejection = graph.process_id(graph.nodes().next().unwrap()).unwrap_err();
        assert!(rejection.contains("no valid claimed"));
    }

    #[test]
    fn read_failure_cannot_return_a_successful_partial_graph() {
        struct FailingReader {
            offset: usize,
        }
        impl Read for FailingReader {
            fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
                if self.offset < 2 {
                    output[0] = b"[]"[self.offset];
                    self.offset += 1;
                    Ok(1)
                } else {
                    Err(io::Error::other("injected graph read failure"))
                }
            }
        }
        let error = Graph::from_reader(FailingReader { offset: 0 })
            .err()
            .unwrap();
        assert!(format!("{error:#}").contains("injected graph read failure"));
    }

    #[test]
    fn subprocess_waits_for_clean_exit_and_preserves_bounded_diagnostics() {
        assert!(shell("printf '[]'", Duration::from_secs(3)).is_ok());
        let error = shell(
            "printf '[]'; printf 'server unavailable' >&2; exit 7",
            Duration::from_secs(3),
        )
        .err()
        .unwrap();
        assert!(format!("{error:#}").contains("server unavailable"));
        assert!(format!("{error:#}").contains("pw-dump failed"));
        let error = shell(
            "printf '[]'; head -c 20000 /dev/zero >&2; exit 7",
            Duration::from_secs(3),
        )
        .err()
        .unwrap();
        let detail = format!("{error:#}");
        assert!(detail.contains("diagnostic truncated"));
        assert!(detail.len() < STDERR_RETAIN_BYTES + 512);
    }

    #[test]
    fn subprocess_flood_blocking_and_inherited_pipes_are_bounded_failures() {
        for (script, expected) in [
            (
                "printf '[]'; head -c 1048577 /dev/zero >&2",
                "stderr exceeds",
            ),
            (
                "printf '[]'; head -c 16777217 /dev/zero | tr '\\000' ' '",
                "graph exceeds",
            ),
            ("printf '['; sleep 30", "timed out"),
            ("sleep 30 & printf '[]'", "timed out"),
        ] {
            let timeout = if expected == "timed out" {
                Duration::from_millis(150)
            } else {
                Duration::from_secs(10)
            };
            let start = Instant::now();
            let error = shell(script, timeout).err().unwrap();
            assert!(format!("{error:#}").contains(expected), "{error:#}");
            assert!(start.elapsed() < timeout + Duration::from_secs(2));
        }
    }

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
