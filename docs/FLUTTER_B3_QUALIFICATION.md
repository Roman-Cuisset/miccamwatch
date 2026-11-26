# Flutter B3 qualification — 30 October 2026

Development evidence, not a public0.21.1 release, complete feature parity or an
installed-package qualification. Rust remains the engine/tray/security owner;
Flutter exists only while its window is open. Native GTK/AppKit remain available.

## Checkpoint and scope

Work branch: `work/flutter-hybrid-0211`, B2 baseline `570c40b`.
Products currently exercised: source `7e794d2`; runtime-only replay controller
checkpoint `4cabb1b`. Original CI product digests remain pinned and separate from
harness changes. No installed application, hardware permission or protection changed.

Private evidence root on the Windows qualification host:
`%LOCALAPPDATA%\MicCamWatchQualification\flutter-b3-20261030-081807`.
Raw observations can identify applications, paths and users; review before sharing.

- Strict Rust check/Clippy/release,46desktop and49Windows-platform tests passed.
- Flutter analyze,61tests and Windows release build passed after the Unix copy fix.
  The new permanent cases exercise invalid engine context and consent invalidation,
  not copies of translated paragraphs or text-count expectations.
- Existing seven-language widget checks remain. Real visual inspection is not
  claimed for all seven languages, desktop environments, fonts or DPI settings.
- Windows MSB8029 warns about this worktree being under Temp; build succeeded.
  Dependencies were not upgraded to silence unrelated package-version notices.

## Safe notification commands

Protocol4 is strict; protocol3 is not a downgrade/compatibility lane.
`notification_pause` accepts only `pause_30_minutes` or `resume`. Its expected
value is the exact stored native UTC timestamp or null, not a paused boolean.
Dart preserves the original timestamp text, including nanoseconds, for CAS.
Rust computes the30-minute deadline, locks/revalidates existing settings, saves,
reloads and reports the actual value. Native time determines effective pause.

The existing owned-child/session/sequence/request-ID transport remains in use.
No TCP service, device API, arbitrary command shell, additional privilege or
background Flutter engine is added. Requested state is not protection confirmation.

Actual Windows GUI qualification established:

- Pause saved a real native deadline; the UI displayed native paused state.
- Restarted Rust owner suppressed TEST events with Flutter absent.
- Resume cleared the deadline without enabling a disabled notification master.
- Private read-only settings refused writes without changing their bytes/values.
- Holding only the private settings lock produced uncertain timeout, not success.
  Closing/reopening Flutter did not cancel accepted native work; after unlocking,
  the new UI observed the native result and could resume correctly.
- Original private preference values were restored. Native serialization changed
  fixture CRLF to LF; this is semantic restoration, not byte-identical formatting.
- Owned UI/engine processes exited cleanly; monitoring advanced while UI was absent.
- Windows TEST history was cleared and independently reread0. Only exact metadata
  created for `MicCamWatch.Qualification` was removed; installed AUMID unchanged.

These are TEST-labelled delivery probes, not a new genuine-capture, audible-banner
or physical microphone/camera experiment. On Windows the master/pause also gates
existing native operation/error notifications; all seven-language explanations
reflect that scope. Saving a delivery preference does not prove OS delivery.

### Latest Windows lifecycle and visual result

Frozen7e794d2 release bundle:39.11MiB across15files. One actual read-only run
passed three reopens, ten same-child activations, owned-child crash/recovery,
monitoring advance with Flutter absent, and clean owner exit0/all owned PIDs gone.
First native Ready580ms, visible684ms; reopen Ready179/193/204ms, visible247/285/
311ms. Close-to-process-exit191–224ms across normal closes. These single-run
figures are descriptive, not the controlled AB/BA CPU comparison below.

Latest run RSS22.97MiB closed,136.10MiB open; private commit4.30/120.28MiB.
Its CPU differed with workload and is not substituted for the paired comparison.
`windows-final-lifecycle/qualification.json` retains all raw samples.

PrintWindow remained uniformly blank despite real UIA content. A separate private
offline-built WGC helper captured **only** each owned HWND, holding and validating
the exact process creation FILETIME/path before and after, without a picker,
monitor capture, permission request or fallback. Both actual French125% light/
dark images were examined: official logos, real overview/application-ready state,
readable cards and correctly disabled read-only controls. Automatic nonblank
output alone was not treated as visual proof. Original BMPs, helper source/lock,
JSON receipts and failed PrintWindow attempts remain in private evidence.

