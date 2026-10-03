use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::{
    fs::{File, OpenOptions},
    io::Read,
    os::windows::{
        fs::{MetadataExt, OpenOptionsExt},
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::Command,
};
use winreg::{RegKey, enums::HKEY_CURRENT_USER};

const TASK_NAME: &str = "MicCamWatch Tray";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "MicCamWatch";

/// The file identity is permanent: never truncate, rename or delete it. All CLI,
/// tray and updater mutations share this lock, including the updater's refresh.
pub(crate) struct InstallationLock {
    _file: File,
    directory: PathBuf,
}

pub(crate) fn lock_installation(current: &Path) -> Result<InstallationLock> {
    let directory = current
        .parent()
        .context("executable has no installation directory")?;
    let path = directory.join(".mcw-install.lock");
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        // Allow other participants to open/lock it, but never deletion while held.
        .share_mode(3)
        .custom_flags(0x0020_0000) // FILE_FLAG_OPEN_REPARSE_POINT
        .open(&path)
        .with_context(|| format!("failed to open installation lock {}", path.display()))?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.file_attributes() & 0x400 != 0 {
        bail!(
            "installation lock {} is not an owned regular file; unchanged",
            path.display()
        );
    }
    file.lock()
        .with_context(|| format!("failed to acquire installation lock {}", path.display()))?;
    Ok(InstallationLock {
        _file: file,
        directory: directory.to_owned(),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AutostartState {
    Enabled,
    Disabled,
}

pub fn state() -> Result<AutostartState> {
    let current = std::env::current_exe()?;
    let task = registered_task()?;
    if task
        .as_ref()
        .is_some_and(|task| task.enabled && task.owned_by(&current))
    {
        return Ok(AutostartState::Enabled);
    }
    if run_registration()?
        .is_some_and(|(command, enabled)| enabled && owned_command(&command, &current))
    {
        return Ok(AutostartState::Enabled);
    }
    Ok(AutostartState::Disabled)
}

pub fn enable() -> Result<()> {
    enable_for_version(env!("CARGO_PKG_VERSION"))
}

fn enable_for_version(version: &str) -> Result<()> {
    let current = std::env::current_exe()?;
    let _lock = lock_installation(&current)?;
    let executable = tray_executable(version)?;
    let desired = launch_command(&executable);
    let existing = registered_task()?;
    let run = run_registration()?;
    // A disabled StartupApproved item must not be sidestepped by creating a task.
    if !run_approved()? {
        bail!(
            "Windows Startup Apps has disabled MicCamWatch; enable it there before opting in; autostart unchanged"
        );
    }
    if let Some(task) = &existing {
        if !task.owned_by(&current) {
            bail!("an unmanaged task named {TASK_NAME} exists; it was not modified");
        }
        if !task.enabled {
            bail!(
                "the owned MicCamWatch task is disabled; enable it in Task Scheduler; autostart unchanged"
            );
        }
        if needs_repoint(&task.command, &desired, true) {
            change_task(&desired)?;
        }
        return Ok(());
    }
    if let Some((command, enabled)) = run {
        if !owned_command(&command, &current) {
            bail!("an unmanaged {RUN_VALUE} Run value exists; it was not modified");
        }
        if !enabled {
            bail!("Windows has disabled MicCamWatch autostart; registration unchanged");
        }
        if needs_repoint(&command, &desired, enabled) {
            set_run_command(&desired)?;
        }
        return Ok(());
    }
    // Explicit opt-in is the only path that creates a registration. No /F:
    // never overwrite a task that appeared after the ownership query.
    let output = schtasks(&[
        "/Create", "/TN", TASK_NAME, "/TR", &desired, "/SC", "ONLOGON", "/RL", "LIMITED",
    ]);
    if output.as_ref().is_ok_and(|output| output.status.success()) {
        return Ok(());
    }
    if registered_task()?.is_some() {
        bail!("failed to create the MicCamWatch autostart task; no Run fallback was added");
    }
    set_run_command(&desired)?;
    Ok(())
}

/// Repoint only an existing, enabled, exactly owned registration. Updating bytes
/// at the same path does not require another persistence operation.
pub(crate) fn refresh_if_enabled(
    version: &str,
    current: &Path,
    lock: &InstallationLock,
) -> Result<()> {
    if current.parent() != Some(lock.directory.as_path()) {
        bail!("autostart refresh does not own this installation lock");
    }
    let task = registered_task()?;
    let run = run_registration()?;
    let enabled_owned_task = task
        .as_ref()
        .is_some_and(|task| task.enabled && task.owned_by(current));
    let enabled_owned_run = run
        .as_ref()
        .is_some_and(|(command, enabled)| *enabled && owned_command(command, current));
    if !enabled_owned_task && !enabled_owned_run {
        return Ok(());
    }
    let tray = current.with_file_name("mcw-tray.exe");
    if !tray_matches_version(&tray, version)? {
        bail!("installed tray does not match release {version}; autostart unchanged");
    }
    let desired = launch_command(&tray);
    refresh_commands(
        task.as_ref(),
        run.as_ref(),
        current,
        &desired,
        change_task,
        set_run_command,
    )
}

fn refresh_commands(
    task: Option<&TaskRegistration>,
    run: Option<&(String, bool)>,
    current: &Path,
    desired: &str,
    mut repoint_task: impl FnMut(&str) -> Result<()>,
    mut repoint_run: impl FnMut(&str) -> Result<()>,
) -> Result<()> {
    if let Some(task) = task
        && task.owned_by(current)
        && needs_repoint(&task.command, desired, task.enabled)
    {
        repoint_task(desired)?;
    }
    if let Some((command, enabled)) = run
        && owned_command(command, current)
        && needs_repoint(command, desired, *enabled)
    {
        repoint_run(desired)?;
    }
    Ok(())
}

pub fn disable() -> Result<()> {
    let current = std::env::current_exe()?;
    let _lock = lock_installation(&current)?;
    if registered_task()?
        .as_ref()
        .is_some_and(|task| task.enabled && task.owned_by(&current))
    {
        checked_schtasks(&["/Delete", "/TN", TASK_NAME, "/F"])?;
    }
    if run_registration()?
        .is_some_and(|(command, enabled)| enabled && owned_command(&command, &current))
    {
        remove_run_value()?;
    }
    Ok(())
}

fn launch_command(executable: &Path) -> String {
    let arguments = if executable
        .file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("mcw-tray.exe"))
    {
        ""
    } else {
        " tray run"
    };
    format!("\"{}\"{arguments}", executable.display())
}

fn owned_command(command: &str, current: &Path) -> bool {
    let cli = if current
        .file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("mcw-tray.exe"))
    {
        current.with_file_name("mcw.exe")
    } else {
        current.to_owned()
    };
    command.eq_ignore_ascii_case(&launch_command(&cli))
        || command.eq_ignore_ascii_case(&launch_command(&current.with_file_name("mcw-tray.exe")))
}

fn needs_repoint(registered: &str, desired: &str, enabled: bool) -> bool {
    enabled && !registered.eq_ignore_ascii_case(desired)
}

struct TaskRegistration {
    command: String,
    enabled: bool,
    simple_logon: bool,
}
impl TaskRegistration {
    fn owned_by(&self, current: &Path) -> bool {
        self.simple_logon && owned_command(&self.command, current)
    }
}

fn registered_task() -> Result<Option<TaskRegistration>> {
    let output = schtasks(&["/Query", "/TN", TASK_NAME, "/XML"])?;
    if !output.status.success() {
        return Ok(None);
    }
    parse_task(&decode_task_xml(&output.stdout)?, &current_user_sid()?).map(Some)
}

fn decode_task_xml(bytes: &[u8]) -> Result<String> {
    if bytes.starts_with(&[0xff, 0xfe]) || (bytes.len() > 1 && bytes[1] == 0) {
        let bytes = bytes.strip_prefix(&[0xff, 0xfe]).unwrap_or(bytes);
        if !bytes.len().is_multiple_of(2) {
            bail!("Task Scheduler returned truncated UTF-16 XML");
        }
        return String::from_utf16(
            &bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|part| u16::from_le_bytes(*part))
                .collect::<Vec<_>>(),
        )
        .context("Task Scheduler returned invalid UTF-16 XML");
    }
    String::from_utf8(bytes.to_vec()).context("Task Scheduler returned invalid XML encoding")
}

fn parse_task(xml: &str, caller_sid: &str) -> Result<TaskRegistration> {
    use windows::{
        Data::Xml::Dom::XmlDocument,
        Win32::System::Com::{
            COINIT_MULTITHREADED, CoIncrementMTAUsage, CoInitializeEx, CoUninitialize,
        },
        core::HSTRING,
    };
    // windows-core caches agile activation factories for the whole process.
    // A short-lived COM guard alone leaves that cache dangling after the last
    // MTA caller exits. Keep its apartment alive until process termination.
    static MTA: std::sync::LazyLock<Result<(), String>> = std::sync::LazyLock::new(|| {
        unsafe { CoIncrementMTAUsage() }
            .map(|_| ())
            .map_err(|error| format!("{error}"))
    });
    if let Err(error) = &*MTA {
        bail!("cannot initialize task XML runtime: {error}");
    }
    let initialized = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    struct ComGuard(bool);
    impl Drop for ComGuard {
        fn drop(&mut self) {
            if self.0 {
                unsafe {
                    CoUninitialize();
                }
            }
        }
    }
    let _guard = ComGuard(initialized.is_ok());
    let document = XmlDocument::new()?;
    document.LoadXml(&HSTRING::from(xml))?;
    let text = |xpath: &str| -> Result<String> {
        let nodes = document.SelectNodes(&HSTRING::from(xpath))?;
        if nodes.Length()? == 0 {
            return Ok(String::new());
        }
        if nodes.Length()? != 1 {
            bail!("MicCamWatch task has ambiguous XML fields; registration unchanged");
        }
        Ok(nodes.Item(0)?.InnerText()?.to_string())
    };
    let actions = "/*[local-name()='Task']/*[local-name()='Actions']/*";
    let command = text(
        "/*[local-name()='Task']/*[local-name()='Actions']/*[local-name()='Exec']/*[local-name()='Command']",
    )?;
    let arguments = text(
        "/*[local-name()='Task']/*[local-name()='Actions']/*[local-name()='Exec']/*[local-name()='Arguments']",
    )?;
    let enabled = text(
        "/*[local-name()='Task']/*[local-name()='Settings']/*[local-name()='Enabled']",
    )? != "false"
        && text(
            "/*[local-name()='Task']/*[local-name()='Triggers']/*[local-name()='LogonTrigger']/*[local-name()='Enabled']",
        )? != "false";
    let logon_count = document
        .SelectNodes(&HSTRING::from(
            "/*[local-name()='Task']/*[local-name()='Triggers']/*[local-name()='LogonTrigger']",
        ))?
        .Length()?;
    let run_level = text(
        "/*[local-name()='Task']/*[local-name()='Principals']/*[local-name()='Principal']/*[local-name()='RunLevel']",
    )?;
    let principal = text(
        "/*[local-name()='Task']/*[local-name()='Principals']/*[local-name()='Principal']/*[local-name()='UserId']",
    )?;
    let logon_user = text(
        "/*[local-name()='Task']/*[local-name()='Triggers']/*[local-name()='LogonTrigger']/*[local-name()='UserId']",
    )?;
    let logon_type = text(
        "/*[local-name()='Task']/*[local-name()='Principals']/*[local-name()='Principal']/*[local-name()='LogonType']",
    )?;
    let command = command.trim_matches('\"');
    Ok(TaskRegistration {
        command: format!(
            "\"{command}\"{}",
            if arguments.trim().is_empty() {
                String::new()
            } else {
                format!(" {}", arguments.trim())
            }
        ),
        enabled,
        simple_logon: document.SelectNodes(&HSTRING::from(actions))?.Length()? == 1
            && logon_count == 1
            && document
                .SelectNodes(&HSTRING::from(
                    "/*[local-name()='Task']/*[local-name()='Triggers']/*",
                ))?
                .Length()?
                == 1
            && run_level != "HighestAvailable"
            && principal == caller_sid
            && (logon_user.is_empty() || logon_user == caller_sid)
            && (logon_type.is_empty() || logon_type == "InteractiveToken"),
    })
}

fn current_user_sid() -> Result<String> {
    use windows::Win32::{
        Foundation::{CloseHandle, HANDLE},
        Security::{
            GetSidIdentifierAuthority, GetSidSubAuthority, GetSidSubAuthorityCount,
            GetTokenInformation, TOKEN_QUERY, TOKEN_USER, TokenUser,
        },
        System::Threading::{GetCurrentProcess, OpenProcessToken},
    };
    struct Token(HANDLE);
    impl Drop for Token {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
    let mut handle = HANDLE::default();
    unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut handle) }?;
    let token = Token(handle);
    let mut size = 0;
    let _ = unsafe { GetTokenInformation(token.0, TokenUser, None, 0, &mut size) };
    if size == 0 {
        bail!("cannot inspect current user SID; autostart unchanged");
    }
    // Pointer-aligned storage, because TOKEN_USER contains a SID pointer.
    let mut storage = vec![0usize; (size as usize).div_ceil(std::mem::size_of::<usize>())];
    unsafe {
        GetTokenInformation(
            token.0,
            TokenUser,
            Some(storage.as_mut_ptr().cast()),
            size,
            &mut size,
        )
    }?;
    let user = unsafe { &*storage.as_ptr().cast::<TOKEN_USER>() };
    let sid = user.User.Sid;
    let authority = unsafe { (*GetSidIdentifierAuthority(sid)).Value };
    let authority = authority
        .iter()
        .fold(0u64, |value, byte| (value << 8) | u64::from(*byte));
    let mut formatted = format!("S-1-{authority}");
    let count = unsafe { *GetSidSubAuthorityCount(sid) };
    for index in 0..u32::from(count) {
        use std::fmt::Write;
        write!(&mut formatted, "-{}", unsafe {
            *GetSidSubAuthority(sid, index)
        })?;
    }
    Ok(formatted)
}

