use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::Read,
    os::{
        fd::AsRawFd,
        unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt},
    },
    path::{Path, PathBuf},
};

const USB: &str = "/sys/bus/usb/devices";
const MAX_ATTRIBUTE: u64 = 65536;

// Kernel UAPI (linux/usbdevice_fs.h). libc's encoders preserve architecture ABI.
#[repr(C)]
struct UsbIoctl {
    ifno: libc::c_int,
    ioctl_code: libc::c_int,
    data: *mut libc::c_void,
}
#[repr(C)]
struct ConnectionInfo {
    size: u32,
    busnum: u32,
    devnum: u32,
    speed: u32,
    num_ports: u8,
    ports: [u8; 7],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Generation {
    filesystem: u64,
    directory_inode: u64,
    attribute_inode: u64,
    attribute_ctime: i64,
    attribute_ctime_nsec: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct InterfaceIdentity {
    pub name: String,
    pub subclass: u8,
    number: u8,
    protocol: u8,
    generation: Generation,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DeviceIdentity {
    pub boot_id: String,
    pub name: String,
    pub topology: String,
    generation: Generation,
    busnum: u32,
    devnum: u32,
    descriptors: Vec<u8>,
    serial: Option<String>,
    pub interfaces: Vec<InterfaceIdentity>,
}

#[derive(Debug)]
pub(super) struct Device {
    pub identity: DeviceIdentity,
    /// None is independently observed absence of a driver, not a requested state.
    pub bindings: BTreeMap<String, Option<String>>,
}

#[derive(Debug)]
pub(super) struct Inventory {
    pub boot_id: String,
    pub devices: Vec<Device>,
    pub unsupported: Vec<String>,
}

pub(super) fn valid_boot_id(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(i, c)| {
            if matches!(i, 8 | 13 | 18 | 23) {
                c == b'-'
            } else {
                c.is_ascii_hexdigit()
            }
        })
}

pub(super) fn boot_id() -> Result<String> {
    let value = fs::read_to_string("/proc/sys/kernel/random/boot_id")?
        .trim()
        .to_owned();
    if !valid_boot_id(&value) {
        bail!("invalid kernel boot identity");
    }
    Ok(value)
}

fn device_name(name: &str) -> bool {
    if name.len() > 80 {
        return false;
    }
    let Some((bus, ports)) = name.split_once('-') else {
        return false;
    };
    !bus.is_empty()
        && bus.bytes().all(|c| c.is_ascii_digit())
        && ports
            .split('.')
            .all(|port| !port.is_empty() && port.bytes().all(|c| c.is_ascii_digit()))
}

fn interface_name(name: &str, device: &str) -> bool {
    let Some((prefix, suffix)) = name.split_once(':') else {
        return false;
    };
    let Some((configuration, number)) = suffix.split_once('.') else {
        return false;
    };
    prefix == device
        && device_name(prefix)
        && name.len() <= 100
        && !configuration.is_empty()
        && configuration.bytes().all(|c| c.is_ascii_digit())
        && !number.is_empty()
        && number.bytes().all(|c| c.is_ascii_digit())
}

impl DeviceIdentity {
    pub fn validate(&self) -> Result<()> {
        let path = Path::new(&self.topology);
        if !valid_boot_id(&self.boot_id)
            || !device_name(&self.name)
            || !path.starts_with("/sys/devices")
            || path.file_name().and_then(|v| v.to_str()) != Some(self.name.as_str())
            || path.components().any(|part| {
                !matches!(
                    part,
                    std::path::Component::RootDir | std::path::Component::Normal(_)
                )
            })
            || self.topology.len() > 4096
            || self.descriptors.len() < 18
            || self.descriptors.len() > MAX_ATTRIBUTE as usize
            || self.descriptors.first() != Some(&18)
            || self.descriptors.get(1) != Some(&1)
            || self.serial.as_ref().is_some_and(|v| v.len() > 4096)
            || self.interfaces.is_empty()
            || self.interfaces.len() > 256
            || self.busnum == 0
            || self.busnum > 999
            || self.devnum == 0
            || self.devnum > 127
            || self
                .name
                .split_once('-')
                .and_then(|(bus, _)| bus.parse::<u32>().ok())
                != Some(self.busnum)
            || self.interfaces.iter().enumerate().any(|(i, interface)| {
                !interface_name(&interface.name, &self.name)
                    || self.interfaces[..i]
                        .iter()
                        .any(|other| other.name == interface.name)
                    || !matches!(interface.subclass, 1 | 2)
            })
        {
            bail!("invalid USB camera identity; refusing any kernel mutation");
        }
        Ok(())
    }
}

fn require_sysfs(path: &Path) -> Result<()> {
    let file = File::open(path)?;
    let mut stat = std::mem::MaybeUninit::<libc::statfs>::uninit();
    if unsafe { libc::fstatfs(file.as_raw_fd(), stat.as_mut_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let stat = unsafe { stat.assume_init() };
    if stat.f_type as u64 != 0x62656572 {
        bail!("camera control requires genuine kernel sysfs");
    }
    Ok(())
}

fn canonical_usb(path: &Path) -> Result<PathBuf> {
    let canonical = fs::canonicalize(path)
        .with_context(|| format!("USB device changed while reading {}", path.display()))?;
    if !canonical.starts_with("/sys/devices") {
        bail!("USB topology escapes kernel device tree");
    }
    require_sysfs(&canonical)?;
    Ok(canonical)
}

fn bytes(path: &Path) -> Result<Vec<u8>> {
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)?;
    let mut data = Vec::new();
    (&mut file).take(MAX_ATTRIBUTE + 1).read_to_end(&mut data)?;
    if data.len() as u64 > MAX_ATTRIBUTE {
        bail!("oversized USB descriptor/attribute");
    }
    Ok(data)
}

fn text(path: &Path) -> Result<String> {
    Ok(String::from_utf8(bytes(path)?)
        .context("USB attribute is not UTF-8")?
        .trim_end_matches('\n')
        .to_owned())
}

fn hex(path: &Path) -> Result<u8> {
    u8::from_str_radix(text(path)?.trim(), 16).context("invalid USB interface descriptor")
}

fn generation(directory: &Path, attribute: &str) -> Result<Generation> {
    let dir = fs::metadata(directory)?;
    // Driver bind/unbind changes directory ctime. An immutable descriptor inode
    // survives those operations but changes when the USB object is re-created.
    let attr = fs::symlink_metadata(directory.join(attribute))?;
    if !attr.is_file() || attr.uid() != 0 || dir.uid() != 0 || !dir.is_dir() {
        bail!("invalid kernel USB identity attributes");
    }
    Ok(Generation {
        filesystem: dir.dev(),
        directory_inode: dir.ino(),
        attribute_inode: attr.ino(),
        attribute_ctime: attr.ctime(),
        attribute_ctime_nsec: attr.ctime_nsec(),
    })
}

fn binding(path: &Path) -> Result<Option<String>> {
    let driver = path.join("driver");
    match fs::symlink_metadata(&driver) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
        Ok(metadata) => {
            if !metadata.file_type().is_symlink() {
                bail!("invalid USB driver link");
            }
            let target = fs::canonicalize(driver)?;
            require_sysfs(&target)?;
            if !target.starts_with("/sys/bus/usb/drivers") {
                bail!("invalid USB driver target");
            }
            Ok(Some(
                target
                    .file_name()
                    .and_then(|name| name.to_str())
                    .context("invalid USB driver name")?
                    .to_owned(),
            ))
        }
    }
}

pub(super) fn inventory() -> Result<Inventory> {
    require_sysfs(Path::new(USB))?;
    let boot_id = boot_id()?;
    let mut grouped: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut unsupported = Vec::new();
    for entry in fs::read_dir(USB)? {
        let name = entry?
            .file_name()
            .into_string()
            .map_err(|_| anyhow::anyhow!("invalid USB sysfs name"))?;
        let Some((device, _)) = name.split_once(':') else {
            continue;
        };
        if !interface_name(&name, device) {
            bail!("unexpected USB interface name");
        }
        let path = canonical_usb(&Path::new(USB).join(&name))?;
        if hex(&path.join("bInterfaceClass"))? != 0x0e {
            continue;
        }
        let subclass = hex(&path.join("bInterfaceSubClass"))?;
        if !matches!(subclass, 1 | 2) {
            unsupported.push(format!(
                "USB video interface {name} has unsupported subclass {subclass}"
            ));
            continue;
        }
        grouped.entry(device.to_owned()).or_default().push(name);
    }
    if grouped.len() > 256 {
        bail!("USB camera inventory exceeds safety bound");
    }
    let mut devices = Vec::new();
    for (name, mut names) in grouped {
        names.sort();
        let path = canonical_usb(&Path::new(USB).join(&name))?;
        let before = generation(&path, "devnum")?;
        let mut interfaces = Vec::new();
        let mut bindings = BTreeMap::new();
        for interface in names {
            let interface_path = canonical_usb(&Path::new(USB).join(&interface))?;
            if interface_path.parent() != Some(path.as_path()) {
                bail!("USB interface topology changed");
            }
            let generation = generation(&interface_path, "bInterfaceNumber")?;
            let driver = binding(&interface_path)?;
            if driver.as_deref().is_some_and(|name| name != "uvcvideo") {
                unsupported.push(format!(
                    "USB video interface {interface} is controlled by {}",
                    driver.as_deref().unwrap_or("unknown")
                ));
            }
            interfaces.push(InterfaceIdentity {
                name: interface.clone(),
                subclass: hex(&interface_path.join("bInterfaceSubClass"))?,
                number: hex(&interface_path.join("bInterfaceNumber"))?,
                protocol: hex(&interface_path.join("bInterfaceProtocol"))?,
                generation,
            });
            if self::generation(&interface_path, "bInterfaceNumber")?
                != interfaces
                    .last()
                    .context("missing interface identity")?
                    .generation
            {
                bail!("USB interface generation changed during inventory");
            }
            bindings.insert(interface, driver);
        }
        let serial = match text(&path.join("serial")) {
            Ok(serial) if serial.trim().is_empty() => None,
            Ok(serial) => Some(serial),
            Err(error)
                if error
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) =>
            {
                None
            }
            Err(error) => return Err(error),
        };
        let identity = DeviceIdentity {
            boot_id: boot_id.clone(),
            name,
            topology: path.to_str().context("non-UTF8 USB topology")?.to_owned(),
            generation: before.clone(),
            busnum: text(&path.join("busnum"))?
                .parse()
                .context("invalid USB bus number")?,
            devnum: text(&path.join("devnum"))?
                .parse()
                .context("invalid USB device number")?,
            descriptors: bytes(&path.join("descriptors"))?,
            serial,
            interfaces,
        };
        if generation(&path, "devnum")? != before {
            bail!("USB device generation changed during inventory");
        }
        identity.validate()?;
        if let Err(error) = supported_configuration(&identity) {
            unsupported.push(format!(
                "USB camera {} is outside the safe configuration envelope: {error:#}",
                identity.name
            ));
        }
        devices.push(Device { identity, bindings });
    }
    match fs::read_dir("/sys/class/video4linux") {
        Ok(nodes) => {
            for node in nodes {
                let node = node?;
                let path = fs::canonicalize(node.path().join("device"))?;
                require_sysfs(&path)?;
                if !devices.iter().any(|device| {
                    device.identity.interfaces.iter().any(|interface| {
                        path.starts_with(Path::new(&device.identity.topology).join(&interface.name))
                    })
                }) {
                    unsupported.push(format!(
                        "{} is not a supported USB/UVC camera node",
                        node.file_name().to_string_lossy()
                    ));
                }
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    if boot_id != self::boot_id()? {
        bail!("kernel boot identity changed");
    }
    Ok(Inventory {
        boot_id,
        devices,
        unsupported,
    })
}

/// The pinned usbfs FD pins the physical USB generation, not a mutable
/// interface number. Admit only one immutable configuration whose selected
/// interface numbers have the same video role in EVERY alternate setting.
pub(super) fn supported_configuration(identity: &DeviceIdentity) -> Result<()> {
    identity.validate()?;
    let descriptors = &identity.descriptors;
    if descriptors[17] != 1 {
        bail!(
            "multi-configuration USB cameras are unsupported; interface numbers can change roles"
        );
    }
    let configuration = &descriptors[18..];
    if configuration.len() < 9 || configuration[0] != 9 || configuration[1] != 2 {
        bail!("missing or ambiguous USB configuration descriptor");
    }
    let total = usize::from(u16::from_le_bytes([configuration[2], configuration[3]]));
    if total != configuration.len() || configuration[5] == 0 {
        bail!("damaged/ambiguous USB configuration extent or identity");
    }
    for interface in &identity.interfaces {
        let suffix = interface
            .name
            .split_once(':')
            .context("invalid interface topology")?
            .1;
        let (configuration_number, interface_number) = suffix
            .split_once('.')
            .context("invalid interface topology")?;
        if configuration_number.parse::<u8>().ok() != Some(configuration[5])
            || interface_number.parse::<u8>().ok() != Some(interface.number)
        {
            bail!("active interface topology disagrees with the sole USB configuration");
        }
    }
    let mut interface_numbers = [false; 256];
    let mut alternate_zero = [false; 256];
    let mut alternate_pairs = [0u8; 8192];
    let mut offset = 9;
    while offset < configuration.len() {
        if configuration.len() - offset < 2 {
            bail!("truncated USB descriptor header");
        }
        let length = usize::from(configuration[offset]);
        if length < 2 || length > configuration.len() - offset {
            bail!("truncated/zero-length USB descriptor");
        }
        let descriptor = &configuration[offset..offset + length];
        match descriptor[1] {
            1 | 2 => bail!("nested device/configuration descriptor is ambiguous"),
            4 => {
                if length != 9 {
                    bail!("invalid USB interface descriptor length");
                }
                let number = usize::from(descriptor[2]);
                let alternate = usize::from(descriptor[3]);
                let bit = number * 256 + alternate;
                let mask = 1 << (bit % 8);
                if alternate_pairs[bit / 8] & mask != 0 {
                    bail!("duplicate USB interface/alternate descriptor");
                }
                alternate_pairs[bit / 8] |= mask;
                interface_numbers[number] = true;
                alternate_zero[number] |= alternate == 0;
                if let Some(selected) = identity
                    .interfaces
                    .iter()
                    .find(|interface| usize::from(interface.number) == number)
                    && (descriptor[5] != 0x0e || descriptor[6] != selected.subclass)
                {
                    bail!(
                        "camera interface {number} has a non-video or changing control/streaming role in alternate {alternate}"
                    );
                }
            }
            _ => {}
        }
        offset += length;
    }
    if interface_numbers.iter().filter(|seen| **seen).count() != usize::from(configuration[4])
        || identity
            .interfaces
            .iter()
            .any(|interface| !alternate_zero[usize::from(interface.number)])
    {
        bail!("USB interface count or original alternate-zero role is ambiguous");
    }
    Ok(())
}

pub(super) fn matching<'a>(
    inventory: &'a Inventory,
    identity: &DeviceIdentity,
) -> Result<&'a Device> {
    identity.validate()?;
    if inventory.boot_id != identity.boot_id {
        bail!("owned camera record belongs to a stale boot; restoration evidence retained");
    }
    inventory.devices.iter().find(|device| device.identity == *identity)
        .context("original USB camera descriptor/topology/generation is absent or changed; restoration evidence retained")
}

pub(super) fn write_driver(identity: &DeviceIdentity, interface: &str, bind: bool) -> Result<()> {
    if unsafe { libc::geteuid() } != 0 {
        bail!("USB driver changes require the root helper");
    }
    identity.validate()?;
    supported_configuration(identity)?;
    if !identity
        .interfaces
        .iter()
        .any(|entry| entry.name == interface)
    {
        bail!("interface is not in the owned original identity");
    }
    let before = inventory()?;
    let device = matching(&before, identity)?;
    let current = device
        .bindings
        .get(interface)
        .context("owned interface absent")?
        .as_deref();
    if (bind && current == Some("uvcvideo")) || (!bind && current.is_none()) {
        return Ok(());
    }
    if (bind && current.is_some()) || (!bind && current != Some("uvcvideo")) {
        bail!("owned interface has another driver; refusing to override it");
    }
    let number = identity
        .interfaces
        .iter()
        .find(|entry| entry.name == interface)
        .context("owned interface missing")?
        .number;
    let control = pinned_usb_device(identity)?;
    let verify = inventory()?;
    let verified = matching(&verify, identity)?;
    let driver = verified
        .bindings
        .get(interface)
        .context("owned interface absent before ioctl")?
        .as_deref();
    if (bind && driver == Some("uvcvideo")) || (!bind && driver.is_none()) {
        return Ok(());
    }
    if (bind && driver.is_some()) || (!bind && driver != Some("uvcvideo")) {
        bail!("owned interface driver changed before action; refusing to override it");
    }
    let mut command = UsbIoctl {
        ifno: i32::from(number),
        ioctl_code: libc::_IO(b'U' as _, if bind { 23 } else { 22 }) as libc::c_int,
        data: std::ptr::null_mut(),
    };
    // The kernel locks and dereferences the FD's original usb_device, never a
    // reused sysfs name. We do NOT claim interfaces, submit URBs, reset devices,
    // change configurations, or auto-reattach anything when closing the FD.
    let result = unsafe {
        libc::ioctl(
            control.as_raw_fd(),
            libc::_IOWR::<UsbIoctl>(b'U' as _, 18),
            &mut command,
        )
    };
    let error = if result < 0 {
        Some(std::io::Error::last_os_error())
    } else {
        None
    };
    // A streaming sibling may be claimed/released by the control interface.
    // Even a failed ioctl can have effects; always inspect actual state.
    let after = inventory()?;
    let device = matching(&after, identity)?;
    let actual = device
        .bindings
        .get(interface)
        .context("owned interface absent after action")?
        .as_deref();
    if (bind && actual == Some("uvcvideo")) || (!bind && actual.is_none()) {
        return Ok(());
    }
    if let Some(error) = error {
        return Err(error).context("USB driver ioctl failed; original-state journal retained");
    }
    bail!(
        "kernel did not establish requested USB interface binding; original-state journal retained"
    )
}

fn pinned_usb_device(identity: &DeviceIdentity) -> Result<File> {
    let directory = PathBuf::from(format!("/dev/bus/usb/{:03}", identity.busnum));
    super::store::safe_directory(&directory)?;
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
        .open(directory.join(format!("{:03}", identity.devnum)))
        .context("cannot open original USB kernel device for scoped driver control")?;
    let metadata = file.metadata()?;
    let expected_minor = (identity.busnum - 1) * 128 + identity.devnum - 1;
    if !metadata.file_type().is_char_device()
        || metadata.uid() != 0
        || metadata.nlink() != 1
        || libc::major(metadata.rdev()) != 189
        || libc::minor(metadata.rdev()) != expected_minor
    {
        bail!("unsafe/mismatched USB character device");
    }
    let mut info = ConnectionInfo {
        size: 0,
        busnum: 0,
        devnum: 0,
        speed: 0,
        num_ports: 0,
        ports: [0; 7],
    };
    let result = unsafe {
        libc::ioctl(
            file.as_raw_fd(),
            libc::_IOR::<ConnectionInfo>(b'U' as _, 32),
            &mut info,
        )
    };
    if result < 0 {
        return Err(std::io::Error::last_os_error())
            .context("kernel USBDEVFS_CONNINFO_EX is unavailable; safe scoped control requires Linux 5.9 or later");
    }
    let ports = identity
        .name
        .split_once('-')
        .context("invalid USB topology")?
        .1;
    let port_count = ports.split('.').count();
    if info.size < std::mem::size_of::<ConnectionInfo>() as u32
        || info.busnum != identity.busnum
        || info.devnum != identity.devnum
        || usize::from(info.num_ports) != port_count
        || port_count > info.ports.len()
    {
        bail!("pinned USB device topology/address does not match original camera");
    }
    for (index, port) in ports.split('.').enumerate() {
        if port.parse::<u8>().context("invalid USB port number")? != info.ports[index] {
            bail!("pinned USB device physical port topology changed");
        }
    }
    let mut descriptors = Vec::new();
    (&mut file)
        .take(MAX_ATTRIBUTE + 1)
        .read_to_end(&mut descriptors)?;
    if descriptors.len() > MAX_ATTRIBUTE as usize || descriptors.len() < 18 {
        bail!("invalid pinned USB device descriptors");
    }
    // usbfs device-descriptor u16 fields are host endian; sysfs is bus endian.
    for offset in [2, 8, 10, 12] {
        let value =
            u16::from_ne_bytes([descriptors[offset], descriptors[offset + 1]]).to_le_bytes();
        descriptors[offset..offset + 2].copy_from_slice(&value);
    }
    if descriptors != identity.descriptors {
        bail!("pinned USB device descriptors changed; refusing driver action");
    }
    Ok(file)
}

/// Retirement is bookkeeping only: it never binds a replacement camera.
/// Descriptor differences alone are NOT proof that the old kernel object died.
pub(super) fn old_generation_gone(identity: &DeviceIdentity) -> Result<bool> {
    identity.validate()?;
    require_sysfs(Path::new("/sys/devices"))?;
    if boot_id()? != identity.boot_id {
        return Ok(true);
    }
    let old_object_gone = |path: &Path, original: &Generation| -> Result<bool> {
        let metadata = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(true),
            Err(error) => return Err(error.into()),
        };
        if !metadata.is_dir() || metadata.uid() != 0 {
            bail!("unsafe original USB object during retirement");
        }
        require_sysfs(path)?;
        if metadata.dev() != original.filesystem {
            bail!(
                "sysfs filesystem changed; old camera generation disappearance cannot be established"
            );
        }
        Ok(metadata.ino() != original.directory_inode)
    };
    let device = Path::new(&identity.topology);
    if old_object_gone(device, &identity.generation)? {
        return Ok(true);
    }
    // Configuration/alternate changes may recreate interfaces while the original
    // parent USB device still exists. They do NOT prove all owned effects vanished.
    Ok(false)
}

#[cfg(test)]
pub(super) fn test_device() -> Device {
    let generation = Generation {
        filesystem: 10,
        directory_inode: 100,
        attribute_inode: 101,
        attribute_ctime: 42,
        attribute_ctime_nsec: 123,
    };
    let identity = DeviceIdentity {
        boot_id: "01234567-89ab-cdef-0123-456789abcdef".to_owned(),
        name: "1-6".to_owned(),
        topology: "/sys/devices/pci0000:00/usb1/1-6".to_owned(),
        generation: generation.clone(),
        busnum: 1,
        devnum: 2,
        descriptors: vec![
            18, 1, 0, 2, 0, 0, 0, 64, 0xd3, 0x13, 0x11, 0x5a, 0, 1, 0, 0, 0, 1, 9, 2, 27, 0, 2, 1,
            0, 128, 50, 9, 4, 0, 0, 0, 0x0e, 1, 0, 0, 9, 4, 1, 0, 0, 0x0e, 2, 0, 0,
        ],
        serial: None,
        interfaces: vec![
            InterfaceIdentity {
                name: "1-6:1.0".to_owned(),
                subclass: 1,
                number: 0,
                protocol: 0,
                generation: Generation {
                    directory_inode: 110,
                    attribute_inode: 111,
                    ..generation.clone()
                },
            },
            InterfaceIdentity {
                name: "1-6:1.1".to_owned(),
                subclass: 2,
                number: 1,
                protocol: 0,
                generation: Generation {
                    directory_inode: 120,
                    attribute_inode: 121,
                    ..generation
                },
            },
        ],
    };
    let bindings = identity
        .interfaces
        .iter()
        .map(|interface| (interface.name.clone(), Some("uvcvideo".to_owned())))
        .collect();
    Device { identity, bindings }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_serial_port_and_descriptor_match_does_not_adopt_replug() {
        let device = test_device();
        let original = device.identity.clone();
        let mut replacement = device;
        replacement.identity.generation.directory_inode += 1;
        replacement.identity.devnum += 1;
        let inventory = Inventory {
            boot_id: original.boot_id.clone(),
            devices: vec![replacement],
            unsupported: vec![],
        };
        assert!(matching(&inventory, &original).is_err());
    }

    #[test]
    fn boot_descriptor_and_interface_generation_are_all_identity() {
        let original = test_device().identity;
        for kind in 0..4 {
            let mut current = test_device();
            match kind {
                0 => current.identity.descriptors[0] ^= 1,
                1 => current.identity.interfaces[1].generation.directory_inode += 1,
                2 => current.identity.serial = Some("replacement".to_owned()),
                _ => current.identity.boot_id = "11234567-89ab-cdef-0123-456789abcdef".to_owned(),
            }
            let inventory = Inventory {
                boot_id: current.identity.boot_id.clone(),
                devices: vec![current],
                unsupported: vec![],
            };
            assert!(matching(&inventory, &original).is_err());
        }
    }

    #[test]
    fn journal_names_cannot_supply_paths_or_other_interfaces() {
        let mut identity = test_device().identity;
        identity.interfaces[0].name = "../../driver/unbind".to_owned();
        assert!(identity.validate().is_err());
        identity = test_device().identity;
        identity.topology = "/tmp/1-6".to_owned();
        assert!(identity.validate().is_err());
    }
}

#[cfg(test)]
mod configuration_tests {
    use super::*;