## Windows CPU: paired clean comparison

Immutable prefixes: B2 `runtime-final-9aa4689`, B3 `candidate-win-9773a1b`.
Two AB/BA rounds,60seconds per open/closed interval, optimized builds, per-owned-
process accounting and equal normalized observation/protection snapshots.
One low-confidence camera-ready application was observed; no confirmed active
capture. The stable MSI remained running independently throughout.

| State | B2 CPU, % of one core | B3 CPU, % of one core | B2 RSS MiB | B3 RSS MiB |
|---|---:|---:|---:|---:|
| Flutter closed | 5.7303 | 5.5347 | 20.8066 | 20.7656 |
| Flutter open | 8.4557 | 7.9983 | 136.7119 | 138.3555 |

Closed paired deltas:−0.2340 and−0.1572percentage points. Open deltas:
−2.5585 and+1.6439points. Small descriptive differences, **not a demonstrated
large, statistically significant or universal speedup**. Do not compare these
numbers directly to B2's11.60% from a different workload/session.

RSS includes shared pages. Candidate closed/open private commit was4.2266/123.1318
MiB; USS and shareable RSS are separately retained in the raw comparison. They are
not interchangeable RAM definitions and do not include GPU/DWM accounting.

`cpu-clean-9773/comparison.json` is the authoritative clean receipt. Timing there
uses two fresh owners: native positive-snapshot Ready290/205ms, owned HWND visible
404/289ms. These are not boot-cold measurements or same-owner reopen medians.
A ready frame submission is distinct from human-visible compositor proof.

### Optimization and separate instrumentation

Fresh Toolhelp enumeration is unchanged. Only the four existing exact ASCII
capture DLL basenames are decoded from UTF-16; irrelevant module names no longer
allocate/decode strings. Case, ordering, duplicates and existing pipeline gates
are preserved, including tested malformed/full-buffer boundaries.

No trust verdict, file content, signer, policy context or changing observation
is cached. WinVerifyTrust remains fresh; monitoring cadence is not reduced.

Separate `MCW_COLLECTOR_PROFILE=1` AB/BA runs used30-second intervals. Closed totals:

- Baseline/candidate each performed54WinVerifyTrust calls in approximately60s.
- Candidate enumerated46410module entries in1364fresh snapshots and decoded162names.
 46248avoided decodes are mechanism evidence, **not measured CPU savings**.
- Inclusive candidate stage CPU: module scan2.71%, WinVerifyTrust3.25%, containing
  camera stage7.08%. Stages nest; never add them. Baseline module counters are
  unavailable, not zero. Per-call CPU is coarsely quantized by the Windows timer.
- Instrumented closed process CPU8.38→8.51% is not the clean comparison and is not
  evidence of a gain. Additional diagnostic events have a cost.
- Sensor report wall time remains about504ms per call, not504ms of CPU.
  Completion rate stayed about0.9Hz; controlled active-capture/trust-mutation
  end-to-end latency is **not measured** without a separately authorized scenario.

Signature and module enumeration remain material costs. No claim that the CPU
problem is completely solved. Private `cpu-b3-analysis.json` retains weighted
stages, interval boundaries, counts and accounting caveats.

## Linux bundle and real Mint environment

Ubuntu22.04 x64 release bundle from source `fc941c0`, run41024851203:
19payload checksums, manifest, ELF symbols and tar content verified before transfer.
Tar SHA-256: `abed38deb2dcf78330a6faee88f683b2f75bb3ab6ac26a0a5edc13a806cf4384`.
The bundle is private qualification content, not an installer or stable replacement.

Existing authorized host: Mint22.2, GTK3.24.41, PipeWire1.0.5, glibc2.39.
Existing XFCE/XRDP X11 session reused; no login, driver/permission change, package
installation, autostart or desktop shortcut created. Stable executable remained
`/home/serveur/.local/bin/mcw` and was never stopped or replaced.

A real owned SNI item, activation, Rust data and Flutter1040×700 window were
observed. Actual child llvmpipe threads and no DRM FDs establish **software
rendering**, despite the physical Intel HD520. This is not an accelerated-console
or native-Wayland validation. The first light screenshot was examined.

