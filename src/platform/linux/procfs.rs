use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use std::{fs, os::unix::fs::MetadataExt, path::Path};

pub(super) struct BootTime {
    seconds: u64,
    ticks_per_second: u64,
}

impl BootTime {
    pub(super) fn read() -> Result<Self> {
        let stat = fs::read_to_string("/proc/stat").context("cannot read /proc/stat")?;
        let seconds = parse_boot_time(&stat)?;
        let ticks = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
        if ticks <= 0 {
            bail!("cannot determine Linux clock ticks per second");
        }
        Ok(Self {
            seconds,
            ticks_per_second: ticks as u64,
        })
    }

    fn instant(&self, ticks: u64) -> Result<DateTime<Utc>> {
        let seconds = self.seconds + ticks / self.ticks_per_second;
        let nanos =
            ((ticks % self.ticks_per_second) * 1_000_000_000 / self.ticks_per_second) as u32;
        DateTime::from_timestamp(i64::try_from(seconds)?, nanos)
            .context("process start timestamp is outside the supported range")
    }
}

pub(super) struct ProcessIdentity {
    pub(super) pid: u32,
    pub(super) instance_id: String,
    pub(super) executable: String,
    pub(super) executable_file: fs::Metadata,
    pub(super) name: String,
    pub(super) parent_pid: Option<u32>,
    pub(super) uid: u32,
    pub(super) created_at: DateTime<Utc>,
}

impl ProcessIdentity {
    pub(super) fn verify(pid: u32, boot: &BootTime) -> Result<Self> {
        let dir = Path::new("/proc").join(pid.to_string());
        let stat_path = dir.join("stat");
        let first = fs::read_to_string(&stat_path)
            .with_context(|| format!("cannot read {}", stat_path.display()))?;
        let (parent, start_ticks) = parse_process_stat(&first, pid)?;
        let executable_link = dir.join("exe");
        let executable_path = fs::read_link(&executable_link)
            .with_context(|| format!("cannot resolve /proc/{pid}/exe"))?;
        if !executable_path.is_absolute() {
            bail!("PID {pid} executable is not an absolute filesystem path");
        }
        let executable = executable_path
            .to_str()
            .context("process executable path is not valid UTF-8")?
            .to_owned();
        let process_executable = fs::metadata(&executable_link)?;
        let path_executable = fs::metadata(&executable_path)
            .context("process executable is deleted or no longer available at its path")?;
        if !same_executable(&process_executable, &path_executable) {
            bail!("PID {pid} executable path no longer identifies its running executable");
        }
        let uid = fs::metadata(&dir)?.uid();
        // Re-check process and executable identity: exec does not change starttime,
        // and replacing a pathname must not authenticate a different running inode.
        let second = fs::read_to_string(&stat_path)?;
        if parse_process_stat(&second, pid)?.1 != start_ticks
            || fs::read_link(&executable_link)? != executable_path
            || fs::metadata(&dir)?.uid() != uid
            || !same_executable(&process_executable, &fs::metadata(&executable_link)?)
            || !same_executable(&process_executable, &fs::metadata(&executable_path)?)
        {
            bail!("PID {pid} changed process or executable identity during observation");
        }
        let name = Path::new(&executable)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| executable.clone());
        Ok(Self {
            pid,
            instance_id: format!("linux:{pid}:{}:{start_ticks}", boot.seconds),
            executable,
            executable_file: process_executable,
            name,
            parent_pid: (parent != 0).then_some(parent),
            uid,
            created_at: boot.instant(start_ticks)?,
        })
    }
}

// SO_PEERCRED can name the non-dumpable systemd socket activator, not
// PipeWire itself. Session ownership pins its UID and process generation;
// capture attribution still requires the full executable checks above.
pub(super) fn verify_peer_instance(pid: u32, uid: u32, boot: &BootTime) -> Result<String> {
    let directory = Path::new("/proc").join(pid.to_string());
    let stat_path = directory.join("stat");
    let first = fs::read_to_string(&stat_path)
        .with_context(|| format!("cannot read {}", stat_path.display()))?;
    let start_ticks = parse_process_stat(&first, pid)?.1;
    if fs::metadata(&directory)?.uid() != uid {
        bail!("PipeWire socket peer UID changed during identity verification");
    }
    let second = fs::read_to_string(&stat_path)?;
    if parse_process_stat(&second, pid)?.1 != start_ticks || fs::metadata(&directory)?.uid() != uid
    {
        bail!("PipeWire socket peer process changed during identity verification");
    }
    Ok(format!("linux:{pid}:{}:{start_ticks}", boot.seconds))
}

