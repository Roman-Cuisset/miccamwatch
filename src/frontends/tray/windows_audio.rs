use crate::{
    config::Policy,
    platform::{MicrophoneLockJournal, MicrophoneProtectionStatus, PlatformMonitor},
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicIsize, AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread,
};
use windows::Win32::{
    Foundation::{HWND, LPARAM, WPARAM},
    UI::WindowsAndMessaging::PostMessageW,
};

#[derive(Clone, Copy, Debug)]
pub(super) enum Request {
    Manual,
    #[cfg(feature = "flutter-desktop")]
    Set {
        ticket: u64,
        requested: bool,
        expected_requested: bool,
        expected_generation: u64,
    },
    Lock,
    Unlock,
}
impl Request {
    pub(super) fn is_manual(self) -> bool {
        match self {
            Self::Manual => true,
            Self::Lock | Self::Unlock => false,
            #[cfg(feature = "flutter-desktop")]
            Self::Set { .. } => true,
        }
    }
}
pub(super) struct Worker {
    pub requests: SyncSender<Request>,
    pub results: Receiver<(Request, u64, Result<MicrophoneProtectionStatus, String>)>,
    pub sequence: Arc<AtomicU64>,
}
impl Worker {
    pub fn start(policy: Policy, hwnd: Arc<AtomicIsize>) -> Self {
        Self::start_mode(policy, hwnd, false)
    }
    #[cfg(feature = "flutter-desktop")]
    pub(super) fn start_controlled(policy: Policy, hwnd: Arc<AtomicIsize>) -> Self {
        Self::start_mode(policy, hwnd, true)
    }
    fn start_mode(policy: Policy, hwnd: Arc<AtomicIsize>, controlled: bool) -> Self {
        #[cfg(not(feature = "flutter-desktop"))]
        let _ = controlled;
        let (requests, pending) = mpsc::sync_channel::<Request>(8);
        let (completed, results) = mpsc::sync_channel(8);
        let sequence = Arc::new(AtomicU64::new(0));
        let generation = Arc::clone(&sequence);
        thread::spawn(move || {
            // COM interfaces and their journal registrations never cross apartments.
            let monitor = PlatformMonitor::new(policy).map_err(|error| format!("{error:#}"));
            let mut journal = MicrophoneLockJournal::default();
            #[cfg(feature = "flutter-desktop")]
            let mut controlled_baseline = false;
            while let Ok(request) = pending.recv() {
                let result = match &monitor {
                    Err(error) => Err(error.clone()),
                    Ok(monitor) => (|| {
                        match request {
                            Request::Manual => {
                                monitor.toggle_microphone_mute()?;
                                journal.invalidate_manual();
                            }
                            #[cfg(feature = "flutter-desktop")]
                            Request::Set { requested, expected_requested, expected_generation, .. } => {
                                // This is the native worker's sequence, not an SDK mute
                                // boolean guessed in Dart. Recheck just before mutation.
                                anyhow::ensure!(generation.load(Ordering::Acquire) == expected_generation,
                                    "Microphone worker generation changed; no command applied");
                                let before = monitor.microphone_protection_status()?;
                                anyhow::ensure!(before.requested == expected_requested,
                                    "Microphone request changed; no command applied");
                                if controlled && !controlled_baseline {
                                    anyhow::ensure!(!before.requested && !before.service_active
                                        && before.endpoint_count > 0
                                        && before.mute_state == crate::model::MicrophoneMuteState::Unmuted,
                                        "Controlled test baseline is not completely unmuted/released; no command applied");
                                    // Keep recovery available if the native operation is partial.
                                    controlled_baseline = true;
                                }
                                monitor.set_microphone_mute(requested)?;
                                journal.invalidate_manual();
                            }
                            Request::Lock => {
                                monitor.mute_for_lock(&mut journal)?;
                            }
                            Request::Unlock => {
                                monitor.restore_after_lock(&mut journal)?;
                            }
                        }
                        monitor.microphone_protection_status()
                    })()
                    .map_err(|error: anyhow::Error| format!("{error:#}")),
                };
                let completed_generation =
                    generation.fetch_add(1, Ordering::AcqRel).wrapping_add(1);
                if completed
                    .send((request, completed_generation, result))
                    .is_err()
                {
                    break;
                }
                let h = hwnd.load(Ordering::Acquire);
                if h != 0 {
                    let _ = unsafe {
                        PostMessageW(
                            Some(HWND(h as *mut _)),
                            super::WM_TRAY_REFRESH,
                            WPARAM(0),
                            LPARAM(0),
                        )
                    };
                }
            }
            // Automatic policy ownership must be resolved or durably handed back
            // before losing this consumer's token. Explicit manual protection
            // remains owned by the background service, independent of UI lifetime.
            if let Ok(monitor) = &monitor
                && let Err(error) = monitor.finish_after_lock(&mut journal)
            {
                eprintln!(
                    "mcw: microphone automatic-lock shutdown restore/handoff failed or remains pending: {error:#}"
                );
            }
            drop(journal);
        });
        Self {
            requests,
            results,
            sequence,
        }
    }
}
