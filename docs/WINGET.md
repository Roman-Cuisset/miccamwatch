# Windows distribution, Visual C++ runtime and winget

## Why the Windows executables link the CRT statically

`mcw.exe` and `mcw-tray.exe` used to import `VCRUNTIME140.dll`. That DLL is not
part of Windows (only the Universal CRT, `ucrtbase.dll` and the `api-ms-win-crt-*`
API sets are). It ships with the Visual C++ Redistributable, so a clean Windows
image loads neither executable and fails in the loader, before `main()`, with
`0xC0000135 STATUS_DLL_NOT_FOUND`.

That is what winget validation reported for MicCamWatch 0.13.4, and the same
defect was present in the published 0.21.0 assets.

`.cargo/config.toml` now sets, for the Windows x64 MSVC target only:

```toml
[target.x86_64-pc-windows-msvc]
rustflags = ["-C", "target-feature=+crt-static"]
```

The setting alone is not trusted. `installer/tests/windows_distribution_imports.py`
parses the import tables of the executables that are actually built and packaged
and fails the build when a dynamic CRT/C++ runtime DLL or any unknown non-system
DLL appears. CI runs it on the release binaries, and the release workflow runs it
again over the ZIP contents and the MSI payload.

## winget submission strategy

The open PR [#441153](https://github.com/microsoft/winget-pkgs/pull/441153) targets
0.13.4, which is several releases behind and shares the runtime defect. Do not
publish that version; a corrected submission should match a release users should
actually install.

Two honest options:

| Release | Executable runtime | Manifest treatment |
|---|---|---|
| ≤ 0.21.0 (already published) | dynamic `VCRUNTIME140.dll` | must declare `Microsoft.VCRedist.2015+.x64`; never claim a static CRT |
| ≥ 0.21.1 (after this fix) | static CRT, no redistributable needed | no `Dependencies` entry; verified by the import audit |

Do **not** point a new manifest at the published 0.21.0 assets and omit the
dependency: those binaries still need the redistributable.

### After the corrected release is published

1. Confirm the shipped executables are clean:
   `python -B installer/tests/windows_distribution_imports.py <mcw.exe> <mcw-tray.exe>`
2. Generate the manifests with the real version and the real MSI SHA-256
   (from `SHA256SUMS` published with the release):
   `pwsh distribution/winget/Generate-Manifests.ps1 -Version <version> -InstallerSha256 <sha256>`
   Add `-RuntimeDependency` only for a release whose binaries still import
   `VCRUNTIME140.dll`.
3. Validate locally: `winget validate --manifest <generated directory>`.
4. Copy the directory to
   `manifests/r/RomanCuisset/MicCamWatch/<version>/` in a winget-pkgs fork and
   open a pull request. Never edit the GitHub release assets of a published
   version; publish a new version instead.
5. If PR #441153 is still open, close it in favour of the current version — with
   the repository owner's explicit approval.

`ManifestVersion` defaults to `1.28.0`. Raise it when winget-pkgs deprecates that
schema; the generator takes it as a parameter.

## Executable validation behaviour

winget's post-install validation runs the installed executables and reports
`Validation-Executable-Error` when one fails. Two product behaviours matter:

- `mcw.exe --version` prints the version and exits `0`.
- `mcw-tray.exe` is resident: with no arguments it starts the tray and keeps
  running. It now also supports `--version` (and `-V`), which prints
  `mcw-tray <version>` and returns immediately without reading settings, starting
  the observer or opening a window. That gives packagers, installers and support
  scripts a side-effect-free readiness query for the resident component.

`mcw.exe` without arguments prints help and exits `2`; that is the documented CLI
behaviour and is intentionally unchanged. If validation ever flags it, the fix is
to declare the executable argument to the winget maintainers rather than to
change the CLI's exit status.

## Remaining risks

- Windows **N** editions do not include Media Foundation. The executables import
  `mfplat.dll` and `mfsensorgroup.dll` and would fail there; the Media Feature
  Pack is required. This is unrelated to the CRT and is not covered by the
  redistributable dependency.
- A resident tray may be reported as "still running" by tooling that expects a
  process to exit. Use `--version` for a terminating check.
- Static CRT applies to Rust code only. A future C/C++ component built with `/MD`
  would reintroduce a runtime dependency; the CI audit catches it.
