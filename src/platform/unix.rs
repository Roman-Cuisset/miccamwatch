use anyhow::{Context, Result, bail};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionLockState {
    Locked,
    Unlocked,
    Unknown,
}

/// Compatibility query. An unavailable service is never interpreted as unlock.
/// Call `query_session_lock_state` when displaying capability/metadata errors.
pub fn session_lock_state() -> SessionLockState {
    query_session_lock_state().unwrap_or(SessionLockState::Unknown)
}

#[cfg(target_os = "linux")]
pub fn query_session_lock_state() -> Result<SessionLockState> {
    use std::collections::HashMap;
    use zbus::{
        blocking::{Proxy, connection::Builder},
        zvariant::{OwnedObjectPath, OwnedValue},
    };

    let connection = Builder::system()
        .context("system D-Bus/logind is required to observe graphical session lock state")?
        .method_timeout(std::time::Duration::from_secs(3))
        .build()
        .context("cannot connect to system D-Bus for graphical session lock evidence")?;
    let manager = Proxy::new(
        &connection,
        "org.freedesktop.login1",
        "/org/freedesktop/login1",
        "org.freedesktop.login1.Manager",
    )?;
    let sessions: Vec<(String, u32, String, String, OwnedObjectPath)> = manager
        .call("ListSessions", &())
        .context("logind cannot enumerate the user's graphical sessions")?;
    let uid = unsafe { libc::geteuid() };
    let mut evidence = Vec::new();
    for (id, session_uid, _, _, path) in sessions {
        if session_uid != uid {
            continue;
        }
        let properties = Proxy::new(
            &connection,
            "org.freedesktop.login1",
            path,
            "org.freedesktop.DBus.Properties",
        )?;
        // One GetAll snapshot, not a mixture of individually cached properties.
        let values: HashMap<String, OwnedValue> = properties
            .call("GetAll", &("org.freedesktop.login1.Session",))
            .with_context(|| format!("cannot read logind session {id}"))?;
        let text = |key: &str| -> Result<&str> {
            let value = values
                .get(key)
                .with_context(|| format!("logind session {id} omitted {key}"))?;
            <&str>::try_from(value)
                .with_context(|| format!("logind session {id} has invalid {key}"))
        };
        let boolean = |key: &str| -> Result<bool> {
            let value = values
                .get(key)
                .with_context(|| format!("logind session {id} omitted {key}"))?;
            bool::try_from(value).with_context(|| format!("logind session {id} has invalid {key}"))
        };
        let graphical = matches!(text("Type")?, "x11" | "wayland" | "mir");
        if !graphical || !matches!(text("Class")?, "user" | "user-early") {
            continue;
        }
        if boolean("Remote")? {
            continue;
        }
        let usable = matches!(text("State")?, "active" | "online");
        evidence.push(SessionEvidence {
            graphical,
            remote: false,
            usable,
            locked: Some(boolean("LockedHint")?),
        });
    }
    Ok(combine_sessions(&evidence))
}

#[cfg(target_os = "macos")]
pub fn query_session_lock_state() -> Result<SessionLockState> {
    // There is no supported public API for distinguishing a locked macOS session.
    Ok(SessionLockState::Unknown)
}

#[cfg(target_os = "linux")]
struct SessionEvidence {
    graphical: bool,
    remote: bool,
    usable: bool,
    locked: Option<bool>,
}

#[cfg(target_os = "linux")]
fn combine_sessions(sessions: &[SessionEvidence]) -> SessionLockState {
    let mut state = None;
    for session in sessions
        .iter()
        .filter(|session| session.graphical && !session.remote)
    {
        let next = match (session.usable, session.locked) {
            (true, Some(true)) => SessionLockState::Locked,
            (true, Some(false)) => SessionLockState::Unlocked,
            _ => return SessionLockState::Unknown,
        };
        if state.is_some_and(|current| current != next) {
            return SessionLockState::Unknown;
        }
        state = Some(next);
    }
    state.unwrap_or(SessionLockState::Unknown)
}

