# miccamwatch

`mcw` is a Windows beta monitoring tool that attributes microphone and camera signals to local processes and explains the evidence behind each assessment. It supports high-contrast terminal colors, multilingual core labels, policy-driven trust validation, JSONL logging, Windows Event Log integration, and desktop toast notifications. Detailed evidence remains in English for stable machine-readable diagnostics.

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
- High-contrast terminal color coding for instant status recognition (green, yellow, orange, red).
- Multilingual user interface with automatic Windows system language detection and 7 supported languages: English (`en`), French (`fr`), German (`de`), Spanish (`es`), Japanese (`ja`), Simplified Chinese (`zh`), Russian (`ru`).
- Emergency hardware microphone kill-switch (`mcw mute` / `mcw unmute` / `mcw mute --toggle`).
- Interactive full-terminal live dashboard with keyboard shortcuts (`mcw top`).
- Windows Notification Area background mode with green (idle), yellow (camera-ready), red (confirmed active), and gray (collector error) states (`mcw tray`).
- Privacy Control Center commands for hardware camera allow/block (administrator approval required), scheduled autostart with a per-user fallback, notification pause, lock policies, profiles, and tray lifecycle control.
- Persistent settings in `%APPDATA%\MicCamWatch\settings.toml` and rotating JSONL activity history in `%LOCALAPPDATA%\MicCamWatch`.
- Single-instance tray service with local Windows message IPC (`mcw tray status` / `mcw tray stop`).
- Opt-in termination of explicitly policy-denied active capture processes after two consecutive observations (`--kill-unauthorized`, disabled by default).
- Three-state workstation lock detection; unknown lock state never triggers enforcement.
- Discreet native audio chime upon confirmed capture initiation (`--sound`).
- Monitoring runs without administrator privileges; disabling or re-enabling camera devices prompts for administrator approval.

## Commands

```console
mcw status
mcw status --microphone
mcw status --camera --json
mcw status --risk suspicious
mcw status --include-ready
mcw watch
mcw status --lang fr
mcw watch --notify
mcw watch --interval 250
mcw watch --log events.jsonl
mcw watch --eventlog
mcw watch --no-color
mcw watch --sound
mcw watch --kill-unauthorized
mcw mute
mcw mute --status
mcw mute --toggle
mcw unmute
mcw top
mcw tray
mcw tray status
mcw tray stop
mcw camera status
mcw camera block
mcw camera allow
mcw camera toggle
mcw autostart enable
mcw autostart status
mcw autostart disable
mcw notifications pause 60
mcw notifications resume
mcw profile private
mcw lock-policy enable --microphone --camera
mcw lock-policy disable
mcw history path
mcw history clear
mcw config path
mcw config settings-path
mcw devices
mcw explain 1234
mcw explain 1234 --json
mcw doctor
mcw doctor --json
mcw --config policy.toml status
mcw --config policy.toml config validate
mcw update
```

`status` excludes low-confidence camera-ready pipelines unless `--include-ready` is supplied. It exits with code `0` when no access matching the filters is detected, `1` when a matching access is reported, and `2` on error. `explain` returns `1` when the requested PID has no current observation.

## Assessment model

The three assessment dimensions are intentionally independent:

| Field | Values | Meaning |
|---|---|---|
| `activity` | `active`, `ready` | `active` is reported by a live OS activity source; `ready` means a camera-capable pipeline is loaded but frame flow is unproven. |
| `risk` | `expected`, `unexplained`, `suspicious`, `blocked` | Security interpretation of all collected evidence. `blocked` means Windows permission is denied; it does not claim that frames bypassed Windows. |
| `confidence` | `high`, `medium`, `low` | Strength of the activity claim, not a probability or threat score. |

Microphone attribution uses an active WASAPI capture session and has high confidence. An open Capability Access Manager interval provides medium-confidence camera activity. Loaded camera modules provide low-confidence readiness only.

`mcw` deliberately has no heuristic `unauthorized` result. Enforcement is separate from risk: only an explicit publisher/path policy mismatch produces `enforcement = "deny"`. Automatic termination additionally requires confirmed `active` capture, a PID, two consecutive observations, and a non-protected process.

## JSON contract

Status JSON is an object with an explicit schema version:

```json
{
  "schema_version": 3,
  "tool_version": "0.13.3",
  "collectors": [],
  "accesses": []
}
```

Watch events are newline-delimited JSON objects with `schema_version`, `action`, `observed_at`, and the flattened access assessment. Consumers must reject unsupported schema versions instead of guessing field semantics.

Every access also includes `enforcement`: `allow`, `alert`, `deny`, or `unknown`. Risk is explanatory; enforcement is policy-driven.

## Detection limits

