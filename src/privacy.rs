use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet, fs, mem::size_of, os::windows::process::CommandExt, process::Command,
};
use windows::{
    Win32::{
        Foundation::{CloseHandle, ERROR_CANCELLED},
        System::Threading::{GetExitCodeProcess, INFINITE, WaitForSingleObject},
        UI::Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW},
    },
    core::PCWSTR,
};
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[link(name = "cfgmgr32")]
unsafe extern "system" {
    fn CM_Locate_DevNodeW(pdndevinst: *mut u32, pdeviceid: *const u16, ulflags: u32) -> u32;
    fn CM_Get_DevNode_Status(
        pulstatus: *mut u32,
        pulproblemnumber: *mut u32,
        dndevinst: u32,
        ulflags: u32,
    ) -> u32;
}

fn device_status(instance_id: &str) -> Option<String> {
    let wide: Vec<u16> = instance_id.encode_utf16().chain(Some(0)).collect();
    let mut devinst = 0u32;
    let res = unsafe { CM_Locate_DevNodeW(&mut devinst, wide.as_ptr(), 0) };
    if res != 0 {
        return None;
    }
    let mut status = 0u32;
    let mut problem = 0u32;
    let res = unsafe { CM_Get_DevNode_Status(&mut status, &mut problem, devinst, 0) };
    if res != 0 {
        return Some("Error".to_owned());
    }
    if problem == 0 {
        Some("OK".to_owned())
    } else {
        Some("Error".to_owned())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CameraPrivacyState {
    Allowed,
    Blocked,
    SystemManaged,
}

impl CameraPrivacyState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Allowed => "allowed",
            Self::Blocked => "blocked",
            Self::SystemManaged => "system_managed",
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct CameraDevice {
    instance_id: String,
    status: String,
}

fn devices() -> Result<Vec<CameraDevice>> {
    let mut found = Vec::new();
    let mut seen = HashSet::new();

    for class in &["Camera", "Image"] {
        if let Ok(output) = Command::new("pnputil.exe")
            .args(["/enum-devices", "/class", class])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            && output.status.success()
        {
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines() {
                let lower = line.to_ascii_lowercase();
                if (lower.contains("instance") || lower.contains("d'instance"))
                    && let Some((_, val)) = line.split_once(':')
                {
                    let id = val.trim();
                    if !id.is_empty()
                        && seen.insert(id.to_ascii_lowercase())
                        && let Some(status) = device_status(id)
                    {
                        found.push(CameraDevice {
                            instance_id: id.to_owned(),
                            status,
                        });
                    }
                }
            }
        }
    }

    if !found.is_empty() {
        return Ok(found);
    }

    devices_powershell()
}

fn devices_powershell() -> Result<Vec<CameraDevice>> {
    let output = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-Command",
            "Get-PnpDevice -Class Camera -PresentOnly | Select-Object InstanceId,Status | ConvertTo-Json -Compress",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .context("failed to query Windows camera devices")?;
    if !output.status.success() {
        bail!(
            "failed to query Windows camera devices: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let text =
        String::from_utf8(output.stdout).context("invalid camera device inventory encoding")?;
    let text = text.trim_start_matches('\u{feff}').trim();
    if text.is_empty() {
        return Ok(Vec::new());
    }
    if text.starts_with('[') {
        serde_json::from_str(text).context("invalid Windows camera device inventory")
    } else {
        Ok(vec![
            serde_json::from_str(text).context("invalid Windows camera device inventory")?,
        ])
    }
}

fn blocked_devices_path() -> Result<std::path::PathBuf> {
    Ok(crate::settings::data_dir()?.join("blocked-camera-devices.json"))
}

/// Camera devices this tool disabled on the user's behalf.
///
/// The two lists differ in intent, which decides what happens when a camera is
/// plugged back in: `blocked` keeps it off, while `restore_on_arrival` completes
/// an `allow` that could not be applied because the camera was unplugged.
#[derive(Debug, Default, Deserialize, PartialEq, Serialize)]
struct CameraBlockRecord {
    #[serde(default)]
    blocked: Vec<String>,
    #[serde(default)]
    restore_on_arrival: Vec<String>,
}

impl CameraBlockRecord {
    fn is_empty(&self) -> bool {
        self.blocked.is_empty() && self.restore_on_arrival.is_empty()
    }

    fn owns(&self, instance_id: &str) -> bool {
        self.blocked
            .iter()
            .chain(self.restore_on_arrival.iter())
            .any(|id| id.eq_ignore_ascii_case(instance_id))
    }

    /// Moves every recorded device into `blocked`, re-asserting the block over
    /// anything that was still waiting to be restored.
    fn supersede_pending_restore(&mut self) {
        self.blocked.append(&mut self.restore_on_arrival);
        dedupe_ascii_case(&mut self.blocked);
    }
}

fn dedupe_ascii_case(ids: &mut Vec<String>) {
    let mut seen = HashSet::new();
    ids.retain(|id| seen.insert(id.to_ascii_lowercase()));
}

fn load_record() -> Result<Option<CameraBlockRecord>> {
    let path = blocked_devices_path()?;
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", path.display()));
        }
    };
    let invalid = || format!("invalid camera block record {}", path.display());
    // Releases up to v0.13.3 stored a bare array; those devices were blocked with
    // no pending intent, so they migrate into `blocked` and keep their state.
    match serde_json::from_slice::<CameraBlockRecord>(&bytes) {
        Ok(record) => Ok(Some(record)),
        Err(_) => Ok(Some(CameraBlockRecord {
            blocked: serde_json::from_slice(&bytes).with_context(invalid)?,
            restore_on_arrival: Vec::new(),
        })),
    }
}

fn save_record(record: &CameraBlockRecord) -> Result<()> {
    let path = blocked_devices_path()?;
    if record.is_empty() {
        return match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error).context("failed to clear camera block record"),
        };
    }
    fs::create_dir_all(path.parent().context("camera block path has no parent")?)?;
    fs::write(&path, serde_json::to_vec(record)?)
        .with_context(|| format!("failed to save camera device state at {}", path.display()))
}

