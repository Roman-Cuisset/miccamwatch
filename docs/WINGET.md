# Windows distribution, Visual C++ runtime and winget

## Why the Windows executables link the CRT statically

The inspected published **0.13.4 and 0.21.0** `mcw.exe` and `mcw-tray.exe`
import `VCRUNTIME140.dll`. This DLL is supplied by the Visual C++ Redistributable,
not the Windows inbox Universal CRT. Its absence prevents process startup with
`0xC0000135 STATUS_DLL_NOT_FOUND`, before `main()`.

The winget maintainer reported that status for0.13.4. The imports and a controlled
missing-DLL reproduction support this explanation; no loader trace from the
maintainer's machine identifies the exact missing module. Do not generalize this
inspection to every historical or future release.

`.cargo/config.toml` now sets, for the Windows x64 MSVC target only:

```toml
[target.x86_64-pc-windows-msvc]
rustflags = ["-C", "target-feature=+crt-static"]
```

The setting alone is not trusted. `installer/tests/windows_distribution_imports.py`
examines real PE ordinary and delay imports, rejects unexpected CRT families and
unreviewed dependencies, and checks ZIP/MSI executables against the exact build bytes.
The OS baseline is explicit: a DLL's presence in a developer's System32 proves nothing.
The native Windows CI and release package workflow run this gate.

Flutter is a **separate C++ build**: its B4 runner still imported `MSVCP140.dll`,
`VCRUNTIME140.dll` and `VCRUNTIME140_1.dll`. Its Windows CMake runtime is now static
as well; the official Flutter engine is unchanged. Use `--flutter-bundle <Release>`
to audit the runner, engine and any other PE in the actual bundle, not just Rust.
The Dart AOT `data/app.so` is ELF, not a PE DLL import table.

## winget submission strategy

The open PR [#441153](https://github.com/microsoft/winget-pkgs/pull/441153) targets
0.13.4, which is several releases behind and shares the runtime defect. Do not
publish that version; a corrected submission should match a release users should
actually install.

Two honest options:

| Release | Executable runtime | Manifest treatment |
|---|---|---|
| Published0.13.4 /0.21.0, inspected | dynamic `VCRUNTIME140.dll` | needs an appropriate runtime dependency or a new corrected release; never claim these assets were rebuilt |
| A future corrected release | determined from its actual complete payload | omit the redistributable only after all shipped components pass the import gate and applicable runtime checks |

Do **not** point a new manifest at the published 0.21.0 assets and omit the
dependency: those binaries still need the redistributable.

### After the corrected release is published

1. Audit the actual released Rust binaries, ZIP and MSI, and the Flutter bundle
   if distributed. Retain hashes and runtime evidence for that exact commit.
2. Generate manifests using only the **published** version and real MSI SHA-256:
   `pwsh distribution/winget/Generate-Manifests.ps1 -Version <version> -InstallerSha256 <sha256>`.
   The generator formats supplied metadata; it does not prove publication or hash authenticity.
   For a payload that still needs VC++, supply `-RuntimeDependency` **and**
   `-RuntimeMinimumVersion <verified-package-version>`. No universal14.0 floor is asserted;
   justify a supported runtime at least as recent as the relevant build toolset.
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

`mcw.exe` without arguments prints help and exits `2`; that CLI behaviour is unchanged.
If flagged, provide the executable and safe `--version` arguments to the winget
maintainers for their validation mechanism. This is not an invented manifest
validation-arguments field or a guarantee of public winget acceptance.

## Remaining risks

- The native binaries import Media Foundation DLLs. Windows N without the
  appropriate Media Feature Pack, or other configurations missing those OS features,
  needs separate qualification. A clean-OS launch is not established by an import
  allowlist, and a VC++ dependency does not supply Media Foundation.
- A resident tray may be reported as "still running" by tooling that expects a
  process to exit. Use `--version` for a terminating check.
- Cargo's CRT setting does not configure the separate Flutter CMake build.
  Rust native dependencies and future plugins must also be checked in the actual
  payload; runtime-loaded DLLs and OS symbol availability remain outside an import-table audit.
