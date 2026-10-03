use crate::{
    config::Policy,
    model::{EnforcementDecision, Evidence, EvidenceKind, Risk, SignatureInfo},
};

#[cfg(windows)]
use crate::config::TrustPolicy;
/// Ordinal comparison, not Unicode case folding or path canonicalization. Keep
/// Windows consent attribution and publisher matching on the same comparator.
#[cfg(windows)]
pub(crate) fn same_windows_path(left: &str, right: &str) -> bool {
    if left.is_ascii() && right.is_ascii() {
        return left.eq_ignore_ascii_case(right);
    }
    let left = left.encode_utf16().collect::<Vec<_>>();
    let right = right.encode_utf16().collect::<Vec<_>>();
    unsafe {
        windows::Win32::Globalization::CompareStringOrdinal(&left, &right, true)
            == windows::Win32::Globalization::CSTR_EQUAL
    }
}

/// Lexical component boundary only; neither aliases nor symlinks establish
/// process identity. Callers must supply an independently validated executable.
#[cfg(windows)]
pub(crate) fn path_matches_policy_prefix(path: &str, prefix: &str) -> bool {
    let Some(head) = path.get(..prefix.len()) else {
        return false;
    };
    same_windows_path(head, prefix)
        && (path.len() == prefix.len()
            || prefix.ends_with('\\')
            || path.as_bytes()[prefix.len()] == b'\\')
}

#[cfg(not(windows))]
pub(crate) fn path_matches_policy_prefix(path: &str, prefix: &str) -> bool {
    use std::path::{Component, Path};
    let path = Path::new(path);
    let prefix = Path::new(prefix);
    path.is_absolute()
        && prefix.is_absolute()
        && !path.components().any(|part| part == Component::ParentDir)
        && !prefix.components().any(|part| part == Component::ParentDir)
        && path.starts_with(prefix)
}

/// Deliberately no pathname/mtime cache: replacement can preserve timestamps.
#[cfg(windows)]
pub(crate) fn signature_for_path(
    path: &str,
    trust_policy: TrustPolicy,
    verify: impl FnOnce(&str, TrustPolicy) -> SignatureInfo,
) -> SignatureInfo {
    verify(path, trust_policy)
}

#[cfg(windows)]
fn publisher_matches(actual: &str, expected: &str) -> bool {
    same_windows_path(actual.trim(), expected.trim())
}

#[cfg(not(windows))]
fn publisher_matches(actual: &str, expected: &str) -> bool {
    actual.trim() == expected.trim()
}

