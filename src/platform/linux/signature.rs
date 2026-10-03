//! Detached OpenPGP only: `<executable>.sig`, never package-maintainer names.
//! Publisher rules pin `openpgp:<FULL_UPPERCASE_PRIMARY_FINGERPRINT>` (40 or
//! 64 hex digits). A valid signature authenticates that key, not its UID or a
//! human publisher name; the explicit fingerprint rule supplies the trust pin.
//! Offline overrides automatic retrieval/import/WKD. Online permits retrieval
//! only through the user's configured keyservers, not a signature-supplied URL
//! or signer UID. Keyserver operators may learn the requested key, IP and time.
//! See GnuPG doc/DETAILS and GPG-Configuration-Options.html.

use crate::{config::TrustPolicy, model::SignatureInfo};
use anyhow::{Context, Result, bail};
use std::{
    fs::{self, File, Metadata},
    io::{ErrorKind, Read},
    os::{
        fd::AsRawFd,
        unix::{fs::MetadataExt, process::CommandExt},
    },
    path::Path,
    process::{Command, ExitStatus, Stdio},
    time::{Duration, Instant},
};

const STATUS_LIMIT: usize = 64 * 1024;
const VERIFY_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileStamp {
    device: u64,
    inode: u64,
    size: u64,
    modified: (i64, i64),
    changed: (i64, i64),
}

impl FileStamp {
    fn read(metadata: &Metadata) -> Result<Self> {
        if !metadata.is_file() {
            bail!("signature verification requires regular files");
        }
        Ok(Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            size: metadata.size(),
            modified: (metadata.mtime(), metadata.mtime_nsec()),
            changed: (metadata.ctime(), metadata.ctime_nsec()),
        })
    }
}

/// Err means evidence unavailable, never a policy mismatch. Ok(false) is
/// confirmed bad/revoked/expired evidence. Files remain open throughout GnuPG
/// verification and both descriptor and pathname identities are checked again.
/// No verification cache; this is on-disk evidence, not proof of loaded pages.
pub(crate) fn verify_signature_with_policy(
    path: &str,
    trust_policy: TrustPolicy,
) -> Result<SignatureInfo> {
    let executable_path = Path::new(path);
    if !executable_path.is_absolute() {
        bail!("executable signature path must be absolute");
    }
    let mut signature_path = executable_path.as_os_str().to_os_string();
    signature_path.push(".sig");
    let signature_path = Path::new(&signature_path);
    let executable =
        File::open(executable_path).context("executable signature evidence unavailable")?;
    let signature =
        File::open(signature_path).context("detached OpenPGP .sig evidence unavailable")?;
    let executable_stamp = FileStamp::read(&executable.metadata()?)?;
    let signature_stamp = FileStamp::read(&signature.metadata()?)?;
    let (status, output) = run_verifier(&executable, &signature, trust_policy)?;
    for (file, path, original) in [
        (&executable, executable_path, executable_stamp),
        (&signature, signature_path, signature_stamp),
    ] {
        if FileStamp::read(&file.metadata()?)? != original
            || FileStamp::read(&fs::metadata(path)?)? != original
        {
            bail!(
                "executable or detached signature changed during verification; identity evidence unavailable"
            );
        }
    }
    parse_status(&output, status.success())
}

