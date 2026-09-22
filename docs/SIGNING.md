# Authenticode release signing

MicCamWatch releases support Authenticode signing of both `mcw.exe` and the MSI. Signing is intentionally performed only in the protected GitHub `release` environment. Private keys must never be committed to this repository.

## GitHub release environment

Configure these environment secrets:

- `WINDOWS_CERTIFICATE_BASE64`: Base64 representation of a password-protected PKCS#12/PFX code-signing certificate.
- `WINDOWS_CERTIFICATE_PASSWORD`: Password for that PFX.

The release workflow writes the certificate only for the duration of each signing step, calls the Windows SDK `signtool` with SHA-256 and DigiCert's RFC 3161 timestamp service, then deletes the temporary PFX. The executable is signed before it is embedded in the MSI; the resulting MSI is signed separately.

If the certificate secret is absent, the workflow deliberately publishes unsigned artifacts. It does not create a test certificate or claim publisher identity. SHA-256 checksums, GitHub artifact attestations, and the SPDX SBOM remain available, but they are not substitutes for Authenticode publisher verification.

## Local verification

```powershell
Get-AuthenticodeSignature .\mcw.exe | Format-List
Get-AuthenticodeSignature .\miccamwatch-windows-x86_64.msi | Format-List
```

A signed production artifact must report `Status: Valid`, the expected publisher subject, and a valid timestamp. Release operators should verify both artifacts before announcing a signed release.
