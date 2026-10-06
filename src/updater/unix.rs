use crate::autostart::{Directory, ordinary_user};
use anyhow::{Context, Result, bail};
use std::{
    cmp::Ordering,
    ffi::OsStr,
    fs::File,
    io::Write,
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering as AtomicOrdering},
};
#[cfg(target_os = "macos")]
use std::{
    fs::{Metadata, OpenOptions},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
};

const INSTALLER: &str = include_str!("../../installer/install.sh");
const LATEST: &str = "https://github.com/Roman-Cuisset/miccamwatch/releases/latest";
const TAG_BASE: &str = "https://github.com/Roman-Cuisset/miccamwatch/releases/tag/";
static STAGING_ID: AtomicU64 = AtomicU64::new(0);
#[cfg(target_os = "linux")]
const CAMERA_PAYLOAD: [(&str, &str); 3] = [
    ("mcw-camera-helper", "camera-helper.sha256"),
    ("install-camera-helper.sh", "camera-installer.sha256"),
    (
        "com.roman-cuisset.miccamwatch.camera.policy",
        "camera-policy.sha256",
    ),
];

// All variable data travels as positional argv. The script never evaluates a
// receipt, interpolates a filename into shell code, or excludes a running PID.
// exec replaces the updater itself BEFORE the installer's running-target guard.
const ORCHESTRATION: &str = r#"#!/bin/sh
set -eu
umask 077
stage=$1
prefix=$2
tag=$3
target=$prefix/bin/mcw
cleanup() {
    rm -f "$stage/install.sh" "$stage/update.sh"
    rmdir "$stage"
}
trap cleanup 0
trap 'exit 130' INT
trap 'exit 143' TERM HUP
/bin/sh "$stage/install.sh" --version "$tag" --prefix "$prefix" --no-modify-path
reported=$("$target" --version)
[ "$reported" = "mcw ${tag#v}" ] || {
    printf '%s\n' 'mcw update: installed version validation failed; autostart was not refreshed.' >&2
    exit 1
}
"$target" autostart refresh || {
    printf '%s\n' 'mcw update: installation succeeded, but enabled autostart could not be refreshed; retry autostart enable after correcting the reported registration problem.' >&2
    exit 1
}
printf 'mcw updated to %s.\n' "${tag#v}"
"#;

#[cfg(target_os = "macos")]
const PORTABLE_ORCHESTRATION: &str = r#"#!/bin/sh
set -eu
umask 077
stage=$1
target=$2
tag=$3
digest=$4
cleanup() {
    rm -f "$stage/install.sh" "$stage/update.sh"
    rmdir "$stage"
}
trap cleanup 0
trap 'exit 130' INT
trap 'exit 143' TERM HUP
/bin/sh "$stage/install.sh" --update-portable "$target" --current-sha256 "$digest" --version "$tag" --no-modify-path
reported=$("$target" --version)
[ "$reported" = "mcw ${tag#v}" ] || {
    printf '%s\n' 'mcw update: installed version validation failed; autostart was not refreshed.' >&2
    exit 1
}
"$target" autostart refresh || {
    printf '%s\n' 'mcw update: installation succeeded, but enabled autostart could not be refreshed; retry autostart enable after correcting the reported registration problem.' >&2
    exit 1
}
printf 'mcw updated to %s.\n' "${tag#v}"
"#;

