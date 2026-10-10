//! Lock policy owns only verified per-endpoint changes, never an aggregate mute value.
use super::{PlatformMonitor, device_id};
use anyhow::{Context, Result};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
    mpsc::SyncSender,
};
use windows::{
    Win32::{
        Foundation::PROPERTYKEY,
        Media::Audio::{
            AUDIO_VOLUME_NOTIFICATION_DATA, DEVICE_STATE, DEVICE_STATE_ACTIVE, EDataFlow, ERole,
            Endpoints::{
                IAudioEndpointVolume, IAudioEndpointVolumeCallback,
                IAudioEndpointVolumeCallback_Impl,
            },
            IMMDevice, IMMDeviceEnumerator, IMMNotificationClient, IMMNotificationClient_Impl,
            eCapture,
        },
        System::Com::{CLSCTX_ALL, CoCreateGuid},
    },
    core::{GUID, PCWSTR},
};

const MAX_ENDPOINTS: u32 = 256;

trait Endpoint {
    fn id(&self) -> &str;
    fn revision(&self) -> u64;
    fn current(&self) -> Result<bool>;
    fn read(&self) -> Result<bool>;
    fn write(&self, muted: bool) -> Result<()>;
}

struct Owned<E> {
    endpoint: E,
    original: bool,
    applied: bool,
    revision: u64,
}

struct Journal<E> {
    entries: Vec<Owned<E>>,
}
impl<E> Default for Journal<E> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}

impl<E: Endpoint> Journal<E> {
    fn apply(&mut self, endpoints: Vec<E>) -> Result<usize> {
        anyhow::ensure!(
            self.entries.is_empty(),
            "previous microphone restore is unresolved"
        );
        let mut changed = 0;
        for endpoint in endpoints {
            let revision = endpoint.revision();
            let attempt = (|| -> Result<()> {
                anyhow::ensure!(
                    endpoint.current()?,
                    "{}: endpoint generation changed",
                    endpoint.id()
                );
                let original = endpoint.read()?;
                // An already-muted endpoint is not ours. Never unmute it later.
                if original {
                    return Ok(());
                }
                anyhow::ensure!(
                    endpoint.revision() == revision,
                    "{}: external change during lock",
                    endpoint.id()
                );
                endpoint.write(true).with_context(|| format!("{}: lock mute write failed; ownership not acquired, endpoint state unknown", endpoint.id()))?;
                // Retain the original even when readback fails after a successful write.
                self.entries.push(Owned {
                    endpoint,
                    original,
                    applied: true,
                    revision,
                });
                let entry = self.entries.last().unwrap();
                anyhow::ensure!(
                    entry.endpoint.read()? == entry.applied,
                    "{}: mute readback mismatch",
                    entry.endpoint.id()
                );
                anyhow::ensure!(
                    entry.endpoint.revision() == revision,
                    "{}: external change during mute",
                    entry.endpoint.id()
                );
                changed += 1;
                Ok(())
            })();
            if let Err(error) = attempt {
                let rollback = self.restore();
                return match rollback {
                    Ok(_) => Err(error.context("lock mute failed; owned changes rolled back")),
                    Err(rollback) => Err(anyhow::anyhow!(
                        "lock mute failed: {error:#}; partial rollback: {rollback:#}"
                    )),
                };
            }
        }
        Ok(changed)
    }

    fn restore(&mut self) -> Result<usize> {
        let mut restored = 0;
        let mut errors = Vec::new();
        self.entries.retain(|entry| {
            let mut relinquished = false;
            let attempt = (|| -> Result<bool> {
                // Callback intent invalidates ownership even when the value is unchanged.
                if entry.endpoint.revision() != entry.revision {
                    relinquished = true;
                    anyhow::bail!("{}: restore relinquished after external/manual or topology change", entry.endpoint.id());
                }
                if !entry.endpoint.current()? {
                    relinquished = true;
                    anyhow::bail!("{}: restore relinquished permanently after retained endpoint disconnect/generation change", entry.endpoint.id());
                }
                let actual = entry.endpoint.read()?;
                if actual != entry.applied {
                    relinquished = true;
                    anyhow::bail!("{}: restore relinquished after mute readback changed", entry.endpoint.id());
                }
                anyhow::ensure!(entry.endpoint.revision() == entry.revision, "{}: external change during restore", entry.endpoint.id());
                entry.endpoint.write(entry.original)?;
                anyhow::ensure!(entry.endpoint.read()? == entry.original, "{}: restore readback mismatch", entry.endpoint.id());
                anyhow::ensure!(entry.endpoint.revision() == entry.revision, "{}: restore ownership lost during readback; no retry", entry.endpoint.id());
                restored += 1;
                Ok(false)
            })();
            match attempt {
                Ok(keep) => keep,
                Err(error) => {
                    errors.push(format!("{error:#}"));
                    // Newer intent ends ownership; real read/write failures retain originals.
                    !relinquished && entry.endpoint.revision() == entry.revision
                }
            }
        });
        if errors.is_empty() {
            Ok(restored)
        } else {
            anyhow::bail!(
                "microphone restore partial ({restored} restored, {} retained): {}",
                self.entries.len(),
                errors.join("; ")
            )
        }
    }
}

