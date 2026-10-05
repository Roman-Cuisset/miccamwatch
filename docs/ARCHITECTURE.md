# Architecture

MicCamWatch has a shared Rust library, CLI, TUI and desktop lifecycle. Windows uses native capture/privacy APIs; Linux uses PipeWire and read-only V4L2 evidence; macOS 15+ uses CoreAudio and AVFoundation with narrower attribution. Native implementation does not imply equal device-control coverage.

## Layers

- `model`: versioned observations, evidence, risk, enforcement decisions, and diagnostics.
- `collector`: platform-neutral `CaptureScope`, `CaptureCollector`, and capability contracts.
- `platform`: Windows uses WASAPI, Media Foundation, ConsentStore, process inspection and Authenticode. Linux uses `pw-dump` against the real PipeWire graph, `/proc` process identity and `/dev/video*` inventory/read-only FD probes. macOS uses CoreAudio `AudioHardwareSystem.processes` and AVFoundation `AVCaptureDevice.isInUseByAnotherApplication`.
- `config`, `settings`, `history`: policy, persistent preferences, rotating event storage (Windows application data; XDG paths on Unix).
- `watcher`, `notify`, `updater`: native event dispatch and effects; Unix updates require an owned public-installer receipt and preserve explicit autostart intent.
- `frontends/cli`, `frontends/tui`: shared controls and collector contracts. `frontends/tray`: Win32 Notification Area, Linux StatusNotifierItem, or macOS AppKit status item. The separate Windows tray executable requires the `windows-tray` Cargo feature; Unix desktop mode is part of `mcw`.

The observable activity state, heuristic risk, confidence, and enforcement decision remain independent. A collector must not convert incomplete evidence into an enforcement denial.

## Unix installer boundary

`installer/install.sh` selects the published OS/CPU archive, resolves `latest`
to a concrete release tag, and verifies its named SHA-256 manifest entry before
extracting the CLI. Installation is per-user, without `sudo`; the executable is
staged on the destination filesystem and checked before atomic replacement.
Running, unmanaged, or externally modified executables are not overwritten.

The install receipt and exact managed PATH blocks belong to the installer, not
to capture collection or application preferences. Explicit `--add-path` or
terminal consent updates the active bash, zsh, or fish configuration;
noninteractive installation does not change it by default. Uninstall removes
only owned, unchanged artifacts and preserves unrelated configuration and data.
Installer staging and locks live under the install prefix or configuration
directory; a live macOS watcher's extracted helper in `TMPDIR` is not an
installer download leak.

The public `v0.14.0` Unix archives contain the monitoring CLI. The `0.15.1`
source adds native desktop frontends and scoped controls; installing it never
implicitly enables autostart, changes devices, or approves a macOS profile.


The [public installer verification run 36827199694](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/36827199694)
passed on Ubuntu 22.04, macOS 15 arm64, and macOS 15 Intel, downloading the
commit-pinned public script and released `v0.14.0` archives. It exercised fresh
interactive/login shells, PATH consent and idempotence, native CLI/watch,
same-version atomic replacement, hostile downloads, and uninstall preservation.
This run does not establish physical capture coverage or cross-version upgrades.

## Collector contract

A platform backend implements `CaptureCollector`:

1. `snapshot(CaptureScope)` returns observations plus explicit collector health.
2. `devices()` enumerates physical capture devices when the platform exposes them.
3. `diagnostics()` explains backend availability and degraded behavior.

`CollectorContract` records the evidence classes required from platform backends. These constants are design requirements, not proof that hardware was exercised in CI.

## Platform boundaries

