use super::{PlatformMonitor, native_request};
use crate::model::MicrophoneMuteState;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    os::fd::AsRawFd,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub(crate) struct ControlIdentity {
    pub uid: String,
    pub selector: u32,
    pub scope: u32,
    pub element: u32,
}
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct MuteControl {
    pub identity: ControlIdentity,
    pub name: String,
    pub muted: bool,
    pub writable: bool,
}
#[derive(Debug, Deserialize)]
pub(crate) struct UnsupportedInput {
    pub uid: String,
    pub name: String,
    pub reason: String,
}
#[derive(Debug, Deserialize)]
pub(crate) struct ControlResult {
    pub identity: ControlIdentity,
    pub ok: bool,
    pub muted: Option<bool>,
    pub error: Option<String>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CameraProfile {
    pub state: String,
    pub owned_installed: bool,
    pub complete_inventory: bool,
    pub detail: String,
}
#[derive(Debug, Deserialize)]
pub(crate) struct MacControlReply {
    pub ok: bool,
    pub error: Option<String>,
    pub controls: Vec<MuteControl>,
    pub unsupported: Vec<UnsupportedInput>,
    pub results: Vec<ControlResult>,
    pub camera: Option<CameraProfile>,
}
#[derive(Deserialize)]
struct Envelope {
    control: MacControlReply,
}
#[derive(Serialize)]
pub(crate) struct Request<'a> {
    pub operation: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changes: Option<&'a [Change]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identifier: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<&'a str>,
}
impl<'a> Request<'a> {
    pub fn new(operation: &'a str) -> Self {
        Self {
            operation,
            changes: None,
            identifier: None,
            uuid: None,
            path: None,
        }
    }
}
#[derive(Serialize)]
pub(crate) struct Change {
    identity: ControlIdentity,
    muted: bool,
}
pub(crate) fn request(request: &Request<'_>, timeout: Duration) -> Result<MacControlReply> {
    let json = serde_json::to_string(request)?;
    let envelope: Envelope = native_request("control", &[&json], timeout)?;
    Ok(envelope.control)
}
fn inventory() -> Result<MacControlReply> {
    let reply = request(&Request::new("inventory"), Duration::from_secs(10))?;
    if !reply.ok {
        bail!(
            "{}",
            reply
                .error
                .as_deref()
                .unwrap_or("CoreAudio control inventory failed")
        );
    }
    Ok(reply)
}

// This lock spans journal reads, native readback, and atomic journal replacement.
// O_NOFOLLOW and private modes prevent symlink-based writes to unrelated files.
pub(crate) struct OperationLock {
    file: File,
    pub directory: PathBuf,
}
impl OperationLock {
    pub fn acquire(name: &str) -> Result<Self> {
        let directory = crate::settings::data_dir()?.join("macos-controls");
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true).mode(0o700);
        builder.create(&directory)?;
        let metadata = fs::symlink_metadata(&directory)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            bail!("control state directory is not a real directory");
        }
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(directory.join(format!("{name}.lock")))?;
        loop {
            if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) } == 0 {
                break;
            }
            let error = std::io::Error::last_os_error();
            if error.kind() != std::io::ErrorKind::Interrupted {
                return Err(error.into());
            }
        }
        Ok(Self { file, directory })
    }
}
impl Drop for OperationLock {
    fn drop(&mut self) {
        unsafe {
            libc::flock(self.file.as_raw_fd(), libc::LOCK_UN);
        }
    }
}
pub(crate) fn atomic_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let temp = path.with_extension(format!("tmp-{}", std::process::id()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&temp)?;
        serde_json::to_writer(&mut file, value)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        fs::rename(&temp, path)?;
        File::open(path.parent().context("journal has no parent")?)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}
