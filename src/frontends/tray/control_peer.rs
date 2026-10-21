//! Socket-bound identity and exit observation only: never numeric-PID signalling.
//!
//! Image verification is a bounded observation, not an atomic exec lock. A peer
//! can exec after the last check; callers must send commands on the authenticated
//! socket, not convert `pid()` into signalling authority. Missing kernel evidence
//! is an error, including when a PID has disappeared before retention succeeds.

use anyhow::{Context, Result, bail};
use std::{
    cell::Cell,
    fs, io,
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::{fs::MetadataExt, net::UnixStream},
    },
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

pub(super) struct PeerProcess {
    pid: u32,
    uid: u32,
    authority: OwnedFd,
    instance: String,
    exited: Cell<bool>,
    #[cfg(target_os = "macos")]
    token: mac::AuditToken,
    #[cfg(target_os = "macos")]
    exec_changed: Cell<bool>,
}

struct Image {
    instance: String,
    path: PathBuf,
    metadata: fs::Metadata,
    uid: u32,
}

impl PeerProcess {
    pub(super) fn retain(stream: &UnixStream) -> Result<Self> {
        let (pid, uid) = socket_credentials(stream)?;
        if pid <= 1 || pid > i32::MAX as u32 {
            bail!("control socket peer PID is not a sane application PID");
        }
        if uid != unsafe { libc::geteuid() } {
            bail!("control socket peer belongs to a different effective UID");
        }
        #[cfg(target_os = "linux")]
        let peer = {
            // SO_PEERPIDFD binds the fd to the socket's original peer, unlike
            // pidfd_open(SO_PEERCRED.pid), which can retain a reused PID when
            // an inherited socket outlives its original process.
            const SO_PEERPIDFD: libc::c_int = 77;
            let mut fd: libc::c_int = -1;
            let mut size = std::mem::size_of_val(&fd) as libc::socklen_t;
            let result = unsafe {
                libc::getsockopt(
                    stream.as_raw_fd(),
                    libc::SOL_SOCKET,
                    SO_PEERPIDFD,
                    (&mut fd as *mut libc::c_int).cast(),
                    &mut size,
                )
            };
            if result != 0 {
                return Err(io::Error::last_os_error()).context(
                    "cannot retain socket-bound peer pidfd (Linux 6.5+ required for stopping an active tray); exit the owned tray via its menu, then retry uninstall",
                );
            }
            if fd < 0 {
                bail!("SO_PEERPIDFD returned no exit authority");
            }
            let authority = unsafe { OwnedFd::from_raw_fd(fd) };
            if size as usize != std::mem::size_of::<libc::c_int>() {
                bail!("SO_PEERPIDFD returned an unexpected descriptor size");
            }
            close_on_exec(&authority)?;
            let mut peer = Self {
                pid,
                uid,
                authority,
                instance: String::new(),
                exited: Cell::new(false),
            };
            peer.require_live()?;
            let before = peer.image()?;
            peer.instance = before.instance;
            peer.require_live()?;
            let after = peer.image()?;
            if after.instance != peer.instance || after.uid != uid {
                bail!("control peer identity changed while retaining its pidfd");
            }
            peer
        };
        #[cfg(target_os = "macos")]
        let peer = {
            let token = mac::socket_token(stream, pid, uid)?;
            // Audited lookup rejects a stale token even if the numeric PID was
            // reused before kqueue registration. Bracket registration and birth
            // observation with it; never infer retention from ESRCH.
            mac::audited_path(&token)?;
            let before = mac::birth(pid, uid)?;
            let fd = unsafe { libc::kqueue() };
            if fd < 0 {
                return Err(io::Error::last_os_error()).context("cannot create peer exit kqueue");
            }
            let authority = unsafe { OwnedFd::from_raw_fd(fd) };
            close_on_exec(&authority)?;
            mac::register(&authority, pid)?;
            let peer = Self {
                pid,
                uid,
                authority,
                instance: before,
                token,
                exited: Cell::new(false),
                exec_changed: Cell::new(false),
            };
            peer.require_live()?;
            let after = mac::birth(pid, uid)?;
            mac::audited_path(&peer.token)?;
            if after != peer.instance {
                bail!("control peer birth identity changed during kqueue registration");
            }
            peer.require_live()?;
            peer
        };
        if socket_credentials(stream)? != (pid, uid) {
            bail!("control socket credentials changed during authority retention");
        }
        Ok(peer)
    }

