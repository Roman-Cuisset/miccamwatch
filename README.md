# miccamwatch

`mcw` monitors microphone and camera access with platform-specific evidence. Windows is the release-gated beta with process attribution and privacy controls; Linux and macOS provide native CLI monitoring with narrower evidence. It supports high-contrast terminal colors, multilingual core labels, policy-driven trust validation, JSONL logging, and, on Windows, Event Log integration and desktop toast notifications. Detailed evidence remains in English for stable machine-readable diagnostics.

## Current capabilities

- Enumerates active microphone sessions through Windows Core Audio/WASAPI with real-time session callbacks.
- Reports PID, stable process instance, parent process, full ancestry chain, executable, signature, user, session, integrity level, and capture device.
- Enumerates physical camera devices through Windows Media Foundation.
- Correlates camera privacy activity, loaded capture modules, process lineage, command line, permission, file location, and Authenticode status.
- Monitors ConsentStore registry changes via native notifications for near-instant camera event detection.
- Separates observable activity from security risk and confidence.
- Supports configurable TOML policy files with trust profiles (conservative, balanced, strict), publisher/path validation, and online/offline revocation checking.
- Re-verifies Authenticode for each observed capture; executable path and modification time alone cannot safely cache signer identity after a file replacement.
- Emits deduplicated start, update, and stop events with stable event codes.
- Supports human-readable and versioned JSON output.
- Filters output by minimum risk level (`--risk`).
- Writes JSONL event logs to file (`--log`).
- Optionally writes events to the Windows Application event log (`--eventlog`).
- Sends Windows desktop toast notifications on access events (`--notify`).
- High-contrast terminal color coding for instant status recognition (green, yellow, orange, red).
- Multilingual user interface with automatic Windows system language detection and 7 supported languages: English (`en`), French (`fr`), German (`de`), Spanish (`es`), Japanese (`ja`), Simplified Chinese (`zh`), Russian (`ru`).
- Emergency hardware microphone kill-switch (`mcw mute` / `mcw unmute` / `mcw mute --toggle`).
- Interactive full-terminal live dashboard with keyboard shortcuts (`mcw top`); `[b]` blocks connected cameras and `[a]` restores cameras previously blocked by MicCamWatch. Both actions may require Windows administrator approval; the displayed camera state and any failure remain visible.
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

On Windows, microphone attribution uses an active WASAPI capture session and has high confidence. An open Capability Access Manager interval provides medium-confidence camera activity; loaded camera modules provide low-confidence readiness only. Linux requires running PipeWire nodes and an active capture link for `active`. On macOS 15+, CoreAudio process `isRunningInput` signals microphone `active` only for a process exposed by `AudioHardwareSystem.processes`; AVFoundation `AVCaptureDevice.isInUseByAnotherApplication` signals camera `active` without a PID. Neither signal proves audio samples or camera frame flow.

`mcw` deliberately has no heuristic `unauthorized` result. Enforcement is separate from risk: only an explicit publisher/path policy mismatch produces `enforcement = "deny"`. Automatic termination additionally requires confirmed `active` capture, a PID, two consecutive observations, and a non-protected process. An unattributed macOS camera observation has unknown enforcement and cannot trigger termination.

## JSON contract

Status JSON is an object with an explicit schema version:

```json
{
  "schema_version": 3,
  "tool_version": "0.13.4",
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
- NonPackaged camera records are matched to running processes by full executable path; inaccessible paths, short-name aliases, junctions and other path representations may remain unattributed rather than borrowing another process's signature.
- Toast notifications register the per-user `MicCamWatch.MicCamWatch` application identity. Notification errors are reported instead of silently ignored.

The output is suitable for diagnostics and monitoring. It is not a forensic proof that camera frames were captured.

## Install

Download `miccamwatch-windows-x86_64.msi` from the [latest release](https://github.com/Roman-Cuisset/miccamwatch/releases/latest) for a per-user installation with `mcw` on `PATH` and a Start Menu entry. The portable `miccamwatch-windows-x86_64.zip` remains available.

For MSI installations, upgrade by running the latest MSI from [GitHub Releases](https://github.com/Roman-Cuisset/miccamwatch/releases/latest). For portable or Cargo-based installs, stop the tray and use `mcw update` to install the matching `mcw.exe` and `mcw-tray.exe` from the same release; alternatively replace both ZIP binaries together. If an active tray locks its executable, the update fails without reporting success: stop the tray and retry. `mcw update` does not update MSI registration, so use the MSI for MSI installations.

The CLI updater verifies the SHA-256 checksum published with the GitHub release. Because the archive and checksum share the same release channel, this protects integrity but is not an independent publisher signature. Production signing is conditional on a configured release certificate; see [Authenticode release signing](docs/SIGNING.md).

### v0.14.0 native CLI packages

| Platform | Release asset | Requirements |
| --- | --- | --- |
| Windows x64 | `miccamwatch-windows-x86_64.zip` or `.msi` | Windows 10/11; ZIP includes matching CLI and tray |
| Linux x64 (experimental) | `miccamwatch-linux-x86_64.tar.gz` | glibc 2.35+; `pw-dump` and accessible user PipeWire socket |
| macOS Apple Silicon (experimental) | `miccamwatch-macos-aarch64.tar.gz` | macOS 15+ |
| macOS Intel (experimental) | `miccamwatch-macos-x86_64.tar.gz` | macOS 15+ |

Download the matching archive and `SHA256SUMS` from the release. On Linux use `sha256sum --check --ignore-missing SHA256SUMS`; on macOS compare `shasum -a 256 <archive>` with the named checksum. Extract the archive and run `./mcw --version`, `./mcw doctor --json`, then `./mcw watch --json`. The macOS helper is embedded; no Swift compiler is needed at runtime. Apple Developer ID signing/notarization is not provided; Gatekeeper may require explicit approval under your organization's policy. Do not bypass managed security controls. Unix self-update is unavailable: replace the extracted binary with a verified newer archive.

v0.14.0 adds Linux PipeWire/V4L2 observation, macOS CoreAudio/AVFoundation observation, a shared Unix watcher, multi-OS CI and matched Windows CLI/tray updates. Native-runner smoke is required before publishing each package; physical microphone/camera capture remains unverified. Linux/macOS packages are experimental, not a promise of hardware blocking or complete camera coverage.

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
- **applications**: explicit per-executable publisher and path rules. Publisher names must match the verified Authenticode signer exactly (case-insensitive); path prefixes end at a Windows directory boundary. These are lexical checks, not a defense against reparse-point redirects. A fully observed mismatch produces `enforcement = "deny"`; missing identity evidence produces `unknown`, never termination.

Validate a policy file:

```console
mcw --config policy.toml config validate
```

## Privacy Control Center

`mcw tray` keeps the monitor in the Windows notification area. Its menu controls microphone mute, camera privacy, one-hour notification pause, the active profile, and autostart. The tray runs as a single per-user instance; the CLI can inspect or stop it. A left or right click on the icon opens the menu; a double click raises a toast with the current summary.

Installed and release packages include `mcw-tray.exe`, a windowless tray host used by autostart and the Start Menu shortcut. It prevents a terminal window from remaining open at login. `mcw.exe tray` remains available for interactive diagnostics.

`mcw camera block` disables currently enabled, connected devices in the Windows Camera setup class and legacy Image-class devices that expose a video-camera interface or both video and capture interfaces. Image also contains scanners; an Image device without positive video-capture membership is not disabled. Inventory uses native Windows Configuration Manager APIs rather than localized command output; inventory failures stop the operation. Windows requests one administrator approval per block or allow command, even with several webcams. This affects every application; a blocked physical webcam disappears from capture-device enumeration. `mcw camera allow` restores only devices saved by a block operation. If a previously blocked webcam is unplugged, the command restores connected webcams immediately and records the unplugged one in `%LOCALAPPDATA%\MicCamWatch\blocked-camera-devices.json`. The record distinguishes devices that must stay blocked from those still owed restoration; when the webcam reconnects, the running tray prompts for administrator approval to restore it.

If the tray is not running when a webcam reconnects, run `mcw camera allow` after reconnecting to complete a pending restoration. While an unplugged webcam is waiting to be restored, `mcw camera status` reports `system_managed`, not fully allowed. If Windows fails to change a device or administrator approval is declined, its saved state is retained so the action can be retried; do not delete `blocked-camera-devices.json` to recover. `mcw camera block` affects only webcams connected and enabled at the time of the command; newly connected webcams are not automatically blocked.

For the Windows Camera app, a sustained capture-process workload is treated as active even when Windows stops updating the registry activity interval. Idle browser capture modules remain `ready`; their brief wakeups during Windows Camera capture do not override that app's active attribution. Process CPU is a heuristic, not direct frame telemetry, so simultaneous browser capture while Windows Camera is active cannot be attributed independently.

Autostart first uses a limited per-user Task Scheduler task. On systems that deny task creation, it uses the current user's `Run` registry key instead, without requesting elevation. Autostart refuses a companion tray whose embedded version differs from the CLI, rather than silently starting an older version. Successful `mcw update` refreshes an enabled MicCamWatch autostart registration to the updated tray path, without enabling autostart for users who disabled it. `mcw autostart disable` removes both mechanisms.

Lock policies are opt-in. On transition to a locked session, the tray can mute microphones. Camera device control runs on a worker so the tray remains responsive while approval is pending; it restores a previously allowed camera on unlock only after blocking succeeded. Windows cannot approve the administrator prompt while the session is locked, so `block-camera-on-lock` is not a reliable automatic safeguard. Manually block the cameras before locking when hardware isolation is required. Unknown lock state never triggers enforcement.

The default policy path is `%APPDATA%\MicCamWatch\policy.toml`. It is loaded automatically when present; `--config` overrides it. Application settings and rotating history paths are available through `mcw config settings-path` and `mcw history path`.

## Build from source

Install the stable Rust MSVC toolchain and Visual Studio C++ Build Tools, then run:

```console
cargo build --release --locked --features windows-tray
```

Windows executables are created at `target/release/mcw.exe` and `target/release/mcw-tray.exe`. Without `windows-tray`, only the CLI is built.

For a Linux CLI build, install stable Rust and run `cargo build --release --locked`; `pw-dump` is needed at runtime. On macOS 15+, install Xcode Command Line Tools with a macOS 15 SDK and Swift compiler, then run the same Cargo command. Cargo compiles the native Swift capture helper and embeds it in `mcw`; no development script is needed at runtime.

## Platform scope

| OS / environment | Microphone `active` / PID | Camera `active` / PID | Camera `ready` | Hardware blocking and desktop controls |
| --- | --- | --- | --- | --- |
| Windows 10/11 | WASAPI session / validated process | Capture activity evidence / validated process where available | Loaded capture pipeline, unconfirmed | Administrator-approved camera device controls, mute, tray and notifications |
| Linux desktop with PipeWire | Running source, stream and active capture link / authenticated Client PID validated with `/proc` | Running video source, stream and active capture link / authenticated Client PID validated with `/proc` | Idle stream or direct V4L2 open FD, low confidence | Unavailable; CLI/watch only |
| macOS 15+ | CoreAudio `AudioHardwareSystem.processes` with `isRunningInput` / validated PID when libproc identity is readable | AVFoundation `isInUseByAnotherApplication` / **unknown PID** | Not inferred from camera availability | Unavailable; CLI/watch only |
| WSL or virtual/headless runners | No guaranteed access to physical capture hardware or user session | No guaranteed camera signal | Inventory is not access proof | No hardware behavior claim |

Linux builds a native CLI using the `pw-dump` PipeWire client and `/proc` (no administrator privileges required for the CLI). The Linux host needs an accessible user PipeWire socket (`XDG_RUNTIME_DIR`, optionally `PIPEWIRE_REMOTE`), `pw-dump`, and readable `/proc/<pid>/stat` and `/proc/<pid>/exe` for PID attribution. `mcw status --json`, `mcw devices`, `mcw doctor`, and `mcw watch --json` run on Linux and macOS. `watch --json` emits access-event JSONL and a status document containing `collectors` when health changes; Ctrl+C stops it.

On Linux, only a **running PipeWire capture stream with an active source link and a running source node** is marked `active`. A claimed application PID must match the owning Client's server-authenticated `pipewire.sec.pid`, then pass `/proc` start-time and executable validation; forwarded portal/PulseAudio clients without matching identity remain unattributed. A direct V4L2 open FD never proves frame flow; video health is `degraded` when a camera may be accessed outside PipeWire. An inaccessible or restarting PipeWire socket is `unavailable` (`status` exits 2), not an all-clear. `--include-ready` exposes unconfirmed PipeWire streams and direct `/dev/video*` handles as `ready`, never as active.

On macOS, the camera signal is device-level only: an application name or PID cannot be inferred from AVFoundation's in-use boolean. The camera collector remains `degraded` because it cannot see use by this application and noninteractive TCC/device discovery may miss cameras; an empty scan is not proof of no use. CI is configured to verify backend commands and honest health reports, **not** microphone or camera hardware transitions. No macOS camera/TCC bypass or intrusive probe is attempted. Linux and macOS do not provide hardware-block, mute, tray, autostart, desktop notifications, Windows updater, `top`, or process termination. Android needs a separate application.

Native CI passed on Windows, Ubuntu and macOS arm64 (macOS 26.6.2, deployment target 15.0): format, Clippy, tests, builds and CLI smoke; Windows also passed dependency audit and MSI smoke. See [verified run](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/36677827660). On `serveur-asus` (PipeWire 1.0.5), a temporary virtual `Audio/Source` plus `pw-record` proved idle → active → stopped, authenticated recorder PID and `watch` START/STOP. This was real PipeWire flow, **not a physical microphone test**. The camera inventory listed `/dev/video0` and `/dev/video1`, but opening `/dev/video0` was denied to the SSH user; physical Linux/macOS capture transitions remain unverified. See [Architecture](docs/ARCHITECTURE.md).

## Privacy

MicCamWatch is designed from the ground up as an offline-first privacy tool:
- **Offline by default**: monitoring has no telemetry or analytics. Explicit `mcw update` contacts GitHub; a policy with `trust_policy = "online"` can contact certificate-revocation services through Windows.
- **No media capture**: `mcw` inspects capture session metadata, loaded modules, and registry activity timestamps. It never records audio samples or captures video frames.
- **Local storage**: Settings and history logs remain local: under `%APPDATA%\MicCamWatch` and `%LOCALAPPDATA%\MicCamWatch` on Windows, and XDG/HOME directories on Linux and macOS. macOS extracts its embedded native helper to a private temporary directory, removed on normal exit.

See the full [Privacy Policy](PRIVACY.md).

## License

MIT
