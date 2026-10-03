//! Native Security.framework verification through the existing embedded helper.
//! Offline disables network work. Online explicitly allows native certificate
//! trust/revocation network checks; it is not a notarization or Gatekeeper claim.

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
