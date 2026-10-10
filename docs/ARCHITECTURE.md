# Architecture

MicCamWatch has a shared Rust library, CLI, TUI and desktop lifecycle. Windows uses native capture/privacy APIs; Linux uses PipeWire and read-only V4L2 evidence; macOS 15+ uses CoreAudio, AVFoundation discovery and CoreMediaIO running-state with narrower attribution. Native implementation does not imply equal device-control coverage.

## Layers

- `model`: versioned observations, evidence, risk, enforcement decisions, and diagnostics.
- `collector`: platform-neutral `CaptureScope`, `CaptureCollector`, and capability contracts.
- `platform`: Windows uses WASAPI, Media Foundation, ConsentStore, process inspection and Authenticode. Linux uses `pw-dump` against the real PipeWire graph, `/proc` process identity and `/dev/video*` inventory/read-only FD probes. macOS uses CoreAudio `AudioHardwareSystem.processes`, AVFoundation device discovery and CoreMediaIO `kCMIODevicePropertyDeviceIsRunningSomewhere`.
- `config`, `settings`, `history`: policy, persistent preferences, rotating event storage (Windows application data; XDG paths on Unix).
- `watcher`, `notify`, `updater`: native event dispatch and effects; managed Unix updates require an owned public-installer receipt, macOS also supports verified standalone replacement outside `bin`, and both preserve explicit autostart intent.
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
The private recovery copy also preserves the original quarantine bytes exactly:
macOS `cp -p` rewrites this attribute, so the installer reinstates and checks
the original value on that copy and refuses a concurrent source-attribute change.
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

## Candidate 0.17.0 Windows-first protection contract

This section describes candidate source only. Public stable/latest remains
immutable `v0.16.1`, with its existing installation URLs, hashes and historical
proofs unchanged. Windows-first persistent protection was explicitly chosen;
Linux/macOS remain scoped one-shot controls, not substitutes for this owner.
P1-RAM/P1-RESTORE are implemented in candidate source but remain **in progress**
pending native qualification. No signing/notarization or publication is claimed.

### Intent, effect and observation

Requested protection is an owned intent, SDK mute/PnP state is an effective
control readback, and observed capture is collector evidence. They are distinct:
muted does not mean capture access is denied, and no capture observation is not
proof of enforced protection. Windows microphone control operates on capture
endpoint mute, not electrical isolation, exclusive/ASIO/direct-driver denial or
a universal hardware mute. Hardware mute capability is not proof of such denial.

Manual microphone protection is owned by MCW's native background broker until
explicit release, even after CLI, `top` or tray exit. It is launched on demand;
there is no implicitly installed login service, autostart or promise of reboot
persistence. Automatic owners record each stable endpoint's original mute,
generation, applied/readback state and intention token. Restoration uses this
record, never an aggregate `Mixed`/`Muted` boolean applied to all endpoints.
Generation changes, disconnects, hotplug and partial failures do not become
silent success. A newer manual intent takes precedence over an older automatic
restore. When release cannot safely finish, original/ownership evidence remains
release-pending; dropping a UI or request is not permission to erase it.

Automatic restoration relinquishes ownership after observable external changes.
Manual persistent protection deliberately reasserts mute until explicit MCW
release; an application's unmute does not deactivate the requested protection.
CoreAudio's event-context
GUID is advisory, not authenticated caller identity. `SetMute` of the same
value can return `S_FALSE` without emitting a callback. The SDK cannot identify
an exact Ktalk/Audition actor or detect every invisible same-valued external
intent. Generation/readback/manual precedence are safeguards, not an absolute
ownership or security boundary against other software or administrators.
Missing helper endpoint is normal only when no protection is requested, no
automatic token exists and no resource conflict is present; requested-owner
failures and other native transport errors stay explicit.