/// Alternatives within a group are OR; path and publisher groups are AND.
/// None denotes unavailable signature evidence, not a negative verification.
/// A known mismatch cannot authorize enforcement while another required group
/// is unknown. Risk is not itself an enforcement decision.
pub(crate) fn assess_policy(
    policy: &Policy,
    application: &str,
    executable: Option<&str>,
    signature: Option<&SignatureInfo>,
    evidence: &mut Vec<Evidence>,
) -> (Risk, EnforcementDecision) {
    #[cfg(not(windows))]
    {
        use std::path::{Component, Path};
        // Display names and portal/client claims are not process identities.
        // Even a name-only rule must never grant trust to an unknown owner.
        let valid_identity = executable.is_some_and(|path| {
            let path = Path::new(path);
            path.is_absolute()
                && !path.components().any(|part| part == Component::ParentDir)
                && path.file_name().is_some_and(|name| name == application)
        });
        if !valid_identity {
            evidence.push(Evidence::new(
                EvidenceKind::ApplicationProfile,
                "policy",
                "A validated executable identity is unavailable; an application name alone cannot authorize policy enforcement.",
            ));
            return (Risk::Unexplained, EnforcementDecision::Unknown);
        }
    }
    #[cfg(windows)]
    let rule = policy.application(application);
    #[cfg(not(windows))]
    let rule = policy
        .applications
        .iter()
        .find(|rule| rule.executable == application);
    let Some(rule) = rule else {
        return (Risk::Expected, EnforcementDecision::Alert);
    };
    let publisher = signature.and_then(|value| value.signer.as_deref());
    let publisher_ok = rule.publishers.is_empty()
        || signature.is_some_and(|value| value.verified)
            && publisher.is_some_and(|signer| {
                rule.publishers
                    .iter()
                    .any(|expected| publisher_matches(signer, expected))
            });
    let path_ok = rule.paths.is_empty()
        || executable.is_some_and(|path| {
            rule.paths
                .iter()
                .any(|expected| path_matches_policy_prefix(path, expected))
        });
    let evidence_complete = (rule.publishers.is_empty()
        || signature.is_some_and(|value| !value.verified || value.signer.is_some()))
        && (rule.paths.is_empty() || executable.is_some());

    if !evidence_complete {
        evidence.push(Evidence::new(
            EvidenceKind::ApplicationProfile,
            "policy",
            "Policy rule matched the executable name, but required identity evidence is unavailable.",
        ));
        return (Risk::Unexplained, EnforcementDecision::Unknown);
    }
    if publisher_ok && path_ok {
        evidence.push(Evidence::new(
            EvidenceKind::ApplicationProfile,
            "policy",
            "Application matches its configured policy rule.",
        ));
        return (Risk::Expected, EnforcementDecision::Allow);
    }
    if !publisher_ok {
        evidence.push(Evidence::new(
            EvidenceKind::Signature,
            "policy",
            format!(
                "Publisher mismatch: expected one of {:?}, got {:?}.",
                rule.publishers, publisher
            ),
        ));
    }
    if !path_ok {
        evidence.push(Evidence::new(
            EvidenceKind::FileLocation,
            "policy",
            format!(
                "Path mismatch: expected a prefix in {:?}, executable at {:?}.",
                rule.paths, executable
            ),
        ));
    }
    (Risk::Suspicious, EnforcementDecision::Deny)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ApplicationRule;

    #[cfg(windows)]
    const ROOT: &str = r"C:\Capture";
    #[cfg(not(windows))]
    const ROOT: &str = "/opt/capture";
    #[cfg(windows)]
    const PATH: &str = r"C:\Capture\capture";
    #[cfg(not(windows))]
    const PATH: &str = "/opt/capture/capture";
    #[cfg(windows)]
    const OTHER: &str = r"C:\Other\capture";
    #[cfg(not(windows))]
    const OTHER: &str = "/other/capture";

    fn policy() -> Policy {
        Policy {
            applications: vec![ApplicationRule {
                executable: "capture".to_owned(),
                paths: vec![ROOT.to_owned()],
                publishers: vec!["Publisher".to_owned()],
            }],
            ..Policy::default()
        }
    }

    fn signature(verified: bool, signer: Option<&str>) -> SignatureInfo {
        SignatureInfo {
            verified,
            signer: signer.map(str::to_owned),
            error: (!verified).then(|| "Invalid signature".to_owned()),
        }
    }

    fn assess(
        policy: &Policy,
        path: Option<&str>,
        sig: Option<&SignatureInfo>,
    ) -> (Risk, EnforcementDecision) {
        assess_policy(policy, "capture", path, sig, &mut Vec::new())
    }

    #[test]
    fn unavailable_required_evidence_wins_over_a_known_mismatch() {
        assert_eq!(
            assess(&policy(), Some(OTHER), None),
            (Risk::Unexplained, EnforcementDecision::Unknown)
        );
        assert_eq!(
            assess(&policy(), None, Some(&signature(false, None))),
            (Risk::Unexplained, EnforcementDecision::Unknown)
        );
        assert_eq!(
            assess(&policy(), Some(OTHER), Some(&signature(true, None))),
            (Risk::Unexplained, EnforcementDecision::Unknown)
        );
    }

    #[test]
    fn alternatives_are_or_but_identity_groups_are_and() {
        let mut rules = policy();
        rules.applications[0].paths.insert(0, OTHER.to_owned());
        rules.applications[0]
            .publishers
            .insert(0, "Other Publisher".to_owned());
        assert_eq!(
            assess(
                &rules,
                Some(PATH),
                Some(&signature(true, Some("Publisher")))
            ),
            (Risk::Expected, EnforcementDecision::Allow)
        );
        for sig in [
            signature(false, Some("Publisher")),
            signature(true, Some("Publisher Evil")),
        ] {
            assert_eq!(
                assess(&rules, Some(PATH), Some(&sig)),
                (Risk::Suspicious, EnforcementDecision::Deny)
            );
        }
        let policy = policy();
        assert_eq!(
            assess(
                &policy,
                Some(OTHER),
                Some(&signature(true, Some("Publisher")))
            ),
            (Risk::Suspicious, EnforcementDecision::Deny)
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn unix_paths_and_publishers_are_case_sensitive_and_component_bounded() {
        assert!(path_matches_policy_prefix(PATH, ROOT));
        assert!(!path_matches_policy_prefix(
            "/opt/captureEvil/capture",
            ROOT
        ));
        assert!(!path_matches_policy_prefix("/opt/Capture/capture", ROOT));
        assert!(!path_matches_policy_prefix(
            "/opt/capture/../evil/capture",
            ROOT
        ));
        assert!(!path_matches_policy_prefix(
            "opt/capture/capture",
            "opt/capture"
        ));
        assert!(!publisher_matches("publisher", "Publisher"));
        assert!(publisher_matches(" Publisher ", "Publisher"));
    }

    #[cfg(windows)]
    #[test]
    fn windows_paths_preserve_ordinal_case_and_backslash_boundary() {
        assert!(path_matches_policy_prefix(
            r"C:\Capture\capture",
            r"c:\capture"
        ));
        assert!(!path_matches_policy_prefix(
            r"C:\CaptureEvil\capture",
            r"C:\Capture"
        ));
        assert!(same_windows_path("Ä", "ä"));
        assert!(!same_windows_path("ß", "SS"));
    }

    #[cfg(not(windows))]
    #[test]
    fn claimed_or_missing_process_names_never_allow_or_deny() {
        let mut policy = policy();
        policy.applications[0].paths.clear();
        policy.applications[0].publishers.clear();
        assert_eq!(assess(&policy, None, None).1, EnforcementDecision::Unknown);
        assert_eq!(
            assess(&policy, Some("/opt/capture/other"), None).1,
            EnforcementDecision::Unknown
        );
        assert_eq!(
            assess(&policy, Some(PATH), None).1,
            EnforcementDecision::Allow
        );
        assert_eq!(
            assess_policy(
                &policy,
                "CAPTURE",
                Some("/opt/capture/CAPTURE"),
                None,
                &mut Vec::new()
            )
            .1,
            EnforcementDecision::Alert
        );
    }
}