fn change_task(command: &str) -> Result<()> {
    checked_schtasks(&["/Change", "/TN", TASK_NAME, "/TR", command])
}

fn checked_schtasks(arguments: &[&str]) -> Result<()> {
    let output = schtasks(arguments)?;
    if !output.status.success() {
        bail!(
            "Windows Task Scheduler operation failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

fn run_registration() -> Result<Option<(String, bool)>> {
    let root = RegKey::predef(HKEY_CURRENT_USER);
    let command = match root
        .open_subkey(RUN_KEY)
        .and_then(|key| key.get_value::<String, _>(RUN_VALUE))
    {
        Ok(command) => command,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).context("failed to inspect per-user autostart"),
    };
    Ok(Some((command, run_approved()?)))
}

fn run_approved() -> Result<bool> {
    let root = RegKey::predef(HKEY_CURRENT_USER);
    let approval = match root
        .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run")
        .and_then(|key| key.get_raw_value(RUN_VALUE))
    {
        Ok(value) => Some(value.bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(error).context("failed to inspect user-disabled autostart intent");
        }
    };
    // Missing StartupApproved entries are enabled. Unknown states are preserved,
    // never treated as permission to re-enable the user's disabled startup item.
    Ok(approval
        .as_ref()
        .is_none_or(|bytes| matches!(bytes.first(), Some(2 | 6))))
}

fn set_run_command(command: &str) -> Result<()> {
    let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(RUN_KEY)?;
    key.set_value(RUN_VALUE, &command)
        .context("failed to configure per-user autostart")
}

fn remove_run_value() -> Result<()> {
    let root = RegKey::predef(HKEY_CURRENT_USER);
    match root.open_subkey_with_flags(RUN_KEY, winreg::enums::KEY_SET_VALUE) {
        Ok(key) => match key.delete_value(RUN_VALUE) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).context("failed to disable per-user autostart"),
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).context("failed to open the per-user Run registry key"),
    }
    Ok(())
}