The [IAudioEndpointVolume SDK contract](https://learn.microsoft.com/en-us/windows/win32/api/endpointvolume/nn-endpointvolume-iaudioendpointvolume)
distinguishes hardware and software controls: advertised hardware mute affects
shared/exclusive endpoint streams, while software-only mute affects shared mode
and is bypassed in exclusive mode. `QueryHardwareSupport` reports capability,
not actual capture denial. Direct-driver/ASIO paths are not certified by that
readback. MCW's reassertion is reactive, not an atomic permission boundary or a
guarantee of zero intervening audio frames.


### Global Windows camera ownership

Global Block/Allow is an intention independent of discovered count, including
zero, one or 1,000 cameras. Explicit Block requests UAC for a temporary native
background helper, which maintains the requested block for supported present
devices and future arrivals. UI exit does not release this manual intent.
Allow clears the global intent and restores only the journaled protected owned
changes. It is not "enable all cameras": externally disabled or initially
disabled devices are not borrowed as MCW-owned changes.

Inventory is current observation, not a retained list of expected cameras:
an absent camera is neither an expected inventory entry nor an error by itself.
Owned restoration evidence is separate. Device arrival and subsequent SDK
enforcement have a brief window; restart-required changes, SDK vetoes, unknown
status and partial failure remain reported. Intent alone cannot promise instant
capture revocation, successful driver restart or verified denial.

Ownership evidence is in an administrator-protected journal. Unsigned legacy
user-writable ownership is not imported into elevated authority. Explicit
`mcw camera allow --restore-legacy INSTANCE_ID` instead requests fresh UAC for
that one target, validates its camera class and reports the actual result.
It is general recovery, not an automatic unsigned-journal migration, arbitrary
device-enable RPC or hardcoded Logitech privilege exception.

The temporary helper requests no new console through `SEE_MASK_NO_CONSOLE`,
then clears/closes unique inherited standard handles and calls `FreeConsole`
after validating its administrator token and invoker. `FreeConsole` alone does
not release redirected pipes inherited through the shell launch.
It uses IPC, not a hidden console host, and must not share the caller's
console-close lifetime. `SEE_MASK_NOASYNC` completes shell launch before the
short-lived caller exits. The invoker's console is not destroyed by detachment.


### Shared native control boundary

Microphone/camera owners use the shared fixed-capability native IPC transport.
Endpoints bind the exact user SID, session and hash of the operationally scoped
data directory; native peer tokens/processes are checked rather than trusting
caller-supplied PID/name fields. Retained process handles and creation times
authenticate process birth, preventing recycled numeric PIDs from authorizing
an invoker. Native resource ownership refuses conflicting SDK mutations across
owners/scopes.

Normal RPC has typed fixed actions, no arbitrary path, device, command or shell
payload. Frames are bounded (4,096 B request, 65,536 B response), with bounded
three-second transport phases and a bounded reply acknowledgement before
disconnect. Malformed/untrusted clients are disconnected without terminating
the owner. Alternate-administrator UAC delegation permits QUERY-only native
process/token inspection needed to validate the invoker; it grants neither
token use/adjustment, termination nor general privileged authority.

The microphone owner starts detached with native handle inheritance disabled.
Redirecting Rust `Command` standard streams to NUL is insufficient on Windows:
other inherited caller pipes can keep a completed CLI's output open. Operational
scope files and locks stay outside the directory uploaded as native CI evidence.

Updates must not implicitly release protection to replace a locked executable.
Explicitly release microphone and camera intent and complete pending restoration
before replacing binaries, including portable, MSI/package-manager or manual
replacement. Merely closing the tray/top is insufficient. A missing readiness
proof or a failed/pending release must not be described as safe ownership loss.

Candidate portable Windows `mcw update` obtains the camera and microphone
request locks and both validated global native `ResourceLease` reservations
before tray shutdown or installed-byte swaps. It refuses active/foreign owners,
requested manual intent, automatic microphone token/generation, pending
microphone release/restore, desired global camera Block, and protected camera
journal requested/owned/unfulfilled receipts or unreadable/corrupt/unsupported
records. Unsigned legacy `restore_on_arrival` owed-only history alone neither
authorizes elevated changes nor blocks an otherwise permitted update: it is
preserved unchanged for explicit legacy recovery. Readiness inspection is
read-only: it constructs no capture/trust collector, starts no helper, changes
no SDK state and never selects Allow/unmute or clears intent.

Reservations cover replacement and rollback, are released before restart and
reacquired before rollback following restart failure. If protection resumed,
rollback refuses and keeps backups rather than disturbing the new owner.
Same-version already-matching CLI/tray no-op skips replacement reservations;
explicit tray stop remains unchanged and is not release. A delayed unarmed
helper after startup timeout can temporarily make ownership busy; this is not
proof of successfully armed protection, and it cannot receive mismatched-version
intent or write a false default. External MSI upgrades and manual/package-manager
replacement bypass this portable-updater mechanism: no MSI enforcement or
permission action is claimed. Explicit prior release remains the procedure.


### Bounded collection and frontends

The candidate Linux `pw-dump` reader parses the graph as a bounded stream rather
than retaining a full raw document plus `Vec<Value>`. Child stdout/stderr,
retained graph data and deadlines are bounded; oversized, malformed, stalled
or failed child output becomes explicit unavailable/degraded health and an
observation gap. Never silently truncate captures or infer STOP from that gap;
recovery rebaselines. Identity evidence is not discarded to make RSS attractive.

UI delivery queues, notification work, cooldown and identity caches are bounded.
Refresh can coalesce where appropriate; control/failure state stays truthful.
Read-only camera refresh uses the coalesced observation worker, not the joined
action worker. The latter accepts only actual mutations; Quit never waits for
a read-only refresh to finish. Camera reads started before a completed mutation
cannot overwrite that newer state, including failed/partial completions.
The shared TUI has one horizontal row of six controls, with compact labels at
narrow widths rather than wrapping the controls into multiple rows. Narrow
representations retain shortcuts, capability state and mouse hitboxes. These
bounds do not prove every native OS memory/latency budget.

### Local candidate evidence and remaining qualification

Observed in the private Windows clone at source `fe86151`: strict feature Clippy,
both native unit suites and the optimized build passed, with 160 library + 1 main
feature tests and 1 ignored visual test. This includes normal missing-owner status,
native error preservation and portable-update reservations. Eight real native IPC
tests passed after queued-overlapped ownership and bounded response-ACK fixes.
An isolated native process fixture reproduced the completed parent's pipes
remaining open until its five-second child exited. The non-inheriting native
launch released both pipes in 0.282 seconds with the child still alive.

Actual native ConPTY `top` passed seven languages at 120/150/40/20 columns with
refresh, mouse and quit. The outside-Quit hitbox was tested at 150 columns;
the rendered French 150-column screenshot was visually inspected: six compact
bordered controls on one row, no overflow. The read-only host inventory has one
active microphone advertising hardware mute and one present camera. No
physical endpoint/capture/device setting was changed by this candidate proof.

The latest completed read-only local resource comparison used the `fe86151`
candidate binary (SHA-256 `059c06062f814ad12488f4a7d2d9efb84d86e51f11d98c94079a201e371e429e`)
and checksum-verified immutable `v0.16.1`. Functional/completed were true;
**15,000,000 B is unmet**. Values below are decimal MB; CPU is process-tree
CPU seconds over the approximately 20-second sustained interval after two
seconds of warm-up. No active protection guard or tray was included.

| Product / profile | Sustained RSS median MB | RSS p95 MB | Sampled concurrent peak MB | OS root peak MB | Sustained CPU seconds |
| --- | ---: | ---: | ---: | ---: | ---: |
| v0.16.1 watch idle | 17.805 | 17.965 | 23.769 | 23.880 | 2.078 |
| 0.17.0 watch idle | 20.079 | 20.713 | 25.653 | 26.206 | 2.141 |
| v0.16.1 top idle | 19.091 | 19.419 | 24.068 | 25.141 | 2.453 |
| 0.17.0 top idle | 21.123 | 21.512 | 26.350 | 27.111 | 2.484 |
| v0.16.1 top refresh burst / slow consumer | 19.227 | 19.800 | 23.306 | 25.330 | 3.000 |
| 0.17.0 top refresh burst / slow consumer | 21.152 | 21.742 | 25.027 | 27.193 | 3.109 |

Three status runs had OS root peaks of 21.373–21.385 MB for the baseline
and 21.672–21.742 MB for the candidate, with complete validated JSON observed
in 0.781–0.797 seconds for the candidate. Top's first native output arrived in
0.109–0.125 seconds; the rendered Quit control was validated after the harness's
three-second drain, not a measured first-frame/physical-event deadline.
Watch exposes no readiness protocol; survival is not proof of a first scan.
Maximum candidate sampling gaps were approximately 63 ms, not at most 50 ms.
Short-lived children and between-sample peaks may escape observation; OS peak,
sampled concurrent peak and sustained values are not interchangeable.

Qualification must account for the entire product process tree, including
native guard/tray/embedded helper and transient `pw-dump`/sound children, and
report shared resident pages without pretending summed RSS is unique physical
memory. A tree sum can count shared pages repeatedly; root-only RSS can omit
children. Report both the accounting method and known sampling gaps. The soft
target is 15,000,000 B; the approximately 30 MB reference is not a hard cutoff.
Neither bounded code nor these local numbers establish optimized sub-target RAM.

A private Linux PipeWire fixture actually proved external unmute remained
after five seconds; the fixture was cleaned, without physical audio access.
The macOS candidate has source-scoped limitation evidence only, not new physical
proof. Material multi-endpoint, hotplug, exclusive/ASIO capture and camera
denial/owned restoration remain unverified on physical devices. Historical
v0.16.1 publication and earlier hardware evidence do not certify this candidate.

### Native candidate qualification

[CI 38037123382](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38037123382)
passed at source `fe86151362bd4df71a9c6040dcbb99432aaf8bcd` on Windows,
Linux x86_64 and macOS ARM/Intel. Actual CLI, seven-language TUI, native
tray/menu/details, packaging and Windows MSI install/uninstall passed.
Resource comparisons against checksum-verified immutable `v0.16.1` completed
and passed their functional checks on all four runners. The following are
sampled whole-product-tree RSS, decimal MB; CPU is the observed tree delta over
the approximately 20-second sustained window, not a physical-event deadline.

| Runner / profile | v0.16.1 median MB | Candidate median MB | v0.16.1 concurrent peak MB | Candidate concurrent peak MB | Observed CPU seconds baseline / candidate |
| --- | ---: | ---: | ---: | ---: | ---: |
| Windows watch | 12.743 | 14.934 | 12.968 | 15.167 | 0.734 / 0.672 |
| Windows top idle | 13.779 | 15.909 | 14.021 | 16.175 | 0.672 / 0.891 |
| Windows top burst / slow consumer | 13.824 | 15.942 | 14.090 | 16.105 | 1.281 / 1.516 |
| Windows tray | 15.405 | 16.470 | 25.113 | 24.752 | 0.281 / 0.281 |
| Linux watch | 6.291 | 5.767 | 6.554 | 5.898 | 0.100 / 0.070 |
| Linux top idle | 10.269 | 10.039 | 10.473 | 10.826 | 0.390 / 0.320 |
| Linux top burst / slow consumer | 10.318 | 10.113 | 11.227 | 10.936 | 0.550 / 0.410 |
| Linux tray | 12.685 | 12.583 | 12.685 | 12.640 | 0.080 / 0.060 |
| macOS ARM watch | 5.407 | 5.374 | 18.760 | 17.416 | 0.025 / 0.009 |
| macOS ARM top idle | 6.537 | 6.521 | 25.117 | 20.152 | 0.018 / 0.014 |
| macOS ARM top burst / slow consumer | 12.304 | 6.521 | 25.805 | 18.596 | 0.046 / 0.017 |
| macOS ARM menu bar | 33.554 | 33.522 | 45.482 | 45.515 | 0.014 / 0.006 |
| macOS Intel watch | 3.670 | 3.690 | 12.247 | 12.337 | 1.867 / 2.574 |
| macOS Intel top idle | 4.772 | 4.719 | 15.819 | 13.341 | 3.108 / 2.833 |
| macOS Intel top burst / slow consumer | 8.987 | 8.708 | 16.749 | 13.263 | 4.026 / 4.167 |
| macOS Intel menu bar | 22.929 | 22.987 | 30.048 | 30.175 | 0.682 / 1.230 |

Guards were measured separately in a disposable administrator-owned Windows
profile with independently verified zero native inputs, zero present cameras
and no unknown inventory. Actual microphone and camera owners survived CLI/top
exit, foreign scope could not release the owner, explicit release retired the
helpers, and cleanup completed without error. Oversized/malformed peers and a
peer holding its pipe open recovered; the slow-peer case completed in 4.031 s.
No physical device mutation or audio capture occurred. At this pre-console-fix
source the two guards and their hidden `conhost.exe` used a sustained median/
p95 of 39.748 MB, with a sampled concurrent control/startup peak of 62.886 MB.
The console host alone had an observed peak of 11.796 MB and OS highwater of
12.202 MB. Those are measurements, not bytes subtracted from a newer result.

Source `2e6be9b` removed the camera console host, but
[qualification 38039480512](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38039480512)
found redirected CLI pipes retained by the background camera owner: the command
timed out after 25 s. Unix native jobs passed; Windows guard proof was incomplete,
with verified cleanup and no hardware changes. This failed run is not a RAM or
protection qualification. An isolated native ShellExecute fixture reproduced
pipe EOF only after the five-second child exited (5.235 s). Closing unique
inherited standard handles before detachment gave EOF in 0.250 s; the child's
image/birth-verified native process handle proved it was still alive afterward
and later exited 0. The corrected helper path needs actual hosted qualification;
do not project savings or replace the failed result with that fixture.

The [full native requalification `078cdc7`](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38047504453)
passed on all four targets, including actual detached Windows owners, captured
CLI pipes, explicit retirement and clean teardown. Foreign scope could not
release the owner; oversized/malformed peers recovered in 0.032/0.031 s and a
held-open peer in 4.047 s. No `conhost.exe` was observed in the guard product tree.
The measured guard-only sustained window still includes verification/control
commands; sampling gaps reached 62 ms, so no guaranteed 50 ms cadence is claimed.

| Windows hosted phase / source | Sustained median MB | Sustained p95 MB | Sampled concurrent peak MB | Observed whole-scenario CPU seconds |
| --- | --- | --- | --- | --- |
| Both guards + console host, `fe86151` | 39.748 | 39.748 | 62.886 | — |
| Both guards, no console host, `078cdc7` | 27.959 | 27.959 | 50.991 | 1.156 |
| Baseline v0.16.1 watch idle, same `078cdc7` runner | 12.726 | 12.743 | 12.923 | 0.844 |
| Candidate watch idle, `078cdc7` | 14.901 | 14.905 | 15.135 | 0.750 |
| Baseline v0.16.1 top idle, same runner | 13.730 | 13.730 | 13.926 | 0.938 |
| Candidate top idle, `078cdc7` | 15.892 | 15.901 | 16.089 | 1.031 |
| Baseline v0.16.1 top burst / slow consumer, same runner | 13.799 | 13.799 | 14.017 | 1.656 |
| Candidate top burst / slow consumer, `078cdc7` | 15.929 | 15.946 | 16.372 | 1.578 |
| Baseline v0.16.1 tray, same runner | 15.450 | 15.450 | 27.967 | 0.453 |
| Candidate tray, `078cdc7` | 16.519 | 16.519 | 24.183 | 0.453 |

Decimal MB and shared-page-counting caveats apply. Guard steady-window observed
CPU was 0.015625 s; the table's CPU includes control/startup/release instead.
Current Windows budget qualification remains **unmet**, not sub-15 MB. Status
completed in 0.031–0.047 s but had only one/two RSS samples; OS highwater and
unobserved transient costs still matter. This empty runner does not supersede
the local populated-machine measurements or certify hardware enforcement.

The earlier guard window does not include a concurrently sustained top/tray.
Additional hosted phases now retain both real owners while sustaining each
frontend, verify all three roles in every retained sample and confirm that Quit/
WM_CLOSE does not release either requested protection. This composed scenario
still needs its own native run; do not add the separate-profile medians and call
that a measured combined total.

An approved one-target legacy recovery cleared the disconnected camera's native
disabled-configuration flag from 1 to 0. Its protected receipt is fulfilled,
generation 1, requested=false, with no owned entries. The medium client then
failed opening the two shared metadata-only parents, although named ACL queries
and handles to the scoped directory/journal succeeded. The correction uses named
security inspection plus a reparse-attribute check for those admin-owned shared
parents only; existing ancestor/scoped-directory/journal handle checks remain.
ACLs are unchanged. This is not protection against malicious administrators.
A native restricted-medium regression verifies metadata readability and denied
journal-data creation; its administrative fixture runs in hosted Windows CI.
The rebuilt normal medium client completed `mcw --lang fr camera status` in
1.000 s, exit 0: allowed, present=1, blocked/pending/absent/unknown=0, guard
inactive, no stderr or new UAC. Only the previously authorized disconnected
camera's historical flag was changed; no microphone mutation or capture.
Local fmt, both native unit suites (161 library +1 main, 1 visual ignored),
strict feature Clippy and release build passed. The new administrative
restricted-medium fixture returns without impersonation on this medium host.
It passed in both hosted native Windows unit suites, with the administrator
fixture enabled; the guard harness separately verified hosted administrator
authority and zero-device SDK inventories. Its skip checks now interrogate
TokenElevation explicitly and propagate unexpected native errors.

Linux's real owned virtual PipeWire capture remained identifiable after adding
16,384 irrelevant objects (3,080,587 output bytes). Malformed input, retained
object limit, stdout/stderr flood, blocked child and inherited-pipe orphan
reported unavailable health, explicit watch gaps and subsequent recovery,
never a false STOP for the continuously owned capture. The baseline orphan
timed out after 8.067 s; the candidate returned its timeout diagnosis in 3.038 s.
This is controlled virtual/native-subprocess evidence, not hardware recording.

The 15 MB target is unmet on Windows and macOS. Linux's full budget is
**not proven**, not "met": fast status and transient children escaped samples.
Maximum observed candidate gaps were about 63 ms on Windows, 31.4 ms in Linux
long-lived profiles, 191 ms on macOS ARM and 294 ms on macOS Intel; macOS does
not supply OS RSS highwater here. Shared pages remain counted in summed RSS.
Brief helpers and their final CPU can escape sampling, so observed CPU is a
lower-bound measurement, particularly on macOS, not a complete CPU total.
Top readiness follows the harness's three-second drain; watch has no first-scan
readiness protocol. No ≤50 ms sampling or physical-event latency is claimed.
The macOS ARM menu bar also exceeds the 30 MB reference; that reference is not
a hard cutoff. No working-set trimming, hidden helper cost or false sub-target
claim is used.

