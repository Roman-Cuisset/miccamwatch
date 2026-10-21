use crate::platform::macos::control::{
    CameraProfile, OperationLock, Request, atomic_json, request,
};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::Path,
    time::Duration,
};

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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CameraInspectionAuthorization {
    NotRequested,
    Granted,
    Cancelled,
    Denied,
}

#[derive(Debug, Serialize)]
pub struct CameraInspection {
    pub observed_at: chrono::DateTime<chrono::Utc>,
    pub state: &'static str,
    pub complete_inventory: bool,
    pub administrator_required: bool,
    pub authorization: CameraInspectionAuthorization,
    pub detail: String,
}

#[derive(Debug)]
pub struct CameraInventoryIncomplete {
    detail: String,
}
impl std::fmt::Display for CameraInventoryIncomplete {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Camera restriction status unknown: {}",
            self.detail
        )
    }
}
impl std::error::Error for CameraInventoryIncomplete {}

fn inspection(profile: CameraProfile) -> Result<CameraInspection> {
    if profile.detail.len() > 8192 {
        bail!("Native camera-profile diagnostic exceeds the 8192-byte limit");
    }
    let state = match profile.state.as_str() {
        "allowed" if profile.complete_inventory => "allowed",
        "blocked" if profile.owned_installed => "blocked",
        "system_managed" => "system_managed",
        "unknown" => "unknown",
        _ => bail!("Inconsistent native camera-profile inventory"),
    };
    let authorization = match profile.authorization.as_str() {
        "not_requested" => CameraInspectionAuthorization::NotRequested,
        "granted" => CameraInspectionAuthorization::Granted,
        "cancelled" => CameraInspectionAuthorization::Cancelled,
        "denied" => CameraInspectionAuthorization::Denied,
        _ => bail!("Unrecognized native inspection authorization outcome"),
    };
    if matches!(
        authorization,
        CameraInspectionAuthorization::Cancelled | CameraInspectionAuthorization::Denied
    ) && (state != "unknown" || profile.complete_inventory)
    {
        bail!("Refused inspection cannot establish a complete camera inventory");
    }
    if authorization == CameraInspectionAuthorization::Granted
        && (!profile.complete_inventory || state == "unknown")
    {
        bail!("Administrator approval did not establish a complete camera inventory");
    }
    Ok(CameraInspection {
        observed_at: chrono::Utc::now(),
        state,
        complete_inventory: profile.complete_inventory,
        administrator_required: state == "unknown" && !profile.complete_inventory,
        authorization,
        detail: profile.detail,
    })
}

