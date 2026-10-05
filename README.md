# miccamwatch

`mcw` monitors microphone and camera access using platform-specific evidence. Windows uses native capture and privacy APIs; Linux and macOS 15+ provide native monitoring, desktop frontends and scoped controls with narrower evidence. It supports high-contrast terminal colors, seven display languages, policy-driven trust validation, JSONL history and native event notifications. Detailed evidence remains in English for stable machine-readable diagnostics. Stable `v0.14.0` Unix packages are monitoring-only; the expanded native implementation is distributed through the explicitly selected Unix `v0.15.0` prerelease.

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
- Optionally writes events to the native system journal (`--eventlog`): Windows Application Event Log, Linux syslog, or macOS Unified Logging.
- Sends native desktop notifications on access events (`--notify`), subject to the desktop service and user authorization.
- High-contrast terminal color coding for instant status recognition (green, yellow, orange, red).
- Seven display/help languages with native locale detection: English (`en`), French (`fr`), German (`de`), Spanish (`es`), Japanese (`ja`), Simplified Chinese (`zh`), Russian (`ru`).
- Microphone mute/query/owned restoration (`mcw mute` / `mcw unmute` / `mcw mute --toggle`): Windows capture endpoints, Linux PipeWire session sources, or writable macOS CoreAudio inputs. This is not a universal hardware kill-switch or a guarantee against bypassing those scopes.
- Interactive full-terminal live dashboard (`mcw top`) with keyboard and left-click controls. Shortcuts accept either case; `[b]` blocks connected Windows cameras and `[a]` restores cameras previously blocked by MicCamWatch. The controls wrap on narrow terminals. `[k]` requires a second confirmation for the same live process identity; Esc cancels without quitting. Camera approval runs off the UI thread; pending operations prevent an unsafe quit and failures remain visible.
- Native desktop mode (`mcw tray`): Windows Notification Area, Linux StatusNotifierItem on a supporting desktop, or macOS AppKit menu bar.
- Privacy commands expose the actual platform scope: Windows device blocking, Linux session-source mute, and writable macOS input mute plus an explicitly approved owned camera profile. Unsupported controls are disabled or refused, never reported as successful.
- Persistent settings and rotating JSONL history use application-data directories on Windows and XDG/HOME directories on Unix. Query exact paths with `mcw config settings-path` and `mcw history path`.
- Single-instance per-user desktop service with native lifecycle IPC (`mcw tray status` / `mcw tray stop`).
- Opt-in termination of explicitly policy-denied active capture processes after two consecutive observations (`--kill-unauthorized`, disabled by default).
- Three-state session lock detection on Windows/Linux; unknown lock state never triggers enforcement. Public macOS lock state stays unknown and enabling lock policy is refused.
- Discreet native audio chime upon confirmed capture initiation (`--sound`).
- Monitoring runs without administrator privileges. Windows camera device changes request administrator approval; macOS camera profile installation/removal requires manual approval in System Settings.

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
  "tool_version": "0.15.0",
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

