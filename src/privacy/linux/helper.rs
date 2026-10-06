use super::{
    Action, CameraPrivacyState, HELPER, PROTOCOL, Reply, VERSION,
    store::{Entry, Intent, Journal, Store},
    usb::{self, Device, Inventory},
};
use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::{
    env, fs,
    io::{self, Write},
    os::unix::fs::MetadataExt,
};

/// Evaluate ownership evidence against current driver links. No requested action,
/// cached state flag, or mere absence of /dev/video nodes can establish blocking.
pub(super) fn established_state(
    inventory: &Inventory,
    entries: &[Entry],
) -> Result<(CameraPrivacyState, String)> {
    for entry in entries {
        if let Err(error) = usb::matching(inventory, &entry.device) {
            return Ok((
                CameraPrivacyState::SystemManaged,
                format!(
                    "USB camera state is partial/unknown: {error}. Retained {} original binding record(s).",
                    entries.len()
                ),
            ));
        }
    }
    if !inventory.unsupported.is_empty() {
        return Ok((
            CameraPrivacyState::SystemManaged,
            format!(
                "Unsupported/externally controlled camera inventory: {}. USB controls do not claim a global camera block.",
                inventory.unsupported.join("; ")
            ),
        ));
    }
    if inventory.devices.is_empty() {
        bail!(
            "No supported USB video-class camera interfaces are present; non-USB cameras cannot be controlled by this helper"
        );
    }
    let mut bound = 0;
    let mut owned_unbound = 0;
    let mut external_unbound = 0;
    for device in &inventory.devices {
        let owned = entries.iter().find(|entry| entry.device == device.identity);
        for (name, driver) in &device.bindings {
            match driver.as_deref() {
                Some("uvcvideo") => bound += 1,
                None if owned.is_some_and(|entry| entry.original_bound.contains(name)) => {
                    owned_unbound += 1
                }
                None if owned.is_some() => {} // Originally unbound siblings remain untouched.
                _ => external_unbound += 1,
            }
        }
    }
    if external_unbound > 0 || (bound > 0 && owned_unbound > 0) {
        return Ok((
            CameraPrivacyState::SystemManaged,
            format!(
                "USB camera bindings are partial/externally managed: {bound} bound, {owned_unbound} owned unbound, {external_unbound} unowned/unavailable. Restoration evidence is retained; no global block is claimed."
            ),
        ));
    }
    if bound == 0 && owned_unbound > 0 {
        return Ok((
            CameraPrivacyState::Blocked,
            format!(
                "Verified all {} currently present supported USB/UVC interface(s) unbound; {} owned original device binding record(s) retained. No automatic block of new hotplug cameras; non-USB cameras are outside this control.",
                owned_unbound,
                entries.len()
            ),
        ));
    }
    if bound > 0 && owned_unbound == 0 {
        return Ok((
            CameraPrivacyState::Allowed,
            format!(
                "Verified {bound} supported USB/UVC interface(s) bound to uvcvideo. {} original binding record(s) retained; external restrictions and non-USB devices are not overridden. Cameras replugged/replaced since a block are not persistently disabled; explicit allow can retire demonstrably vanished old kernel generations without touching replacements.",
                entries.len()
            ),
        ));
    }
    Ok((CameraPrivacyState::SystemManaged, "USB camera interfaces are unbound without owned restoration evidence; they will not be adopted or rebound.".to_owned()))
}

fn restored(device: &Device, entry: &Entry) -> bool {
    device.bindings.iter().all(|(name, driver)| {
        if entry.original_bound.contains(name) {
            driver.as_deref() == Some("uvcvideo")
        } else {
            driver.is_none()
        }
    })
}

fn ordered(entry: &Entry) -> Vec<&str> {
    let mut names: Vec<_> = entry.original_bound.iter().map(String::as_str).collect();
    // Binding the UVC control interface may claim streaming siblings; unbinding
    // it may release them. Only operate on still-bound/unbound originals afterwards.
    names.sort_by(|a, b| {
        let streaming = |name: &str| {
            entry
                .device
                .interfaces
                .iter()
                .find(|interface| interface.name == name)
                .map(|interface| interface.subclass != 1)
                .unwrap_or(true)
        };
        streaming(a).cmp(&streaming(b)).then_with(|| a.cmp(b))
    });
    names
}

fn supported_original_arrangement(device: &Device) -> Result<()> {
    let bound = device
        .bindings
        .values()
        .filter(|driver| driver.as_deref() == Some("uvcvideo"))
        .count();
    if bound != 0 && bound != device.bindings.len() {
        bail!(
            "camera {} is partly bound/externally managed; blocking it could create unowned streaming sibling claims during restoration, so no intent or mutation is recorded",
            device.identity.name
        );
    }
    Ok(())
}

