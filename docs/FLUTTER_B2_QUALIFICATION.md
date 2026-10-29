# Flutter hybrid B2 — native alerts, CPU and Unix qualification

29 October 2026. Local branch `work/flutter-hybrid-0211`; no push, merge, tag, release, site deployment or stable installation change authorized.

This is a functional B2 tranche, **not full Flutter parity or release qualification**. The paired performance checkpoint is `d37c8e8`, following `27948eb`. Final code/build checkpoint `9aa4689` adds build-gating, equivalent Clippy syntax and capture-harness corrections without changing the measured collector algorithm. Windows and Linux lifecycle smoke runs also passed on that final code. See [ROADMAP](ROADMAP.md) and the [migration matrix](FLUTTER_MIGRATION_MATRIX.md).

## Delivered behavior

- Rust owns Windows capture notifications independently of Flutter. Existing capture snapshots and transitions are reused; there is no second detector.
- Only Active starts are eligible. Ready observations, repeated observations and incomplete-source gaps do not manufacture capture starts. Per-access cooldown is 30 seconds; identity retention and the 32-slot delivery queue are bounded. Delivery errors/overflow remain diagnostic failures.
- Persisted master, pause and audio preferences are reread at delivery. Disabling or pausing suppresses capture toast and its audio together. Audio uses the toast request, not an additional duplicate chime.
- The Windows collector now uses a fresh kernel process-generation inventory where supported. It caches **only immutable process birth times**, never signature, token, executable, policy or authorization decisions.
- Linux has an owned Rust-to-Flutter transport and real common Flutter UI execution. Native GTK/AppKit remain available; Unix Flutter hardware commands stay disabled pending their platform-specific qualification.
- macOS runner branding, inherited transport integration, activation and native parent monitoring are prepared, not compiled or executed here.

## Windows notification qualification

Private prefix/data beneath `%LOCALAPPDATA%\MicCamWatchQualification\flutter-b2-20261029-082656`. The explicit `--settings-notification-probe-development` mode uses `MicCamWatch.Qualification`, not the installed application's AUMID. Its TEST-labelled synthetic capture sequence never enters the dashboard; dashboard observations remain real. No microphone/camera action was executed.

Observed through the real Rust worker and Windows WinRT:

1. Two eligible TEST starts submitted with Flutter absent; one owned process remained.
2. Immediate replay produced two cooldown suppressions, not additional submissions. Ready/duplicate/gap fixture phases produced no extra starts.
3. The real Flutter notification switch saved false through authenticated IPC. Native readback and unchecked UI confirmed it; subsequent TEST starts were suppressed while Flutter was closed.
4. Restart preserved disabled state. Flutter re-enabled notifications and enabled sound through actual native writes/readback.
5. Restart with saved sound enabled submitted two requests. The native OS toast history contained two `audio silent="false"` requests. Sound was subsequently restored to false through Flutter.
6. Existing paired native CLI `notifications pause 30` persisted the pause. A fresh owner suppressed both starts with `paused=2`, Flutter absent. Monitoring sequence advanced from 1 to 5 during this check.
7. Native CLI `notifications resume` removed the pause; a fresh owner submitted both starts again, Flutter absent.
8. Seven TEST entries were read back from the qualification AUMID only: two sound-enabled and five silent. The installed AUMID's availability was read, never used to send a TEST.

`Submitted` means WinRT accepted the request. Stored XML confirms OS retention/requested audio; it does **not** prove banner visibility, physical audibility, user volume, focus-assist behavior or universal platform delivery.

### First-use failure found and fixed

An initial genuine attempt returned `0x80070490` twice and submitted nothing. A native diagnostic isolated `ToastNotifier.Setting()`: for a new unpackaged AUMID the catalog entry did not exist before the first official `Show`. That single explicit TEST `Show` succeeded, created the entry, and subsequent availability readback returned Enabled.

The corrected path permits only this absent-catalog initialization attempt, lets Windows enforce delivery policy, and requires Enabled readback afterward. Other errors and known disabled settings remain errors; no Windows notification setting, shortcut, privilege or security policy is changed. Master/pause gates still precede delivery.

### Preserved semantics

Operation/error messages and capture events remain separate callers. The existing B1 generic-toast preference behavior was inspected and preserved; this tranche does not invent a new notification-policy split. CLI `watch --sound` semantics are not silently rewritten.