The combined actual Mint run then passed all six cycles (three per theme),
SNI same-child activation, sound preference mutation/restoration, native pause/
resume, intentional private settings-read failure and final real UI restoration.
Every Rust owner exited0; no owned Flutter or private accessibility fixture survived.
The ordinary disappearing-`/proc` race was fixed in qualification only; permission/
generation failures remain errors. Original fc941c0 products were not modified.

Evidence: private `mint-first-qualified-evidence.tar.gz`, SHA256
`edf4611cf163f214507881d0e73b44b789db689cb6482faacb05968e30f94175`.
Final closed RSS10.11–10.17MiB; open RSS327.73–334.76MiB, PSS296.05–303.19MiB.
Open CPU24.17–26.21% of a core; final closed1.45–1.50%. These are lower-bound
process-tree samples missing very short-lived children, not whole-system CPU.
Mapped first windows4.72–4.84s; same-owner reopens0.97–1.21s. Software llvmpipe
is confirmed in the actual child; no accelerated Mint result is claimed.

The later source7e794d2 run also passed six cycles and real preference/error/
restoration actions, with zero forced cleanup or remaining owned UI/AT-SPI process.
Archive SHA256 `5c8408f4cd75eb279164a4f6fb5d386e5c897a540735fa30db05157fcd2d27c9`.
Both **normal Overview** captures after native-state reload were examined.

| Actual Mint source7e794d2 | Light | Dark |
|---|---:|---:|
| Open RSS, MiB | 325.40 | 329.76 |
| Open PSS, MiB | 293.77 | 298.12 |
| Final closed RSS, MiB | 9.97 | 10.24 |
| Final closed PSS, MiB | 7.50 | 7.76 |
| Open CPU, % of one core, lower bound | 23.18 | 23.07 |
| Closed CPU, % of one core, lower bound | 1.60 | 1.60 |
| First positive native Ready, ms | 1492 | 1409 |
| First mapped window, ms | 4971 | 4858 |

Reopen native Ready768–852ms, mapped976–1091ms; UI process exit172–234ms.
Ready acknowledgment can precede the software compositor's mapped-window evidence;
do not report it as first human-usable display. Actual llvmpipe remains the
dominant open-window limitation. No driver, GPU setting or permission was changed.

### Reproducible private Mint wrapper

`miccamwatch-b3-mint.tar.gz` is13501124bytes, SHA256
`d17595f5d910a644872bc5ecaaa2f477b2dea7a06a3c52a7a6b7e001d3736254`.
It contains the byte-identical original native archive, separately pinned4cabb1b
controllers, owned-window helper, provenance, six wrapper checksums and French
instructions. No personal evidence/logs, SDK, installer or modified stable files.
Its extraction, wrapper/product hashes and real runtime preflight were exercised
in another new0700Mint prefix. Existing exact products already passed six cycles;
this packaging check is not relabelled as another application lifecycle run.

### Qualification defects diagnosed without weakening guards

- Nested test HOME/data paths exceeded the native104-byte socket guard. Linux
  retains real HOME and uses exclusively created shorter private leaves; macOS
  uses short owned0700config/data beneath `~/mcw-qualification`. The socket comes
  from XDG_DATA_HOME, not XDG_RUNTIME_DIR. Native limit/peer checks unchanged.
- Python safe-tar filtering omitted directory modes and inherited SSH umask775.
  Only eight newly extracted directories were restored to archive-declared700;
  no existing parent/system permission was changed. Prefer `umask 077` plus native
  tar extraction into a new private directory, never overwrite an installation.
- Negative DBusMenu recursion depth needs a `--` argument terminator in gdbus.
- French readelf output initially produced false empty version requirements.
  Machine command locale is now C only in qualification children, not globally.
  Actual parsed maxima: Rust/runner GLIBC2.34; Flutter engine2.18; GLIBCXX3.4 and
  CXXABI1.3. Empty requirements for the self-contained AOT module are genuine.
- The existing desktop AT-SPI registry had stale X11 authorization and repeatedly
  failed `Cannot open default display`. It was not repaired or enabled globally.
  A qualification-only private EXTERNAL/UID bus and directly started registry use
  the already-authorized display. Real session/SNI bus remains unchanged.
  Explicit process generations, registry D-Bus owner PID, bounded teardown and
  separate controller lifetime prevent namespace/cache confusion.
