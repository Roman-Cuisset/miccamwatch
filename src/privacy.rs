use crate::model::CameraBlockRecord;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    io::Write,
    mem::size_of,
    os::windows::ffi::OsStrExt,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};
use windows::{
    Win32::Storage::FileSystem::{MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW},
    core::{GUID, PCWSTR},
};

#[link(name = "cfgmgr32")]
unsafe extern "system" {
    fn CM_Locate_DevNodeW(pdndevinst: *mut u32, pdeviceid: *const u16, ulflags: u32) -> u32;
    fn CM_Get_DevNode_Status(
        pulstatus: *mut u32,
        pulproblemnumber: *mut u32,
        dndevinst: u32,
        ulflags: u32,
    ) -> u32;
    fn CM_Get_Device_ID_List_SizeW(pul_len: *mut u32, filter: *const u16, flags: u32) -> u32;
    fn CM_Get_Device_ID_ListW(
        filter: *const u16,
        buffer: *mut u16,
        buffer_len: u32,
        flags: u32,
    ) -> u32;
    fn CM_Get_Device_Interface_List_SizeW(
        length: *mut u32,
        interface_class: *const GUID,
        device_id: *const u16,
        flags: u32,
    ) -> u32;
    fn CM_Get_Device_Interface_ListW(
        interface_class: *const GUID,
        device_id: *const u16,
        buffer: *mut u16,
        buffer_len: u32,
        flags: u32,
    ) -> u32;
}