#[windows::core::implement(IAudioEndpointVolumeCallback)]
struct VolumeCallback {
    context: GUID,
    revision: Arc<AtomicU64>,
    wake: Option<SyncSender<()>>,
}
impl IAudioEndpointVolumeCallback_Impl for VolumeCallback_Impl {
    fn OnNotify(&self, data: *mut AUDIO_VOLUME_NOTIFICATION_DATA) -> windows::core::Result<()> {
        if unsafe { data.as_ref() }.is_none_or(|data| data.guidEventContext != self.context) {
            self.revision.fetch_add(1, Ordering::AcqRel);
            if let Some(wake) = &self.wake {
                let _ = wake.try_send(());
            }
        }
        Ok(())
    }
}

#[windows::core::implement(IMMNotificationClient)]
struct TopologyCallback {
    generation: Arc<AtomicU64>,
    wake: Option<SyncSender<()>>,
}
impl IMMNotificationClient_Impl for TopologyCallback_Impl {
    fn OnDeviceStateChanged(&self, _: &PCWSTR, _: DEVICE_STATE) -> windows::core::Result<()> {
        self.generation.fetch_add(1, Ordering::AcqRel);
        if let Some(wake) = &self.wake {
            let _ = wake.try_send(());
        }
        Ok(())
    }
    fn OnDeviceAdded(&self, _: &PCWSTR) -> windows::core::Result<()> {
        self.generation.fetch_add(1, Ordering::AcqRel);
        if let Some(wake) = &self.wake {
            let _ = wake.try_send(());
        }
        Ok(())
    }
    fn OnDeviceRemoved(&self, _: &PCWSTR) -> windows::core::Result<()> {
        self.generation.fetch_add(1, Ordering::AcqRel);
        if let Some(wake) = &self.wake {
            let _ = wake.try_send(());
        }
        Ok(())
    }
    fn OnDefaultDeviceChanged(
        &self,
        _: EDataFlow,
        _: ERole,
        _: &PCWSTR,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn OnPropertyValueChanged(&self, _: &PCWSTR, _: &PROPERTYKEY) -> windows::core::Result<()> {
        Ok(())
    }
}

struct TopologyGuard {
    enumerator: IMMDeviceEnumerator,
    callback: IMMNotificationClient,
}
impl Drop for TopologyGuard {
    fn drop(&mut self) {
        if let Err(error) = unsafe {
            self.enumerator
                .UnregisterEndpointNotificationCallback(&self.callback)
        } {
            eprintln!("microphone topology callback unregister failed: {error}");
        }
    }
}

struct NativeEndpoint {
    id: String,
    device: IMMDevice,
    volume: IAudioEndpointVolume,
    callback: IAudioEndpointVolumeCallback,
    context: GUID,
    revision: Arc<AtomicU64>,
    topology: Arc<AtomicU64>,
    generation: u64,
}
impl Endpoint for NativeEndpoint {
    fn id(&self) -> &str {
        &self.id
    }
    fn revision(&self) -> u64 {
        // Topology invalidates all owned endpoints conservatively, not only replacements.
        if self.topology.load(Ordering::Acquire) != self.generation {
            u64::MAX
        } else {
            self.revision.load(Ordering::Acquire)
        }
    }
    fn current(&self) -> Result<bool> {
        Ok(self.topology.load(Ordering::Acquire) == self.generation
            && unsafe { self.device.GetState()? } == DEVICE_STATE_ACTIVE
            && device_id(&self.device)? == self.id)
    }
    fn read(&self) -> Result<bool> {
        Ok(unsafe { self.volume.GetMute()? }.as_bool())
    }
    fn write(&self, muted: bool) -> Result<()> {
        unsafe { self.volume.SetMute(muted, &self.context)? };
        Ok(())
    }
}
impl Drop for NativeEndpoint {
    fn drop(&mut self) {
        if let Err(error) = unsafe { self.volume.UnregisterControlChangeNotify(&self.callback) } {
            eprintln!(
                "microphone ownership callback unregister failed for {}: {error}",
                self.id
            );
        }
    }
}

#[derive(Default)]
pub(super) struct NativeLockJournal {
    journal: Journal<NativeEndpoint>,
    topology: Option<TopologyGuard>,
}
impl NativeLockJournal {
    pub(super) fn is_empty(&self) -> bool {
        self.journal.entries.is_empty()
    }
    pub(crate) fn invalidate_manual(&mut self) {
        self.journal.entries.clear();
        self.topology = None;
    }
}

impl PlatformMonitor {
    pub(super) fn native_mute_for_lock(&self, journal: &mut NativeLockJournal) -> Result<usize> {
        anyhow::ensure!(
            journal.journal.entries.is_empty(),
            "previous microphone restore is unresolved"
        );
        let generation = Arc::new(AtomicU64::new(0));
        let callback: IMMNotificationClient = TopologyCallback {
            generation: Arc::clone(&generation),
            wake: None,
        }
        .into();
        unsafe {
            self.enumerator
                .RegisterEndpointNotificationCallback(&callback)?
        };
        let topology = TopologyGuard {
            enumerator: self.enumerator.clone(),
            callback,
        };
        let baseline = generation.load(Ordering::Acquire);
        let context = unsafe { CoCreateGuid()? };
        let collection = unsafe {
            self.enumerator
                .EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE)?
        };
        let count = unsafe { collection.GetCount()? };
        anyhow::ensure!(count > 0, "no active microphone capture device was found");
        anyhow::ensure!(
            count <= MAX_ENDPOINTS,
            "microphone endpoint overload: {count} exceeds {MAX_ENDPOINTS}; no lock mute applied"
        );
        let mut endpoints = Vec::with_capacity(count as usize);
        for index in 0..count {
            let device = unsafe { collection.Item(index)? };
            let id = device_id(&device)?;
            let volume: IAudioEndpointVolume = unsafe { device.Activate(CLSCTX_ALL, None)? };
            let revision = Arc::new(AtomicU64::new(0));
            let callback: IAudioEndpointVolumeCallback = VolumeCallback {
                context,
                revision: Arc::clone(&revision),
                wake: None,
            }
            .into();
            unsafe { volume.RegisterControlChangeNotify(&callback) }
                .context("register microphone ownership callback")?;
            endpoints.push(NativeEndpoint {
                id,
                device,
                volume,
                callback,
                context,
                revision,
                topology: Arc::clone(&generation),
                generation: baseline,
            });
        }
        journal.topology = Some(topology);
        let result = journal.journal.apply(endpoints);
        if journal.journal.entries.is_empty() {
            journal.topology = None;
        }
        result
    }
    pub(super) fn native_restore_after_lock(
        &self,
        journal: &mut NativeLockJournal,
    ) -> Result<usize> {
        let result = journal.journal.restore();
        if journal.journal.entries.is_empty() {
            journal.topology = None;
        }
        result
    }
}