fn block(store: &Store, journal: &mut Journal, uid: u32) -> Result<()> {
    let inventory = usb::inventory()?;
    if !inventory.unsupported.is_empty() {
        bail!(
            "cannot establish a complete supported camera block: {}; no new USB changes requested",
            inventory.unsupported.join("; ")
        );
    }
    for entry in &journal.entries {
        if entry.original_bound.len() != entry.device.interfaces.len() {
            bail!(
                "owned camera has an unsupported partly-bound original arrangement; evidence retained without new intent or mutation"
            );
        }
        usb::matching(&inventory, &entry.device)?;
        if entry.uid != uid {
            bail!(
                "USB bindings are owned by another authorized user; their original-state records are preserved"
            );
        }
    }
    // Preflight every unowned device before recording ANY new intent. UVC
    // control restoration can claim streaming siblings; only fully bound
    // originals have a durable, complete ownership set for those effects.
    for device in &inventory.devices {
        if !journal
            .entries
            .iter()
            .any(|entry| entry.device == device.identity)
        {
            supported_original_arrangement(device)?;
        }
    }
    for device in &inventory.devices {
        if journal
            .entries
            .iter()
            .any(|entry| entry.device == device.identity)
        {
            continue;
        }
        let original_bound: Vec<_> = device
            .bindings
            .iter()
            .filter(|(_, driver)| driver.as_deref() == Some("uvcvideo"))
            .map(|(name, _)| name.clone())
            .collect();
        if original_bound.is_empty() {
            continue;
        }
        journal.entries.push(Entry {
            uid,
            intent: Intent::Block,
            device: device.identity.clone(),
            original_bound,
        });
    }
    if journal.entries.is_empty() {
        bail!(
            "no currently bound supported USB/UVC interfaces can be blocked; originally unbound cameras are not adopted"
        );
    }
    for entry in &mut journal.entries {
        entry.intent = Intent::Block;
    }
    // Durable exact identities AND original binding sets precede every unbind.
    store.save(journal)?;
    let mut failures = Vec::new();
    for entry in &journal.entries {
        for name in ordered(entry) {
            if let Err(error) = usb::write_driver(&entry.device, name, false) {
                failures.push(format!("{name}: {error:#}"));
                break;
            }
        }
    }
    store.cache(journal)?;
    let actual = usb::inventory()?;
    let (state, detail) = established_state(&actual, &journal.entries)?;
    if !failures.is_empty() || state != CameraPrivacyState::Blocked {
        bail!(
            "USB camera block failed/partial; original binding journal retained. {} {}",
            failures.join("; "),
            detail
        );
    }
    Ok(())
}

fn restore_device(entry: &Entry) -> Result<()> {
    let before = usb::inventory()?;
    usb::matching(&before, &entry.device)?;
    for name in ordered(entry) {
        usb::write_driver(&entry.device, name, true)?;
    }
    let actual = usb::inventory()?;
    if !restored(usb::matching(&actual, &entry.device)?, entry) {
        bail!("kernel did not restore the exact original interface binding set; evidence retained");
    }
    Ok(())
}

fn allow(store: &Store, journal: &mut Journal, uid: u32) -> Result<String> {
    for entry in journal.entries.iter().filter(|entry| entry.uid == uid) {
        usb::supported_configuration(&entry.device)?;
        if entry.original_bound.len() != entry.device.interfaces.len() {
            bail!(
                "partly-bound original camera arrangement cannot be restored safely; evidence retained without new intent or binding unowned siblings"
            );
        }
    }
    for entry in journal.entries.iter_mut().filter(|entry| entry.uid == uid) {
        entry.intent = Intent::Restore;
    }
    store.save(journal)?;
    let mut failures = Vec::new();
    let mut retired = Vec::new();
    let mut index = 0;
    while index < journal.entries.len() {
        if journal.entries[index].uid != uid {
            index += 1;
            continue;
        }
        match usb::old_generation_gone(&journal.entries[index].device) {
            Ok(true) => {
                retired.push(journal.entries[index].device.name.clone());
                journal.entries.remove(index);
                store.save(journal)?;
                continue;
            }
            Err(error) => {
                failures.push(format!(
                    "{}: cannot establish safe retirement: {error:#}",
                    journal.entries[index].device.name
                ));
                index += 1;
                continue;
            }
            Ok(false) => {}
        }
        match restore_device(&journal.entries[index]) {
            Ok(()) => {
                journal.entries.remove(index);
                store.save(journal)?;
            }
            Err(error) => {
                failures.push(format!("{}: {error:#}", journal.entries[index].device.name));
                index += 1;
            }
        }
    }
    store.cache(journal)?;
    let actual = usb::inventory()?;
    let retirement = if retired.is_empty() {
        String::new()
    } else {
        format!(
            "Retired vanished old kernel generation record(s) for {}; replacement/replugged cameras were not touched and are not persistently disabled. ",
            retired.join(", ")
        )
    };
    let (state, detail) = established_state(&actual, &journal.entries)
        .with_context(|| format!("{retirement}Original-state bookkeeping completed; current USB camera state is unavailable"))?;
    if !failures.is_empty() || !journal.entries.is_empty() || state != CameraPrivacyState::Allowed {
        bail!(
            "{retirement}USB restore incomplete/unavailable; only owned original bindings are restored and remaining evidence is retained. {} {}",
            failures.join("; "),
            detail
        );
    }
    Ok(retirement)
}

