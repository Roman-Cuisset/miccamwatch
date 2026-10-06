# Architecture

MicCamWatch has a shared Rust library, CLI, TUI and desktop lifecycle. Windows uses native capture/privacy APIs; Linux uses PipeWire and read-only V4L2 evidence; macOS 15+ uses CoreAudio, AVFoundation discovery and CoreMediaIO running-state with narrower attribution. Native implementation does not imply equal device-control coverage.

## Layers

- `model`: versioned observations, evidence, risk, enforcement decisions, and diagnostics.
- `collector`: platform-neutral `CaptureScope`, `CaptureCollector`, and capability contracts.
- `platform`: Windows uses WASAPI, Media Foundation, ConsentStore, process inspection and Authenticode. Linux uses `pw-dump` against the real PipeWire graph, `/proc` process identity and `/dev/video*` inventory/read-only FD probes. macOS uses CoreAudio `AudioHardwareSystem.processes`, AVFoundation device discovery and CoreMediaIO `kCMIODevicePropertyDeviceIsRunningSomewhere`.
- `config`, `settings`, `history`: policy, persistent preferences, rotating event storage (Windows application data; XDG paths on Unix).
- `watcher`, `notify`, `updater`: native event dispatch and effects; Unix updates require an owned public-installer receipt and preserve explicit autostart intent.
- `frontends/cli`, `frontends/tui`: shared controls and collector contracts. `frontends/tray`: Win32 Notification Area, Linux StatusNotifierItem, or macOS AppKit status item. The separate Windows tray executable requires the `windows-tray` Cargo feature; Unix desktop mode is part of `mcw`.
- `privacy/linux`, `mcw-camera-helper`: optional explicit USB `uvcvideo` driver detach/reconnect through generation-bound USBFS ioctls, authenticated on-demand Polkit root actions, root-owned identity journal, and no-prompt status. The helper binary requires `linux-camera-helper`; it is not included in default Windows/macOS builds.

The observable activity state, heuristic risk, confidence, and enforcement decision remain independent. A collector must not convert incomplete evidence into an enforcement denial.

## Unix installer boundary

`installer/install.sh` selects the published OS/CPU archive, resolves `latest`
to a concrete release tag, and verifies its named SHA-256 manifest entry before
extracting the CLI and, from Linux `0.16.0`, its same-version optional camera
helper, reviewed administrator installer and policy payload. Installation is
per-user, without `sudo`; files are staged on the destination filesystem and
validated before transactional replacement. Normal prefix installation never
overwrites unmanaged, running, externally modified, symlinked or hardlinked artifacts.

From macOS `0.16.1`, an explicitly selected standalone `mcw` outside `bin` can
be updated in place without claiming installer ownership. Rust inspection pins
its safe owned parent/file identity and SHA-256 before positional-argument exec
handoff. The embedded installer rechecks the expected digest under its install
lock, validates the same-volume staged CLI/package and refuses another running
target. Original rwx permissions and quarantine are retained before staged
execution; no Gatekeeper or TCC bypass is performed. No receipt or PATH change
is made. A removed/disabled autostart registration stays absent/disabled.
Failure rollback restores only an unchanged owned new target; a concurrent
change or quarantined/missing file is preserved with the old backup for manual
inspection. SIGKILL cannot run cleanup: a verified old/new executable and
possibly a stale lock/backup require inspection. `bin/mcw` never falls back to
this mode on receipt errors; Linux still requires the managed layout.

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

Linux camera payloads belong under `PREFIX/share/miccamwatch/linux-camera`,
with checksums in the existing data-only receipt. Managed update validates all
required payloads and preserves a usable old installation on failure. It never
updates `/usr/local/libexec/miccamwatch/mcw-camera-helper` implicitly.
Administrator setup is a separate reviewed `install-camera-helper.sh` action:
an explicit trusted archive digest, private root-copy checksum verification,
exact CLI/helper protocol/version pairing, fixed root-owned helper/policy paths
and unchanged-file receipts. Setup performs no device action or autostart.
User update/reinstall/uninstall refuses any root helper installation presence,
including malformed/untrusted remnants; an `Allowed` driver state is not an
empty-journal lifecycle guarantee. The ordered procedure is old matching CLI
explicit allow, reviewed root-helper uninstall, ordinary user update/remove,
then explicit new same-version root setup if desired. Root removal refuses
nonempty/malformed evidence. Explicit root uninstall transactionally removes only
integrity-verified empty journal/cache files and preserves the permanent
operation-lock inode; stale version-specific empty status does not survive the
cutover. User uninstall never removes root files.
Concurrent administrator reinstallation during the user lifecycle is outside
this procedure. No implicit elevation or compatibility shim is used.