Windows microphone release still means unmuting supported active inputs, **not restoring each pre-existing mute value**. B2 neither invokes nor broadens that command. Unix commands are not enabled by copying this Windows behavior.

## Windows CPU — paired release-build measurements

Reference `27948eb`, candidate `d37c8e8`, two AB/BA rounds, 20 seconds per closed/open interval, private default data, no manufactured capture workload or attached debugger. Status schema3 comparisons retained process generations, signatures, risk, enforcement, timestamps, evidence and protection metadata; only tool version and ordering were normalized. All boundary comparisons passed. Boundary equality cannot prove uninterrupted equality between boundaries.

One observed Ready camera pipeline, no observed Active microphone/camera capture in this paired workload. Earlier cold profiling had two signature checks per cycle and is **not** a directly comparable workload. CPU100% means one logical core, not the whole PC.

| State | Reference CPU | Candidate CPU | Reference RSS | Candidate RSS | Processes |
|---|---:|---:|---:|---:|---:|
| Flutter closed | 17.53% | 11.60% | 19.29MiB | 20.63MiB | 1 |
| Flutter open | 15.94% | 15.07% | 132.88MiB | 133.99MiB | 2 |

Closed paired deltas were −7.67 and −4.19 percentage points. Open deltas were +0.59 and −2.34 points: no reliable open-window improvement is claimed. These are descriptive small-sample results, not significance, battery-life or final-release promises. Closed CPU remains materially above a near-zero-idle target.

### What changed, and what did not