    pub(super) fn pid(&self) -> u32 {
        self.pid
    }

    pub(super) fn verify_image(&self, expected: &Path) -> Result<()> {
        self.require_live()?;
        let expected_path = fs::canonicalize(expected)
            .context("cannot canonicalize expected control peer executable")?;
        let expected_file = fs::metadata(&expected_path)
            .context("cannot inspect expected control peer executable")?;
        let before = self.image()?;
        self.check_image(&before, &expected_path, &expected_file)?;
        self.require_live()?;
        let after = self.image()?;
        self.check_image(&after, &expected_path, &expected_file)?;
        if before.instance != after.instance
            || !same_file(&before.metadata, &after.metadata)
            || fs::canonicalize(expected)? != expected_path
            || !same_file(&expected_file, &fs::metadata(&expected_path)?)
        {
            bail!("control peer executable changed during verification");
        }
        self.require_live()
    }

    fn check_image(&self, image: &Image, expected: &Path, file: &fs::Metadata) -> Result<()> {
        if image.instance != self.instance
            || image.uid != self.uid
            || image.path != expected
            || !same_file(&image.metadata, file)
        {
            bail!(
                "control socket peer is not the expected executable and retained process instance"
            );
        }
        Ok(())
    }

    fn image(&self) -> Result<Image> {
        #[cfg(target_os = "linux")]
        {
            let (instance, executable, uid) =
                crate::platform::linux::verified_process_identity(self.pid)?;
            let path = fs::canonicalize(&executable)
                .context("cannot canonicalize control peer executable")?;
            let metadata = fs::metadata(format!("/proc/{}/exe", self.pid))
                .context("cannot inspect running control peer executable inode")?;
            if !same_file(&metadata, &fs::metadata(&path)?) {
                bail!("running control peer image no longer matches its executable path");
            }
            let final_identity = crate::platform::linux::verified_process_identity(self.pid)?;
            if final_identity.0 != instance
                || final_identity.1 != executable
                || final_identity.2 != uid
                || !same_file(&metadata, &fs::metadata(format!("/proc/{}/exe", self.pid))?)
            {
                bail!("control peer identity changed during procfs image lookup");
            }
            Ok(Image {
                instance,
                path,
                metadata,
                uid,
            })
        }
        #[cfg(target_os = "macos")]
        {
            let instance = mac::birth(self.pid, self.uid)?;
            let audited = mac::audited_path(&self.token)?;
            let executable = mac::pid_path(self.pid)?;
            if audited != executable {
                bail!("audited control peer image differs from numeric-PID image lookup");
            }
            let path = fs::canonicalize(&executable)
                .context("cannot canonicalize control peer executable")?;
            let metadata = fs::metadata(&path).context("cannot inspect control peer executable")?;
            mac::verify_mapped_image(self.pid, &path, &metadata)?;
            if mac::birth(self.pid, self.uid)? != instance
                || mac::audited_path(&self.token)? != executable
                || mac::pid_path(self.pid)? != executable
                || !same_file(&metadata, &fs::metadata(&path)?)
            {
                bail!("control peer identity changed during libproc image lookup");
            }
            Ok(Image {
                instance,
                path,
                metadata,
                uid: self.uid,
            })
        }
    }

    fn require_live(&self) -> Result<()> {
        if self.observe_exit(Duration::ZERO)? {
            bail!("retained control peer has exited; refusing executable authentication");
        }
        #[cfg(target_os = "macos")]
        if self.exec_changed.get() {
            bail!("retained control peer executed a new image; refusing authentication");
        }
        Ok(())
    }