/// Terminates only the observed process instance, through retained native
/// authority. Missing PID/identity, protected targets and denied authority fail.
#[cfg(target_os = "linux")]
pub fn terminate_process_by_pid(pid: u32, expected_instance: &str) -> Result<()> {
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

    if pid <= 1
        || pid > i32::MAX as u32
        || pid == std::process::id()
        || pid == unsafe { libc::getppid() } as u32
        || expected_instance.is_empty()
    {
        bail!("refusing to terminate a protected or unattributed process");
    }
    // Linux 5.3+ pidfd_open: never fall back to kill(pid) on an older kernel.
    let descriptor = unsafe { libc::syscall(libc::SYS_pidfd_open, pid as libc::pid_t, 0u32) };
    if descriptor < 0 {
        return Err(std::io::Error::last_os_error()).context(
            "cannot obtain a stable Linux process handle (pidfd_open requires Linux 5.3+)",
        );
    }
    let handle = unsafe { OwnedFd::from_raw_fd(descriptor as libc::c_int) };
    let signal = |value: libc::c_int| -> Result<()> {
        let result = unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                handle.as_raw_fd(),
                value,
                std::ptr::null::<libc::siginfo_t>(),
                0u32,
            )
        };
        if result < 0 {
            return Err(std::io::Error::last_os_error())
                .context("stable Linux process handle cannot accept the requested signal");
        }
        Ok(())
    };
    signal(0)?;
    let (instance, executable, uid) = super::linux::verified_process_identity(pid)?;
    if instance != expected_instance {
        bail!("process instance changed or its observed identity was unavailable");
    }
    validate_linux_target(&executable, uid)?;
    let metadata = std::fs::metadata(format!("/proc/{pid}/exe"))
        .context("cannot validate the target's executable through procfs")?;
    if !metadata.is_file() {
        bail!("target executable is not a regular file");
    }
    let (final_instance, final_executable, final_uid) =
        super::linux::verified_process_identity(pid)?;
    if final_instance != instance || final_executable != executable || final_uid != uid {
        bail!("process identity or executable changed before termination");
    }
    // If the process behind the pidfd exited while procfs was read, signal(0)
    // fails even when a new process already uses its old numeric PID. SIGKILL
    // itself always targets this same handle, so reuse after this check is safe.
    signal(0)?;
    signal(libc::SIGKILL)
}

#[cfg(target_os = "linux")]
fn validate_linux_target(executable: &str, uid: u32) -> Result<()> {
    if uid == 0 || uid != unsafe { libc::geteuid() } {
        bail!("refusing to terminate a system process or another user's process");
    }
    let path = std::path::Path::new(executable);
    if !path.is_absolute() || executable.ends_with(" (deleted)") {
        bail!("target executable identity is unavailable or no longer present");
    }
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .context("target executable name is unavailable")?;
    if matches!(
        name,
        "systemd"
            | "init"
            | "dbus-daemon"
            | "dbus-broker"
            | "dbus-broker-launch"
            | "sshd"
            | "login"
            | "Xorg"
            | "Xwayland"
            | "gnome-shell"
            | "kwin_wayland"
            | "kwin_x11"
            | "plasmashell"
            | "sway"
            | "weston"
    ) {
        bail!("refusing to terminate a protected session/system executable");
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn terminate_process_by_pid(pid: u32, expected_instance: &str) -> Result<()> {
    if pid <= 1
        || pid > i32::MAX as u32
        || pid == std::process::id()
        || expected_instance.is_empty()
    {
        bail!("refusing to terminate a protected or unattributed process");
    }
    native_effect("terminate", &[&pid.to_string(), expected_instance])?;
    Ok(())
}

/// Plays a desktop sound only when explicitly requested. Playback acceptance is
/// not a guarantee that physical speakers are connected or unmuted.
pub fn play_chime() -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        use std::process::{Command, Stdio};
        use std::time::{Duration, Instant};
        let mut child = Command::new("canberra-gtk-play")
            .args(["--id=message-new-instant", "--application-name=miccamwatch"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .context("desktop sound requires canberra-gtk-play and a graphical sound session")?;
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    if !status.success() {
                        bail!("desktop sound playback failed ({status})");
                    }
                    return Ok(());
                }
                Ok(None) => {}
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(error).context("cannot observe desktop sound playback");
                }
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                bail!("desktop sound playback timed out");
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    #[cfg(target_os = "macos")]
    {
        native_effect("sound", &[])?;
        Ok(())
    }
}

/// Explicit local syslog/journal delivery, never a network request. Success is
/// native service acceptance, not a promise about journal retention policy.
pub fn write_system_log(message: &str) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::net::UnixDatagram;
        if message.contains('\0') {
            bail!("system log messages cannot contain NUL bytes");
        }
        let socket = UnixDatagram::unbound().context("cannot create a local syslog socket")?;
        socket.set_write_timeout(Some(std::time::Duration::from_secs(3)))?;
        socket
            .connect("/dev/log")
            .context("local journal/syslog delivery requires an available /dev/log service")?;
        let payload = format!(
            "<14>{} miccamwatch[{}]: {message}",
            chrono::Local::now().format("%b %e %T"),
            std::process::id()
        );
        let sent = socket
            .send(payload.as_bytes())
            .context("local journal/syslog rejected delivery")?;
        if sent != payload.len() {
            bail!("local journal/syslog accepted an incomplete event");
        }
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        native_effect("log", &[message])?;
        Ok(())
    }
}

