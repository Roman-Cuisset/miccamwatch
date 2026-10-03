import Darwin
import Foundation
import Security

// Only certificate-backed native signing identities are publishers. Code
// integrity alone, an ad-hoc signature, a bundle ID or Team ID is not enough.
// Public contract: Apple Security.framework SecStaticCode.h / SecCode.h /
// CSCommon.h and SecCSFlags Swift imports. "anchor trusted" uses actual
// system/user code-signing trust. No Gatekeeper/notarization claim.
struct MacSignatureReply: Encodable {
    var available: Bool
    var verified: Bool
    var signer: String?
    var error: String?
}

private struct SignatureFileStamp: Equatable {
    let device: Int32
    let inode: UInt64
    let size: Int64
    let modifiedSeconds: Int
    let modifiedNanoseconds: Int
    let changedSeconds: Int
    let changedNanoseconds: Int

    init?(_ value: stat) {
        guard value.st_mode & mode_t(S_IFMT) == mode_t(S_IFREG) else { return nil }
        device = value.st_dev
        inode = value.st_ino
        size = value.st_size
        modifiedSeconds = value.st_mtimespec.tv_sec
        modifiedNanoseconds = value.st_mtimespec.tv_nsec
        changedSeconds = value.st_ctimespec.tv_sec
        changedNanoseconds = value.st_ctimespec.tv_nsec
    }
}

private func signatureDescriptorStamp(_ descriptor: Int32) -> SignatureFileStamp? {
    var value = stat()
    guard fstat(descriptor, &value) == 0 else { return nil }
    return SignatureFileStamp(value)
}

private func signaturePathStamp(_ path: String) -> SignatureFileStamp? {
    var value = stat()
    guard stat(path, &value) == 0 else { return nil }
    return SignatureFileStamp(value)
}

private func signatureUnavailable(_ message: String) -> MacSignatureReply {
    MacSignatureReply(available: false, verified: false, error: message)
}

private func signatureStatusMessage(_ status: OSStatus) -> String {
    let description = SecCopyErrorMessageString(status, nil) as String? ?? "Security.framework failure"
    return "\(description) (OSStatus \(status))"
}

// An explicit allowlist separates completed negative verification from OS/API,
// file-permission, unsupported-format, trust-service and network failures. A
// generic error must never authorize enforcement as if it proved forgery.
private func signatureIsNegative(_ status: OSStatus) -> Bool {
    switch status {
    case errSecCSSignatureFailed, errSecCSResourcesNotSealed,
         errSecCSResourcesNotFound, errSecCSResourcesInvalid,
         errSecCSBadResource, errSecCSResourceRulesInvalid,
         errSecCSReqFailed, errSecCSInfoPlistFailed,
         errSecCSResourceDirectoryFailed, errSecCSUnsignedNestedCode,
         errSecCSBadNestedCode, errSecCSBadMainExecutable,
         errSecCSBadFrameworkVersion, errSecCSBadTeamIdentifier,
         errSecCSSignatureUntrusted, errSecCSRevokedNotarization,
         errSecCertificateExpired, errSecCertificateRevoked,
         errSecNotTrusted:
        return true
    default:
        return false
    }
}