- Accessibility bus/registry/controller are fixtures, outside and excluded from
  engine/Flutter performance totals. Every success requires their stopped receipt;
  forced cleanup remains a qualification failure, never a lifecycle success.

## macOS hosted qualification

Official Flutter3.47.8/Dart3.13.7, pinned Rust1.98.1 and Xcode26.3/SDK26.2.
Intel and Apple Silicon release builds and targeted Rust/Flutter suites passed.
Each Mac ran52desktop and5settings Rust cases. macOS minimum15 retained.

Run41028974295 verified actual architectures, plist/ACLs, exact sandbox entitlements,
ad-hoc seal and executable/mode/symlink-preserving tar restoration for both normal
and separately instrumented bundles. Xcode's automatic get-task-allow injection
was removed for Release using CODE_SIGN_INJECT_BASE_ENTITLEMENTS=NO. The strict
entitlement assertion and sandbox were not relaxed. No Developer ID or notarization.

Aqua was genuinely available. Run41037181504 reached positive real release IPC
on both architectures. ARM also passed the complete instrumented window lifecycle:
three same-child activations, owned-window capture, close without a resident engine,
reopen and owner-death shutdown. Intel passed initial launch/activation/capture/close
but reopening did not establish Ready. Its cause remains undetermined; no timeout
or production guard is relaxed. The next controller records the new child before
waiting and attempts a bounded read-only sample of that still-owned process.

The later AppKit fixture incorrectly expected disabled preferences to toggle.
Its test now preserves the authoritative value and exact no-event dispatch, while
writable controls retain both real toggles. This is not a product permission fix.

Runtime-only replay is prepared against immutable run41037181504 products:
exact artifact IDs, API source/digest, ZIP/TAR SHA256, every payload manifest and
source ancestry/controller-only diff checked before execution. Products remain
source7e794d2; controller revision is recorded separately. Release builds and unit
suites are inherited from the original run, not reported as newly executed.
Replay41066470241 passed ARM real release/instrumented IPC/lifecycle and the corrected
native AppKit fixture. Ubuntu hosted SNI, twelve combined lifecycle/preferences
cycles, native failure/restoration and owned desktop teardown also passed.
Intel reproducibly failed reopening; its owned live sample localized one1-second
interval to NSApplication/WindowServer session-port lookup before Flutter startup.
This is **not** proof of a permanent runner defect or the whole timeout's cause;
another Impeller line appears later without PID/timestamp attribution. The subsequent
bounded controls below supply stronger evidence. No timeout increase or workaround.

### Intel control result: session/bootstrap availability is also failing

[Intel-only41070188421](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/41070188421)
reused the exact source7e794d2 products, with controller e2f3be8. Original source/
digests/manifests/seals were verified; no release product or passing ARM/Linux job
was rebuilt/rerun. Two one-shot unsandboxed probes contain no Flutter or product IPC.

Before open they completed: session0.274s, NSApplication.shared0.214s. After normal
close, **both probes stopped responding within their bounded observation**; during
reopen, fresh probes did likewise and were reaped by their owning controllers.
The AppKit-only sample waits in the same WindowServer/get_session_port path;
the session-only sample waits in SLSessionCopyCurrentDictionary/Mach message.
The after-close and reopen-stalled AppKit stdout files contain only the pre-initialization marker.
Product PID5230 was still in AppKit bootstrap in early and late live, identity-
checked samples; the late sample began about11.19s after launch. No second Metal
line appeared in this run's timestamped sequence. Native15s Ready still failed.

This establishes a **broader graphical-service/context responsiveness failure in
this experiment**, not an isolated Dart/protocol failure. It does not establish
whether the OS/runner image, launch context or closing the product triggered it,
nor that all hosted Intel runners are fundamentally incapable. Controls can
perturb timing and are not sandboxed Rust children; their exit codes are not
product success. No safe product fix is demonstrated; do not remove the sandbox,
ownership, signatures or timeouts to make this pass.