#[cfg(target_os = "macos")]
#[derive(serde::Deserialize)]
struct EffectOutput {
    effect: Option<EffectReply>,
}

#[cfg(target_os = "macos")]
#[derive(serde::Deserialize)]
struct EffectReply {
    ok: bool,
    error: Option<String>,
    state: Option<String>,
}

#[cfg(target_os = "macos")]
pub(crate) fn native_effect(mode: &str, args: &[&str]) -> Result<Option<String>> {
    let timeout = if matches!(mode, "notification-identity" | "notify") {
        60
    } else {
        35
    };
    let output: EffectOutput =
        super::macos::native_request(mode, args, std::time::Duration::from_secs(timeout))?;
    let reply = output
        .effect
        .context("native helper omitted its effect result")?;
    if !reply.ok {
        bail!(
            "{}",
            reply
                .error
                .as_deref()
                .unwrap_or("native helper rejected the requested effect without a diagnostic")
        );
    }
    if let Some(error) = reply.error {
        bail!("native helper returned a contradictory success/error result: {error}");
    }
    Ok(reply.state)
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    fn session(
        graphical: bool,
        remote: bool,
        usable: bool,
        locked: Option<bool>,
    ) -> SessionEvidence {
        SessionEvidence {
            graphical,
            remote,
            usable,
            locked,
        }
    }

    #[test]
    fn ssh_session_never_supplies_graphical_unlock_evidence() {
        assert_eq!(
            combine_sessions(&[session(false, true, true, Some(false))]),
            SessionLockState::Unknown
        );
        assert_eq!(
            combine_sessions(&[
                session(false, true, true, Some(false)),
                session(true, false, true, Some(true)),
            ]),
            SessionLockState::Locked
        );
        assert_eq!(
            combine_sessions(&[session(true, true, true, Some(false))]),
            SessionLockState::Unknown
        );
    }

    #[test]
    fn conflicting_or_incomplete_graphical_evidence_is_unknown() {
        assert_eq!(combine_sessions(&[]), SessionLockState::Unknown);
        assert_eq!(
            combine_sessions(&[
                session(true, false, true, Some(false)),
                session(true, false, true, Some(true)),
            ]),
            SessionLockState::Unknown
        );
        assert_eq!(
            combine_sessions(&[session(true, false, true, None)]),
            SessionLockState::Unknown
        );
        assert_eq!(
            combine_sessions(&[session(true, false, false, Some(false))]),
            SessionLockState::Unknown
        );
        assert_eq!(
            combine_sessions(&[session(true, false, true, Some(false))]),
            SessionLockState::Unlocked
        );
    }

    #[test]
    fn unknown_and_protected_identifiers_never_reach_process_signaling() {
        assert!(terminate_process_by_pid(0, "").is_err());
        assert!(terminate_process_by_pid(1, "linux:1:0:0").is_err());
        assert!(terminate_process_by_pid(std::process::id(), "not-an-instance").is_err());
        assert!(terminate_process_by_pid(u32::MAX, "not-an-instance").is_err());
    }

    #[test]
    fn stale_process_instance_does_not_terminate_a_live_child() {
        let mut child = std::process::Command::new("/bin/sleep")
            .arg("30")
            .spawn()
            .unwrap();
        let result = terminate_process_by_pid(child.id(), "linux:stale:identity");
        let still_running = child.try_wait().unwrap().is_none();
        let _ = child.kill();
        let _ = child.wait();
        assert!(result.is_err());
        assert!(
            still_running,
            "stale identity must leave the real child alive"
        );
    }

    #[test]
    fn retained_process_authority_terminates_only_the_observed_child() -> Result<()> {
        use std::os::unix::process::ExitStatusExt;
        use std::time::{Duration, Instant};

        if unsafe { libc::geteuid() } == 0 {
            eprintln!(
                "Root processes are intentionally protected; native termination requires an ordinary user"
            );
            return Ok(());
        }
        let mut child = std::process::Command::new("/bin/sleep").arg("30").spawn()?;
        let observed = (|| -> Result<std::process::ExitStatus> {
            let (instance, _, _) = crate::platform::linux::verified_process_identity(child.id())?;
            terminate_process_by_pid(child.id(), &instance)?;
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                if let Some(status) = child.try_wait()? {
                    return Ok(status);
                }
                if Instant::now() >= deadline {
                    bail!("the observed child did not terminate after native signaling");
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        })();
        let _ = child.kill();
        let _ = child.wait();
        assert_eq!(observed?.signal(), Some(libc::SIGKILL));
        Ok(())
    }
}
