use super::procfs::{BootTime, ProcessIdentity};
use crate::model::{
    Access, Activity, Confidence, Device, EnforcementDecision, Evidence, EvidenceKind,
    ProcessContext, Resource, Risk,
};
use anyhow::{Context, Result};
use std::{collections::HashSet, fs, io, path::Path};

#[derive(Default)]
pub(super) struct Scan {
    pub(super) accesses: Vec<Access>,
    pub(super) warning: Option<String>,
}

pub(super) fn devices() -> Result<Vec<Device>> {
    let entries = match fs::read_dir("/dev") {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error).context("cannot enumerate /dev video devices"),
    };
    let mut devices = Vec::new();
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !is_video_name(&name) {
            continue;
        }
        let id = format!("/dev/{name}");
        let friendly = fs::read_to_string(
            Path::new("/sys/class/video4linux")
                .join(name.as_ref())
                .join("name"),
        )
        .ok()
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| name.into_owned());
        devices.push(Device {
            resource: Resource::Camera,
            id,
            name: friendly,
        });
    }
    devices.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(devices)
}

fn is_video_name(name: &str) -> bool {
    name.strip_prefix("video").is_some_and(|suffix| {
        !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
    })
}

pub(super) fn scan(devices: &[Device], boot: Option<&BootTime>) -> Scan {
    let mut result = Scan {
        accesses: Vec::new(),
        warning: None,
    };
    if devices.is_empty() {
        return result;
    }
    let paths: HashSet<_> = devices.iter().map(|device| device.id.as_str()).collect();
    let processes = match fs::read_dir("/proc") {
        Ok(processes) => processes,
        Err(error) => {
            result.warning = Some(format!(
                "cannot enumerate /proc for V4L2 camera descriptors: {error}"
            ));
            return result;
        }
    };
    let mut seen = HashSet::new();
    for process in processes.flatten() {
        let Some(pid) = process
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
        else {
            continue;
        };
        let descriptors = match fs::read_dir(process.path().join("fd")) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => {
                if error.kind() != io::ErrorKind::PermissionDenied {
                    result.warning = Some(format!("cannot inspect /proc/{pid}/fd: {error}"));
                } else {
                    result.warning = Some("some /proc/<pid>/fd directories are inaccessible; direct V4L2 usage may be missed".to_owned());
                }
                continue;
            }
        };
        for fd in descriptors.flatten() {
            let Ok(target) = fs::read_link(fd.path()) else {
                continue; // A process can close an fd during the scan.
            };
            let Some(path) = target.to_str() else {
                continue;
            };
            if !paths.contains(path) || !seen.insert((pid, path.to_owned())) {
                continue;
            }
            result.warning.get_or_insert_with(|| "V4L2 camera descriptor observed; open fd does not prove streaming, direct capture cannot be confirmed".to_owned());
            let identity = boot.and_then(|boot| ProcessIdentity::verify(pid, boot).ok());
            let device = devices
                .iter()
                .find(|device| device.id == path)
                .expect("path came from device inventory");
            result.accesses.push(ready_access(device, identity));
        }
    }
    result
}

fn ready_access(device: &Device, identity: Option<ProcessIdentity>) -> Access {
    let instance = identity
        .as_ref()
        .map(|id| id.instance_id.as_str())
        .unwrap_or("unverified");
    Access {
        key: format!("linux:v4l2:{}:{instance}", device.id),
        resource: Resource::Camera,
        activity: Activity::Ready,
        risk: Risk::Unexplained,
        confidence: Confidence::Low,
        enforcement: if identity.is_some() {
            EnforcementDecision::Alert
        } else {
            EnforcementDecision::Unknown
        },
        application: identity
            .as_ref()
            .map(|id| id.name.clone())
            .unwrap_or_else(|| "Unknown V4L2 client".to_owned()),
        pid: identity.as_ref().map(|id| id.pid),
        parent_pid: identity.as_ref().and_then(|id| id.parent_pid),
        parent_name: None,
        executable: identity.as_ref().map(|id| id.executable.clone()),
        signature: None,
        device: Some(device.name.clone()),
        started_at: None,
        modules: Vec::new(),
        evidence: vec![Evidence::new(
            EvidenceKind::LiveApi,
            "v4l2_fd",
            format!(
                "{} is open; this alone does not prove video frames are flowing",
                device.id
            ),
        )],
        process: identity.map(|id| ProcessContext {
            instance_id: id.instance_id,
            user: Some(id.uid.to_string()),
            ..ProcessContext::default()
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_v4l2_handle_never_becomes_confirmed_activity() {
        let device = Device {
            resource: Resource::Camera,
            id: "/dev/video7".into(),
            name: "USB camera".into(),
        };
        let observation = ready_access(&device, None);
        assert_eq!(observation.activity, Activity::Ready);
        assert_eq!(observation.confidence, Confidence::Low);
        assert_eq!(observation.enforcement, EnforcementDecision::Unknown);
        assert!(observation.pid.is_none());
        assert!(is_video_name("video7"));
        assert!(!is_video_name("video7-control"));
    }
}