fn device_status_strict(instance_id: &str) -> Result<String> {
    let wide: Vec<u16> = instance_id.encode_utf16().chain(Some(0)).collect();
    let mut devinst = 0u32;
    let res = unsafe { CM_Locate_DevNodeW(&mut devinst, wide.as_ptr(), 0) };
    if res != 0 {
        bail!("failed to locate device {instance_id}: configuration manager error {res}");
    }
    let mut status = 0u32;
    let mut problem = 0u32;
    let res = unsafe { CM_Get_DevNode_Status(&mut status, &mut problem, devinst, 0) };
    if res != 0 {
        bail!("failed to read device {instance_id} status: configuration manager error {res}");
    }
    Ok(if problem == 22 {
        "Disabled"
    } else if problem == 0 && status & 8 != 0 {
        "OK"
    } else {
        "Unknown"
    }
    .to_owned())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
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

struct CameraDevice {
    instance_id: String,
    status: String,
}

// Setup classes are distinct from the KS video device interface categories below.
const CAMERA_CLASS: &str = "{ca3e7ab9-b4c3-4ae6-8251-579ef933890f}";
const IMAGE_CLASS: &str = "{6bdd1fc6-810f-11d0-bec7-08002be2092f}";
// A legacy DirectShow video capture device registers both VIDEO and CAPTURE.
// CAPTURE alone also includes audio, while VIDEO alone also includes non-capture devices.
const VIDEO_CAMERA_INTERFACE: GUID = GUID::from_u128(0xe5323777_f976_4f5b_9b55_b94699c46e44);
const VIDEO_INTERFACE: GUID = GUID::from_u128(0x6994ad05_93ef_11d0_a3cc_00a0c9223196);
const CAPTURE_INTERFACE: GUID = GUID::from_u128(0x65e8773d_8f56_11d0_a3b9_00a0c9223196);
const CLASS_PRESENT: u32 = 0x0000_0300; // CM_GETIDLIST_FILTER_CLASS | CM_GETIDLIST_FILTER_PRESENT
const INTERFACE_REGISTERED: u32 = 1; // CM_GET_DEVICE_INTERFACE_LIST_ALL_DEVICES
const CR_BUFFER_SMALL: u32 = 0x1a;

fn parse_device_ids(buffer: &[u16]) -> Result<Vec<String>> {
    let mut ids = Vec::new();
    let mut start = 0;
    loop {
        let end = buffer[start..]
            .iter()
            .position(|&c| c == 0)
            .map(|end| start + end)
            .context("unterminated Windows device inventory")?;
        if end == start {
            return Ok(ids);
        }
        ids.push(String::from_utf16(&buffer[start..end]).context("invalid Windows device ID")?);
        start = end + 1;
        if start >= buffer.len() {
            bail!("unterminated Windows device inventory");
        }
    }
}

fn class_device_ids(class_guid: &str) -> Result<Vec<String>> {
    let filter: Vec<u16> = class_guid.encode_utf16().chain(Some(0)).collect();
    for _ in 0..8 {
        let mut length = 0;
        let res =
            unsafe { CM_Get_Device_ID_List_SizeW(&mut length, filter.as_ptr(), CLASS_PRESENT) };
        if res != 0 {
            bail!("failed to size Windows device inventory: configuration manager error {res}");
        }
        if length == 0 {
            return Ok(Vec::new());
        }
        anyhow::ensure!(
            length <= 1_048_576,
            "Windows camera inventory exceeds the explicit 2 MiB UTF-16 buffer limit"
        );
        let mut buffer = vec![0u16; length as usize];
        let res = unsafe {
            CM_Get_Device_ID_ListW(filter.as_ptr(), buffer.as_mut_ptr(), length, CLASS_PRESENT)
        };
        if res == CR_BUFFER_SMALL {
            continue; // PnP tree changed between the size and list calls.
        }
        if res != 0 {
            bail!("failed to list Windows devices: configuration manager error {res}");
        }
        return parse_device_ids(&buffer);
    }
    bail!("Windows camera inventory changed during all eight enumeration attempts")
}

// Configuration Manager can filter interface paths by their owning PnP
// instance ID. Do not infer identity from the symbolic-link text or query a
// device-instance property on the interface object.
fn has_video_interface(interface: &GUID, instance_id: &str) -> Result<bool> {
    video_interface(interface, instance_id, INTERFACE_REGISTERED)
}

// Class inventory independently selects present devnodes. Registered interfaces
// classify their exact identity even when a disable makes interfaces inactive;
// native status, not interface activation, determines whether a camera is enabled.
fn video_interface(interface: &GUID, instance_id: &str, flags: u32) -> Result<bool> {
    let wide: Vec<u16> = instance_id.encode_utf16().chain(Some(0)).collect();
    for _ in 0..8 {
        let mut length = 0;
        let res = unsafe {
            CM_Get_Device_Interface_List_SizeW(&mut length, interface, wide.as_ptr(), flags)
        };
        if res != 0 {
            bail!(
                "failed to size video interfaces for {instance_id}: configuration manager error {res}"
            );
        }
        if length == 0 {
            return Ok(false);
        }
        anyhow::ensure!(
            length <= 1_048_576,
            "Windows camera interface inventory exceeds the explicit 2 MiB UTF-16 buffer limit"
        );
        let mut buffer = vec![0u16; length as usize];
        let res = unsafe {
            CM_Get_Device_Interface_ListW(
                interface,
                wide.as_ptr(),
                buffer.as_mut_ptr(),
                length,
                flags,
            )
        };
        if res == CR_BUFFER_SMALL {
            continue;
        }
        if res != 0 {
            bail!(
                "failed to list video interfaces for {instance_id}: configuration manager error {res}"
            );
        }
        return Ok(!parse_device_ids(&buffer)?.is_empty());
    }
    bail!("Windows camera interface inventory changed during all eight enumeration attempts")
}

fn video_capture_ids(
    images: &[String],
    mut query_interface: impl FnMut(&GUID, &str) -> Result<bool>,
) -> Result<HashSet<String>> {
    let mut capture_ids = HashSet::new();
    for id in images {
        let modern = query_interface(&VIDEO_CAMERA_INTERFACE, id)
            .with_context(|| format!("failed to enumerate video camera interfaces for {id}"))?;
        let video = query_interface(&VIDEO_INTERFACE, id)
            .with_context(|| format!("failed to enumerate legacy video interfaces for {id}"))?;
        let capture = query_interface(&CAPTURE_INTERFACE, id)
            .with_context(|| format!("failed to enumerate legacy capture interfaces for {id}"))?;
        if modern || (video && capture) {
            capture_ids.insert(id.to_ascii_lowercase());
        }
    }
    Ok(capture_ids)
}

fn collect_camera_devices(
    mut query_class: impl FnMut(&str) -> Result<Vec<String>>,
    query_interface: impl FnMut(&GUID, &str) -> Result<bool>,
    mut status: impl FnMut(&str) -> Result<String>,
) -> Result<Vec<CameraDevice>> {
    let camera = query_class(CAMERA_CLASS).context("failed to enumerate Camera devices")?;
    let image = query_class(IMAGE_CLASS).context("failed to enumerate Image devices")?;
    let capture = video_capture_ids(&image, query_interface)?;
    let mut found = Vec::new();
    let mut seen = HashSet::new();
    for id in camera.into_iter().chain(
        image
            .into_iter()
            .filter(|id| capture.contains(&id.to_ascii_lowercase())),
    ) {
        if seen.insert(id.to_ascii_lowercase()) {
            found.push(CameraDevice {
                status: status(&id)?,
                instance_id: id,
            });
        }
    }
    Ok(found)
}

fn devices() -> Result<Vec<CameraDevice>> {
    collect_camera_devices(class_device_ids, has_video_interface, device_status_strict)
}

fn blocked_devices_path() -> Result<std::path::PathBuf> {
    Ok(crate::settings::data_dir()?.join("blocked-camera-devices.json"))
}

impl CameraBlockRecord {
    fn is_empty(&self) -> bool {
        !self.desired_blocked
            && self.intent_generation == 0
            && self.blocked.is_empty()
            && self.restore_on_arrival.is_empty()
    }

    fn owns(&self, instance_id: &str) -> bool {
        self.blocked
            .iter()
            .chain(self.restore_on_arrival.iter())
            .any(|id| id.eq_ignore_ascii_case(instance_id))
    }
}

fn dedupe_ascii_case(ids: &mut Vec<String>) {
    let mut seen = HashSet::new();
    ids.retain(|id| seen.insert(id.to_ascii_lowercase()));
}

fn load_record() -> Result<Option<CameraBlockRecord>> {
    load_record_at(&blocked_devices_path()?)
}

fn load_record_at(path: &Path) -> Result<Option<CameraBlockRecord>> {
    if let Ok(metadata) = fs::metadata(path) {
        anyhow::ensure!(
            metadata.len() <= 2 * 1024 * 1024,
            "camera intent record exceeds 2 MiB"
        );
    }
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", path.display()));
        }
    };
    anyhow::ensure!(
        bytes.len() <= 2 * 1024 * 1024,
        "camera intent record exceeds 2 MiB"
    );
    let mut record = CameraBlockRecord::from_json(&bytes)
        .with_context(|| format!("invalid camera block record {}", path.display()))?;
    // Unsigned pre-policy records retain their historical requested intent, but
    // generation zero never becomes privileged native restoration authority.
    if record.intent_generation == 0 && !record.blocked.is_empty() {
        record.desired_blocked = true;
    }
    Ok(Some(record))
}