/// Whether a recorded device is connected but not running.
fn needs_restore(instance_id: &str) -> bool {
    device_status(instance_id).is_some_and(|status| status != "OK")
}

/// Recorded restorations whose camera is plugged back in but still disabled.
///
/// A device that is absent is not an arrival, and one that already runs needs no
/// elevation, so neither may raise a prompt.
fn pending_targets(
    record: &CameraBlockRecord,
    status: impl Fn(&str) -> Option<String>,
) -> Vec<String> {
    record
        .restore_on_arrival
        .iter()
        .filter(|id| status(id).is_some_and(|report| report != "OK"))
        .cloned()
        .collect()
}

/// Blocked cameras the user asked to restore that have since been reconnected.
pub fn arrived_restore_targets() -> Result<Vec<String>> {
    let Some(record) = load_record()? else {
        return Ok(Vec::new());
    };
    Ok(pending_targets(&record, device_status))
}

/// Completes the restore for reconnected cameras recorded by a previous `allow`.
///
/// Returns how many devices were re-enabled. The block record is the source of
/// truth: a device that refuses to come back is kept for a later attempt rather
/// than silently dropped.
pub fn restore_arrived() -> Result<usize> {
    let targets = arrived_restore_targets()?;
    if targets.is_empty() {
        return Ok(0);
    }
    pnputil_elevated("enable", &targets)?;
    let Some(mut record) = load_record()? else {
        return Ok(targets.len());
    };
    record
        .restore_on_arrival
        .retain(|id| needs_restore(id.as_str()));
    let failed = record.restore_on_arrival.clone();
    save_record(&record)?;
    if !failed.is_empty() {
        bail!("Windows did not restore every reconnected camera device");
    }
    Ok(targets.len())
}

pub fn camera_state() -> Result<CameraPrivacyState> {
    let Some(record) = load_record()? else {
        return Ok(CameraPrivacyState::Allowed);
    };
    let mut current = Vec::new();
    for id in record
        .blocked
        .iter()
        .chain(record.restore_on_arrival.iter())
    {
        if let Some(status) = device_status(id) {
            current.push(CameraDevice {
                instance_id: id.clone(),
                status,
            });
        }
    }
    if let Ok(all_devices) = devices() {
        for dev in all_devices {
            if !current
                .iter()
                .any(|c| c.instance_id.eq_ignore_ascii_case(&dev.instance_id))
            {
                current.push(dev);
            }
        }
    }
    let state = classify_devices(&record.blocked, &current);
    Ok(apply_pending_restore(state, &record))
}

