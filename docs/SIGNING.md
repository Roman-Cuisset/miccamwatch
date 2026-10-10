# Authenticode release signing

MicCamWatch releases support Authenticode signing of `mcw.exe`, `mcw-tray.exe`, and the MSI. Signing is performed only in the GitHub `release` environment; release operators should protect that environment with appropriate approvals. Private keys must never be committed to this repository.

## GitHub release environment

Configure these environment secrets:

- `WINDOWS_CERTIFICATE_BASE64`: Base64 representation of a password-protected PKCS#12/PFX code-signing certificate.
- `WINDOWS_CERTIFICATE_PASSWORD`: Password for that PFX.

The release workflow writes the certificate only for the duration of each signing step, calls the Windows SDK `signtool` with SHA-256 and DigiCert's RFC 3161 timestamp service, then deletes the temporary PFX. Both executables are signed before they are embedded in the MSI; the resulting MSI is signed separately.

Windows ZIP/MSI publication requires explicit `WINDOWS_RELEASE_APPROVED=true`; ordinary Windows CI builds, tests and exercises its MSI independently. The owner approved the complete coordinated `v0.16.0` release on 2026-10-06 and enabled this gate. Windows, Linux and both macOS architectures are now published together as stable `latest`.

If the gate is held again, Unix-only releases remain **prereleases**, never stable `latest` without Windows packages. The historical `v0.15.1` Unix prerelease still requires explicit `--version v0.15.1`; its assets, old `v0.14.0` assets and the unshipped `v0.15.0` tag remain immutable. Publication approval never authorizes protection bypass or restoration of quarantined bytes.

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

## Published stable Windows v0.16.0 verification

The [release run 37437092551](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37437092551)
built and exercised the ZIP CLI/tray and native per-user MSI installation/removal
from `9515fb664ac0c75601ebf6b8321b6954666faec9`, tag `v0.16.0`.
No certificate was configured; local `Get-AuthenticodeSignature` independently
reported `NotSigned` and no signer for both downloaded EXEs and the MSI.

- Public ZIP SHA-256: `2a2c6205261c6d678ba46c8e8a126e4b926208a602af5c7403f3ebeda32e3c24`.
- Public MSI SHA-256: `48dbadcaabab6664faa2ab289d5884dc102aea3e68bf83708185cd5d73117f7c`.
- CLI SHA-256: `9539f1d823526a0fc4d4522db4f09505fec4dbb699615475f2fe3d3e03ad3c3a`.
- Tray SHA-256: `3cfc152aa9423653b5b07e57361267843e62dc5a685ac1f11cacce48de465428`.

Both release packages matched public API digests, exact `SHA256SUMS` entries
and GitHub attestations constrained to the release workflow, source commit,
tag ref and hosted runner. A real `0.15.1` portable pair, copied to a private
prefix, ran `mcw update` against public stable latest: both replacements matched
the downloaded ZIP exactly and reported `0.16.0`; a second call was up-to-date.
Existing HKCU startup values were unchanged. Separately, the user's installed
pair was observed to have changed during verification: CLI `--version` reported
`0.16.0` and both installed hashes matched the public pair above. The origin of
that concurrent update was not observed; it is not attributed to the private
smoke. No user-installed files were restored/replaced by cleanup.
The old mapped CLI's backup was retained with the documented access-denied
cleanup warning until that updater exited; only private smoke files were removed.

Defender AM/AV, real-time protection and behavior monitoring were active,
signature `1.459.568.0`. An actual custom scan of the public payload/smoke
directory completed with matching events 1000/1001, scan ID
`C8DEF81B-4EC1-4193-B0D1-EFE18EE36DFD`; no 1116/1117 detection/remediation events
occurred from publication to this check. This is local observed evidence,
not Authenticode identity, a Microsoft submission/verdict or universal clearance.

## Published stable Windows v0.16.1 verification

The [release run 37493346742](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37493346742)
built and exercised the extracted ZIP CLI/tray and native per-user MSI
installation/removal from `7b134e3500860bd185ddc3955107b8082d414e78`,
immutable tag `v0.16.1`. Windows publication remained explicitly approved.
The complete seven-asset release was stable/latest when this verification ran.

- Public ZIP SHA-256: `276f0719b595e30b171ab1010e418de7bc4defcfc05e87b7e048a11406981e70`.
- Public MSI SHA-256: `0d9aad13942538c00f5735e8366bc0dd215b817feb964644b062bf404f0aba71`.

All seven downloaded assets matched API digests; the six payload/SBOM files
also matched the manifest and authenticated GitHub attestations constrained
to the release workflow, exact source commit, tag ref and hosted runners.
A genuine public v0.16.0 portable pair in a private prefix ran `mcw update`
against stable latest: both replaced EXEs matched the v0.16.1 public ZIP.
A second call was up-to-date and preserved both hashes.

`Get-AuthenticodeSignature` on these downloaded EXEs and MSI reported
`NotSigned`, with no signer. No new Defender scan or Microsoft verdict is
claimed for v0.16.1; the historical v0.16.0 observations above are not
transferable clearance. No protection bypass was used.

## Published stable Windows v0.17.0 verification

The [release workflow](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38058344725)
passed Windows ZIP/MSI and Linux/macOS ARM/Intel native package smoke. Annotated
tag `v0.17.0` points to `d1b8557f827e102df20c33dbac4ec99badba7d1a`; API tag/latest
agree on a stable, non-draft seven-asset release. All downloaded bytes matched
API SHA-256 digests; all six payload/SBOM entries matched `SHA256SUMS` and GitHub
attestations constrained to the release workflow, exact tag/source and hosted
runners.

- Public ZIP SHA-256: `6dda0fc0b35744e15a1a76b39e43c8264bba81c13446f31dfe7d08d18c9b8e36`.
- Public MSI SHA-256: `a0b469654b8b1a1f3dcc0cec7fd9088b92d86f11926256096a028350398484ed`.

A genuine private public `0.16.1` CLI/tray pair updated through its actual
`mcw update` to `0.17.0`. Both EXEs matched the verified ZIP; preferences survived,
and a second update retained hashes and mtimes. An unrelated existing user tray
was retained by exact process handle and remained alive. No user installation,
microphone, camera or capture permission was changed by this migration smoke.
Published read-only status returned schema 3/version 0.17.0 and normal camera
Allow with no pending/absent/unknown entries.

Actual `Get-AuthenticodeSignature` on the downloaded public EXEs/MSI reported
`NotSigned`. No new Defender scan, Microsoft clearance or Apple Developer ID/
notarization is claimed. Provenance is not antivirus clearance; no security
protection was bypassed.

