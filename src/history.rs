use crate::{model::AccessEvent, settings};
use anyhow::{Context, Result, bail};
use std::{fs, io::Write, path::PathBuf};
use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE, WAIT_ABANDONED, WAIT_OBJECT_0},
        System::Threading::{CreateMutexW, INFINITE, ReleaseMutex, WaitForSingleObject},
    },
    core::w,
};

// The CLI and tray may write/rotate/clear the same JSONL files concurrently.
struct HistoryLock(HANDLE);

impl HistoryLock {
    fn acquire() -> Result<Self> {
        let handle = unsafe { CreateMutexW(None, false, w!("Local\\MicCamWatch.History")) }
            .context("failed to create history mutex")?;
        let result = unsafe { WaitForSingleObject(handle, INFINITE) };
        if result != WAIT_OBJECT_0 && result != WAIT_ABANDONED {
            let _ = unsafe { CloseHandle(handle) };
            bail!("failed to wait for history mutex: {result:?}");
        }
        Ok(Self(handle))
    }
}

impl Drop for HistoryLock {
    fn drop(&mut self) {
        unsafe {
            let _ = ReleaseMutex(self.0);
            let _ = CloseHandle(self.0);
        }
    }
}

const MAX_HISTORY_BYTES: u64 = 5 * 1024 * 1024;
const HISTORY_FILES: usize = 3;

pub fn path() -> Result<PathBuf> {
    Ok(settings::data_dir()?.join("history.jsonl"))
}

pub fn append(event: &AccessEvent) -> Result<()> {
    append_at(&path()?, event)
}

fn append_at(path: &std::path::Path, event: &AccessEvent) -> Result<()> {
    let _lock = HistoryLock::acquire()?;
    if path
        .metadata()
        .is_ok_and(|metadata| metadata.len() >= MAX_HISTORY_BYTES)
    {
        rotate(path)?;
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("failed to open history {}", path.display()))?;
    serde_json::to_writer(&mut file, event)?;
    file.write_all(b"\n")?;
    Ok(())
}

pub fn clear() -> Result<()> {
    let base = path()?;
    let _lock = HistoryLock::acquire()?;
    for index in 0..=HISTORY_FILES {
        let candidate = if index == 0 {
            base.clone()
        } else {
            base.with_extension(format!("jsonl.{index}"))
        };
        if candidate.exists() {
            fs::remove_file(candidate)?;
        }
    }
    Ok(())
}

fn rotate(base: &std::path::Path) -> Result<()> {
    let oldest = base.with_extension(format!("jsonl.{HISTORY_FILES}"));
    if oldest.exists() {
        fs::remove_file(oldest)?;
    }
    for index in (1..HISTORY_FILES).rev() {
        let source = base.with_extension(format!("jsonl.{index}"));
        if source.exists() {
            fs::rename(source, base.with_extension(format!("jsonl.{}", index + 1)))?;
        }
    }
    if base.exists() {
        fs::rename(base, base.with_extension("jsonl.1"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        Access, Action, Activity, Confidence, EnforcementDecision, Resource, Risk, SCHEMA_VERSION,
        event_code,
    };
    use std::sync::Arc;

    #[test]
    fn concurrent_writers_keep_every_history_line_parseable() {
        let path = std::env::temp_dir().join(format!(
            "mcw-history-{}-{}.jsonl",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let event = Arc::new(AccessEvent {
            schema_version: SCHEMA_VERSION,
            event_code: event_code(Action::Start),
            tool_version: env!("CARGO_PKG_VERSION"),
            action: Action::Start,
            observed_at: chrono::Utc::now(),
            access: Access {
                key: "mic:1".into(),
                resource: Resource::Microphone,
                activity: Activity::Active,
                risk: Risk::Expected,
                confidence: Confidence::High,
                enforcement: EnforcementDecision::Alert,
                application: "test.exe".into(),
                pid: Some(1),
                parent_pid: None,
                parent_name: None,
                executable: None,
                signature: None,
                device: None,
                started_at: None,
                modules: vec![],
                evidence: vec![],
                process: None,
            },
        });
        let workers = (0..6)
            .map(|_| {
                let path = path.clone();
                let event = Arc::clone(&event);
                std::thread::spawn(move || {
                    for _ in 0..25 {
                        append_at(&path, &event).unwrap();
                    }
                })
            })
            .collect::<Vec<_>>();
        for worker in workers {
            worker.join().unwrap();
        }
        let lines = fs::read_to_string(&path).unwrap();
        assert_eq!(lines.lines().count(), 150);
        for line in lines.lines() {
            let document: serde_json::Value = serde_json::from_str(line).unwrap();
            assert_eq!(document["action"], "start");
        }
        fs::remove_file(path).unwrap();
    }
}