Microsoft documents [SystemBasicProcessInformation](https://learn.microsoft.com/windows/win32/api/winternl/nf-winternl-ntquerysysteminformation) on Windows11 26100.4770+. Its unique kernel sequence number detects PID reuse. Every inventory remains fresh:

- Cached birth time is reusable only for the same PID and kernel sequence.
- A newly queried birth is confirmed against a second fresh kernel sequence before publication. A newly arrived/reused unsampled generation falls back to the original Toolhelp path rather than weakening ancestry.
- Missing/denied births are not cached; later permissions can be observed. Disappeared or changed generations are evicted.
- Unsupported API, malformed/bounded-buffer failure or unstable reconciliation retains the existing Toolhelp behavior. Windows10 still has this fallback; no Windows10 runtime measurement was made.
- File, signature, trust context, token, module, policy and capture observations are not reused from a verdict cache. No collector cadence was reduced.

A second matched closed AB/BA run with opt-in profiling showed 14.13→8.98% CPU. It is separate from the uninstrumented table. Inclusive scopes must not be summed:

| Counter / scope | Reference | Candidate |
|---|---:|---:|
| Fresh WinVerifyTrust checks per completed snapshot | 1 | 1 |
| Snapshot completions in each measured interval | 34 / 34 | 36 / 36 |
| Approximate collection cadence | 1.65–1.70Hz | 1.77Hz |
| Mean CPU per process-inventory call | 12.87–14.91ms | 0.22–0.45ms |

Candidate logs showed 76 basic-inventory calls per whole Host run and no Toolhelp fallback in this workload. The final ten calls had medians of 184 retained birth facts/hits and 163 actual birth-query attempts; denied facts are deliberately retried. Scratch buffer was 65,536 bytes. Before-change birth-query counts were not instrumented, so they are not presented as measured. Reconciliation/invalidation work is included in inventory timing.

Native signature verification remains the dominant unresolved cost. Active-capture latency, controlled file/trust-store mutation, several simultaneously observed capture applications and physical device transitions were not exercised.

## Memory and launch accounting

RSS includes shareable pages; sums may double-count. Windows private commit is not RSS/USS. GPU/DWM/WSLg compositor memory is outside these process totals. Virtual address size is not RAM.

Additional functional Windows modes, not interchangeable benchmark rows:

- Native owner after real TEST delivery, Flutter closed: 26.86MiBRSS, 4.91MiB private commit, one process in the five-second sample.
- Real settings UI with native worker/accessibility: 143.23MiBRSS and 128.00MiB private commit, two processes.
- Same settings-only owner after UI closure: 20.12MiBRSS, 4.56MiB private commit, one process. This owner had not initialized the toast client through capture submission.

Protocol-ready, window-visible and first usable raster are distinct. Windows comparison receipts retain their actual open/close timings; no universal launch improvement over B1 is claimed.

Final Windows `9aa4689` smoke: three opens/closes, three concurrent activation requests per opening, one frontend while open and none after close; collection sequence advanced. Protocol-ready times were470/371/309ms in this one trial, not universal first-raster or comparative launch results. Private preference SHA-256 was unchanged. Final Linux smoke reused the existing child, captured its actual window, closed Flutter while Rust stayed alive, then stopped the owner through the native CLI with exit0.

Final uncompressed qualification prefixes: Windows15files,38.31MiB including native CLI/tray and Flutter runtime; Linux14files,33.36MiB including native CLI and Flutter runtime. System/shared graphics libraries, private SDKs and test tools are not included. These are not signed installer sizes or final release packages.

## Linux x64 — actual execution, not just a build

Private WSL2 Kali x64, kernel6.6.87, WSLg **X11**. Rust1.98.1 and Flutter3.47.8/Dart3.13.7 were verified from official distributions. Required packages were SHA-256-verified against private authenticated APT metadata and extracted into a private sysroot: no system package installation, maintainer hook, elevation, permission/group/ptrace change or device action.

The common release Flutter app was launched by the real Rust settings-development owner. Its snapshots displayed Engine `linux`, degraded sources, microphone/camera **Unknown**, and hardware controls unavailable. No simulated media state or false blocking assurance was supplied.

- Six actual opens/closes, three per theme; same-owner activation reused the one Flutter child during three extra requests per theme.
- After each close the child stopped and Rust remained alive. Initial visible-window time was about2.08s; subsequent visible times0.87–0.97s. These are X11 mapped-window measurements with ~80ms polling, not protocol-ready or raster latency guarantees.
- Normal native `tray stop` exited0 after owned shutdown. An explicitly stopped Flutter process was also terminated when its disposable Rust parent was killed, exercising Linux's kernel parent-death signal independently of Dart.
- Screenshots in both themes were captured from the owned PID's X11 window and visually examined. WSLg reparents client windows, so the capture tool searches the bounded tree rather than assuming root children are clients.
- No actual SNI system-tray surface, native fallback window, Linux hardware control or new installer was exercised in WSLg. The development owner deliberately does not initialize an operational tray/control session. Safe preference transport is wired/compiled, but Linux user-driven preference mutation has not been qualified here.

### Stabilized Linux graphics limitation

A separate run waited15seconds, measured20seconds open and10seconds closed. Real `llvmpipe-*` threads confirmed software rendering; DRI3 warnings were retained, not hidden.

| WSLg development state | Process RSS | CPU, one logical core | Processes |
|---|---:|---:|---:|
| Open after stabilization | 270.30–271.24MiB, endpoint range | 39.09% | 2 |
| Closed after stabilization | 5.75MiB | 1.20% | 1 |

This is **not a low-cost open-window result** and not a physical Linux/GPU benchmark. It requires real accelerated Linux/Mint and Wayland qualification before replacing the native UI by default. No refresh/capture cadence was lowered to improve it. After close, no Flutter library was mapped into Rust; the owned frontend was gone. These development-owner numbers must not be advertised as operational SNI-tray measurements.

## Unix containment and macOS limits

Review found and corrected: leader reaped before group cleanup; unvalidated sibling executable payload; mutable XDG ancestors; detached observer shutdown; missing asynchronous pre-ready fallback; parent-death reliance on Dart.

Current containment pins directory/file ownership, checks complete code-bearing payloads, rejects other-user-writable ancestors, observes exited leaders with `waitid(WNOWAIT)`, kills the still-owned group before reaping, joins the observer before shutdown acknowledgement, and distinguishes failed-before-ready opening from normal ready-window close.

Linux installs race-checked `PR_SET_PDEATHSIG(SIGKILL)` before exec. macOS prepares an independent native parent monitor on a dedicated queue before engine setup, without new entitlement. Entire-process suspension can also suspend that macOS monitor; this is not an unconditional OS-enforced parent-death guarantee. A user closing before protocol Ready is indistinguishable from failed startup and may trigger native fallback in operational mode. Ordinary ready-window close stays closed.

macOS Intel/Apple Silicon: no host/Xcode SDK was available; no build, runtime, IPC, visual, control, signing or package claim. Native engine/helper require macOS15 even where Flutter project settings declare an older minimum. Sandbox/inherited handles, framework symlinks/ACLs, signals, seals and deployment-target intersection require real macOS checks. AppKit remains intact.

Linux ARM64: generic Unix transport/ownership code is prepared; no new ARM64 Flutter build/package or physical ARM execution was made. Earlier native ARM CI success is not B2 Flutter or Raspberry Pi validation. Existing ELF/architecture/installer safeguards remain unchanged.

## Checks and evidence

- Windows: strict Clippy with `windows-tray` and with `windows-tray,flutter-desktop`;5process-generation tests,10notification tests,42desktop tests; release CLI/tray build.
- Linux: check all targets; strict Clippy with `flutter-desktop`;49desktop tests, including15Unix containment tests; release native CLI build.
- Flutter on Windows and Linux: analyze clean,47tests each, release builds. Seven-language widget coverage is not seven actual native visual reviews. Real captures were French125% Windows and English100% Linux; other DPI/accessibility/screen-reader/platform cases remain open.
- Initial qualification failures are preserved: first-use WinRT preflight; missing private GTK/MIME/runtime libraries; partial CMake default-prefix cache; helper smaps parsing and X11 reparent assumptions; blank occluded Windows GPU captures. They are not counted as successful app scenarios.
- Native-only Windows build exposed accidentally included QA symbols. Qualification-only code is now conditionally compiled; exact fixture wording assertions were replaced by production cooldown/disconnected-channel behavior, not repinned. Clippy safety rules were not relaxed.
- A resumed partial CMake configure could retain `/usr/local`; the experimental frontend now rejects that implicit system destination. A negative configure refused it before install, and the private bundle build remains required. No system installation was performed.

Private receipts are under `%LOCALAPPDATA%\MicCamWatchQualification\flutter-b2-20261029-082656` and `~/mcw-qualification/b2-20261029-082656`. Main files: `cpu-abba-d37/comparison.json`, `cpu-profile-abba-d37/comparison.json`, `notification-functional-progress.json`, `native-toast-history-readback.txt`, `runtime-proof-v3/result.json`, `runtime-proof-v3/steady.json`, and `B2-AGGREGATE-EVIDENCE.json`. Raw status/logs can contain paths/process information: do not publish them without privacy review.

## Stable installation and own fixture cleanup

Final read-only checks confirmed unchanged MSI executable hashes, stable settings and stable notification identity. The original history prefix remained intact; the running application appended more history. No qualification executable remained among the observed Windows processes; the MSI tray still ran from its installed path, with an access-restricted child left untouched.

**Do not claim that all nine baseline files remained byte-identical.** Camera and microphone intent-journal hashes changed during the session; the camera timestamp was10:18:59, and the microphone journal was observed at10:18:53 and later18:29:36. The exact initiator is not established. An active owner lock could not be read. These files/processes were neither reset nor interrupted. No B2 microphone/camera mutation command was executed, but the file comparison alone cannot establish unchanged live protection state.

The seven saved TEST notification entries were removed using WinRT `ClearWithId` for **only** `MicCamWatch.Qualification`. Readback confirmed zero entries; the two registry metadata values/key created for that test identity were removed, restoring their initially absent state. No installed AUMID, global notification preference or OS-managed notification catalog was edited. The disposable cleanup helper initially faulted on exit after confirming the successful clear; its COM-interface lifetime was corrected and a read-only verification exited0, without repeating the clear. This was not an application-process failure.

Additional private receipts: `final-smoke-9aa4689.json`, Linux `runtime-proof-v3/final-smoke-9aa4689.json`, `test-notification-cleanup.json`, `test-notification-clear-receipt.json` and `stable-preservation-final.json`. Final Windows/light and Linux/dark images were also captured and visually examined.


## Next gate

Owner review before B3. Priorities: real Linux accelerated graphics/SNI and safe preference interaction; actual macOS/ARM64 builds and owned lifecycle; explicit native pause UI/autostart/diagnostic/history contracts. Camera/lock/microphone-original restoration and bundle-aware installation/update remain separately gated. Do not remove GTK/AppKit or advertise complete parity, physical ARM compatibility, signing, notarization or a public0.21.1 release.