pub fn update() -> Result<()> {
    ordinary_user()?;
    #[cfg(target_os = "linux")]
    root_camera_installation_absent()?;
    let installation = Installation::inspect()?;
    let tag = latest_tag()?;
    let latest = Version::parse(
        tag.strip_prefix('v')
            .context("release tag must start with v")?,
    )?;
    let current = Version::parse(env!("CARGO_PKG_VERSION"))?;
    #[cfg(target_os = "macos")]
    if let Installation::Portable(portable) = &installation {
        portable.verify_unchanged()?;
    }
    if current.compare(&latest) != Ordering::Less {
        println!(
            "mcw {} is already up to date{}.",
            env!("CARGO_PKG_VERSION"),
            if current.compare(&latest) == Ordering::Greater {
                " (newer than the latest published release)"
            } else {
                ""
            }
        );
        return Ok(());
    }
    // The embedded installer rechecks ownership and running watchers/trays
    // under its own installation lock immediately before atomic replacement.
    let staging = Staging::new(installation.staging_parent())?;
    staging.write("install.sh", INSTALLER.as_bytes())?;
    staging.write("update.sh", installation.orchestration().as_bytes())?;
    let installer_mode = match &installation {
        Installation::Managed { .. } => "managed",
        #[cfg(target_os = "macos")]
        Installation::Portable(_) => "standalone",
    };
    println!(
        "Updating mcw {} to {} using the {} public installer.",
        env!("CARGO_PKG_VERSION"),
        &tag[1..],
        installer_mode
    );
    std::io::stdout().flush()?;
    let mut command = Command::new("/bin/sh");
    command
        .arg(staging.path.join("update.sh"))
        .arg(&staging.path);
    match &installation {
        Installation::Managed { prefix } => {
            command.arg(prefix).arg(&tag);
        }
        #[cfg(target_os = "macos")]
        Installation::Portable(portable) => {
            portable.verify_unchanged()?;
            command
                .arg(&portable.target)
                .arg(&tag)
                .arg(&portable.digest);
        }
    }
    let error = command
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .exec();
    // Successful exec never returns and the shell's trap owns cleanup. On an
    // exec error Rust retains ownership of the private staging directory.
    Err(error).context("cannot hand off the update to /bin/sh; installation was not changed")
}

