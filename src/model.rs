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

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Confirmed,
    Inferred,
    Forensic,
}

#[derive(Clone, Debug, Serialize)]
pub struct Access {
    #[serde(skip)]
    pub key: String,
    pub resource: Resource,
    pub application: String,
    pub pid: Option<u32>,
    pub executable: Option<String>,
    pub device: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub confidence: Confidence,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
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