fn save_record(record: &CameraBlockRecord) -> Result<()> {
    save_record_at(&blocked_devices_path()?, record)
}

fn save_record_at(path: &Path, record: &CameraBlockRecord) -> Result<()> {
    if record.is_empty() {
        return match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error).context("failed to clear camera block record"),
        };
    }
    fs::create_dir_all(path.parent().context("camera block path has no parent")?)?;
    let bytes = serde_json::to_vec(record)?;
    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
    let (temp_path, mut file) = loop {
        let nonce = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let candidate = path.with_extension(format!("json.tmp.{}.{}", std::process::id(), nonce));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => break (candidate, file),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error).context("failed to create camera block record update"),
        }
    };
    let result: Result<()> = (|| {
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        let from: Vec<u16> = temp_path.as_os_str().encode_wide().chain(Some(0)).collect();
        let to: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        // Replace in place so concurrent readers see either complete record.
        unsafe {
            MoveFileExW(
                PCWSTR(from.as_ptr()),
                PCWSTR(to.as_ptr()),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        }?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result.with_context(|| format!("failed to save camera device state at {}", path.display()))
}
mod windows_guard;
pub(crate) use windows_guard::ensure_update_inactive as ensure_camera_update_inactive;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraControlObservation {
    pub desired_blocked: bool,
    pub state: CameraPrivacyState,
    pub present_total: usize,
    pub owned_blocked_present: usize,
    pub pending_restore: usize,
    pub absent_owned: usize,
    pub unknown_devices: usize,
    pub helper_active: bool,
    pub detail: Option<String>,
}

pub fn camera_observation() -> Result<CameraControlObservation> {
    windows_guard::observation()
}

pub fn camera_state() -> Result<CameraPrivacyState> {
    Ok(camera_observation()?.state)
}

pub fn set_camera_state(state: CameraPrivacyState) -> Result<()> {
    anyhow::ensure!(
        state != CameraPrivacyState::SystemManaged,
        "system-managed camera state cannot be selected"
    );
    windows_guard::set_requested(state == CameraPrivacyState::Blocked)
}

pub fn toggle_camera() -> Result<CameraPrivacyState> {
    windows_guard::toggle_requested()?;
    camera_state()
}

pub fn arrived_restore_targets() -> Result<Vec<String>> {
    windows_guard::arrived_targets()
}

pub fn restore_arrived() -> Result<usize> {
    windows_guard::restore_arrived()
}

pub fn run_camera_protection_service(bootstrap: &str) -> Result<()> {
    windows_guard::run(bootstrap)
}

/// Explicit informed permission to override one historical camera disable.
/// Never invoked automatically from a writable camera record.
pub fn restore_legacy_camera(instance_id: &str) -> Result<CameraControlObservation> {
    windows_guard::restore_legacy(instance_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_inventory_parses_utf16_ids_without_localized_headings() -> Result<()> {
        let ids: Vec<u16> = "USB\\CAMÉRA\0USB\\画像\0\0".encode_utf16().collect();
        assert_eq!(parse_device_ids(&ids)?, vec!["USB\\CAMÉRA", "USB\\画像"]);
        assert!(parse_device_ids(&"USB\\CAMERA\0".encode_utf16().collect::<Vec<_>>()).is_err());
        assert!(parse_device_ids(&[0xd800, 0, 0]).is_err());
        Ok(())
    }

    #[test]
    fn video_capture_membership_filters_legacy_image_devices() -> Result<()> {
        let devices = collect_camera_devices(
            |class| {
                Ok(if class == CAMERA_CLASS {
                    vec!["USB\\MODERN".to_owned()]
                } else {
                    vec![
                        "usb\\modern".to_owned(),
                        "USB\\LEGACY".to_owned(),
                        "USB\\CAMERA_INTERFACE".to_owned(),
                        "USB\\SCANNER".to_owned(),
                        "USB\\VIDEO_ONLY".to_owned(),
                        "USB\\CAPTURE_ONLY".to_owned(),
                    ]
                })
            },
            |category, id| {
                Ok(if *category == VIDEO_CAMERA_INTERFACE {
                    id.eq_ignore_ascii_case("USB\\CAMERA_INTERFACE")
                } else if *category == VIDEO_INTERFACE {
                    id.eq_ignore_ascii_case("USB\\LEGACY") || id == "USB\\VIDEO_ONLY"
                } else {
                    id == "USB\\LEGACY" || id == "USB\\CAPTURE_ONLY"
                })
            },
            |_| Ok("OK".to_owned()),
        )?;
        assert_eq!(
            devices
                .iter()
                .map(|device| device.instance_id.as_str())
                .collect::<Vec<_>>(),
            vec!["USB\\MODERN", "USB\\LEGACY", "USB\\CAMERA_INTERFACE"]
        );
        Ok(())
    }

    #[test]
    fn registered_inactive_image_video_interfaces_keep_disabled_present_cameras() -> Result<()> {
        assert_eq!(
            INTERFACE_REGISTERED, 1,
            "identity classification must not depend on interface activation"
        );
        let inventory = collect_camera_devices(
            |class| {
                Ok(if class == IMAGE_CLASS {
                    vec![
                        "USB\\DISABLED_CAMERA".to_owned(),
                        "USB\\SCANNER".to_owned(),
                        "USB\\AUDIO_ONLY".to_owned(),
                    ]
                } else {
                    Vec::new()
                })
            },
            |category, id| {
                Ok((id == "USB\\DISABLED_CAMERA"
                    && (*category == VIDEO_INTERFACE || *category == CAPTURE_INTERFACE))
                    || (id == "USB\\AUDIO_ONLY" && *category == CAPTURE_INTERFACE))
            },
            |id| {
                assert_eq!(id, "USB\\DISABLED_CAMERA");
                Ok("Disabled".to_owned())
            },
        )?;
        assert_eq!(inventory.len(), 1);
        assert_eq!(inventory[0].instance_id, "USB\\DISABLED_CAMERA");
        assert_eq!(inventory[0].status, "Disabled");
        Ok(())
    }

    #[test]
    fn failed_native_queries_and_status_reject_entire_inventory() {
        for failed_class in [CAMERA_CLASS, IMAGE_CLASS] {
            let error = collect_camera_devices(
                |class| {
                    if class == failed_class {
                        bail!("class failed");
                    }
                    Ok(vec!["USB\\CAMERA".to_owned()])
                },
                |_, _| Ok(false),
                |_| Ok("OK".to_owned()),
            )
            .err()
            .expect("failed setup class must reject inventory");
            assert!(format!("{error:#}").contains("class failed"));
        }
        for failed_interface in [VIDEO_CAMERA_INTERFACE, VIDEO_INTERFACE, CAPTURE_INTERFACE] {
            let error = collect_camera_devices(
                |class| {
                    Ok(if class == CAMERA_CLASS {
                        vec!["USB\\CAMERA".to_owned()]
                    } else {
                        vec!["USB\\LEGACY".to_owned()]
                    })
                },
                |category, _| {
                    if *category == failed_interface {
                        bail!("interface failed");
                    }
                    Ok(false)
                },
                |_| Ok("OK".to_owned()),
            )
            .err()
            .expect("failed interface mapping must reject inventory");
            assert!(format!("{error:#}").contains("interface failed"));
        }
        let error = collect_camera_devices(
            |class| {
                Ok(if class == IMAGE_CLASS {
                    vec!["USB\\LEGACY".to_owned()]
                } else {
                    vec!["USB\\CAMERA".to_owned()]
                })
            },
            |category, _| Ok(*category == VIDEO_INTERFACE || *category == CAPTURE_INTERFACE),
            |id| {
                if id == "USB\\LEGACY" {
                    bail!("status failed")
                } else {
                    Ok("OK".to_owned())
                }
            },
        )
        .err()
        .expect("unknown included camera status must reject inventory");
        assert!(format!("{error:#}").contains("status failed"));
    }

    #[test]
    fn record_replacement_preserves_valid_json_and_cleans_temporary_file() -> Result<()> {
        use std::path::PathBuf;

        struct TempRecordDir(PathBuf);
        impl Drop for TempRecordDir {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        static NEXT_TEST_DIR: AtomicU64 = AtomicU64::new(0);
        let dir = TempRecordDir(std::env::temp_dir().join(format!(
            "mcw-privacy-{}-{}",
            std::process::id(),
            NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed)
        )));
        fs::create_dir(&dir.0)?;
        let path = dir.0.join("blocked-camera-devices.json");
        let original = CameraBlockRecord {
            blocked: vec!["USB\\CAMERA_OLD".to_owned()],
            restore_on_arrival: Vec::new(),
            desired_blocked: true,
            ..Default::default()
        };
        let replacement = CameraBlockRecord {
            blocked: vec!["USB\\CAMERA_NEW".to_owned()],
            restore_on_arrival: vec!["USB\\CAMERA_LATER".to_owned()],
            desired_blocked: true,
            ..Default::default()
        };
        save_record_at(&path, &original)?;
        assert_eq!(load_record_at(&path)?, Some(original));
        save_record_at(&path, &replacement)?;
        assert_eq!(load_record_at(&path)?, Some(replacement));
        assert_eq!(fs::read_dir(&dir.0)?.count(), 1);
        for legacy in [
            br#"["USB\\CAMERA_A","USB\\CAMERA_B"]"#.as_slice(),
            br#"{"devices":["USB\\CAMERA_A","USB\\CAMERA_B"]}"#,
        ] {
            fs::write(&path, legacy)?;
            let migrated = load_record_at(&path)?.expect("legacy record remains readable");
            assert_eq!(migrated.blocked, ["USB\\CAMERA_A", "USB\\CAMERA_B"]);
            assert!(migrated.restore_on_arrival.is_empty());
            assert!(
                migrated.desired_blocked,
                "legacy active blocks retain a requested global Block"
            );
            assert_eq!(migrated.intent_generation, 0, "legacy IDs stay unsigned");
            save_record_at(&path, &migrated)?;
            assert_eq!(load_record_at(&path)?, Some(migrated));
        }
        fs::write(&path, br#"{"blocked":[],"restore_on_arrival":["USB\\VID_1234&PID_5678&MI_00\\synthetic-camera-instance"]}"#)?;
        let owed_only =
            load_record_at(&path)?.expect("owed-only historical record remains readable");
        assert!(
            !owed_only.desired_blocked,
            "owed historical restoration is not an active global Block"
        );
        assert_eq!(owed_only.intent_generation, 0);
        assert_eq!(owed_only.restore_on_arrival.len(), 1);
        fs::write(&path, br#"{"devices":["A"],"blocked":["B"]}"#)?;
        assert!(
            load_record_at(&path).is_err(),
            "ambiguous intent must not be discarded"
        );
        Ok(())
    }
}
