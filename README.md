# miccamwatch

`mcw` monitors microphone and camera access using platform-specific evidence. Windows uses native capture and privacy APIs; Linux and macOS 15+ provide native monitoring, desktop frontends and scoped controls with narrower evidence. It supports high-contrast terminal colors, seven display languages, policy-driven trust validation, JSONL history and native event notifications. Detailed evidence remains in English for stable machine-readable diagnostics. Distribution covers Windows ZIP/MSI, Linux x64 and macOS Apple Silicon/Intel CLI archives and native application disk images; previous release tags and assets remain immutable.

## macOS camera activity

Camera inventory uses passive AVFoundation discovery; activity uses public CoreMediaIO device running-state. A real-device report on Apple Silicon/macOS 27 confirmed Telegram **0 → 1 → 0** and FaceTime HD Camera START/STOP after the correction; that reported evidence is documented with attribution in [Architecture](docs/ARCHITECTURE.md), not represented as a maintainer or hosted-CI hardware run.

Client/PID remain **unknown**, confidence medium and coverage degraded. Running-state does not prove frame flow or permit per-application enforcement. Read errors stay unknown, never silently inactive.

The revised helper passed native macOS 15.7.9 Apple Silicon/Intel builds, regressions and extracted CLI/TUI/menu-bar smoke in the [successful three-platform dry run 37425122074](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37425122074). Both system and MCW inventories reported zero cameras on the macOS runners; they cannot establish macOS 27 hardware parity. Linux privileged installation/recovery and ordinary-user lifecycle checks also passed. On the authorized Linux hardware host, real Polkit-approved CLI block/allow revoked an ongoing UVC capture, denied new capture even to root, and restored the original bindings and capture. See [verification boundaries](docs/ARCHITECTURE.md#native-0160-software-verification-boundaries) for exact sources, agent prerequisites and remaining physical limits.

The [latest native dry run 37429480145](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37429480145) also passed the PipeWire socket-activation correction in source `8be1f1b`: session mute now verifies the socket creator's UID/process generation without requiring its executable, while capture-process identity checks remain strict. On the real systemd-activated host, `doctor` returned 0 and independent PipeWire readback verified mute/restoration and preservation of an already-muted original.

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
- Interactive full-terminal live dashboard (`mcw top`) with keyboard and left-click controls. Shortcuts accept either case; `[b]` blocks supported Windows/USB Linux cameras and `[a]` restores owned changes; macOS retains manual profile approval/removal. Linux USB controls require separate administrator setup and explicit Polkit authorization. `0.17.0` keeps all six buttons in one horizontal row with compact narrow representations. `[k]` requires a second confirmation for the same live process identity; Esc cancels without quitting. Camera approval runs off the UI thread; pending operations prevent an unsafe quit and failures remain visible.
- Native desktop mode (`mcw tray`): Windows Notification Area, Linux StatusNotifierItem on a supporting desktop, or macOS AppKit menu bar.
- Privacy commands expose the actual platform scope: Windows device blocking, Linux session-source mute and explicitly authorized USB `uvcvideo` controls, and writable macOS input mute plus an explicitly approved owned camera profile. Unsupported controls are disabled or refused, never reported as successful.
- Persistent settings and rotating JSONL history use application-data directories on Windows and XDG/HOME directories on Unix. Query exact paths with `mcw config settings-path` and `mcw history path`.
- Single-instance per-user desktop service with native lifecycle IPC (`mcw tray status` / `mcw tray stop`).
- Opt-in termination of explicitly policy-denied active capture processes after two consecutive observations (`--kill-unauthorized`, disabled by default).
- Three-state session lock detection on Windows/Linux; unknown lock state never triggers enforcement. Public macOS lock state stays unknown and enabling lock policy is refused.
- Discreet native audio chime upon confirmed capture initiation (`--sound`).
- Monitoring runs without administrator privileges. Windows camera device changes request administrator approval; Linux USB camera changes use a separately installed root-owned helper with explicit Polkit authorization; macOS camera profile installation/removal requires manual approval in System Settings.

## v0.17.0 — Published Windows-first protection baseline

**[Published baseline `v0.17.0`](https://github.com/Roman-Cuisset/miccamwatch/releases/tag/v0.17.0).**
P1-RAM and P1-RESTORE are published with software/native qualification, not
hardware certification. Previous binaries remain immutable. Linux/macOS controls
retain their honest one-shot scopes; they do not inherit the Windows background
protection owner.

- **Microphone:** requested manual protection, effective SDK endpoint mute and
  observed capture are separate facts. An owned native background broker keeps
  manual intent after `top`/tray exit until explicit `mcw unmute`; it is not an
  implicitly installed login service and does not enable autostart. Automatic
  protection tracks each endpoint's original state, generation and ownership;
  newer manual intent takes precedence. Release failures remain pending rather
  than being reported as restored. No requested intent plus no owner is normal;
  owner/transport failures while protection is requested remain explicit.
  The Windows broker does not retire while an admitted request is being prepared
  or queued; intervening admission invalidates an earlier inactive snapshot.
  A response timeout does not prove a requested change was unapplied, and does
  not trigger automatic command replay.
- **Limits:** mute is software SDK control, not capture denial. Exclusive,
  ASIO, direct-driver and hardware paths may bypass or differ from that scope.
  The CoreAudio event-context GUID is advisory; same-value `SetMute` can return
  `S_FALSE` without a callback. MCW cannot identify an exact Ktalk/Audition actor
  or detect every invisible same-valued external intent. Ownership safeguards
  are not an absolute security boundary.
- **Windows camera:** global Block/Allow does not depend on present count:
  zero, one or 1,000 supported cameras use the same intent. Block starts an
  explicitly UAC-approved temporary native owner covering future arrivals;
  Allow clears that intent and restores only protected owned changes, never
  every disabled device. An absent camera is not expected inventory or an
  error by itself. Arrival enforcement has a brief window; SDK restart needs,
  vetoes and unknown/partial readback stay visible, not guaranteed denial.
- **Legacy recovery:** old unsigned ownership files are not imported.
  `mcw camera allow --restore-legacy INSTANCE_ID` is an explicit fresh,
  single-target UAC action with camera-class validation, not automatic recovery
  or a vendor-specific Logitech privilege exception. Camera ownership evidence
  lives in an administrator-protected journal.
- **Local control:** fixed-capability native IPC authenticates exact SID,
  session, scoped data-directory hash and native process birth identity.
  Requests/responses and deadlines are bounded; normal RPC accepts no arbitrary
  paths/devices. Alternate-administrator delegation grants QUERY-only process
  inspection, not token use, termination or general administrator authority.
- **Replacement safety:** explicitly release microphone/camera protection and
  finish pending restoration before replacing binaries. Closing a UI is not
  release. Portable Windows `mcw update` reserves both request locks
  and validated native resource leases before tray shutdown or file swaps;
  active/foreign owners, manual/automatic intent, pending owned restoration and
  unreadable/corrupt/unsupported records refuse replacement. Unsigned legacy
  owed-only camera history alone does not block update and survives unchanged
  for explicit recovery; it is not privileged ownership. No helper launch, SDK
  mutation or implicit release is used for readiness. Reservations cover swaps/
  rollback; if protection resumes before a post-restart rollback, it refuses
  rollback and retains backups. A same-version matching-pair no-op needs no
  replacement reservation. **External MSI/manual replacement is not guarded by
  this portable-updater mechanism** and still requires explicit prior release.

PipeWire graph reading is streaming and bounded, with child-output
limits/deadlines and truthful health errors instead of false STOP. UI queues,
notifications and caches are bounded. These bounds are not a claim of RSS below
**15,000,000 B**, the soft target; approximately 30 MB is a reference, not a hard
cutoff.

**Native evidence:** Windows strict feature Clippy and both native unit suites
passed (161 library +1 main tests, 1 ignored visual test), including missing-owner
status, protected medium-reader and update-reservation regressions.
Eight real native IPC tests passed. Real ConPTY `top` exercised all seven
languages at 120/150/40/20 columns, refresh, mouse and quit; the rendered French
150-column view was visually inspected with six bordered controls on one row.
An isolated native process fixture reproduced inherited caller pipes keeping
a completed command open; the detached, non-inheriting launch released them
in 0.282 seconds while its child remained alive for five seconds.
Read-only resource sampling completed with **15 MB unmet**; the latest baseline/
candidate comparison and sampling limits are in the linked architecture table.
No active guard/tray was included in that local resource run.
[Native CI `32f87cd`](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38051530685)
passed on Windows, Linux and both macOS architectures; all four baseline/
candidate resource comparisons completed and qualified. On a zero-device
Windows runner, both real guards remained active through sustained `top`/tray
and their Quit/WM_CLOSE, rejected foreign scope and malformed peers, then retired
on explicit release with clean teardown. Composed sustained RSS medians were
**42.820 MB with top / 43.516 MB with tray**; guards alone used **27.922 MB**.
The sampled whole-scenario control/startup/release peak was **56.041 MB**.
The complete scenario exceeds both the 15 MB target and 30 MB reference; shared
RSS counting and sampling gaps remain explicit. The protected medium-reader
regression passed without broadening permissions. Approved one-target legacy
camera recovery completed, followed by normal medium-client status, no new UAC.
Further [native qualification at `29db092`](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38055262096)
passed all four targets after correcting a Unix/AppKit stop-readiness race.
Real Linux/macOS tray stop controls and process exits returned 0; all four
baseline/candidate resource comparisons and Windows composed-owner proof passed.
Physical microphone protection/restoration remains unqualified. A private Linux
PipeWire fixture proved an external unmute remained after five seconds and was
cleaned; that is not physical audio proof. macOS has no new physical validation.
See [protection architecture and proof
boundaries](docs/ARCHITECTURE.md#candidate-0170-windows-first-protection-contract).

The [exact tagged-source CI](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38056837640)
and [release workflow](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38058344725)
passed all four native targets. All seven published assets and six constrained
provenance attestations were verified against annotated tag `v0.17.0`, source
`d1b8557f827e102df20c33dbac4ec99badba7d1a`. A genuine private Windows portable
`0.16.1` pair ran `mcw update` to `0.17.0`; both EXEs matched the public ZIP,
preferences survived, and a second update preserved bytes/mtime. The unrelated
user tray remained alive; the user installation was not replaced.
[Public Unix migration](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38059262974)
also passed Linux and both macOS architectures: genuine packages, managed
0.16.1→0.17.0 upgrades and macOS public-0.16.0 standalone bootstrap. The
installation-only Linux session had no usable PipeWire session; physical
capture, enabled autostart and Gatekeeper approval are not proven by these
migrations. Full fixture boundaries and post-publication DNS checks are in
[Architecture](docs/ARCHITECTURE.md#post-publication-v0170-evidence).

## v0.17.1 — Corrective release, published stable

Windows tray status is a compact first line; complete privacy/collector diagnostics
remain in **Status details**. A stopped approved camera guard stays a warning:
requested Block is not proof that future camera arrivals are protected.
Recovered privacy operations clear only their own applicable errors; automatic
policy cannot acknowledge a failed explicit request. A successful native
notification API call clears current unavailability without deleting failure
history; API acceptance is not proof that a notification was visible.

Local Windows qualification passed strict Clippy, 162 library tests and one CLI
test, release build, and the real Win32 popup/details consumer. Three native popup
cases (idle with protection details, long failure, Unicode/control characters)
stayed compact; native text preserved complete diagnostics. Clipboard contents,
microphone/camera state and the installed user tray were not changed. The local
details screenshot was occluded by another application and is not GUI evidence.
Shared-native readiness tests require unowned resources and run explicitly on a
disposable Windows runner, not by stopping the user's protection.

Distribution recipes are under `distribution/homebrew` and `distribution/aur`.
They build immutable commit/digest-pinned source with locked Cargo dependencies;
they do not manufacture public-installer receipts. Upgrade through the owning
package manager, never overwrite a package with the curl installer or `mcw update`.
Homebrew embeds the existing Swift/AppKit helper; its refresh preserves absent,
dormant, disabled and externally owned autostart rather than enabling it.
Arch installs only reviewable camera-bootstrap materials under
`/usr/share/miccamwatch/linux-camera`, not an active root helper or Polkit action.
Administrator setup remains a separate reviewed matching-release archive operation;
package installation/removal never grants camera control or deletes user data.

The corrective installer reports platform, destination, download, checksum
verification, installation, and the exact command to run. Terminal colors are
optional (`NO_COLOR` is respected); redirected output stays plain. It retains
explicit PATH consent and never activates autostart or privacy controls.

Modern Arch's Clang 22+ exposes bindgen 0.69.5's forward-declaration handling bug,
producing an opaque SPA builder despite unchanged native header layout.
The vendored `libspa-sys` build uses bindgen 0.72.1's upstream fix, not a handwritten
ABI layout. A native C/Rust POD roundtrip exercises buffer growth and nested frames
against installed headers; the Ubuntu legacy-header and current Arch native
roundtrips both passed.

[Exact native CI](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38152020817)
passed Windows/Linux/macOS ARM/Intel, including real tray/details and MSI consumers.
[Unix installation](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38152020898)
passed all three platforms, including the real HTTPS curl pipeline.
The DNS endpoint serves the reviewed installer with verified SHA-256
`b7348c9a6f37a3d4f89faa2b767fd4880162d90c8f5d7c151185e0823384d1bb`;
its colored terminal install and owned uninstall were exercised on the server
inside an isolated HOME/prefix, without altering the site's page or Caddy routes.

[Native Arch qualification](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38152020811)
passed makepkg/pacman install/reinstall/pkgrel-upgrade/removal, private PipeWire,
real PTY and preservation of user state. AUR publication remains blocked by
missing maintainer-authorized SSH access. Homebrew publication is blocked:
the ARM runner's Cellar is group-writable (0775), and Intel's current upstream
z3 source formula fails before Rust provisioning. No permissions, checksums or
upstream formulas were weakened to manufacture acceptance. A source recipe is
not a published tap, Homebrew/core acceptance or AUR submission.
**[Published corrective baseline `v0.17.1`](https://github.com/Roman-Cuisset/miccamwatch/releases/tag/v0.17.1).**
The [native release workflow](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38210407613)
passed, and all seven assets/digests plus six constrained provenance attestations
were verified against annotated tag/source
`dd35f7ddc1bb36c079017f235703175c52d77da5`. The public Windows CLI was executed
from a private directory; the companion version was inspected without launching
it. The installed user pair and protections were not replaced or stopped.
Public [0.17.0→0.17.1 Unix migrations](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38212488807)
passed Linux and both macOS architectures, including PATH/user-state preservation
and standalone bootstrap boundaries. The [Arch recipe pinned to the actual
release source](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38212488566)
also passed its complete native package lifecycle. Neither test publishes an
external package-manager entry or certifies physical capture.
After explicit permission for the UI restart, the workstation's installed
CLI/companion pair was updated to the public 0.17.1 bytes. Its legacy main-executable
tray initially remained 0.17.0; a PID-bound native cooperative close, not process
termination or privacy commands, completed the approved handoff. The new companion
menu was visibly 0.17.1 and compact; the grey error cross was gone. Existing mute/
camera intent was preserved. This is software/UI evidence, not physical protection
certification or reproduction of the user's earlier notification failure.

## v0.17.2 — Windows camera attribution and partial tray coverage

The 0.17.2 correction keeps detected microphone/camera accesses visible when
a collector has only partial coverage. An amber exclamation indicates incomplete
coverage, distinct from the camera-ready pause glyph; actual collection/control
failures retain error precedence. Status details name active/ready accesses,
unknown client identities, unhealthy collectors and their reasons. A subsequent
healthy refresh recovers normally without clearing unrelated operation failures.

Windows sensor reports with a missing/zero report time remain **unattributed**.
Native PID/report-time query errors and raw metadata are now explained in the
existing evidence fields; raw PID metadata is never kill authorization. The
process-instance and report-freshness checks are not relaxed to guess Zoom.
Published as [stable v0.17.2](https://github.com/Roman-Cuisset/miccamwatch/releases/tag/v0.17.2),
source `a58c8b9f3a967589a576879d6d27bfdbc40f9dff`; the historical v0.17.1 release
is unchanged. After the user's explicit **“Maintenant”** permission, the workstation
CLI/companion now match the verified public 0.17.2 ZIP. Its actual native menu is
319×262 pixels, with Ktalk microphone/camera accesses and the red activity icon
visible; the full protection snapshot is unchanged. Local SDK/CLI and native menu,
window-rendering and icon checks passed. During a real user-started Zoom session,
the candidate and native C++ SDK probe observed microphone/camera activity, a
matching Zoom client PID, and successful but zero report time. Ktalk later had a
valid camera timestamp and verified attribution. No physical enforcement or
automatic unmute prevention is certified. See [ROADMAP](docs/ROADMAP.md#p0-cam-tray--caméra-zoom-non-attribuée-et-croix-grise-pendant-une-capture).

The [exact pre-tag CI](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38361454701)
and [native release](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38364052525)
passed all four targets. Seven public assets match API digests; the six
payload/SBOM files also match their exact named checksum entries and attestations,
constrained to the exact workflow, tag and source. Windows EXEs/MSI are **NotSigned**.
The genuine private Windows updater migration 0.17.1→0.17.2 and same-version
no-op passed. [Public Unix installer migrations](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38366922327)
passed bash/zsh/fish on Linux and both macOS architectures; these nine managed
migrations are installer paths, not cross-version managed CLI updater proofs.
Native [Arch qualification](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38362707189)
passed with the reviewed immutable release archive; AUR submission and Homebrew
tap publication remain blocked as documented.

Two issues were observed on the installed public v0.17.2 Windows pair:
the shell's accessible icon name contradicted active captures, and captured
updater output did not reach EOF after tray restart. The captured workstation
exit/stdout was unavailable after its timeout; the new paired files, native tray
and unchanged protections were verified read-only without rerunning update.
The old mapped-CLI cleanup warning remains separate. See the unreleased source
corrections below and [ROADMAP](docs/ROADMAP.md); no installed application or
security protection has been changed by that source work.

## 0.19.0 candidate — not published

This is an explicitly selected, **unpublished test candidate**, not a stable
update. Published stable remains v0.18.1; no tag or `latest` change is made by
building candidate artifacts. It includes direct tray profile selection on all
platforms and the Windows corrections below. Windows Explorer tooltip popup
rendering remains unqualified, and updating with an active protection owner is
not fixed. Existing protection ownership and restoration refusals remain enforced.

The Windows microphone owner now coordinates idle retirement with admitted
requests. A preparing or queued control prevents retirement; a fresh
reconciliation must still match the admission revision before shutdown. This
does not increase request deadlines or replay possibly applied controls.

The tray starts with a neutral product identity rather than asserting idle
before its first observation. Its native tooltip contains the current bounded
headline; full capture, health and control diagnostics remain in Status details.
Encoding reserves the native terminator and never splits a UTF-16 surrogate pair.

The Windows updater restarts its authenticated companion with native
`CreateProcessW` and handle inheritance disabled. Redirecting standard streams to
`NUL` alone still lets Windows inherit other pipe handles; it is insufficient.
Exact-image ownership, cooperative shutdown, busy/UAC refusal and restoration
requirements are unchanged. Native proof exercises captured caller EOF while a
successor remains alive; actual tray and Explorer/UIA checks run only on a
disposable hosted Windows desktop. Qualification status is recorded in ROADMAP.

Hosted qualification now exercises genuine public-old trays through the native
candidate replacement and mixed-pair repair transactions, without manual Exit.
It also checks refusal before shutdown while native ownership is held, and
helper retirement after explicit release while the tray remains open. These
native transaction proofs do not claim public acquisition of an unpublished
candidate. Immutable public 0.18.0/0.18.1 updaters completed their real active-tray
transactions but failed captured-stream EOF; the source restart fix is not
retroactively present in those published clients.


When camera Block remains requested but its elevated guard is unavailable, the
Windows tray offers **Restart camera protection (UAC)…** alongside the separate
Allow action. Restart revalidates the latest intent under the camera request lock;
it cannot turn a newer Allow back into Block. It preserves automatic restoration
ownership, reuses normal administrator approval and never inserts an Allow step.
The original native helper-status error is retained in diagnostics instead of
assuming that every failed IPC query proves the helper process stopped.

Windows lock policy treats its first known session state as a passive startup
baseline, not a new locking event. Unknown readings do not erase that baseline;
later configured transitions still work. Existing requested protection and
pending restoration are not reset or implicitly released at startup.
Updater reservation errors now distinguish camera/microphone and request/resource
ownership. A real protected owner remains a refusal: closing its frontend first
is not a safe workaround, because it can resolve automatic restoration.

Microphone control failures preserve the original IPC error when native ownership
is busy. An occupied global lease alone does not identify a different operational
namespace or missing administrator rights: an incompatible guard version is also
possible. Version compatibility and ownership checks remain enforced. Release
protection explicitly through its matching version before changing binaries;
terminating the guard process is not a substitute for release.

Concurrent Windows observers and controls wait at most three seconds for a busy
native pipe using the [documented Win32 admission pattern](https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-client).
Only opening the connection is retried; request bytes are sent once, after the
same peer authentication. Missing/denied endpoints fail normally, and transport
failure never replays a possibly applied command. This does not bypass the
separate native protection-ownership mutex or updater readiness checks.


These fixes are not retroactively included in the published v0.18.1 artifacts.

## v0.18.1 — ZIP dependency security maintenance

Version 0.18.1 upgrades the Windows updater to `self_update` 1.3.1 and ZIP 9;
qualification/publication status is recorded in ROADMAP. It removes the ZIP
versions affected by [RUSTSEC-2026-0367](https://rustsec.org/advisories/RUSTSEC-2026-0367),
[0368](https://rustsec.org/advisories/RUSTSEC-2026-0368) and
[0369](https://rustsec.org/advisories/RUSTSEC-2026-0369), issued 2026-10-17.
The advisories concern upstream full/stream extraction and Unix permission or
symlink handling. MCW selects two named executable entries through the single-file
Windows updater; no MCW exploit is demonstrated. The dependency is nevertheless
updated rather than ignored. Privacy behavior and permission requirements are
unchanged; the published 0.18.0 tag/assets remain immutable.

## v0.18.0 — Unix diagnostics and native macOS application

Version 0.18.0 adds the following capabilities; publication and qualification status is recorded in ROADMAP.
The minor version covers the new macOS diagnostic command and desktop IPC v2;
the Rust frontend and its embedded native helper must be built together.

The Unix tray keeps observed active/ready clients visible during incomplete
observation, with a distinct warning badge and full collector reasons in Details.
An unknown camera restriction read is not a failed action or camera permission.
Manual profile approval remains available; unrelated action failures stay visible.

On macOS, `mcw camera inspect --json` reads current-user profile metadata without
requesting rights or writing ownership state. If that scope is insufficient,
`mcw camera inspect --admin` or the explicit read-only menu action offers a
confirmation followed by macOS administrator authorization. Only a fixed,
bounded system-wide `profiles` query is elevated, never the tray or observer.
Results carry observation time, inventory completeness and authorization outcome;
cancellation/refusal leaves the effective restriction unknown. Global matches
are never adopted as MCW-owned profiles and inspection changes no controls.

Administrator access is not camera TCC permission. CoreMediaIO still cannot
identify camera clients or prove frame flow; public Quartz console metadata
does not prove screen-lock state. Neither limitation is repaired by elevation.
Existing Windows UAC and Linux Polkit controls retain their own scoped protocols.

The macOS menu has four direct, checked profile choices; an explicit `--config`
overrides preferences and disables profile changes. The authorized MCW mark
identifies the menu icon, with distinct activity/partial/error badges, and
Details sits immediately above Exit.

Native packaging produces `miccamwatch-macos-aarch64.dmg` and
`miccamwatch-macos-x86_64.dmg`, each containing `MicCamWatch.app` and an
Applications link. Opening the copied app launches directly into the menu bar
without Terminal, implicit autostart, protection or permission grants. CLI
tarballs remain available. `mcw update` inside an app refuses binary-only
replacement: quit and replace the whole app using the matching DMG.
Bundles are ad-hoc sealed, **not Developer ID signed or notarized**; no
Gatekeeper/quarantine bypass is included.

Native qualification and its GUI/authorization/hardware limits are recorded in
[ROADMAP](docs/ROADMAP.md#p0-unix-obs--causes-réelles-permissions-et-autorisations-utiles).
No interactive administrator approval or physical capture claim follows from
compilation or a hosted runner without the required hardware/authorization.

### macOS application (DMG)

Choose Apple Silicon (`aarch64`) or Intel (`x86_64`). Mount the matching DMG,
drag `MicCamWatch.app` onto its Applications shortcut, then open the copied app.
No Terminal or development tools are needed to start the menu bar. The app is
ad-hoc sealed only; a checksum or attestation does not replace Developer ID
signing, notarization or the user's Gatekeeper decision.

For manual replacement, quit MCW and copy the **whole app** from the new DMG to
the same location. Do not replace only `Contents/MacOS/mcw`. Preferences and
history live outside the app, at paths reported by the bundled CLI:

```sh
"/Applications/MicCamWatch.app/Contents/MacOS/mcw" config settings-path
"/Applications/MicCamWatch.app/Contents/MacOS/mcw" history path
```

If you explicitly own a login registration, refresh it after replacement:

```sh
"/Applications/MicCamWatch.app/Contents/MacOS/mcw" autostart refresh
```

Refresh must not enable dormant/disabled intent or adopt an unrelated registration.
Before moving an app with owned autostart, disable that registration from its
old bundled CLI; re-enable from the new location only if you still choose login
startup. For removal, disable owned autostart before quitting and deleting only
the app. Preferences/history are retained. Quit or deleting the app does not
implicitly restore device/profile changes; handle any owned restriction explicitly
before removal. Never remove TCC, SIP, quarantine or unrelated launch registrations.


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

On Windows, microphone attribution uses an active WASAPI capture session and has high confidence. An open Capability Access Manager interval provides medium-confidence camera activity; loaded camera modules provide low-confidence readiness only. Linux requires running PipeWire nodes and an active capture link for `active`. On macOS 15+, CoreAudio process `isRunningInput` signals microphone `active` only for a process exposed by `AudioHardwareSystem.processes`; AVFoundation discovers cameras and public CoreMediaIO [`kCMIODevicePropertyDeviceIsRunningSomewhere`](https://developer.apple.com/documentation/coremediaio/kcmiodevicepropertydeviceisrunningsomewhere) supplies medium-confidence, device-level camera `active` when its `UInt32` value is nonzero, without a client PID. Neither signal proves audio samples or camera frame flow. Inventory (`mcw devices`) does not query camera activity or request capture permissions. Empty discovery, missing states and API errors create observation gaps that suppress invented STOP events; known active/idle states still permit real START/STOP while camera coverage remains degraded.

`mcw` deliberately has no heuristic `unauthorized` result. Enforcement is separate from risk: only an explicit publisher/path policy mismatch produces `enforcement = "deny"`. Automatic termination additionally requires confirmed `active` capture, a PID, two consecutive observations, and a non-protected process. An unattributed macOS camera observation has unknown enforcement and cannot trigger termination.

## JSON contract

Status JSON is an object with an explicit schema version:

```json
{
  "schema_version": 3,
  "tool_version": "0.17.2",
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

The following installer/update description applies to public `v0.17.2`.
Explicit protection release and completed restoration are prerequisites for
replacement; older binaries do not acquire the new readiness checks retroactively.

For MSI installations, upgrade with the latest MSI from [GitHub Releases](https://github.com/Roman-Cuisset/miccamwatch/releases/latest); `mcw update` does not update MSI registration. Portable/Cargo installs use `mcw update` to install a matching CLI/tray pair. It verifies the selected release before requesting safe shutdown of a tray from this exact installation, refuses shutdown while camera/UAC operations are pending, and waits for actual process termination. A legacy v0.14.0 tray cannot acknowledge this protocol: finish pending operations and quit it manually, then retry. No force-kill or elevation is used to release an executable lock. Tray replacement is atomic; replacing the running CLI requires recoverable same-volume renames and has a brief pathname gap under the installation lock. Failed transactions roll back; incomplete recovery is reported without restarting an inconsistent pair. Only a previously running tray is restarted. Same-version repair verifies the pair but replaces only the missing/outdated tray; it does not pretend the CLI version is `0.0.0`.

The Windows updater downloads one exact release ZIP and checks its SHA-256 against both the selected GitHub API asset digest and its exact named `SHA256SUMS` entry, then validates both staged executable versions before shutdown. These share a release channel and are not an independent publisher signature. Signing is conditional on a real release certificate. If Defender quarantines `mcw.exe`, a `PATH` change is not a remedy: preserve Protection History details and follow [signing, provenance and safe incident handling](docs/SIGNING.md). No exclusion, disabled protection or quarantine restoration is recommended.

### v0.17.2 coordinated native packages

| Platform | Release asset | Requirements |
| --- | --- | --- |
| Windows x64 | `miccamwatch-windows-x86_64.zip` or `.msi` | Windows 10/11; ZIP includes matching CLI and tray |
| Linux x64 | `miccamwatch-linux-x86_64.tar.gz` | glibc 2.35+; PipeWire/SPA libraries, `pw-dump`, accessible user PipeWire socket |
| macOS Apple Silicon | `miccamwatch-macos-aarch64.tar.gz` | macOS 15+; Aqua session for menu bar |
| macOS Intel | `miccamwatch-macos-x86_64.tar.gz` | macOS 15+; Aqua session for menu bar |

Download the matching archive and `SHA256SUMS` from the release. On Linux use `sha256sum --check --ignore-missing SHA256SUMS`; on macOS compare `shasum -a 256 <archive>` with the named checksum. Prefer the managed Unix installer below for owned removal. For direct extraction, run `./mcw --version`, `./mcw doctor --json`, then `./mcw watch --json`. From macOS v0.16.1, `./mcw update` also updates an owned standalone executable in place outside a `bin` directory; unmanaged Linux archives still require manual replacement. The macOS helper is embedded; no Swift compiler is needed at runtime. Apple Developer ID signing/notarization is not provided; Gatekeeper may require explicit approval under your organization's policy. Do not bypass managed security controls.

`v0.16.0` includes the CoreMediaIO camera correction, systemd-activated PipeWire mute fix and optional explicitly approved Linux USB camera controls. Supported UVC block/restore was physically verified on one authorized Linux host; remaining physical/session limits are documented separately. A stable distribution does not imply universal hardware blocking or complete camera attribution. Windows signing requires a real configured certificate; without one ZIP/MSI executables are unsigned, and neither checksums nor provenance is antivirus clearance.

**v0.16.1 corrections:** macOS standalone updates no longer require a managed `PREFIX/bin/mcw` layout. In-place replacement retains existing executable permissions and quarantine, rejects modified/running/unsafe targets, and does not create receipts or change PATH, preferences or history. Tray menus on Windows, Linux and macOS keep diagnostic rows compact with full details available separately rather than stretching across the screen. Windows/macOS details are selectable/copyable native text; Linux uses the existing native status notification, whose visible height depends on the notification daemon. Shared `mcw top` buttons have clearer boundaries, shortcuts and responsive layouts in all seven languages; control capabilities do not change.

### Unix installer (macOS and Linux)

By default the installer selects the latest **stable** release into `$HOME/.local/bin`, without sudo. Use `--version v0.17.2` to pin this release, or omit `--version` to follow stable latest. The installer chooses the native macOS Apple Silicon/Intel or Linux x64 package, resolves a concrete release, and verifies the exact archive entry in that release's `SHA256SUMS`. Supported systems are macOS 15+ and Linux x64 with glibc 2.35+. Unsupported architectures are rejected, never substituted.

Download and inspect the installer before running it:

```sh
curl --proto '=https' --tlsv1.2 -fsSL \
  https://raw.githubusercontent.com/Roman-Cuisset/miccamwatch/a58c8b9f3a967589a576879d6d27bfdbc40f9dff/installer/install.sh \
  -o mcw-install.sh
less mcw-install.sh
sh mcw-install.sh --version v0.17.2 --add-path
```

Or follow stable latest after the same download and review:

```sh
sh mcw-install.sh --no-modify-path
```

An alternative HTTPS installer and documentation endpoint is available at
[serveur-asus.duckdns.org:9443/mcw/](https://serveur-asus.duckdns.org:9443/mcw/).
Replace the script URL above with
`https://serveur-asus.duckdns.org:9443/mcw/install.sh`; download and inspect it
before running the same installer options. This endpoint serves only static
documentation and the reviewed installer, not a binary mirror. Release payloads
and checksum validation still use GitHub. Do not disable TLS certificate checks.

For a short Linux/macOS install command, after trusting the reviewed endpoint:

```sh
curl --proto '=https' --tlsv1.2 -fsSL https://serveur-asus.duckdns.org:9443/mcw/install.sh | sh
```

This executes a remote script; downloading and reviewing it first is safer.
Do not use `sudo` or disable TLS verification. `| sh` is a Unix command, not a
PowerShell installer: Windows uses the native MSI or ZIP described above.

Use `--add-path` for explicitly requested automatic PATH integration, or `--no-modify-path` to leave shell configuration untouched. Without either flag, a missing PATH entry is proposed through `/dev/tty`; a noninteractive installation does not wait for input or silently edit the shell configuration. Bash, zsh (including `ZDOTDIR`), and fish are supported. Open a new terminal after accepting PATH integration: an installer subprocess cannot change its parent's environment. You can then run `mcw --version`, `mcw doctor --json`, and `mcw watch --json` from any directory.

`--prefix "$HOME/Applications/MicCamWatch"` installs into that prefix's `bin` directory. Repeat the installer with `--version v0.17.2` to upgrade an older managed Unix installation, or omit the version to follow stable latest. Stop its active watcher/tray first. Failed downloads, integrity checks, or executable validation preserve an existing working installation. The script URL above pins the immutable release source; the `main` installer URL follows source updates. Archives and checksums share a release channel and are not an independent publisher signature.

**Linux v0.15.1 migration:** its `mcw update` embeds the old three-file archive installer and safely refuses the six-file v0.16.0 package. Download the current public installer above and rerun it with the same `--prefix` (omit `--version` to follow stable latest). This migration installs the matching user camera payload without root setup or changes to preferences/history. After migration, v0.16.0 `mcw update` understands the new package format.

**macOS standalone v0.16.0 or older migration:** the old executable embeds its old updater and cannot fix itself. Quit its tray/watcher, download and inspect the current installer above, then run this once **from the directory containing your existing `mcw`**:

```sh
target="$(pwd -P)/mcw"
digest="$(/usr/bin/shasum -a 256 "$target")"
sh mcw-install.sh --version v0.17.2 --no-modify-path \
  --update-portable "$target" --current-sha256 "${digest%% *}"
./mcw --version
./mcw update
```

This explicit bootstrap replaces that same file, not another installation on PATH. Future updates use `./mcw update`. It requires an owned single-link regular executable and safe, non-shared ancestor directories; move an archive extracted in a shared writable directory to a private directory first. `bin/mcw` remains a managed-installer/package-manager layout: missing or damaged receipts do not fall back to standalone replacement. The portable updater does not enable autostart; it only refreshes an already-owned registration. An update interrupted before completion restores an unchanged owned target where safe; a concurrent change or quarantine is never overwritten, and a retained backup requires manual inspection.

Root execution is refused. Normal prefix installation does not overwrite an unmanaged executable, a symlink/hardlinked target, or an installer-owned executable modified independently; select a different prefix or move the old installation yourself. Only explicit macOS `--update-portable PATH --current-sha256 DIGEST` replacement can update a verified standalone executable. The installer refuses to replace or remove an installed executable while that target is running, without stopping any process. Ownership receipts live in `PREFIX/.miccamwatch-install`; retain them for managed upgrades and uninstall.

To uninstall an installer-managed installation, run `sh mcw-install.sh --uninstall`, supplying the same `--prefix` if you used a custom prefix. Uninstallation removes only installer-owned executable/state and managed PATH entries; it does not delete policy, settings, or activity history. An executable or PATH block changed independently must not be removed as though it were still installer-owned.

`v0.16.1` installs `mcw` with its shared TUI, native desktop mode and platform-scoped controls; old `v0.14.0` Unix assets remain monitoring-only. Installation never enables autostart, changes devices, approves a profile or grants capture permissions. Gatekeeper, SIP, TCC and quarantine protections are not bypassed. Apple Developer ID signing/notarization remains unavailable. `mcw update` on a managed Unix installation or a macOS standalone executable follows the stable channel and refuses downgrades; select a prerelease explicitly through the installer. An installed Linux root camera helper must be restored/removed in the order documented below before any user upgrade.

The [stable/latest v0.16.1 release](https://github.com/Roman-Cuisset/miccamwatch/releases/tag/v0.16.1), source `7b134e3500860bd185ddc3955107b8082d414e78`, passed [complete native CI](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37487806861) and [release-package smoke](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37493346742). Its seven public assets matched API SHA-256 digests; all six payload/SBOM assets matched the checksum manifest and authenticated attestations constrained to this tag, source commit, release workflow and GitHub-hosted runners. [Public v0.16.0 → v0.16.1 migration](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37495128116) passed on Linux x64 and macOS ARM/Intel with bash/zsh/fish, including genuine standalone macOS bootstrap and the corrected updater against public HTTPS assets. A genuine Windows v0.16.0 portable pair also updated against public latest: both EXEs matched the downloaded v0.16.1 ZIP, and a second call was up-to-date. These proofs do not extend physical-device support or provide a signing/antivirus verdict.

The [stable v0.16.0 release](https://github.com/Roman-Cuisset/miccamwatch/releases/tag/v0.16.0) passed [native build/package smoke](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37437092551) and public current-installer migrations from [v0.14.0](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37438076601) and [v0.15.1](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37438076504) on all three Unix architectures with bash/zsh/fish. Unpinned latest installation and the Linux v0.15.1 migration above were also exercised on a real SSH host. All seven downloaded public assets matched API digests; all six payload/SBOM assets matched `SHA256SUMS` and exact-tag build attestations. Windows v0.15.1 `mcw update` installed the exact public v0.16.0 CLI/tray pair in a private portable prefix without changing existing startup registration. Windows executables/MSI are unsigned; see [signing and observed Defender checks](docs/SIGNING.md).

The [published v0.15.1 prerelease](https://github.com/Roman-Cuisset/miccamwatch/releases/tag/v0.15.1) passed [public installation and v0.14.0 → v0.15.1 migration](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37301377449) on Linux x64, macOS Apple Silicon and Intel, with bash/zsh/fish: fresh shells, consent, idempotent PATH, running-executable refusal, failed-upgrade preservation and owned uninstall retaining preferences/history. The unshipped `v0.15.0` tag remains immutable and has no release/assets.

All three archives and the SPDX SBOM were independently checked against public API digests and `SHA256SUMS`; their GitHub attestations verified the release workflow, exact source commit and `refs/tags/v0.15.1`. The checksum manifest is not separately attested. On the real Linux SSH host, managed `mcw update` retained `0.15.1` rather than downgrade to stable `0.14.0`. The native updater refuses a prefix whose owned ancestor directories are writable by another user; it does not silently change those permissions.

The `0.16.0` Linux package adds optional USB camera controls. Physical USB
block/restore was separately verified within the documented support envelope.
Linux packages built from this source include a matching `mcw-camera-helper`,
reviewable administrator installer and Polkit policy. The ordinary installer
places these payloads in `PREFIX/share/miccamwatch/linux-camera` and maintains
their same-version ownership receipts alongside `mcw`. A managed update refreshes
the user payload transactionally, **not** the root helper. User uninstall removes
only unchanged owned user payloads, never privileged files or restoration state.
User upgrade/reinstall/uninstall is refused while any root helper installation
is present, including untrusted or malformed remnants. An `allowed` camera state
does not prove ownership evidence is empty or exclude a concurrent camera action.
Explicitly restore and remove the privileged installation first; the installer
never does that for you.
For root setup/update/removal, see [Linux USB camera administrator setup](#linux-usb-camera-administrator-setup).

## Static product website — Cloudflare Pages

`site/` contains shared templates and complete catalogs for English, French,
German, Spanish, Japanese, Simplified Chinese and Russian. Each language covers
the product, all platform instructions, maintenance, privacy, FAQ, accessibility,
metadata and 404. Native screenshots remain explicitly historical Windows
captures; only their captions and share-card artwork are localized. No framework,
analytics, visitor-side Node runtime, ASUS dependency or executable mirror.
Release facts in `site/release.json` describe the **published** release, not an
unreleased source build.

Build from the repository root with Node.js (qualified here under Node 24):

```console
node site/build.mjs
```

Output: **`site-dist/`**, with English default `index.html` and `404.html`, `/en/`,
`/fr/`, `/de/`, `/es/`, `/ja/`, `/zh/`, `/ru/`, localized 404 pages and root `install.sh`.
The exporter verifies the canonical installer SHA-256 and every catalog,
substitutes release facts, and rejects missing translations or modified technical
markup/code/tokens rather than silently falling back. It refuses unowned/symlinked
output and preserves unrelated files. Upload the output, **never `site/`**;
the latter contains build inputs, not a deployable website.
The root stays English regardless of browser language; language selection uses
the explicit locale links, without an automatic language redirect.
Home and 404 footers display **© 2026 Roman Cuisset**. The home footer links to
the MIT license; the author name in `LICENSE` uses the same spelling without a
hyphen. The MIT permissions are unchanged.

### Publish by direct upload

In Cloudflare **Workers & Pages → Create application → Get started → Drag and
drop your files**, upload the contents of `site-dist/`, or a ZIP with `index.html`
at its root; choose **Deploy site**. Use the real `pages.dev` address assigned to
your project. No Worker/Functions are required. [Official Direct Upload guide](https://developers.cloudflare.com/pages/get-started/direct-upload/).
Direct Upload projects cannot later switch to Git integration; create a separate
Git-integrated project if automatic deployments are preferred.

### Publish from Git

Cloudflare Git integration: framework **none**, repository root **empty/default**
(not `site`), build **`node site/build.mjs`**, output **`site-dist`**.
An empty build with root `site` previously published unresolved templates and
omitted the generated installer. Saving these fields and redeploying repaired
the actual Pages URL. `NODE_VERSION=24` reproduces the qualified major.

`site/origin.json` records the real production origin
**`https://miccamwatch.pages.dev`**. Set `MCW_SITE_URL` to another actual HTTPS
root origin when needed. Canonical/hreflang/sitemap and localized social images
use that stable origin, **not** an ephemeral `CF_PAGES_URL` preview hostname.
A custom domain is optional; do not replace the ASUS routing.

All seven languages were exercised in the real Wrangler Pages runtime on
desktop/mobile, with keyboard OS tabs, direct language links, no-JS content,
localized nearest-path 404s and secure headers. Actual native clipboard denial
and manual selection feedback passed without changing the user's clipboard;
successful system clipboard writing was intentionally not exercised.
The repaired **production** `/install.sh` was checksum-verified and used through
HTTPS curl to install/version/uninstall 0.17.2 in a disposable Linux HOME.

Regression tests: **`node --test site/localization.test.mjs`**; the static-site CI
runs them and exports all languages plus the verified installer.
On this PC, replacing the pre-existing `site-dist/` returned Windows EPERM.
The exporter preserved it; no ACL/antivirus bypass or forced replacement was used.
Fresh exact-source exports passed. The complete direct-upload bundle is
`Downloads/miccamwatch-pages-0.17.2-multilingual.zip` (34 files, 490,598 bytes),
SHA-256 `1747056b969ea5ca017895743af6d6b05eeb6f925c391af4eba8b6caa720bf65`.
The multilingual site is **published at https://miccamwatch.pages.dev/**.
Source `07eb1734dd177a5319969da5645983a6b94097d5` passed
[static-site CI](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38530338888)
and Cloudflare deployment. Every public locale, stable canonical, localized
deep-path 404, same-origin command and the exact51249-byte installer were verified.
Cloudflare currently injects its Web Analytics beacon, blocked by our restrictive
CSP; disable that project setting to remove the injection, not relax CSP.

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

In unreleased source, Windows and Linux join macOS with four direct, checked
profile choices: Balanced, Private, Meeting and Development. Selection uses the
existing saved preferences and survives a tray restart. An explicit `--config`
keeps authority: profile choices are disabled and no saved preference is
presented as the active override. Selecting a profile does not grant microphone
or camera permissions or implicitly release owned device protection.

The Windows parent menu caption also shows the selected localized profile without
opening the submenu. With an explicit `--config`, it shows the config override
instead of presenting a saved profile as effective.

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
server/node identity. They do not deny direct ALSA access. Linux camera controls
use a separate explicitly authorized helper to detach supported USB video-class
interfaces from `uvcvideo` through `USBDEVFS_IOCTL(DISCONNECT)`, and restore owned
bindings through `USBDEVFS_IOCTL(CONNECT)`. Each operation uses a pinned
`/dev/bus/usb` device-generation file descriptor, without claiming any interfaces;
driver and generation state are independently read back. Sysfs is read-only
inventory/status, not a path-name mutation target.
This affects direct V4L2 as well as PipeWire for those interfaces, not non-USB
cameras or every possible capture path. No global driver unload, device-node
permission change, or USB audio/storage control is performed.
New blocks require an original arrangement whose video-class siblings are all
bound to `uvcvideo`; mixed bound/unbound arrangements are externally managed
and refused before recording intent or changing drivers. Initially all-unbound
cameras are left untouched.
The supported descriptor envelope is deliberately conservative: exactly one USB
configuration, with every alternate of a selected interface number retaining
video class `0x0e` and the same control/streaming subclass. Multi-configuration,
role-changing, damaged/ambiguous descriptors and current-configuration mismatches
are unsupported, not handled by a broader fallback. Single-configuration
composite devices can retain separate audio/storage interfaces untouched.

### Linux USB camera administrator setup

Optional camera control needs Linux USBFS generation metadata
(`USBDEVFS_CONNINFO_EX`, kernel 5.9+; the documented baseline is Linux 5.15+),
Polkit (`/usr/bin/pkexec`), a usable authorization agent for interactive actions,
and a matching root-installed helper. Missing kernel support is an honest
unsupported result, not a fallback to generation-unsafe sysfs writes.
Administrator setup additionally needs `/usr/bin/python3`. The CLI, TUI `[b]`/`[a]`, and native
tray menu initiate only explicit actions; startup, status and polling never prompt
for privilege. Missing/untrusted/version-mismatched helpers are errors with setup
instructions, not successful blocks.

On the authorized SSH host, `pkexec`'s internal text agent failed with
`No session for cookie`, including a native invocation outside MCW; restarting
Polkit did not fix that failure. A standard **unprivileged `pkttyagent` registered
for the CLI process** successfully requested fresh administrator passwords for
block and restore. No authorization rule, PAM file, account or permission was
changed to obtain success. Headless/SSH use needs a functioning registered
agent; the presence of `pkexec` alone does not prove that authorization works.

Download the matching Linux archive and `SHA256SUMS` from the selected release,
verify the archive digest, and review `install-camera-helper.sh` and
`com.roman-cuisset.miccamwatch.camera.policy`. Checksums and an archive from the
same release protect integrity, not independent publisher authenticity; an
administrator must establish trust in the release and reviewed installer.
Do not elevate `mcw`, the user-side helper, a curl pipeline or an arbitrary
shell command through Polkit. Run the separate reviewed installer explicitly:

```sh
# Replace these arguments with absolute paths and the reviewed archive digest.
sudo /bin/sh /absolute/path/install-camera-helper.sh \
  --archive /absolute/path/miccamwatch-linux-x86_64.tar.gz \
  --sha256 '<reviewed 64-character archive SHA-256>'
mcw camera status
mcw camera block    # Explicit administrator authorization; supported current USB cameras only.
mcw camera allow    # Restore only exact identities recorded by MCW.
```

The administrator installer makes a private root-owned copy, verifies its
checksum before extraction/execution, checks protocol and CLI/helper version
pairing, and installs the fixed root-owned
`/usr/local/libexec/miccamwatch/mcw-camera-helper` plus the fixed Polkit policy.
It does not start anything, request camera permission, or change devices.
The root journal lives under `/var/lib/miccamwatch`, with private ownership
records and an integrity-checked readable status cache; cache data never
authorizes mutation.

Use this ordered upgrade procedure:

1. Run `mcw camera allow` with the **old matching CLI/helper** to restore changes
   and retire eligible vanished-generation records.
2. Run the reviewed administrator installer with `--uninstall`. It refuses
   removal while restoration records remain or evidence is malformed.
3. Update/reinstall the ordinary managed user installation. It refuses while
   any root helper installation is still present, even if status says `allowed`.
4. Explicitly repeat administrator setup with the verified new same-version
   archive.

For removal, perform steps 1 and 2 before ordinary user uninstall. These steps
never implicitly elevate or mutate a device; restoration is a separate explicit
authorized action. Modified/unmanaged files and nonempty/malformed restoration
evidence are preserved, not forcibly removed. Successful explicit root removal
transactionally deletes only integrity-verified empty journal/cache files, so
stale version-bound empty status cannot obstruct the new user CLI. The permanent
operation-lock inode is retained. Root-helper setup/removal and camera mutations share the
root operation lock, and a mapped helper revalidates its fixed executable after
acquiring that lock so it cannot act after root uninstall. Concurrent
administrator reinstallation during the ordinary user lifecycle is outside this
ordered procedure and must not be performed.

Blocking journals each original exact USB identity before kernel changes.
Reboot, replacement, unplug/replug, partial changes or authorization denial can
produce an honest incomplete/stale state; MCW must not bind a different camera
as though it were the recorded device. Explicit `camera allow` may retire records
from a prior boot or a demonstrably vanished/replaced parent USB device generation,
without touching replacements. Changed interface inodes alone do not authorize
retirement. Inspect `mcw camera status` and its detail; never delete the journal
as a recovery shortcut. Newly plugged cameras
are not automatically blocked. Non-USB cameras and automatic Linux
camera-on-lock are unsupported: explicitly block before locking, rather than
expect an authorization dialog to work on a locked desktop. Physical capture
denial/restoration remains unverified until independently exercised on hardware.

### macOS desktop camera restrictions

macOS microphone controls operate only on writable CoreAudio input-mute
properties. Missing/read-only properties are unavailable, not muted.
`mcw camera block` prepares an owned Restrictions profile and opens the manual
approval path in System Settings; pending approval is not a block.
`mcw camera allow` requests removal of that exact owned profile. Profile
installation metadata is not proof of physical capture denial. There is no
universal microphone deny or private lock-state fallback.

Mute and profile ownership records survive uninstall so restoration remains
possible. Restore owned changes explicitly before uninstall if desired.
`mcw update` requires a public-installer receipt for managed Unix `bin/mcw`
layouts; macOS v0.16.1 also supports safe standalone replacement outside `bin`.
Stop an active installed watcher/tray first. Native notifications, sound and menu-bar/tray
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

For Linux, install stable Rust, `pkg-config`, PipeWire/SPA development headers and libclang, then run `cargo build --release --locked --features linux-camera-helper` to build both `mcw` and the optional camera helper. Without that feature only the CLI is built; separately reviewed administrator setup is still required for camera mutation. Runtime monitoring needs the user PipeWire service and `pw-dump`; detached signature rules need GnuPG. Ubuntu 22.04's stock PipeWire 0.3.48 headers are supported. On macOS 15+, install Xcode Command Line Tools with a macOS 15 SDK and Swift compiler, then run `cargo build --release --locked`. Cargo embeds a private ad-hoc-signed native helper application in `mcw`; no development script is needed at runtime. Ad-hoc signing establishes helper integrity/identity, not Developer ID trust or notarization.

## Platform scope

| OS / environment | Microphone `active` / PID | Camera `active` / PID | Camera `ready` | Hardware blocking and desktop controls |
| --- | --- | --- | --- | --- |
| Windows 10/11 | WASAPI session / validated process | Capture activity evidence / validated process where available | Loaded capture pipeline, unconfirmed | Administrator-approved camera device controls, mute, tray and notifications |
| Linux desktop with PipeWire | Running source, stream and active capture link / authenticated Client PID validated with `/proc` | Running video source, stream and active capture link / validated authenticated PID | Idle stream or direct V4L2 open FD, low confidence | Session-source mute; explicitly authorized USB `uvcvideo` controls (separate root setup); non-USB cameras unsupported; native TUI, notifications and StatusNotifierItem host required |
| macOS 15+ | CoreAudio input activity / validated PID when libproc identity is readable | CoreMediaIO device running-state / **unknown PID** | Not inferred from camera availability | Writable input mute, manually approved owned camera profile, native TUI and AppKit menu bar |
| WSL or virtual/headless runners | No guaranteed access to physical capture hardware or user session | No guaranteed camera signal | Inventory is not access proof | No hardware behavior claim |

Linux builds a native CLI using the `pw-dump` PipeWire client and `/proc` (no administrator privileges required for the CLI). The Linux host needs an accessible user PipeWire socket (`XDG_RUNTIME_DIR`, optionally `PIPEWIRE_REMOTE`), `pw-dump`, and readable `/proc/<pid>/stat` and `/proc/<pid>/exe` for PID attribution. `mcw status --json`, `mcw devices`, `mcw doctor`, and `mcw watch --json` run on Linux and macOS. `watch --json` emits access-event JSONL and a status document containing `collectors` when health changes; Ctrl+C stops it.

Linux desktop effects require a StatusNotifierItem host, a native notification
daemon with `notify-send`, and `canberra-gtk-play` with an installed sound theme
and a backend for the actual sound session. On Ubuntu, the
[libcanberra-pulse backend](https://packages.ubuntu.com/jammy/libcanberra-pulse)
uses PulseAudio or PipeWire's PulseAudio server; installing only the GTK player
does not provide that backend. Playback acceptance is not physical audibility.

On Linux, only a **running PipeWire capture stream with an active source link and a running source node** is marked `active`. A claimed application PID must match the owning Client's server-authenticated `pipewire.sec.pid`, then pass `/proc` start-time and executable validation; forwarded portal/PulseAudio clients without matching identity remain unattributed. A direct V4L2 open FD never proves frame flow; video health is `degraded` when a camera may be accessed outside PipeWire. An inaccessible or restarting PipeWire socket is `unavailable` (`status` exits 2), not an all-clear. `--include-ready` exposes unconfirmed PipeWire streams and direct `/dev/video*` handles as `ready`, never as active.

On macOS, camera activity uses CoreMediaIO device running-state, not AVFoundation's unreliable in-use boolean. The client/PID remains unknown, and running-state does not prove frame flow. Camera health stays `degraded`; failed or empty discovery and unknown/error activity queries create observation gaps, never invented idle or STOP events. CI verifies backend commands and honest health reports, **not** physical microphone/camera transitions. No camera/TCC bypass or intrusive probe is attempted. Linux enforcement requires stable pidfd authority; macOS requires retained task/audit-token authority and refuses protected or inaccessible targets. Android needs a separate application.

The [0.15.1 native release dry run](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37299191864)
passed Ubuntu 22.04/PipeWire 0.3.48 and macOS 15.7.9 on Apple Silicon and Intel:
format, Clippy, tests, debug/release builds and extracted-package runtime checks.
macOS used SDK 15.5 and Swift 6.1.2 in Swift 5 mode, deployment target 15.0.
The smoke exercised real desktop icons, owned start/stop and autostart,
seven-language help, Linux native notifications/journal delivery and session mute,
and native chime acceptance. Windows [CI](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37299189617)
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
