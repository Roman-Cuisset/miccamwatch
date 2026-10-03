//! Metadata-only camera activity monitoring. No media source is activated.
//! API contract and Microsoft's camera-in-use sample:
//! https://learn.microsoft.com/windows/win32/api/mfidl/nn-mfidl-imfsensoractivitymonitor
use super::{filetime_value, process_creation_time};
use crate::model::Device;
use anyhow::{Context, Result};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use windows::{
    Win32::Media::MediaFoundation::{
        IMFSensorActivitiesReport, IMFSensorActivitiesReportCallback,
        IMFSensorActivitiesReportCallback_Impl, IMFSensorActivityMonitor, IMFShutdown,
        MF_E_NOT_FOUND, MFCreateSensorActivityMonitor,
    },
    core::{Interface, PCWSTR, Ref},
};

#[derive(Clone, Debug)]
pub(super) struct ProcessActivity {
    pub pid: u32,
    pub streaming: bool,
    pub reported_at: Option<u64>,
    pub created_at: Option<u64>,
}

impl ProcessActivity {
    // A report predating the current process cannot authorize attribution to a
    // reused PID. Inaccessible identity remains device-level evidence instead.
    pub fn attributed_pid(&self, current_creation: Option<u64>) -> Option<u32> {
        let created = self.created_at?;
        (self.pid != 0
            && current_creation == Some(created)
            && self.reported_at.is_some_and(|reported| created <= reported))
        .then_some(self.pid)
    }
}

#[derive(Default)]
struct Reports {
    devices: HashMap<String, Vec<ProcessActivity>>,
    error: Option<String>,
}

impl Reports {
    fn replace_device(&mut self, id: &str, processes: Vec<ProcessActivity>) {
        // Each device report is a current process list, not a start-only event.
        // Replacing the list (including an empty one) is essential on stop.
        self.devices.insert(id.to_owned(), processes);
    }
}

struct CameraKey {
    id: String,
    wide: Vec<u16>,
}

#[windows::core::implement(IMFSensorActivitiesReportCallback)]
struct ActivityCallback {
    cameras: Vec<CameraKey>,
    reports: Arc<Mutex<Reports>>,
}

impl IMFSensorActivitiesReportCallback_Impl for ActivityCallback_Impl {
    fn OnActivitiesReport(
        &self,
        report: Ref<IMFSensorActivitiesReport>,
    ) -> windows::core::Result<()> {
        let result = (|| -> windows::core::Result<()> {
            let report = report.as_ref().ok_or_else(windows::core::Error::empty)?;
            let mut reports = self
                .reports
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            for camera in &self.cameras {
                let activity = match unsafe {
                    report.GetActivityReportByDeviceName(PCWSTR(camera.wide.as_ptr()))
                } {
                    Ok(activity) => activity,
                    Err(error) if error.code() == MF_E_NOT_FOUND => continue,
                    Err(error) => return Err(error),
                };
                let mut processes = Vec::new();
                for index in 0..unsafe { activity.GetProcessCount()? } {
                    let process = unsafe { activity.GetProcessActivity(index)? };
                    let streaming = unsafe { process.GetStreamingState()? }.as_bool();
                    // Failure to identify a real stream must not erase it or
                    // guess the client from loaded DLLs, CPU, or app names.
                    let pid = unsafe { process.GetProcessId() }.unwrap_or(0);
                    let reported_at = unsafe { process.GetReportTime() }.ok().map(filetime_value);
                    processes.push(ProcessActivity {
                        pid,
                        streaming,
                        reported_at,
                        created_at: (pid != 0).then(|| process_creation_time(pid)).flatten(),
                    });
                }
                reports.replace_device(&camera.id, processes);
            }
            Ok(())
        })();
        if let Err(error) = &result {
            self.reports
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .error = Some(error.to_string());
        }
        result
    }
}

struct MonitorGuard {
    monitor: IMFSensorActivityMonitor,
    finished: bool,
}

impl Drop for MonitorGuard {
    fn drop(&mut self) {
        if !self.finished {
            let _ = unsafe { self.monitor.Stop() };
            if let Ok(shutdown) = self.monitor.cast::<IMFShutdown>() {
                let _ = unsafe { shutdown.Shutdown() };
            }
        }
    }
}

pub(super) fn collect(cameras: &[Device]) -> Result<HashMap<String, Vec<ProcessActivity>>> {
    if cameras.is_empty() {
        return Ok(HashMap::new());
    }
    let reports = Arc::new(Mutex::new(Reports::default()));
    let callback: IMFSensorActivitiesReportCallback = ActivityCallback {
        cameras: cameras
            .iter()
            .map(|camera| CameraKey {
                id: camera.id.clone(),
                wide: camera.id.encode_utf16().chain(Some(0)).collect(),
            })
            .collect(),
        reports: Arc::clone(&reports),
    }
    .into();
    let mut monitor = MonitorGuard {
        monitor: unsafe { MFCreateSensorActivityMonitor(&callback) }
            .context("failed to create the Windows sensor activity monitor")?,
        finished: false,
    };
    unsafe { monitor.monitor.Start() }
        .context("failed to start the Windows sensor activity monitor")?;
    // Microsoft's sample allows 500 ms for reports, including already-running
    // clients. Collect for the full window so a stop can supersede a start.
    // A fresh monitor on every snapshot prevents retained reports becoming
    // perpetual Active rows if notifications cease or a client exits.
    std::thread::sleep(Duration::from_millis(500));
    unsafe { monitor.monitor.Stop() }
        .context("failed to stop the Windows sensor activity monitor")?;
    let shutdown = monitor
        .monitor
        .cast::<IMFShutdown>()
        .context("sensor activity monitor has no shutdown interface")?;
    unsafe { shutdown.Shutdown() }
        .context("failed to shut down the Windows sensor activity monitor")?;
    monitor.finished = true;
    let mut reports = reports.lock().unwrap_or_else(|error| error.into_inner());
    if let Some(error) = reports.error.take() {
        anyhow::bail!("failed to read a Windows sensor activity report: {error}");
    }
    Ok(std::mem::take(&mut reports.devices))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stop_and_empty_device_reports_remove_prior_streams() {
        let mut reports = Reports::default();
        let activity = ProcessActivity {
            pid: 12,
            streaming: true,
            reported_at: Some(200),
            created_at: Some(100),
        };
        reports.replace_device("camera", vec![activity.clone()]);
        reports.replace_device(
            "camera",
            vec![ProcessActivity {
                streaming: false,
                ..activity
            }],
        );
        assert!(!reports.devices["camera"][0].streaming);
        reports.replace_device("camera", Vec::new());
        assert!(reports.devices["camera"].is_empty());
    }

    #[test]
    fn stream_cannot_be_attributed_to_reused_or_unreadable_process() {
        let activity = ProcessActivity {
            pid: 12,
            streaming: true,
            reported_at: Some(200),
            created_at: Some(100),
        };
        assert_eq!(activity.attributed_pid(Some(100)), Some(12));
        assert_eq!(activity.attributed_pid(Some(300)), None);
        assert_eq!(activity.attributed_pid(None), None);
        let old_report = ProcessActivity {
            reported_at: Some(99),
            ..activity.clone()
        };
        assert_eq!(old_report.attributed_pid(Some(100)), None);
        let unknown_time = ProcessActivity {
            reported_at: None,
            ..activity
        };
        assert_eq!(unknown_time.attributed_pid(Some(100)), None);
    }
}