#[derive(Clone, Debug, Deserialize, Serialize)]
struct Original {
    identity: ControlIdentity,
    name: String,
    muted: bool,
}
#[derive(Default, Debug, Deserialize, Serialize)]
struct Journal {
    version: u32,
    lock_active: bool,
    restore_pending: bool,
    originals: Vec<Original>,
}
fn load(path: &Path) -> Result<Journal> {
    match fs::read(path) {
        Ok(bytes) => {
            let journal: Journal = serde_json::from_slice(&bytes).context(
                "invalid microphone restoration journal; refusing to overwrite original state",
            )?;
            if journal.version != 1 {
                bail!("unsupported microphone restoration journal version");
            }
            Ok(journal)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Journal {
            version: 1,
            ..Journal::default()
        }),
        Err(error) => Err(error.into()),
    }
}
fn remember(journal: &mut Journal, controls: &[MuteControl]) {
    for control in controls.iter().filter(|c| c.writable) {
        if !journal
            .originals
            .iter()
            .any(|o| o.identity == control.identity)
        {
            journal.originals.push(Original {
                identity: control.identity.clone(),
                name: control.name.clone(),
                muted: control.muted,
            });
        }
    }
}
fn reconcile_restore(
    journal: &mut Journal,
    changes: &[Change],
    results: &[ControlResult],
) -> Vec<String> {
    let mut errors = Vec::new();
    journal.originals.retain(|original| {
        let expected = changes.iter().find(|c| c.identity == original.identity);
        let result = results.iter().find(|r| r.identity == original.identity);
        if let (Some(expected), Some(result)) = (expected, result) {
            if result.ok && result.muted == Some(expected.muted) {
                return false;
            }
            errors.push(format!(
                "{} channel {}: {}",
                original.name,
                original.identity.element,
                result
                    .error
                    .as_deref()
                    .unwrap_or("mute restoration readback failed")
            ));
        } else {
            errors.push(format!(
                "{} channel {}: no restoration readback; original retained",
                original.name, original.identity.element
            ));
        }
        true
    });
    errors
}
fn restore(journal: &mut Journal, path: &Path) -> Result<usize> {
    if journal.originals.is_empty() {
        journal.restore_pending = false;
        atomic_json(path, journal)?;
        return Ok(0);
    }
    journal.restore_pending = true;
    atomic_json(path, journal)?;
    // Resolve UID/address in the helper on every write, including reconnects.
    // Original true values are restored true, never blindly unmuted.
    let changes: Vec<_> = journal
        .originals
        .iter()
        .map(|o| Change {
            identity: o.identity.clone(),
            muted: o.muted,
        })
        .collect();
    let mut req = Request::new("set");
    req.changes = Some(&changes);
    let reply = request(&req, Duration::from_secs(30))?;
    let before = journal.originals.len();
    let errors = reconcile_restore(journal, &changes, &reply.results);
    journal.restore_pending = !journal.originals.is_empty();
    atomic_json(path, journal)?;
    if !errors.is_empty() {
        bail!(
            "Partial microphone restoration; originals retained for retry: {}",
            errors.join("; ")
        );
    }
    Ok(before)
}
fn mute(journal: &mut Journal, path: &Path) -> Result<usize> {
    let reply = inventory()?;
    let writable: Vec<_> = reply.controls.into_iter().filter(|c| c.writable).collect();
    if writable.is_empty() {
        bail!(
            "No writable device INPUT mute controls: {}",
            reply
                .unsupported
                .iter()
                .map(|d| format!("{}: {}", d.name, d.reason))
                .collect::<Vec<_>>()
                .join("; ")
        );
    }
    remember(journal, &writable);
    journal.restore_pending = false;
    // Commit all original per-control state BEFORE the first possible side effect.
    atomic_json(path, journal)?;
    let changes: Vec<_> = writable
        .iter()
        .map(|c| Change {
            identity: c.identity.clone(),
            muted: true,
        })
        .collect();
    let mut req = Request::new("set");
    req.changes = Some(&changes);
    let reply = request(&req, Duration::from_secs(30))?;
    let mut errors = Vec::new();
    let mut changed = 0;
    for (control, change) in writable.iter().zip(&changes) {
        match reply.results.iter().find(|r| r.identity == change.identity) {
            Some(result) if result.ok && result.muted == Some(true) => {
                if !control.muted {
                    changed += 1;
                }
            }
            result => errors.push(format!(
                "{} channel {}: {}",
                control.name,
                control.identity.element,
                result
                    .and_then(|r| r.error.as_deref())
                    .unwrap_or("missing mute readback")
            )),
        }
    }
    if !errors.is_empty() {
        bail!(
            "Partial input mute; original states retained: {}",
            errors.join("; ")
        );
    }
    Ok(changed)
}
impl PlatformMonitor {
    pub fn microphone_mute_state(&self) -> Result<MicrophoneMuteState> {
        let reply = inventory()?;
        let mut controls = reply.controls.iter().filter(|c| c.writable);
        let Some(first) = controls.next() else {
            return Ok(MicrophoneMuteState::Unavailable);
        };
        if controls.any(|c| c.muted != first.muted) {
            return Ok(MicrophoneMuteState::Mixed);
        }
        Ok(if first.muted {
            MicrophoneMuteState::Muted
        } else {
            MicrophoneMuteState::Unmuted
        })
    }
    pub fn microphone_control_detail(&self) -> Result<String> {
        let reply = inventory()?;
        let supported = reply.controls.iter().filter(|c| c.writable).count();
        let unsupported = reply
            .unsupported
            .iter()
            .map(|d| format!("{} [{}]: {}", d.name, d.uid, d.reason))
            .collect::<Vec<_>>();
        Ok(format!(
            "{supported} writable device INPUT mute controls; {} unsupported/degraded inputs: {}. No universal microphone permission or physical cutoff is asserted.",
            unsupported.len(),
            unsupported.join("; ")
        ))
    }
    pub fn set_microphone_mute(&self, muted: bool) -> Result<usize> {
        let lock = OperationLock::acquire("microphone")?;
        let path = lock.directory.join("microphone.json");
        let mut journal = load(&path)?;
        // Manual intent wins even if the subsequent device operation fails.
        journal.lock_active = false;
        atomic_json(&path, &journal)?;
        if !muted
            && journal.originals.is_empty()
            && matches!(
                self.microphone_mute_state()?,
                MicrophoneMuteState::Muted | MicrophoneMuteState::Mixed
            )
        {
            bail!(
                "No MicCamWatch original state is recorded; unrelated pre-muted inputs will not be unmuted"
            );
        }
        if muted {
            mute(&mut journal, &path)
        } else {
            restore(&mut journal, &path)
        }
    }
    pub fn toggle_microphone_mute(&self) -> Result<bool> {
        let lock = OperationLock::acquire("microphone")?;
        let path = lock.directory.join("microphone.json");
        let mut journal = load(&path)?;
        let muted = self.microphone_mute_state()?;
        if muted == MicrophoneMuteState::Unavailable {
            bail!("No writable device INPUT mute controls");
        }
        let next = muted != MicrophoneMuteState::Muted;
        journal.lock_active = false;
        atomic_json(&path, &journal)?;
        if next {
            mute(&mut journal, &path)?;
        } else {
            restore(&mut journal, &path)?;
        }
        let actual = self.microphone_mute_state()?;
        if actual
            != if next {
                MicrophoneMuteState::Muted
            } else {
                MicrophoneMuteState::Unmuted
            }
        {
            bail!(
                "Original input states were restored, but the requested aggregate state is not established; unrelated pre-muted controls remain untouched"
            );
        }
        Ok(next)
    }
    pub fn begin_lock_mute(&self) -> Result<()> {
        let lock = OperationLock::acquire("microphone")?;
        let path = lock.directory.join("microphone.json");
        let mut journal = load(&path)?;
        // Never make an existing manual mute belong to a later lock cycle.
        if !journal.lock_active && !journal.originals.is_empty() && !journal.restore_pending {
            return Ok(());
        }
        if journal.restore_pending {
            restore(&mut journal, &path)?;
        }
        journal.lock_active = true;
        atomic_json(&path, &journal)?;
        mute(&mut journal, &path)?;
        Ok(())
    }
    pub fn restore_lock_mute(&self) -> Result<()> {
        let lock = OperationLock::acquire("microphone")?;
        let path = lock.directory.join("microphone.json");
        let mut journal = load(&path)?;
        if !journal.lock_active {
            return Ok(());
        }
        restore(&mut journal, &path)?;
        journal.lock_active = false;
        atomic_json(&path, &journal)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn identity(uid: &str, element: u32) -> ControlIdentity {
        ControlIdentity {
            uid: uid.into(),
            selector: 0x6d757465,
            scope: 0x696e7074,
            element,
        }
    }
    #[test]
    fn reconnect_and_multiple_channels_preserve_first_original() {
        let mut journal = Journal::default();
        let controls = vec![
            MuteControl {
                identity: identity("usb", 0),
                name: "USB".into(),
                muted: false,
                writable: true,
            },
            MuteControl {
                identity: identity("usb", 1),
                name: "USB".into(),
                muted: true,
                writable: true,
            },
        ];
        remember(&mut journal, &controls);
        let reconnected = vec![MuteControl {
            identity: identity("usb", 0),
            name: "Renamed".into(),
            muted: true,
            writable: true,
        }];
        remember(&mut journal, &reconnected);
        assert_eq!(journal.originals.len(), 2);
        assert!(!journal.originals[0].muted);
        assert!(journal.originals[1].muted);
    }
    #[test]
    fn partial_restore_retains_disconnected_and_failed_controls_only() {
        let mut journal = Journal {
            originals: vec![
                Original {
                    identity: identity("a", 0),
                    name: "A".into(),
                    muted: false,
                },
                Original {
                    identity: identity("b", 0),
                    name: "B".into(),
                    muted: true,
                },
                Original {
                    identity: identity("c", 1),
                    name: "C".into(),
                    muted: false,
                },
            ],
            ..Journal::default()
        };
        let changes = journal
            .originals
            .iter()
            .map(|o| Change {
                identity: o.identity.clone(),
                muted: o.muted,
            })
            .collect::<Vec<_>>();
        let results = vec![
            ControlResult {
                identity: identity("a", 0),
                ok: true,
                muted: Some(false),
                error: None,
            },
            ControlResult {
                identity: identity("b", 0),
                ok: false,
                muted: None,
                error: Some("disconnected".into()),
            },
            ControlResult {
                identity: identity("c", 1),
                ok: true,
                muted: Some(true),
                error: None,
            },
        ];
        assert_eq!(reconcile_restore(&mut journal, &changes, &results).len(), 2);
        assert_eq!(
            journal
                .originals
                .iter()
                .map(|o| o.identity.uid.as_str())
                .collect::<Vec<_>>(),
            ["b", "c"]
        );
        assert!(journal.originals[0].muted);
    }
}