- Loaded Media Foundation or DirectShow modules indicate capture capability, not current frame flow.
- Capability Access Manager values can be historical, delayed, or unavailable.
- Protected or higher-privilege processes can prevent path, command-line, module, or signature inspection.
- A trusted Authenticode signature proves integrity and chain acceptance under the configured Windows policy; it does not prove benign behavior.
- Signer identity may be unavailable for catalog-signed files even when WinVerifyTrust accepts the signature.
- Application-name profiles add context only. They are not allowlists and cannot trigger automatic termination.
- Toast notifications register the per-user `MicCamWatch.MicCamWatch` application identity. Notification errors are reported instead of silently ignored.

The output is suitable for diagnostics and monitoring. It is not a forensic proof that camera frames were captured.

## Install

Download `miccamwatch-windows-x86_64.msi` from the [latest release](https://github.com/Roman-Cuisset/miccamwatch/releases/latest) for a per-user installation with `mcw` on `PATH` and a Start Menu entry. The portable `miccamwatch-windows-x86_64.zip` remains available.

Upgrade later with:

```console
mcw update
```

The updater verifies the SHA-256 checksum published with the GitHub release. Because the archive and checksum share the same release channel, this protects integrity but is not an independent publisher signature. Production signing is conditional on a configured release certificate; see [Authenticode release signing](docs/SIGNING.md).

## Policy configuration

Create a TOML policy file to control trust evaluation:

```toml
language = "fr"             # en | fr | de | es | ja | zh | ru
profile = "strict"          # conservative | balanced | strict
trust_policy = "online"     # offline | online
action = "alert"            # alert (default) | kill
[[applications]]
executable = "zoom.exe"
publishers = ["Zoom Video Communications"]
paths = ["C:\\Program Files\\Zoom"]
```

- **strict**: escalates heuristic `unexplained` assessments to `suspicious`; it does not authorize termination.
- **online**: performs live certificate revocation checking (CRL/OCSP) instead of cache-only.
- **applications**: explicit per-executable publisher and path rules. A fully observed mismatch produces `enforcement = "deny"`; missing identity evidence produces `unknown`, never termination.

Validate a policy file:

```console
mcw --config policy.toml config validate
```

## Privacy Control Center

`mcw tray` keeps the monitor in the Windows notification area. Its menu controls microphone mute, camera privacy, one-hour notification pause, the active profile, and autostart. The tray runs as a single per-user instance; the CLI can inspect or stop it.

Installed and release packages include `mcw-tray.exe`, a windowless tray host used by autostart and the Start Menu shortcut. It prevents a terminal window from remaining open at login. `mcw.exe tray` remains available for interactive diagnostics.

`mcw camera block` disables the currently enabled, connected devices in the Windows Camera device class through PnP. Windows requests one administrator approval per block or allow command, even with several webcams. This affects every application; a blocked physical webcam disappears from capture-device enumeration. `mcw camera allow` restores only the devices saved by the block operation. If a previously blocked webcam has been unplugged, the CLI and tray restore the connected webcams immediately and keep the unplugged one in `%LOCALAPPDATA%\MicCamWatch\blocked-camera-devices.json`. On reconnection, select **Allow camera** again to restore it; a detached webcam is never reported as physically re-enabled. When all connected webcams are restored, `mcw camera status` reports `allowed`, even if a detached device is pending. The tray reports the result or error through a notification. Do not delete the state file while devices remain disabled. If administrator approval is declined or Windows cannot change a connected device, the command reports failure and retains the record. Cameras connected after blocking are not automatically disabled.

For the Windows Camera app, a sustained capture-process workload is treated as active even when Windows stops updating the registry activity interval. Idle browser capture modules remain `ready`; their brief wakeups during Windows Camera capture do not override that app's active attribution. Process CPU is a heuristic, not direct frame telemetry, so simultaneous browser capture while Windows Camera is active cannot be attributed independently.

Autostart first uses a limited per-user Task Scheduler task. On systems that deny task creation, it uses the current user's `Run` registry key instead, without requesting elevation. `mcw autostart disable` removes both mechanisms.

Lock policies are opt-in. On transition to a locked session, the tray can mute microphones. Camera device control requires an administrator prompt; Windows cannot approve that prompt while the session is locked, so `block-camera-on-lock` is not a reliable automatic safeguard. Manually block the cameras before locking when hardware isolation is required. Unknown lock state never triggers enforcement.

The default policy path is `%APPDATA%\MicCamWatch\policy.toml`. It is loaded automatically when present; `--config` overrides it. Application settings and rotating history paths are available through `mcw config settings-path` and `mcw history path`.

## Build from source

Install the stable Rust MSVC toolchain and Visual Studio C++ Build Tools, then run:

```console
cargo build --release
```

The executable is created at `target/release/mcw.exe`.

## Platform scope

Windows 10 and Windows 11 are supported. The reusable library, collector interface, and separate CLI/TUI/tray frontends establish the boundary for additional backends; see [Architecture](docs/ARCHITECTURE.md).

Linux requires PipeWire/V4L2 collectors, macOS requires CoreAudio/AVFoundation/TCC collectors, and Android requires a separate application and permission architecture. These are explicit contracts and roadmap targets, not currently implemented support.

## License

MIT