fn classify_devices(blocked: &[String], current: &[CameraDevice]) -> CameraPrivacyState {
    let blocked_present = blocked.iter().any(|id| {
        current
            .iter()
            .any(|device| device.instance_id.eq_ignore_ascii_case(id) && device.status != "OK")
    });
    let allowed_present = current.iter().any(|device| device.status == "OK");
    if allowed_present && !blocked_present {
        CameraPrivacyState::Allowed
    } else if !allowed_present && blocked_present {
        CameraPrivacyState::Blocked
    } else {
        CameraPrivacyState::SystemManaged
    }
}

/// Splits recorded cameras into the ones to re-enable now, plus the record that
/// must survive the operation.
///
/// Anything still unplugged carries its request forward, so reconnecting the
/// camera later completes the `allow` without another command.
fn plan_allow(
    record: &CameraBlockRecord,
    status: impl Fn(&str) -> Option<String>,
) -> (Vec<String>, CameraBlockRecord) {
    let mut candidates = record.blocked.clone();
    candidates.extend(record.restore_on_arrival.iter().cloned());
    dedupe_ascii_case(&mut candidates);

    let mut to_enable = Vec::new();
    let mut awaiting = Vec::new();
    for id in candidates {
        match status(&id) {
            None => awaiting.push(id),
            Some(report) if report != "OK" => to_enable.push(id),
            Some(_) => {}
        }
    }
    (
        to_enable,
        CameraBlockRecord {
            blocked: Vec::new(),
            restore_on_arrival: awaiting,
        },
    )
}

/// A camera that is still waiting to come back keeps the switch partially
/// engaged, so reporting a plain `allowed` would be a lie.
fn apply_pending_restore(
    state: CameraPrivacyState,
    record: &CameraBlockRecord,
) -> CameraPrivacyState {
    if state == CameraPrivacyState::Allowed && !record.restore_on_arrival.is_empty() {
        CameraPrivacyState::SystemManaged
    } else {
        state
    }
}

fn pnputil_elevated(action: &str, instance_ids: &[String]) -> Result<()> {
    if instance_ids.is_empty() {
        return Ok(());
    }
    let (file, parameters) = if instance_ids.len() == 1 {
        (
            "pnputil.exe".to_owned(),
            format!("/{action}-device \"{}\"", instance_ids[0]),
        )
    } else {
        let commands = instance_ids
            .iter()
            .map(|id| format!("pnputil.exe /{action}-device \"{id}\""))
            .collect::<Vec<_>>()
            .join(" & ");
        ("cmd.exe".to_owned(), format!("/d /c {commands}"))
    };

    let wide_verb: Vec<u16> = "runas\0".encode_utf16().collect();
    let wide_file: Vec<u16> = file.encode_utf16().chain(Some(0)).collect();
    let wide_params: Vec<u16> = parameters.encode_utf16().chain(Some(0)).collect();

    let mut info = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        lpVerb: PCWSTR(wide_verb.as_ptr()),
        lpFile: PCWSTR(wide_file.as_ptr()),
        lpParameters: PCWSTR(wide_params.as_ptr()),
        nShow: 0,
        ..Default::default()
    };

    let result = unsafe { ShellExecuteExW(&mut info) };
    if let Err(error) = result {
        if error.code().0 == 0x8007_04C7_u32 as i32
            || unsafe { windows::Win32::Foundation::GetLastError() } == ERROR_CANCELLED
        {
            bail!("camera {action} administrator approval was declined");
        }
        bail!("failed to request administrator approval for camera control: {error}");
    }

    if !info.hProcess.is_invalid() {
        unsafe {
            WaitForSingleObject(info.hProcess, INFINITE);
            let mut exit_code = 0u32;
            let _ = GetExitCodeProcess(info.hProcess, &mut exit_code);
            let _ = CloseHandle(info.hProcess);
            if exit_code != 0 {
                bail!("camera {action} failed with exit code {exit_code}");
            }
        }
    }
    Ok(())
}