For MSI installations, upgrade with the latest MSI from [GitHub Releases](https://github.com/Roman-Cuisset/miccamwatch/releases/latest); `mcw update` does not update MSI registration. Portable/Cargo installs use `mcw update` to install a matching CLI/tray pair. It verifies the selected release before requesting safe shutdown of a tray from this exact installation, refuses shutdown while camera/UAC operations are pending, and waits for actual process termination. A legacy v0.14.0 tray cannot acknowledge this protocol: finish pending operations and quit it manually, then retry. No force-kill or elevation is used to release an executable lock. Tray replacement is atomic; replacing the running CLI requires recoverable same-volume renames and has a brief pathname gap under the installation lock. Failed transactions roll back; incomplete recovery is reported without restarting an inconsistent pair. Only a previously running tray is restarted. Same-version repair verifies the pair but replaces only the missing/outdated tray; it does not pretend the CLI version is `0.0.0`.

The Windows updater downloads one exact release ZIP and checks its SHA-256 against both the selected GitHub API asset digest and its exact named `SHA256SUMS` entry, then validates both staged executable versions before shutdown. These share a release channel and are not an independent publisher signature. Signing is conditional on a real release certificate. If Defender quarantines `mcw.exe`, a `PATH` change is not a remedy: preserve Protection History details and follow [signing, provenance and safe incident handling](docs/SIGNING.md). No exclusion, disabled protection or quarantine restoration is recommended.

### v0.14.0 native CLI packages

| Platform | Release asset | Requirements |
| --- | --- | --- |
| Windows x64 | `miccamwatch-windows-x86_64.zip` or `.msi` | Windows 10/11; ZIP includes matching CLI and tray |
| Linux x64 (experimental) | `miccamwatch-linux-x86_64.tar.gz` | glibc 2.35+; `pw-dump` and accessible user PipeWire socket |
| macOS Apple Silicon (experimental) | `miccamwatch-macos-aarch64.tar.gz` | macOS 15+ |
| macOS Intel (experimental) | `miccamwatch-macos-x86_64.tar.gz` | macOS 15+ |

Download the matching archive and `SHA256SUMS` from the release. On Linux use `sha256sum --check --ignore-missing SHA256SUMS`; on macOS compare `shasum -a 256 <archive>` with the named checksum. Extract the archive and run `./mcw --version`, `./mcw doctor --json`, then `./mcw watch --json`. The macOS helper is embedded; no Swift compiler is needed at runtime. Apple Developer ID signing/notarization is not provided; Gatekeeper may require explicit approval under your organization's policy. Do not bypass managed security controls. Unix self-update is unavailable: replace the extracted binary with a verified newer archive.

v0.14.0 adds Linux PipeWire/V4L2 observation, macOS CoreAudio/AVFoundation observation, a shared Unix watcher, multi-OS CI and matched Windows CLI/tray updates. Native-runner smoke is required before publishing each package; physical microphone/camera capture remains unverified. Linux/macOS packages are experimental, not a promise of hardware blocking or complete camera coverage.

### Unix installer (macOS and Linux)

By default the installer selects the latest **stable** release into `$HOME/.local/bin`, without sudo. Select `--version v0.15.0` for the Unix prerelease with native desktop/scoped-control features. Windows publication remains held; `v0.14.0` stays stable latest so existing Windows installer/updater selection keeps working. The installer chooses the native macOS Apple Silicon/Intel or Linux x64 package, resolves a concrete release, and verifies the exact archive entry in that release's `SHA256SUMS`. Supported systems are macOS 15+ and Linux x64 with glibc 2.35+. Unsupported architectures are rejected, never substituted.

Download and inspect the installer before running it:

```sh
curl --proto '=https' --tlsv1.2 -fsSL \
  https://raw.githubusercontent.com/Roman-Cuisset/miccamwatch/main/installer/install.sh \
  -o mcw-install.sh
less mcw-install.sh
sh mcw-install.sh --version v0.15.0 --add-path
```

Or explicitly select the Unix prerelease, with an interactive PATH proposal when a terminal is available:

```sh
curl --proto '=https' --tlsv1.2 -fsSL \
  https://raw.githubusercontent.com/Roman-Cuisset/miccamwatch/main/installer/install.sh \
  | sh -s -- --version v0.15.0
```

Use `--add-path` for explicitly requested automatic PATH integration, or `--no-modify-path` to leave shell configuration untouched. Without either flag, a missing PATH entry is proposed through `/dev/tty`; a noninteractive installation does not wait for input or silently edit the shell configuration. Bash, zsh (including `ZDOTDIR`), and fish are supported. Open a new terminal after accepting PATH integration: an installer subprocess cannot change its parent's environment. You can then run `mcw --version`, `mcw doctor --json`, and `mcw watch --json` from any directory.

`--prefix "$HOME/Applications/MicCamWatch"` installs into that prefix's `bin` directory. Repeat the installer with `--version v0.15.0` to upgrade an older managed Unix installation. Stop its active watcher/tray first. Failed downloads, integrity checks, or executable validation preserve an existing working installation. The `main` installer URL follows source updates; use a reviewed commit permalink to pin the script. Archives and checksums share a release channel and are not an independent publisher signature.

Root execution is refused. The installer does not overwrite an unmanaged executable, a symlink/hardlinked target, or an installer-owned executable modified independently; select a different prefix or move the old installation yourself. It refuses to replace or remove the installed executable while that target is running, without stopping any process. Ownership receipts live in `PREFIX/.miccamwatch-install`; retain them for safe upgrades and uninstall.

To uninstall an installer-managed installation, run `sh mcw-install.sh --uninstall`, supplying the same `--prefix` if you used a custom prefix. Uninstallation removes only installer-owned executable/state and managed PATH entries; it does not delete policy, settings, or activity history. An executable or PATH block changed independently must not be removed as though it were still installer-owned.

`v0.15.0` installs `mcw` with its shared TUI, native desktop mode and platform-scoped controls; `v0.14.0` remains monitoring-only. Installation never enables autostart, changes devices, approves a profile or grants capture permissions. Gatekeeper, SIP, TCC and quarantine protections are not bypassed. Apple Developer ID signing/notarization remains unavailable. `mcw update` on a managed Unix `0.15.0` installation follows the stable channel and refuses downgrades; select a prerelease explicitly through the installer.

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
- **online**: permits native online trust checks; offline monitoring never enables retrieval implicitly. Windows uses certificate revocation checks; macOS uses Apple's configured code-signing validation without a guarantee of fresh online revocation data. Linux detached OpenPGP verification permits key retrieval only when explicitly configured online.
- **applications**: explicit per-executable publisher and path rules. Windows publisher names match the verified Authenticode signer; Linux publishers pin `openpgp:<FULL_UPPERCASE_PRIMARY_FINGERPRINT>` of the verified detached `<executable>.sig`; macOS uses native trusted certificate identity, never ad-hoc integrity as publisher trust. Path prefixes must end at a directory-component boundary. Publisher and path groups are both required when configured. Missing identity evidence produces `unknown`, never termination. These are lexical path checks, not a defense against reparse-point or symlink redirection.

Validate a policy file:

```console
mcw --config policy.toml config validate
```

## Privacy Control Center

`mcw tray` keeps the monitor in the Windows notification area. Its menu controls microphone mute, camera privacy, one-hour notification pause, the active profile, and autostart. The tray runs as a single per-user instance; the CLI can inspect or stop it. A left or right click on the icon opens the menu; a double click raises a toast with the current summary.

Installed and release packages include `mcw-tray.exe`, a windowless tray host used by autostart and the Start Menu shortcut. It prevents a terminal window from remaining open at login. `mcw.exe tray` remains available for interactive diagnostics.

`mcw camera block` disables currently enabled, connected devices in the Windows Camera setup class and legacy Image-class devices that expose a video-camera interface or both video and capture interfaces. Image also contains scanners; an Image device without positive video-capture membership is not disabled. Inventory uses native Windows Configuration Manager APIs rather than localized command output; inventory failures stop the operation. Windows requests one administrator approval per block or allow command, even with several webcams. This affects every application; a blocked physical webcam disappears from capture-device enumeration. `mcw camera allow` restores only devices saved by a block operation. If a previously blocked webcam is unplugged, the command restores connected webcams immediately and records the unplugged one in `%LOCALAPPDATA%\MicCamWatch\blocked-camera-devices.json`. The record distinguishes devices that must stay blocked from those still owed restoration; when the webcam reconnects, the running tray prompts for administrator approval to restore it.

If the tray is not running when a webcam reconnects, run `mcw camera allow` after reconnecting to complete a pending restoration. While an unplugged webcam is waiting to be restored, `mcw camera status` reports `system_managed`, not fully allowed. If Windows fails to change a device or administrator approval is declined, its saved state is retained so the action can be retried; do not delete `blocked-camera-devices.json` to recover. `mcw camera block` affects only webcams connected and enabled at the time of the command; newly connected webcams are not automatically blocked.

Windows additionally queries Media Foundation sensor activity reports for per-client camera streaming and validates any reported PID against its current process instance. This is passive OS metadata: MCW does not open a camera or receive frames. An explicit non-streaming report leaves a loaded pipeline `ready`; a stale report does not persist across snapshots. ConsentStore activity intervals remain a medium-confidence fallback for capture paths not exposed by this API. Loaded modules and CPU workload alone never establish `active`. An authorized native smoke on a Logi C270 used an eight-second FFmpeg DirectShow client outside ktalk/Windows Camera: 121 frames were discarded to the null output, while MCW reported camera `active`, then watch START/STOP and no remaining active observation. Windows did not expose a verifiable client PID on that path, so MCW correctly retained unknown attribution and enforcement; this is not proof of arbitrary-client PID coverage. STOP events retain the last observed access fields rather than claiming the stream remains active.

Autostart first uses a limited per-user Task Scheduler task. On systems that deny task creation, it uses the current user's `Run` registry key instead, without elevation. A companion tray must match the CLI's embedded version. Update preserves the existing owned mechanism and repoints it only if its enabled target changed; unchanged, disabled and unmanaged registrations are left alone. Task ownership is checked from XML against the target, current user's SID and limited logon configuration. A Task Manager/Startup Apps-disabled Run entry is not silently re-enabled, and MCW never rewrites `StartupApproved` to override that intent. Disable removes only owned entries. Update and registration changes share the installation lock.

Lock policies are opt-in. On transition to a locked session, the tray can mute microphones. Camera device control runs on a worker so the tray remains responsive while approval is pending; it restores a previously allowed camera on unlock only after blocking succeeded. Windows cannot approve the administrator prompt while the session is locked, so `block-camera-on-lock` is not a reliable automatic safeguard. Manually block the cameras before locking when hardware isolation is required. Unknown lock state never triggers enforcement.

The default policy path is `%APPDATA%\MicCamWatch\policy.toml`. It is loaded automatically when present; `--config` overrides it. Application settings and rotating history paths are available through `mcw config settings-path` and `mcw history path`.

### Unix desktop controls

`mcw top` uses the shared dashboard. `mcw tray` stays in the foreground when
started interactively; opt-in `mcw autostart enable` registers native
desktop-session startup without a terminal (`.desktop` on Linux, a per-user
LaunchAgent on macOS). No installer implicitly enables it. Owned registration
is refreshed during managed update; unrelated or externally changed entries
are not overwritten or removed.

Linux microphone controls mute PipeWire session sources through native Props,
retain original values, and restore only MCW-owned changes in the same live
server/node identity. They do not deny direct ALSA access. Global Linux camera
blocking is unsupported: direct V4L2 access can bypass PipeWire.

macOS microphone controls operate only on writable CoreAudio input-mute
properties. Missing/read-only properties are unavailable, not muted.
`mcw camera block` prepares an owned Restrictions profile and opens the manual
approval path in System Settings; pending approval is not a block.
`mcw camera allow` requests removal of that exact owned profile. Profile
installation metadata is not proof of physical capture denial. There is no
universal microphone deny or private lock-state fallback.

Mute and profile ownership records survive uninstall so restoration remains
possible. Restore owned changes explicitly before uninstall if desired.
`mcw update` requires a public-installer receipt on Unix; stop an active
installed watcher/tray first. Native notifications, sound and menu-bar/tray
registration need the corresponding graphical session and services.

Human STOP output labels its retained access details as the **last observation**,
not current capture. JSON keeps the schema-3 event contract: a STOP can carry
the last `active` evidence after capture ended. CoreAudio output-device lists
do not identify Sound/Telegram's microphone; incomplete input-device identity
remains unknown.

## Build from source

Install the stable Rust MSVC toolchain and Visual Studio C++ Build Tools, then run:

```console
cargo build --release --locked --features windows-tray
```

Windows executables are created at `target/release/mcw.exe` and `target/release/mcw-tray.exe`. Without `windows-tray`, only the CLI is built.

For Linux, install stable Rust, `pkg-config`, PipeWire/SPA development headers and libclang, then run `cargo build --release --locked`. Runtime monitoring needs the user PipeWire service and `pw-dump`; detached signature rules need GnuPG. Ubuntu 22.04's stock PipeWire 0.3.48 headers are supported. On macOS 15+, install Xcode Command Line Tools with a macOS 15 SDK and Swift compiler, then run the same Cargo command. Cargo embeds a private ad-hoc-signed native helper application in `mcw`; no development script is needed at runtime. Ad-hoc signing establishes helper integrity/identity, not Developer ID trust or notarization.

## Platform scope

| OS / environment | Microphone `active` / PID | Camera `active` / PID | Camera `ready` | Hardware blocking and desktop controls |
| --- | --- | --- | --- | --- |
| Windows 10/11 | WASAPI session / validated process | Capture activity evidence / validated process where available | Loaded capture pipeline, unconfirmed | Administrator-approved camera device controls, mute, tray and notifications |
| Linux desktop with PipeWire | Running source, stream and active capture link / authenticated Client PID validated with `/proc` | Running video source, stream and active capture link / validated authenticated PID | Idle stream or direct V4L2 open FD, low confidence | Session-source mute; no global camera block; native TUI, notifications and StatusNotifierItem host required |
| macOS 15+ | CoreAudio input activity / validated PID when libproc identity is readable | AVFoundation other-application use / **unknown PID** | Not inferred from camera availability | Writable input mute, manually approved owned camera profile, native TUI and AppKit menu bar |
| WSL or virtual/headless runners | No guaranteed access to physical capture hardware or user session | No guaranteed camera signal | Inventory is not access proof | No hardware behavior claim |

Linux builds a native CLI using the `pw-dump` PipeWire client and `/proc` (no administrator privileges required for the CLI). The Linux host needs an accessible user PipeWire socket (`XDG_RUNTIME_DIR`, optionally `PIPEWIRE_REMOTE`), `pw-dump`, and readable `/proc/<pid>/stat` and `/proc/<pid>/exe` for PID attribution. `mcw status --json`, `mcw devices`, `mcw doctor`, and `mcw watch --json` run on Linux and macOS. `watch --json` emits access-event JSONL and a status document containing `collectors` when health changes; Ctrl+C stops it.

Linux desktop effects require a StatusNotifierItem host, a native notification
daemon with `notify-send`, and `canberra-gtk-play` with an installed sound theme
and a backend for the actual sound session. On Ubuntu, the
[libcanberra-pulse backend](https://packages.ubuntu.com/jammy/libcanberra-pulse)
uses PulseAudio or PipeWire's PulseAudio server; installing only the GTK player
does not provide that backend. Playback acceptance is not physical audibility.

On Linux, only a **running PipeWire capture stream with an active source link and a running source node** is marked `active`. A claimed application PID must match the owning Client's server-authenticated `pipewire.sec.pid`, then pass `/proc` start-time and executable validation; forwarded portal/PulseAudio clients without matching identity remain unattributed. A direct V4L2 open FD never proves frame flow; video health is `degraded` when a camera may be accessed outside PipeWire. An inaccessible or restarting PipeWire socket is `unavailable` (`status` exits 2), not an all-clear. `--include-ready` exposes unconfirmed PipeWire streams and direct `/dev/video*` handles as `ready`, never as active.

On macOS, camera usage is device-level only: an application name or PID cannot be inferred from AVFoundation's in-use boolean. Camera health stays `degraded` because own-application use and noninteractive TCC/device discovery can be missed; an empty scan is not proof of no use. CI verifies backend commands and honest health reports, **not** physical microphone/camera transitions. No camera/TCC bypass or intrusive probe is attempted. Linux enforcement requires stable pidfd authority; macOS requires retained task/audit-token authority and refuses protected or inaccessible targets. Android needs a separate application.

The [0.15.0 native release dry run](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37270375386)
passed Ubuntu 22.04/PipeWire 0.3.48 and macOS 15.7.9 on Apple Silicon and Intel:
format, Clippy, tests, debug/release builds and extracted-package runtime checks.
macOS used SDK 15.5 and Swift 6.1.2 in Swift 5 mode, deployment target 15.0.
The smoke exercised real desktop icons, owned start/stop and autostart,
seven-language help, Linux native notifications/journal delivery and session mute,
and native chime acceptance. Windows [CI](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37267498847)
passed 99 tests, dependency audit and per-user MSI smoke; its publication remains held.

On `serveur-asus` (PipeWire 1.0.5), a disposable virtual `Audio/Source` and
authenticated `pw-cat` PID proved active capture, mute/unmute readback,
preservation of an already-muted source, TUI K/Esc/Q cancellation and human
START/STOPPED with retained-last-observation labeling. This was real session
flow, **not physical microphone or global access-blocking proof**. The SSH user
has no physical microphone source and cannot open `/dev/video0`; permissions
were not changed. MacBook M1 Pro/macOS 27, physical Linux/macOS capture and
mute/profile effects, real lock transitions, macOS notification authorization
and speaker audibility remain unverified. See [Architecture](docs/ARCHITECTURE.md).

## Privacy

MicCamWatch is designed from the ground up as an offline-first privacy tool:
- **Offline by default**: monitoring has no telemetry or analytics. Explicit `mcw update` contacts GitHub; an explicitly online trust policy permits the native trust-network behavior described above.
- **No media capture**: `mcw` inspects capture session metadata, loaded modules, and registry activity timestamps. It never records audio samples or captures video frames.
- **Local storage**: Settings and history logs remain local: under `%APPDATA%\MicCamWatch` and `%LOCALAPPDATA%\MicCamWatch` on Windows, and XDG/HOME directories on Linux and macOS. macOS extracts its embedded native helper to a private temporary directory, removed on normal exit.

See the full [Privacy Policy](PRIVACY.md).

## License

MIT
