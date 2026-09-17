use crate::{
    cli::Filter,
    model::{Access, AccessEvent, Action, SCHEMA_VERSION},
    output,
    platform::PlatformMonitor,
};
use anyhow::{Context, Result};
use chrono::Utc;
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

pub fn watch(
    monitor: &PlatformMonitor,
    filter: &Filter,
    json: bool,
    interval: Duration,
    notify: bool,
) -> Result<()> {
    let running = Arc::new(AtomicBool::new(true));
    let signal = Arc::clone(&running);
    ctrlc::set_handler(move || signal.store(false, Ordering::SeqCst))
        .context("failed to install Ctrl+C handler")?;

    let initial = monitor.snapshot(filter)?;
    let mut previous = by_key(initial);
    let mut last_notifications: HashMap<String, Instant> = HashMap::new();
    for access in previous.values() {
        output::print_event(
            &AccessEvent {
                schema_version: SCHEMA_VERSION,
                action: Action::Start,
                observed_at: Utc::now(),
                access: access.clone(),
            },
            json,
        )?;
        if notify && notification_due(&mut last_notifications, &access.key) {
            crate::notify::notify_access(access, Action::Start);
        }
    }

    while running.load(Ordering::SeqCst) {
        thread::sleep(interval);
        let current = by_key(monitor.snapshot(filter)?);

        for (key, access) in &current {
            if !previous.contains_key(key) {
                output::print_event(
                    &AccessEvent {
                        schema_version: SCHEMA_VERSION,
                        action: Action::Start,
                        observed_at: Utc::now(),
                        access: access.clone(),
                    },
                    json,
                )?;
                if notify && notification_due(&mut last_notifications, key) {
                    crate::notify::notify_access(access, Action::Start);
                }
            }
        }
        for (key, access) in &current {
            if let Some(old) = previous.get(key)
                && (old.activity != access.activity
                    || old.risk != access.risk
                    || old.confidence != access.confidence)
            {
                output::print_event(
                    &AccessEvent {
                        schema_version: SCHEMA_VERSION,
                        action: Action::Update,
                        observed_at: Utc::now(),
                        access: access.clone(),
                    },
                    json,
                )?;
                if notify && notification_due(&mut last_notifications, key) {
                    crate::notify::notify_access(access, Action::Update);
                }
            }
        }
        for (key, access) in &previous {
            if !current.contains_key(key) {
                output::print_event(
                    &AccessEvent {
                        schema_version: SCHEMA_VERSION,
                        action: Action::Stop,
                        observed_at: Utc::now(),
                        access: access.clone(),
                    },
                    json,
                )?;
                if notify && notification_due(&mut last_notifications, key) {
                    crate::notify::notify_access(access, Action::Stop);
                }
            }
        }

        previous = current;
    }
    Ok(())
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

fn by_key(accesses: Vec<Access>) -> HashMap<String, Access> {
    accesses
        .into_iter()
        .map(|access| (access.key.clone(), access))
        .collect()
}