fn authenticated_uid() -> Result<u32> {
    if unsafe { libc::geteuid() } != 0 || unsafe { libc::getuid() } != 0 {
        bail!("camera helper actions require administrator authorization through pkexec");
    }
    let value = env::var("PKEXEC_UID")
        .context("authenticated PKEXEC_UID is absent; invoke through pkexec")?;
    if value.is_empty() || value.len() > 10 || !value.bytes().all(|c| c.is_ascii_digit()) {
        bail!("invalid authenticated pkexec caller identity");
    }
    value
        .parse()
        .context("invalid authenticated pkexec caller UID")
}

fn fixed_executable() -> Result<()> {
    super::store::trusted_executable(HELPER)?;
    let running = fs::metadata("/proc/self/exe")?;
    let installed = fs::metadata(HELPER)?;
    if fs::canonicalize("/proc/self/exe")? != std::path::Path::new(HELPER)
        || running.dev() != installed.dev()
        || running.ino() != installed.ino()
    {
        bail!("privileged camera actions are restricted to the fixed root-owned installed helper");
    }
    Ok(())
}

fn execute(args: &[String]) -> Result<(CameraPrivacyState, String)> {
    if args.len() != 6
        || args[0] != "--protocol"
        || args[1] != "1"
        || args[2] != "--version"
        || args[3] != VERSION
        || args[4] != "--action"
    {
        bail!(
            "camera helper accepts only protocol 1 and exact matching application version {VERSION} with a bounded action"
        );
    }
    let action = match args[5].as_str() {
        "block" => Action::Block,
        "allow" => Action::Allow,
        "toggle" => Action::Toggle,
        "status" => Action::Status,
        _ => bail!("invalid camera helper action"),
    };
    fixed_executable()?;
    let uid = authenticated_uid()?;
    let store = Store::open()?;
    // The root installer shares this lock. An old already-mapped executable may
    // have waited while its fixed installation was replaced/removed.
    fixed_executable()?;
    let mut journal = store.load()?;
    let action = match action {
        Action::Toggle => {
            let actual = usb::inventory()?;
            let (state, _) = established_state(&actual, &journal.entries)?;
            if state == CameraPrivacyState::Allowed && journal.entries.is_empty() {
                Action::Block
            } else {
                Action::Allow
            }
        }
        action => action,
    };
    let note = match action {
        Action::Block => {
            block(&store, &mut journal, uid)?;
            String::new()
        }
        Action::Allow => allow(&store, &mut journal, uid)?,
        Action::Status => {
            store.cache(&journal)?;
            String::new()
        }
        Action::Toggle => unreachable!("toggle was resolved under the operation lock"),
    };
    let actual = usb::inventory()?;
    let (state, mut detail) = established_state(&actual, &journal.entries)?;
    if (matches!(action, Action::Block) && state != CameraPrivacyState::Blocked)
        || (matches!(action, Action::Allow) && state != CameraPrivacyState::Allowed)
    {
        bail!(
            "camera inventory changed after the operation; no complete requested state was established: {detail}"
        );
    }
    detail.push_str(&note);
    Ok((state, detail))
}

