use crate::model::{Device, DiagnosticCheck, Snapshot};
use anyhow::Result;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CaptureScope {
    pub microphone: bool,
    pub camera: bool,
    pub include_ready: bool,
}

impl CaptureScope {
    pub const fn all() -> Self {
        Self {
            microphone: true,
            camera: true,
            include_ready: false,
        }
    }
}

impl Default for CaptureScope {
    fn default() -> Self {
        Self::all()
    }
}

pub trait CaptureCollector {
    fn snapshot(&self, scope: CaptureScope) -> Result<Snapshot>;
    fn devices(&self) -> Result<Vec<Device>>;
    fn diagnostics(&self) -> Vec<DiagnosticCheck>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlatformFamily {
    Windows,
    Linux,
    MacOs,
    Android,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CollectorContract {
    pub platform: PlatformFamily,
    pub requires_microphone_sessions: bool,
    pub requires_camera_activity: bool,
    pub requires_process_attribution: bool,
    pub requires_privacy_controls: bool,
}

pub const WINDOWS_CONTRACT: CollectorContract = CollectorContract {
    platform: PlatformFamily::Windows,
    requires_microphone_sessions: true,
    requires_camera_activity: true,
    requires_process_attribution: true,
    requires_privacy_controls: true,
};

pub const LINUX_CONTRACT: CollectorContract = CollectorContract {
    platform: PlatformFamily::Linux,
    requires_microphone_sessions: true,
    requires_camera_activity: true,
    requires_process_attribution: true,
    requires_privacy_controls: false,
};

pub const MACOS_CONTRACT: CollectorContract = CollectorContract {
    platform: PlatformFamily::MacOs,
    requires_microphone_sessions: true,
    requires_camera_activity: true,
    requires_process_attribution: false,
    requires_privacy_controls: false,
};

pub const ANDROID_CONTRACT: CollectorContract = CollectorContract {
    platform: PlatformFamily::Android,
    requires_microphone_sessions: false,
    requires_camera_activity: false,
    requires_process_attribution: false,
    requires_privacy_controls: false,
};

pub const PLATFORM_CONTRACTS: [CollectorContract; 4] = [
    WINDOWS_CONTRACT,
    LINUX_CONTRACT,
    MACOS_CONTRACT,
    ANDROID_CONTRACT,
];
