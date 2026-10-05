# Authenticode release signing

MicCamWatch releases support Authenticode signing of `mcw.exe`, `mcw-tray.exe`, and the MSI. Signing is performed only in the GitHub `release` environment; release operators should protect that environment with appropriate approvals. Private keys must never be committed to this repository.

## GitHub release environment

Configure these environment secrets:

- `WINDOWS_CERTIFICATE_BASE64`: Base64 representation of a password-protected PKCS#12/PFX code-signing certificate.
- `WINDOWS_CERTIFICATE_PASSWORD`: Password for that PFX.

The release workflow writes the certificate only for the duration of each signing step, calls the Windows SDK `signtool` with SHA-256 and DigiCert's RFC 3161 timestamp service, then deletes the temporary PFX. Both executables are signed before they are embedded in the MSI; the resulting MSI is signed separately.

New Windows ZIP/MSI publication is held by default during the Defender investigation. The release workflow requires the repository variable `WINDOWS_RELEASE_APPROVED=true` before building and publishing Windows release assets; ordinary Windows CI still builds, tests and exercises its MSI. Linux/macOS releases can proceed independently. This is a publication hold, not removal of Windows support or authorization to reinstall quarantined bytes.

While this hold is active, the Unix-only release is a **prerelease**, not GitHub's
stable `latest` release. The approved `v0.15.1` channel requires explicit
`--version v0.15.1` installation. Stable `v0.14.0` and its existing Windows assets
remain unchanged, so legacy Windows installers/updaters querying
`/releases/latest` are not redirected to a release without Windows packages.

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

## Approved local Windows v0.15.1 replacement

On 2026-10-05, the user approved replacing the local installation without publishing new Windows release assets. The paired CLI/tray were compiled with the native MSVC toolchain and locked dependencies from the immutable published source commit `7d1e3dc17a6171b521a2f53b3cfebe663425ceb0`. The running v0.14.0 tray matched its public hash and exited through its normal **Exit** menu action after the camera-operation mutex was available; no process was force-terminated and no quarantined CLI was restored.

Cargo installed both executables under `%USERPROFILE%\.cargo\bin` while the existing MicCamWatch installation lock was held. Preexisting configuration/data bytes were unchanged during installation. The previously enabled per-user autostart registration retained the same command; no new autostart enable action was performed.

Installed executable SHA-256:

- `mcw.exe`: `7274d0e1c98f1617bfff8d1cc85319f2efae9c125b35a5ec298e9c5d87cea805`.
- `mcw-tray.exe`: `d78ccade67c8f1d5cc97e65e15932585f422e4bdcc29b1a24236036ccf589371`.

An actual fresh PowerShell resolved the installed CLI and reported `mcw 0.15.1`. Native `doctor`, schema-3 `status`, populated TUI, tray menu version/actions, cooperative tray stop/restart, and the updater's matching-pair check were exercised. `status` returned the documented activity code `1` with five healthy collectors; it was not a runtime failure. The TUI exited normally with code `0`.

Defender's service, antivirus, real-time protection and behavior monitor remained enabled, with signature version `1.459.557.0`. Separate custom scans of the two installed files have paired start/completion events: CLI scan `{A52EA338-74B2-4EF0-B5B8-C43BBF532BA0}` and tray scan `{86EA13AD-D77E-49F2-BB79-A3FEE50C734A}`. No detection/remediation event was returned in that scan interval, and both hashes were unchanged after scans and actual runtime checks. No Defender preferences, exclusions or quarantine contents were changed. Exclusion visibility required administrator access and was not established.

These are unsigned, locally built binaries, not a new public Windows release or a Microsoft malware verdict. Runtime autostart-registration changes and an actual downloaded Windows upgrade were not exercised in this local replacement; neither broader Defender acceptance nor elimination of the earlier detection is implied.