Source inspection also confirms the development Mac owner is window-only
(`run_flutter_development`), whereas operational `run_tray` keeps an AppKit
DesktopHost. A device-mutation-disabled persistent native menu-host qualification
and a pure-AppKit window-close control are the next discriminants. Keep the native
fallback and the Intel reopen gate **failed**, without requiring a physical Mac.
Do not repeat the same failed workflow without changing the hypothesis/context.

Primary context: [Apple NSApplication.shared](https://developer.apple.com/documentation/appkit/nsapplication/shared)
creates the WindowServer connection; [Apple TN2083](https://developer.apple.com/library/archive/technotes/tn2083/_index.html)
explains inherited GUI/bootstrap contexts. Unix UID/SID/PGID alone is not proof of
Mach session equivalence. Historical documentation is not a macOS15 root-cause proof.

CI6 artifact12984942123 ZIP SHA256
`e9326e141db6eb3175d7c717a6b0ad677e84dddeaa142e767151eed04b005d71`;
native evidence TAR
`1590fba0e5b47d5dfba4992b66da83e28bf257996d05a3f9650048d791eb282f`.
Private raw controls, timestamps, early/late samples and diagnosis remain together.

CI: [41023711442](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/41023711442),
[41024851203](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/41024851203),
[41028974295](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/41028974295).
Current integrated requalification:
[41037181504](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/41037181504).
Runtime-only replay:
[41066470241](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/41066470241).

A physical/manual Mac is neither available nor a blocking development prerequisite.
Hardware/TCC, installed packages, Developer ID/notarization and configurations not
actually exercised remain unverified. No screen-recording permission is requested;
AppKit fixture uses no full-screen capture, only owned view/window evidence.

## Parity and preservation

See [migration matrix](FLUTTER_MIGRATION_MATRIX.md). Pause/resume is additional to
four safe preferences; Flutter offers30minutes/resume, while CLI retains1–10080.
Autostart, complete history/doctor, camera/lock/maintenance flows, Unix microphone
commands and persisted language/theme APIs remain unimplemented in Flutter.
Windows release-all semantics are not generalized to Unix owned-original restore.

Linux ARM64 Flutter build/package/runtime are not newly validated. Architecture-
neutral transport and native safety checks remain; earlier native ARM CI is not
Flutter or physical ARM evidence. No stable ARM support/fallback is removed.

Windows stable binaries/settings/protection and lock-file bytes matched baseline;
history grew but its entire original prefix was retained. Installed AUMID unchanged,
only mission TEST metadata/history cleaned. No forced restoration of live data.

No main/tag/release/site or installed MSI change. Only the explicitly authorized
`ci/flutter-b3-20261030-081807` ref was pushed for manual checks. Every pushed head
has `[CF-Pages-Skip] [skip ci]`; no deployment/release job is invoked. Direct
Cloudflare inventory lacked a token, so live Cloudflare configuration is not
claimed inspected. Local work branch, other worktrees/stash remain preserved.

## Handoff and next authorized tranche

Saved code checkpoints:3cf3a06 (safe pause),1fe5640 (module-name decoding),
f4bb9bd (validated engine-platform controls),7e794d2 (frozen integrated products),
4cabb1b (runtime replay/qualification fixes),e2f3be8 (Intel controls).
Later documentation commits do not change the qualified products. Work branch
remains local; only the separately authorized CI ref was pushed. Eight worktrees
and the existing stash remain; main73369ab and the stable annotated tag/target
06d800b/3d74686 are unchanged. The public site/README were not modified in B3.

1. Resolve/qualify Intel bootstrap using the native permanent-menu architecture
   and a discriminating no-Flutter control in a suitable hosted context. No physical
   Mac prerequisite, no blind timeout or environment/security workaround.
2. Measure accelerated Mint from an already-authorized local graphical session;
   do not treat XRDP llvmpipe figures as accelerated-console results or alter drivers.
3. Qualify Flutter ARM64 builds/packages, then continue safe autostart/history/
   doctor and platform-specific protection parity with their authorization gates.
4. Complete macOS dark/native preferences/menu integration, seven-language native
   visual/accessibility and actual installation/update packaging gates.

B3 delivers real Windows/Linux features and ARM hosted runtime evidence, **not**
a fully migrated or release-ready0.21.1. Intel remains a recorded failed gate;
accelerated Linux, physical hardware and the remaining parity are not invented.
Stable installations/settings/protections are preserved. Stop here for owner review.
