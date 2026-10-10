use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

pub const SCHEMA_VERSION: u8 = 3;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Resource {
    Microphone,
    Camera,
}

impl fmt::Display for Resource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Microphone => "MIC",
            Self::Camera => "CAM",
        })
    }
}

/// Observable operating state, independent from the security assessment.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Activity {
    Active,
    Ready,
}

impl fmt::Display for Activity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Active => "ACTIVE",
            Self::Ready => "READY",
        })
    }
}

/// Security interpretation of the collected evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Risk {
    Expected,
    Unexplained,
    Suspicious,
    Blocked,
}

impl fmt::Display for Risk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Expected => "EXPECTED",
            Self::Unexplained => "UNEXPLAINED",
            Self::Suspicious => "SUSPICIOUS",
            Self::Blocked => "BLOCKED",
        })
    }
}

/// Strength of the activity claim, not a probability or threat score.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    High,
    Medium,
    Low,
}

impl fmt::Display for Confidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
        })
    }
}

/// Policy decision used for enforcement. This is deliberately independent from
/// the heuristic risk assessment.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EnforcementDecision {
    /// An explicit policy rule matched.
    Allow,
    /// No explicit policy decision is available; report only.
    #[default]
    Alert,
    /// An explicit policy rule was violated.
    Deny,
    /// Required identity evidence was unavailable.
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MicrophoneMuteState {
    Unavailable,
    Muted,
    Unmuted,
    Mixed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    LiveApi,
    PrivacyActivity,
    CaptureModule,
    Permission,
    Signature,
    ProcessLineage,
    CommandLine,
    FileLocation,
    ApplicationProfile,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Evidence {
    pub kind: EvidenceKind,
    pub source: &'static str,
    pub detail: String,
}

impl Evidence {
    pub fn new(kind: EvidenceKind, source: &'static str, detail: impl Into<String>) -> Self {
        Self {
            kind,
            source,
            detail: detail.into(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct SignatureInfo {
    pub verified: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CollectorState {
    Healthy,
    Degraded,
    Unavailable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CollectorHealth {
    pub collector: &'static str,
    pub state: CollectorState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProcessAncestor {
    pub pid: u32,
    pub name: String,
    #[cfg_attr(unix, serde(skip_serializing_if = "Option::is_none"))]
    pub created_at_filetime: Option<u64>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct ProcessContext {
    pub instance_id: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ancestry: Vec<ProcessAncestor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub integrity: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Access {
    #[serde(skip)]
    pub key: String,
    pub resource: Resource,
    pub activity: Activity,
    pub risk: Risk,
    pub confidence: Confidence,
    pub enforcement: EnforcementDecision,
    pub application: String,
    pub pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_name: Option<String>,
    pub executable: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<SignatureInfo>,
    pub device: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub modules: Vec<String>,
    pub evidence: Vec<Evidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process: Option<ProcessContext>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Start,
    Update,
    Stop,
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Start => "START",
            Self::Update => "UPDATE",
            Self::Stop => "STOP",
        })
    }
}

#[derive(Debug, Serialize)]
pub struct AccessEvent {
    pub schema_version: u8,
    pub event_code: u16,
    pub tool_version: &'static str,
    pub action: Action,
    pub observed_at: DateTime<Utc>,
    #[serde(flatten)]
    pub access: Access,
}

#[derive(Debug)]
pub struct Snapshot {
    pub collectors: Vec<CollectorHealth>,
    pub accesses: Vec<Access>,
    /// Resources whose current scan cannot establish that missing accesses stopped.
    pub observation_gaps: Vec<Resource>,
}

#[derive(Debug, Serialize)]
pub struct Device {
    pub resource: Resource,
    pub id: String,
    pub name: String,
}

/// Camera intent shared by the Windows CLI and tray. An unplugged device in
/// `restore_on_arrival` remains owed an allow operation after reconnection.
#[cfg(any(windows, test))]
#[derive(Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct CameraBlockRecord {
    pub(crate) blocked: Vec<String>,
    pub(crate) restore_on_arrival: Vec<String>,
    pub(crate) desired_blocked: bool,
    pub(crate) intent_generation: u64,
}

#[cfg(any(windows, test))]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyCameraDevices {
    devices: Vec<String>,
}

#[cfg(any(windows, test))]
#[derive(Deserialize)]
#[serde(untagged)]
enum StoredCameraBlockRecord {
    Current(CameraBlockRecord),
    LegacyList(Vec<String>),
    LegacyDevices(LegacyCameraDevices),
}

#[cfg(any(windows, test))]
impl CameraBlockRecord {
    pub(crate) fn from_json(bytes: &[u8]) -> serde_json::Result<Self> {
        let stored: StoredCameraBlockRecord = serde_json::from_slice(bytes)?;
        Ok(match stored {
            StoredCameraBlockRecord::Current(record) => record,
            StoredCameraBlockRecord::LegacyList(blocked) => Self {
                blocked,
                ..Self::default()
            },
            StoredCameraBlockRecord::LegacyDevices(legacy) => Self {
                blocked: legacy.devices,
                ..Self::default()
            },
        })
    }
}

pub fn event_code(action: Action) -> u16 {
    match action {
        Action::Start => 1001,
        Action::Update => 1002,
        Action::Stop => 1003,
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct DiagnosticCheck {
    pub name: &'static str,
    pub status: DiagnosticStatus,
    pub detail: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticStatus {
    Ok,
    Warning,
    Error,
}

impl fmt::Display for DiagnosticStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Ok => "OK",
            Self::Warning => "WARN",
            Self::Error => "ERR",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_record_reads_both_legacy_shapes_without_dropping_device_ids() {
        for encoded in [
            br#"["CAMERA_A","CAMERA_B"]"#.as_slice(),
            br#"{"devices":["CAMERA_A","CAMERA_B"]}"#,
            br#"{"blocked":["CAMERA_A","CAMERA_B"],"restore_on_arrival":[]}"#,
        ] {
            let record = CameraBlockRecord::from_json(encoded).unwrap();
            assert_eq!(record.blocked, ["CAMERA_A", "CAMERA_B"]);
            assert!(record.restore_on_arrival.is_empty());
            let canonical = serde_json::to_value(&record).unwrap();
            assert_eq!(
                canonical["blocked"],
                serde_json::json!(["CAMERA_A", "CAMERA_B"])
            );
            assert_eq!(canonical["restore_on_arrival"], serde_json::json!([]));
        }
        let owed = CameraBlockRecord::from_json(
            br#"{"blocked":["CAMERA_A"],"restore_on_arrival":["CAMERA_B"]}"#,
        )
        .unwrap();
        assert_eq!(owed.restore_on_arrival, ["CAMERA_B"]);
        assert!(
            CameraBlockRecord::from_json(br#"{"devices":["CAMERA_A"],"blocked":["CAMERA_B"]}"#)
                .is_err(),
            "ambiguous records must fail, not silently lose blocked intent"
        );
    }

    #[test]
    fn ancestor_json_preserves_windows_null_and_omits_missing_unix_filetime() {
        let ancestor = ProcessAncestor {
            pid: 7,
            name: "parent".into(),
            created_at_filetime: None,
        };
        let value = serde_json::to_value(&ancestor).unwrap();
        assert_eq!(value["pid"], 7);
        #[cfg(windows)]
        assert!(
            value
                .get("created_at_filetime")
                .is_some_and(|field| field.is_null())
        );
        #[cfg(unix)]
        assert!(value.get("created_at_filetime").is_none());
    }
}