/// Point-in-time read-only metadata. Never updates control intent or ownership.
pub fn camera_inspection(request_admin: bool) -> Result<CameraInspection> {
    // No receipt is needed: diagnostics must work with corrupt/nonregular user
    // state and must never adopt ownership from a global profile match.
    let current = inspection(camera_request("camera_status", None, None)?)?;
    if !request_admin || !current.administrator_required {
        return Ok(current);
    }
    // The privileged query accepts no journal identity, path, command or output file.
    inspection(camera_request("camera_status_admin", None, None)?)
}
#[derive(Debug, Serialize, Deserialize)]
struct OwnedProfile {
    version: u32,
    identifier: String,
    uuid: String,
    payload_uuid: String,
    intent: String,
}
fn canonical_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}
fn valid_owned(profile: &OwnedProfile) -> bool {
    profile.version == 1
        && canonical_uuid(&profile.uuid)
        && canonical_uuid(&profile.payload_uuid)
        && profile.identifier
            == format!(
                "com.roman-cuisset.miccamwatch.camera.{}",
                profile.uuid.to_ascii_lowercase()
            )
        && matches!(profile.intent.as_str(), "install" | "remove")
}
fn load(path: &Path) -> Result<Option<OwnedProfile>> {
    match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)
    {
        Ok(file) => {
            use std::os::unix::fs::MetadataExt;
            let metadata = file.metadata()?;
            if !metadata.is_file() || metadata.uid() != unsafe { libc::geteuid() } {
                bail!("camera ownership receipt is not a regular file owned by this user");
            }
            let mut bytes = Vec::new();
            file.take(16_385).read_to_end(&mut bytes)?;
            if bytes.len() > 16_384 {
                bail!("camera ownership receipt exceeds the 16 KiB limit");
            }
            let profile: OwnedProfile =
                serde_json::from_slice(&bytes).context("invalid owned camera profile record")?;
            if !valid_owned(&profile) {
                bail!("invalid owned camera profile identity; refusing profile action");
            }
            Ok(Some(profile))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}
fn uuid() -> Result<String> {
    let mut bytes = [0u8; 16];
    File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Ok(format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15]
    ))
}
fn create_owned() -> Result<OwnedProfile> {
    let profile_uuid = uuid()?;
    Ok(OwnedProfile {
        version: 1,
        identifier: format!("com.roman-cuisset.miccamwatch.camera.{profile_uuid}"),
        uuid: profile_uuid,
        payload_uuid: uuid()?,
        intent: "install".into(),
    })
}
fn profile_xml(profile: &OwnedProfile) -> Result<String> {
    if !valid_owned(profile) {
        bail!("invalid owned profile identity");
    }
    // Apple device-management release schema: Restrictions supports manual
    // macOS installation and user channel; allowCamera=false disables cameras.
    // https://github.com/apple/device-management/blob/release/mdm/profiles/com.apple.applicationaccess.yaml
    // This is NOT PPPC, TCC modification, SIP modification, or physical cutoff.
    Ok(format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>PayloadType</key><string>Configuration</string>
<key>PayloadVersion</key><integer>1</integer>
<key>PayloadIdentifier</key><string>{identifier}</string>
<key>PayloadUUID</key><string>{uuid}</string>
<key>PayloadScope</key><string>User</string>
<key>PayloadDisplayName</key><string>MicCamWatch Camera Restriction</string>
<key>PayloadDescription</key><string>User-approved camera restriction. Remove only this profile to restore its prior restriction state; other managed profiles are unaffected.</string>
<key>PayloadOrganization</key><string>MicCamWatch</string>
<key>PayloadRemovalDisallowed</key><false/>
<key>PayloadContent</key><array><dict>
<key>PayloadType</key><string>com.apple.applicationaccess</string>
<key>PayloadVersion</key><integer>1</integer>
<key>PayloadIdentifier</key><string>{identifier}.restrictions</string>
<key>PayloadUUID</key><string>{payload_uuid}</string>
<key>PayloadDisplayName</key><string>Camera Restriction</string>
<key>allowCamera</key><false/>
</dict></array>
</dict></plist>
"#,
        identifier = profile.identifier,
        uuid = profile.uuid,
        payload_uuid = profile.payload_uuid
    ))
}
fn write_profile(path: &Path, profile: &OwnedProfile) -> Result<()> {
    let temp = path.with_extension(format!("tmp-{}", std::process::id()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&temp)?;
        file.write_all(profile_xml(profile)?.as_bytes())?;
        file.sync_all()?;
        fs::rename(&temp, path)?;
        File::open(path.parent().context("profile has no parent")?)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}
fn camera_request(
    operation: &str,
    profile: Option<&OwnedProfile>,
    path: Option<&Path>,
) -> Result<CameraProfile> {
    let mut req = Request::new(operation);
    req.identifier = profile.map(|p| p.identifier.as_str());
    req.uuid = profile.map(|p| p.uuid.as_str());
    req.path = path
        .map(|p| p.to_str().context("profile path is not UTF-8"))
        .transpose()?;
    let timeout = if operation == "camera_status" {
        Duration::from_secs(15)
    } else {
        Duration::from_secs(120)
    };
    let reply = request(&req, timeout)?;
    if !reply.ok {
        bail!(
            "{}",
            reply
                .error
                .as_deref()
                .unwrap_or("native camera profile operation failed")
        );
    }
    reply
        .camera
        .context("native helper omitted camera profile metadata")
}
fn established_state(profile: &CameraProfile) -> Result<CameraPrivacyState> {
    match profile.state.as_str() {
        "allowed" if profile.complete_inventory => Ok(CameraPrivacyState::Allowed),
        "blocked" if profile.owned_installed => Ok(CameraPrivacyState::Blocked),
        "system_managed" => Ok(CameraPrivacyState::SystemManaged),
        "unknown" if !profile.complete_inventory => Err(CameraInventoryIncomplete {
            detail: profile.detail.clone(),
        }
        .into()),
        _ => bail!("Camera restriction status unknown: {}", profile.detail),
    }
}
pub fn camera_capability() -> &'static str {
    "Manually approved macOS Restrictions profile (allowCamera=false); no silent action while locked, no per-app permission changes, no proven physical cutoff"
}
pub fn camera_detail() -> Result<String> {
    let lock = OperationLock::try_acquire("camera").context(
        "camera observation unavailable: native ownership is busy or cannot be inspected",
    )?;
    let owned = load(&lock.directory.join("camera.json"))?;
    let status = camera_request("camera_status", owned.as_ref(), None)?;
    let pending = owned.as_ref().map(|p| {
        if p.intent == "install" && !status.owned_installed { " Installation approval is pending; file creation does not block the camera." }
        else if p.intent == "remove" && status.owned_installed { " Removal approval is pending; remove only the owned MicCamWatch profile in Device Management." }
        else { "" }
    }).unwrap_or("");
    Ok(format!("{}{}", status.detail, pending))
}
pub fn camera_state() -> Result<CameraPrivacyState> {
    let lock = OperationLock::try_acquire("camera").context(
        "camera observation unavailable: native ownership is busy or cannot be inspected",
    )?;
    let owned = load(&lock.directory.join("camera.json"))?;
    established_state(&camera_request("camera_status", owned.as_ref(), None)?)
}