#[cfg(target_os = "linux")]
fn root_camera_installation_absent() -> Result<()> {
    use std::os::unix::fs::MetadataExt;

    const GUIDANCE: &str = "User installation was preserved. First explicitly restore with the old matching mcw camera allow, then have an administrator run the reviewed root helper installer --uninstall. After root removal, retry the user update; afterward explicitly set up the matching same-version root helper if wanted. mcw update never elevates, calls camera APIs or removes the root helper.";
    // Metadata and directory names only: do not read privileged evidence or call
    // camera status. Allowed state cannot prove an empty restoration journal.
    for directory in [
        c"/",
        c"/usr",
        c"/usr/local",
        c"/usr/local/libexec",
        c"/usr/local/libexec/miccamwatch",
        c"/usr/share",
        c"/usr/share/polkit-1",
        c"/usr/share/polkit-1/actions",
        c"/var",
        c"/var/lib",
        c"/var/lib/miccamwatch",
    ] {
        let path = Path::new(directory.to_str().expect("static ASCII directory"));
        let metadata = match std::fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => bail!(
                "Cannot inspect root camera directory {}: {error}. {GUIDANCE}",
                path.display()
            ),
        };
        if !metadata.is_dir()
            || metadata.uid() != 0
            || metadata.mode() & 0o022 != 0
            || unsafe { libc::access(directory.as_ptr(), libc::R_OK | libc::X_OK) } != 0
        {
            bail!(
                "Root camera directory {} is unsafe or inaccessible. {GUIDANCE}",
                path.display()
            );
        }
    }
    for path in [
        "/usr/local/libexec/miccamwatch/mcw-camera-helper",
        "/usr/share/polkit-1/actions/com.roman-cuisset.miccamwatch.camera.policy",
        "/var/lib/miccamwatch/camera-helper-install.json",
        "/var/lib/miccamwatch/.camera-helper-transaction",
    ] {
        match std::fs::symlink_metadata(path) {
            Ok(_) => {
                bail!("Root camera installation or transaction is present at {path}. {GUIDANCE}")
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                bail!("Cannot inspect root camera installation at {path}: {error}. {GUIDANCE}")
            }
        }
    }
    match std::fs::read_dir("/var/lib/miccamwatch") {
        Ok(entries) => {
            for entry in entries {
                let entry = entry.with_context(|| {
                    format!("Cannot inspect root camera transaction remnants. {GUIDANCE}")
                })?;
                if entry
                    .file_name()
                    .as_encoded_bytes()
                    .starts_with(b".camera-helper-")
                {
                    bail!("Root camera transaction remnant is present. {GUIDANCE}");
                }
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => bail!("Cannot inspect root camera transaction remnants: {error}. {GUIDANCE}"),
    }
    Ok(())
}

enum Installation {
    Managed {
        prefix: PathBuf,
    },
    #[cfg(target_os = "macos")]
    Portable(PortableInstallation),
}

impl Installation {
    fn inspect() -> Result<Self> {
        let current = std::env::current_exe().context("cannot locate installed mcw")?;
        Self::inspect_at(&current)
    }

    fn inspect_at(current: &Path) -> Result<Self> {
        if !current.is_absolute() || current.file_name() != Some(OsStr::new("mcw")) {
            bail!("mcw update requires an absolute installed executable named mcw");
        }
        if current.parent().and_then(Path::file_name) != Some(OsStr::new("bin")) {
            #[cfg(target_os = "macos")]
            {
                let portable = PortableInstallation::open(current)?;
                verify_executable_version(current)?;
                portable.verify_unchanged()?;
                return Ok(Self::Portable(portable));
            }
            #[cfg(target_os = "linux")]
            bail!("mcw update requires a managed public-installer installation at PREFIX/bin/mcw");
        }
        // A bin layout is never eligible for portable fallback: a missing or
        // corrupt receipt may belong to a managed or package-manager install.
        let prefix = current
            .parent()
            .and_then(Path::parent)
            .context("cannot locate installation prefix")?
            .to_owned();
        let _prefix_directory = Directory::open(&prefix, false)?;
        let receipt = Directory::open(&prefix.join(".miccamwatch-install"), false).context(
            "no safe public-installer receipt; reinstall with the public installer instead",
        )?;
        validate_receipt_files(&receipt)?;
        if receipt.read("format")?.as_deref() != Some(b"1\n")
            || receipt.read("prefix")?.as_deref()
                != Some(
                    format!(
                        "{}\n",
                        prefix
                            .to_str()
                            .context("installation prefix must be UTF-8")?
                    )
                    .as_bytes(),
                )
        {
            bail!("unrecognized or mismatched installer receipt; installed files were preserved");
        }
        let version = receipt
            .read("version")?
            .context("installer version receipt is missing")?;
        let expected_version = format!("v{}\n", env!("CARGO_PKG_VERSION"));
        if version != expected_version.as_bytes() {
            bail!(
                "installed version receipt does not match this mcw executable; repair using the public installer before updating"
            );
        }
        let hashes = receipt
            .read("binary.sha256")?
            .context("installer binary ownership receipt is missing")?;
        let hashes = checksum_receipt(&hashes)?;
        let bin = Directory::open(current.parent().unwrap(), false)?;
        let executable = bin
            .file(OsStr::new("mcw"))?
            .context("installed executable disappeared")?;
        let digest = sha256(executable)?;
        if !hashes.iter().any(|hash| *hash == digest) {
            bail!(
                "installed executable was changed outside the public installer; it was preserved"
            );
        }
        verify_executable_version(current)?;
        #[cfg(target_os = "linux")]
        inspect_camera_payload(&prefix, &receipt)?;
        Ok(Self::Managed { prefix })
    }

    fn staging_parent(&self) -> &Path {
        match self {
            Self::Managed { prefix } => prefix,
            #[cfg(target_os = "macos")]
            Self::Portable(portable) => portable.target.parent().unwrap(),
        }
    }

    fn orchestration(&self) -> &'static str {
        match self {
            Self::Managed { .. } => ORCHESTRATION,
            #[cfg(target_os = "macos")]
            Self::Portable(_) => PORTABLE_ORCHESTRATION,
        }
    }
}