fn tray_executable(version: &str) -> Result<std::path::PathBuf> {
    let current = std::env::current_exe().context("failed to locate mcw executable")?;
    let tray = current.with_file_name("mcw-tray.exe");
    if tray.exists() {
        if !tray_matches_version(&tray, version)? {
            bail!(
                "the companion {} does not match mcw {}; install matching binaries before enabling autostart",
                tray.display(),
                version
            );
        }
        return Ok(tray);
    }
    Ok(current)
}

/// The tray menu embeds this exact version label, including in older releases
/// that predate an explicit Windows file-version resource. Inspect it without
/// executing a possibly stale tray binary.
pub(crate) fn tray_matches_current_version(path: &Path) -> Result<bool> {
    tray_contains_marker(
        path,
        concat!("miccamwatch v", env!("CARGO_PKG_VERSION")).as_bytes(),
    )
}

pub(crate) fn tray_matches_version(path: &Path, version: &str) -> Result<bool> {
    tray_contains_marker(path, format!("miccamwatch v{version}").as_bytes())
}

fn tray_contains_marker(path: &Path, marker: &[u8]) -> Result<bool> {
    let mut file = std::fs::File::open(path)
        .with_context(|| format!("failed to inspect companion tray {}", path.display()))?;
    let mut bytes = [0u8; 8192];
    let mut retained = 0;
    loop {
        let count = file.read(&mut bytes[retained..])?;
        if count == 0 {
            return Ok(bytes[..retained].ends_with(marker));
        }
        let end = retained + count;
        if bytes[..end].windows(marker.len() + 1).any(|part| {
            part.starts_with(marker)
                && !matches!(part[marker.len()], b'0'..=b'9' | b'.' | b'-' | b'+')
        }) {
            return Ok(true);
        }
        retained = marker.len().min(end);
        bytes.copy_within(end - retained..end, 0);
    }
}

