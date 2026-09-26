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
    Win32::{
        Foundation::{CloseHandle, ERROR_CANCELLED, HANDLE, WAIT_ABANDONED, WAIT_OBJECT_0},
        Storage::FileSystem::{MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW},
        System::Threading::{
            CreateMutexW, GetExitCodeProcess, INFINITE, ReleaseMutex, WaitForSingleObject,
        },
        UI::Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW},
    },
    core::{GUID, PCWSTR, w},
};

// Both the tray and `mcw camera` can change privacy intent. Keep record reads,
// UAC approval, device actions and record writes in one cross-process critical
// section; otherwise an old auto-restore can run after a newer block commits.
// A block requested during an already launched restore waits for it to finish;
// that elevated enable cannot be canceled, but the block executes last.
struct CameraOperationLock(HANDLE);

impl CameraOperationLock {
    fn acquire() -> Result<Self> {
        let handle = unsafe { CreateMutexW(None, false, w!("Local\\MicCamWatch.CameraPrivacy")) }
            .context("failed to create camera operation mutex")?;
        let result = unsafe { WaitForSingleObject(handle, INFINITE) };
        if result != WAIT_OBJECT_0 && result != WAIT_ABANDONED {
            unsafe {
                let _ = CloseHandle(handle);
            }
            bail!("failed to wait for camera operation mutex: {result:?}");
        }
        Ok(Self(handle))
    }
}

impl Drop for CameraOperationLock {
    fn drop(&mut self) {
        unsafe {
            let _ = ReleaseMutex(self.0);
            let _ = CloseHandle(self.0);
        }
    }
}

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
    Ok(if problem == 0 { "OK" } else { "Error" }.to_owned())
}