fn verify_executable_version(current: &Path) -> Result<()> {
    let output = Command::new(current)
        .arg("--version")
        .output()
        .context("cannot verify installed mcw version")?;
    if !output.status.success()
        || output.stdout != format!("mcw {}\n", env!("CARGO_PKG_VERSION")).as_bytes()
    {
        bail!(
            "installed executable does not match the running updater; installed files were preserved"
        );
    }
    Ok(())
}

#[cfg(target_os = "macos")]
struct PortableInstallation {
    target: PathBuf,
    digest: String,
    directory: Directory,
    parent: File,
    executable: File,
    identity: Metadata,
}

#[cfg(target_os = "macos")]
impl PortableInstallation {
    fn open(target: &Path) -> Result<Self> {
        if !target.is_absolute()
            || target.file_name() != Some(OsStr::new("mcw"))
            || target.parent().and_then(Path::file_name) == Some(OsStr::new("bin"))
        {
            bail!("portable update requires an absolute standalone mcw outside bin");
        }
        let text = target
            .to_str()
            .context("portable executable path must be UTF-8")?;
        if text.contains(':') || text.chars().any(char::is_control) {
            bail!("portable executable path contains a colon or control character");
        }
        let parent_path = target
            .parent()
            .context("portable executable has no parent")?;
        let directory = Directory::open(parent_path, false)?;
        let parent = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(parent_path)
            .context("cannot retain portable executable parent")?;
        let metadata = parent.metadata()?;
        if metadata.uid() != unsafe { libc::getuid() } || metadata.mode() & 0o022 != 0 {
            bail!("portable executable parent must be owned by you and not writable by others");
        }
        let executable = directory
            .file(OsStr::new("mcw"))?
            .context("portable executable disappeared")?;
        let identity = executable.metadata()?;
        if identity.mode() & 0o100 == 0 {
            bail!("portable mcw must be executable by its owner");
        }
        let digest = sha256(executable.try_clone()?)?;
        Ok(Self {
            target: target.to_owned(),
            digest,
            directory,
            parent,
            executable,
            identity,
        })
    }