- **Windows**: implemented and release-gated. WASAPI provides microphone session attribution; Capability Access Manager and capture-module inspection provide camera evidence.
- **Linux**: audio `Active` requires PipeWire `Stream/Input/Audio` and `Audio/Source` nodes in `running` state plus an `active` source-to-stream link. Video requires analogous video nodes and links. An idle/unlinked stream is at most `Ready` with `--include-ready`; opening `/dev/video*` is only `Ready`/low confidence. PID attribution requires protocol-authenticated Client identity and stable `/proc` executable/start-time reads. PipeWire failure reports `Unavailable`; V4L2 bypass or incomplete identity remains `Degraded`. Scoped microphone control uses native PipeWire Props and owned restoration tied to the live server cookie/node serial, never numeric-node guesses or ALSA/V4L2 revocation. Process termination uses a revalidated `pidfd`; unavailable authority is an error, never a PID-only fallback.
- **macOS 15+**: Cargo compiles and embeds a private Swift helper application. CoreAudio microphone `Active` requires `AudioHardwareProcess.isRunningInput`; PID requires stable `libproc` executable/birth-time validation. Device enumeration never implies `Ready`. AVFoundation camera usage is device-level only: PID null, application unknown, enforcement `Unknown`. Camera health stays `Degraded` for own-application/noninteractive visibility gaps; helper failure is `Unavailable`, never an all-clear. Writable input-mute controls retain original values. The separate owned camera Restrictions profile requires explicit manual approval; profile metadata is not physical frame-flow proof. Termination retains native task/audit-token authority; protected processes and failed authority are refused. No camera capture probe, TCC bypass, private lock API, or PID-only kill fallback.
- **Android**: requires a separate application and permission architecture. Android does not expose a general third-party per-process capture collector, so the desktop contract must not be simulated.

PipeWire application properties are client-supplied: `/proc` alone does not authenticate their PID. Attribution additionally requires matching the owning Client's protocol-authenticated `pipewire.sec.pid`; forwarded PulseAudio/portal identity remains unknown when this cannot be established. See [PipeWire client security properties](https://docs.pipewire.org/page_man_pipewire-props_7.html#client-prop__pipewire_sec_pid).

`Snapshot.observation_gaps` is internal and separate from health: permanently degraded AVFoundation coverage can still contain complete device observations and normal START/STOP cycles. Empty camera discovery or failed CoreAudio process properties create an observation gap; the Unix watcher suppresses STOP and rebaselines after recovery instead of treating missing data as inactivity.

Unsupported backends must report unavailable capabilities rather than emit synthetic activity or confidence.

## Resource matrix

| OS / environment | Microphone active / PID | Camera active / PID | Camera ready | Blocking / notifications / tray |
| --- | --- | --- | --- | --- |
| Windows 10/11 | WASAPI sessions / validated process | Media Foundation sensor streaming and capture evidence / validated process only where available | Unconfirmed pipeline evidence | Administrator-approved camera device control, owned endpoint mute, notifications and tray |
| Linux desktop | Running PipeWire capture link / authenticated Client PID validated with `/proc` | Running PipeWire video capture link / validated authenticated PID | Idle stream or V4L2 open FD, low confidence | Session-source mute; no global camera block; native notifications and StatusNotifierItem host required |
| macOS 15+ | CoreAudio input activity / validated `libproc` PID if readable | AVFoundation other-application use / unknown PID | No camera ready inference | Writable input mute; manually approved owned camera profile; native notifications and AppKit menu bar |
| WSL / virtual or headless runners | Physical hardware and user session not guaranteed | Physical camera visibility not guaranteed | Inventory is not flow proof | No hardware assertion from CI |

Linux device inventory does not imply access permission. Direct V4L2 capture can bypass PipeWire and has no reliable frame-flow signal in this backend; `doctor` explains degraded coverage. On `serveur-asus` with PipeWire 1.0.5, a temporary virtual source and `pw-record` verified authenticated recorder PID plus idle/active/stop and watcher START/STOP; no physical microphone source was available and `/dev/video0` was denied to the SSH user. [Release dry run 36682926446](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/36682926446) passed native Windows, Ubuntu 22.04, macOS 15 arm64 and macOS 15 Intel builds and extracted-package smoke. The arm64 runner was macOS 15.7.9; the embedded helper targets macOS 15.0, with 23 macOS tests passing. Physical capture remains unverified; Linux/macOS packages in v0.14.0 are experimental.