fn schtasks(arguments: &[&str]) -> Result<std::process::Output> {
    Command::new(crate::windows_tools::system_executable("schtasks.exe")?)
        .args(arguments)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .context("failed to execute Windows Task Scheduler")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unchanged_registration_has_no_refresh_operation() {
        let current = Path::new(r"C:\Users\alice\.cargo\bin\mcw.exe");
        let desired = launch_command(&current.with_file_name("mcw-tray.exe"));
        assert!(owned_command(&desired, current));
        assert!(!needs_repoint(&desired, &desired, true));
        assert!(!needs_repoint(&desired.to_uppercase(), &desired, true));
        assert!(needs_repoint(&launch_command(current), &desired, true));
        assert!(!needs_repoint(&launch_command(current), &desired, false));
        assert!(!owned_command(
            r#""C:\Users\bob\.cargo\bin\mcw-tray.exe""#,
            current
        ));
        assert!(!owned_command(&format!("{desired} --unexpected"), current));
        assert!(owned_command(
            &launch_command(current),
            &current.with_file_name("mcw-tray.exe")
        ));
    }

    #[test]
    fn refresh_never_registers_or_reenables_unchanged_disabled_unmanaged_items() -> Result<()> {
        use std::cell::Cell;
        let current = Path::new(r"C:\Users\alice\.cargo\bin\mcw.exe");
        let desired = launch_command(&current.with_file_name("mcw-tray.exe"));
        let writes = Cell::new(0);
        for (registered, enabled, owned) in [
            (desired.clone(), true, true),
            (launch_command(current), false, true),
            (r#""C:\unmanaged\mcw.exe" tray run"#.into(), true, false),
        ] {
            let task = TaskRegistration {
                command: registered.clone(),
                enabled,
                simple_logon: owned,
            };
            let run = (registered, enabled);
            refresh_commands(
                Some(&task),
                Some(&run),
                current,
                &desired,
                |_| {
                    writes.set(writes.get() + 1);
                    Ok(())
                },
                |_| {
                    writes.set(writes.get() + 1);
                    Ok(())
                },
            )?;
        }
        refresh_commands(
            None,
            None,
            current,
            &desired,
            |_| {
                writes.set(writes.get() + 1);
                Ok(())
            },
            |_| {
                writes.set(writes.get() + 1);
                Ok(())
            },
        )?;
        assert_eq!(writes.get(), 0);
        let task = TaskRegistration {
            command: launch_command(current),
            enabled: true,
            simple_logon: true,
        };
        let run = (launch_command(current), true);
        refresh_commands(
            Some(&task),
            Some(&run),
            current,
            &desired,
            |command| {
                assert_eq!(command, desired);
                writes.set(writes.get() + 1);
                Ok(())
            },
            |command| {
                assert_eq!(command, desired);
                writes.set(writes.get() + 1);
                Ok(())
            },
        )?;
        assert_eq!(writes.get(), 2);
        Ok(())
    }

    #[test]
    fn task_xml_preserves_disabled_and_unmanaged_ownership() -> Result<()> {
        let xml = r#"<Task xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
          <Triggers><LogonTrigger><Enabled>true</Enabled></LogonTrigger></Triggers>
          <Principals><Principal><UserId>S-1-5-21-123</UserId><RunLevel>LeastPrivilege</RunLevel></Principal></Principals>
          <Settings><Enabled>true</Enabled></Settings>
          <Actions><Exec><Command>C:\Users\alice\.cargo\bin\mcw-tray.exe</Command></Exec></Actions>
        </Task>"#;
        let current = Path::new(r"C:\Users\alice\.cargo\bin\mcw.exe");
        let task = parse_task(xml, "S-1-5-21-123")?;
        assert!(task.enabled && task.owned_by(current));
        assert!(!parse_task(xml, "S-1-5-21-999")?.owned_by(current));
        assert!(
            !parse_task(
                &xml.replace("<Enabled>true</Enabled>", "<Enabled>false</Enabled>"),
                "S-1-5-21-123"
            )?
            .enabled
        );
        assert!(
            !parse_task(
                &xml.replace("LeastPrivilege", "HighestAvailable"),
                "S-1-5-21-123"
            )?
            .owned_by(current)
        );
        assert!(
            !parse_task(
                &xml.replace("</Exec>", "</Exec><ComHandler/>"),
                "S-1-5-21-123"
            )?
            .owned_by(current)
        );
        Ok(())
    }

    #[test]
    fn scheduler_xml_utf16_decodes_without_locale_assumptions() -> Result<()> {
        let xml = "<?xml version=\"1.0\"?><Task/>";
        let mut bytes = vec![0xff, 0xfe];
        for word in xml.encode_utf16() {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        assert_eq!(decode_task_xml(&bytes)?, xml);
        assert_eq!(decode_task_xml(xml.as_bytes())?, xml);
        Ok(())
    }

    #[test]
    fn tray_version_rejects_prefixes_and_handles_read_boundaries() -> Result<()> {
        let path =
            std::env::temp_dir().join(format!("mcw-tray-version-{}.bin", std::process::id()));
        let check = || -> Result<()> {
            for padding in [0, 8190, 8192] {
                for (suffix, expected) in [
                    ("", true),
                    ("Microphone unavailable", true),
                    ("\0", true),
                    ("0", false),
                    (".1", false),
                    ("-beta", false),
                    ("+build", false),
                ] {
                    let mut bytes = vec![0; padding];
                    bytes.extend_from_slice(b"miccamwatch v0.13.5");
                    bytes.extend_from_slice(suffix.as_bytes());
                    std::fs::write(&path, bytes)?;
                    assert_eq!(tray_matches_version(&path, "0.13.5")?, expected);
                }
            }
            Ok(())
        };
        let result = check();
        let _ = std::fs::remove_file(path);
        result
    }
}

#[cfg(test)]
mod installation_lock_tests {
    use super::*;

    #[test]
    fn cli_and_tray_share_a_stable_nonreplaceable_installation_lock() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let cli = directory.path().join("mcw.exe");
        let tray = directory.path().join("mcw-tray.exe");
        let path = directory.path().join(".mcw-install.lock");
        let first = lock_installation(&cli)?;
        let competing = OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(3)
            .open(&path)?;
        assert!(competing.try_lock().is_err());
        // Held participants exclude deletion/replacement of the common identity.
        assert!(std::fs::remove_file(&path).is_err());
        drop(first);
        competing.try_lock()?;
        assert!(std::fs::remove_file(&path).is_err());
        drop(competing);
        let second = lock_installation(&tray)?;
        let competing = OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(3)
            .open(&path)?;
        assert!(competing.try_lock().is_err());
        drop(competing);
        drop(second);
        assert!(path.try_exists()?);
        Ok(())
    }
}
