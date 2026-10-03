//! Native Security.framework verification through the existing embedded helper.
//! Offline forbids validation network work. Online explicitly forces native
//! OCSP/CRL checks, but Apple's best-attempt policy does not guarantee a fresh
//! positive revocation response. Neither mode makes a Gatekeeper/notarization
//! claim. A trusted certificate identity and sealed on-disk integrity are both
//! required; executable metadata checks are not a lock on bundle resources.

use crate::{config::TrustPolicy, model::SignatureInfo};
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::{fs, os::unix::fs::MetadataExt, path::Path, time::Duration};

#[derive(Deserialize)]
struct Output {
    signature: Option<MacSignatureReply>,
}

#[derive(Deserialize)]
struct MacSignatureReply {
    available: bool,
    verified: bool,
    signer: Option<String>,
    error: Option<String>,
}

/// Err denotes unavailable evidence; only a completed native negative verdict
/// becomes Ok(verified=false). Parent must not substitute a failed transport
/// request or unavailable signature with fabricated unsigned evidence.
pub(crate) fn verify_signature_with_policy(
    path: &str,
    trust_policy: TrustPolicy,
) -> Result<SignatureInfo> {
    if !Path::new(path).is_absolute() {
        bail!("executable signature path must be absolute");
    }
    let before = fs::metadata(path).context("executable signature evidence unavailable")?;
    if !before.is_file() {
        bail!("executable signature verification requires a regular file");
    }
    let output: Output = super::native_request(
        "signature",
        &[
            path,
            match trust_policy {
                TrustPolicy::Offline => "offline",
                TrustPolicy::Online => "online",
            },
        ],
        Duration::from_secs(20),
    )?;
    let after = fs::metadata(path).context("executable changed during native verification")?;
    if (
        before.dev(),
        before.ino(),
        before.size(),
        before.mtime(),
        before.mtime_nsec(),
        before.ctime(),
        before.ctime_nsec(),
    ) != (
        after.dev(),
        after.ino(),
        after.size(),
        after.mtime(),
        after.mtime_nsec(),
        after.ctime(),
        after.ctime_nsec(),
    ) {
        bail!("executable changed during native verification; identity evidence unavailable");
    }
    let reply = output
        .signature
        .context("native signature reply unavailable")?;
    if !reply.available {
        bail!(
            "{}",
            reply
                .error
                .as_deref()
                .unwrap_or("native signing identity unavailable")
        );
    }
    if reply.verified
        && reply
            .signer
            .as_ref()
            .is_none_or(|value| value.trim().is_empty())
    {
        bail!("native verification did not return a trusted certificate identity");
    }
    Ok(SignatureInfo {
        verified: reply.verified,
        signer: reply.signer,
        error: reply.error,
    })
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use std::{
        io::{Seek, SeekFrom, Write},
        process::{Command, Output},
    };

    // This standalone Apple-signed Mach-O is present on both macOS architectures.
    // Only disposable copies are changed; none of these fixtures is executed.
    const APPLE_EXECUTABLE: &str = "/usr/bin/true";

    fn codesign(arguments: &[&str], path: &Path) -> Result<Output> {
        Command::new("/usr/bin/codesign")
            .args(arguments)
            .arg(path)
            .output()
            .context("running the system codesign fixture tool")
    }

    fn assert_trusted_identity(signature: &SignatureInfo) {
        assert!(signature.verified, "{signature:?}");
        assert!(
            signature
                .signer
                .as_ref()
                .is_some_and(|signer| !signer.trim().is_empty()),
            "{signature:?}"
        );
        assert!(signature.error.is_none(), "{signature:?}");
    }

    fn assert_native_rejection(signature: &SignatureInfo) {
        assert!(!signature.verified, "{signature:?}");
        assert!(signature.signer.is_none(), "{signature:?}");
        assert!(
            signature
                .error
                .as_ref()
                .is_some_and(|error| !error.trim().is_empty()),
            "{signature:?}"
        );
    }

    // Locate a sealed byte without damaging the Mach-O header/load commands or
    // signature blob. A universal image's first slice is enough because native
    // verification checks all architectures, not just the running architecture.
    fn text_segment_last_byte(bytes: &[u8]) -> usize {
        let slice = match bytes.get(..4).expect("Mach-O fixture header") {
            [0xca, 0xfe, 0xba, 0xbe] => {
                u32::from_be_bytes(bytes[16..20].try_into().unwrap()) as usize
            }
            [0xca, 0xfe, 0xba, 0xbf] => {
                usize::try_from(u64::from_be_bytes(bytes[16..24].try_into().unwrap())).unwrap()
            }
            _ => 0,
        };
        let image = &bytes[slice..];
        assert_eq!(
            &image[..4],
            &[0xcf, 0xfa, 0xed, 0xfe],
            "expected a little-endian 64-bit macOS executable"
        );
        let command_count = u32::from_le_bytes(image[16..20].try_into().unwrap());
        let command_bytes = u32::from_le_bytes(image[20..24].try_into().unwrap()) as usize;
        let commands_end = 32 + command_bytes;
        assert!(commands_end <= image.len());
        let mut cursor = 32;
        for _ in 0..command_count {
            let command = u32::from_le_bytes(image[cursor..cursor + 4].try_into().unwrap());
            let size =
                u32::from_le_bytes(image[cursor + 4..cursor + 8].try_into().unwrap()) as usize;
            assert!(size >= 8 && cursor + size <= commands_end);
            // LC_SEGMENT_64, whose segment name starts after cmd/cmdsize.
            if command == 0x19 {
                assert!(size >= 72);
                if &image[cursor + 8..cursor + 24] == b"__TEXT\0\0\0\0\0\0\0\0\0\0" {
                    let offset = usize::try_from(u64::from_le_bytes(
                        image[cursor + 40..cursor + 48].try_into().unwrap(),
                    ))
                    .unwrap();
                    let length = usize::try_from(u64::from_le_bytes(
                        image[cursor + 48..cursor + 56].try_into().unwrap(),
                    ))
                    .unwrap();
                    assert!(length > 0);
                    let last = offset.checked_add(length - 1).unwrap();
                    assert!(last >= commands_end && last < image.len());
                    return slice + last;
                }
            }
            cursor += size;
        }
        panic!("Apple executable fixture has no sealed __TEXT segment");
    }

    #[test]
    fn native_apple_executable_has_trusted_certificate_identity() -> Result<()> {
        let signature = verify_signature_with_policy(APPLE_EXECUTABLE, TrustPolicy::Offline)?;
        assert_trusted_identity(&signature);
        Ok(())
    }

    #[test]
    fn native_adhoc_executable_is_not_a_trusted_publisher() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let path = temporary.path().join("adhoc-executable");
        fs::copy(APPLE_EXECUTABLE, &path)?;
        let signed = codesign(&["--force", "--sign", "-", "--timestamp=none"], &path)?;
        assert!(
            signed.status.success(),
            "ad-hoc fixture signing failed: {}",
            String::from_utf8_lossy(&signed.stderr)
        );
        let integrity = codesign(&["--verify", "--strict", "--all-architectures"], &path)?;
        assert!(
            integrity.status.success(),
            "ad-hoc fixture must have valid native integrity: {}",
            String::from_utf8_lossy(&integrity.stderr)
        );
        let signature = verify_signature_with_policy(path.to_str().unwrap(), TrustPolicy::Offline)?;
        assert_native_rejection(&signature);
        Ok(())
    }

    #[test]
    fn native_tampered_apple_executable_is_rejected() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let path = temporary.path().join("tampered-executable");
        fs::copy(APPLE_EXECUTABLE, &path)?;
        let original = verify_signature_with_policy(path.to_str().unwrap(), TrustPolicy::Offline)?;
        assert_trusted_identity(&original);

        let bytes = fs::read(&path)?;
        let offset = text_segment_last_byte(&bytes);
        {
            let mut file = fs::OpenOptions::new().write(true).open(&path)?;
            file.seek(SeekFrom::Start(offset as u64))?;
            file.write_all(&[bytes[offset] ^ 1])?;
            file.sync_all()?;
        }
        let integrity = codesign(&["--verify", "--strict", "--all-architectures"], &path)?;
        assert!(
            !integrity.status.success(),
            "tampered fixture must fail native integrity verification"
        );
        let signature = verify_signature_with_policy(path.to_str().unwrap(), TrustPolicy::Offline)?;
        assert_native_rejection(&signature);
        Ok(())
    }

    #[test]
    fn native_missing_and_relative_paths_have_unavailable_evidence() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let missing = temporary.path().join("nonexistent-executable");
        for policy in [TrustPolicy::Offline, TrustPolicy::Online] {
            assert!(
                verify_signature_with_policy(missing.to_str().unwrap(), policy).is_err(),
                "a nonexistent executable must not become an unsigned verdict"
            );
            assert!(
                verify_signature_with_policy("usr/bin/true", policy).is_err(),
                "a relative executable must not become an unsigned verdict"
            );
        }
        Ok(())
    }
}