/// Manual protection deliberately does not revoke intent on foreign callbacks.
fn protection_pass<E: Endpoint>(endpoints: &[E], muted: bool) -> (usize, Vec<String>) {
    let mut changed = 0;
    let mut errors = Vec::new();
    for endpoint in endpoints {
        let attempt = (|| -> Result<()> {
            anyhow::ensure!(
                endpoint.current()?,
                "{}: stale protection endpoint",
                endpoint.id()
            );
            if endpoint.read()? != muted {
                endpoint.write(muted)?;
                anyhow::ensure!(
                    endpoint.read()? == muted,
                    "{}: protection readback mismatch",
                    endpoint.id()
                );
                changed += 1;
            }
            Ok(())
        })();
        if let Err(error) = attempt {
            errors.push(format!("{error:#}"));
        }
    }
    (changed, errors)
}

fn refresh_registrations<E>(
    generation: u64,
    baseline: &mut Option<u64>,
    entries: &mut Vec<E>,
    errors: &mut Vec<String>,
    load: impl FnOnce(&mut Vec<E>, &mut Vec<String>) -> Result<()>,
) -> Result<()> {
    if *baseline == Some(generation) {
        return Ok(());
    }
    // Invalidate first. A failure after a previous partial pass must never cache an empty success.
    *baseline = None;
    entries.clear();
    errors.clear();
    if let Err(error) = load(entries, errors) {
        errors.push(format!("{error:#}"));
        return Err(error);
    }
    anyhow::ensure!(
        errors.is_empty(),
        "microphone registration partial: {}",
        errors.join("; ")
    );
    *baseline = Some(generation);
    Ok(())
}

