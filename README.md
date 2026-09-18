# miccamwatch

`mcw` is a lightweight Windows command-line monitor that attributes microphone and camera signals to local processes and explains the evidence behind each assessment. It supports policy-driven trust validation, JSONL logging, Windows Event Log integration, and desktop toast notifications.

## Current capabilities

- Enumerates active microphone sessions through Windows Core Audio/WASAPI with real-time session callbacks.
- Reports PID, stable process instance, parent process, full ancestry chain, executable, signature, user, session, integrity level, and capture device.
- Enumerates physical camera devices through Windows Media Foundation.
- Correlates camera privacy activity, loaded capture modules, process lineage, command line, permission, file location, and Authenticode status.
- Monitors ConsentStore registry changes via native notifications for near-instant camera event detection.
- Separates observable activity from security risk and confidence.
- Supports configurable TOML policy files with trust profiles (conservative, balanced, strict), publisher/path validation, and online/offline revocation checking.
- Caches Authenticode verification by executable path and modification time.
- Emits deduplicated start, update, and stop events with stable event codes.
- Supports human-readable and versioned JSON output.
- Filters output by minimum risk level (`--risk`).
- Writes JSONL event logs to file (`--log`).
- Optionally writes events to the Windows Application event log (`--eventlog`).
- Sends Windows desktop toast notifications on access events (`--notify`).
- Runs without administrator privileges.

## Commands

```console
mcw status
mcw status --microphone
mcw status --camera --json
mcw status --risk suspicious
mcw watch
mcw watch --notify
mcw watch --interval 250
mcw watch --log events.jsonl
mcw watch --eventlog
mcw devices
mcw explain 1234
mcw explain 1234 --json
mcw doctor
mcw doctor --json
mcw --config policy.toml status
mcw --config policy.toml config validate
mcw update

`status` exits with code `0` when no activity is detected, `1` when activity is reported, and `2` on error. `explain` returns `1` when the requested PID has no current observation.

## Assessment model

The three assessment dimensions are intentionally independent:

| Field | Values | Meaning |
|---|---|---|
| `activity` | `active`, `ready` | `active` is reported by a live OS activity source; `ready` means a camera-capable pipeline is loaded but frame flow is unproven. |
| `risk` | `expected`, `unexplained`, `suspicious`, `blocked` | Security interpretation of all collected evidence. `blocked` means Windows permission is denied; it does not claim that frames bypassed Windows. |
| `confidence` | `high`, `medium`, `low` | Strength of the activity claim, not a probability or threat score. |

Microphone attribution uses an active WASAPI capture session and has high confidence. An open Capability Access Manager interval provides medium-confidence camera activity. Loaded camera modules provide low-confidence readiness only.

`mcw` deliberately has no `unauthorized` result: user-mode module inspection cannot prove that video frames were acquired despite a denied permission. It reports `blocked` and exposes the underlying evidence instead.

## JSON contract

Status JSON is an object with an explicit schema version:

```json
{
  "schema_version": 2,
  "tool_version": "0.7.0",
  "collectors": [],
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
- `--notify` uses the WinRT toast API via XML DOM. Process-derived text is sanitized through XML escaping before insertion.

The output is suitable for diagnostics and monitoring. It is not a forensic proof that camera frames were captured.

## Install

Download `miccamwatch-windows-x86_64.zip` from the [latest release](https://github.com/Roman-Cuisset/miccamwatch/releases/latest), extract `mcw.exe`, and place it in a directory listed in `PATH`.

Upgrade later with:

```console
mcw update
```

The updater verifies the SHA-256 checksum published with the GitHub release. Because the archive and checksum share the same release channel, this protects integrity but is not an independent publisher signature.

## Policy configuration

Create a TOML policy file to control trust evaluation:

```toml
profile = "strict"          # conservative | balanced | strict
trust_policy = "online"     # offline | online

[[applications]]
executable = "zoom.exe"
publishers = ["Zoom Video Communications"]
paths = ["C:\\Program Files\\Zoom"]
```

- **strict**: escalates `unexplained` accesses to `suspicious`.
- **online**: performs live certificate revocation checking (CRL/OCSP) instead of cache-only.
- **applications**: per-executable publisher and path validation rules. Mismatches are flagged as `suspicious` with detailed evidence.

Validate a policy file:

```console
mcw --config policy.toml config validate
```

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