    fn verify_unchanged(&self) -> Result<()> {
        let parent_path = self.target.parent().unwrap();
        // Validate all ancestors again as well as the retained parent identity.
        let directory = Directory::open(parent_path, false)?;
        let parent = std::fs::symlink_metadata(parent_path)?;
        let retained = self.parent.metadata()?;
        if !parent.is_dir()
            || parent.dev() != retained.dev()
            || parent.ino() != retained.ino()
            || parent.uid() != unsafe { libc::getuid() }
            || parent.mode() & 0o022 != 0
        {
            bail!("portable executable parent changed; installed files were preserved");
        }
        let executable = directory
            .file(OsStr::new("mcw"))?
            .context("portable executable disappeared")?;
        let retained_executable = self
            .directory
            .file(OsStr::new("mcw"))?
            .context("retained portable executable disappeared")?;
        if !same_executable(&self.identity, &executable.metadata()?)
            || !same_executable(&self.identity, &self.executable.metadata()?)
            || !same_executable(&self.identity, &retained_executable.metadata()?)
            || sha256(executable)? != self.digest
        {
            bail!("portable executable changed during inspection; it was preserved");
        }
        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn same_executable(left: &Metadata, right: &Metadata) -> bool {
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.len() == right.len()
        && left.uid() == right.uid()
        && left.mode() == right.mode()
        && left.nlink() == right.nlink()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

#[cfg(target_os = "linux")]
fn inspect_camera_payload(prefix: &Path, receipt: &Directory) -> Result<()> {
    let required = Version::parse(env!("CARGO_PKG_VERSION"))?.core >= [0, 16, 0];
    if !required {
        let mut recorded = false;
        for (_, name) in CAMERA_PAYLOAD {
            if receipt.read(name)?.is_some() {
                recorded = true;
                break;
            }
        }
        if !recorded {
            return Ok(());
        }
    }
    let path = prefix.join("share/miccamwatch/linux-camera");
    let payload = Directory::open(&path, false)
        .context("camera payload is missing or unsafe; repair with the public installer")?;
    for (name, record) in CAMERA_PAYLOAD {
        let hashes = receipt
            .read(record)?
            .with_context(|| format!("camera payload ownership receipt is missing: {record}"))?;
        let hashes = checksum_receipt(&hashes)?;
        let file = payload
            .file(OsStr::new(name))?
            .with_context(|| format!("installed camera payload is missing: {name}"))?;
        let digest = sha256(file)?;
        if !hashes.iter().any(|hash| *hash == digest) {
            bail!(
                "camera payload {name} was changed outside the public installer; it was preserved"
            );
        }
    }
    let output = Command::new(path.join("mcw-camera-helper"))
        .arg("--protocol-version")
        .output()
        .context("cannot verify installed camera helper protocol/app version")?;
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct HelperVersion {
        protocol: u64,
        version: String,
    }
    let reported: HelperVersion = serde_json::from_slice(&output.stdout)
        .context("invalid installed camera helper protocol response")?;
    if !output.status.success()
        || reported.protocol != 1
        || reported.version != env!("CARGO_PKG_VERSION")
    {
        bail!("installed camera helper does not match mcw; repair with the public installer");
    }
    Ok(())
}

fn validate_receipt_files(receipt: &Directory) -> Result<()> {
    for name in receipt.names()? {
        let text = name
            .to_str()
            .context("non-UTF-8 installer receipt filename")?;
        let path_record = receipt_name(text)?;
        receipt
            .file(&name)?
            .context("installer receipt changed concurrently")?;
        if let Some(index) = path_record {
            receipt
                .file(OsStr::new(&format!("path-{index}.block")))?
                .context("PATH filename receipt is missing its safe ownership block")?;
        }
    }
    Ok(())
}

// Return the corresponding block index only for .path records. Orphaned .block
// snapshots are accepted by the installer because a crash may precede .path.
fn receipt_name(name: &str) -> Result<Option<&str>> {
    if matches!(name, "format" | "prefix" | "version" | "binary.sha256") {
        return Ok(None);
    }
    #[cfg(target_os = "linux")]
    if CAMERA_PAYLOAD.iter().any(|(_, record)| *record == name) {
        return Ok(None);
    }
    let rest = name
        .strip_prefix("path-")
        .context("unknown installer receipt content; it was preserved")?;
    let (index, suffix) = rest
        .rsplit_once('.')
        .context("invalid PATH receipt filename")?;
    if index.is_empty()
        || !index.bytes().all(|c| c.is_ascii_digit())
        || !matches!(suffix, "path" | "block")
    {
        bail!("invalid PATH receipt filename");
    }
    Ok((suffix == "path").then_some(index))
}

fn checksum_receipt(bytes: &[u8]) -> Result<Vec<&str>> {
    let text = std::str::from_utf8(bytes).context("binary ownership receipt is not UTF-8")?;
    let lines = text.lines().collect::<Vec<_>>();
    if lines.is_empty()
        || lines.len() > 2
        || lines.iter().any(|line| {
            line.len() != 64
                || !line
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
    {
        bail!("invalid binary ownership receipt; installed files were preserved");
    }
    Ok(lines)
}

fn sha256(file: File) -> Result<String> {
    #[cfg(target_os = "linux")]
    let mut command = Command::new("/usr/bin/sha256sum");
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("/usr/bin/shasum");
        command.args(["-a", "256"]);
        command
    };
    let output = command
        .stdin(Stdio::from(file))
        .output()
        .context("cannot checksum installed executable")?;
    if !output.status.success() {
        bail!(
            "installed executable checksum failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let text = std::str::from_utf8(&output.stdout).context("invalid checksum command response")?;
    let digest = text
        .split_whitespace()
        .next()
        .context("checksum command omitted digest")?;
    checksum_receipt(digest.as_bytes())?;
    Ok(digest.to_owned())
}

fn latest_tag() -> Result<String> {
    let output = Command::new("/usr/bin/curl")
        .args([
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "--tlsv1.2",
            "--fail",
            "--location",
            "--silent",
            "--show-error",
            "--connect-timeout",
            "30",
            "--max-time",
            "300",
            "--head",
            "--output",
            "/dev/null",
            "--write-out",
            "%{url_effective}",
            LATEST,
        ])
        .stdin(Stdio::null())
        .output()
        .context("cannot execute curl to check the latest release")?;
    if !output.status.success() {
        bail!(
            "cannot resolve latest MicCamWatch release: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    if output.stdout.len() > 4096 {
        bail!("latest release URL exceeds size limit");
    }
    let url = std::str::from_utf8(&output.stdout).context("latest release URL is not UTF-8")?;
    let tag = url
        .strip_prefix(TAG_BASE)
        .context("latest did not resolve to a MicCamWatch release tag")?;
    let version = tag
        .strip_prefix('v')
        .context("release tag must start with v")?;
    Version::parse(version).context("latest release does not have a supported semantic version")?;
    Ok(tag.to_owned())
}

struct Staging {
    path: PathBuf,
    name: String,
    parent: Directory,
    directory: Directory,
}

impl Staging {
    fn new(prefix: &Path) -> Result<Self> {
        let parent = Directory::open(prefix, false)?;
        for _ in 0..32 {
            let name = format!(
                ".mcw-update-{}-{}",
                std::process::id(),
                STAGING_ID.fetch_add(1, AtomicOrdering::Relaxed)
            );
            if let Some(directory) = parent.create_directory(&name)? {
                return Ok(Self {
                    path: prefix.join(&name),
                    name,
                    parent,
                    directory,
                });
            }
        }
        bail!("cannot allocate a unique private updater staging directory");
    }

    fn write(&self, name: &str, bytes: &[u8]) -> Result<()> {
        self.directory.create_file(name, bytes)
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        let _ = self.directory.remove_entry("install.sh", false);
        let _ = self.directory.remove_entry("update.sh", false);
        let _ = self.parent.remove_entry(&self.name, true);
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Version<'a> {
    core: [u64; 3],
    pre: Vec<&'a str>,
}

impl<'a> Version<'a> {
    fn parse(text: &'a str) -> Result<Self> {
        let (base, build) = text
            .split_once('+')
            .map_or((text, None), |(base, build)| (base, Some(build)));
        if let Some(build) = build {
            identifiers(build, false)?;
        }
        let (core, pre) = base
            .split_once('-')
            .map_or((base, Vec::new()), |(core, pre)| {
                (core, pre.split('.').collect())
            });
        if !pre.is_empty() {
            identifiers(base.split_once('-').unwrap().1, true)?;
        }
        let components = core.split('.').collect::<Vec<_>>();
        if components.len() != 3 {
            bail!("semantic version requires major.minor.patch");
        }
        let mut numbers = [0; 3];
        for (number, text) in numbers.iter_mut().zip(components) {
            if text.is_empty()
                || !text.bytes().all(|c| c.is_ascii_digit())
                || (text.len() > 1 && text.starts_with('0'))
            {
                bail!("invalid semantic version number");
            }
            *number = text.parse().context("semantic version number overflows")?;
        }
        Ok(Self { core: numbers, pre })
    }

    fn compare(&self, other: &Self) -> Ordering {
        let core = self.core.cmp(&other.core);
        if core != Ordering::Equal {
            return core;
        }
        if self.pre.is_empty() != other.pre.is_empty() {
            return if self.pre.is_empty() {
                Ordering::Greater
            } else {
                Ordering::Less
            };
        }
        for (left, right) in self.pre.iter().zip(&other.pre) {
            let numeric_left = left.bytes().all(|c| c.is_ascii_digit());
            let numeric_right = right.bytes().all(|c| c.is_ascii_digit());
            let ordering = match (numeric_left, numeric_right) {
                (true, true) => left.len().cmp(&right.len()).then_with(|| left.cmp(right)),
                (true, false) => Ordering::Less,
                (false, true) => Ordering::Greater,
                (false, false) => left.cmp(right),
            };
            if ordering != Ordering::Equal {
                return ordering;
            }
        }
        self.pre.len().cmp(&other.pre.len())
    }
}

fn identifiers(text: &str, numeric_leading_zero_rejected: bool) -> Result<()> {
    for part in text.split('.') {
        if part.is_empty()
            || !part.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
            || (numeric_leading_zero_rejected
                && part.len() > 1
                && part.starts_with('0')
                && part.bytes().all(|c| c.is_ascii_digit()))
        {
            bail!("invalid semantic version identifier");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_os = "macos")]
    use std::os::unix::fs::symlink;
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};

    struct Scratch(PathBuf);

    impl Scratch {
        fn new() -> Result<Self> {
            let name = format!(
                "mcw-updater-test-{}-{}",
                std::process::id(),
                STAGING_ID.fetch_add(1, AtomicOrdering::Relaxed)
            );
            let path = PathBuf::from(std::env::var_os("HOME").context("HOME is not set")?)
                .canonicalize()?
                .join(name);
            std::fs::DirBuilder::new().mode(0o700).create(&path)?;
            Ok(Self(path))
        }

        fn executable(&self) -> Result<PathBuf> {
            let path = self.0.join("mcw");
            // Filesystem-only fixture: never used as a substitute for a CLI.
            std::fs::write(&path, b"owned fixture contents\n")?;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))?;
            Ok(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn managed_layout_without_valid_receipt_never_becomes_portable() -> Result<()> {
        let scratch = Scratch::new()?;
        let bin = scratch.0.join("bin");
        std::fs::create_dir(&bin)?;
        let target = bin.join("mcw");
        std::fs::write(&target, b"not an executable")?;
        assert!(Installation::inspect_at(&target).is_err());
        let receipt = scratch.0.join(".miccamwatch-install");
        std::fs::DirBuilder::new().mode(0o700).create(&receipt)?;
        let format = receipt.join("format");
        std::fs::write(&format, b"corrupt\n")?;
        std::fs::set_permissions(&format, std::fs::Permissions::from_mode(0o600))?;
        assert!(Installation::inspect_at(&target).is_err());
        assert_eq!(std::fs::read(&target)?, b"not an executable");
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_owned_standalone_still_requires_managed_installation() -> Result<()> {
        let scratch = Scratch::new()?;
        let target = scratch.executable()?;
        assert!(Installation::inspect_at(&target).is_err());
        assert_eq!(std::fs::read(&target)?, b"owned fixture contents\n");
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn portable_snapshot_admits_owned_file_and_detects_replacement() -> Result<()> {
        let scratch = Scratch::new()?;
        let target = scratch.executable()?;
        let portable = PortableInstallation::open(&target)?;
        // A safe path and digest alone do not authorize an arbitrary executable.
        assert!(Installation::inspect_at(&target).is_err());
        let replacement = scratch.0.join("replacement");
        std::fs::write(&replacement, b"owned fixture contents\n")?;
        std::fs::set_permissions(&replacement, std::fs::Permissions::from_mode(0o700))?;
        std::fs::rename(replacement, &target)?;
        assert!(portable.verify_unchanged().is_err());
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn portable_snapshot_refuses_aliases_and_unsafe_ownership() -> Result<()> {
        let scratch = Scratch::new()?;
        let target = scratch.executable()?;
        let hardlink = scratch.0.join("hardlink");
        std::fs::hard_link(&target, &hardlink)?;
        assert!(PortableInstallation::open(&target).is_err());
        std::fs::remove_file(hardlink)?;
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o722))?;
        assert!(PortableInstallation::open(&target).is_err());
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o600))?;
        assert!(PortableInstallation::open(&target).is_err());
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o700))?;
        let saved = scratch.0.join("saved");
        std::fs::rename(&target, &saved)?;
        symlink(&saved, &target)?;
        assert!(PortableInstallation::open(&target).is_err());
        std::fs::remove_file(&target)?;
        std::fs::rename(&saved, &target)?;
        let alias = scratch.0.join("alias");
        symlink(&scratch.0, &alias)?;
        assert!(PortableInstallation::open(&alias.join("mcw")).is_err());
        std::fs::set_permissions(&scratch.0, std::fs::Permissions::from_mode(0o777))?;
        assert!(PortableInstallation::open(&target).is_err());
        std::fs::set_permissions(&scratch.0, std::fs::Permissions::from_mode(0o700))?;
        // Existing root-owned macOS directory; refusal precedes file lookup.
        if unsafe { libc::getuid() } != 0 {
            assert!(PortableInstallation::open(Path::new("/usr/libexec/mcw")).is_err());
            let system = Directory::open(Path::new("/usr/libexec"), false)?;
            assert!(system.file(OsStr::new("PlistBuddy")).is_err());
        }
        assert!(PortableInstallation::open(&scratch.0.join("other-name")).is_err());
        assert!(PortableInstallation::open(Path::new("mcw")).is_err());
        Ok(())
    }

    #[test]
    fn actual_executable_output_must_identify_this_cli_version() -> Result<()> {
        // A genuine executable with successful exit but no mcw version output
        // must not pass version identity validation.
        assert!(verify_executable_version(Path::new("/usr/bin/true")).is_err());
        Ok(())
    }

    #[test]
    fn installer_receipts_do_not_authorize_unknown_or_ambiguous_files() -> Result<()> {
        assert_eq!(receipt_name("path-12.path")?, Some("12"));
        assert_eq!(receipt_name("path-12.block")?, None);
        #[cfg(target_os = "linux")]
        for (_, record) in CAMERA_PAYLOAD {
            assert_eq!(receipt_name(record)?, None);
        }
        for invalid in [
            "extra",
            "path-.path",
            "path-../other.path",
            "path-1.other",
            "path-1.path.backup",
            "path-1x.path",
            "camera-extra.sha256",
            "camera-helper.sha256.backup",
            "camera-helper.path",
        ] {
            assert!(receipt_name(invalid).is_err(), "{invalid}");
        }
        Ok(())
    }

    #[test]
    fn release_comparison_never_downgrades_and_obeys_prerelease_precedence() -> Result<()> {
        let ordered = [
            "1.0.0-alpha",
            "1.0.0-alpha.1",
            "1.0.0-alpha.beta",
            "1.0.0-beta",
            "1.0.0-beta.2",
            "1.0.0-beta.11",
            "1.0.0-rc.1",
            "1.0.0",
            "1.0.1",
            "2.0.0",
        ];
        for pair in ordered.windows(2) {
            let earlier = Version::parse(pair[0])?;
            let later = Version::parse(pair[1])?;
            assert_eq!(earlier.compare(&later), Ordering::Less);
            assert_eq!(later.compare(&earlier), Ordering::Greater);
        }
        assert_eq!(
            Version::parse("1.0.0+new")?.compare(&Version::parse("1.0.0+old")?),
            Ordering::Equal
        );
        for invalid in [
            "1.0",
            "01.0.0",
            "1.0.0-01",
            "1.0.0-",
            "1.0.0+",
            "1.0.0/../../other",
            "1.0.0+meta+more",
        ] {
            assert!(Version::parse(invalid).is_err(), "{invalid}");
        }
        Ok(())
    }

    #[test]
    fn checksum_ownership_accepts_only_installer_journal_hashes() -> Result<()> {
        let first = "a".repeat(64);
        let second = "1".repeat(64);
        let journal = format!("{first}\n{second}\n");
        assert_eq!(
            checksum_receipt(journal.as_bytes())?,
            [first.as_str(), second.as_str()]
        );
        for invalid in [
            String::new(),
            "a".repeat(63),
            "G".repeat(64),
            format!("{first}\n\n"),
            format!("{first}\n{first}\n{first}\n"),
            format!("{first}  mcw\n"),
        ] {
            assert!(checksum_receipt(invalid.as_bytes()).is_err());
        }
        Ok(())
    }
}
