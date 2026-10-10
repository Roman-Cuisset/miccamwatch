//! Temporary elevated camera-class coordinator. Wire commands contain only intent,
//! never device IDs, paths, executables, or arbitrary native operations.
use super::*;
use anyhow::ensure;
use serde::{Deserialize, Serialize};
use std::{
    ffi::c_void,
    path::PathBuf,
    ptr,
    sync::{Arc, Mutex, OnceLock, atomic::AtomicBool},
    time::{Duration, Instant},
};
use windows::Win32::{
    System::Console::FreeConsole,
    UI::Shell::{
        SEE_MASK_NO_CONSOLE, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
        ShellExecuteExW,
    },
};

const MAX_CAMERAS: usize = 4096;
const MAX_JOURNAL_BYTES: u64 = 2 * 1024 * 1024;
const MAX_INSTANCE_ID_UNITS: usize = 200; // SDK MAX_DEVICE_ID_LEN, including NUL
const CAMERA_GUID: GUID = GUID::from_u128(0xca3e7ab9_b4c3_4ae6_8251_579ef933890f);
const IMAGE_GUID: GUID = GUID::from_u128(0x6bdd1fc6_810f_11d0_bec7_08002be2092f);
type Raw = *mut c_void;
#[repr(C)]
struct SecurityAttributes {
    size: u32,
    descriptor: Raw,
    inherit: i32,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct DeviceInfo {
    size: u32,
    class: GUID,
    devinst: u32,
    reserved: usize,
}
#[repr(C)]
struct PropertyChange {
    header_size: u32,
    function: u32,
    state: u32,
    scope: u32,
    profile: u32,
}
#[link(name = "setupapi")]
unsafe extern "system" {
    fn SetupDiCreateDeviceInfoList(class: *const GUID, window: Raw) -> Raw;
    fn SetupDiOpenDeviceInfoW(
        set: Raw,
        id: *const u16,
        window: Raw,
        flags: u32,
        info: *mut DeviceInfo,
    ) -> i32;
    fn SetupDiDestroyDeviceInfoList(set: Raw) -> i32;
    fn SetupDiSetClassInstallParamsW(
        set: Raw,
        info: *mut DeviceInfo,
        params: *const PropertyChange,
        size: u32,
    ) -> i32;
    fn SetupDiCallClassInstaller(function: u32, set: Raw, info: *mut DeviceInfo) -> i32;
    fn SetupDiGetDeviceInstanceIdW(
        set: Raw,
        info: *mut DeviceInfo,
        buffer: *mut u16,
        size: u32,
        needed: *mut u32,
    ) -> i32;
    fn SetupDiGetDeviceRegistryPropertyW(
        set: Raw,
        info: *mut DeviceInfo,
        property: u32,
        kind: *mut u32,
        buffer: *mut u8,
        size: u32,
        needed: *mut u32,
    ) -> i32;
    fn SetupDiSetDeviceRegistryPropertyW(
        set: Raw,
        info: *mut DeviceInfo,
        property: u32,
        buffer: *const u8,
        size: u32,
    ) -> i32;
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentProcess() -> Raw;
    fn CloseHandle(handle: Raw) -> i32;
    fn LocalFree(memory: Raw) -> Raw;
    fn CreateDirectoryW(path: *const u16, attributes: *const SecurityAttributes) -> i32;
    fn CreateFileW(
        path: *const u16,
        access: u32,
        share: u32,
        attributes: *const SecurityAttributes,
        creation: u32,
        flags: u32,
        template: Raw,
    ) -> Raw;
    fn GetFileInformationByHandleEx(handle: Raw, class: u32, buffer: Raw, size: u32) -> i32;
}
#[link(name = "shell32")]
unsafe extern "system" {
    fn SHGetKnownFolderPath(
        folder: *const GUID,
        flags: u32,
        token: Raw,
        path: *mut *mut u16,
    ) -> i32;
}
#[link(name = "ole32")]
unsafe extern "system" {
    fn CoTaskMemFree(memory: Raw);
}
#[repr(C)]
struct NotifyFilter {
    size: u32,
    flags: u32,
    kind: u32,
    reserved: u32,
    data: [u64; 50],
}
#[link(name = "cfgmgr32")]
unsafe extern "system" {
    fn CM_Register_Notification(
        filter: *const NotifyFilter,
        context: Raw,
        callback: unsafe extern "system" fn(Raw, Raw, u32, Raw, u32) -> u32,
        notification: *mut Raw,
    ) -> u32;
    fn CM_Unregister_Notification(notification: Raw) -> u32;
}
struct Wake {
    dirty: AtomicBool,
    thread: OnceLock<std::thread::Thread>,
}
unsafe extern "system" fn pnp_changed(_: Raw, context: Raw, _: u32, _: Raw, _: u32) -> u32 {
    let wake = unsafe { &*(context.cast::<Wake>()) };
    wake.dirty.store(true, Ordering::Release);
    if let Some(thread) = wake.thread.get() {
        thread.unpark();
    }
    0
}
struct Notification {
    raw: Raw,
    _wake: Arc<Wake>,
}
impl Notification {
    fn register(wake: &Arc<Wake>) -> Result<Self> {
        let filter = NotifyFilter {
            size: size_of::<NotifyFilter>() as u32,
            flags: 2,
            kind: 2,
            reserved: 0,
            data: [0; 50],
        };
        let mut raw = ptr::null_mut();
        let result = unsafe {
            CM_Register_Notification(
                &filter,
                Arc::as_ptr(wake).cast_mut().cast(),
                pnp_changed,
                &mut raw,
            )
        };
        ensure!(
            result == 0,
            "PnP callback unavailable (CM error {result}); camera enforcement uses the two-second fallback poll"
        );
        Ok(Self {
            raw,
            _wake: Arc::clone(wake),
        })
    }
    fn unregister(&mut self) -> Result<()> {
        if self.raw.is_null() {
            return Ok(());
        }
        let result = unsafe { CM_Unregister_Notification(self.raw) };
        self.raw = ptr::null_mut();
        if result != 0 {
            // If Windows cannot confirm callback retirement, retain its tiny
            // context until process exit rather than risk a callback UAF.
            std::mem::forget(Arc::clone(&self._wake));
            bail!("Windows could not retire camera PnP callbacks: CM error {result}");
        }
        Ok(())
    }
}
impl Drop for Notification {
    fn drop(&mut self) {
        let _ = self.unregister();
    }
}
#[link(name = "advapi32")]
unsafe extern "system" {
    fn OpenProcessToken(process: Raw, access: u32, token: *mut Raw) -> i32;
    fn GetTokenInformation(
        token: Raw,
        class: u32,
        buffer: Raw,
        length: u32,
        needed: *mut u32,
    ) -> i32;
    fn ConvertSidToStringSidW(sid: Raw, text: *mut *mut u16) -> i32;
    fn ConvertStringSecurityDescriptorToSecurityDescriptorW(
        text: *const u16,
        revision: u32,
        descriptor: *mut Raw,
        size: *mut u32,
    ) -> i32;
    fn GetSecurityInfo(
        handle: Raw,
        object_type: u32,
        information: u32,
        owner: *mut Raw,
        group: *mut Raw,
        dacl: *mut Raw,
        sacl: *mut Raw,
        descriptor: *mut Raw,
    ) -> u32;
    fn GetAce(acl: Raw, index: u32, ace: *mut Raw) -> i32;
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
fn check(result: i32) -> Result<()> {
    ensure!(
        result != 0,
        "native camera operation: {}",
        std::io::Error::last_os_error()
    );
    Ok(())
}
struct Handle(Raw);
impl Handle {
    fn new(raw: Raw) -> Result<Self> {
        if raw.is_null() || raw as isize == -1 {
            return Err(std::io::Error::last_os_error()).context("native camera handle");
        }
        Ok(Self(raw))
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
struct Local(Raw);
impl Drop for Local {
    fn drop(&mut self) {
        unsafe {
            LocalFree(self.0);
        }
    }
}
fn token_identity(require_elevated: bool) -> Result<String> {
    let mut raw = ptr::null_mut();
    unsafe {
        check(OpenProcessToken(GetCurrentProcess(), 8, &mut raw))?;
    }
    let token = Handle::new(raw)?;
    let mut needed = 0;
    if require_elevated {
        let mut elevated = 0u32;
        unsafe {
            check(GetTokenInformation(
                token.0,
                20,
                (&mut elevated as *mut u32).cast(),
                4,
                &mut needed,
            ))?;
        }
        ensure!(
            elevated != 0,
            "camera helper requires an actually elevated administrator token"
        );
    }
    unsafe {
        GetTokenInformation(token.0, 1, ptr::null_mut(), 0, &mut needed);
    }
    ensure!(
        needed > 0 && needed < 65536,
        "invalid camera user token size"
    );
    let mut data = vec![0usize; (needed as usize).div_ceil(size_of::<usize>())];
    unsafe {
        check(GetTokenInformation(
            token.0,
            1,
            data.as_mut_ptr().cast(),
            needed,
            &mut needed,
        ))?;
    }
    let sid = unsafe { *(data.as_ptr().cast::<Raw>()) };
    let mut text = ptr::null_mut();
    unsafe {
        check(ConvertSidToStringSidW(sid, &mut text))?;
    }
    let _text = Local(text.cast());
    let mut len = 0;
    while len < 256 && unsafe { *text.add(len) } != 0 {
        len += 1;
    }
    ensure!(len < 256, "camera SID is oversized");
    Ok(String::from_utf16(unsafe {
        std::slice::from_raw_parts(text, len)
    })?)
}

fn sid_text(sid: Raw) -> Result<String> {
    let mut text = ptr::null_mut();
    unsafe {
        check(ConvertSidToStringSidW(sid, &mut text))?;
    }
    let _text = Local(text.cast());
    let mut length = 0;
    while length < 256 && unsafe { *text.add(length) } != 0 {
        length += 1;
    }
    ensure!(length < 256, "oversized security SID");
    Ok(String::from_utf16(unsafe {
        std::slice::from_raw_parts(text, length)
    })?)
}
fn trusted_owner(sid: &str) -> bool {
    matches!(
        sid,
        "S-1-5-18"
            | "S-1-5-32-544"
            | "S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464"
    )
}
fn inspect_security(
    path: &Path,
    user: Option<&str>,
    shared: bool,
    ancestor: bool,
) -> Result<Local> {
    let name: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let handle = Handle::new(unsafe {
        CreateFileW(
            name.as_ptr(),
            0x20080,
            7,
            ptr::null(),
            3,
            0x42200000,
            ptr::null_mut(),
        )
    })?;
    #[repr(C)]
    struct AttributeTag {
        attributes: u32,
        tag: u32,
    }
    let mut tag = AttributeTag {
        attributes: 0,
        tag: 0,
    };
    unsafe {
        check(GetFileInformationByHandleEx(
            handle.0,
            9,
            (&mut tag as *mut AttributeTag).cast(),
            size_of::<AttributeTag>() as u32,
        ))?;
    }
    ensure!(
        tag.attributes & 0x400 == 0,
        "camera journal path is a reparse point: {}",
        path.display()
    );
    let (mut acl, mut owner, mut descriptor) = (ptr::null_mut(), ptr::null_mut(), ptr::null_mut());
    let result = unsafe {
        GetSecurityInfo(
            handle.0,
            1,
            5,
            &mut owner,
            ptr::null_mut(),
            &mut acl,
            ptr::null_mut(),
            &mut descriptor,
        )
    };
    ensure!(
        result == 0,
        "cannot inspect protected camera journal security: {result}"
    );
    let security = Local(descriptor);
    let owner = sid_text(owner)?;
    ensure!(
        if ancestor {
            trusted_owner(&owner)
        } else {
            matches!(owner.as_str(), "S-1-5-18" | "S-1-5-32-544")
        },
        "camera journal ancestor/file is not owned by administrators or SYSTEM: {}",
        path.display()
    );
    ensure!(!acl.is_null(), "camera journal cannot use a null DACL");
    let count = unsafe { *(acl.cast::<u16>().add(2)) };
    for index in 0..u32::from(count) {
        let mut raw = ptr::null_mut();
        unsafe {
            check(GetAce(acl, index, &mut raw))?;
        }
        let ace = raw.cast::<u8>();
        let kind = unsafe { *ace };
        let flags = unsafe { *ace.add(1) };
        // Standard Windows ancestors may carry creator-owner inheritance for
        // new children. Our explicitly protected app directories never inherit it.
        if ancestor && flags & 8 != 0 {
            continue;
        }
        if kind == 1 {
            continue;
        } // deny ACE grants no privilege
        ensure!(
            kind == 0,
            "unsupported ACE in protected camera journal path"
        );
        let access = unsafe { ptr::read_unaligned(ace.add(4).cast::<u32>()) };
        let sid = sid_text(unsafe { ace.add(8).cast() })?;
        let administrator = matches!(sid.as_str(), "S-1-5-18" | "S-1-5-32-544")
            || (ancestor && trusted_owner(&sid));
        if administrator {
            continue;
        }
        let dangerous = ordinary_dangerous_access(ancestor);
        ensure!(
            access & dangerous == 0,
            "ordinary user/group can write/delete/replace a camera journal path"
        );
        if !ancestor {
            ensure!(
                user == Some(sid.as_str()) || (shared && sid == "S-1-5-32-545"),
                "unexpected ordinary-user ACE in camera journal path"
            );
            let allowed = if shared { 0x000200a0 } else { 0x001200a9 };
            ensure!(
                access & !allowed == 0,
                "camera journal ordinary-user ACL is not read/traverse only"
            );
        }
    }
    Ok(security)
}
fn ordinary_dangerous_access(ancestor: bool) -> u32 {
    if ancestor {
        0x100d0040 | 0x40000000
    } else {
        0x100d0156 | 0x40000000
    }
}
fn create_protected_directory(path: &Path, user: Option<&str>, create: bool) -> Result<()> {
    if create {
        let ordinary = user.map_or_else(
            || "(A;;0x000200a0;;;BU)".to_owned(),
            |sid| format!("(A;OICI;GRGX;;;{sid})"),
        );
        let sddl = wide(&format!(
            "O:BAG:BAD:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA){ordinary}"
        ));
        let mut descriptor = ptr::null_mut();
        unsafe {
            check(ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                1,
                &mut descriptor,
                ptr::null_mut(),
            ))?;
        }
        let _descriptor = Local(descriptor);
        let attributes = SecurityAttributes {
            size: size_of::<SecurityAttributes>() as u32,
            descriptor,
            inherit: 0,
        };
        let name: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        if unsafe { CreateDirectoryW(name.as_ptr(), &attributes) } == 0 {
            ensure!(
                std::io::Error::last_os_error().raw_os_error() == Some(183),
                "cannot create protected camera journal directory: {}",
                std::io::Error::last_os_error()
            );
        }
    }
    inspect_security(path, user, user.is_none(), false)?;
    Ok(())
}
fn protected_path(
    identity: &crate::windows_control::ControlIdentity,
    create: bool,
) -> Result<PathBuf> {
    const PROGRAM_DATA: GUID = GUID::from_u128(0x62ab5d82_fdc1_4dc3_a9dd_070d1d495d97);
    let mut raw = ptr::null_mut();
    let result = unsafe { SHGetKnownFolderPath(&PROGRAM_DATA, 0, ptr::null_mut(), &mut raw) };
    ensure!(
        result >= 0 && !raw.is_null(),
        "native ProgramData known folder is unavailable"
    );
    struct FolderMemory(*mut u16);
    impl Drop for FolderMemory {
        fn drop(&mut self) {
            unsafe {
                CoTaskMemFree(self.0.cast());
            }
        }
    }
    let _memory = FolderMemory(raw);
    let mut length = 0;
    while length < 32768 && unsafe { *raw.add(length) } != 0 {
        length += 1;
    }
    ensure!(length < 32768, "native ProgramData path exceeds limit");
    let mut root = PathBuf::from(String::from_utf16(unsafe {
        std::slice::from_raw_parts(raw, length)
    })?);
    for ancestor in root.ancestors() {
        if !ancestor.as_os_str().is_empty() {
            inspect_security(ancestor, None, false, true)?;
        }
    }
    root.push("MicCamWatch");
    create_protected_directory(&root, None, create)?;
    root.push("CameraGuard");
    create_protected_directory(&root, None, create)?;
    root.push(format!(
        "{}-{}-{}",
        identity.user, identity.session, identity.scope
    ));
    create_protected_directory(&root, Some(&identity.user), create)?;
    Ok(root.join("journal.json"))
}

struct NativeDevice {
    set: Raw,
    info: DeviceInfo,
    id: String,
}
impl Drop for NativeDevice {
    fn drop(&mut self) {
        unsafe {
            SetupDiDestroyDeviceInfoList(self.set);
        }
    }
}
impl NativeDevice {
    fn open(id: &str) -> Result<Option<Self>> {
        ensure!(
            !id.is_empty()
                && id.encode_utf16().count() < MAX_INSTANCE_ID_UNITS
                && !id.contains('\0'),
            "native camera instance ID exceeds SDK MAX_DEVICE_ID_LEN"
        );
        let set = unsafe { SetupDiCreateDeviceInfoList(ptr::null(), ptr::null_mut()) };
        ensure!(
            set as isize != -1 && !set.is_null(),
            "cannot create native camera device information set"
        );
        let mut device = Self {
            set,
            info: DeviceInfo {
                size: size_of::<DeviceInfo>() as u32,
                class: GUID::zeroed(),
                devinst: 0,
                reserved: 0,
            },
            id: id.to_owned(),
        };
        if unsafe {
            SetupDiOpenDeviceInfoW(set, wide(id).as_ptr(), ptr::null_mut(), 0, &mut device.info)
        } == 0
        {
            let error = std::io::Error::last_os_error().raw_os_error().unwrap_or(0) as u32;
            if matches!(error, 0xe000020b | 433 | 1168) {
                return Ok(None);
            }
            bail!("cannot open native camera identity {id}: Windows error {error:#x}");
        }
        ensure!(
            device.info.class == CAMERA_GUID || device.info.class == IMAGE_GUID,
            "recorded identity is not a Camera/Image class device"
        );
        let mut buffer = [0u16; MAX_INSTANCE_ID_UNITS];
        let mut needed = 0;
        unsafe {
            check(SetupDiGetDeviceInstanceIdW(
                set,
                &mut device.info,
                buffer.as_mut_ptr(),
                buffer.len() as u32,
                &mut needed,
            ))?;
        }
        ensure!(
            needed > 0 && needed as usize <= buffer.len(),
            "native camera instance ID exceeds limit"
        );
        let canonical = String::from_utf16(&buffer[..needed as usize - 1])?;
        ensure!(
            canonical.eq_ignore_ascii_case(id),
            "native camera identity changed"
        );
        device.id = canonical;
        Ok(Some(device))
    }
    fn status(&self) -> Result<NativeStatus> {
        if !self.present()? {
            return Ok(NativeStatus::Absent);
        }
        let (mut flags, mut problem) = (0, 0);
        let result =
            unsafe { CM_Get_DevNode_Status(&mut flags, &mut problem, self.info.devinst, 0) };
        if result == 0x0d {
            return Ok(NativeStatus::Absent);
        } // CR_NO_SUCH_DEVNODE only
        ensure!(
            result == 0,
            "unknown native camera status: configuration manager error {result}"
        );
        Ok(if matches!(problem, 24 | 45) {
            NativeStatus::Absent
        } else if problem == 22 {
            NativeStatus::Disabled
        } else if problem == 0 && flags & 8 != 0 {
            NativeStatus::Running
        } else {
            NativeStatus::Unknown
        })
    }
    fn present(&self) -> Result<bool> {
        let mut current = 0;
        let result = unsafe { CM_Locate_DevNodeW(&mut current, wide(&self.id).as_ptr(), 0) };
        if result == 0x0d {
            return Ok(false);
        }
        ensure!(
            result == 0,
            "unknown native camera presence: configuration manager error {result}"
        );
        ensure!(
            current == self.info.devinst,
            "native camera devnode changed during presence readback"
        );
        Ok(true)
    }
    fn validate_identity_class(&mut self) -> Result<()> {
        let mut identity = [0u16; MAX_INSTANCE_ID_UNITS];
        let mut needed = 0;
        unsafe {
            check(SetupDiGetDeviceInstanceIdW(
                self.set,
                &mut self.info,
                identity.as_mut_ptr(),
                identity.len() as u32,
                &mut needed,
            ))?;
        }
        ensure!(
            needed > 0 && needed as usize <= identity.len(),
            "native camera identity readback exceeds limit"
        );
        let actual = &identity[..needed as usize - 1];
        ensure!(
            identity[needed as usize - 1] == 0
                && actual.len() == self.id.encode_utf16().count()
                && actual
                    .iter()
                    .copied()
                    .zip(self.id.encode_utf16())
                    .all(|(a, b)| a == b
                        || (a < 128 && b < 128 && (a as u8).eq_ignore_ascii_case(&(b as u8)))),
            "native camera identity changed before privileged operation"
        );
        let mut class = [0u16; 64];
        let mut kind = 0;
        unsafe {
            check(SetupDiGetDeviceRegistryPropertyW(
                self.set,
                &mut self.info,
                8,
                &mut kind,
                class.as_mut_ptr().cast(),
                (class.len() * 2) as u32,
                &mut needed,
            ))?;
        }
        let expected = class_text(self.info.class);
        ensure!(
            (self.info.class == CAMERA_GUID || self.info.class == IMAGE_GUID)
                && kind == 1
                && needed as usize == (expected.len() + 1) * 2
                && class[expected.len()] == 0
                && class[..expected.len()]
                    .iter()
                    .copied()
                    .zip(expected.bytes())
                    .all(|(native, expected)| native < 128
                        && (native as u8).eq_ignore_ascii_case(&expected)),
            "native camera class changed before privileged operation"
        );
        Ok(())
    }
    fn configuration_flags(&mut self) -> Result<u32> {
        let (mut flags, mut kind, mut needed) = (0u32, 0, 0);
        let result = unsafe {
            SetupDiGetDeviceRegistryPropertyW(
                self.set,
                &mut self.info,
                10,
                &mut kind,
                (&mut flags as *mut u32).cast(),
                4,
                &mut needed,
            )
        };
        if result == 0 && std::io::Error::last_os_error().raw_os_error() == Some(13) {
            return Ok(0);
        }
        check(result)?;
        ensure!(
            kind == 4 && needed == 4,
            "invalid native camera CONFIGFLAGS property"
        );
        Ok(flags)
    }
    fn disabled_configuration(&mut self) -> Result<bool> {
        Ok(self.configuration_flags()? & 1 != 0)
    }
    fn enable_offline_configuration(&mut self) -> Result<()> {
        ensure!(
            !self.present()?,
            "camera became present before offline configuration restore"
        );
        let original = self.configuration_flags()?;
        let enabled = allow_configuration_flags(original);
        if enabled != original {
            self.validate_identity_class()?;
            ensure!(
                !self.present()?,
                "camera arrived before offline configuration restore"
            );
            // Documented native PnP property API; only the owned disabled bit,
            // never arbitrary registry paths, classes, or property IDs.
            unsafe {
                check(SetupDiSetDeviceRegistryPropertyW(
                    self.set,
                    &mut self.info,
                    10,
                    (&enabled as *const u32).cast(),
                    4,
                ))?;
            }
        }
        ensure!(
            !self.disabled_configuration()?,
            "Windows did not clear the owned phantom disabled configuration"
        );
        Ok(())
    }
    fn change(&mut self, enable: bool) -> Result<()> {
        self.change_before(enable, |_| Ok(()))
    }
    fn change_before(
        &mut self,
        enable: bool,
        before_native: impl FnOnce(&str) -> Result<()>,
    ) -> Result<()> {
        self.validate_identity_class()?;
        // Fixed property-change operation, global config (also attempts phantom
        // configurations). No caller-selected install function, class, or ID.
        let params = PropertyChange {
            header_size: 8,
            function: 0x12,
            state: if enable { 1 } else { 2 },
            scope: 1,
            profile: 0,
        };
        unsafe {
            check(SetupDiSetClassInstallParamsW(
                self.set,
                &mut self.info,
                &params,
                size_of::<PropertyChange>() as u32,
            ))?;
        }
        if !enable {
            ensure!(
                self.status()? == NativeStatus::Running,
                "native camera changed before disable"
            );
        }
        before_native(&self.id)?;
        unsafe {
            check(SetupDiCallClassInstaller(0x12, self.set, &mut self.info))?;
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum NativeStatus {
    Running,
    Disabled,
    ConfiguredDisabledRunning,
    Absent,
    Unknown,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnedDevice {
    id: String,
    class: String,
    generation: u64,
    explicit_legacy: bool,
    prepared: bool,
}
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Journal {
    generation: u64,
    requested: bool,
    owned: Vec<OwnedDevice>,
    #[serde(default)]
    migration_receipts: Vec<MigrationReceipt>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MigrationReceipt {
    id: String,
    class: String,
    generation: u64,
    fulfilled: bool,
}
fn disable_authorized(
    requested: bool,
    active: bool,
    current: u64,
    expected: u64,
    status: NativeStatus,
) -> Result<bool> {
    ensure!(
        requested && active && current == expected,
        "camera block generation is no longer authoritative"
    );
    ensure!(
        status != NativeStatus::Unknown,
        "camera native status is unknown"
    );
    Ok(status == NativeStatus::Running)
}
fn restore_fulfilled(status: NativeStatus, disabled_config: bool) -> bool {
    !disabled_config && matches!(status, NativeStatus::Running | NativeStatus::Absent)
}
fn restoration_needed(status: Option<NativeStatus>, disabled_config: bool) -> Result<bool> {
    let Some(status) = status else {
        return Ok(false);
    };
    ensure!(
        status != NativeStatus::Unknown,
        "owed native camera restore status is unknown"
    );
    Ok(!restore_fulfilled(status, disabled_config))
}
fn allow_configuration_flags(flags: u32) -> u32 {
    flags & !1
}
fn aggregate_state(
    requested: bool,
    active: bool,
    unknown: usize,
    pending: usize,
    all_disabled: bool,
    all_running: bool,
) -> CameraPrivacyState {
    if unknown > 0 {
        CameraPrivacyState::SystemManaged
    } else if requested && active && all_disabled {
        CameraPrivacyState::Blocked
    } else if !requested && pending == 0 && all_running {
        CameraPrivacyState::Allowed
    } else {
        CameraPrivacyState::SystemManaged
    }
}
impl Journal {
    fn advance(&mut self, blocked: bool, generation: u64) -> Result<()> {
        ensure!(
            generation > self.generation,
            "camera intent generation was superseded"
        );
        self.generation = generation;
        self.requested = blocked;
        Ok(())
    }
    fn load(path: &Path, user: &str) -> Result<Self> {
        match fs::metadata(path) {
            Ok(metadata) => {
                ensure!(
                    metadata.len() <= MAX_JOURNAL_BYTES,
                    "privileged camera journal exceeds 2 MiB"
                );
                inspect_security(path, Some(user), false, false)?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(error) => return Err(error).context("unknown privileged camera journal metadata"),
        }
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(e).context("cannot read privileged camera ownership journal"),
        };
        ensure!(
            bytes.len() as u64 <= MAX_JOURNAL_BYTES,
            "privileged camera journal exceeds 2 MiB"
        );
        let journal: Self = serde_json::from_slice(&bytes)
            .context("invalid privileged camera ownership journal")?;
        journal.validate()?;
        Ok(journal)
    }
    fn validate(&self) -> Result<()> {
        let valid_identity = |id: &str, class: &str, generation: u64| {
            !id.is_empty()
                && !id.contains('\0')
                && id.encode_utf16().count() < MAX_INSTANCE_ID_UNITS
                && generation > 0
                && generation <= self.generation
                && matches!(class, CAMERA_CLASS | IMAGE_CLASS)
        };
        ensure!(
            self.owned.len() <= MAX_CAMERAS,
            "privileged camera journal exceeds explicit 4096-device limit"
        );
        ensure!(
            self.owned.iter().all(|entry| valid_identity(
                &entry.id,
                &entry.class,
                entry.generation
            )),
            "invalid privileged camera ownership identity/generation/class"
        );
        ensure!(
            self.owned
                .iter()
                .all(|entry| !entry.prepared || !entry.explicit_legacy),
            "explicit migration authority cannot be an unattempted Block preparation"
        );
        ensure!(
            self.migration_receipts.len() <= MAX_CAMERAS
                && self.migration_receipts.iter().all(|receipt| valid_identity(
                    &receipt.id,
                    &receipt.class,
                    receipt.generation
                )),
            "invalid protected legacy migration receipts"
        );
        ensure!(
            self.owned
                .iter()
                .filter(|entry| entry.explicit_legacy)
                .all(|entry| self
                    .migration_receipts
                    .iter()
                    .any(|receipt| !receipt.fulfilled
                        && receipt.id.eq_ignore_ascii_case(&entry.id)
                        && receipt.class == entry.class
                        && receipt.generation == entry.generation)),
            "legacy restoration lacks an explicit unfulfilled protected migration receipt"
        );
        Ok(())
    }
    fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;
        let bytes = serde_json::to_vec(self)?;
        ensure!(
            bytes.len() as u64 <= MAX_JOURNAL_BYTES,
            "camera journal exceeds 2 MiB"
        );
        atomic_bytes(path, &bytes)
    }
}
fn atomic_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    static NONCE: AtomicU64 = AtomicU64::new(0);
    let temp = path.with_extension(format!(
        "tmp.{}.{}",
        std::process::id(),
        NONCE.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| -> Result<()> {
        use std::os::windows::io::FromRawHandle;
        let security = inspect_security(
            path.parent().context("camera journal has no parent")?,
            None,
            false,
            true,
        )?;
        let attributes = SecurityAttributes {
            size: size_of::<SecurityAttributes>() as u32,
            descriptor: security.0,
            inherit: 0,
        };
        let name: Vec<u16> = temp.as_os_str().encode_wide().chain(Some(0)).collect();
        let handle = Handle::new(unsafe {
            CreateFileW(
                name.as_ptr(),
                0x40000000,
                0,
                &attributes,
                1,
                0x00200080,
                ptr::null_mut(),
            )
        })?;
        let mut file = unsafe { fs::File::from_raw_handle(handle.0) };
        std::mem::forget(handle);
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        let from: Vec<u16> = temp.as_os_str().encode_wide().chain(Some(0)).collect();
        let to: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        unsafe {
            MoveFileExW(
                PCWSTR(from.as_ptr()),
                PCWSTR(to.as_ptr()),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        }?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result.context("cannot persist privileged camera ownership")
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
enum Request {
    Status,
    Intent { blocked: bool, generation: u64 },
    ReconcileAllow { generation: u64 },
    FinishAllow { generation: u64 },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Response {
    observation: CameraControlObservation,
    generation: u64,
    completed_generation: u64,
    retiring: bool,
    error: Option<String>,
}
struct ControlState {
    generation: u64,
    blocked: bool,
    armed: bool,
    completed_generation: u64,
    observation: CameraControlObservation,
    error: Option<String>,
}
impl ControlState {
    fn accept(&mut self, blocked: bool, generation: u64) -> Result<()> {
        ensure!(
            generation > self.generation,
            "camera intent generation was superseded"
        );
        self.generation = generation;
        self.blocked = blocked;
        self.armed = true;
        self.error = None;
        self.observation.desired_blocked = blocked;
        self.observation.state = CameraPrivacyState::SystemManaged;
        self.observation.helper_active = true;
        self.observation.detail = Some(
            "Camera intent accepted; native enforcement/restoration is in progress.".to_owned(),
        );
        Ok(())
    }
    fn reconcile_allow(&mut self, generation: u64) -> Result<()> {
        ensure!(
            !self.armed && !self.blocked && self.generation == generation,
            "automatic camera restoration was superseded or is already in progress"
        );
        self.armed = true;
        self.error = None;
        self.observation.state = CameraPrivacyState::SystemManaged;
        self.observation.helper_active = true;
        self.observation.detail = Some(
            "Existing global Allow is being reconciled; no new camera intent was selected."
                .to_owned(),
        );
        Ok(())
    }
    fn retiring(&self) -> bool {
        self.armed && !self.blocked && self.completed_generation >= self.generation
    }
    fn response(&self) -> Response {
        Response {
            observation: self.observation.clone(),
            generation: self.generation,
            completed_generation: self.completed_generation,
            retiring: self.retiring(),
            error: self.error.clone(),
        }
    }
}
struct Coordinator {
    journal: Journal,
    path: PathBuf,
    control: Arc<Mutex<ControlState>>,
    legacy: Option<LegacyTarget>,
    dirty: bool,
}
fn class_text(guid: GUID) -> &'static str {
    if guid == CAMERA_GUID {
        CAMERA_CLASS
    } else {
        IMAGE_CLASS
    }
}
fn bounded_detail(error: &anyhow::Error) -> String {
    let mut message = format!("{error:#}");
    if message.len() > 8192 {
        let mut end = 8192;
        while !message.is_char_boundary(end) {
            end -= 1;
        }
        message.truncate(end);
    }
    message
}
impl Coordinator {
    fn current(&self, epoch: u64, blocked: bool) -> Result<bool> {
        let current = self
            .control
            .lock()
            .map_err(|_| anyhow::anyhow!("camera intent lock poisoned"))?;
        Ok(current.armed && current.generation == epoch && current.blocked == blocked)
    }
    fn flush(&mut self) -> Result<()> {
        if self.dirty {
            self.journal.save(&self.path)?;
            self.dirty = false;
        }
        Ok(())
    }
    fn commit(&mut self) -> Result<()> {
        self.dirty = true;
        self.flush()
    }
    fn authorize_native_attempt(
        &mut self,
        epoch: u64,
        id: &str,
        newly_prepared: bool,
    ) -> Result<()> {
        self.flush()?;
        ensure!(
            self.current(epoch, true)?,
            "camera Block was superseded before native disable"
        );
        if newly_prepared {
            self.journal
                .owned
                .iter_mut()
                .find(|entry| entry.id.eq_ignore_ascii_case(id))
                .context("prepared ownership disappeared")?
                .prepared = false;
            self.commit()?;
        }
        self.flush()?;
        ensure!(
            self.current(epoch, true)?,
            "camera Block was superseded during ownership persistence"
        );
        Ok(())
    }
    fn withdraw_unattempted(&mut self, id: &str) -> Result<()> {
        self.journal
            .owned
            .retain(|entry| !entry.id.eq_ignore_ascii_case(id));
        self.commit()
    }
    fn enforce(&mut self, blocked: bool, epoch: u64) -> Result<()> {
        // Uncommitted live changes are never privileged authority on a retry.
        self.flush()?;
        if self.journal.owned.iter().any(|entry| entry.prepared) {
            self.journal.owned.retain(|entry| !entry.prepared);
            self.commit()?;
        }
        if epoch > self.journal.generation {
            self.journal.advance(blocked, epoch)?;
            self.commit()?;
        }
        ensure!(
            self.journal.generation == epoch && self.journal.requested == blocked,
            "privileged camera journal intent was superseded"
        );
        if !self.current(epoch, blocked)? {
            return Ok(());
        }
        if !blocked && let Some(target) = self.legacy.take() {
            let Some(native) = validate_legacy_target(&target)? else {
                bail!(
                    "Explicit legacy restoration is not fulfilled: Windows has no registered native identity for this camera; no target was imported or enabled."
                );
            };
            ensure!(
                self.journal.owned.len() < MAX_CAMERAS,
                "camera journal exceeds explicit 4096-device bound"
            );
            ensure!(
                self.journal.migration_receipts.len() < MAX_CAMERAS,
                "legacy migration receipts exceed explicit 4096-device bound"
            );
            if !self
                .journal
                .owned
                .iter()
                .any(|entry| entry.id.eq_ignore_ascii_case(&native.id))
            {
                self.journal.owned.push(OwnedDevice {
                    id: native.id.clone(),
                    class: target.class.clone(),
                    generation: epoch,
                    explicit_legacy: true,
                    prepared: false,
                });
            }
            self.journal.migration_receipts.push(MigrationReceipt {
                id: native.id.clone(),
                class: target.class,
                generation: epoch,
                fulfilled: false,
            });
            self.commit()?;
        }
        let mut first_error = None;
        if blocked {
            let inventory = devices()?;
            ensure!(
                inventory.len() <= MAX_CAMERAS,
                "camera inventory exceeds explicit 4096-device enforcement bound"
            );
            for camera in inventory {
                if !self.current(epoch, true)? {
                    return Ok(());
                }
                if camera.status == "Disabled" {
                    continue;
                }
                let result = (|| -> Result<()> {
                    ensure!(
                        camera.status == "OK",
                        "native camera status is unknown: {}",
                        camera.instance_id
                    );
                    let mut native = NativeDevice::open(&camera.instance_id)?
                        .context("camera disappeared before native disable")?;
                    ensure!(
                        disable_authorized(true, true, epoch, epoch, native.status()?)?,
                        "camera changed before disable"
                    );
                    if let Some(entry) = self
                        .journal
                        .owned
                        .iter()
                        .find(|entry| entry.id.eq_ignore_ascii_case(&native.id))
                    {
                        ensure!(
                            entry.class == class_text(native.info.class)
                                && entry.generation <= epoch,
                            "owned native camera class/generation changed"
                        );
                    } else {
                        ensure!(
                            !native.disabled_configuration()?,
                            "camera already has an external persistent disabled configuration; no ownership will be acquired"
                        );
                        ensure!(
                            self.journal.owned.len() < MAX_CAMERAS,
                            "camera ownership exceeds explicit 4096-device bound"
                        );
                        self.journal.owned.push(OwnedDevice {
                            id: native.id.clone(),
                            class: class_text(native.info.class).to_owned(),
                            generation: epoch,
                            explicit_legacy: false,
                            prepared: true,
                        });
                        self.commit()?;
                    }
                    let newly_prepared =
                        self.journal.owned.iter().any(|entry| {
                            entry.prepared && entry.id.eq_ignore_ascii_case(&native.id)
                        });
                    let mut attempted = false;
                    let write = native.change_before(false, |id| {
                        self.authorize_native_attempt(epoch, id, newly_prepared)?;
                        attempted = true;
                        Ok(())
                    });
                    if newly_prepared && !attempted {
                        self.withdraw_unattempted(&native.id)?;
                        return write;
                    }
                    let readback = native.status()?;
                    if write.is_err()
                        && readback == NativeStatus::Running
                        && !native.disabled_configuration()?
                    {
                        self.journal
                            .owned
                            .retain(|entry| !entry.id.eq_ignore_ascii_case(&native.id));
                        self.commit()?;
                    }
                    write?;
                    ensure!(
                        readback == NativeStatus::Disabled,
                        "Windows did not confirm camera disabled (restart/removal may be required): {}",
                        native.id
                    );
                    Ok(())
                })();
                if let Err(error) = result
                    && first_error.is_none()
                {
                    first_error = Some(error);
                }
            }
        } else {
            let mut index = 0;
            while index < self.journal.owned.len() {
                if !self.current(epoch, false)? {
                    return Ok(());
                }
                let entry = &self.journal.owned[index];
                let result = (|| -> Result<bool> {
                    let Some(mut native) = NativeDevice::open(&entry.id)? else {
                        return Ok(false);
                    };
                    ensure!(
                        entry.class == class_text(native.info.class) && entry.generation <= epoch,
                        "owned camera native class/generation changed"
                    );
                    let before = native.status()?;
                    if restore_fulfilled(before, native.disabled_configuration()?) {
                        return Ok(true);
                    }
                    ensure!(
                        before != NativeStatus::Unknown,
                        "owned camera status is unknown; no blind enable attempted"
                    );
                    if !self.current(epoch, false)? {
                        return Ok(false);
                    }
                    if before == NativeStatus::Absent {
                        native.enable_offline_configuration()?;
                    } else {
                        native.change(true)?;
                    }
                    let after = native.status()?;
                    if restore_fulfilled(after, native.disabled_configuration()?) {
                        Ok(true)
                    } else if after == NativeStatus::Absent {
                        Ok(false)
                    } else {
                        bail!(
                            "Windows did not positively confirm owned camera restoration: {}",
                            native.id
                        )
                    }
                })();
                match result {
                    Ok(true) => {
                        let restored = self.journal.owned.remove(index);
                        for receipt in &mut self.journal.migration_receipts {
                            if receipt.id.eq_ignore_ascii_case(&restored.id) {
                                receipt.fulfilled = true;
                            }
                        }
                        self.commit()?;
                    }
                    Ok(false) => index += 1,
                    Err(error) => {
                        index += 1;
                        if first_error.is_none() {
                            first_error = Some(error);
                        }
                    }
                }
            }
        }
        if let Some(error) = first_error {
            return Err(error);
        }
        Ok(())
    }
}
fn journal_record(journal: &Journal, requested: bool, generation: u64) -> CameraBlockRecord {
    let mut record = CameraBlockRecord {
        desired_blocked: requested,
        intent_generation: generation,
        ..Default::default()
    };
    let target = if requested {
        &mut record.blocked
    } else {
        &mut record.restore_on_arrival
    };
    target.extend(
        journal
            .owned
            .iter()
            .filter(|entry| !entry.prepared)
            .map(|entry| entry.id.clone()),
    );
    record
}
fn unknown_observation(blocked: bool, active: bool, detail: String) -> CameraControlObservation {
    CameraControlObservation {
        desired_blocked: blocked,
        state: CameraPrivacyState::SystemManaged,
        present_total: 0,
        owned_blocked_present: 0,
        pending_restore: 0,
        absent_owned: 0,
        unknown_devices: 1,
        helper_active: active,
        detail: Some(detail),
    }
}

fn request(command: &Request) -> Result<Response> {
    crate::windows_control::request("camera", command)
}
fn owned_blocked_count(journal: Option<&Journal>, inventory: &[CameraDevice]) -> usize {
    journal.map_or(0, |journal| {
        inventory
            .iter()
            .filter(|device| {
                device.status == "Disabled"
                    && journal.owned.iter().any(|entry| {
                        !entry.prepared && entry.id.eq_ignore_ascii_case(&device.instance_id)
                    })
            })
            .count()
    })
}
fn observe_inventory(
    record: &CameraBlockRecord,
    helper_active: bool,
    detail: Option<&str>,
    inventory: &[CameraDevice],
    probe: impl FnMut(&str) -> Result<NativeStatus>,
) -> Result<CameraControlObservation> {
    observe_entries(
        record,
        helper_active,
        detail,
        inventory,
        record
            .blocked
            .iter()
            .chain(&record.restore_on_arrival)
            .map(String::as_str),
        probe,
    )
}
fn observe_entries<'a>(
    record: &CameraBlockRecord,
    helper_active: bool,
    detail: Option<&str>,
    inventory: &[CameraDevice],
    ids: impl Iterator<Item = &'a str>,
    mut probe: impl FnMut(&str) -> Result<NativeStatus>,
) -> Result<CameraControlObservation> {
    ensure!(
        inventory.len() <= MAX_CAMERAS,
        "camera inventory exceeds explicit 4096-device observation limit"
    );
    let mut owned_blocked_present = 0;
    let mut pending_restore = 0;
    let mut absent_owned = 0;
    let mut unknown_devices = inventory
        .iter()
        .filter(|device| device.status == "Unknown")
        .count();
    let mut has_records = false;
    for id in ids {
        has_records = true;
        let present = inventory
            .iter()
            .any(|device| device.instance_id.eq_ignore_ascii_case(id));
        let state = if let Some(device) = inventory
            .iter()
            .find(|device| device.instance_id.eq_ignore_ascii_case(id))
        {
            match device.status.as_str() {
                "OK" => NativeStatus::Running,
                "Disabled" => NativeStatus::Disabled,
                "ConfiguredDisabled" => NativeStatus::ConfiguredDisabledRunning,
                _ => NativeStatus::Unknown,
            }
        } else {
            probe(id)?
        };
        if state == NativeStatus::Absent {
            absent_owned += 1;
        }
        match state {
            NativeStatus::Running => {} // fulfilled restores are not pending
            NativeStatus::ConfiguredDisabledRunning => {
                if !record.desired_blocked {
                    pending_restore += 1;
                }
            }
            NativeStatus::Disabled => {
                if inventory
                    .iter()
                    .any(|device| device.instance_id.eq_ignore_ascii_case(id))
                {
                    owned_blocked_present += 1;
                }
                if !record.desired_blocked {
                    pending_restore += 1;
                }
            }
            NativeStatus::Absent => {}
            NativeStatus::Unknown => {
                if !present {
                    unknown_devices += 1;
                }
            }
        }
    }
    let all_disabled = inventory.iter().all(|device| device.status == "Disabled");
    let all_running = inventory.iter().all(|device| device.status == "OK");
    let state = aggregate_state(
        record.desired_blocked,
        helper_active,
        unknown_devices,
        pending_restore,
        all_disabled,
        all_running,
    );
    let detail = detail.map(str::to_owned).or_else(|| {
        if record.desired_blocked && !helper_active { Some("Global Block requested, but its approved elevated helper is not running; future arrivals are not protected.".to_owned()) }
        else if record.intent_generation == 0 && has_records { Some("Legacy camera restore information is retained; unsigned historical records are not privileged ownership authority. Absent entries are informational, not a blocked-camera count.".to_owned()) }
        else if absent_owned > 0 && !record.desired_blocked { Some("Allow is requested globally. Owned offline configurations await positive Windows restoration; no permanent elevated helper remains after Allow.".to_owned()) }
        else { None }
    });
    Ok(CameraControlObservation {
        desired_blocked: record.desired_blocked,
        state,
        present_total: inventory.len(),
        owned_blocked_present,
        pending_restore,
        absent_owned,
        unknown_devices,
        helper_active,
        detail,
    })
}
fn native_restore_status(id: &str) -> Result<NativeStatus> {
    let Some(mut native) = NativeDevice::open(id)? else {
        return Ok(NativeStatus::Absent);
    };
    let status = native.status()?;
    let disabled_config = native.disabled_configuration()?;
    if status == NativeStatus::Absent && !disabled_config {
        Ok(NativeStatus::Running)
    } else if status == NativeStatus::Running && disabled_config {
        Ok(NativeStatus::ConfiguredDisabledRunning)
    } else {
        Ok(status)
    }
}
fn inventory_with_owned(journal: &Journal) -> Result<Vec<CameraDevice>> {
    let mut inventory = devices()?;
    for entry in &journal.owned {
        let Some(mut native) = NativeDevice::open(&entry.id)? else {
            continue;
        };
        ensure!(
            class_text(native.info.class) == entry.class && entry.generation <= journal.generation,
            "owned camera native identity/class/generation changed during observation"
        );
        if native.present()? {
            let status = match native.status()? {
                NativeStatus::Running if native.disabled_configuration()? => "ConfiguredDisabled",
                NativeStatus::Running => "OK",
                NativeStatus::Disabled => "Disabled",
                _ => "Unknown",
            };
            if let Some(device) = inventory
                .iter_mut()
                .find(|device| device.instance_id.eq_ignore_ascii_case(&native.id))
            {
                if device.status != status {
                    device.status.clear();
                    device.status.push_str(status);
                }
            } else {
                inventory.push(CameraDevice {
                    instance_id: native.id.clone(),
                    status: status.to_owned(),
                });
            }
        }
    }
    Ok(inventory)
}
fn observe_journal(
    journal: &Journal,
    blocked: bool,
    generation: u64,
    active: bool,
    detail: Option<&str>,
) -> Result<CameraControlObservation> {
    let inventory = inventory_with_owned(journal)?;
    let intent = CameraBlockRecord {
        desired_blocked: blocked,
        intent_generation: generation,
        ..Default::default()
    };
    observe_entries(
        &intent,
        active,
        detail,
        &inventory,
        journal
            .owned
            .iter()
            .filter(|entry| !entry.prepared)
            .map(|entry| entry.id.as_str()),
        native_restore_status,
    )
}
fn read_journal() -> Result<Option<Journal>> {
    let identity = crate::windows_control::current_identity()?;
    let path = match protected_path(&identity, false) {
        Ok(path) => path,
        Err(error)
            if error.chain().any(|cause| {
                cause
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|io| io.kind() == std::io::ErrorKind::NotFound)
            }) =>
        {
            return Ok(None);
        }
        Err(error) => return Err(error),
    };
    match fs::metadata(&path) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).context("unknown protected camera journal metadata"),
    }
    Ok(Some(Journal::load(&path, &identity.user)?))
}
/// No inventory, helper launch, journal write or SDK action is needed for update safety.
/// The caller holds the camera request lock and native resource reservation.
pub(crate) fn ensure_update_inactive() -> Result<()> {
    let user = load_record()?.unwrap_or_default();
    ensure!(
        !user.desired_blocked,
        "camera protection is requested; update refused without selecting Allow"
    );
    // Unsigned restoration inventory is informational, not protected ownership.
    // It survives replacement and remains eligible for explicit legacy restoration.
    if let Some(journal) = read_journal()? {
        ensure!(
            !journal.requested
                && journal.owned.is_empty()
                && journal
                    .migration_receipts
                    .iter()
                    .all(|receipt| receipt.fulfilled),
            "protected camera ownership or restoration is pending; update refused without selecting Allow"
        );
    }
    Ok(())
}

fn combined_record(user: CameraBlockRecord, journal: Option<&Journal>) -> CameraBlockRecord {
    let Some(journal) = journal else {
        return user;
    };
    let (requested, generation) = if journal.generation > user.intent_generation {
        (journal.requested, journal.generation)
    } else {
        (user.desired_blocked, user.intent_generation)
    };
    let mut record = journal_record(journal, requested, generation);
    for id in user.blocked.into_iter().chain(user.restore_on_arrival) {
        let fulfilled = journal
            .migration_receipts
            .iter()
            .any(|receipt| receipt.fulfilled && receipt.id.eq_ignore_ascii_case(&id));
        if !fulfilled && !record.owns(&id) {
            record.restore_on_arrival.push(id);
        }
    }
    record
}
pub(super) fn observation() -> Result<CameraControlObservation> {
    if let Ok(response) = request(&Request::Status) {
        if response.observation.unknown_devices > 0
            && response.completed_generation >= response.generation
        {
            if let Some(error) = response.error {
                bail!("{error}");
            }
            bail!(
                "native camera inventory has {} unknown device status(es); no absence or fulfilled protection is inferred",
                response.observation.unknown_devices
            );
        }
        return Ok(response.observation);
    }
    let user = load_record()?.unwrap_or_default();
    let journal = read_journal()?;
    let legacy = user
        .blocked
        .iter()
        .chain(&user.restore_on_arrival)
        .any(|id| {
            !journal.as_ref().is_some_and(|journal| {
                journal
                    .owned
                    .iter()
                    .any(|entry| entry.id.eq_ignore_ascii_case(id))
                    || journal
                        .migration_receipts
                        .iter()
                        .any(|receipt| receipt.fulfilled && receipt.id.eq_ignore_ascii_case(id))
            })
        });
    let record = combined_record(user, journal.as_ref());
    let detail = legacy.then_some("Legacy restoration is not fulfilled: original ownership cannot be authenticated from unsigned history. Absent entries are informational, not a blocked-camera count; use explicit user-authorized legacy restoration for that camera.");
    let inventory = match journal.as_ref() {
        Some(journal) => inventory_with_owned(journal)?,
        None => devices()?,
    };
    let mut observed =
        observe_inventory(&record, false, detail, &inventory, native_restore_status)?;
    ensure!(
        observed.unknown_devices == 0,
        "native camera inventory has {} unknown device status(es); no unplugged-device conclusion is permitted",
        observed.unknown_devices
    );
    // Historical writable IDs remain informational; only the protected native
    // journal can authenticate an actual owned blocked-device count.
    observed.owned_blocked_present = owned_blocked_count(journal.as_ref(), &inventory);
    Ok(observed)
}
pub(super) fn arrived_targets() -> Result<Vec<String>> {
    let user = load_record()?.unwrap_or_default();
    if user.desired_blocked {
        return Ok(Vec::new());
    }
    let Some(journal) = read_journal()? else {
        return Ok(Vec::new());
    };
    let present = inventory_with_owned(&journal)?;
    Ok(present
        .into_iter()
        .filter(|device| {
            matches!(device.status.as_str(), "Disabled" | "ConfiguredDisabled")
                && journal.owned.iter().any(|entry| {
                    !entry.prepared && entry.id.eq_ignore_ascii_case(&device.instance_id)
                })
        })
        .map(|device| device.instance_id)
        .collect())
}
pub(super) fn restore_arrived() -> Result<usize> {
    let transaction = crate::windows_control::acquire_request_lock("camera")?;
    let user = load_record()?.unwrap_or_default();
    let Some(journal) = read_journal()? else {
        return Ok(0);
    };
    // Automatic work may complete ONLY this already-approved durable Allow,
    // never replace a pending or newly selected explicit global intent.
    if user.desired_blocked || journal.requested || user.intent_generation > journal.generation {
        return Ok(0);
    }
    let generation = journal.generation;
    let targets = arrived_targets()?;
    if targets.is_empty() {
        drop(transaction);
        reconcile_running()?;
        return Ok(0);
    }
    let mut existing = request(&Request::Status).ok();
    if let Some(status) = existing.as_ref() {
        if status.generation != generation || status.observation.desired_blocked {
            return Ok(0);
        }
        if status.observation.helper_active && !status.retiring {
            return Ok(0);
        }
        if status.retiring {
            request(&Request::FinishAllow { generation })?;
            wait_for_shutdown()?;
            existing = None;
        }
    }
    let _process = if existing.is_none() {
        Some(launch(None)?)
    } else {
        None
    };
    let status = match existing {
        Some(status) => status,
        None => wait_for_service()?,
    };
    ensure!(
        status.generation == generation && !status.observation.desired_blocked,
        "automatic arrival restoration no longer matches the approved Allow"
    );
    request(&Request::ReconcileAllow { generation })?;
    drop(transaction);
    finish_intent(false, generation)?;
    reconcile_running()?;
    Ok(targets.len())
}
fn registered_restore_needed(journal: &Journal) -> Result<bool> {
    for entry in journal.owned.iter().filter(|entry| !entry.prepared) {
        let Some(mut native) = NativeDevice::open(&entry.id)? else {
            continue;
        };
        ensure!(
            entry.class == class_text(native.info.class),
            "owed native camera class changed"
        );
        if restoration_needed(Some(native.status()?), native.disabled_configuration()?)? {
            return Ok(true);
        }
    }
    Ok(false)
}
pub(super) fn reconcile_running() -> Result<()> {
    let _transaction = crate::windows_control::acquire_request_lock("camera")?;
    let Some(mut record) = load_record()? else {
        return Ok(());
    };
    if record.desired_blocked {
        return Ok(());
    }
    let mut remaining = Vec::new();
    for id in &record.restore_on_arrival {
        let running = match NativeDevice::open(id)? {
            Some(mut native) => {
                restore_fulfilled(native.status()?, native.disabled_configuration()?)
            }
            None => false,
        };
        if !running {
            remaining.push(id.clone());
        }
    }
    if remaining != record.restore_on_arrival {
        record.restore_on_arrival = remaining;
        save_record(&record)?;
    }
    Ok(())
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyTarget {
    instance_id: String,
    class: String,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Bootstrap {
    identity: crate::windows_control::ControlIdentity,
    invoker_pid: u32,
    invoker_created: u64,
    #[serde(default)]
    legacy_restore: Option<LegacyTarget>,
}
fn validate_legacy_target(target: &LegacyTarget) -> Result<Option<NativeDevice>> {
    ensure!(
        matches!(target.class.as_str(), CAMERA_CLASS | IMAGE_CLASS),
        "legacy restoration requires a native camera class"
    );
    let Some(native) = NativeDevice::open(&target.instance_id)? else {
        return Ok(None);
    };
    ensure!(
        class_text(native.info.class) == target.class,
        "legacy camera native identity/class changed"
    );
    if native.info.class == IMAGE_GUID {
        let modern = video_interface(&VIDEO_CAMERA_INTERFACE, &native.id, 1)?;
        let legacy = video_interface(&VIDEO_INTERFACE, &native.id, 1)?
            && video_interface(&CAPTURE_INTERFACE, &native.id, 1)?;
        ensure!(
            modern || legacy,
            "Windows cannot positively prove this offline/disabled Image-class identity is a video camera; no scanner or unrelated Image device will be enabled"
        );
    }
    Ok(Some(native))
}
fn quote_argument(text: &str) -> String {
    let mut quoted = String::with_capacity(text.len() + 2);
    quoted.push('"');
    let mut slashes = 0;
    for ch in text.chars() {
        if ch == '\\' {
            slashes += 1;
            continue;
        }
        if ch == '"' {
            quoted.extend(std::iter::repeat_n('\\', slashes * 2 + 1));
            quoted.push('"');
        } else {
            quoted.extend(std::iter::repeat_n('\\', slashes));
            quoted.push(ch);
        }
        slashes = 0;
    }
    quoted.extend(std::iter::repeat_n('\\', slashes * 2));
    quoted.push('"');
    quoted
}
fn launch(legacy_restore: Option<LegacyTarget>) -> Result<Handle> {
    crate::windows_control::prepare_invoker_query()?;
    let bootstrap = Bootstrap {
        identity: crate::windows_control::current_identity()?,
        invoker_pid: std::process::id(),
        invoker_created: crate::windows_control::current_process_created()?,
        legacy_restore,
    };
    let encoded = serde_json::to_string(&bootstrap)?;
    ensure!(
        encoded.len() <= 4096,
        "camera helper bootstrap exceeds 4 KiB"
    );
    let exe = std::env::current_exe()?;
    let exe = if exe
        .file_stem()
        .and_then(|stem| stem.to_str())
        .is_some_and(|stem| stem.eq_ignore_ascii_case("mcw-tray"))
    {
        exe.with_file_name("mcw.exe")
    } else {
        exe
    };
    ensure!(
        exe.is_file(),
        "paired camera guard executable is unavailable"
    );
    let file: Vec<u16> = exe.as_os_str().encode_wide().chain(Some(0)).collect();
    let verb = wide("runas");
    let arguments = wide(&format!(
        "__camera-guard --bootstrap {}",
        quote_argument(&encoded)
    ));
    let mut info = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC | SEE_MASK_NO_CONSOLE,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(arguments.as_ptr()),
        nShow: 0,
        ..Default::default()
    };
    unsafe { ShellExecuteExW(&mut info) }
        .context("camera administrator approval declined or helper launch failed")?;
    Handle::new(info.hProcess.0)
}
fn wait_for_service() -> Result<Response> {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match request(&Request::Status) {
            Ok(status) => return Ok(status),
            Err(error) if Instant::now() >= deadline => {
                return Err(error).context("approved camera helper did not become available");
            }
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    }
}
fn finish_intent(blocked: bool, generation: u64) -> Result<CameraControlObservation> {
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        let response = request(&Request::Status)?;
        ensure!(
            response.generation == generation,
            "camera intent was superseded by a newer explicit request"
        );
        if response.completed_generation >= generation {
            if !blocked {
                request(&Request::FinishAllow { generation })?;
            }
            if let Some(error) = response.error {
                bail!("{error}");
            }
            if !blocked || response.observation.state == CameraPrivacyState::Blocked {
                return Ok(response.observation);
            }
        }
        ensure!(
            Instant::now() < deadline,
            "camera intent is accepted, but Windows native action/readback has not completed; no success or OS cancellation is claimed"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}
fn wait_for_shutdown() -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    while request(&Request::Status).is_ok() {
        ensure!(
            Instant::now() < deadline,
            "temporary camera helper has not stopped after completed Allow"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    Ok(())
}
fn control_requested(selection: Option<bool>) -> Result<()> {
    let transaction = crate::windows_control::acquire_request_lock("camera")?;
    let mut record = load_record()?.unwrap_or_default();
    let journal = read_journal()?;
    let mut existing = request(&Request::Status).ok();
    if existing.as_ref().is_some_and(|status| status.retiring) {
        let retiring = existing
            .take()
            .context("retiring camera helper disappeared")?;
        request(&Request::FinishAllow {
            generation: retiring.generation,
        })?;
        wait_for_shutdown()?;
    }
    let blocked = selection.unwrap_or_else(|| {
        !existing.as_ref().map_or(record.desired_blocked, |status| {
            status.observation.desired_blocked
        })
    });
    let generation = record
        .intent_generation
        .max(journal.as_ref().map_or(0, |journal| journal.generation))
        .max(existing.as_ref().map_or(0, |status| status.generation))
        .checked_add(1)
        .context("camera intent generation exhausted")?;
    record.desired_blocked = blocked;
    if !blocked {
        record.restore_on_arrival.append(&mut record.blocked);
        dedupe_ascii_case(&mut record.restore_on_arrival);
    }
    // Keep generation-zero unsigned history explicitly unsigned on read-only
    // Allow; no automatic migration or privilege follows writable IDs.
    let legacy_only =
        record.intent_generation == 0 && journal.is_none() && existing.is_none() && !blocked;
    if !legacy_only {
        record.intent_generation = generation;
    }
    save_record(&record)?;
    if !blocked && existing.is_none() {
        reconcile_running()?;
        let needs_restore = match journal.as_ref() {
            Some(journal) => registered_restore_needed(journal)?,
            None => false,
        };
        if !needs_restore {
            return Ok(());
        }
    }
    let _process = if existing.is_none() {
        Some(launch(None)?)
    } else {
        None
    };
    let status = if let Some(status) = existing {
        status
    } else {
        wait_for_service()?
    };
    let generation = generation.max(
        status
            .generation
            .checked_add(1)
            .context("privileged camera intent generation exhausted")?,
    );
    record.intent_generation = generation;
    save_record(&record)?;
    let accepted = request(&Request::Intent {
        blocked,
        generation,
    })?;
    ensure!(
        accepted.generation == generation,
        "camera helper did not accept the requested generation"
    );
    // Explicit Allow from another client must be accepted while this client's
    // native action is still in flight; never hold the intent lock while waiting.
    drop(transaction);
    finish_intent(blocked, generation)?;
    reconcile_running()?;
    Ok(())
}
pub(super) fn set_requested(blocked: bool) -> Result<()> {
    control_requested(Some(blocked))
}
pub(super) fn toggle_requested() -> Result<()> {
    control_requested(None)
}
pub(super) fn restore_legacy(instance_id: &str) -> Result<CameraControlObservation> {
    // Global Allow is authoritative before one NEW UAC approval for this exact
    // target; a writable historical list never supplies blanket admin authority.
    set_requested(false)?;
    let Some(mut native) = NativeDevice::open(instance_id)? else {
        let mut observed = observation()?;
        observed.detail = Some("Explicit legacy restoration is not fulfilled: Windows has no registered native camera identity. Global Allow is selected; history remains pending without a UAC prompt or hardware action.".to_owned());
        return Ok(observed);
    };
    let target = LegacyTarget {
        instance_id: native.id.clone(),
        class: class_text(native.info.class).to_owned(),
    };
    validate_legacy_target(&target)?
        .context("legacy camera disappeared before administrator approval")?;
    ensure!(
        native.status()? != NativeStatus::Unknown,
        "legacy camera native status is unknown; no blind administrator enable will be attempted"
    );
    if restore_fulfilled(native.status()?, native.disabled_configuration()?) {
        reconcile_running()?;
        return observation();
    }
    // Never broaden an already-approved Block helper with a device-addressed
    // command. The target is fixed ONLY in a fresh runas-approved bootstrap.
    let deadline = Instant::now() + Duration::from_secs(5);
    while request(&Request::Status).is_ok() {
        ensure!(
            Instant::now() < deadline,
            "previous temporary camera helper has not stopped after Allow"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    let transaction = crate::windows_control::acquire_request_lock("camera")?;
    ensure!(
        !load_record()?.unwrap_or_default().desired_blocked,
        "legacy restoration was superseded by a new Block request"
    );
    let _process = launch(Some(target))?;
    let status = wait_for_service()?;
    let mut record = load_record()?.unwrap_or_default();
    let generation = record
        .intent_generation
        .max(status.generation)
        .checked_add(1)
        .context("legacy restore generation exhausted")?;
    record.desired_blocked = false;
    record.intent_generation = generation;
    save_record(&record)?;
    let accepted = request(&Request::Intent {
        blocked: false,
        generation,
    })?;
    ensure!(
        accepted.generation == generation,
        "legacy helper did not accept restoration"
    );
    drop(transaction);
    finish_intent(false, generation)?;
    reconcile_running()?;
    observation()
}
pub(super) fn run(encoded: &str) -> Result<()> {
    ensure!(encoded.len() <= 4096, "camera bootstrap exceeds 4 KiB");
    let bootstrap: Bootstrap =
        serde_json::from_str(encoded).context("invalid strict camera helper bootstrap")?;
    token_identity(true)?;
    crate::windows_control::validate_invoker(
        &bootstrap.identity,
        bootstrap.invoker_pid,
        bootstrap.invoker_created,
    )?;
    // The helper is IPC-only and must outlive the caller's terminal without
    // allocating a hidden console host or sharing its console-close events.
    unsafe { FreeConsole() }.context("detach camera helper from caller console")?;
    crate::windows_control::grant_invoker_query(
        &bootstrap.identity,
        bootstrap.invoker_pid,
        bootstrap.invoker_created,
    )?;
    let _resource = crate::windows_control::acquire_resource_for("camera", &bootstrap.identity)?;
    let path = protected_path(&bootstrap.identity, true)?;
    let journal = Journal::load(&path, &bootstrap.identity.user)?;
    let startup = Instant::now();
    let state = Arc::new(Mutex::new(ControlState {
        generation: journal.generation,
        blocked: journal.requested,
        armed: false,
        completed_generation: 0,
        observation: unknown_observation(
            journal.requested,
            false,
            "Awaiting an authenticated explicit camera intent.".to_owned(),
        ),
        error: None,
    }));
    let stop = Arc::new(AtomicBool::new(false));
    let wake = Arc::new(Wake {
        dirty: AtomicBool::new(true),
        thread: OnceLock::new(),
    });
    let mut notification = Notification::register(&wake);
    let notification_detail = notification.as_ref().err().map(bounded_detail);
    let legacy_mode = bootstrap.legacy_restore.is_some();
    let worker_state = Arc::clone(&state);
    let worker_stop = Arc::clone(&stop);
    let worker_wake = Arc::clone(&wake);
    let worker = std::thread::spawn(move || {
        let _ = worker_wake.thread.set(std::thread::current());
        let mut owner = Coordinator {
            journal,
            path,
            control: Arc::clone(&worker_state),
            legacy: bootstrap.legacy_restore,
            dirty: false,
        };
        let mut allow_completed: Option<(u64, Instant)> = None;
        while !worker_stop.load(Ordering::Acquire) {
            let (blocked, epoch, armed) = match worker_state.lock() {
                Ok(current) => (current.blocked, current.generation, current.armed),
                Err(_) => {
                    worker_stop.store(true, Ordering::Release);
                    break;
                }
            };
            if !armed {
                if startup.elapsed() >= Duration::from_secs(30) {
                    worker_stop.store(true, Ordering::Release);
                    break;
                }
                std::thread::park_timeout(Duration::from_millis(100));
                continue;
            }
            if let Some((completed, at)) = allow_completed {
                if completed == epoch {
                    if at.elapsed() >= Duration::from_secs(2)
                        && let Ok(current) = worker_state.lock()
                        && current.generation == completed
                        && !current.blocked
                    {
                        worker_stop.store(true, Ordering::Release);
                        break;
                    }
                    std::thread::park_timeout(Duration::from_millis(50));
                    continue;
                }
                allow_completed = None;
            }
            let started = Instant::now();
            let failure = owner
                .enforce(blocked, epoch)
                .err()
                .map(|error| bounded_detail(&error));
            // In-flight native action cannot be cancelled. The separate I/O
            // thread already accepted newer epochs; never run its old next job.
            if !owner.current(epoch, blocked).unwrap_or(false) {
                continue;
            }
            let detail = failure.as_deref().or(notification_detail.as_deref());
            let (observed, failure) =
                match observe_journal(&owner.journal, blocked, epoch, blocked, detail) {
                    Ok(observed) => (observed, failure),
                    Err(error) => {
                        let detail = bounded_detail(&error);
                        (
                            unknown_observation(blocked, blocked, detail.clone()),
                            Some(failure.unwrap_or(detail)),
                        )
                    }
                };
            if let Ok(mut current) = worker_state.lock()
                && current.generation == epoch
                && current.blocked == blocked
            {
                current.observation = observed;
                current.completed_generation = epoch;
                current.error = failure;
                if !blocked {
                    allow_completed = Some((epoch, Instant::now()));
                }
            }
            if let Some(delay) = Duration::from_millis(250).checked_sub(started.elapsed()) {
                std::thread::sleep(delay);
            }
            if blocked {
                worker_wake.dirty.swap(false, Ordering::AcqRel);
                std::thread::park_timeout(Duration::from_secs(2));
            }
        }
    });
    let serve_state = Arc::clone(&state);
    let serve_stop = Arc::clone(&stop);
    let serve_wake = Arc::clone(&wake);
    let result = crate::windows_control::serve_bound::<Request, Response>(
        "camera",
        &stop,
        &bootstrap.identity,
        move |command| {
            // Only tiny intent/snapshot work under this mutex: NEVER native SDK.
            let mut current = serve_state
                .lock()
                .map_err(|_| anyhow::anyhow!("camera intent lock poisoned"))?;
            match command {
                Request::Status => {}
                Request::Intent {
                    blocked,
                    generation,
                } => {
                    ensure!(
                        !serve_stop.load(Ordering::Acquire),
                        "temporary camera helper is stopping after Allow"
                    );
                    ensure!(
                        !current.retiring(),
                        "approved camera helper has retired after Allow; a new Block needs fresh approval"
                    );
                    ensure!(
                        !legacy_mode || !blocked,
                        "legacy migration helper is Allow-only"
                    );
                    current.accept(blocked, generation)?;
                    serve_wake.dirty.store(true, Ordering::Release);
                    if let Some(thread) = serve_wake.thread.get() {
                        thread.unpark();
                    }
                }
                Request::ReconcileAllow { generation } => {
                    ensure!(
                        !serve_stop.load(Ordering::Acquire) && !current.retiring(),
                        "temporary camera helper has stopped after Allow"
                    );
                    current.reconcile_allow(generation)?;
                    serve_wake.dirty.store(true, Ordering::Release);
                    if let Some(thread) = serve_wake.thread.get() {
                        thread.unpark();
                    }
                }
                Request::FinishAllow { generation } => {
                    ensure!(
                        !current.blocked
                            && current.generation == generation
                            && current.completed_generation >= generation,
                        "camera native Allow has not completed or was superseded"
                    );
                    serve_stop.store(true, Ordering::Release);
                }
            }
            Ok(current.response())
        },
    );
    stop.store(true, Ordering::Release);
    if let Some(thread) = wake.thread.get() {
        thread.unpark();
    }
    worker
        .join()
        .map_err(|_| anyhow::anyhow!("camera native worker panicked"))?;
    if let Ok(notification) = &mut notification {
        notification.unregister()?;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_camera_block_is_a_real_global_intent() -> Result<()> {
        let record = CameraBlockRecord {
            desired_blocked: true,
            intent_generation: 7,
            ..Default::default()
        };
        let active = observe_inventory(&record, true, None, &[], |_| unreachable!())?;
        assert!(active.desired_blocked && active.helper_active);
        assert_eq!(active.state, CameraPrivacyState::Blocked);
        assert_eq!(active.present_total, 0);
        assert!(!record.is_empty());
        let missing = observe_inventory(&record, false, None, &[], |_| unreachable!())?;
        assert_eq!(missing.state, CameraPrivacyState::SystemManaged);
        assert!(missing.detail.as_deref().unwrap().contains("not running"));
        Ok(())
    }

    #[test]
    fn legacy_owed_entry_is_not_an_active_block() -> Result<()> {
        let record = CameraBlockRecord::from_json(br#"{"blocked":[],"restore_on_arrival":["USB\\VID_1234&PID_5678&MI_00\\synthetic-camera-instance"]}"#)?;
        assert!(!record.desired_blocked);
        assert_eq!(record.intent_generation, 0);
        let current = [CameraDevice {
            instance_id: "USB\\BUILTIN".to_owned(),
            status: "OK".to_owned(),
        }];
        let absent =
            observe_inventory(&record, false, None, &current, |_| Ok(NativeStatus::Absent))?;
        assert_eq!(absent.state, CameraPrivacyState::Allowed);
        assert_eq!(absent.absent_owned, 1);
        assert_eq!(absent.pending_restore, 0);
        assert_eq!(absent.owned_blocked_present, 0);
        assert!(absent.detail.as_deref().unwrap().contains("informational"));
        assert!(
            !record.is_empty(),
            "retain historical authority information until positive fulfillment"
        );
        let running = observe_inventory(&record, false, None, &current, |_| {
            Ok(NativeStatus::Running)
        })?;
        assert_eq!(running.pending_restore, 0);
        assert_eq!(running.unknown_devices, 0);
        Ok(())
    }

    #[test]
    fn unknown_native_probe_is_not_absence() -> Result<()> {
        let record = CameraBlockRecord {
            restore_on_arrival: vec!["USB\\CAM".to_owned()],
            ..Default::default()
        };
        assert!(
            observe_inventory(&record, false, None, &[], |_| bail!("CM status failed")).is_err()
        );
        let unknown = observe_inventory(&record, false, None, &[], |_| Ok(NativeStatus::Unknown))?;
        assert_eq!(unknown.unknown_devices, 1);
        assert_eq!(unknown.state, CameraPrivacyState::SystemManaged);
        Ok(())
    }

    #[test]
    fn mixed_and_external_disable_are_not_fulfilled_allow() -> Result<()> {
        let current = [
            CameraDevice {
                instance_id: "USB\\OWNED".to_owned(),
                status: "Disabled".to_owned(),
            },
            CameraDevice {
                instance_id: "USB\\FOREIGN".to_owned(),
                status: "OK".to_owned(),
            },
        ];
        let block = CameraBlockRecord {
            desired_blocked: true,
            intent_generation: 1,
            blocked: vec!["USB\\OWNED".to_owned()],
            ..Default::default()
        };
        let partial = observe_inventory(&block, true, None, &current, |_| unreachable!())?;
        assert_eq!(partial.state, CameraPrivacyState::SystemManaged);
        assert_eq!(partial.owned_blocked_present, 1);
        let allow = CameraBlockRecord::default();
        assert_eq!(
            observe_inventory(&allow, false, None, &current, |_| unreachable!())?.state,
            CameraPrivacyState::SystemManaged
        );
        assert!(!disable_authorized(
            true,
            true,
            1,
            1,
            NativeStatus::Disabled
        )?);
        assert!(
            !allow.owns("USB\\OWNED"),
            "foreign disabled device is never acquired"
        );
        Ok(())
    }

    #[test]
    fn allow_epoch_invalidates_every_old_block_job() -> Result<()> {
        let mut journal = Journal::default();
        journal.advance(true, 1)?;
        assert!(disable_authorized(
            journal.requested,
            true,
            journal.generation,
            1,
            NativeStatus::Running
        )?);
        journal.advance(false, 2)?;
        assert!(
            disable_authorized(
                journal.requested,
                false,
                journal.generation,
                1,
                NativeStatus::Running
            )
            .is_err()
        );
        assert!(journal.advance(true, 1).is_err());
        assert!(!journal.requested);
        journal.advance(true, 3)?;
        assert!(disable_authorized(true, true, 3, 1, NativeStatus::Running).is_err());
        assert!(disable_authorized(true, true, 3, 3, NativeStatus::Running)?);
        assert!(disable_authorized(true, true, 3, 3, NativeStatus::Unknown).is_err());
        Ok(())
    }

    #[test]
    fn offline_restore_requires_positive_configuration_readback() {
        assert!(restore_fulfilled(NativeStatus::Running, false));
        assert!(
            !restore_fulfilled(NativeStatus::Running, true),
            "a live endpoint with an owned future-disabled configuration is not fulfilled Allow"
        );
        assert!(restore_fulfilled(NativeStatus::Absent, false));
        assert!(!restore_fulfilled(NativeStatus::Absent, true));
        assert!(!restore_fulfilled(NativeStatus::Disabled, false));
        assert!(!restore_fulfilled(NativeStatus::Unknown, false));
    }

    #[test]
    fn wire_protocol_cannot_address_a_device_or_native_action() {
        assert!(
            serde_json::from_str::<Request>(
                r#"{"Intent":{"blocked":true,"generation":1,"device_id":"ROOT\\ARBITRARY"}}"#
            )
            .is_err()
        );
        assert!(serde_json::from_str::<Request>(r#"{"Enable":{"device_id":"USB\\CAM"}}"#).is_err());
        assert!(
            serde_json::from_str::<Request>(r#"{"Intent":{"blocked":false,"generation":2}}"#)
                .is_ok()
        );
    }

    #[test]
    fn inventory_cap_fails_instead_of_claiming_all_blocked() {
        let inventory: Vec<_> = (0..MAX_CAMERAS + 1)
            .map(|index| CameraDevice {
                instance_id: index.to_string(),
                status: "Disabled".to_owned(),
            })
            .collect();
        let record = CameraBlockRecord {
            desired_blocked: true,
            intent_generation: 1,
            ..Default::default()
        };
        let error =
            observe_inventory(&record, true, None, &inventory, |_| unreachable!()).unwrap_err();
        assert!(error.to_string().contains("4096-device"));
    }

    #[test]
    fn failed_intent_persistence_never_reaches_native_actions() -> Result<()> {
        let path = std::env::temp_dir()
            .join(format!("mcw-camera-missing-parent-{}", std::process::id()))
            .join("uncreated")
            .join("journal.json");
        let control = Arc::new(Mutex::new(ControlState {
            generation: 1,
            blocked: true,
            armed: true,
            completed_generation: 0,
            observation: unknown_observation(true, true, "pending".to_owned()),
            error: None,
        }));
        let mut owner = Coordinator {
            journal: Journal::default(),
            path,
            control: Arc::clone(&control),
            legacy: None,
            dirty: false,
        };
        assert!(owner.enforce(true, 1).is_err());
        assert!(owner.dirty);
        assert!(
            owner.enforce(true, 1).is_err(),
            "a failed generation save cannot be bypassed by the next polling pass"
        );
        assert!(owner.dirty);
        let current = control
            .lock()
            .map_err(|_| anyhow::anyhow!("test control lock poisoned"))?;
        assert_eq!(current.completed_generation, 0);
        assert_ne!(current.observation.state, CameraPrivacyState::Blocked);
        Ok(())
    }

    #[test]
    fn sdk_native_structure_layouts_and_utf8_error_budget() {
        assert_eq!(size_of::<DeviceInfo>(), 32);
        assert_eq!(size_of::<PropertyChange>(), 20);
        assert_eq!(size_of::<NotifyFilter>(), 416);
        let detail = bounded_detail(&anyhow::anyhow!("界".repeat(4000)));
        assert!(detail.len() <= 8192);
        assert!(std::str::from_utf8(detail.as_bytes()).is_ok());
    }

    #[test]
    fn allow_control_is_accepted_while_native_worker_is_in_flight() -> Result<()> {
        let control = Arc::new(Mutex::new(ControlState {
            generation: 1,
            blocked: true,
            armed: true,
            completed_generation: 0,
            observation: unknown_observation(true, true, "native action in flight".to_owned()),
            error: None,
        }));
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let worker_control = Arc::clone(&control);
        let worker = std::thread::spawn(move || -> Result<bool> {
            let owner = Coordinator {
                journal: Journal::default(),
                path: PathBuf::new(),
                control: worker_control,
                legacy: None,
                dirty: false,
            };
            entered_tx.send(())?;
            release_rx.recv()?;
            owner.current(1, true)
        });
        entered_rx.recv()?;
        {
            let mut current = control
                .lock()
                .map_err(|_| anyhow::anyhow!("test control lock poisoned"))?;
            current.accept(false, 2)?;
            assert_eq!(current.generation, 2);
            assert!(!current.observation.desired_blocked);
        }
        release_tx.send(())?;
        assert!(
            !worker.join().unwrap()?,
            "old job must not execute its next disable after Allow"
        );
        Ok(())
    }

    #[test]
    fn one_thousand_cameras_fit_the_global_model_and_count_only_response() -> Result<()> {
        let inventory: Vec<_> = (0..1000)
            .map(|index| CameraDevice {
                instance_id: format!("USB\\CAM{index}"),
                status: "Disabled".to_owned(),
            })
            .collect();
        let record = CameraBlockRecord {
            desired_blocked: true,
            intent_generation: 1,
            ..Default::default()
        };
        let observed = observe_inventory(&record, true, None, &inventory, |_| unreachable!())?;
        assert_eq!(observed.present_total, 1000);
        assert_eq!(observed.state, CameraPrivacyState::Blocked);
        let response = Response {
            observation: observed,
            generation: 1,
            completed_generation: 1,
            retiring: false,
            error: None,
        };
        let json = serde_json::to_vec(&response)?;
        assert!(json.len() < 4096);
        assert!(!String::from_utf8(json)?.contains("instance_id"));
        Ok(())
    }

    #[test]
    fn bootstrap_legacy_permission_is_explicit_and_not_a_wire_device_action() {
        let value = serde_json::json!({"identity":{"user":"S-1-5-21-1","session":1,"scope":"0".repeat(64)},"invoker_pid":4,"invoker_created":9});
        let ordinary: Bootstrap = serde_json::from_value(value.clone()).unwrap();
        assert!(ordinary.legacy_restore.is_none());
        let mut consent = value;
        consent["legacy_restore"] =
            serde_json::json!({"instance_id":"USB\\CAMERA","class":CAMERA_CLASS});
        let approved: Bootstrap = serde_json::from_value(consent).unwrap();
        assert_eq!(approved.legacy_restore.unwrap().instance_id, "USB\\CAMERA");
        assert!(
            serde_json::from_str::<Request>(r#"{"RestoreLegacy":{"instance_id":"USB\\CAMERA"}}"#)
                .is_err()
        );
    }

    #[test]
    fn offline_global_enable_preserves_every_other_configuration_bit() {
        for flags in [0, 1, 3, 0x1000_0001, u32::MAX] {
            assert_eq!(allow_configuration_flags(flags) & 1, 0);
            assert_eq!(allow_configuration_flags(flags) & !1, flags & !1);
        }
    }

    #[test]
    fn ordinary_journal_acl_never_permits_write_delete_or_replacement() {
        let protected = ordinary_dangerous_access(false);
        for right in [
            2u32, 4, 0x10, 0x40, 0x100, 0x10000, 0x40000, 0x80000, 0x10000000, 0x40000000,
        ] {
            assert_ne!(protected & right, 0);
        }
        assert_eq!(
            protected & 0x1200a9,
            0,
            "original SID read/traverse is not a write privilege"
        );
        assert_eq!(
            protected & 0x200a0,
            0,
            "shared empty ancestor metadata/traverse remains read-only"
        );
    }

    #[test]
    fn enabled_runtime_with_disabled_owned_future_config_is_still_pending() -> Result<()> {
        let current = [CameraDevice {
            instance_id: "USB\\CAM".to_owned(),
            status: "ConfiguredDisabled".to_owned(),
        }];
        let record = CameraBlockRecord {
            intent_generation: 3,
            restore_on_arrival: vec!["USB\\CAM".to_owned()],
            ..Default::default()
        };
        let observed = observe_inventory(&record, false, None, &current, |_| unreachable!())?;
        assert_eq!(observed.pending_restore, 1);
        assert_eq!(observed.owned_blocked_present, 0);
        assert_eq!(observed.state, CameraPrivacyState::SystemManaged);
        Ok(())
    }

    #[test]
    fn signed_ownership_count_excludes_unsigned_history_and_external_disable() {
        let inventory = [
            CameraDevice {
                instance_id: "USB\\OWNED".to_owned(),
                status: "Disabled".to_owned(),
            },
            CameraDevice {
                instance_id: "USB\\LEGACY".to_owned(),
                status: "Disabled".to_owned(),
            },
            CameraDevice {
                instance_id: "USB\\EXTERNAL".to_owned(),
                status: "Disabled".to_owned(),
            },
        ];
        assert_eq!(owned_blocked_count(None, &inventory), 0);
        let journal = Journal {
            generation: 3,
            owned: vec![OwnedDevice {
                id: "usb\\owned".to_owned(),
                class: CAMERA_CLASS.to_owned(),
                generation: 2,
                explicit_legacy: false,
                prepared: false,
            }],
            ..Default::default()
        };
        assert_eq!(owned_blocked_count(Some(&journal), &inventory), 1);
    }

    #[test]
    fn protected_journal_rejects_unproved_classes_epochs_and_legacy_receipts() -> Result<()> {
        let mut journal = Journal {
            generation: 4,
            owned: vec![OwnedDevice {
                id: "USB\\CAM".to_owned(),
                class: CAMERA_CLASS.to_owned(),
                generation: 3,
                explicit_legacy: false,
                prepared: false,
            }],
            ..Default::default()
        };
        journal.validate()?;
        journal.owned[0].generation = 0;
        assert!(
            journal.validate().is_err(),
            "unsigned epoch zero cannot become native authority"
        );
        journal.owned[0].generation = 5;
        assert!(journal.validate().is_err());
        journal.owned[0].generation = 3;
        journal.owned[0].class = "{arbitrary-non-camera-class}".to_owned();
        assert!(journal.validate().is_err());
        journal.owned[0].class = CAMERA_CLASS.to_owned();
        journal.owned[0].explicit_legacy = true;
        assert!(
            journal.validate().is_err(),
            "writable history is not a migration receipt"
        );
        journal.migration_receipts.push(MigrationReceipt {
            id: "usb\\cam".to_owned(),
            class: CAMERA_CLASS.to_owned(),
            generation: 3,
            fulfilled: false,
        });
        journal.validate()?;
        journal.migration_receipts[0].fulfilled = true;
        assert!(
            journal.validate().is_err(),
            "completed receipt cannot authorize a new enable"
        );
        journal.owned.clear();
        journal.validate()?;
        Ok(())
    }

    #[test]
    fn failed_dirty_ownership_cannot_authorize_consecutive_native_attempts() -> Result<()> {
        let control = Arc::new(Mutex::new(ControlState {
            generation: 1,
            blocked: true,
            armed: true,
            completed_generation: 0,
            observation: unknown_observation(true, true, "pending".to_owned()),
            error: None,
        }));
        let path = std::env::temp_dir()
            .join(format!("mcw-camera-uncommitted-{}", std::process::id()))
            .join("uncreated")
            .join("journal.json");
        let journal = Journal {
            generation: 1,
            requested: true,
            owned: vec![OwnedDevice {
                id: "USB\\PREPARED".to_owned(),
                class: CAMERA_CLASS.to_owned(),
                generation: 1,
                explicit_legacy: false,
                prepared: true,
            }],
            ..Default::default()
        };
        let mut owner = Coordinator {
            journal,
            path,
            control,
            legacy: None,
            dirty: true,
        };
        for _ in 0..2 {
            assert!(
                owner
                    .authorize_native_attempt(1, "USB\\PREPARED", true)
                    .is_err()
            );
            assert!(owner.dirty);
            assert!(
                owner.journal.owned[0].prepared,
                "failed persistence never becomes attempted native authority"
            );
            assert!(
                owner.enforce(true, 1).is_err(),
                "a consecutive polling pass must still stop at the dirty barrier"
            );
        }
        assert!(owner.withdraw_unattempted("USB\\PREPARED").is_err());
        assert!(
            owner.journal.owned.is_empty(),
            "failed withdrawal remains dirty, not a retained foreign-enable capability"
        );
        assert!(owner.flush().is_err());
        assert!(owner.dirty);
        Ok(())
    }

    #[test]
    fn aborted_preparation_preserves_preexisting_ownership() -> Result<()> {
        let control = Arc::new(Mutex::new(ControlState {
            generation: 2,
            blocked: false,
            armed: true,
            completed_generation: 0,
            observation: unknown_observation(false, true, "Allow selected".to_owned()),
            error: None,
        }));
        let path = std::env::temp_dir()
            .join(format!("mcw-camera-aborted-{}", std::process::id()))
            .join("uncreated")
            .join("journal.json");
        let owned = vec![
            OwnedDevice {
                id: "USB\\NEW".to_owned(),
                class: CAMERA_CLASS.to_owned(),
                generation: 1,
                explicit_legacy: false,
                prepared: true,
            },
            OwnedDevice {
                id: "USB\\ORIGINAL".to_owned(),
                class: CAMERA_CLASS.to_owned(),
                generation: 1,
                explicit_legacy: false,
                prepared: false,
            },
        ];
        let mut owner = Coordinator {
            journal: Journal {
                generation: 1,
                requested: true,
                owned,
                ..Default::default()
            },
            path,
            control,
            legacy: None,
            dirty: false,
        };
        assert!(
            owner.authorize_native_attempt(1, "USB\\NEW", true).is_err(),
            "accepted Allow forbids the old Block dispatch"
        );
        assert!(owner.journal.owned[0].prepared);
        assert!(owner.withdraw_unattempted("USB\\NEW").is_err());
        assert_eq!(owner.journal.owned.len(), 1);
        assert_eq!(owner.journal.owned[0].id, "USB\\ORIGINAL");
        assert!(
            owner.dirty,
            "even failed withdrawal must commit before any next privileged action"
        );
        Ok(())
    }

    #[test]
    fn automatic_restore_cannot_select_a_new_allow_after_explicit_block() -> Result<()> {
        let mut control = ControlState {
            generation: 8,
            blocked: false,
            armed: false,
            completed_generation: 0,
            observation: unknown_observation(false, false, "approved Allow".to_owned()),
            error: None,
        };
        control.reconcile_allow(8)?;
        assert_eq!(
            control.generation, 8,
            "automatic work never mints a global intent"
        );
        assert!(
            control.reconcile_allow(8).is_err(),
            "in-flight Allow is not falsely reported as a new completed restoration"
        );
        control.accept(true, 9)?;
        assert!(control.reconcile_allow(8).is_err());
        assert!(control.blocked && control.generation == 9);
        Ok(())
    }

    #[test]
    fn explicit_allow_retry_reaches_registered_phantom_debt_only() -> Result<()> {
        assert!(restoration_needed(Some(NativeStatus::Absent), true)?);
        assert!(!restoration_needed(Some(NativeStatus::Absent), false)?);
        assert!(
            !restoration_needed(None, true)?,
            "unregistered history cannot cause repeated administrator prompts"
        );
        assert!(restoration_needed(Some(NativeStatus::Disabled), true)?);
        assert!(!restoration_needed(Some(NativeStatus::Running), false)?);
        assert!(restoration_needed(Some(NativeStatus::Unknown), true).is_err());
        Ok(())
    }

    #[test]
    fn filtered_nonmembership_is_not_native_absence() -> Result<()> {
        let record = CameraBlockRecord {
            intent_generation: 2,
            restore_on_arrival: vec!["USB\\OWNED".to_owned()],
            ..Default::default()
        };
        for status in [
            NativeStatus::Running,
            NativeStatus::Disabled,
            NativeStatus::ConfiguredDisabledRunning,
            NativeStatus::Unknown,
        ] {
            let observed = observe_inventory(&record, false, None, &[], |_| Ok(status))?;
            assert_eq!(
                observed.absent_owned, 0,
                "{status:?} is not proof of absence"
            );
            assert_eq!(
                observed.unknown_devices,
                usize::from(status == NativeStatus::Unknown)
            );
        }
        let observed = observe_inventory(&record, false, None, &[], |_| Ok(NativeStatus::Absent))?;
        assert_eq!(observed.absent_owned, 1);
        Ok(())
    }
}
