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

fn blocked_devices() -> Result<Option<Vec<String>>> {
    let path = blocked_devices_path()?;
    match fs::read(&path) {
        Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes).with_context(|| {
            format!("invalid camera block record {}", path.display())
        })?)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("failed to read {}", path.display())),
    }
}

pub fn camera_state() -> Result<CameraPrivacyState> {
    let Some(blocked) = blocked_devices()? else {
        return Ok(CameraPrivacyState::Allowed);
    };
    let mut current = Vec::new();
    for id in &blocked {
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
    Ok(classify_devices(&blocked, &current))
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

fn restoration_plan(blocked: &[String], current: &[CameraDevice]) -> (Vec<String>, Vec<String>) {
    let mut pending = Vec::new();
    let mut to_restore = Vec::new();
    for id in blocked {
        match current
            .iter()
            .find(|device| device.instance_id.eq_ignore_ascii_case(id))
        {
            Some(device) if device.status == "OK" => {}
            Some(_) => to_restore.push(id.clone()),
            None => pending.push(id.clone()),
        }
    }
    (to_restore, pending)
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
            let mut blocked = blocked_devices()?.unwrap_or_default();
            let enabled = devices()?
                .into_iter()
                .filter(|device| device.status == "OK")
                .map(|device| device.instance_id)
                .collect::<Vec<_>>();
            if enabled.is_empty() {
                if !blocked.is_empty() && camera_state()? == CameraPrivacyState::Blocked {
                    return Ok(());
                }
                bail!("no enabled physical camera devices were found");
            }
            for id in &enabled {
                if !blocked.iter().any(|saved| saved.eq_ignore_ascii_case(id)) {
                    blocked.push(id.clone());
                }
            }
            let path = blocked_devices_path()?;
            fs::create_dir_all(path.parent().context("camera block path has no parent")?)?;
            fs::write(&path, serde_json::to_vec(&blocked)?).with_context(|| {
                format!("failed to save camera device state at {}", path.display())
            })?;
            pnputil_elevated("disable", &enabled)?;
            if devices()?.iter().any(|device| device.status == "OK") {
                bail!("Windows did not disable every connected camera device");
            }
        }
        CameraPrivacyState::Allowed => {
            if let Some(blocked) = blocked_devices()? {
                let current = devices()?;
                let (to_restore, pending) = restoration_plan(&blocked, &current);
                if pending.len() == blocked.len() {
                    if current.iter().any(|device| device.status == "OK") {
                        return Ok(());
                    }
                    bail!("all blocked cameras are disconnected; reconnect a camera to restore it");
                }
                pnputil_elevated("enable", &to_restore)?;
                let current = devices()?;
                if blocked.iter().any(|id| {
                    current.iter().any(|device| {
                        device.instance_id.eq_ignore_ascii_case(id) && device.status != "OK"
                    })
                }) {
                    bail!("Windows did not restore every connected camera device");
                }
                let path = blocked_devices_path()?;
                if pending.is_empty() {
                    fs::remove_file(&path).context("failed to clear camera block record")?;
                } else {
                    fs::write(&path, serde_json::to_vec(&pending)?)
                        .context("failed to save disconnected cameras for later restoration")?;
                }
            }
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
    fn disconnected_camera_does_not_prevent_restoring_connected_camera() {
        let blocked = vec!["USB\\CAMERA_A".to_owned(), "USB\\CAMERA_B".to_owned()];
        let present = CameraDevice {
            instance_id: blocked[0].clone(),
            status: "Error".to_owned(),
        };
        let (restore, pending) = restoration_plan(&blocked, &[present]);
        assert_eq!(restore, vec!["USB\\CAMERA_A"]);
        assert_eq!(pending, vec!["USB\\CAMERA_B"]);

        let reconnected = CameraDevice {
            instance_id: pending[0].clone(),
            status: "Error".to_owned(),
        };
        let (restore_again, still_pending) = restoration_plan(&pending, &[reconnected]);
        assert_eq!(restore_again, pending);
        assert!(still_pending.is_empty());
        let restored = CameraDevice {
            instance_id: blocked[0].clone(),
            status: "OK".to_owned(),
        };
        assert_eq!(
            classify_devices(&pending, &[restored]),
            CameraPrivacyState::Allowed
        );
    }
}