Root installation prepares all rollback backups and the complete manifest in a
private uniquely named root-owned directory, fsyncs them, then atomically
publishes the fixed transaction directory and fsyncs state before changing
installed files. An interrupted unpublished preparation cannot poison the fixed
recovery path; a published durable transaction supports rollback.

The Polkit action `com.roman-cuisset.miccamwatch.camera` authorizes only the fixed
root-owned helper; it never elevates `mcw`, a user-prefix helper or a shell.
Bounded protocol actions contain no device path, command or chosen UID; caller
identity comes from authenticated `PKEXEC_UID`. Root mutation serializes on the
operation lock under `/var/lib/miccamwatch`. Journal/lock are root-only0600.
The helper revalidates its fixed executable after acquiring this lock; an already
mapped process cannot mutate devices after administrator uninstall.
The readable root-owned status cache cannot authorize a privileged mutation.
All executable/state ancestors and files are checked against symlink, hardlink,
ownership and mode substitution. Version mismatch, cancellation, denial,
unsupported devices and partial kernel changes remain truthful failures.


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
- **macOS 15+**: Cargo compiles and embeds a private Swift helper application linked with CoreMediaIO. CoreAudio microphone `Active` requires `AudioHardwareProcess.isRunningInput`; PID requires stable `libproc` executable/birth-time validation. Device enumeration never implies `Ready`. AVFoundation camera UIDs are translated through `kCMIOHardwarePropertyDeviceForUID` using `AudioValueTranslation`; CoreMediaIO [`kCMIODevicePropertyDeviceIsRunningSomewhere`](https://developer.apple.com/documentation/coremediaio/kcmiodevicepropertydeviceisrunningsomewhere) supplies a `UInt32` device running-state (zero idle, nonzero active). Both native calls require successful status and exact output sizes; an unknown translated device is refused. Swift `runningSomewhere: Bool?` maps to Rust `Option<bool>`: null/missing/read failure means unknown, not false. Inventory mode skips running-state queries. Camera evidence/collector is `coremediaio_video`, medium confidence, PID null, application unknown, enforcement `Unknown`; no frames, client attribution or per-app enforcement follow from device activity. Camera health stays `Degraded` for attribution/frame-flow/coverage limits; helper failure is `Unavailable`, never an all-clear. Writable input-mute controls retain original values. The separate owned camera Restrictions profile requires explicit manual approval; profile metadata is not physical frame-flow proof. Termination retains native task/audit-token authority; protected processes and failed authority are refused. No capture session, permission request, TCC bypass, private lock API, or PID-only kill fallback.
- **Android**: requires a separate application and permission architecture. Android does not expose a general third-party per-process capture collector, so the desktop contract must not be simulated.

PipeWire application properties are client-supplied: `/proc` alone does not authenticate their PID. Attribution additionally requires matching the owning Client's protocol-authenticated `pipewire.sec.pid`; forwarded PulseAudio/portal identity remains unknown when this cannot be established. See [PipeWire client security properties](https://docs.pipewire.org/page_man_pipewire-props_7.html#client-prop__pipewire_sec_pid).

`Snapshot.observation_gaps` is internal and separate from health: permanently degraded CoreMediaIO coverage can still contain complete device running-state observations and normal START/STOP cycles. Empty camera discovery, null/missing camera state, failed CoreMediaIO translation/property reads or failed CoreAudio process properties create an observation gap; the Unix watcher suppresses STOP and rebaselines after recovery instead of treating missing data as inactivity. Known active cameras remain visible during partial errors, without authorizing unknown-owner enforcement.

Unsupported backends must report unavailable capabilities rather than emit synthetic activity or confidence.

## Resource matrix

| OS / environment | Microphone active / PID | Camera active / PID | Camera ready | Blocking / notifications / tray |
| --- | --- | --- | --- | --- |
| Windows 10/11 | WASAPI sessions / validated process | Media Foundation sensor streaming and capture evidence / validated process only where available | Unconfirmed pipeline evidence | Administrator-approved camera device control, owned endpoint mute, notifications and tray |
| Linux desktop | Running PipeWire capture link / authenticated Client PID validated with `/proc` | Running PipeWire video capture link / validated authenticated PID | Idle stream or V4L2 open FD, low confidence | Session-source mute; explicitly authorized USB `uvcvideo` controls with separate root setup; non-USB unsupported; native notifications and StatusNotifierItem host required |
| macOS 15+ | CoreAudio input activity / validated `libproc` PID if readable | CoreMediaIO device running-state / unknown PID | No camera ready inference | Writable input mute; manually approved owned camera profile; native notifications and AppKit menu bar |
| WSL / virtual or headless runners | Physical hardware and user session not guaranteed | Physical camera visibility not guaranteed | Inventory is not flow proof | No hardware assertion from CI |

Linux device inventory does not imply access permission. Direct V4L2 capture can bypass PipeWire and has no reliable frame-flow signal in this backend; `doctor` explains degraded coverage. On `serveur-asus` with PipeWire 1.0.5, a temporary virtual source and `pw-record` verified authenticated recorder PID plus idle/active/stop and watcher START/STOP; no physical microphone source was available and `/dev/video0` was denied to the SSH user. [Release dry run 36682926446](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/36682926446) passed native Windows, Ubuntu 22.04, macOS 15 arm64 and macOS 15 Intel builds and extracted-package smoke. The arm64 runner was macOS 15.7.9; the embedded helper targets macOS 15.0, with 23 macOS tests passing. Physical capture remains unverified; Linux/macOS packages in v0.14.0 are experimental.

## Frontend invariants

CLI, TUI, and tray may format, filter, and initiate explicit user controls. They do not collect evidence or reinterpret enforcement. New frontends consume the public library contract instead of importing Windows internals.

## Functional parity baseline and native targets

The public `v0.14.0` Unix packages are monitoring-only. This table describes
the current source implementation and its runtime prerequisites, not hardware proof.

| Family | Windows baseline | Linux target / prerequisite | macOS target / prerequisite |
| --- | --- | --- | --- |
| Observation and CLI | WASAPI and Media Foundation sensor streaming; streaming can be confirmed without an attributable PID | Authenticated PipeWire graph and read-only V4L2 evidence | CoreAudio input and unattributed CoreMediaIO camera running-state |
| Policy and trust | Executable, publisher and path rules; offline/explicit-online Authenticode | Full verified OpenPGP primary fingerprint pin; missing/unavailable trust stays unknown | Native configured code-signing validation against trusted certificate anchors; ad-hoc integrity is not publisher trust |
| Events and storage | START/UPDATE/STOP, schema 3, JSONL, rotating history | Preserve observation-gap reconciliation, history and persistent locks | Same portable contracts; STOP retains the last observed active evidence |
| System journals | Windows Application Event Log | Native journal/syslog delivery on explicit `--eventlog` | Native Unified Logging on explicit `--eventlog` |
| Microphone controls | WASAPI endpoint mute, not an access-denial guarantee | Approved scope: session PipeWire mute/restoration, not direct ALSA blocking | Approved scope: input mute properties that are actually writable |
| Camera controls | Approved PnP disable and owned-device restoration | Explicitly approved USB video-class `uvcvideo` detach and exact owned reconnect via pinned-generation USBFS; matching root helper/Polkit and kernel generation metadata required; non-USB unsupported | Approved scope: manually approved camera restriction profile; pending approval is not blocked |
| TUI | Dashboard, mic/camera actions, verified double-confirmed termination | Shared dashboard, explicit authorized USB actions; honest capability/state/setup diagnostics | Same dashboard, with explicit profile-approval and supported-input control scope |
| Tray / menu bar | Win32 icons, popup actions, singleton and stop/status IPC | Native StatusNotifierItem, supported desktop host and per-user IPC | Native AppKit status item and per-user IPC |
| Notifications and sound | WinRT, event cooldown, pause, optional chime | Native notification service and desktop sound availability | Native application identity, notification authorization and system sound |
| Autostart | Limited per-user Task Scheduler / Run registration | Opt-in native desktop-session registration | Opt-in per-user LaunchAgent in a graphical login session |
| Session lock and profiles | Native lock tri-state; opt-in tray actions and owned restoration | Native logind microphone actions; SSH without graphical session stays unknown; automatic camera-on-lock refused because authorization must be explicit before locking | Public lock state stays unknown; enabling lock policy is refused; camera approval remains manual |
| Enforcement | Active + explicit Deny, two observations, process-instance revalidation | Same safeguards and a stable pidfd before termination | Same safeguards; retained task/audit-token authority and protected-process refusals |
| Languages and help | Seven display languages, including command help | Seven-language locale detection and truthful command availability | Same; no Windows privacy/task/session descriptions |
| Distribution and update | ZIP/MSI, paired updater and autostart refresh | Public installer receipt, same-version CLI/helper/admin assets and preserved opt-in; root-helper refresh separate and explicit | Same-version embedded helpers/frontends, public archives and preserved opt-in |

Camera restrictions and microphone mute are different operations. Apple allows
manual installation of the [Restrictions payload](https://developer.apple.com/documentation/devicemanagement/restrictions);
[PPPC camera/microphone denials](https://support.apple.com/en-gb/guide/deployment/dep38df53c2a/web)
are per-application controls requiring device management. The user explicitly
approved limited native controls instead of a universal macOS microphone block.

The user approved limited Linux session PipeWire controls and separately
administrator-authorized USB camera controls. PipeWire
[access policy](https://docs.pipewire.org/page_module_access.html) and
[node mute](https://pipewire.pages.freedesktop.org/wireplumber/man/wpctl.html)
are not kernel device revocation and do not deny direct ALSA/V4L2 access.
The camera helper instead journals exact original identities before targeted
`USBDEVFS_IOCTL(DISCONNECT)` of `uvcvideo` interfaces, and restores through
`USBDEVFS_IOCTL(CONNECT)`. Mutation is bound to a pinned `/dev/bus/usb` device FD,
with zero claimed interfaces. `USBDEVFS_CONNINFO_EX` supplies generation-bound
metadata (kernel 5.9+; documented Linux 5.15+ baseline), followed by independent
driver/generation readback. Sysfs provides read-only inventory/status; mutation
never uses reusable sysfs interface names as write targets, and unsupported
generation metadata is refused rather than using an unsafe fallback.
Restoration reconnects only owned unchanged identities, including descriptor/serial,
topology, boot and device/interface generation checks; unplug/replug or replacement
cannot silently restore another camera. Explicit allow retires records only from
a prior boot or a proven vanished/replaced parent USB device generation, without
touching replacement devices; changed interface inodes alone do not justify
retirement. Partial or stale state is not a global block. USB audio/storage interfaces, device-node
permissions and global modules are untouched; no interface claims, URBs, reset
or automatic close-time reattach are used. Newly connected cameras are not
automatically blocked.
For a new camera, mixed `uvcvideo`-bound/unbound video-class siblings are treated
as externally managed and refused before intent recording or mutation. Fully
bound original UVC arrangements are supported; initially all-unbound cameras
are untouched. Restoration never claims a sibling interface to manufacture a
binding arrangement.
Immutable full descriptors admit exactly one USB configuration. Every alternate
of a selected interface number must remain video class `0x0e` with the same
expected control/streaming subclass. Damaged, duplicate or ambiguous descriptor
maps and current-configuration-number mismatches are rejected before intent or
mutation. Multi-configuration/role-changing cameras have no fallback. A
single-configuration composite with separate audio/storage interface numbers
remains supported, with those unrelated interfaces untouched.
The kernel documents that [V4L2 unregister](https://docs.kernel.org/driver-api/media/v4l2-dev.html)
rejects new opens and existing file operations; a pathname permission change or
suspended PipeWire node must not be advertised as that operation.
Native hosted CI builds/packages the paired helper without root installation or
device mutation. Physical USB capture-denial/restoration proof is still required.

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

Friend-reported evidence from **[@repentandliveholy](https://github.com/repentandliveholy)** on a
MacBook Pro with Apple Silicon/macOS 27: Telegram circle recording kept the old
AVFoundation activity property false; a passive CoreMediaIO probe transitioned
0 → 1 → 0, and their locally patched release reported FaceTime HD Camera
START/STOP. The production `0.16.0` source integrates this correction without
modifying published `v0.15.1` or publishing their raw logs.

Not exercised by maintainers or hosted CI: that macOS 27 hardware/session,
physical Linux input mute and macOS capture/mute transitions, approved camera-profile
effectiveness, real lock/unlock actions, macOS notification authorization or
physical speakers. There is no configured Mac SSH target or self-hosted runner
providing that hardware/session. The revised helper passed native macOS 15.7.9
arm64/Intel builds, regressions and real CLI/TUI/menu-bar smoke with SDK 15.5
in the [successful three-platform dry run 37425122074](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37425122074).
Separate system and MCW inventories both reported zero cameras; these runs do
not establish physical/macOS 27 parity.

## Native 0.16.0 software verification boundaries

The latest successful [dry run 37429480145](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37429480145) used source
`8be1f1b78a907fbeaa47c60e3995369974212588`; it generated and attested a
candidate Unix bundle, **not a public release**. Ubuntu 22.04 passed Clippy
with warnings denied, 74 library tests plus one CLI test, debug/release builds,
and real extracted CLI/PTY/desktop lifecycle. The Linux StatusNotifier menu
and capture notification were visually inspected. PipeWire capture/mute
evidence uses an isolated virtual source, not a physical microphone.

The genuine six-member archive also passed ordinary-user payload install,
update, rollback and removal checks, explicit administrator bootstrap,
same-version refresh and owned uninstall. Real strace SIGTERM/SIGKILL
interrupted archive copying, preparation, transaction publication and
completion; recovery/orphan removal retained the permanent lock inode.
Root-presence checks refused user install/update/uninstall before release
transport and preserved existing assets. Wrong archive digests were refused
before any archive executable ran. Invalid protocol/version/action/caller
requests preserved journal/cache/device state.

Administrator read-only status used explicit sudo and the real runner UID,
**not an interactive Polkit authentication dialogue**. No supported USB
camera was present: the helper returned a real unavailable error, never a
fake Allowed state. No block/allow/toggle or physical camera mutation ran
on hosted runners. The separate physical Linux checks below establish
detach/revocation, restoration/capture and cancelled Polkit approval on one
supported webcam. No non-USB control, hotplug reblock or camera-on-lock is claimed.

The [public installer run 37422717465](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37422717465)
passed real `v0.14.0` to `v0.15.1` upgrades on Linux x64 and macOS ARM/Intel
with bash/zsh/fish. Linux hosted system directories were observed as root-owned
but mode 0777; only disposable hosted VM prerequisites secure the fixed
ancestry. Production installer safety checks were not weakened.

### Authorized Linux physical verification

The same exact `e4886c3` Linux archive was SHA-256 checked locally and after
SSH transfer; GitHub provenance verified the release workflow, full source
digest, branch ref and hosted runner. Its archive digest was
`2fb1dbabeb5f297317c3fc99c32164b2cc3485646573a3f40b58d07541e4b86a`.
The host used kernel `7.0.0-28-generic`, PipeWire `1.0.5` and ordinary UID 1000.
USB `13d3:5a11` had one configuration and two UVC interfaces,
`1-6:1.0` (control) and `1-6:1.1` (streaming), both originally `uvcvideo`.

The reviewed root installer installed only its authenticated fixed payload.
Cancelling real Polkit approval left every USB binding and video node unchanged.
An ordinary-user `mcw camera block` with fresh Polkit administrator approval
then detached those two interfaces only. Ongoing FFmpeg V4L2 capture terminated
with `VIDIOC_DQBUF: No such device`, not a timeout. Both video nodes disappeared;
a new capture attempted as root failed with `No such file or directory`.
Unprivileged `mcw camera status` read back `blocked` without authorization.
USB hubs, Bluetooth and Ethernet bindings were unchanged.

Fresh Polkit approval for ordinary-user `mcw camera allow` restored the exact
original interface drivers and video nodes; FFmpeg captured five YUYV 640x480
frames to the null output again. No images were saved. A separate explicit-sudo
cycle also verified repeated block ownership. The original-state journal was
empty after allow. Reviewed root uninstall then removed only unchanged owned
helper/policy/receipt and validated empty journal/cache, preserving the permanent
operation-lock inode and all restored device bindings.

The host's internal `pkexec` text agent failed with `No session for cookie`,
including a native read-only invocation outside MCW. Restarting the daemon did
not fix it. Successful CLI actions used a standard unprivileged `pkttyagent`
registered for the CLI PID and the genuine policy/password challenge; no
authorization rule, PAM file, account or permission was changed. This is proof
with a functioning registered agent, not proof that this host's internal agent
or every graphical authentication dialogue works.

No physical microphone was exported in this host's MCW inventory. Real
lock/unlock and speaker audibility were not established from SSH; the active
seat was a display-manager greeter, not a logged-in graphical test user.

### Socket-activated PipeWire mute verification

The same hardware host exposed its connected socket's `SO_PEERCRED` PID as
the systemd user manager, not the PipeWire daemon. The previous
session-control probe failed resolving `/proc/1021/exe`, even though graph
access and the peer UID were valid. Source `8be1f1b` verifies this socket
creator's UID and PID/starttime twice without requiring an executable.
Boot identity, socket device/inode, native core cookie and node serial still
pin restoration ownership. Full executable/process verification for capture
attribution and enforcement remains unchanged; no `/proc` permissions or
systemd/PipeWire configuration was relaxed.

The corrected Linux archive digest was
`df7c4700ad34d1c141dbdf806c044f15972de511f80460d48b9b82a55383d966`;
all bundle digests matched, Linux provenance verified its full source/ref,
and the transferred archive matched again. Extracted CLI `doctor` changed
from an error to exit 0 with working source-mute controls. An owned silent
virtual source independently read back **false → true → false** through
`pw-dump`. Starting with that source already muted, `mcw mute`/`mcw unmute`
correctly left it muted. Final capabilities retained zero original states
and zero pending restorations. The virtual source and private candidates
were removed; existing user configuration and physical inputs were untouched.
This verifies PipeWire signal mute, not physical microphone denial/audibility.

A permanent regression uses a real non-dumpable child process and requires
peer-generation verification to succeed while rejecting the wrong UID.
Linux passed 75 total library/CLI tests and the real desktop/PTY smoke;
Apple Silicon and Intel also passed their native regression/build/smoke jobs.

## Stable v0.16.0 publication and public consumers

PR #2 was merged into `main` as `0e7cc9c97d6cb926ad61ab2b26a25ae85096638a`.
The owner subsequently authorized complete stable publication, including
Windows; `WINDOWS_RELEASE_APPROVED=true` is approval, not a signing certificate.
Tag `v0.16.0` fixes source `9515fb664ac0c75601ebf6b8321b6954666faec9`.
[Main CI 37435307905](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37435307905)
passed all four native jobs before tagging.
[Release 37437092551](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37437092551)
then built/exercised Windows ZIP/MSI and three Unix packages, generated SPDX,
checksums and attestations, and published stable latest on 2026-10-06.

Public `/releases/latest` returned `v0.16.0`, release ID `404493096`,
`draft=false`, `prerelease=false`, and all seven expected assets. Downloads
matched API digests; six payload/SBOM entries matched `SHA256SUMS` and
attestations constrained to the exact source, `refs/tags/v0.16.0`, release
workflow and hosted runner. The manifest itself is not separately attested.
Old assets/tags were not replaced.

Public current-installer migrations from
[v0.14.0](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37438076601)
and [v0.15.1](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37438076504)
passed on Linux x64 and macOS ARM/Intel with bash/zsh/fish.
The real Linux SSH host also installed unpinned stable latest without root,
device mutation, PATH edits or changes to existing user configuration.

**Legacy Linux updater boundary:** v0.15.1 embeds its three-member installer
and rejects the new six-member Linux archive before replacing its installation;
the actual failed call retained CLI 0.15.1. Re-running the current public
installer at that same managed prefix migrated to 0.16.0 with matching user
camera payload. The new CLI's `mcw update` then reported up-to-date.
Immutable older clients cannot acquire the new embedded parser automatically.

A real Windows 0.15.1 pair in a private portable prefix updated from public
latest to the exact downloaded 0.16.0 CLI/tray bytes; the second call was
up-to-date and existing HKCU startup values were preserved. Native hosted
ZIP tray and MSI install/remove smoke passed. Both EXEs and MSI independently
reported `NotSigned`; local Defender completed its exact custom scan with
active protections and no detection/remediation events in the observed window.
See [signing evidence](SIGNING.md); this is not universal antivirus clearance.