pub fn set_camera_state(state: CameraPrivacyState) -> Result<()> {
    match state {
        CameraPrivacyState::Blocked => {
            let mut record = load_record()?.unwrap_or_default();
            // An explicit block re-asserts ownership over anything that was still
            // waiting to be restored.
            record.supersede_pending_restore();
            let enabled = devices()?
                .into_iter()
                .filter(|device| device.status == "OK")
                .map(|device| device.instance_id)
                .collect::<Vec<_>>();
            if enabled.is_empty() {
                if !record.blocked.is_empty() && camera_state()? == CameraPrivacyState::Blocked {
                    return Ok(());
                }
                bail!("no enabled physical camera devices were found");
            }
            for id in &enabled {
                if !record.owns(id) {
                    record.blocked.push(id.clone());
                }
            }
            save_record(&record)?;
            pnputil_elevated("disable", &enabled)?;
            if devices()?.iter().any(|device| device.status == "OK") {
                bail!("Windows did not disable every connected camera device");
            }
        }
        CameraPrivacyState::Allowed => {
            let Some(record) = load_record()? else {
                return Ok(());
            };
            if record.is_empty() {
                return Ok(());
            }
            let (to_enable, updated) = plan_allow(&record, device_status);
            if !to_enable.is_empty() {
                pnputil_elevated("enable", &to_enable)?;
                if let Some(stuck) = to_enable.iter().find(|id| needs_restore(id)) {
                    bail!("Windows did not restore {stuck}; it is still disabled");
                }
            }
            save_record(&updated)?;
        }
        CameraPrivacyState::SystemManaged => {
            bail!("system-managed camera state cannot be selected")
        }
    }
    Ok(())
}