## Frontend invariants

CLI, TUI, and tray may format, filter, and initiate explicit user controls. They do not collect evidence or reinterpret enforcement. New frontends consume the public library contract instead of importing Windows internals.

## Functional parity baseline and native targets

The public `v0.14.0` Unix packages are monitoring-only. This table describes
the `0.15.1` implementation and its runtime prerequisites, not hardware proof.

| Family | Windows baseline | Linux target / prerequisite | macOS target / prerequisite |
| --- | --- | --- | --- |
| Observation and CLI | WASAPI and Media Foundation sensor streaming; streaming can be confirmed without an attributable PID | Authenticated PipeWire graph and read-only V4L2 evidence | CoreAudio input and unattributed AVFoundation camera evidence |
| Policy and trust | Executable, publisher and path rules; offline/explicit-online Authenticode | Full verified OpenPGP primary fingerprint pin; missing/unavailable trust stays unknown | Native configured code-signing validation against trusted certificate anchors; ad-hoc integrity is not publisher trust |
| Events and storage | START/UPDATE/STOP, schema 3, JSONL, rotating history | Preserve observation-gap reconciliation, history and persistent locks | Same portable contracts; STOP retains the last observed active evidence |
| System journals | Windows Application Event Log | Native journal/syslog delivery on explicit `--eventlog` | Native Unified Logging on explicit `--eventlog` |
| Microphone controls | WASAPI endpoint mute, not an access-denial guarantee | Approved scope: session PipeWire mute/restoration, not direct ALSA blocking | Approved scope: input mute properties that are actually writable |
| Camera controls | Approved PnP disable and owned-device restoration | Global camera blocking is outside the approved limited PipeWire scope | Approved scope: manually approved camera restriction profile; pending approval is not blocked |
| TUI | Dashboard, mic/camera actions, verified double-confirmed termination | Shared dashboard consuming the existing collector; unsupported controls visibly disabled | Same dashboard, with explicit profile-approval and supported-input control scope |
| Tray / menu bar | Win32 icons, popup actions, singleton and stop/status IPC | Native StatusNotifierItem, supported desktop host and per-user IPC | Native AppKit status item and per-user IPC |
| Notifications and sound | WinRT, event cooldown, pause, optional chime | Native notification service and desktop sound availability | Native application identity, notification authorization and system sound |
| Autostart | Limited per-user Task Scheduler / Run registration | Opt-in native desktop-session registration | Opt-in per-user LaunchAgent in a graphical login session |
| Session lock and profiles | Native lock tri-state; opt-in tray actions and owned restoration | Native logind session reports; SSH without a graphical session stays unknown | Public lock state stays unknown; enabling lock policy is refused; camera approval remains manual |
| Enforcement | Active + explicit Deny, two observations, process-instance revalidation | Same safeguards and a stable pidfd before termination | Same safeguards; retained task/audit-token authority and protected-process refusals |
| Languages and help | Seven display languages, including command help | Seven-language locale detection and truthful command availability | Same; no Windows privacy/task/session descriptions |
| Distribution and update | ZIP/MSI, paired updater and autostart refresh | Public installer receipt, atomic CLI upgrade and preserved opt-in | Same-version embedded helpers/frontends, public archives and preserved opt-in |