    pub(super) fn wait_exit(&self, timeout: Duration) -> Result<()> {
        let deadline = Instant::now()
            .checked_add(timeout)
            .context("control peer exit timeout exceeds the monotonic clock range")?;
        loop {
            if self.observe_exit(deadline.saturating_duration_since(Instant::now()))? {
                return Ok(());
            }
            if Instant::now() >= deadline {
                bail!(
                    "timed out waiting for retained control peer PID {} to exit",
                    self.pid
                );
            }
        }
    }

    fn observe_exit(&self, timeout: Duration) -> Result<bool> {
        if self.exited.get() {
            return Ok(true);
        }
        #[cfg(target_os = "linux")]
        {
            let mut event = libc::pollfd {
                fd: self.authority.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            // Round up sub-millisecond waits; cap each syscall, not the deadline.
            let millis = timeout
                .as_millis()
                .saturating_add(u128::from(
                    !timeout.subsec_nanos().is_multiple_of(1_000_000),
                ))
                .min(i32::MAX as u128) as i32;
            let result = unsafe { libc::poll(&mut event, 1, millis) };
            if result < 0 {
                let error = io::Error::last_os_error();
                return Err(error).context("cannot poll retained control peer pidfd");
            }
            if event.revents & (libc::POLLNVAL | libc::POLLERR) != 0 {
                bail!("retained control peer pidfd reported invalid/error readiness");
            }
            if event.revents & libc::POLLIN != 0 {
                self.exited.set(true);
            } else if event.revents != 0 {
                bail!("retained control peer pidfd reported unrecognized readiness");
            }
        }
        #[cfg(target_os = "macos")]
        {
            let mut event: libc::kevent = unsafe { std::mem::zeroed() };
            let timeout = libc::timespec {
                tv_sec: timeout.as_secs().min(i32::MAX as u64) as libc::time_t,
                tv_nsec: timeout.subsec_nanos() as libc::c_long,
            };
            let result = unsafe {
                libc::kevent(
                    self.authority.as_raw_fd(),
                    std::ptr::null(),
                    0,
                    &mut event,
                    1,
                    &timeout,
                )
            };
            if result < 0 {
                let error = io::Error::last_os_error();
                return Err(error).context("cannot observe retained control peer kqueue");
            }
            if result > 0 {
                if event.ident != self.pid as libc::uintptr_t
                    || event.filter != libc::EVFILT_PROC
                    || event.flags & libc::EV_ERROR != 0
                {
                    bail!("unexpected event from retained control peer kqueue");
                }
                if event.fflags & libc::NOTE_EXEC != 0 {
                    self.exec_changed.set(true);
                }
                if event.fflags & libc::NOTE_EXIT != 0 {
                    self.exited.set(true);
                }
                if event.fflags & (libc::NOTE_EXIT | libc::NOTE_EXEC) == 0 {
                    bail!("retained control peer kqueue supplied no recognized process evidence");
                }
            }
        }
        Ok(self.exited.get())
    }
}

fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.is_file()
        && right.is_file()
        && left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

fn close_on_exec(fd: &OwnedFd) -> Result<()> {
    if unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
        return Err(io::Error::last_os_error())
            .context("cannot make peer exit authority close-on-exec");
    }
    Ok(())
}

