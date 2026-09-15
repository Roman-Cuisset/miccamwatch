use crate::{
    cli::Filter,
    model::{Access, AccessEvent, Action},
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
    time::Duration,
};

pub fn watch(
    monitor: &PlatformMonitor,
    filter: &Filter,
    json: bool,
    interval: Duration,
) -> Result<()> {
    let running = Arc::new(AtomicBool::new(true));
    let signal = Arc::clone(&running);
    ctrlc::set_handler(move || signal.store(false, Ordering::SeqCst))
        .context("failed to install Ctrl+C handler")?;

    let initial = monitor.snapshot(filter)?;
    let mut previous = by_key(initial);
    for access in previous.values().cloned() {
        output::print_event(
            &AccessEvent {
                action: Action::Start,
                observed_at: Utc::now(),
                access,
            },
            json,
        )?;
    }

    while running.load(Ordering::SeqCst) {
        thread::sleep(interval);
        let current = by_key(monitor.snapshot(filter)?);

        for (key, access) in &current {
            if !previous.contains_key(key) {
                output::print_event(
                    &AccessEvent {
                        action: Action::Start,
                        observed_at: Utc::now(),
                        access: access.clone(),
                    },
                    json,
                )?;
            }
        }
        for (key, access) in &previous {
            if !current.contains_key(key) {
                output::print_event(
                    &AccessEvent {
                        action: Action::Stop,
                        observed_at: Utc::now(),
                        access: access.clone(),
                    },
                    json,
                )?;
            }
        }

        previous = current;
    }
    Ok(())
}

fn by_key(accesses: Vec<Access>) -> HashMap<String, Access> {
    accesses
        .into_iter()
        .map(|access| (access.key.clone(), access))
        .collect()
}