pub(super) fn same_executable(left: &fs::Metadata, right: &fs::Metadata) -> bool {
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

fn parse_boot_time(contents: &str) -> Result<u64> {
    let value = contents
        .lines()
        .find_map(|line| line.strip_prefix("btime "))
        .context("/proc/stat does not contain btime")?;
    value.trim().parse().context("invalid /proc/stat btime")
}

fn parse_process_stat(contents: &str, expected_pid: u32) -> Result<(u32, u64)> {
    let open = contents.find('(').context("missing /proc/pid/stat comm")?;
    let close = contents
        .rfind(") ")
        .context("unterminated /proc/pid/stat comm")?;
    if close <= open || contents[..open].trim().parse::<u32>()? != expected_pid {
        bail!("/proc/pid/stat PID mismatch");
    }
    let fields: Vec<_> = contents[close + 2..].split_whitespace().collect();
    let parent = fields
        .get(1)
        .context("missing /proc/pid/stat ppid")?
        .parse()?;
    let start_ticks = fields
        .get(19)
        .context("missing /proc/pid/stat starttime")?
        .parse()?;
    Ok((parent, start_ticks))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_comm_with_spaces_and_closing_parenthesis_without_shifting_start_time() {
        let mut fields = vec!["S".to_owned(), "17".to_owned()];
        fields.extend((5..=21).map(|_| "0".to_owned()));
        fields.push("4301".to_owned());
        let stat = format!("42 (a name ) with spaces) {}", fields.join(" "));
        assert_eq!(parse_process_stat(&stat, 42).unwrap(), (17, 4301));
        assert!(parse_process_stat(&stat, 41).is_err());
        assert!(parse_process_stat("42 (broken)", 42).is_err());
    }

    #[test]
    fn boot_time_and_ticks_calculate_process_start_in_unix_time() {
        assert_eq!(
            parse_boot_time("cpu 1 2 3\nbtime 1710000000\n").unwrap(),
            1710000000
        );
        assert!(parse_boot_time("cpu 1 2\n").is_err());
        let boot = BootTime {
            seconds: 1710000000,
            ticks_per_second: 100,
        };
        assert_eq!(boot.instant(225).unwrap().timestamp(), 1710000002);
        assert_eq!(boot.instant(225).unwrap().timestamp_subsec_millis(), 250);
    }
    #[test]
    fn current_process_identity_uses_proc_executable_and_stable_starttime() {
        let boot = BootTime::read().unwrap();
        let pid = std::process::id();
        let first = ProcessIdentity::verify(pid, &boot).unwrap();
        let second = ProcessIdentity::verify(pid, &boot).unwrap();
        assert_eq!(first.instance_id, second.instance_id);
        assert_eq!(first.pid, pid);
        assert_eq!(
            first.executable,
            std::env::current_exe().unwrap().to_string_lossy()
        );
        assert!(first.created_at <= Utc::now());
        assert!(ProcessIdentity::verify(u32::MAX, &boot).is_err());
    }

    #[test]
    fn socket_peer_generation_is_verified_without_reading_non_dumpable_executable() {
        use std::{
            io::Read,
            os::fd::{FromRawFd, OwnedFd},
        };

        struct Child(libc::pid_t);
        impl Drop for Child {
            fn drop(&mut self) {
                unsafe {
                    libc::kill(self.0, libc::SIGTERM);
                    while libc::waitpid(self.0, std::ptr::null_mut(), 0) < 0
                        && std::io::Error::last_os_error().raw_os_error() == Some(libc::EINTR)
                    {
                    }
                }
            }
        }

        let boot = BootTime::read().unwrap();
        let mut descriptors = [0; 2];
        assert_eq!(
            unsafe { libc::pipe2(descriptors.as_mut_ptr(), libc::O_CLOEXEC) },
            0
        );
        let reader = unsafe { OwnedFd::from_raw_fd(descriptors[0]) };
        let writer = unsafe { OwnedFd::from_raw_fd(descriptors[1]) };
        let pid = unsafe { libc::fork() };
        assert!(pid >= 0);
        if pid == 0 {
            // After a multithreaded fork, the child uses only libc syscalls.
            unsafe {
                libc::close(descriptors[0]);
                if libc::prctl(libc::PR_SET_DUMPABLE, 0) != 0
                    || libc::write(descriptors[1], [1_u8].as_ptr().cast(), 1) != 1
                {
                    libc::_exit(1);
                }
                libc::close(descriptors[1]);
                loop {
                    libc::pause();
                }
            }
        }
        let _child = Child(pid);
        drop(writer);
        let mut ready = [0];
        fs::File::from(reader).read_exact(&mut ready).unwrap();
        assert_eq!(ready, [1]);
        let pid = pid as u32;
        let uid = unsafe { libc::geteuid() };
        if uid != 0 {
            assert!(ProcessIdentity::verify(pid, &boot).is_err());
        }
        let stat = fs::read_to_string(format!("/proc/{pid}/stat")).unwrap();
        let ticks = parse_process_stat(&stat, pid).unwrap().1;
        assert_eq!(
            verify_peer_instance(pid, uid, &boot).unwrap(),
            format!("linux:{pid}:{}:{ticks}", boot.seconds)
        );
        assert!(verify_peer_instance(pid, uid.wrapping_add(1), &boot).is_err());
    }
}
