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
    Lock,
    Unlock,
}
pub(super) struct Worker {
    pub requests: SyncSender<Request>,
    pub results: Receiver<(Request, u64, Result<MicrophoneProtectionStatus, String>)>,
    pub sequence: Arc<AtomicU64>,
}
impl Worker {
    pub fn start(policy: Policy, hwnd: Arc<AtomicIsize>) -> Self {
        let (requests, pending) = mpsc::sync_channel::<Request>(8);
        let (completed, results) = mpsc::sync_channel(8);
        let sequence = Arc::new(AtomicU64::new(0));
        let generation = Arc::clone(&sequence);
        thread::spawn(move || {
            // COM interfaces and their journal registrations never cross apartments.
            let monitor = PlatformMonitor::new(policy).map_err(|error| format!("{error:#}"));
            let mut journal = MicrophoneLockJournal::default();
            while let Ok(request) = pending.recv() {
                let result = match &monitor {
                    Err(error) => Err(error.clone()),
                    Ok(monitor) => (|| {
                        match request {
                            Request::Manual => {
                                monitor.toggle_microphone_mute()?;
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