Camera restrictions and microphone mute are different operations. Apple allows
manual installation of the [Restrictions payload](https://developer.apple.com/documentation/devicemanagement/restrictions);
[PPPC camera/microphone denials](https://support.apple.com/en-gb/guide/deployment/dep38df53c2a/web)
are per-application controls requiring device management. The user explicitly
approved limited native controls instead of a universal macOS microphone block.

The user also approved limited Linux session PipeWire controls. They do not
deny direct ALSA/V4L2 access. PipeWire [access policy](https://docs.pipewire.org/page_module_access.html)
and [node mute](https://pipewire.pages.freedesktop.org/wireplumber/man/wpctl.html)
are not kernel device revocation. The kernel documents that
[V4L2 unregister](https://docs.kernel.org/driver-api/media/v4l2-dev.html)
rejects new opens and existing file operations; changing a pathname or presenting
a suspended PipeWire node must not be advertised as that operation.

CoreAudio [`AudioHardwareProcess.devices`](https://developer.apple.com/documentation/coreaudio/audiohardwareprocess)
describes output devices. It is not evidence identifying the microphone used by
Sound or Telegram; their input-device attribution remains unknown.

## Native 0.15.1 verification boundaries

[Release dry run 37299191864](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37299191864)
passed Ubuntu 22.04/PipeWire 0.3.48 (55 tests), macOS 15.7.9 arm64 and Intel
(45 tests each), native format/Clippy, debug/release builds, extracted packages,
the SPDX bundle and provenance attestation. Apple helpers used SDK 15.5 and
Swift 6.1.2 in Swift 5 mode with deployment target 15.0.

[Tag publication 37299861471](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37299861471)
passed all three Unix package smoke runs and published only the three archives,
SPDX SBOM and checksum manifest as prerelease `v0.15.1`. Stable/latest remains
`v0.14.0`; the immutable, unshipped `v0.15.0` tag has no release/assets.
[Public installer run 37301377449](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37301377449)
passed real HTTPS installation and `v0.14.0` → `v0.15.1` upgrade with bash/zsh/fish
on all three native hosts, preserving PATH, preferences/history and failed/running
installations. Public archive/SBOM digests and exact source/tag/workflow attestations
were independently verified; `SHA256SUMS` is not separately attested.
The real SSH host also passed bash interactive/login resolution, owned upgrade and
uninstall, and managed `mcw update` without downgrading to stable `v0.14.0`, using a
private temporary prefix. Its system shell startup banner was left untouched.

Linux proof includes an authenticated virtual capture PID, independent mute
readback and owned restoration, preservation of an already-muted source, actual
PTY cancellation, a visible XFCE StatusNotifierItem and notification, real
schema-3 START delivery to journald, and no fabricated START/STOP during a real
PipeWire outage. The owned desktop entry starts and stops the actual tray.
The native sound API accepted playback through a private PulseAudio null sink;
that is not physical audibility. Screenshots were visually reviewed.

Both macOS architectures registered visible AppKit status items in real Aqua
sessions, removed them on owned stop, accepted native NSSound playback and
bootstrapped/disabled the actual owned LaunchAgent in its login environment.
This does not establish notification permission or physical capture controls.
Windows [CI 37299189617](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37299189617)
passed 99 tests, dependency audit and the native per-user MSI install/uninstall
smoke. Source validation does not lift the Windows publication hold.

The real `serveur-asus` PipeWire 1.0.5 session also passed virtual capture,
False→True→False mute, preservation of a pre-muted source, selected-row K/Esc/Q
cancellation, and human STOPPED explicitly labeled retained last observation.
No physical source or camera permission was added to that host.

The AppKit protocol acknowledges readiness only after applying the first complete
menu state. Its stdin reader uses POSIX `read`, which accepts currently available
pipe bytes instead of waiting for a full 4096-byte Foundation read. Shutdown is
scheduled in common main-run-loop modes, wakes that loop and cancels menu tracking
before removing the status item. Native popup-open/owned-stop smoke exercises
this lifecycle, including the formerly blocked small bootstrap/stop frames.

Not exercised: the reported MacBook M1 Pro/macOS 27, physical Linux/macOS
capture transitions and input mute, approved camera-profile effectiveness,
real lock/unlock actions, macOS notification authorization or physical speakers.
There is no configured Mac SSH target or self-hosted runner providing that
hardware/session. These are explicit evidence limits, not full hardware parity.