func nativeSignature(path: String, online: Bool) -> MacSignatureReply {
    guard path.hasPrefix("/"), !path.utf8.contains(0) else {
        return signatureUnavailable("Signature verification requires an absolute executable path")
    }
    let descriptor = open(path, O_RDONLY | O_CLOEXEC)
    guard descriptor >= 0 else {
        return signatureUnavailable("Executable cannot be opened for native signature verification")
    }
    defer { close(descriptor) }
    guard let before = signatureDescriptorStamp(descriptor),
          signaturePathStamp(path) == before else {
        return signatureUnavailable("A stable regular executable file is unavailable")
    }
    // Each call constructs a fresh static code object. No pathname/mtime cache.
    var code: SecStaticCode?
    let creation = SecStaticCodeCreateWithPath(URL(fileURLWithPath: path) as CFURL,
                                             SecCSFlags(rawValue: 0), &code)
    guard creation == errSecSuccess, let code else {
        return signatureUnavailable(signatureStatusMessage(creation))
    }
    // Require actual code-signing trust, not just a chain ending in any
    // self-signed certificate. Apple's public requirement language consults
    // system/user trust and honors explicit distrust closer to the leaf.
    // The globally declared .checkTrustedAnchors option is not accepted by
    // SecStaticCodeCheckValidity's flag mask; the requirement supplies this
    // trust check without an invalid flag or a second, different trust policy.
    var requirement: SecRequirement?
    let requirementStatus = SecRequirementCreateWithString("anchor trusted" as CFString,
                                                          SecCSFlags(rawValue: 0), &requirement)
    guard requirementStatus == errSecSuccess, let requirement else {
        return signatureUnavailable(signatureStatusMessage(requirementStatus))
    }
    var flags = SecCSFlags(rawValue:
        kSecCSCheckAllArchitectures | kSecCSStrictValidate | kSecCSCheckNestedCode)
    // CSCommon.h's typed CF_OPTIONS constants import as SecCSFlags members,
    // unlike the call-specific UInt32 constants declared by SecStaticCode.h.
    if online {
        flags.insert(SecCSFlags(rawValue: kSecCSAllowNetworkAccess))
        flags.insert(.enforceRevocationChecks)
        // Force native OCSP/CRL checks regardless of preferences. Apple's
        // policy is best-attempt, not proof of a fresh positive server response.
    } else {
        // Forbid validation work requiring the network, including revocation
        // and notarization requests. Offline success is not fresh revocation
        // evidence.
        flags.insert(.noNetworkAccess)
    }
    // Validate before requesting the certificate chain: signing-information
    // lookup can itself trigger certificate validation. This fresh code object
    // must receive the explicit network policy before any such lookup.
    let status = SecStaticCodeCheckValidity(code, flags, requirement)
    guard signatureDescriptorStamp(descriptor) == before,
          signaturePathStamp(path) == before else {
        return signatureUnavailable("Executable changed during native verification; signing evidence unavailable")
    }
    guard status == errSecSuccess else {
        if signatureIsNegative(status) {
            return MacSignatureReply(available: true, verified: false,
                                     error: signatureStatusMessage(status))
        }
        return signatureUnavailable(signatureStatusMessage(status))
    }
    var information: CFDictionary?
    let informationStatus = SecCodeCopySigningInformation(code,
        SecCSFlags(rawValue: kSecCSSigningInformation), &information)
    guard informationStatus == errSecSuccess else {
        guard signatureDescriptorStamp(descriptor) == before,
              signaturePathStamp(path) == before else {
            return signatureUnavailable("Executable changed while reading native signing information")
        }
        if signatureIsNegative(informationStatus) {
            return MacSignatureReply(available: true, verified: false,
                                     error: signatureStatusMessage(informationStatus))
        }
        return signatureUnavailable(signatureStatusMessage(informationStatus))
    }
    guard let values = information as? [String: Any] else {
        return signatureUnavailable("Native signing information dictionary unavailable")
    }
    guard let certificates = values[kSecCodeInfoCertificates as String] as? [SecCertificate],
          let certificate = certificates.first else {
        return signatureUnavailable("No certificate-backed signing identity (unsigned or ad-hoc code)")
    }
    var name: CFString?
    let nameStatus = SecCertificateCopyCommonName(certificate, &name)
    guard nameStatus == errSecSuccess, let name,
          !(name as String).trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
        return signatureUnavailable("Validated signing certificate has no usable publisher identity")
    }
    guard signatureDescriptorStamp(descriptor) == before,
          signaturePathStamp(path) == before else {
        return signatureUnavailable("Executable changed while reading the validated publisher identity")
    }
    return MacSignatureReply(available: true, verified: true, signer: name as String)
}