pub(crate) fn reserve_uninstall() -> Result<OperationLock> {
    let lock = OperationLock::try_acquire("camera")?;
    let owned = load(&lock.directory.join("camera.json"))?;
    // Uninstall reserves this application's restoration obligations, not the
    // effective camera policy of other users or device management. An absent
    // owned receipt under the native lock needs no global inventory or admin
    // approval, and does not establish that the camera is globally allowed.
    if owned.is_none() {
        return Ok(lock);
    }
    if owned
        .as_ref()
        .is_some_and(|profile| profile.intent == "install")
    {
        bail!(
            "uninstall refused: camera protection is requested; explicitly run mcw camera allow and complete profile removal in System Settings > General > Device Management"
        );
    }
    let status = camera_request("camera_status", owned.as_ref(), None)?;
    if status.owned_installed || established_state(&status)? != CameraPrivacyState::Allowed {
        bail!(
            "uninstall refused: camera restoration is pending or unknown; run mcw camera allow, resolve Device Management approval and inspect the status before retrying"
        );
    }
    Ok(lock)
}
pub fn set_camera_state(state: CameraPrivacyState) -> Result<()> {
    let lock = OperationLock::acquire("camera")?;
    set_locked(state, &lock)
}
fn set_locked(state: CameraPrivacyState, lock: &OperationLock) -> Result<()> {
    if state == CameraPrivacyState::SystemManaged {
        bail!("Other management profiles cannot be changed by MicCamWatch");
    }
    let record_path = lock.directory.join("camera.json");
    let mut owned = load(&record_path)?;
    match state {
        CameraPrivacyState::Blocked => {
            if owned.is_none() {
                owned = Some(create_owned()?);
            }
            let profile = owned.as_mut().unwrap();
            profile.intent = "install".into();
            // Identity/intent is durable BEFORE preparing/opening the profile.
            atomic_json(&record_path, profile)?;
            let before = camera_request("camera_status", Some(profile), None)?;
            if before.owned_installed && before.state == "blocked" {
                return Ok(());
            }
            let path = lock.directory.join("camera-restriction.mobileconfig");
            write_profile(&path, profile)?;
            let opened = camera_request("camera_open", Some(profile), Some(&path))?;
            let after = camera_request("camera_status", Some(profile), None)?;
            if after.owned_installed && after.state == "blocked" {
                return Ok(());
            }
            bail!(
                "Camera blocking pending manual approval (not blocked success): {} Open System Settings > General > Device Management, review and install MicCamWatch Camera Restriction ({}; UUID {}).",
                opened.detail,
                profile.identifier,
                profile.uuid
            );
        }
        CameraPrivacyState::Allowed => {
            let Some(profile) = owned.as_mut() else {
                let current = camera_request("camera_status", None, None)?;
                if established_state(&current)? == CameraPrivacyState::Allowed {
                    return Ok(());
                }
                bail!(
                    "No owned camera restriction can be removed; other profiles are preserved: {}",
                    current.detail
                );
            };
            profile.intent = "remove".into();
            atomic_json(&record_path, profile)?;
            let before = camera_request("camera_status", Some(profile), None)?;
            if before.owned_installed {
                let opened = camera_request("camera_remove", Some(profile), None)?;
                let after = camera_request("camera_status", Some(profile), None)?;
                if after.owned_installed {
                    bail!(
                        "Camera restoration pending explicit removal approval: {}",
                        opened.detail
                    );
                }
                if established_state(&after)? != CameraPrivacyState::Allowed {
                    bail!(
                        "Owned profile removed but another camera restriction remains: {}",
                        after.detail
                    );
                }
            } else if established_state(&before)? != CameraPrivacyState::Allowed {
                bail!(
                    "Owned profile is not installed, but effective restoration is not established: {}",
                    before.detail
                );
            }
            // Keep ownership identity for later reconnect/readback and reuse.
            Ok(())
        }
        CameraPrivacyState::SystemManaged => unreachable!(),
    }
}
pub fn toggle_camera() -> Result<CameraPrivacyState> {
    let lock = OperationLock::acquire("camera")?;
    let owned = load(&lock.directory.join("camera.json"))?;
    let current = camera_request("camera_status", owned.as_ref(), None)?;
    // Unknown absence may still start a manually approved restriction workflow.
    let next = if current.owned_installed {
        CameraPrivacyState::Allowed
    } else {
        CameraPrivacyState::Blocked
    };
    set_locked(next, &lock)?;
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn profile() -> OwnedProfile {
        let uuid = "a19a13b1-fbbd-4ecd-8c05-ffb265a0e435".to_owned();
        OwnedProfile {
            version: 1,
            identifier: format!("com.roman-cuisset.miccamwatch.camera.{uuid}"),
            uuid,
            payload_uuid: "900c15c3-5b51-4aaa-b914-0f9f1529510f".into(),
            intent: "install".into(),
        }
    }
    #[test]
    fn ownership_requires_exact_identifier_and_both_uuids() {
        let mut owned = profile();
        assert!(valid_owned(&owned));
        owned.identifier = "com.other.camera".into();
        assert!(!valid_owned(&owned));
        owned = profile();
        owned.payload_uuid = "</string><true/>".into();
        assert!(profile_xml(&owned).is_err());
    }
    #[test]
    fn receipt_and_partial_inventory_do_not_establish_allowed_or_blocked() {
        let mut metadata = CameraProfile {
            state: "unknown".into(),
            owned_installed: false,
            complete_inventory: false,
            detail: "partial scope".into(),
            authorization: "not_requested".into(),
        };
        assert!(established_state(&metadata).is_err());
        metadata.state = "allowed".into();
        assert!(established_state(&metadata).is_err());
        metadata.complete_inventory = true;
        assert_eq!(
            established_state(&metadata).unwrap(),
            CameraPrivacyState::Allowed
        );
        metadata.state = "blocked".into();
        assert!(established_state(&metadata).is_err());
        metadata.owned_installed = true;
        assert_eq!(
            established_state(&metadata).unwrap(),
            CameraPrivacyState::Blocked
        );
        metadata.state = "system_managed".into();
        assert_eq!(
            established_state(&metadata).unwrap(),
            CameraPrivacyState::SystemManaged
        );
    }

    #[test]
    fn inspection_scope_and_authorization_do_not_manufacture_camera_permission() {
        let profile = |state: &str, complete, authorization: &str| CameraProfile {
            state: state.into(),
            owned_installed: false,
            complete_inventory: complete,
            detail: "profile metadata only".into(),
            authorization: authorization.into(),
        };
        let partial = inspection(profile("unknown", false, "not_requested")).unwrap();
        assert_eq!(partial.state, "unknown");
        assert!(partial.administrator_required);
        let cancelled = inspection(profile("unknown", false, "cancelled")).unwrap();
        assert_eq!(cancelled.state, "unknown");
        assert_eq!(
            cancelled.authorization,
            CameraInspectionAuthorization::Cancelled
        );
        assert!(!cancelled.complete_inventory);
        assert!(inspection(profile("allowed", false, "granted")).is_err());
        assert!(inspection(profile("allowed", true, "cancelled")).is_err());
        assert!(inspection(profile("system_managed", true, "denied")).is_err());
        assert!(inspection(profile("unknown", true, "granted")).is_err());
        let denied = inspection(profile("unknown", false, "denied")).unwrap();
        assert_eq!(denied.authorization, CameraInspectionAuthorization::Denied);
        assert!(denied.administrator_required);
        let restricted = inspection(profile("system_managed", true, "granted")).unwrap();
        assert_eq!(restricted.state, "system_managed");
        assert!(!restricted.administrator_required);
        assert!(inspection(profile("blocked", true, "granted")).is_err());
        assert!(inspection(profile("unknown", false, "unexpected")).is_err());
    }
}