fn run_verifier(
    executable: &File,
    signature: &File,
    trust_policy: TrustPolicy,
) -> Result<(ExitStatus, Vec<u8>)> {
    let executable_fd = executable.as_raw_fd();
    let signature_fd = signature.as_raw_fd();
    let mut command = Command::new("gpg");
    command.args([
        "--batch",
        "--no-tty",
        "--status-fd=1",
        "--no-auto-key-import",
        "--auto-key-locate",
        "clear",
        "--disable-signer-uid",
        "--keyserver-options",
        "no-honor-keyserver-url",
        "--verify-options",
        "no-show-photos",
        match trust_policy {
            TrustPolicy::Offline => "--no-auto-key-retrieve",
            TrustPolicy::Online => "--auto-key-retrieve",
        },
        "--verify",
        "--",
    ]);
    command.arg(format!("/proc/self/fd/{signature_fd}"));
    command.arg(format!("/proc/self/fd/{executable_fd}"));
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    // Only these two read-only descriptors cross exec. fcntl is async-signal
    // safe; no allocation, locks, or Rust callbacks execute in the child.
    unsafe {
        command.pre_exec(move || {
            for fd in [executable_fd, signature_fd] {
                if libc::fcntl(fd, libc::F_SETFD, 0) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
    let mut child = command.spawn().context("GnuPG verifier unavailable")?;
    let result = (|| {
        let mut stdout = child
            .stdout
            .take()
            .context("GnuPG status pipe unavailable")?;
        let fd = stdout.as_raw_fd();
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags == -1 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } == -1
        {
            return Err(std::io::Error::last_os_error().into());
        }
        let deadline = Instant::now() + VERIFY_TIMEOUT;
        let mut output = Vec::new();
        let mut buffer = [0u8; 4096];
        loop {
            if Instant::now() >= deadline {
                bail!("GnuPG verification timed out; identity evidence unavailable");
            }
            loop {
                match stdout.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(count) => {
                        if output.len() + count > STATUS_LIMIT {
                            bail!("GnuPG status output exceeded its bounded limit");
                        }
                        output.extend_from_slice(&buffer[..count]);
                    }
                    Err(error) if error.kind() == ErrorKind::WouldBlock => break,
                    Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                    Err(error) => return Err(error.into()),
                }
            }
            if let Some(status) = child.try_wait()? {
                // Drain bytes written between the last read and process exit.
                loop {
                    match stdout.read(&mut buffer) {
                        Ok(0) => break,
                        Ok(count) => {
                            if output.len() + count > STATUS_LIMIT {
                                bail!("GnuPG status output exceeded its bounded limit");
                            }
                            output.extend_from_slice(&buffer[..count]);
                        }
                        Err(error) if error.kind() == ErrorKind::WouldBlock => break,
                        Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                        Err(error) => return Err(error.into()),
                    }
                }
                return Ok((status, output));
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    })();
    if result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    result
}

fn canonical_fingerprint(value: &str) -> Option<String> {
    matches!(value.len(), 40 | 64)
        .then_some(value)
        .filter(|value| value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .map(str::to_ascii_uppercase)
}

fn parse_status(output: &[u8], success: bool) -> Result<SignatureInfo> {
    let output = std::str::from_utf8(output).context("GnuPG status is not UTF-8")?;
    let mut signer: Option<String> = None;
    let mut invalid = None;
    let mut unavailable = None;
    for line in output.lines() {
        let Some(status) = line.strip_prefix("[GNUPG:] ") else {
            continue;
        };
        let mut fields = status.split_ascii_whitespace();
        match fields.next() {
            Some("VALIDSIG") => {
                // DETAILS: signing subkey fpr, date, timestamp, expiry, version,
                // reserved, public-key algorithm, hash algorithm, class, PRIMARY fpr.
                // Never mistake a short key ID, UID, or subkey for the primary pin.
                let signing = fields.next().and_then(canonical_fingerprint);
                let primary = fields.nth(8).and_then(canonical_fingerprint);
                match (signing, primary) {
                    (Some(_), Some(primary))
                        if signer.as_ref().is_none_or(|value| value == &primary) =>
                    {
                        signer = Some(primary);
                    }
                    (Some(_), Some(_)) => {
                        unavailable = Some(
                            "multiple different primary signers cannot be represented by one publisher identity",
                        )
                    }
                    _ => unavailable = Some("canonical OpenPGP primary fingerprint unavailable"),
                }
            }
            Some("BADSIG") => invalid = Some("OpenPGP cryptographic signature is invalid"),
            Some("REVKEYSIG") => invalid = Some("OpenPGP signing key is revoked"),
            Some("EXPKEYSIG") => invalid = Some("OpenPGP signing key is expired"),
            Some("EXPSIG") => invalid = Some("OpenPGP signature is expired"),
            Some("NO_PUBKEY" | "ERRSIG" | "NODATA" | "ERROR") => {
                unavailable =
                    Some("OpenPGP signature, supported algorithm, or verification key unavailable");
            }
            Some("FAILURE") => {
                // gpg-exit summarizes the process exit, including a completed
                // BADSIG. It is not an independent missing-evidence condition.
                if fields.next() != Some("gpg-exit") {
                    unavailable = Some("OpenPGP verifier could not complete verification");
                }
            }
            _ => {}
        }
    }
    if let Some(error) = unavailable {
        bail!("{error}");
    }
    if let Some(error) = invalid {
        return Ok(SignatureInfo {
            verified: false,
            signer: signer.map(|value| format!("openpgp:{value}")),
            error: Some(error.to_owned()),
        });
    }
    if !success {
        bail!("GnuPG did not complete verification; identity evidence unavailable");
    }
    let signer = signer.context("no verified OpenPGP primary fingerprint available")?;
    Ok(SignatureInfo {
        verified: true,
        signer: Some(format!("openpgp:{signer}")),
        error: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    const PRIMARY: &str = "0123456789ABCDEF0123456789ABCDEF01234567";
    const SUBKEY: &str = "FEDCBA9876543210FEDCBA9876543210FEDCBA98";

    fn valid() -> String {
        format!("[GNUPG:] VALIDSIG {SUBKEY} 2026-01-01 1767225600 0 4 0 1 8 00 {PRIMARY}\n")
    }

    #[test]
    fn publisher_is_full_primary_fingerprint_not_signing_subkey_or_uid() {
        let output = format!("[GNUPG:] GOODSIG DEADBEEF Fake Publisher\n{}", valid());
        let signature = parse_status(output.as_bytes(), true).unwrap();
        assert!(signature.verified);
        assert_eq!(
            signature.signer.as_deref(),
            Some(format!("openpgp:{PRIMARY}").as_str())
        );
        assert!(parse_status(b"[GNUPG:] GOODSIG DEADBEEF Fake Publisher\n", true).is_err());
        assert!(
            parse_status(
                b"[GNUPG:] VALIDSIG DEADBEEF 2026-01-01 0 0 4 0 1 8 00 DEADBEEF\n",
                true
            )
            .is_err()
        );
    }

    #[test]
    fn unavailable_and_invalid_crypto_are_distinct_even_when_mixed() {
        let invalid =
            b"[GNUPG:] BADSIG DEADBEEF Fake Publisher\n[GNUPG:] FAILURE gpg-exit 33554433\n";
        assert!(!parse_status(invalid, false).unwrap().verified);
        for unavailable in [
            "NO_PUBKEY DEADBEEF",
            "ERRSIG DEADBEEF 1 8 00 0 9",
            "NODATA 1",
            "FAILURE verify 1",
        ] {
            let mixed = format!(
                "{}[GNUPG:] {unavailable}\n",
                std::str::from_utf8(invalid).unwrap()
            );
            assert!(parse_status(mixed.as_bytes(), false).is_err());
        }
        assert!(parse_status(valid().as_bytes(), false).is_err());
    }

    #[test]
    fn revoked_and_expired_keys_do_not_become_valid_publishers() {
        for invalid in ["REVKEYSIG", "EXPKEYSIG", "EXPSIG"] {
            let output = format!("[GNUPG:] {invalid} DEADBEEF Name\n{}", valid());
            assert!(!parse_status(output.as_bytes(), true).unwrap().verified);
        }
        let output = format!(
            "{}[GNUPG:] VALIDSIG {PRIMARY} 2026-01-01 0 0 4 0 1 8 00 {SUBKEY}\n",
            valid()
        );
        assert!(parse_status(output.as_bytes(), true).is_err());
    }
}
