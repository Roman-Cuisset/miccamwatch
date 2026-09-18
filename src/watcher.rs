use crate::{
    cli::Filter,
    i18n::Language,
    model::{Access, AccessEvent, Action, SCHEMA_VERSION, event_code},
    output,
    platform::PlatformMonitor,
};
use anyhow::{Context, Result};
use chrono::Utc;
use std::{
    collections::HashMap,
    fs::OpenOptions,
    io::{BufWriter, Write},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
use windows::{
    Win32::{
        Foundation::HANDLE,
        System::{
            EventLog::{
                DeregisterEventSource, EVENTLOG_INFORMATION_TYPE, EVENTLOG_WARNING_TYPE,
                RegisterEventSourceW, ReportEventW,
            },
            Registry::{
                HKEY, REG_NOTIFY_CHANGE_LAST_SET, REG_NOTIFY_CHANGE_NAME, RegNotifyChangeKeyValue,
            },
        },
    },
    core::PCWSTR,
};
#[allow(clippy::too_many_arguments)]
pub fn watch(
    monitor: &PlatformMonitor,
    filter: &Filter,
    json: bool,
    interval: Duration,
    notify: bool,
    log_path: Option<&Path>,
    eventlog: bool,
    lang: Language,
) -> Result<()> {
    let running = Arc::new(AtomicBool::new(true));
    let signal = Arc::clone(&running);
    ctrlc::set_handler(move || signal.store(false, Ordering::SeqCst))
        .context("failed to install Ctrl+C handler")?;

    let (wake_sender, wake_receiver) = mpsc::channel();
    let _audio_notifications = monitor
        .register_audio_notifications(wake_sender.clone())
        .ok();
    spawn_registry_watcher(wake_sender);
    let mut log_writer = log_path
        .map(|path| {
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .map(BufWriter::new)
        })
        .transpose()
        .context("failed to open JSONL log file")?;
    let min_risk = filter.risk;
    let event_source = if eventlog {
        let source: Vec<u16> = "miccamwatch".encode_utf16().chain(Some(0)).collect();
        Some(unsafe { RegisterEventSourceW(None, PCWSTR(source.as_ptr()))? })
    } else {
        None
    };
    let initial = monitor.snapshot(filter)?;
    let mut previous = by_key(initial.accesses);
    let mut last_notifications: HashMap<String, Instant> = HashMap::new();
    for access in previous.values() {
        let event = AccessEvent {
            schema_version: SCHEMA_VERSION,
            event_code: event_code(Action::Start),
            tool_version: env!("CARGO_PKG_VERSION"),
            action: Action::Start,
            observed_at: Utc::now(),
            access: access.clone(),
        };
        log_event(&event, &mut log_writer)?;
        write_eventlog(&event, &event_source);
        output::print_event(&event, json, min_risk, lang)?;
        if notify && notification_due(&mut last_notifications, &access.key) {
            crate::notify::notify_access(access, Action::Start, lang);
        }
    }

    while running.load(Ordering::SeqCst) {
        match wake_receiver.recv_timeout(interval) {
            Ok(()) | Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        let current = by_key(monitor.snapshot(filter)?.accesses);

        for (key, access) in &current {
            if !previous.contains_key(key) {
                let event = AccessEvent {
                    schema_version: SCHEMA_VERSION,
                    event_code: event_code(Action::Start),
                    tool_version: env!("CARGO_PKG_VERSION"),
                    action: Action::Start,
                    observed_at: Utc::now(),
                    access: access.clone(),
                };
                log_event(&event, &mut log_writer)?;
                write_eventlog(&event, &event_source);
                output::print_event(&event, json, min_risk, lang)?;
                if notify && notification_due(&mut last_notifications, key) {
                    crate::notify::notify_access(access, Action::Start, lang);
                }
            }
        }
        for (key, access) in &current {
            if let Some(old) = previous.get(key)
                && (old.activity != access.activity
                    || old.risk != access.risk
                    || old.confidence != access.confidence)
            {
                let event = AccessEvent {
                    schema_version: SCHEMA_VERSION,
                    event_code: event_code(Action::Update),
                    tool_version: env!("CARGO_PKG_VERSION"),
                    action: Action::Update,
                    observed_at: Utc::now(),
                    access: access.clone(),
                };
                log_event(&event, &mut log_writer)?;
                write_eventlog(&event, &event_source);
                output::print_event(&event, json, min_risk, lang)?;
                if notify && notification_due(&mut last_notifications, key) {
                    crate::notify::notify_access(access, Action::Update, lang);
                }
            }
        }
        for (key, access) in &previous {
            if !current.contains_key(key) {
                let event = AccessEvent {
                    schema_version: SCHEMA_VERSION,
                    event_code: event_code(Action::Stop),
                    tool_version: env!("CARGO_PKG_VERSION"),
                    action: Action::Stop,
                    observed_at: Utc::now(),
                    access: access.clone(),
                };
                log_event(&event, &mut log_writer)?;
                write_eventlog(&event, &event_source);
                output::print_event(&event, json, min_risk, lang)?;
                if notify && notification_due(&mut last_notifications, key) {
                    crate::notify::notify_access(access, Action::Stop, lang);
                }
            }
        }

        previous = current;
    }
    if let Some(handle) = event_source {
        let _ = unsafe { DeregisterEventSource(handle) };
    }
    Ok(())
}

fn log_event(event: &AccessEvent, writer: &mut Option<BufWriter<std::fs::File>>) -> Result<()> {
    if let Some(writer) = writer {
        serde_json::to_writer(&mut *writer, event)?;
        writeln!(writer)?;
        writer.flush()?;
    }
    Ok(())
}

fn write_eventlog(event: &AccessEvent, handle: &Option<HANDLE>) {
    let Some(handle) = handle else { return };
    let msg = format!(
        "{} {} {} {} (PID {})",
        event.action,
        event.access.resource,
        event.access.application,
        event.access.risk,
        event.access.pid.map_or("?".into(), |p| p.to_string()),
    );
    let wide: Vec<u16> = msg.encode_utf16().chain(Some(0)).collect();
    let strings = [PCWSTR(wide.as_ptr())];
    let event_type = if event.access.risk >= crate::model::Risk::Suspicious {
        EVENTLOG_WARNING_TYPE
    } else {
        EVENTLOG_INFORMATION_TYPE
    };
    let _ = unsafe {
        ReportEventW(
            *handle,
            event_type,
            0,
            event.event_code as u32,
            None,
            0,
            Some(&strings),
            None,
        )
    };
}

fn notification_due(last: &mut HashMap<String, Instant>, key: &str) -> bool {
    const COOLDOWN: Duration = Duration::from_secs(30);
    let now = Instant::now();
    if last
        .get(key)
        .is_some_and(|previous| now.duration_since(*previous) < COOLDOWN)
    {
        return false;
    }
    last.insert(key.to_owned(), now);
    true
}

fn spawn_registry_watcher(sender: mpsc::Sender<()>) {
    std::thread::spawn(move || {
        let root = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
        let Ok(key) = root.open_subkey(
            r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore",
        ) else {
            return;
        };
        let hkey = HKEY(key.raw_handle().cast());
        loop {
            let status = unsafe {
                RegNotifyChangeKeyValue(
                    hkey,
                    true,
                    REG_NOTIFY_CHANGE_NAME | REG_NOTIFY_CHANGE_LAST_SET,
                    None,
                    false,
                )
            };
            if status.is_err() || sender.send(()).is_err() {
                break;
            }
        }
    });
}

fn by_key(accesses: Vec<Access>) -> HashMap<String, Access> {
    accesses
        .into_iter()
        .map(|access| (access.key.clone(), access))
        .collect()
}
