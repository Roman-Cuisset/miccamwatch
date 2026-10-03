# Authenticode release signing

MicCamWatch releases support Authenticode signing of `mcw.exe`, `mcw-tray.exe`, and the MSI. Signing is performed only in the GitHub `release` environment; release operators should protect that environment with appropriate approvals. Private keys must never be committed to this repository.

## GitHub release environment

Configure these environment secrets:

- `WINDOWS_CERTIFICATE_BASE64`: Base64 representation of a password-protected PKCS#12/PFX code-signing certificate.
- `WINDOWS_CERTIFICATE_PASSWORD`: Password for that PFX.

The release workflow writes the certificate only for the duration of each signing step, calls the Windows SDK `signtool` with SHA-256 and DigiCert's RFC 3161 timestamp service, then deletes the temporary PFX. Both executables are signed before they are embedded in the MSI; the resulting MSI is signed separately.

New Windows ZIP/MSI publication is held by default during the Defender investigation. The release workflow requires the repository variable `WINDOWS_RELEASE_APPROVED=true` before building and publishing Windows release assets; ordinary Windows CI still builds, tests and exercises its MSI. Linux/macOS releases can proceed independently. This is a publication hold, not removal of Windows support or authorization to reinstall quarantined bytes.

Signing is a separate choice. If Windows publication is explicitly approved and the certificate secret is absent, the artifacts are unsigned; the workflow does not create a test certificate or claim publisher identity. SHA-256 checksums, GitHub artifact attestations, and the SPDX SBOM remain available, but they are not substitutes for Authenticode publisher verification or an antivirus verdict.

## Local verification

```powershell
Get-AuthenticodeSignature .\mcw.exe | Format-List
Get-AuthenticodeSignature .\mcw-tray.exe | Format-List
Get-AuthenticodeSignature .\miccamwatch-windows-x86_64.msi | Format-List
```

A signed production artifact must report `Status: Valid`, the expected publisher subject, and a valid timestamp. Release operators should verify all three artifacts before announcing a signed release.

## Defender incident and safe handling

The reported `Behavior:Win32/Persistence.A!ml` incident quarantined the installed CLI while its separately installed tray remained running. A missing quarantined executable explains `mcw` no longer being recognized; changing `PATH` does not recover it. The detection is not established as a false positive.

The updater now verifies one exact release package before stopping an owned tray and waits for a safe shutdown acknowledgement and process termination. It uses atomic tray replacement and recoverable same-volume renames for its own mapped CLI, with rollback under the installation lock; the CLI pathname has a brief gap between the two renames. It refuses shutdown while camera operations are pending. An older tray without this acknowledgement must be exited manually after pending operations finish; the updater never force-kills it. Unchanged, disabled, or unmanaged autostart registrations are not recreated by an update. An explicit autostart enable still registers a legitimate per-user startup entry; that behavior and signing do not guarantee an antivirus verdict.

The published v0.14.0 Windows ZIP was independently downloaded and verified against its GitHub API digest, `SHA256SUMS`, and GitHub artifact attestation:

- ZIP SHA-256: `ea8de21155c1eedd624152a4ee7d4100f9d6fb9945234903d064aaf3ce54064a`.
- `mcw.exe` SHA-256: `b4c7976fcb9439d884f59e91086a3d7825b9b82cef3911f57e982d1580d2599b`.
- `mcw-tray.exe` SHA-256: `4b0dd5da067bea0e8e6ddf2d87627c1fb006c6d936250081ce35eb06cb6c5700`.

Both published executables have no PE certificate table. The accessible installed tray matched the published tray hash. The quarantined CLI's original bytes and hash were unavailable, so its provenance cannot be inferred from the surviving tray or from the public archive. Integrity and build provenance are not malware clearance.

Record the detection name, affected path, version, available hash, and Protection History details. Use Microsoft's [software-developer file submission portal](https://www.microsoft.com/en-us/wdsi/filesubmission) for analysis; it requires human verification, and no submission or Microsoft verdict is implied here. Do not disable Defender, add an exclusion, restore quarantined bytes, or automatically reinstall the detected executable. A new signed release requires the real certificate secrets above; no test certificate substitutes for publisher identity.
