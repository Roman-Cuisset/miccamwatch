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
        let executable = fs::read_link(dir.join("exe"))
            .with_context(|| format!("cannot resolve /proc/{pid}/exe"))?
            .to_string_lossy()
            .into_owned();
        let uid = fs::metadata(&dir)?.uid();
        // Re-check after resolving exe: a PID reused during the read cannot inherit the
        // previous process's identity or executable.
        let second = fs::read_to_string(&stat_path)?;
        if parse_process_stat(&second, pid)?.1 != start_ticks {
            bail!("PID {pid} changed process instance during observation");
        }
        let name = Path::new(&executable)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| executable.clone());
        Ok(Self {
            pid,
            instance_id: format!("linux:{pid}:{}:{start_ticks}", boot.seconds),
            executable,
            name,
            parent_pid: (parent != 0).then_some(parent),
            uid,
            created_at: boot.instant(start_ticks)?,
        })
    }
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
}