    fn add_interface(identity: &mut DeviceIdentity, descriptor: &[u8; 9], new_number: bool) {
        identity.descriptors.extend_from_slice(descriptor);
        let total = u16::try_from(identity.descriptors.len() - 18)
            .unwrap()
            .to_le_bytes();
        identity.descriptors[20..22].copy_from_slice(&total);
        if new_number {
            identity.descriptors[22] += 1;
        }
    }

    #[test]
    fn single_configuration_video_roles_can_coexist_with_untouched_audio() {
        let mut identity = test_device().identity;
        assert!(supported_configuration(&identity).is_ok());
        add_interface(&mut identity, &[9, 4, 2, 0, 0, 1, 1, 0, 0], true);
        assert!(supported_configuration(&identity).is_ok());
    }

    #[test]
    fn every_alternate_must_keep_the_selected_video_role() {
        let mut identity = test_device().identity;
        add_interface(&mut identity, &[9, 4, 1, 1, 0, 0x0e, 2, 0, 0], false);
        assert!(supported_configuration(&identity).is_ok());
        let alternate = identity.descriptors.len() - 9;
        identity.descriptors[alternate + 5] = 1;
        assert!(supported_configuration(&identity).is_err());
        identity.descriptors[alternate + 5] = 0x0e;
        identity.descriptors[alternate + 6] = 1;
        assert!(supported_configuration(&identity).is_err());
    }

    #[test]
    fn multi_configuration_or_ambiguous_descriptor_extents_are_refused() {
        let mut identity = test_device().identity;
        identity.descriptors[17] = 2;
        assert!(supported_configuration(&identity).is_err());
        identity = test_device().identity;
        identity.descriptors.pop();
        assert!(supported_configuration(&identity).is_err());
        identity = test_device().identity;
        add_interface(&mut identity, &[9, 4, 1, 0, 0, 0x0e, 2, 0, 0], false);
        assert!(supported_configuration(&identity).is_err());
    }

    #[test]
    fn current_configuration_and_interface_number_must_match_descriptor_roles() {
        let mut identity = test_device().identity;
        identity.interfaces[1].name = "1-6:2.1".to_owned();
        assert!(supported_configuration(&identity).is_err());
        identity = test_device().identity;
        identity.interfaces[1].number = 2;
        assert!(supported_configuration(&identity).is_err());
    }
}