fn device_status(instance_id: &str) -> Option<String> {
    device_status_strict(instance_id).ok()
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
const INTERFACE_PRESENT: u32 = 0; // CM_GET_DEVICE_INTERFACE_LIST_PRESENT
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
    loop {
        let mut length = 0;
        let res =
            unsafe { CM_Get_Device_ID_List_SizeW(&mut length, filter.as_ptr(), CLASS_PRESENT) };
        if res != 0 {
            bail!("failed to size Windows device inventory: configuration manager error {res}");
        }
        if length == 0 {
            return Ok(Vec::new());
        }
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
}

// Configuration Manager can filter interface paths by their owning PnP
// instance ID. Do not infer identity from the symbolic-link text or query a
// device-instance property on the interface object.
fn has_video_interface(interface: &GUID, instance_id: &str) -> Result<bool> {
    let wide: Vec<u16> = instance_id.encode_utf16().chain(Some(0)).collect();
    loop {
        let mut length = 0;
        let res = unsafe {
            CM_Get_Device_Interface_List_SizeW(
                &mut length,
                interface,
                wide.as_ptr(),
                INTERFACE_PRESENT,
            )
        };
        if res != 0 {
            bail!(
                "failed to size video interfaces for {instance_id}: configuration manager error {res}"
            );
        }
        if length == 0 {
            return Ok(false);
        }
        let mut buffer = vec![0u16; length as usize];
        let res = unsafe {
            CM_Get_Device_Interface_ListW(
                interface,
                wide.as_ptr(),
                buffer.as_mut_ptr(),
                length,
                INTERFACE_PRESENT,
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
    load_record_at(&blocked_devices_path()?)
}

fn load_record_at(path: &Path) -> Result<Option<CameraBlockRecord>> {
    let bytes = match fs::read(path) {
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
    let _operation = CameraOperationLock::acquire()?;
    let Some(mut record) = load_record()? else {
        return Ok(0);
    };
    let targets = pending_targets(&record, device_status);
    if targets.is_empty() {
        return Ok(0);
    }
    pnputil_elevated("enable", &targets)?;
    let failed = settle_arrived(&mut record, &targets, device_status);
    save_record(&record)?;
    if failed {
        bail!("Windows did not restore every reconnected camera device");
    }
    Ok(targets.len())
}

// Only devices targeted by this attempt may make it fail, including a target
// unplugged during elevation. An unrelated absent camera stays pending quietly.
fn settle_arrived(
    record: &mut CameraBlockRecord,
    targets: &[String],
    status: impl Fn(&str) -> Option<String>,
) -> bool {
    let mut failed = false;
    record.restore_on_arrival.retain(|id| match status(id) {
        Some(report) if report == "OK" => false,
        _ => {
            if targets.iter().any(|target| target.eq_ignore_ascii_case(id)) {
                failed = true;
            }
            true
        }
    });
    failed
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
    for dev in devices()? {
        if !current
            .iter()
            .any(|c| c.instance_id.eq_ignore_ascii_case(&dev.instance_id))
        {
            current.push(dev);
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

// Persist the explicit block before returning (including when nothing is
// connected): otherwise an earlier deferred allow would remain live on disk.
fn persist_block_intent(
    mut record: CameraBlockRecord,
    enabled: &[String],
    persist: impl FnOnce(&CameraBlockRecord) -> Result<()>,
) -> Result<()> {
    let had_pending = !record.restore_on_arrival.is_empty();
    record.supersede_pending_restore();
    if enabled.is_empty() {
        if record.blocked.is_empty() {
            bail!("no enabled physical camera devices were found");
        }
        if had_pending {
            persist(&record)?;
        }
        return Ok(());
    }
    for id in enabled {
        if !record.owns(id) {
            record.blocked.push(id.clone());
        }
    }
    persist(&record)
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
    let _operation = CameraOperationLock::acquire()?;
    set_camera_state_locked(state)
}

fn set_camera_state_locked(state: CameraPrivacyState) -> Result<()> {
    match state {
        CameraPrivacyState::Blocked => {
            let record = load_record()?.unwrap_or_default();
            let enabled = devices()?
                .into_iter()
                .filter(|device| device.status == "OK")
                .map(|device| device.instance_id)
                .collect::<Vec<_>>();
            persist_block_intent(record, &enabled, save_record)?;
            if enabled.is_empty() {
                return Ok(());
            }
            pnputil_elevated("disable", &enabled)?;
            // Disabled legacy Image-class cameras lose their present video
            // interface, so checking only a fresh interface inventory would
            // falsely report success. Verify every device we actually targeted.
            for id in &enabled {
                if device_status_strict(id)? == "OK" {
                    bail!("Windows did not disable camera device {id}");
                }
            }
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
    let _operation = CameraOperationLock::acquire()?;
    let next = match camera_state()? {
        CameraPrivacyState::Allowed => CameraPrivacyState::Blocked,
        CameraPrivacyState::Blocked | CameraPrivacyState::SystemManaged => {
            CameraPrivacyState::Allowed
        }
    };
    set_camera_state_locked(next)?;
    camera_state()
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
    fn arrived_restore_settles_only_target_and_leaves_absent_pending() {
        let mut record = CameraBlockRecord {
            blocked: vec!["USB\\CAMERA_BLOCKED".to_owned()],
            restore_on_arrival: vec![
                "USB\\CAMERA_TARGET".to_owned(),
                "USB\\CAMERA_ABSENT".to_owned(),
                "USB\\CAMERA_RUNNING".to_owned(),
            ],
        };
        let targets = pending_targets(&record, |id| match id {
            "USB\\CAMERA_TARGET" => Some("Error".to_owned()),
            "USB\\CAMERA_RUNNING" => Some("OK".to_owned()),
            _ => None,
        });
        assert_eq!(targets, vec!["USB\\CAMERA_TARGET"]);
        let failed = settle_arrived(&mut record, &targets, |id| match id {
            "USB\\CAMERA_TARGET" | "USB\\CAMERA_RUNNING" => Some("OK".to_owned()),
            _ => None,
        });
        assert!(!failed);
        assert_eq!(record.blocked, vec!["USB\\CAMERA_BLOCKED"]);
        assert_eq!(record.restore_on_arrival, vec!["USB\\CAMERA_ABSENT"]);

        let retry = pending_targets(&record, |_| Some("Error".to_owned()));
        assert_eq!(retry, vec!["USB\\CAMERA_ABSENT"]);
        assert!(settle_arrived(&mut record, &retry, |_| Some(
            "Error".to_owned()
        )));
        assert_eq!(record.restore_on_arrival, retry);
    }

    #[test]
    fn targeted_camera_disappearing_during_restore_reports_failure() {
        let mut record = CameraBlockRecord {
            blocked: Vec::new(),
            restore_on_arrival: vec![
                "USB\\CAMERA_TARGET".to_owned(),
                "USB\\CAMERA_STILL_ABSENT".to_owned(),
            ],
        };
        let targets = pending_targets(&record, |id| {
            id.ends_with("TARGET").then(|| "Error".to_owned())
        });
        assert_eq!(targets, vec!["USB\\CAMERA_TARGET"]);
        assert!(settle_arrived(&mut record, &targets, |_| None));
        assert_eq!(
            record.restore_on_arrival,
            vec!["USB\\CAMERA_TARGET", "USB\\CAMERA_STILL_ABSENT"]
        );

        assert!(!settle_arrived(&mut record, &targets, |id| {
            id.ends_with("TARGET").then(|| "OK".to_owned())
        }));
        assert_eq!(record.restore_on_arrival, vec!["USB\\CAMERA_STILL_ABSENT"]);
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
    fn block_with_no_enabled_devices_persists_cancellation() -> Result<()> {
        let record = CameraBlockRecord {
            blocked: vec!["USB\\CAMERA_BUILTIN".to_owned()],
            restore_on_arrival: vec!["USB\\CAMERA_ABSENT".to_owned()],
        };
        let mut persisted = None;
        persist_block_intent(record, &[], |updated| {
            persisted = Some(serde_json::to_vec(updated)?);
            Ok(())
        })?;
        let saved: CameraBlockRecord =
            serde_json::from_slice(&persisted.context("block intent was not saved")?)?;
        assert!(saved.restore_on_arrival.is_empty());
        assert_eq!(
            saved.blocked,
            vec!["USB\\CAMERA_BUILTIN", "USB\\CAMERA_ABSENT"]
        );
        assert!(pending_targets(&saved, |_| Some("Error".to_owned())).is_empty());
        Ok(())
    }

    #[test]
    fn block_waits_out_restore_and_cancels_stale_arrival() -> Result<()> {
        use std::sync::{Arc, Mutex, mpsc};

        let pending = CameraBlockRecord {
            blocked: Vec::new(),
            restore_on_arrival: vec!["USB\\CAMERA_ABSENT".to_owned()],
        };
        let saved = Arc::new(Mutex::new(serde_json::to_vec(&pending)?));
        let _block = CameraOperationLock::acquire()?;
        let (probe_tx, probe_rx) = mpsc::channel();
        let saved_for_restore = Arc::clone(&saved);
        let restore = std::thread::spawn(move || -> Result<Vec<String>> {
            let handle =
                unsafe { CreateMutexW(None, false, w!("Local\\MicCamWatch.CameraPrivacy")) }?;
            let waited = unsafe { WaitForSingleObject(handle, 0) };
            unsafe { CloseHandle(handle)? };
            probe_tx.send(waited)?;
            let _operation = CameraOperationLock::acquire()?;
            let bytes = saved_for_restore
                .lock()
                .expect("simulated record lock")
                .clone();
            let record: CameraBlockRecord = serde_json::from_slice(&bytes)?;
            Ok(pending_targets(&record, |_| Some("Error".to_owned())))
        });
        assert_eq!(probe_rx.recv()?, windows::Win32::Foundation::WAIT_TIMEOUT);
        persist_block_intent(pending, &[], |updated| {
            *saved.lock().expect("simulated record lock") = serde_json::to_vec(updated)?;
            Ok(())
        })?;
        drop(_block);
        assert!(restore.join().expect("restore thread panicked")?.is_empty());
        Ok(())
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
        };
        let replacement = CameraBlockRecord {
            blocked: vec!["USB\\CAMERA_NEW".to_owned()],
            restore_on_arrival: vec!["USB\\CAMERA_LATER".to_owned()],
        };
        save_record_at(&path, &original)?;
        assert_eq!(load_record_at(&path)?, Some(original));
        save_record_at(&path, &replacement)?;
        assert_eq!(load_record_at(&path)?, Some(replacement));
        assert_eq!(fs::read_dir(&dir.0)?.count(), 1);
        Ok(())
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
