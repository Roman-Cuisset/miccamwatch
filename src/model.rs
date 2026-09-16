use chrono::{DateTime, Utc};
use serde::Serialize;
use std::fmt;

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

/// How certain we are that the access is real.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    /// API-confirmed active access (e.g. WASAPI active session).
    Confirmed,
    /// Windows privacy activity data says access is active.
    Inferred,
}

/// Threat level assigned to a forensic camera detection.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ThreatLevel {
    /// Camera capture stack is loaded; expected for the application type.
    Ready,
    /// Camera capture stack is loaded with no correlated Windows privacy
    /// event and at least one anomalous signal.
    Suspect,
    /// Camera access appears to occur despite an explicit Windows permission
    /// denial.
    Unauthorized,
}

impl fmt::Display for ThreatLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Ready => "READY",
            Self::Suspect => "SUSPECT",
            Self::Unauthorized => "UNAUTHORIZED",
        })
    }
}

/// How the access was detected.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case", tag = "method")]
pub enum Detection {
    /// Live system API (microphone WASAPI, etc.).
    Api { confidence: Confidence },
    /// Windows Capability Access Manager privacy activity data.
    PrivacyActivity { confidence: Confidence },
    /// Loaded camera-capture modules in process memory.
    Forensic {
        threat: ThreatLevel,
        modules: Vec<String>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        reasons: Vec<String>,
    },
}

#[derive(Clone, Debug, Serialize)]
pub struct SignatureInfo {
    pub verified: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Access {
    #[serde(skip)]
    pub key: String,
    pub resource: Resource,
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
    pub detection: Detection,
}
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Start,
    Stop,
}

#[derive(Debug, Serialize)]
pub struct AccessEvent {
    pub action: Action,
    pub observed_at: DateTime<Utc>,
    #[serde(flatten)]
    pub access: Access,
}

#[derive(Debug, Serialize)]
pub struct Device {
    pub resource: Resource,
    pub id: String,
    pub name: String,
}
