use chrono::{DateTime, Utc};
use serde::Serialize;
use std::fmt;

pub const SCHEMA_VERSION: u8 = 1;

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
    Normal,
    Unexplained,
    Suspicious,
    Blocked,
}

impl fmt::Display for Risk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Normal => "NORMAL",
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

#[derive(Clone, Debug, Serialize)]
pub struct Access {
    #[serde(skip)]
    pub key: String,
    pub resource: Resource,
    pub activity: Activity,
    pub risk: Risk,
    pub confidence: Confidence,
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
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Start,
    Update,
    Stop,
}

#[derive(Debug, Serialize)]
pub struct AccessEvent {
    pub schema_version: u8,
    pub action: Action,
    pub observed_at: DateTime<Utc>,
    #[serde(flatten)]
    pub access: Access,
}

#[derive(Debug, Serialize)]
pub struct StatusDocument<'a> {
    pub schema_version: u8,
    pub accesses: &'a [Access],
}

#[derive(Debug, Serialize)]
pub struct Device {
    pub resource: Resource,
    pub id: String,
    pub name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_schema_is_explicit_and_dimensions_are_separate() {
        let access = Access {
            key: "camera:1".into(),
            resource: Resource::Camera,
            activity: Activity::Ready,
            risk: Risk::Unexplained,
            confidence: Confidence::Low,
            application: "browser.exe".into(),
            pid: Some(1),
            parent_pid: None,
            parent_name: None,
            executable: None,
            signature: None,
            device: None,
            started_at: None,
            modules: vec!["mfcaptureengine.dll".into()],
            evidence: vec![],
        };
        let value = serde_json::to_value(StatusDocument {
            schema_version: SCHEMA_VERSION,
            accesses: &[access],
        })
        .unwrap();
        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["accesses"][0]["activity"], "ready");
        assert_eq!(value["accesses"][0]["risk"], "unexplained");
        assert_eq!(value["accesses"][0]["confidence"], "low");
    }
}
