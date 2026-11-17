//! Whole-payload identity for the packaged desktop. The package owner's signature
//! or archive digest authenticates the manifest. Per-file digests catch partial
//! replacements and corruption before the child can receive the IPC challenge.
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs::File, io::Read};

pub(crate) const MANIFEST: &str = "miccamwatch-bundle.json";
const LIMIT: u64 = 16 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema: u8,
    version: String,
    ipc: u8,
    target: String,
    revision: String,
    files: BTreeMap<String, String>,
}

pub(crate) fn verify(
    manifest: impl Read,
    open: impl FnMut(&str) -> Result<File>,
) -> Result<Vec<String>> {
    verify_version(manifest, open, env!("CARGO_PKG_VERSION"))
}

pub(crate) fn verify_version(
    mut manifest: impl Read,
    mut open: impl FnMut(&str) -> Result<File>,
    version: &str,
) -> Result<Vec<String>> {
    let mut bytes = Vec::new();
    manifest.by_ref().take(LIMIT + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= LIMIT,
        "hybrid bundle manifest is oversized"
    );
    let manifest: Manifest =
        serde_json::from_slice(&bytes).context("invalid hybrid bundle manifest")?;
    #[cfg(windows)]
    let target = "windows-x86_64";
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    let target = "linux-x86_64";
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    let target = "linux-aarch64";
    ensure!(
        manifest.schema == 1
            && manifest.target == target
            && manifest.version == version
            && manifest.ipc == super::PROTOCOL_VERSION
            && manifest.revision.len() == 40
            && manifest
                .revision
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()),
        "desktop bundle version, architecture or IPC mismatch; reinstall the complete package"
    );
    let contract: BTreeMap<String, Vec<String>> =
        serde_json::from_str(include_str!("../../../installer/hybrid_layout.json"))?;
    let expected = &contract[target];
    ensure!(
        manifest.files.len() == expected.len()
            && expected
                .iter()
                .all(|name| manifest.files.contains_key(name)),
        "incomplete or unexpected desktop bundle; reinstall the complete package"
    );
    for name in expected {
        let mut file = open(name)
            .with_context(|| format!("desktop bundle component {name} is unavailable"))?;
        let mut hash = Sha256::new();
        let mut buffer = [0; 8192];
        loop {
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            hash.update(&buffer[..count]);
        }
        ensure!(
            hex(&hash.finalize()) == manifest.files[name],
            "desktop bundle component {name} failed integrity verification; reinstall the complete package"
        );
    }
    Ok(expected.clone())
}

fn hex(digest: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut result = String::with_capacity(2 * digest.len());
    for byte in digest {
        write!(result, "{byte:02x}").expect("writing to String cannot fail");
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn manifest_admits_the_exact_payload_and_rejects_corruption_or_mixed_versions() {
        #[cfg(windows)]
        let target = "windows-x86_64";
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        let target = "linux-x86_64";
        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
        let target = "linux-aarch64";
        let temp = tempfile::tempdir().unwrap();
        let contract: BTreeMap<String, Vec<String>> =
            serde_json::from_str(include_str!("../../../installer/hybrid_layout.json")).unwrap();
        let expected = &contract[target];
        let known_good = b"same build";
        let files: BTreeMap<_, _> = expected
            .iter()
            .map(|name| {
                let path = temp.path().join(name);
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(path, known_good).unwrap();
                (name.clone(), hex(&Sha256::digest(known_good)))
            })
            .collect();
        let mut manifest = json!({
            "schema": 1, "version": env!("CARGO_PKG_VERSION"),
            "ipc": crate::frontends::desktop::PROTOCOL_VERSION, "target": target,
            "revision": "123456789abcdef0123456789abcdef012345678",
            "files": files,
        });
        let file = temp.path().join(MANIFEST);
        let check = |value: &serde_json::Value| {
            std::fs::write(&file, serde_json::to_vec(value).unwrap()).unwrap();
            verify(File::open(&file).unwrap(), |name| {
                Ok(File::open(temp.path().join(name))?)
            })
        };
        assert_eq!(check(&manifest).unwrap(), *expected);
        let component = temp.path().join(&expected[0]);
        std::fs::write(&component, b"modified").unwrap();
        assert!(
            check(&manifest)
                .unwrap_err()
                .to_string()
                .contains("integrity")
        );
        std::fs::remove_file(component).unwrap();
        assert!(
            check(&manifest)
                .unwrap_err()
                .to_string()
                .contains("unavailable")
        );
        manifest["ipc"] = json!(9);
        assert!(
            check(&manifest)
                .unwrap_err()
                .to_string()
                .contains("IPC mismatch")
        );
    }
}
