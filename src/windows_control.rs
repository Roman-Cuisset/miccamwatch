//! Fixed, same-user/session local control endpoints. No arbitrary pipe paths or commands.
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use std::{
    ffi::c_void,
    mem::size_of,
    ptr,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

type RawHandle = *mut c_void;
#[repr(C)]
struct SecurityAttributes {
    length: u32,
    descriptor: *mut c_void,
    inherit: i32,
}
#[repr(C)]
#[derive(Default)]
struct Overlapped {
    internal: usize,
    internal_high: usize,
    offset: u32,
    offset_high: u32,
    event: RawHandle,
}
#[repr(C)]
#[derive(Default)]
struct FileTime {
    low: u32,
    high: u32,
}
#[repr(C)]
struct Trustee {
    multiple: *mut c_void,
    operation: u32,
    form: u32,
    kind: u32,
    name: *mut u16,
}
#[repr(C)]
struct ExplicitAccess {
    permissions: u32,
    mode: u32,
    inheritance: u32,
    trustee: Trustee,
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn CloseHandle(handle: RawHandle) -> i32;
    fn GetLastError() -> u32;
    fn LocalFree(memory: *mut c_void) -> *mut c_void;
    fn GetCurrentProcess() -> RawHandle;
    fn GetCurrentThread() -> RawHandle;
    fn GetCurrentProcessId() -> u32;
    fn ProcessIdToSessionId(pid: u32, session: *mut u32) -> i32;
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> RawHandle;
    fn GetProcessTimes(
        process: RawHandle,
        created: *mut FileTime,
        exited: *mut FileTime,
        kernel: *mut FileTime,
        user: *mut FileTime,
    ) -> i32;
    fn CreateEventW(
        attributes: *const SecurityAttributes,
        manual: i32,
        initial: i32,
        name: *const u16,
    ) -> RawHandle;
    fn WaitForSingleObject(handle: RawHandle, timeout: u32) -> u32;
    fn CreateMutexW(
        attributes: *const SecurityAttributes,
        initial_owner: i32,
        name: *const u16,
    ) -> RawHandle;
    fn OpenMutexW(access: u32, inherit: i32, name: *const u16) -> RawHandle;
    fn ReleaseMutex(handle: RawHandle) -> i32;
    fn CreateNamedPipeW(
        name: *const u16,
        mode: u32,
        pipe_mode: u32,
        instances: u32,
        out_size: u32,
        in_size: u32,
        timeout: u32,
        attributes: *const SecurityAttributes,
    ) -> RawHandle;
    fn CreateFileW(
        name: *const u16,
        access: u32,
        share: u32,
        attributes: *const SecurityAttributes,
        creation: u32,
        flags: u32,
        template: RawHandle,
    ) -> RawHandle;
    fn ConnectNamedPipe(handle: RawHandle, overlapped: *mut Overlapped) -> i32;
    fn DisconnectNamedPipe(handle: RawHandle) -> i32;
    fn GetNamedPipeClientProcessId(handle: RawHandle, pid: *mut u32) -> i32;
    fn GetNamedPipeServerProcessId(handle: RawHandle, pid: *mut u32) -> i32;
    fn ReadFile(
        handle: RawHandle,
        data: *mut c_void,
        count: u32,
        transferred: *mut u32,
        overlapped: *mut Overlapped,
    ) -> i32;
    fn WriteFile(
        handle: RawHandle,
        data: *const c_void,
        count: u32,
        transferred: *mut u32,
        overlapped: *mut Overlapped,
    ) -> i32;
    fn GetOverlappedResult(
        handle: RawHandle,
        overlapped: *mut Overlapped,
        transferred: *mut u32,
        wait: i32,
    ) -> i32;
    fn CancelIoEx(handle: RawHandle, overlapped: *const Overlapped) -> i32;
}
#[link(name = "advapi32")]
unsafe extern "system" {
    fn OpenProcessToken(process: RawHandle, access: u32, token: *mut RawHandle) -> i32;
    fn OpenThreadToken(
        thread: RawHandle,
        access: u32,
        open_as_self: i32,
        token: *mut RawHandle,
    ) -> i32;
    fn GetTokenInformation(
        token: RawHandle,
        class: u32,
        info: *mut c_void,
        length: u32,
        needed: *mut u32,
    ) -> i32;
    fn ConvertSidToStringSidW(sid: *mut c_void, text: *mut *mut u16) -> i32;
    fn ConvertStringSecurityDescriptorToSecurityDescriptorW(
        text: *const u16,
        revision: u32,
        descriptor: *mut *mut c_void,
        length: *mut u32,
    ) -> i32;
    fn ConvertSecurityDescriptorToStringSecurityDescriptorW(
        descriptor: *const c_void,
        revision: u32,
        information: u32,
        text: *mut *mut u16,
        length: *mut u32,
    ) -> i32;
    fn GetSecurityInfo(
        handle: RawHandle,
        object_type: u32,
        information: u32,
        owner: *mut *mut c_void,
        group: *mut *mut c_void,
        dacl: *mut *mut c_void,
        sacl: *mut *mut c_void,
        descriptor: *mut *mut c_void,
    ) -> u32;
    fn ConvertStringSidToSidW(text: *const u16, sid: *mut *mut c_void) -> i32;
    fn SetEntriesInAclW(
        count: u32,
        entries: *const ExplicitAccess,
        old_acl: *const c_void,
        new_acl: *mut *mut c_void,
    ) -> u32;
    fn SetSecurityInfo(
        handle: RawHandle,
        object_type: u32,
        information: u32,
        owner: *const c_void,
        group: *const c_void,
        dacl: *const c_void,
        sacl: *const c_void,
    ) -> u32;
    fn ImpersonateNamedPipeClient(pipe: RawHandle) -> i32;
    fn RevertToSelf() -> i32;
}
struct Handle(RawHandle);
impl Handle {
    fn new(raw: RawHandle) -> Result<Self> {
        if raw.is_null() || raw as isize == -1 {
            return Err(std::io::Error::last_os_error()).context("Windows control handle");
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
struct Local(*mut c_void);
impl Drop for Local {
    fn drop(&mut self) {
        unsafe {
            LocalFree(self.0);
        }
    }
}
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}
fn check(ok: i32) -> Result<()> {
    ensure!(
        ok != 0,
        "Windows control: {}",
        std::io::Error::last_os_error()
    );
    Ok(())
}
fn sid(token: RawHandle) -> Result<String> {
    let mut needed = 0;
    unsafe {
        GetTokenInformation(token, 1, ptr::null_mut(), 0, &mut needed);
    }
    ensure!(needed > 0 && needed <= 65536, "invalid token SID size");
    // Native TOKEN_USER begins with SID_AND_ATTRIBUTES, requiring pointer alignment.
    let mut buffer = vec![0usize; (needed as usize).div_ceil(size_of::<usize>())];
    unsafe {
        check(GetTokenInformation(
            token,
            1,
            buffer.as_mut_ptr().cast(),
            needed,
            &mut needed,
        ))?;
    }
    let sid = unsafe { *(buffer.as_ptr().cast::<*const c_void>()) };
    let mut text = ptr::null_mut();
    unsafe {
        check(ConvertSidToStringSidW(sid.cast_mut(), &mut text))?;
    }
    let _allocation = Local(text.cast());
    let mut len = 0;
    while len < 256 && unsafe { *text.add(len) } != 0 {
        len += 1;
    }
    ensure!(len < 256, "oversized user SID");
    Ok(String::from_utf16(unsafe {
        std::slice::from_raw_parts(text, len)
    })?)
}
fn process_sid(process: RawHandle) -> Result<String> {
    let mut token = ptr::null_mut();
    unsafe {
        check(OpenProcessToken(process, 8, &mut token))?;
    }
    let token = Handle::new(token)?;
    sid(token.0)
}
fn identity() -> Result<(String, u32)> {
    let user = process_sid(unsafe { GetCurrentProcess() })?;
    let mut session = 0;
    unsafe {
        check(ProcessIdToSessionId(GetCurrentProcessId(), &mut session))?;
    }
    Ok((user, session))
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ControlIdentity {
    pub user: String,
    pub session: u32,
    pub scope: String,
}
fn validate_identity(identity: &ControlIdentity) -> Result<()> {
    ensure!(
        identity.user.starts_with("S-1-")
            && identity.user.len() <= 184
            && identity.user[2..]
                .bytes()
                .all(|byte| byte.is_ascii_digit() || byte == b'-'),
        "invalid invoker SID"
    );
    ensure!(
        identity.scope.len() == 64 && identity.scope.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "invalid operational scope hash"
    );
    Ok(())
}
fn scope_hash(directory: &std::path::Path) -> Result<String> {
    let mut ancestor = directory.to_owned();
    let mut missing = Vec::new();
    while !ancestor.exists() {
        missing.push(
            ancestor
                .file_name()
                .context("operational data path has no existing ancestor")?
                .to_owned(),
        );
        ensure!(ancestor.pop(), "operational data path cannot be resolved");
    }
    let mut canonical =
        std::fs::canonicalize(ancestor).context("canonical operational data path unavailable")?;
    for component in missing.into_iter().rev() {
        canonical.push(component);
    }
    let digest = Sha256::digest(canonical.to_string_lossy().to_lowercase().as_bytes());
    let mut hex = String::with_capacity(64);
    use std::fmt::Write as _;
    for byte in digest {
        write!(&mut hex, "{byte:02x}")?;
    }
    Ok(hex)
}
pub(crate) fn current_identity() -> Result<ControlIdentity> {
    let (user, session) = identity()?;
    Ok(ControlIdentity {
        user,
        session,
        scope: scope_hash(&crate::settings::data_dir()?)?,
    })
}
fn process_created(process: RawHandle) -> Result<u64> {
    let mut created = FileTime::default();
    let mut exited = FileTime::default();
    let mut kernel = FileTime::default();
    let mut user = FileTime::default();
    unsafe {
        check(GetProcessTimes(
            process,
            &mut created,
            &mut exited,
            &mut kernel,
            &mut user,
        ))?;
    }
    Ok((u64::from(created.high) << 32) | u64::from(created.low))
}
pub(crate) fn current_process_created() -> Result<u64> {
    process_created(unsafe { GetCurrentProcess() })
}
pub(crate) struct ProcessLease(Handle);
impl ProcessLease {
    pub(crate) fn exited(&self) -> Result<bool> {
        match unsafe { WaitForSingleObject(self.0.0, 0) } {
            0 => Ok(true),
            258 => Ok(false),
            _ => bail!("initiating process lifetime cannot be observed"),
        }
    }
}
fn open_invoker(identity: &ControlIdentity, pid: u32, created: u64, access: u32) -> Result<Handle> {
    validate_identity(identity)?;
    ensure!(pid != 0 && created != 0, "invalid invoker process identity");
    let process = Handle::new(unsafe { OpenProcess(access, 0, pid) })?;
    ensure!(
        process_created(process.0)? == created,
        "invoker process instance changed"
    );
    ensure!(
        process_sid(process.0)? == identity.user,
        "invoker user SID mismatch"
    );
    let mut session = 0;
    unsafe {
        check(ProcessIdToSessionId(pid, &mut session))?;
    }
    ensure!(session == identity.session, "invoker session mismatch");
    Ok(process)
}
pub(crate) fn invoker_lease(
    identity: &ControlIdentity,
    pid: u32,
    created: u64,
) -> Result<ProcessLease> {
    Ok(ProcessLease(open_invoker(
        identity, pid, created, 0x00101000,
    )?))
}
pub(crate) fn validate_invoker(identity: &ControlIdentity, pid: u32, created: u64) -> Result<()> {
    drop(open_invoker(identity, pid, created, 0x1000)?);
    Ok(())
}
fn grant_readonly_sid(handle: RawHandle, user: &str, access: u32) -> Result<()> {
    ensure!(
        matches!(access, 0x1000 | 8),
        "query delegation cannot grant mutation/duplication/impersonation rights"
    );
    let mut old_acl = ptr::null_mut();
    let mut descriptor = ptr::null_mut();
    let error = unsafe {
        GetSecurityInfo(
            handle,
            6,
            4,
            ptr::null_mut(),
            ptr::null_mut(),
            &mut old_acl,
            ptr::null_mut(),
            &mut descriptor,
        )
    };
    ensure!(
        error == 0,
        "helper query ACL unreadable: Windows error {error}"
    );
    let _descriptor = Local(descriptor);
    ensure!(
        !old_acl.is_null(),
        "helper object has an unrestricted ACL; readonly delegation refused"
    );
    let mut user_sid = ptr::null_mut();
    let text = wide(user);
    unsafe {
        check(ConvertStringSidToSidW(text.as_ptr(), &mut user_sid))?;
    }
    let _sid = Local(user_sid);
    let entry = ExplicitAccess {
        permissions: access,
        mode: 1,
        inheritance: 0,
        trustee: Trustee {
            multiple: ptr::null_mut(),
            operation: 0,
            form: 0,
            kind: 0,
            name: user_sid.cast(),
        },
    };
    let mut acl = ptr::null_mut();
    let error = unsafe { SetEntriesInAclW(1, &entry, old_acl, &mut acl) };
    ensure!(
        error == 0,
        "helper readonly query ACE could not be built: Windows error {error}"
    );
    let _acl = Local(acl);
    let error =
        unsafe { SetSecurityInfo(handle, 6, 4, ptr::null(), ptr::null(), acl, ptr::null()) };
    ensure!(
        error == 0,
        "helper readonly query ACE could not be committed: Windows error {error}"
    );
    Ok(())
}
pub(crate) fn grant_invoker_query(
    identity: &ControlIdentity,
    pid: u32,
    created: u64,
) -> Result<()> {
    let _invoker = open_invoker(identity, pid, created, 0x1000)?;
    let mut raw_token = ptr::null_mut();
    // Only the helper itself opens WRITE_DAC. The invoker is granted QUERY only.
    unsafe {
        check(OpenProcessToken(
            GetCurrentProcess(),
            8 | 0x00020000 | 0x00040000,
            &mut raw_token,
        ))?;
    }
    let token = Handle::new(raw_token)?;
    let mut elevated = 0u32;
    let mut needed = 0;
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
        "query delegation requires the actually elevated camera helper"
    );
    grant_readonly_sid(unsafe { GetCurrentProcess() }, &identity.user, 0x1000)?;
    grant_readonly_sid(token.0, &identity.user, 8)?;
    Ok(())
}
pub(crate) fn prepare_invoker_query() -> Result<()> {
    let mut raw_token = ptr::null_mut();
    unsafe {
        check(OpenProcessToken(
            GetCurrentProcess(),
            8 | 0x00020000 | 0x00040000,
            &mut raw_token,
        ))?;
    }
    let token = Handle::new(raw_token)?;
    // Trusted elevated Administrators can validate this exact caller, never use its token.
    // This is invoked only immediately before the user's explicit runas operation.
    grant_readonly_sid(unsafe { GetCurrentProcess() }, "S-1-5-32-544", 0x1000)?;
    grant_readonly_sid(token.0, "S-1-5-32-544", 8)?;
    Ok(())
}
pub(crate) fn session_id() -> Result<u32> {
    Ok(identity()?.1)
}
fn endpoint(name: &str, identity: &ControlIdentity) -> Result<Vec<u16>> {
    ensure!(
        matches!(name, "microphone" | "camera"),
        "unknown control endpoint"
    );
    validate_identity(identity)?;
    Ok(wide(&format!(
        r"\\.\pipe\MicCamWatch-v1-{}-{}-{}-{name}",
        identity.user, identity.session, identity.scope
    )))
}
fn resource_name(name: &str) -> Result<Vec<u16>> {
    ensure!(
        matches!(name, "microphone" | "camera"),
        "unknown native resource"
    );
    Ok(wide(&format!(r"Global\MicCamWatch-v1-native-{name}")))
}
pub(crate) struct ResourceLease(Handle);
impl Drop for ResourceLease {
    fn drop(&mut self) {
        unsafe {
            ReleaseMutex(self.0.0);
        }
    }
}
fn descriptor_sddl(descriptor: *const c_void) -> Result<String> {
    let mut text = ptr::null_mut();
    let mut length = 0;
    unsafe {
        check(ConvertSecurityDescriptorToStringSecurityDescriptorW(
            descriptor,
            1,
            4,
            &mut text,
            &mut length,
        ))?;
    }
    let _text = Local(text.cast());
    ensure!(
        length > 0 && length <= 4096,
        "native object ACL is oversized"
    );
    Ok(String::from_utf16(unsafe {
        std::slice::from_raw_parts(text, length as usize - 1)
    })?)
}
fn verify_mutex_acl(mutex: &Handle, expected: *const c_void) -> Result<()> {
    let mut actual = ptr::null_mut();
    let error = unsafe {
        GetSecurityInfo(
            mutex.0,
            6,
            4,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            &mut actual,
        )
    };
    ensure!(
        error == 0,
        "native control ACL cannot be verified: Windows error {error}"
    );
    let _actual = Local(actual);
    ensure!(
        descriptor_sddl(actual)? == descriptor_sddl(expected)?,
        "native control mutex has an untrusted pre-existing ACL"
    );
    Ok(())
}
fn acquire_named_mutex(
    name: &[u16],
    identity: &ControlIdentity,
    timeout: u32,
) -> Result<ResourceLease> {
    validate_identity(identity)?;
    // Use the mapped native access mask, so an opened object has the same canonical ACL.
    let acl = wide(&format!(
        "D:P(A;;0x1f0001;;;{})(A;;0x1f0001;;;BA)(A;;0x1f0001;;;SY)",
        identity.user
    ));
    let mut descriptor = ptr::null_mut();
    unsafe {
        check(ConvertStringSecurityDescriptorToSecurityDescriptorW(
            acl.as_ptr(),
            1,
            &mut descriptor,
            ptr::null_mut(),
        ))?;
    }
    let _descriptor = Local(descriptor);
    let attributes = SecurityAttributes {
        length: size_of::<SecurityAttributes>() as u32,
        descriptor,
        inherit: 0,
    };
    let mutex = Handle::new(unsafe { CreateMutexW(&attributes, 0, name.as_ptr()) })?;
    verify_mutex_acl(&mutex, descriptor)?;
    match unsafe { WaitForSingleObject(mutex.0, timeout) } {
        0 | 128 => Ok(ResourceLease(mutex)),
        258 => bail!(
            "native control ownership is busy in another owner/scope; no SDK mutation permitted"
        ),
        _ => bail!(
            "native hardware ownership unknown: {}",
            std::io::Error::last_os_error()
        ),
    }
}
pub(crate) fn acquire_resource_for(
    name: &str,
    identity: &ControlIdentity,
) -> Result<ResourceLease> {
    acquire_named_mutex(&resource_name(name)?, identity, 0)
}
pub(crate) fn acquire_request_lock(name: &str) -> Result<ResourceLease> {
    ensure!(
        matches!(name, "microphone" | "camera"),
        "unknown native request lock"
    );
    let identity = current_identity()?;
    let name = wide(&format!(
        r"Local\MicCamWatch-v1-request-{}-{}-{}-{name}",
        identity.user, identity.session, identity.scope
    ));
    acquire_named_mutex(&name, &identity, 5000)
}
pub(crate) fn resource_conflict(name: &str) -> Result<bool> {
    let name = resource_name(name)?;
    let raw = unsafe { OpenMutexW(0x00100001, 0, name.as_ptr()) };
    if raw.is_null() {
        if unsafe { GetLastError() } == 2 {
            return Ok(false);
        }
        bail!(
            "native hardware owner cannot be queried: {}",
            std::io::Error::last_os_error()
        );
    }
    let mutex = Handle::new(raw)?;
    match unsafe { WaitForSingleObject(mutex.0, 0) } {
        0 | 128 => {
            drop(ResourceLease(mutex));
            Ok(false)
        }
        258 => Ok(true),
        _ => bail!("native hardware ownership query failed"),
    }
}
fn peer(
    pipe: RawHandle,
    server: bool,
    user: &str,
    session: u32,
    require_admin: bool,
) -> Result<()> {
    let mut pid = 0;
    unsafe {
        check(if server {
            GetNamedPipeClientProcessId(pipe, &mut pid)
        } else {
            GetNamedPipeServerProcessId(pipe, &mut pid)
        })?;
    }
    let process = Handle::new(unsafe { OpenProcess(0x1000, 0, pid) })?;
    let mut peer_session = 0;
    unsafe {
        check(ProcessIdToSessionId(pid, &mut peer_session))?;
    }
    ensure!(
        peer_session == session && (require_admin || process_sid(process.0)? == user),
        "control peer user/session mismatch"
    );
    if require_admin {
        let mut raw_token = ptr::null_mut();
        unsafe {
            check(OpenProcessToken(process.0, 8, &mut raw_token))?;
        }
        let token = Handle::new(raw_token)?;
        let mut elevated = 0u32;
        let mut needed = 0;
        unsafe {
            check(GetTokenInformation(
                token.0,
                20,
                (&mut elevated as *mut u32).cast(),
                4,
                &mut needed,
            ))?;
        }
        ensure!(elevated != 0, "camera control server is not elevated");
    }
    if server {
        unsafe {
            check(ImpersonateNamedPipeClient(pipe))?;
        }
        struct Revert;
        impl Drop for Revert {
            fn drop(&mut self) {
                if unsafe { RevertToSelf() } == 0 {
                    std::process::abort();
                }
            }
        }
        let _revert = Revert;
        let mut token = ptr::null_mut();
        unsafe {
            check(OpenThreadToken(GetCurrentThread(), 8, 1, &mut token))?;
        }
        let token = Handle::new(token)?;
        ensure!(sid(token.0)? == user, "impersonated control peer mismatch");
    }
    Ok(())
}
struct Pending<'a> {
    pipe: &'a Handle,
    event: Handle,
    overlapped: Box<Overlapped>,
    in_flight: bool,
}
impl<'a> Pending<'a> {
    fn new(pipe: &'a Handle) -> Result<Self> {
        let event = Handle::new(unsafe { CreateEventW(ptr::null(), 1, 0, ptr::null()) })?;
        let overlapped = Box::new(Overlapped {
            event: event.0,
            ..Default::default()
        });
        Ok(Self {
            pipe,
            event,
            overlapped,
            in_flight: false,
        })
    }
    fn finish(
        &mut self,
        immediate: i32,
        error: u32,
        deadline: Instant,
        stop: Option<&AtomicBool>,
    ) -> Result<u32> {
        if immediate == 0 {
            ensure!(
                error == 997,
                "pipe I/O: {}",
                std::io::Error::from_raw_os_error(error as i32)
            );
            self.in_flight = true;
        } else {
            let mut count = 0;
            unsafe {
                check(GetOverlappedResult(
                    self.pipe.0,
                    &mut *self.overlapped,
                    &mut count,
                    0,
                ))?;
            }
            return Ok(count);
        }
        loop {
            if stop.is_some_and(|stop| stop.load(Ordering::Acquire)) {
                bail!("control endpoint stopped");
            }
            ensure!(Instant::now() < deadline, "control pipe deadline exceeded");
            match unsafe { WaitForSingleObject(self.event.0, 50) } {
                0 => break,
                258 => continue,
                _ => bail!(
                    "control pipe wait failed: {}",
                    std::io::Error::last_os_error()
                ),
            }
        }
        let mut count = 0;
        unsafe {
            check(GetOverlappedResult(
                self.pipe.0,
                &mut *self.overlapped,
                &mut count,
                0,
            ))?;
        }
        self.in_flight = false;
        Ok(count)
    }
}
impl Drop for Pending<'_> {
    fn drop(&mut self) {
        // Only an actually queued operation needs cancellation. ConnectNamedPipe can
        // return PIPE_CONNECTED/NO_DATA without queuing I/O or signaling the event.
        // Waiting on that unsubmitted OVERLAPPED would hang the broker indefinitely.
        if self.in_flight {
            unsafe {
                CancelIoEx(self.pipe.0, &*self.overlapped);
                let mut count = 0;
                GetOverlappedResult(self.pipe.0, &mut *self.overlapped, &mut count, 1);
            }
        }
    }
}
fn transfer(pipe: &Handle, bytes: &mut [u8], writing: bool, deadline: Instant) -> Result<()> {
    let mut offset = 0;
    while offset < bytes.len() {
        let mut pending = Pending::new(pipe)?;
        let ok = unsafe {
            if writing {
                WriteFile(
                    pipe.0,
                    bytes[offset..].as_ptr().cast(),
                    (bytes.len() - offset) as u32,
                    ptr::null_mut(),
                    &mut *pending.overlapped,
                )
            } else {
                ReadFile(
                    pipe.0,
                    bytes[offset..].as_mut_ptr().cast(),
                    (bytes.len() - offset) as u32,
                    ptr::null_mut(),
                    &mut *pending.overlapped,
                )
            }
        };
        let error = if ok == 0 {
            unsafe { GetLastError() }
        } else {
            0
        };
        let count = pending.finish(ok, error, deadline, None)? as usize;
        ensure!(
            count > 0 && count <= bytes.len() - offset,
            "control frame disconnected"
        );
        offset += count;
    }
    Ok(())
}
fn receive<T: DeserializeOwned>(pipe: &Handle, maximum: usize, deadline: Instant) -> Result<T> {
    let mut header = [0; 4];
    transfer(pipe, &mut header, false, deadline)?;
    let size = u32::from_le_bytes(header) as usize;
    ensure!(
        size > 0 && size <= maximum,
        "control frame exceeds {maximum} bytes"
    );
    let mut bytes = vec![0; size];
    transfer(pipe, &mut bytes, false, deadline)?;
    serde_json::from_slice(&bytes).context("invalid typed control frame")
}
fn send<T: Serialize>(pipe: &Handle, value: &T, maximum: usize, deadline: Instant) -> Result<()> {
    let mut bytes = serde_json::to_vec(value)?;
    ensure!(
        !bytes.is_empty() && bytes.len() <= maximum,
        "control frame exceeds {maximum} bytes"
    );
    let mut header = (bytes.len() as u32).to_le_bytes();
    transfer(pipe, &mut header, true, deadline)?;
    transfer(pipe, &mut bytes, true, deadline)
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Reply<T> {
    value: Option<T>,
    error: Option<String>,
}
pub(crate) fn request<Req: Serialize, Res: DeserializeOwned>(name: &str, req: &Req) -> Result<Res> {
    request_for(name, req, &current_identity()?)
}
fn request_for<Req: Serialize, Res: DeserializeOwned>(
    name: &str,
    req: &Req,
    identity: &ControlIdentity,
) -> Result<Res> {
    let require_admin = name == "camera";
    let name = endpoint(name, identity)?;
    // Identification-only impersonation; a server cannot use the client's authority.
    let pipe = Handle::new(unsafe {
        CreateFileW(
            name.as_ptr(),
            0xc0000000,
            0,
            ptr::null(),
            3,
            0x40000000 | 0x00110000,
            ptr::null_mut(),
        )
    })?;
    peer(
        pipe.0,
        false,
        &identity.user,
        identity.session,
        require_admin,
    )?;
    let deadline = Instant::now() + Duration::from_secs(3);
    send(&pipe, req, 4096, deadline)?;
    let response_deadline = Instant::now() + Duration::from_secs(3);
    let reply: Reply<Res> = receive(&pipe, 65536, response_deadline)?;
    // A completed write only means the reply entered the pipe buffer. A bounded
    // acknowledgement keeps server disconnect from discarding an unread response.
    let mut acknowledged = [1];
    transfer(&pipe, &mut acknowledged, true, response_deadline)?;
    match (reply.value, reply.error) {
        (Some(value), None) => Ok(value),
        (None, Some(error)) => bail!("{error}"),
        _ => bail!("invalid control response"),
    }
}
pub(crate) fn serve<Req: DeserializeOwned, Res: Serialize>(
    name: &str,
    stop: &AtomicBool,
    handler: impl FnMut(Req) -> Result<Res>,
) -> Result<()> {
    serve_bound(name, stop, &current_identity()?, handler)
}
pub(crate) fn serve_bound<Req: DeserializeOwned, Res: Serialize>(
    name: &str,
    stop: &AtomicBool,
    identity: &ControlIdentity,
    handler: impl FnMut(Req) -> Result<Res>,
) -> Result<()> {
    serve_ready(name, stop, identity, handler, || {})
}
fn serve_ready<Req: DeserializeOwned, Res: Serialize>(
    name: &str,
    stop: &AtomicBool,
    identity: &ControlIdentity,
    mut handler: impl FnMut(Req) -> Result<Res>,
    ready: impl FnOnce(),
) -> Result<()> {
    validate_identity(identity)?;
    let name = endpoint(name, identity)?;
    let user = &identity.user;
    let session = identity.session;
    let acl = wide(&format!("D:P(A;;GA;;;{user})(A;;GA;;;BA)"));
    let mut descriptor = ptr::null_mut();
    unsafe {
        check(ConvertStringSecurityDescriptorToSecurityDescriptorW(
            acl.as_ptr(),
            1,
            &mut descriptor,
            ptr::null_mut(),
        ))?;
    }
    let _descriptor = Local(descriptor);
    let attributes = SecurityAttributes {
        length: size_of::<SecurityAttributes>() as u32,
        descriptor,
        inherit: 0,
    };
    // FIRST_PIPE_INSTANCE denies pre-existing/spoof endpoints; remote clients are rejected.
    let pipe = Handle::new(unsafe {
        CreateNamedPipeW(
            name.as_ptr(),
            3 | 0x40000000 | 0x00080000,
            8,
            1,
            65540,
            4100,
            0,
            &attributes,
        )
    })?;
    ready();
    while !stop.load(Ordering::Acquire) {
        let mut pending = Pending::new(&pipe)?;
        let connected = unsafe { ConnectNamedPipe(pipe.0, &mut *pending.overlapped) };
        let error = if connected == 0 {
            unsafe { GetLastError() }
        } else {
            0
        };
        let result = if error == 535 {
            Ok(())
        } else {
            pending
                .finish(
                    connected,
                    error,
                    Instant::now() + Duration::from_secs(30),
                    Some(stop),
                )
                .map(|_| ())
        };
        drop(pending);
        if result.is_err() {
            unsafe {
                DisconnectNamedPipe(pipe.0);
            }
            if stop.load(Ordering::Acquire) {
                break;
            }
            // A local peer can connect and close between pipe creation and ConnectNamedPipe.
            // These documented disconnected-client outcomes are not a broker failure.
            if !matches!(error, 0 | 997 | 109 | 232 | 233 | 995) {
                result?;
            }
            continue;
        }
        let deadline = Instant::now() + Duration::from_secs(3);
        // Each malformed/untrusted client is disconnected, never terminates the owner.
        let _ = (|| -> Result<()> {
            let req = receive::<Req>(&pipe, 4096, deadline)?;
            peer(pipe.0, true, user, session, false)?;
            let reply = match handler(req) {
                Ok(value) => Reply {
                    value: Some(value),
                    error: None,
                },
                Err(error) => {
                    let mut error = format!("{error:#}");
                    truncate_detail(&mut error, 8192);
                    Reply {
                        value: None,
                        error: Some(error),
                    }
                }
            };
            let response_deadline = Instant::now() + Duration::from_secs(3);
            send(&pipe, &reply, 65536, response_deadline)?;
            let mut acknowledged = [0];
            transfer(&pipe, &mut acknowledged, false, response_deadline)?;
            ensure!(acknowledged == [1], "invalid control acknowledgement");
            Ok(())
        })();
        unsafe {
            DisconnectNamedPipe(pipe.0);
        }
    }
    Ok(())
}
pub(crate) fn truncate_detail(detail: &mut String, maximum: usize) {
    if detail.len() > maximum {
        let mut end = maximum;
        while !detail.is_char_boundary(end) {
            end -= 1;
        }
        detail.truncate(end);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn endpoint_namespace_accepts_only_fixed_capabilities() {
        let identity = ControlIdentity {
            user: "S-1-5-21-1".into(),
            session: 2,
            scope: "a".repeat(64),
        };
        assert!(endpoint("camera", &identity).is_ok());
        assert!(endpoint("microphone", &identity).is_ok());
        assert!(endpoint(r"\\.\pipe\other", &identity).is_err());
        let mut other = identity.clone();
        other.session += 1;
        assert_ne!(
            endpoint("camera", &identity).unwrap(),
            endpoint("camera", &other).unwrap()
        );
        other = identity.clone();
        other.scope = "b".repeat(64);
        assert_ne!(
            endpoint("camera", &identity).unwrap(),
            endpoint("camera", &other).unwrap()
        );
    }
    #[test]
    fn error_budget_preserves_utf8() {
        let mut value = "界".repeat(4000);
        truncate_detail(&mut value, 8192);
        assert!(value.len() <= 8192);
        assert!(std::str::from_utf8(value.as_bytes()).is_ok());
    }
    #[test]
    fn canonical_scope_is_stable_before_directory_creation_and_isolates_other_data_roots() {
        let directory = tempfile::tempdir().unwrap();
        let first = directory.path().join("one").join("MicCamWatch");
        let before = scope_hash(&first).unwrap();
        std::fs::create_dir_all(&first).unwrap();
        assert_eq!(before, scope_hash(&first).unwrap());
        assert_ne!(
            before,
            scope_hash(&directory.path().join("two").join("MicCamWatch")).unwrap()
        );
    }
    #[test]
    fn native_typed_transport_rejects_oversized_frame_then_serves_same_user_isolated_client() {
        let directory = tempfile::tempdir().unwrap();
        let (user, session) = identity().unwrap();
        let identity = ControlIdentity {
            user,
            session,
            scope: scope_hash(directory.path()).unwrap(),
        };
        let stop = std::sync::Arc::new(AtomicBool::new(false));
        let worker_stop = std::sync::Arc::clone(&stop);
        let worker_identity = identity.clone();
        let worker = std::thread::spawn(move || {
            serve_bound::<u32, u32>("microphone", &worker_stop, &worker_identity, |value| {
                worker_stop.store(true, Ordering::Release);
                Ok(value + 1)
            })
        });
        let pipe_name = endpoint("microphone", &identity).unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        let malformed = loop {
            let raw = unsafe {
                CreateFileW(
                    pipe_name.as_ptr(),
                    0xc0000000,
                    0,
                    ptr::null(),
                    3,
                    0x40000000 | 0x00110000,
                    ptr::null_mut(),
                )
            };
            if let Ok(pipe) = Handle::new(raw) {
                break pipe;
            }
            if Instant::now() >= deadline {
                stop.store(true, Ordering::Release);
                let _ = worker.join();
                panic!("isolated transport server did not become available");
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        let mut oversized = 4097u32.to_le_bytes();
        transfer(
            &malformed,
            &mut oversized,
            true,
            Instant::now() + Duration::from_secs(1),
        )
        .unwrap();
        drop(malformed);
        let result = loop {
            match request_for::<_, u32>("microphone", &41u32, &identity) {
                Ok(value) => break value,
                Err(error) if Instant::now() < deadline => {
                    let _ = error;
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => {
                    stop.store(true, Ordering::Release);
                    let _ = worker.join();
                    panic!("isolated typed transport failed: {error:#}");
                }
            }
        };
        assert_eq!(result, 42);
        worker.join().unwrap().unwrap();
    }
    #[test]
    fn native_scoped_mutex_rejects_precreated_foreign_acl() {
        let directory = tempfile::tempdir().unwrap();
        let (user, session) = identity().unwrap();
        let identity = ControlIdentity {
            user,
            session,
            scope: scope_hash(directory.path()).unwrap(),
        };
        let name = wide(&format!(
            r"Local\MicCamWatch-test-{}-{}",
            std::process::id(),
            identity.scope
        ));
        // A real unrelated kernel object, not a source-text assertion or mock lock.
        let acl = wide("D:P(A;;0x1f0001;;;WD)");
        let mut descriptor = ptr::null_mut();
        unsafe {
            check(ConvertStringSecurityDescriptorToSecurityDescriptorW(
                acl.as_ptr(),
                1,
                &mut descriptor,
                ptr::null_mut(),
            ))
            .unwrap();
        }
        let _descriptor = Local(descriptor);
        let attributes = SecurityAttributes {
            length: size_of::<SecurityAttributes>() as u32,
            descriptor,
            inherit: 0,
        };
        let hostile = Handle::new(unsafe { CreateMutexW(&attributes, 0, name.as_ptr()) }).unwrap();
        assert!(acquire_named_mutex(&name, &identity, 0).is_err());
        drop(hostile);
        let owned = acquire_named_mutex(&name, &identity, 0).unwrap();
        drop(owned);
    }
    #[test]
    fn native_connect_then_close_before_server_connect_does_not_stop_broker() {
        let directory = tempfile::tempdir().unwrap();
        let (user, session) = identity().unwrap();
        let identity = ControlIdentity {
            user,
            session,
            scope: scope_hash(directory.path()).unwrap(),
        };
        let stop = std::sync::Arc::new(AtomicBool::new(false));
        let worker_stop = std::sync::Arc::clone(&stop);
        let worker_identity = identity.clone();
        let (created, pipe_created) = std::sync::mpsc::sync_channel(1);
        let (closed, client_closed) = std::sync::mpsc::sync_channel(1);
        let worker = std::thread::spawn(move || {
            serve_ready::<u32, u32>(
                "microphone",
                &worker_stop,
                &worker_identity,
                |value| {
                    worker_stop.store(true, Ordering::Release);
                    Ok(value + 1)
                },
                || {
                    created.send(()).unwrap();
                    client_closed.recv_timeout(Duration::from_secs(3)).unwrap();
                },
            )
        });
        pipe_created.recv_timeout(Duration::from_secs(3)).unwrap();
        let name = endpoint("microphone", &identity).unwrap();
        let early = Handle::new(unsafe {
            CreateFileW(
                name.as_ptr(),
                0xc0000000,
                0,
                ptr::null(),
                3,
                0x40000000 | 0x00110000,
                ptr::null_mut(),
            )
        })
        .unwrap();
        drop(early);
        closed.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        let result = loop {
            match request_for::<_, u32>("microphone", &9u32, &identity) {
                Ok(value) => break value,
                Err(error) if Instant::now() < deadline => {
                    let _ = error;
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => {
                    stop.store(true, Ordering::Release);
                    let _ = worker.join();
                    panic!("broker stopped after early disconnect: {error:#}");
                }
            }
        };
        assert_eq!(result, 10);
        worker.join().unwrap().unwrap();
    }
    #[test]
    fn native_process_lease_tracks_owned_instance_exit_and_rejects_reused_birth_identity() {
        use std::{
            io::Write,
            os::windows::{io::AsRawHandle, process::CommandExt},
            process::{Command, Stdio},
        };
        struct ChildFixture(std::process::Child);
        impl Drop for ChildFixture {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let command = std::path::PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("System32")
            .join("cmd.exe");
        // A private stdin-blocked built-in process, with no capture/SDK/UAC/device action.
        let mut child = ChildFixture(
            Command::new(command)
                .args(["/D", "/C", "set /P mcw_fixture="])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .creation_flags(0x08000000)
                .spawn()
                .unwrap(),
        );
        let (user, session) = identity().unwrap();
        let directory = tempfile::tempdir().unwrap();
        let identity = ControlIdentity {
            user,
            session,
            scope: scope_hash(directory.path()).unwrap(),
        };
        let created = process_created(child.0.as_raw_handle()).unwrap();
        assert!(invoker_lease(&identity, child.0.id(), created + 1).is_err());
        let lease = invoker_lease(&identity, child.0.id(), created).unwrap();
        assert!(!lease.exited().unwrap());
        child.0.stdin.take().unwrap().write_all(b"\r\n").unwrap();
        child.0.wait().unwrap();
        assert!(lease.exited().unwrap());
    }
    #[test]
    fn restricted_medium_token_can_query_delegated_owned_child_but_cannot_use_its_token() {
        use std::{
            os::windows::{io::AsRawHandle, process::CommandExt},
            process::{Command, Stdio},
        };
        #[repr(C)]
        struct SidAttributes {
            sid: *mut c_void,
            attributes: u32,
        }
        #[link(name = "advapi32")]
        unsafe extern "system" {
            fn CreateRestrictedToken(
                existing: RawHandle,
                flags: u32,
                disabled_count: u32,
                disabled: *const SidAttributes,
                deleted_count: u32,
                deleted: *const c_void,
                restricted_count: u32,
                restricted: *const SidAttributes,
                result: *mut RawHandle,
            ) -> i32;
            fn SetTokenInformation(
                token: RawHandle,
                class: u32,
                information: *const c_void,
                length: u32,
            ) -> i32;
            fn GetLengthSid(sid: *const c_void) -> u32;
            fn GetSecurityDescriptorDacl(
                descriptor: *const c_void,
                present: *mut i32,
                acl: *mut *mut c_void,
                defaulted: *mut i32,
            ) -> i32;
            fn ImpersonateLoggedOnUser(token: RawHandle) -> i32;
        }
        struct ChildFixture(std::process::Child);
        impl Drop for ChildFixture {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        struct Revert;
        impl Drop for Revert {
            fn drop(&mut self) {
                if unsafe { RevertToSelf() } == 0 {
                    std::process::abort();
                }
            }
        }
        let command = std::path::PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("System32")
            .join("cmd.exe");
        let child = ChildFixture(
            Command::new(command)
                .args(["/D", "/C", "set /P mcw_fixture="])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .creation_flags(0x08000000)
                .spawn()
                .unwrap(),
        );
        let process = child.0.as_raw_handle();
        let user = process_sid(process).unwrap();
        let mut child_token = ptr::null_mut();
        unsafe {
            check(OpenProcessToken(
                process,
                8 | 0x00020000 | 0x00040000,
                &mut child_token,
            ))
            .unwrap();
        }
        let child_token = Handle::new(child_token).unwrap();
        let acl_text = wide("D:P(A;;GA;;;BA)(A;;GA;;;SY)");
        let mut descriptor = ptr::null_mut();
        unsafe {
            check(ConvertStringSecurityDescriptorToSecurityDescriptorW(
                acl_text.as_ptr(),
                1,
                &mut descriptor,
                ptr::null_mut(),
            ))
            .unwrap();
        }
        let _descriptor = Local(descriptor);
        let mut acl = ptr::null_mut();
        let mut present = 0;
        let mut defaulted = 0;
        unsafe {
            check(GetSecurityDescriptorDacl(
                descriptor,
                &mut present,
                &mut acl,
                &mut defaulted,
            ))
            .unwrap();
        }
        assert_ne!(present, 0);
        for object in [process, child_token.0] {
            let error = unsafe {
                SetSecurityInfo(
                    object,
                    6,
                    4 | 0x80000000,
                    ptr::null(),
                    ptr::null(),
                    acl,
                    ptr::null(),
                )
            };
            assert_eq!(error, 0);
        }
        let mut source_token = ptr::null_mut();
        unsafe {
            check(OpenProcessToken(
                GetCurrentProcess(),
                2 | 8 | 0x80,
                &mut source_token,
            ))
            .unwrap();
        }
        let source_token = Handle::new(source_token).unwrap();
        let mut admin_sid = ptr::null_mut();
        let mut user_sid = ptr::null_mut();
        let mut medium_sid = ptr::null_mut();
        unsafe {
            check(ConvertStringSidToSidW(
                wide("S-1-5-32-544").as_ptr(),
                &mut admin_sid,
            ))
            .unwrap();
            check(ConvertStringSidToSidW(wide(&user).as_ptr(), &mut user_sid)).unwrap();
            check(ConvertStringSidToSidW(
                wide("S-1-16-8192").as_ptr(),
                &mut medium_sid,
            ))
            .unwrap();
        }
        let _admin = Local(admin_sid);
        let _user = Local(user_sid);
        let _medium = Local(medium_sid);
        let disabled = SidAttributes {
            sid: admin_sid,
            attributes: 0,
        };
        let restricting = SidAttributes {
            sid: user_sid,
            attributes: 0,
        };
        let mut restricted = ptr::null_mut();
        unsafe {
            check(CreateRestrictedToken(
                source_token.0,
                1,
                1,
                &disabled,
                0,
                ptr::null(),
                1,
                &restricting,
                &mut restricted,
            ))
            .unwrap();
        }
        let restricted = Handle::new(restricted).unwrap();
        let integrity = SidAttributes {
            sid: medium_sid,
            attributes: 0x20,
        };
        unsafe {
            check(SetTokenInformation(
                restricted.0,
                25,
                (&integrity as *const SidAttributes).cast(),
                size_of::<SidAttributes>() as u32 + GetLengthSid(medium_sid),
            ))
            .unwrap();
        }
        unsafe {
            check(ImpersonateLoggedOnUser(restricted.0)).unwrap();
        }
        let revert = Revert;
        let before = unsafe { OpenProcess(0x1000, 0, child.0.id()) };
        if !before.is_null() {
            drop(Handle::new(before).unwrap());
        }
        assert!(
            before.is_null(),
            "restricted caller must not inherit administrator process-query authority"
        );
        drop(revert);
        grant_readonly_sid(process, &user, 0x1000).unwrap();
        grant_readonly_sid(child_token.0, &user, 8).unwrap();
        unsafe {
            check(ImpersonateLoggedOnUser(restricted.0)).unwrap();
        }
        let _revert = Revert;
        let readonly_process =
            Handle::new(unsafe { OpenProcess(0x1000, 0, child.0.id()) }).unwrap();
        let mut readonly_token = ptr::null_mut();
        unsafe {
            check(OpenProcessToken(readonly_process.0, 8, &mut readonly_token)).unwrap();
        }
        let readonly_token = Handle::new(readonly_token).unwrap();
        assert_eq!(sid(readonly_token.0).unwrap(), user);
        let mut elevated = 0u32;
        let mut needed = 0;
        unsafe {
            check(GetTokenInformation(
                readonly_token.0,
                20,
                (&mut elevated as *mut u32).cast(),
                4,
                &mut needed,
            ))
            .unwrap();
        }
        for denied in [2u32, 4, 0x20, 0x80] {
            let mut raw = ptr::null_mut();
            let result = unsafe { OpenProcessToken(readonly_process.0, denied, &mut raw) };
            if !raw.is_null() {
                drop(Handle::new(raw).unwrap());
            }
            assert_eq!(
                result, 0,
                "readonly query must not delegate token-use/adjust rights"
            );
        }
        let terminate = unsafe { OpenProcess(1, 0, child.0.id()) };
        if !terminate.is_null() {
            drop(Handle::new(terminate).unwrap());
        }
        assert!(
            terminate.is_null(),
            "readonly query must not delegate process termination"
        );
    }
}
