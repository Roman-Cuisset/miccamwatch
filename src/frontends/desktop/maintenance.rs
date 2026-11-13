//! Read-only ownership guidance and public release metadata. No updater transaction.
use anyhow::{Context as _, Result, ensure};
use serde_json::{Value, json};
use std::path::Path;

const RELEASES: &str = "https://github.com/Roman-Cuisset/miccamwatch/releases";
pub(super) fn inspect() -> Result<Value> {
    inspect_at(&std::env::current_exe().context("Cannot locate this native owner")?)
}
fn inspect_at(current: &Path) -> Result<Value> {
    let current = current.canonicalize().context("Cannot resolve this executable's provenance")?;
    let path = current.to_str().context("Executable path is not UTF-8")?;
    ensure!(path.len() <= 4096, "Executable path exceeds the explicit consultation limit");
    let mut guidance = None;
    #[cfg(windows)]
    let recognized = crate::uninstall::windows::owner_guidance(&current,
        |message| guidance = Some(message.to_string()))?;
    #[cfg(unix)]
    let recognized = crate::uninstall::unix_owner_guidance(&current,
        |message| guidance = Some(message.to_string()))?;
    ensure!(guidance.as_ref().is_none_or(|text| text.len() <= 4096),
        "Native owner guidance exceeds the consultation limit; no command was run");
    ensure!(recognized == guidance.is_some(), "Native ownership guidance is incomplete");
    Ok(json!({"maintenance": {
        "version":env!("CARGO_PKG_VERSION"), "executable":path,
        "native_owner_guidance":guidance, "owner_guidance_available":recognized,
        "transactions_available":false, "reason":"hybrid_bundle_unqualified", "releases_url":RELEASES,
    }}))
}
pub(super) fn check_release() -> Result<Value> {
    check_using(crate::updater::check_latest_release)
}
fn check_using(fetch: impl FnOnce() -> Result<(String, bool)>) -> Result<Value> {
    let (tag, available) = fetch()?;
    ensure!(tag.starts_with('v') && tag.len() <= 128 && !tag.chars().any(char::is_control),
        "Invalid native public release tag");
    // The platform resolver validates semantic version and exact official origin.
    Ok(json!({"release_check": {
        "current_version":env!("CARGO_PKG_VERSION"), "latest_tag":tag,
        "release_url":format!("{RELEASES}/tag/{tag}"), "update_available":available,
        "checked_at":chrono::Utc::now().to_rfc3339(), "scope":"release_metadata_only",
    }}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cargo_guidance_uses_native_provenance_without_removing_payload_or_metadata() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("bin")).unwrap();
        let metadata = root.path().join(".crates2.json");
        let bytes = br#"{"installs":{"miccamwatch 0.22.0 (fixture)":{"bins":["mcw","mcw.exe"]}}}"#;
        std::fs::write(&metadata, bytes).unwrap();
        let executable = root.path().join("bin").join(if cfg!(windows) { "mcw-tray.exe" } else { "mcw" });
        std::fs::write(&executable, b"inert fixture; never executed").unwrap();
        let mut before: Vec<_> = std::fs::read_dir(root.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
        before.sort();
        let report = inspect_at(&executable).unwrap();
        assert_eq!(report["maintenance"]["owner_guidance_available"], true);
        assert_eq!(report["maintenance"]["transactions_available"], false);
        assert!(report["maintenance"]["native_owner_guidance"].as_str().unwrap().contains("cargo uninstall"));
        assert_eq!(std::fs::read(&metadata).unwrap(), bytes);
        assert_eq!(std::fs::read(&executable).unwrap(), b"inert fixture; never executed");
        let mut after: Vec<_> = std::fs::read_dir(root.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
        after.sort();
        assert_eq!(before, after);
    }
    #[test]
    fn invalid_metadata_and_missing_executable_are_errors_not_unmanaged_success() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("bin")).unwrap();
        let executable = root.path().join("bin").join(if cfg!(windows) { "mcw-tray.exe" } else { "mcw" });
        assert!(inspect_at(&executable).is_err());
        std::fs::write(&executable, b"inert fixture").unwrap();
        std::fs::write(root.path().join(".crates2.json"), b"not valid JSON").unwrap();
        assert!(inspect_at(&executable).is_err());
        assert_eq!(std::fs::read(&executable).unwrap(), b"inert fixture");
    }
    #[test]
    fn public_metadata_has_no_installation_authority_and_network_failure_is_not_success() {
        let value = check_using(|| Ok(("v0.21.0".into(), false))).unwrap();
        assert_eq!(value["release_check"]["latest_tag"], "v0.21.0");
        assert_eq!(value["release_check"]["update_available"], false);
        assert_eq!(value["release_check"]["scope"], "release_metadata_only");
        assert!(check_using(|| Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied,"fixture network denied").into())).is_err());
        for tag in ["", "0.21.0", "v0.21.0\nother"] {
            assert!(check_using(|| Ok((tag.into(), true))).is_err());
        }
    }
    #[test]
    fn hybrid_queries_cannot_accept_an_install_or_uninstall_instruction() {
        for action in ["update", "uninstall", "install", "check_updates;uninstall"] {
            assert!(serde_json::from_value::<super::super::tools::Request>(json!({"action":action})).is_err());
        }
        assert!(serde_json::from_value::<super::super::tools::Request>(json!({"action":"check_updates","command":"uninstall"})).is_err());
    }
}