fn socket_credentials(stream: &UnixStream) -> Result<(u32, u32)> {
    #[cfg(target_os = "linux")]
    {
        let mut credentials: libc::ucred = unsafe { std::mem::zeroed() };
        let mut size = std::mem::size_of_val(&credentials) as libc::socklen_t;
        if unsafe {
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                (&mut credentials as *mut libc::ucred).cast(),
                &mut size,
            )
        } != 0
        {
            return Err(io::Error::last_os_error())
                .context("cannot read control socket SO_PEERCRED");
        }
        if size as usize != std::mem::size_of::<libc::ucred>() || credentials.pid <= 1 {
            bail!("control socket supplied invalid peer credentials");
        }
        Ok((credentials.pid as u32, credentials.uid))
    }
    #[cfg(target_os = "macos")]
    {
        let (mut uid, mut gid) = (0, 0);
        if unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) } != 0 {
            return Err(io::Error::last_os_error())
                .context("cannot read control socket getpeereid");
        }
        let mut pid: libc::pid_t = 0;
        let mut size = std::mem::size_of_val(&pid) as libc::socklen_t;
        if unsafe {
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_LOCAL,
                libc::LOCAL_PEERPID,
                (&mut pid as *mut libc::pid_t).cast(),
                &mut size,
            )
        } != 0
        {
            return Err(io::Error::last_os_error())
                .context("cannot read control socket LOCAL_PEERPID");
        }
        if size as usize != std::mem::size_of::<libc::pid_t>() || pid <= 1 {
            bail!("control socket supplied invalid peer PID");
        }
        Ok((pid as u32, uid))
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use super::*;
    use std::{ffi::OsStr, os::unix::ffi::OsStrExt};

    // Public SDK mach/message.h storage. Interpret only using libbsm accessors.
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub(super) struct AuditToken {
        opaque: [u32; 8],
    }

    #[link(name = "bsm")]
    unsafe extern "C" {
        fn audit_token_to_pid(token: AuditToken) -> libc::pid_t;
        fn audit_token_to_euid(token: AuditToken) -> libc::uid_t;
        fn audit_token_to_pidversion(token: AuditToken) -> libc::c_int;
    }
    #[link(name = "proc")]
    unsafe extern "C" {}

    pub(super) fn socket_token(stream: &UnixStream, pid: u32, uid: u32) -> Result<AuditToken> {
        let mut token = AuditToken { opaque: [0; 8] };
        let mut size = std::mem::size_of_val(&token) as libc::socklen_t;
        if unsafe {
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_LOCAL,
                libc::LOCAL_PEERTOKEN,
                (&mut token as *mut AuditToken).cast(),
                &mut size,
            )
        } != 0
        {
            return Err(io::Error::last_os_error())
                .context("cannot read control socket LOCAL_PEERTOKEN");
        }
        if size as usize != std::mem::size_of::<AuditToken>()
            || unsafe { audit_token_to_pid(token) } != pid as libc::pid_t
            || unsafe { audit_token_to_euid(token) } != uid
            || unsafe { audit_token_to_pidversion(token) } == 0
        {
            bail!("control socket audit token does not match its UID/PID credentials");
        }
        Ok(token)
    }

    fn path_result(buffer: &[u8], result: libc::c_int, operation: &str) -> Result<PathBuf> {
        if result <= 0 {
            return Err(io::Error::last_os_error()).with_context(|| format!("{operation} failed"));
        }
        let end = buffer
            .iter()
            .position(|byte| *byte == 0)
            .context("libproc executable path has no terminator")?;
        let path = PathBuf::from(OsStr::from_bytes(&buffer[..end]));
        if !path.is_absolute() {
            bail!("libproc executable path is not absolute");
        }
        Ok(path)
    }

    pub(super) fn pid_path(pid: u32) -> Result<PathBuf> {
        let mut buffer = [0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
        let result = unsafe {
            libc::proc_pidpath(
                pid as libc::pid_t,
                buffer.as_mut_ptr().cast(),
                buffer.len() as u32,
            )
        };
        path_result(&buffer, result, "proc_pidpath")
    }

    pub(super) fn audited_path(token: &AuditToken) -> Result<PathBuf> {
        // SDK-exported since macOS 11. Resolve at runtime so older installations
        // refuse this operation safely instead of making the entire app unloadable.
        type Lookup = unsafe extern "C" fn(*mut AuditToken, *mut libc::c_void, u32) -> libc::c_int;
        let symbol =
            unsafe { libc::dlsym(libc::RTLD_DEFAULT, c"proc_pidpath_audittoken".as_ptr()) };
        if symbol.is_null() {
            bail!(
                "macOS cannot bind a socket audit PID version to a live process; exit the owned tray via its menu, then retry uninstall"
            );
        }
        let lookup: Lookup = unsafe { std::mem::transmute(symbol) };
        let mut token = *token;
        let mut buffer = [0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
        let result = unsafe { lookup(&mut token, buffer.as_mut_ptr().cast(), buffer.len() as u32) };
        path_result(
            &buffer,
            result,
            "proc_pidpath_audittoken (socket PID-version verification)",
        )
    }

    pub(super) fn birth(pid: u32, uid: u32) -> Result<String> {
        let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
        let size = std::mem::size_of_val(&info) as libc::c_int;
        let result = unsafe {
            libc::proc_pidinfo(
                pid as libc::pid_t,
                libc::PROC_PIDTBSDINFO,
                0,
                (&mut info as *mut libc::proc_bsdinfo).cast(),
                size,
            )
        };
        if result != size {
            return Err(io::Error::last_os_error())
                .context("cannot read complete control peer birth identity");
        }
        if info.pbi_pid != pid
            || info.pbi_uid != uid
            || info.pbi_start_tvsec == 0
            || info.pbi_start_tvusec >= 1_000_000
        {
            bail!("control peer birth/UID identity is unavailable or mismatched");
        }
        Ok(format!(
            "macos:{pid}:{}:{}",
            info.pbi_start_tvsec, info.pbi_start_tvusec
        ))
    }

    pub(super) fn register(queue: &OwnedFd, pid: u32) -> Result<()> {
        let change = libc::kevent {
            ident: pid as libc::uintptr_t,
            filter: libc::EVFILT_PROC,
            flags: libc::EV_ADD | libc::EV_ENABLE | libc::EV_CLEAR | libc::EV_RECEIPT,
            fflags: libc::NOTE_EXIT | libc::NOTE_EXEC,
            data: 0,
            udata: std::ptr::null_mut(),
        };
        let mut receipt: libc::kevent = unsafe { std::mem::zeroed() };
        let timeout = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        let result =
            unsafe { libc::kevent(queue.as_raw_fd(), &change, 1, &mut receipt, 1, &timeout) };
        if result < 0 {
            return Err(io::Error::last_os_error())
                .context("cannot register retained peer NOTE_EXIT authority");
        }
        if result != 1
            || receipt.ident != pid as libc::uintptr_t
            || receipt.filter != libc::EVFILT_PROC
            || receipt.flags & libc::EV_ERROR == 0
        {
            bail!("kqueue did not acknowledge control peer process registration");
        }
        if receipt.data != 0 {
            return Err(io::Error::from_raw_os_error(receipt.data as i32))
                .context("kqueue rejected control peer process registration");
        }
        Ok(())
    }

    // Public sys/proc_info.h PROC_PIDREGIONPATHINFO ABI (not exposed by libc).
    // It supplies the running mapping's vnode, not merely stat(proc_pidpath),
    // which could authenticate a replacement file over an older running image.
    #[repr(C)]
    struct RegionInfo {
        protection: u32,
        max_protection: u32,
        inheritance: u32,
        flags: u32,
        offset: u64,
        behavior: u32,
        user_wired_count: u32,
        user_tag: u32,
        pages_resident: u32,
        pages_shared_now_private: u32,
        pages_swapped_out: u32,
        pages_dirtied: u32,
        ref_count: u32,
        shadow_depth: u32,
        share_mode: u32,
        private_pages_resident: u32,
        shared_pages_resident: u32,
        obj_id: u32,
        depth: u32,
        address: u64,
        size: u64,
    }
    #[repr(C)]
    struct RegionWithPath {
        region: RegionInfo,
        vnode: libc::vnode_info_path,
    }

    pub(super) fn verify_mapped_image(
        pid: u32,
        path: &Path,
        metadata: &fs::Metadata,
    ) -> Result<()> {
        const PROC_PIDREGIONPATHINFO: libc::c_int = 8;
        let mut address = 0u64;
        // Deliberate hard bound: unusual/inaccessible layouts refuse authority.
        for _ in 0..4096 {
            let mut info: RegionWithPath = unsafe { std::mem::zeroed() };
            let size = std::mem::size_of_val(&info) as libc::c_int;
            let result = unsafe {
                libc::proc_pidinfo(
                    pid as libc::pid_t,
                    PROC_PIDREGIONPATHINFO,
                    address,
                    (&mut info as *mut RegionWithPath).cast(),
                    size,
                )
            };
            if result != size {
                return Err(io::Error::last_os_error())
                    .context("cannot verify control peer executable mapping vnode");
            }
            let stat = &info.vnode.vip_vi.vi_stat;
            // Mach-O's header mapping can be read-only; its executable segment
            // is a later mapping. Require executable permission, not offset zero.
            if info.region.protection & libc::VM_PROT_EXECUTE as u32 != 0
                && stat.vst_dev as u32 == metadata.dev() as u32
                && stat.vst_ino == metadata.ino()
                && stat.vst_size >= 0
                && stat.vst_size as u64 == metadata.len()
                && stat.vst_mtime == metadata.mtime()
                && stat.vst_mtimensec == metadata.mtime_nsec()
                && stat.vst_ctime == metadata.ctime()
                && stat.vst_ctimensec == metadata.ctime_nsec()
            {
                let raw = unsafe {
                    std::slice::from_raw_parts(info.vnode.vip_path.as_ptr().cast::<u8>(), 1024)
                };
                let end = raw
                    .iter()
                    .position(|byte| *byte == 0)
                    .context("control peer mapping path is unterminated")?;
                if fs::canonicalize(Path::new(OsStr::from_bytes(&raw[..end])))? == path {
                    return Ok(());
                }
            }
            address = info
                .region
                .address
                .checked_add(info.region.size)
                .filter(|next| *next > address)
                .context("control peer mapping enumeration did not advance")?;
        }
        bail!("control peer executable vnode not found within bounded mapping inspection")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        os::unix::net::UnixListener,
        process::Command,
    };

    #[test]
    fn native_peer_child() {
        let Some(path) = std::env::var_os("MCW_CONTROL_PEER_FIXTURE_SOCKET") else {
            return;
        };
        let mut socket = UnixStream::connect(path).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(15)))
            .unwrap();
        socket.write_all(b"R").unwrap();
        let mut release = [0];
        socket.read_exact(&mut release).unwrap();
        assert_eq!(release, *b"E");
    }

    #[test]
    fn native_peer_retention_image_refusal_and_exit_evidence() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("peer.sock");
        let listener = UnixListener::bind(&path)?;
        listener.set_nonblocking(true)?;
        let executable = std::env::current_exe()?;
        let mut child = Command::new(&executable)
            .args([
                "control_peer::tests::native_peer_child",
                "--test-threads=1",
                "--nocapture",
            ])
            .env("MCW_CONTROL_PEER_FIXTURE_SOCKET", &path)
            .spawn()?;
        let result = (|| -> Result<()> {
            let deadline = Instant::now() + Duration::from_secs(10);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        if child.try_wait()?.is_some() || Instant::now() >= deadline {
                            bail!("native peer fixture did not connect");
                        }
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => return Err(error.into()),
                }
            };
            stream.set_read_timeout(Some(Duration::from_secs(5)))?;
            let mut ready = [0];
            stream.read_exact(&mut ready)?;
            assert_eq!(ready, *b"R");
            let peer = PeerProcess::retain(&stream)?;
            assert_eq!(peer.pid(), child.id());
            peer.verify_image(&executable)?;
            assert!(peer.verify_image(Path::new("/bin/sh")).is_err());
            assert!(peer.wait_exit(Duration::from_millis(20)).is_err());
            stream.write_all(b"E")?;
            peer.wait_exit(Duration::from_secs(5))?;
            peer.wait_exit(Duration::ZERO)?;
            assert!(peer.verify_image(&executable).is_err());
            Ok(())
        })();
        // No signal even on failure: the disposable child's read timeout bounds
        // its lifetime, and dropping our connection releases it immediately.
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            if let Some(status) = child.try_wait()? {
                result?;
                assert!(status.success());
                return Ok(());
            }
            if Instant::now() >= deadline {
                bail!("native peer fixture exceeded its natural-exit deadline");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
