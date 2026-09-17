# miccamwatch

`mcw` is a lightweight Windows command-line monitor that attributes microphone and camera signals to local processes and explains the evidence behind each assessment.

## Current capabilities

- Enumerates active microphone sessions through Windows Core Audio/WASAPI.
- Reports PID, stable process instance, parent process, executable, signature, and capture device.
- Enumerates physical camera devices through Windows Media Foundation.
- Correlates camera privacy activity, loaded capture modules, process lineage, command line, permission, file location, and Authenticode status.
- Separates observable activity from security risk and confidence.
- Caches Authenticode verification by executable path and modification time.
- Emits deduplicated start, update, and stop events.
- Supports human-readable and versioned JSON output.
- Runs without administrator privileges.

## Commands

```console
mcw status
mcw status --microphone
mcw status --camera --json
mcw watch
mcw watch --notify
mcw watch --interval 250
mcw devices
mcw explain 1234
mcw explain 1234 --json
mcw update
```

`status` exits with code `0` when no activity is detected, `1` when activity is reported, and `2` on error. `explain` returns `1` when the requested PID has no current observation.

## Assessment model

The three assessment dimensions are intentionally independent:

| Field | Values | Meaning |
|---|---|---|
| `activity` | `active`, `ready` | `active` is reported by a live OS activity source; `ready` means a camera-capable pipeline is loaded but frame flow is unproven. |
| `risk` | `normal`, `unexplained`, `suspicious`, `blocked` | Security interpretation of all collected evidence. `blocked` means Windows permission is denied; it does not claim that frames bypassed Windows. |
| `confidence` | `high`, `medium`, `low` | Strength of the activity claim, not a probability or threat score. |

Microphone attribution uses an active WASAPI capture session and has high confidence. An open Capability Access Manager interval provides medium-confidence camera activity. Loaded camera modules provide low-confidence readiness only.

`mcw` deliberately has no `unauthorized` result: user-mode module inspection cannot prove that video frames were acquired despite a denied permission. It reports `blocked` and exposes the underlying evidence instead.

## JSON contract

Status JSON is an object with an explicit schema version:

```json
{
  "schema_version": 1,
  "accesses": []
}
```

Watch events are newline-delimited JSON objects with `schema_version`, `action`, `observed_at`, and the flattened access assessment. Consumers must reject unsupported schema versions instead of guessing field semantics.

## Detection limits

- Loaded Media Foundation or DirectShow modules indicate capture capability, not current frame flow.
- Capability Access Manager values can be historical, delayed, or unavailable.
- Protected or higher-privilege processes can prevent path, command-line, module, or signature inspection.
- A trusted Authenticode signature proves integrity and chain acceptance under the configured Windows policy; it does not prove benign behavior.
- Signer identity may be unavailable for catalog-signed files even when WinVerifyTrust accepts the signature.
- Application-name profiles add context only. They are not allowlists and do not establish trust by themselves.
- `--notify` currently uses Windows PowerShell to call the WinRT toast API. Process-derived text is passed through environment variables rather than interpolated into PowerShell source.

The output is suitable for diagnostics and monitoring. It is not a forensic proof that camera frames were captured.

## Install

Download `miccamwatch-windows-x86_64.zip` from the [latest release](https://github.com/Roman-Cuisset/miccamwatch/releases/latest), extract `mcw.exe`, and place it in a directory listed in `PATH`.

Upgrade later with:

```console
mcw update
```

The updater verifies the SHA-256 checksum published with the GitHub release. Because the archive and checksum share the same release channel, this protects integrity but is not an independent publisher signature.

## Build from source

Install the stable Rust MSVC toolchain and Visual Studio C++ Build Tools, then run:

```console
cargo build --release
```

The executable is created at `target/release/mcw.exe`.

## Platform scope

Windows 10 and Windows 11 are supported. Linux and macOS would require separate evidence collectors while preserving the versioned assessment model.

## License

MIT