pub(super) fn run() -> Result<()> {
    let args: Vec<String> = env::args_os()
        .skip(1)
        .map(|value| {
            value
                .into_string()
                .map_err(|_| anyhow::anyhow!("helper arguments must be UTF-8"))
        })
        .collect::<Result<_>>()?;
    if args.len() == 1 && args[0] == "--protocol-version" {
        #[derive(Serialize)]
        struct Version {
            protocol: u32,
            version: &'static str,
        }
        serde_json::to_writer(
            io::stdout().lock(),
            &Version {
                protocol: PROTOCOL,
                version: VERSION,
            },
        )?;
        println!();
        return Ok(());
    }
    let result = if args.len() > 6 || args.iter().any(|arg| arg.len() > 128) {
        Err(anyhow::anyhow!(
            "helper request exceeds bounded action schema"
        ))
    } else {
        execute(&args)
    };
    let reply = match &result {
        Ok((state, detail)) => Reply { protocol: PROTOCOL, version: VERSION.to_owned(), ok: true, state: Some(*state), detail: detail.clone(), error: None },
        Err(error) => Reply { protocol: PROTOCOL, version: VERSION.to_owned(), ok: false, state: None, detail: "No successful complete camera state transition was established; any restoration journal is retained.".to_owned(), error: Some(format!("{error:#}")) },
    };
    let mut stdout = io::stdout().lock();
    serde_json::to_writer(&mut stdout, &reply)?;
    stdout.write_all(b"\n")?;
    stdout.flush()?;
    result.map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(device: &Device) -> Entry {
        Entry {
            uid: 1000,
            intent: Intent::Block,
            device: device.identity.clone(),
            original_bound: device.bindings.keys().cloned().collect(),
        }
    }
    fn inventory(device: Device) -> Inventory {
        Inventory {
            boot_id: device.identity.boot_id.clone(),
            devices: vec![device],
            unsupported: vec![],
        }
    }

    #[test]
    fn durable_block_intent_does_not_report_still_bound_camera_blocked() {
        let device = usb::test_device();
        let entry = record(&device);
        assert_eq!(
            established_state(&inventory(device), &[entry]).unwrap().0,
            CameraPrivacyState::Allowed
        );
    }

    #[test]
    fn independent_driver_absence_requires_owned_original_bindings() {
        let mut device = usb::test_device();
        let entry = record(&device);
        for driver in device.bindings.values_mut() {
            *driver = None;
        }
        let actual = inventory(device);
        assert_eq!(
            established_state(&actual, &[entry]).unwrap().0,
            CameraPrivacyState::Blocked
        );
        assert_eq!(
            established_state(&actual, &[]).unwrap().0,
            CameraPrivacyState::SystemManaged
        );
    }

    #[test]
    fn control_release_of_both_siblings_is_complete_but_partial_release_is_not() {
        let mut device = usb::test_device();
        let entry = record(&device);
        device.bindings.insert("1-6:1.0".to_owned(), None);
        assert_eq!(
            established_state(&inventory(device), &[entry]).unwrap().0,
            CameraPrivacyState::SystemManaged
        );
    }

    #[test]
    fn fresh_hotplug_is_not_covered_by_an_existing_block() {
        let mut old = usb::test_device();
        let entry = record(&old);
        for driver in old.bindings.values_mut() {
            *driver = None;
        }
        let mut new = usb::test_device();
        new.identity.name = "1-7".to_owned();
        new.identity.topology = "/sys/devices/pci0000:00/usb1/1-7".to_owned();
        for interface in &mut new.identity.interfaces {
            interface.name = interface.name.replace("1-6:", "1-7:");
        }
        new.bindings = new
            .identity
            .interfaces
            .iter()
            .map(|interface| (interface.name.clone(), Some("uvcvideo".to_owned())))
            .collect();
        let actual = Inventory {
            boot_id: old.identity.boot_id.clone(),
            devices: vec![old, new],
            unsupported: vec![],
        };
        assert_eq!(
            established_state(&actual, &[entry]).unwrap().0,
            CameraPrivacyState::SystemManaged
        );
    }

    #[test]
    fn restoration_readback_requires_each_owned_original_binding() {
        let mut device = usb::test_device();
        let entry = record(&device);
        assert!(restored(&device, &entry));
        device.bindings.insert("1-6:1.1".to_owned(), None);
        assert!(!restored(&device, &entry));
        device
            .bindings
            .insert("1-6:1.1".to_owned(), Some("another_driver".to_owned()));
        assert!(!restored(&device, &entry));
    }

    #[test]
    fn unsupported_camera_prevents_claiming_a_global_block() {
        let mut device = usb::test_device();
        let entry = record(&device);
        for driver in device.bindings.values_mut() {
            *driver = None;
        }
        let mut actual = inventory(device);
        actual.unsupported.push("non-USB video device".to_owned());
        assert_eq!(
            established_state(&actual, &[entry]).unwrap().0,
            CameraPrivacyState::SystemManaged
        );
    }

    #[test]
    fn partly_bound_originals_are_refused_before_intent_and_all_unbound_are_untouched() {
        let mut device = usb::test_device();
        assert!(supported_original_arrangement(&device).is_ok());
        device.bindings.insert("1-6:1.1".to_owned(), None);
        assert!(supported_original_arrangement(&device).is_err());
        for driver in device.bindings.values_mut() {
            *driver = None;
        }
        assert!(supported_original_arrangement(&device).is_ok());
    }
}