pub fn toggle_camera() -> Result<CameraPrivacyState> {
    let next = match camera_state()? {
        CameraPrivacyState::Allowed => CameraPrivacyState::Blocked,
        CameraPrivacyState::Blocked | CameraPrivacyState::SystemManaged => {
            CameraPrivacyState::Allowed
        }
    };
    set_camera_state(next)?;
    camera_state()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_block_reports_partial_and_new_devices() {
        let blocked = vec!["USB\\CAMERA_A".to_owned()];
        let disabled = CameraDevice {
            instance_id: blocked[0].clone(),
            status: "Error".to_owned(),
        };
        assert_eq!(
            classify_devices(&blocked, std::slice::from_ref(&disabled)),
            CameraPrivacyState::Blocked
        );
        assert_eq!(
            classify_devices(&blocked, &[]),
            CameraPrivacyState::SystemManaged
        );
        let enabled = CameraDevice {
            instance_id: "USB\\CAMERA_B".to_owned(),
            status: "OK".to_owned(),
        };
        assert_eq!(
            classify_devices(&blocked, &[enabled]),
            CameraPrivacyState::Allowed
        );
        assert_eq!(
            classify_devices(
                &blocked,
                &[
                    disabled,
                    CameraDevice {
                        instance_id: "USB\\CAMERA_B".to_owned(),
                        status: "OK".to_owned(),
                    }
                ]
            ),
            CameraPrivacyState::SystemManaged
        );
    }

    #[test]
    fn allow_defers_cameras_that_are_still_unplugged() {
        // The reported workflow: two cameras blocked, the external one unplugged,
        // then `allow` restores the built-in and defers the other.
        let record = CameraBlockRecord {
            blocked: vec![
                "USB\\CAMERA_BUILTIN".to_owned(),
                "USB\\CAMERA_LOGI".to_owned(),
            ],
            restore_on_arrival: Vec::new(),
        };
        let disabled_builtin_only = |id: &str| {
            if id.ends_with("BUILTIN") {
                Some("Error".to_owned())
            } else {
                None
            }
        };
        let (to_enable, deferred) = plan_allow(&record, disabled_builtin_only);
        assert_eq!(to_enable, vec!["USB\\CAMERA_BUILTIN"]);
        assert_eq!(
            deferred,
            CameraBlockRecord {
                blocked: Vec::new(),
                restore_on_arrival: vec!["USB\\CAMERA_LOGI".to_owned()],
            }
        );
        assert!(!deferred.is_empty(), "the deferred request must survive");

        // Plugging the camera back in turns the deferred entry into a target, so
        // the tray can finish the job without another command.
        let reconnected =
            |id: &str| Some(if id.ends_with("LOGI") { "Error" } else { "OK" }.to_owned());
        let (to_enable, settled) = plan_allow(&deferred, reconnected);
        assert_eq!(to_enable, vec!["USB\\CAMERA_LOGI"]);
        assert_eq!(settled, CameraBlockRecord::default());
        assert!(settled.is_empty(), "an empty record clears the file");
    }

    #[test]
    fn only_a_reconnected_disabled_camera_raises_a_prompt() {
        let record = CameraBlockRecord {
            blocked: Vec::new(),
            restore_on_arrival: vec![
                "USB\\CAMERA_ABSENT".to_owned(),
                "USB\\CAMERA_DISABLED".to_owned(),
                "USB\\CAMERA_RUNNING".to_owned(),
            ],
        };
        let targets = pending_targets(&record, |id| {
            if id.ends_with("ABSENT") {
                None
            } else if id.ends_with("DISABLED") {
                Some("Error".to_owned())
            } else {
                Some("OK".to_owned())
            }
        });
        // An unplugged camera must not prompt, and a running one needs no elevation.
        assert_eq!(targets, vec!["USB\\CAMERA_DISABLED"]);
    }

    #[test]
    fn plan_allow_leaves_healthy_cameras_alone() {
        let record = CameraBlockRecord {
            blocked: vec!["USB\\CAMERA_A".to_owned()],
            restore_on_arrival: Vec::new(),
        };
        let (to_enable, deferred) = plan_allow(&record, |_| Some("OK".to_owned()));
        assert!(to_enable.is_empty(), "a running camera needs no elevation");
        assert!(deferred.is_empty());
    }

    #[test]
    fn explicit_block_takes_back_devices_awaiting_restoration() {
        let mut record = CameraBlockRecord {
            blocked: vec!["USB\\CAMERA_BUILTIN".to_owned()],
            restore_on_arrival: vec![
                "USB\\CAMERA_LOGI".to_owned(),
                "usb\\camera_builtin".to_owned(),
            ],
        };
        record.supersede_pending_restore();
        assert!(record.restore_on_arrival.is_empty());
        assert_eq!(
            record.blocked,
            vec![
                "USB\\CAMERA_BUILTIN".to_owned(),
                "USB\\CAMERA_LOGI".to_owned()
            ]
        );
    }

    #[test]
    fn legacy_array_record_migrates_to_blocked_without_pending_restore() {
        let bytes = br#"["USB\\CAMERA_A","USB\\CAMERA_B"]"#;
        assert!(serde_json::from_slice::<CameraBlockRecord>(bytes).is_err());
        let migrated = CameraBlockRecord {
            blocked: serde_json::from_slice(bytes).expect("legacy array decodes"),
            restore_on_arrival: Vec::new(),
        };
        assert_eq!(migrated.blocked.len(), 2);
        assert!(migrated.restore_on_arrival.is_empty());
        assert!(migrated.owns("USB\\CAMERA_A"));
    }

    #[test]
    fn pending_restore_is_not_reported_as_allowed() {
        let pending = CameraBlockRecord {
            blocked: Vec::new(),
            restore_on_arrival: vec!["USB\\CAMERA_LOGI".to_owned()],
        };
        // The built-in still runs, yet the switch is not fully released.
        assert_eq!(
            apply_pending_restore(CameraPrivacyState::Allowed, &pending),
            CameraPrivacyState::SystemManaged
        );
        // Real block and partial states keep their meaning.
        assert_eq!(
            apply_pending_restore(CameraPrivacyState::Blocked, &pending),
            CameraPrivacyState::Blocked
        );
        let settled = CameraBlockRecord::default();
        assert_eq!(
            apply_pending_restore(CameraPrivacyState::Allowed, &settled),
            CameraPrivacyState::Allowed
        );
    }
}