/// Registrations are retained on the service's MTA and dropped before its COM guard.
pub(super) struct GuardEndpoints {
    entries: Vec<NativeEndpoint>,
    topology: TopologyGuard,
    generation: Arc<AtomicU64>,
    baseline: Option<u64>,
    wake: SyncSender<()>,
    errors: Vec<String>,
}
impl GuardEndpoints {
    pub(super) fn new(monitor: &PlatformMonitor, wake: SyncSender<()>) -> Result<Self> {
        let generation = Arc::new(AtomicU64::new(0));
        let callback: IMMNotificationClient = TopologyCallback {
            generation: Arc::clone(&generation),
            wake: Some(wake.clone()),
        }
        .into();
        unsafe {
            monitor
                .enumerator
                .RegisterEndpointNotificationCallback(&callback)?;
        }
        let guard = Self {
            entries: Vec::new(),
            topology: TopologyGuard {
                enumerator: monitor.enumerator.clone(),
                callback,
            },
            generation,
            baseline: None,
            wake,
            errors: Vec::new(),
        };
        Ok(guard)
    }
    pub(super) fn refresh(&mut self) -> Result<()> {
        let generation = self.generation.load(Ordering::Acquire);
        refresh_registrations(
            generation,
            &mut self.baseline,
            &mut self.entries,
            &mut self.errors,
            |entries, errors| {
                let collection = unsafe {
                    self.topology
                        .enumerator
                        .EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE)?
                };
                let count = unsafe { collection.GetCount()? };
                anyhow::ensure!(
                    count <= MAX_ENDPOINTS,
                    "microphone endpoint overload: {count}"
                );
                let context = unsafe { CoCreateGuid()? };
                for index in 0..count {
                    let attempt = (|| -> Result<NativeEndpoint> {
                        let device = unsafe { collection.Item(index)? };
                        let id = device_id(&device)?;
                        let volume: IAudioEndpointVolume =
                            unsafe { device.Activate(CLSCTX_ALL, None)? };
                        let revision = Arc::new(AtomicU64::new(0));
                        let callback: IAudioEndpointVolumeCallback = VolumeCallback {
                            context,
                            revision: Arc::clone(&revision),
                            wake: Some(self.wake.clone()),
                        }
                        .into();
                        unsafe {
                            volume.RegisterControlChangeNotify(&callback)?;
                        }
                        Ok(NativeEndpoint {
                            id,
                            device,
                            volume,
                            callback,
                            context,
                            revision,
                            topology: Arc::clone(&self.generation),
                            generation,
                        })
                    })();
                    match attempt {
                        Ok(endpoint) => entries.push(endpoint),
                        Err(error) => errors.push(format!("endpoint {index}: {error:#}")),
                    }
                }
                Ok(())
            },
        )
    }
    pub(super) fn enforce(&mut self) -> (usize, Result<()>) {
        let refresh_error = self.refresh().err();
        let (changed, mut errors) = protection_pass(&self.entries, true);
        if let Some(error) = refresh_error {
            errors.push(format!("{error:#}"));
        }
        let result = if errors.is_empty() {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "microphone protection partial: {}",
                errors.join("; ")
            ))
        };
        (changed, result)
    }
    pub(super) fn explicit_release(&mut self) -> Result<usize> {
        // The caller has already durably invalidated desired protection under the writer lock.
        let refresh_error = self.refresh().err();
        let (changed, mut errors) = protection_pass(&self.entries, false);
        if let Some(error) = refresh_error {
            errors.push(format!("{error:#}"));
        }
        anyhow::ensure!(
            errors.is_empty(),
            "microphone release partial: {}",
            errors.join("; ")
        );
        Ok(changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn endpoint_callback_ignores_own_context_but_revokes_same_valued_foreign_intent() {
        let context = GUID::from_u128(1);
        let revision = Arc::new(AtomicU64::new(0));
        let callback: IAudioEndpointVolumeCallback = VolumeCallback {
            context,
            revision: Arc::clone(&revision),
            wake: None,
        }
        .into();
        let mut data = AUDIO_VOLUME_NOTIFICATION_DATA {
            guidEventContext: context,
            bMuted: true.into(),
            ..Default::default()
        };
        unsafe { callback.OnNotify(&mut data) }.unwrap();
        assert_eq!(revision.load(Ordering::Acquire), 0);
        data.guidEventContext = GUID::from_u128(2);
        unsafe { callback.OnNotify(&mut data) }.unwrap();
        assert_eq!(revision.load(Ordering::Acquire), 1);
        unsafe { callback.OnNotify(&mut data) }.unwrap();
        assert_eq!(revision.load(Ordering::Acquire), 2);
    }

    #[test]
    fn topology_callback_changes_generation_even_for_reused_device_id() {
        let generation = Arc::new(AtomicU64::new(0));
        let callback: IMMNotificationClient = TopologyCallback {
            generation: Arc::clone(&generation),
            wake: None,
        }
        .into();
        let id: Vec<u16> = "same-device".encode_utf16().chain(Some(0)).collect();
        unsafe { callback.OnDeviceRemoved(PCWSTR(id.as_ptr())) }.unwrap();
        unsafe { callback.OnDeviceAdded(PCWSTR(id.as_ptr())) }.unwrap();
        assert_eq!(generation.load(Ordering::Acquire), 2);
    }
    struct Fake {
        id: &'static str,
        muted: Cell<bool>,
        revision: Cell<u64>,
        connected: Cell<bool>,
        fail: Cell<bool>,
        writes: Cell<usize>,
        fail_at: Cell<Option<usize>>,
        change_during_write: Cell<bool>,
        read_failure: Cell<bool>,
        current_failure: Cell<bool>,
    }
    impl Endpoint for Rc<Fake> {
        fn id(&self) -> &str {
            self.id
        }
        fn revision(&self) -> u64 {
            self.revision.get()
        }
        fn current(&self) -> Result<bool> {
            anyhow::ensure!(
                !self.current_failure.get(),
                "injected endpoint identity/state read failure"
            );
            Ok(self.connected.get())
        }
        fn read(&self) -> Result<bool> {
            anyhow::ensure!(!self.read_failure.get(), "injected read failure");
            Ok(self.muted.get())
        }
        fn write(&self, value: bool) -> Result<()> {
            let count = self.writes.get() + 1;
            self.writes.set(count);
            anyhow::ensure!(
                !self.fail.get() && self.fail_at.get() != Some(count),
                "injected write failure"
            );
            self.muted.set(value);
            if self.change_during_write.get() {
                self.revision.set(self.revision.get() + 1);
            }
            Ok(())
        }
    }
    fn fake(id: &'static str, muted: bool) -> Rc<Fake> {
        Rc::new(Fake {
            id,
            muted: Cell::new(muted),
            revision: Cell::new(0),
            connected: Cell::new(true),
            fail: Cell::new(false),
            writes: Cell::new(0),
            fail_at: Cell::new(None),
            change_during_write: Cell::new(false),
            read_failure: Cell::new(false),
            current_failure: Cell::new(false),
        })
    }
    fn journal() -> Journal<Rc<Fake>> {
        Journal {
            entries: Vec::new(),
        }
    }
    #[test]
    fn persistent_pass_corrects_foreign_false_and_hotplug_without_revoking_intent() {
        let initial_muted = fake("initial-muted", true);
        let foreign_unmuted = fake("foreign-unmuted", false);
        foreign_unmuted.revision.set(9);
        let endpoints = [initial_muted.clone(), foreign_unmuted.clone()];
        let (changed, errors) = protection_pass(&endpoints, true);
        assert_eq!(changed, 1);
        assert!(errors.is_empty());
        assert_eq!(initial_muted.writes.get(), 0);
        foreign_unmuted.muted.set(false);
        foreign_unmuted.revision.set(10);
        let arriving = fake("hotplug", false);
        let (changed, errors) = protection_pass(&[foreign_unmuted.clone(), arriving.clone()], true);
        assert_eq!(changed, 2);
        assert!(errors.is_empty());
        assert!(foreign_unmuted.muted.get() && arriving.muted.get());
        let (released, errors) = protection_pass(
            &[
                initial_muted.clone(),
                foreign_unmuted.clone(),
                arriving.clone(),
            ],
            false,
        );
        assert_eq!(released, 3);
        assert!(errors.is_empty());
        assert!(!initial_muted.muted.get());
    }
    #[test]
    fn partial_guard_failure_does_not_leave_other_compatible_inputs_unprotected() {
        let failed = fake("failed", false);
        failed.read_failure.set(true);
        let compatible = fake("compatible", false);
        let (changed, errors) = protection_pass(&[failed.clone(), compatible.clone()], true);
        assert_eq!(changed, 1);
        assert_eq!(errors.len(), 1);
        assert!(compatible.muted.get());
        assert!(!failed.muted.get());
    }
    #[test]
    fn callback_wakes_are_coalesced_and_never_lose_revision_when_queue_is_full() {
        let context = GUID::from_u128(7);
        let revision = Arc::new(AtomicU64::new(0));
        let (wake, wakes) = std::sync::mpsc::sync_channel(1);
        let callback: IAudioEndpointVolumeCallback = VolumeCallback {
            context,
            revision: Arc::clone(&revision),
            wake: Some(wake),
        }
        .into();
        let mut data = AUDIO_VOLUME_NOTIFICATION_DATA {
            guidEventContext: GUID::from_u128(8),
            bMuted: false.into(),
            ..Default::default()
        };
        for _ in 0..100 {
            unsafe { callback.OnNotify(&mut data) }.unwrap();
        }
        assert_eq!(revision.load(Ordering::Acquire), 100);
        assert!(wakes.try_recv().is_ok());
        assert!(wakes.try_recv().is_err());
        data.guidEventContext = context;
        unsafe { callback.OnNotify(&mut data) }.unwrap();
        assert!(wakes.try_recv().is_err());
    }
    #[test]
    fn partial_registration_then_enumeration_failure_cannot_cache_empty_release_success() {
        let endpoint = fake("compatible", false);
        let mut baseline = None;
        let mut entries = Vec::new();
        let mut errors = Vec::new();
        assert!(
            refresh_registrations(
                3,
                &mut baseline,
                &mut entries,
                &mut errors,
                |entries, errors| {
                    entries.push(endpoint.clone());
                    errors.push("injected second endpoint callback registration failure".into());
                    Ok(())
                }
            )
            .is_err()
        );
        assert_eq!(baseline, None);
        assert_eq!(protection_pass(&entries, true).0, 1);
        assert!(endpoint.muted.get());
        assert!(
            refresh_registrations(3, &mut baseline, &mut entries, &mut errors, |_, _| {
                anyhow::bail!("injected transient enumeration failure after partial registration")
            })
            .is_err()
        );
        assert_eq!(baseline, None);
        assert!(!errors.is_empty());
        let loaded = Cell::new(false);
        assert!(
            refresh_registrations(3, &mut baseline, &mut entries, &mut errors, |entries, _| {
                loaded.set(true);
                entries.push(endpoint.clone());
                Ok(())
            })
            .is_ok()
        );
        assert!(
            loaded.get(),
            "same-generation retry must enumerate instead of caching zero inputs"
        );
        assert_eq!(baseline, Some(3));
        let (released, release_errors) = protection_pass(&entries, false);
        assert_eq!(released, 1);
        assert!(release_errors.is_empty());
        assert!(!endpoint.muted.get());
    }
    #[test]
    fn mixed_originals_restore_exactly() {
        let a = fake("muted", true);
        let b = fake("unmuted", false);
        let mut j = journal();
        assert_eq!(j.apply(vec![a.clone(), b.clone()]).unwrap(), 1);
        assert_eq!(j.restore().unwrap(), 1);
        assert!(a.muted.get());
        assert!(!b.muted.get());
    }
    #[test]
    fn same_valued_external_and_manual_intention_end_ownership() {
        for external_value in [true, false] {
            let a = fake("a", false);
            let mut j = journal();
            j.apply(vec![a.clone()]).unwrap();
            a.muted.set(external_value);
            a.revision.set(1);
            assert!(j.restore().is_err());
            assert_eq!(a.muted.get(), external_value);
            assert!(j.entries.is_empty());
        }
    }
    #[test]
    fn disconnected_retained_endpoint_never_targets_replacement() {
        let a = fake("same-id", false);
        let replacement = fake("same-id", true);
        let mut j = journal();
        j.apply(vec![a.clone()]).unwrap();
        a.connected.set(false);
        assert!(j.restore().is_err());
        assert!(replacement.muted.get());
        assert!(j.entries.is_empty());
        // Even without a topology callback or revision change, a reused object/ID
        // becoming active again cannot reacquire the relinquished permission.
        a.connected.set(true);
        assert_eq!(j.restore().unwrap(), 0);
        assert!(a.muted.get());
        assert_eq!(a.writes.get(), 1);
        assert!(replacement.muted.get());
    }
    #[test]
    fn partial_restore_retains_original_and_retries_only_owned_failures() {
        let a = fake("a", false);
        let b = fake("b", false);
        let mut j = journal();
        j.apply(vec![a.clone(), b.clone()]).unwrap();
        b.fail.set(true);
        assert!(j.restore().is_err());
        assert!(!a.muted.get());
        assert!(b.muted.get());
        assert_eq!(j.entries.len(), 1);
        b.fail.set(false);
        assert_eq!(j.restore().unwrap(), 1);
        assert!(!b.muted.get());
    }
    #[test]
    fn failed_apply_rolls_back_prior_success_without_muting_failed_endpoint() {
        let a = fake("a", false);
        let b = fake("b", false);
        b.fail.set(true);
        let mut j = journal();
        assert!(j.apply(vec![a.clone(), b.clone()]).is_err());
        assert!(!a.muted.get());
        assert!(!b.muted.get());
        assert!(j.entries.is_empty());
    }

    #[test]
    fn failed_rollback_retains_only_successfully_owned_originals() {
        let a = fake("a", false);
        let b = fake("b", false);
        a.fail_at.set(Some(2));
        b.fail.set(true);
        let mut j = journal();
        let error = j.apply(vec![a.clone(), b.clone()]).unwrap_err().to_string();
        assert!(error.contains("partial rollback"));
        assert!(a.muted.get());
        assert_eq!(j.entries.len(), 1);
        assert!(!j.entries[0].original);
        a.fail_at.set(None);
        assert_eq!(j.restore().unwrap(), 1);
        assert!(!a.muted.get());
    }

    #[test]
    fn foreign_callback_racing_with_apply_relinquishes_without_writeback() {
        let a = fake("a", false);
        a.change_during_write.set(true);
        let mut j = journal();
        assert!(j.apply(vec![a.clone()]).is_err());
        assert_eq!(a.writes.get(), 1);
        assert!(a.muted.get());
        assert!(j.entries.is_empty());
    }

    #[test]
    fn restore_read_failure_retains_original_without_mutation() {
        let a = fake("a", false);
        let mut j = journal();
        j.apply(vec![a.clone()]).unwrap();
        a.read_failure.set(true);
        assert!(j.restore().is_err());
        assert_eq!(a.writes.get(), 1);
        assert_eq!(j.entries.len(), 1);
        a.read_failure.set(false);
        assert_eq!(j.restore().unwrap(), 1);
        assert!(!a.muted.get());
    }

    #[test]
    fn unverifiable_endpoint_state_retains_original_but_never_mutates() {
        let a = fake("a", false);
        let mut j = journal();
        j.apply(vec![a.clone()]).unwrap();
        a.current_failure.set(true);
        assert!(j.restore().is_err());
        assert!(a.muted.get());
        assert_eq!(a.writes.get(), 1);
        assert_eq!(j.entries.len(), 1);
        // A later confirmed disconnection relinquishes the retained authority.
        a.current_failure.set(false);
        a.connected.set(false);
        assert!(j.restore().is_err());
        assert!(j.entries.is_empty());
        a.connected.set(true);
        assert_eq!(j.restore().unwrap(), 0);
        assert_eq!(a.writes.get(), 1);
    }
}
