# Flutter B4 — reliability, consultation and platform qualification

Development branch: `work/flutter-hybrid-0211`. This is **not** a public0.21.1 release, full functional parity, or permission to replace stable0.21.0. GTK/AppKit fallbacks remain.

## Source and evidence boundaries

- Initial B3 baseline: `14ae566edc44047533f4423ef30d75fcab429898`.
- B4 preservation checkpoint: `79b35db28537c06f692dd7b4d855f3dd35b8744a` (intentionally WIP).
- Reviewed command/qualification code: `d7f778a0b5199913d2bdc0596b9ff812f4c7efd9`.
- Only the previously authorized CI ref `ci/flutter-b3-20261030-081807` was pushed, with `[CF-Pages-Skip] [skip ci]`; never main or the work branch. [CI41120973121](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/41120973121) passed Ubuntu22.04 x64. Its already-green package was retained, not rebuilt for later Mac/ARM preparation fixes.
- [CI41188772673](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/41188772673), source `7e47596`, passed both macOS architectures and native Linux ARM64. Artifact ZIP SHA256 digests were independently checked after download. Earlier preparation failures and their corrections remain in ROADMAP.
- Checkpoint `6bb4781` preserves Mint qualification and its narrowly corrected process sampler. `5a797f2` fixes a subsequently discovered empty native-menu test selection; [targeted Rust-only CI41191181077](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/41191181077) is separate from the already-qualified Flutter products.
- Private evidence root: `%LOCALAPPDATA%\MicCamWatchQualification\flutter-b4-20261030-202725`. It contains local application/process paths and genuine history data; do not publish raw evidence without privacy review.
- Main/origin73369ab, stable tag06d800b→3d74686, installed MSI,8worktrees and stash are preserved. Initial file and startup-registry receipts were taken before local UI tests.

Final read-only checks matched the installed MSI executables, settings, MicCamWatch startup values and the complete original history prefix. Two native protection journals differ from the initial30October receipt: camera intent and microphone intent. Their current data/timestamps are retained, not reset; writer attribution was not recorded and is not retroactively invented. An existing microphone guard from the installed MSI remains active, while the daily tray was absent at the final inventory. One active owner lock refused hashing; no elevation or lock workaround was attempted. No B4 development process remains. Do not claim byte-identical protection journals or restart/release the user's native controls to normalize this audit.

## Implemented behavior and safety

Flutter IPC is strict protocol5; native AppKit helper protocol remains4. The consultation lane has its own authenticated child/session/monotonic IDs, snapshot freshness, single pending work item, bounded channels and explicit accepted/succeeded/failed/rejected/uncertain results. It does not run collection on the UI/pipe thread.

### Real recorded history

`src/history/reader.rs` reads the existing schema3 JSONL history, newest **recorded** first, without synthesizing events from snapshots. Each page has up to50 records and scans up to512KiB. Records are bounded at64KiB; four captured file ranges are bounded at5MiB+64KiB each. A short shared writer lock captures read-only handles, never held while the user browses. Snapshot cursors are browser-local, opaque, single-use and expire after5minutes. Rotation, append, deletion/clear, malformed/future records, unsupported fields and oversized content have explicit outcomes; partial or empty results never prove absence of access. Original event timestamps/timezones are retained.

Flutter provides chronological rows, resource/activity/confidence, application/PID when recorded, Refresh/Load more, native errors and stale/partial labels. Query/pagination is user-driven, not repeated on every observer tick. No clear/export/destructive history command was added.

### Detailed diagnostics

An on-demand worker runs the existing passive native doctor independently of the observer, with bounded checks/text. It does not resume a protection or grant elevation/permissions. The temporary monitor's `policy` row is omitted: its default policy is not the active observer's policy. The response explicitly identifies this separate point-in-time diagnostic context; current applied profile remains in the live observer snapshot. Technical native detail is preserved; UI framing is translated into seven languages.

### Installation-scoped autostart

Startup status and confirmed CAS writes reuse native backends, ownership locks, rollback and readback. The user confirms a change for **this installation**, not another installed copy. Unix checks the physical current executable and version against the registration or retained journal under the same lock; a foreign registration/journal is preserved, including when its target is absent. Existing CLI migration semantics remain unchanged.

Only a trusted native operational context can admit startup mutation; the worker repeats the gate. All development/read-only/settings qualification sessions reject such writes, even with a valid confirmed request. Flutter requires fresh known native state, confirmation tied to the same session/readback, and native success matching the requested state. Unknown/stale/timeout/refusal does not become an enabled/disabled success. Host autostart must not be modified to test this UI. Private filesystem CAS tests and disposable runners are separate from host registration.

### Review corrections retained

Independent source reviews found and verified the fixes for cross-installation Unix startup ownership, consultation-only updates advancing the Unix observer schedule, and temporary default policy being displayed as current. Presentation-only changes now publish without returning an observer-refresh signal. A parser test also caught Serde unit variants ignoring extra parameters: zero-field struct variants now enforce `deny_unknown_fields`. No security check, signature verification, polling cadence, sandbox or timeout was relaxed.

## Local checks actually observed so far

| Check | Observed result | Scope |
|---|---|---|
| Windows Rust check, all targets, locked features windows-tray/flutter-desktop | Passed | Compile, no hardware action |
| Windows Clippy all targets, `-D warnings` | Passed after addressing introduced warnings | Not a runtime guarantee |
| Rust desktop/IPC |50passed | Includes authorization/replay/read-only startup and strict DTO rejection |
| Rust history |11passed | Bounded real-file behavior, pagination and storage transitions |
| Rust Windows autostart |5passed | Existing isolated/pure backend contract tests, no host registration |
| Rust Windows platform tests |19passed | Includes scratch-buffer boundary/reuse behavior |
| Flutter analysis | No issues | Static analysis |
| Flutter suite |89passed on each Mac architecture and native Linux ARM64; x64 CI also passed | Initial local keyboard fixture had an unfinished request timer; targeted correction and later full suites passed. Not seven native visual reviews |
| Windows optimized Rust/Flutter builds | Passed, exact d7f778a products privately pinned | MSBuild warns about incremental builds under Temp; warning retained |
| Python/YAML parsing | Passed | Not execution of platform helpers |

Unix history12 and settings5 tests passed on both Mac architectures and ARM64. Installation-scoped autostart tests passed7 per Mac and9 on ARM64. Desktop/IPC tests passed56 per Mac and57 on ARM64. A separate native-menu filter initially selected **zero tests**: that step is not counted as a pass. The module is `frontends::tray`, not `frontends::tray::unix`; filter and child-fixture name were corrected and empty child execution is now rejected. CI41191181077 then selected and actually passed the two exact native guard tests on macOS Intel/ARM and Linux x64/ARM, without rebuilding Flutter. All four artifact digests, source commits, selected names and two-pass logs were independently verified.

The earlier ARM64 archive failure is preserved: the extractor now validates original ZIP names (before host normalization), rejects absolute/traversal/NUL/backslash/symlink members, and extracts only `dart-sdk/*`. Archive-level metadata/license remains in the original archive. Private malicious-layout smoke passed, followed by the actual native bootstrap/build described below.

The first local CPU run was interrupted after three valid individual intervals, before a complete paired round. Its original receipt remains unchanged and a separate interruption receipt records the missing completion. No paired gain is derived from those partial measurements; no owned B4 process remained on resume.

## macOS observed qualification and its boundary

The representative B4 architecture passed on **Intel and Apple Silicon** in CI41188772673. The existing production DesktopHost remains continuously alive in a read-only isolated mode. Actual native helper identity/hash and NSStatusItem visibility were checked before/after every Flutter close. Three reopen cycles dispatched through the native Status action into the same Rust supervisor, with new Flutter children, positive-sequence IPC and owned visible NSWindows. Singleton activation and native Exit passed. The exact uninstrumented release separately passed inherited-stdio IPC and owner-death cleanup; close/reopen/capture automation used a separately identified instrumented frontend, not a silently substituted release.

Owned signals dispatched the real enabled target/actions; no physical mouse-click claim. Ready15s, pipe5s, sandbox, signatures and permissions were unchanged. Both hosted macOS15/Xcode26.3 targets passed build, strict Clippy, automated suites, bundle/architecture/seal inspection and owned-window capture. Captured real Flutter overview images were examined in light mode on both. Native AppKit visual fixtures include simulated inputs, themes and languages and are labelled accordingly; they are not real Flutter command/device tests. Flutter macOS dark-mode native capture, new consultation-panel clicks, physical sensors/TCC and installed-package lifecycle remain unverified. No physical/manual Mac is a development gate.

The earlier window-only B3 owner lacked the permanent production AppKit host and stalled in WindowServer bootstrap after close. This representative permanent-host experiment no longer reproduces that failure. It does **not** establish the universal WindowServer root cause or qualify every macOS configuration. AppKit fallback remains.

## Linux graphics facts

Read-only inventory of the real Mint22.2 machine confirms the authorized user session is XRDP/XFCE/X11 `:10`, GLX llvmpipe/Mesa25.2.8, acceleration=false. Actual Flutter child threads/maps/fds confirm software rendering. An Intel i915 GPU exists but the user cannot read/write its render nodes. Seat0 is a LightDM greeter, not an authorized accelerated user desktop. No groups, render permissions, drivers, security or logins were changed.

The final private run `mint-b4-20261031-093523-88f7f9ae` passed12 cycles, six per theme: real SNI, IPC, preferences/errors/restoration, history from a byte-identical copy of two genuine preexisting events, Refresh, detailed native Doctor and read-only installation startup status. All owned UI/fixtures exited; stable process/executable/preferences/history/protection data were preserved. Original product tar/ELF hashes and modes remained unchanged. The separately pinned B4 sampler records missing memory explicitly when a short-lived process disappears with ESRCH; it does not invent a live process or weaken parent/lifecycle checks.

| Mint XRDP/X11 software renderer | Light | Dark |
|---|---:|---:|
| Closed RSS before consultation, MiB |9.79|9.80|
| Closed RSS after all cycles/consultation, MiB |12.00|11.96|
| Open aggregate RSS, MiB |316.61|329.54|
| Open aggregate PSS, MiB |285.31|298.29|
| Closed CPU, percent of one core, initial/final |1.40 /1.40|1.20 /2.00|
| Open CPU, percent of one core |25.50|28.49|
| First mapped window, ms |5101|5080|
| Median subsequent mapped window, ms |1059|1137|
| First positive native Ready, ms |1480|1496|
| Median subsequent native Ready, ms |814|793|
| Median process exit, ms |211|181|

CPU intervals were5seconds and omit short-lived children: descriptive lower bounds, not a long steady-state benchmark. RSS sums can double-count shared pages; PSS is reported separately. Open resource sampling covered the first cycle of each theme, not every reopening. Native Ready is not a raster measurement. Software-renderer cost remains unacceptable for claiming accelerated Linux performance; an already-authorized accelerated desktop is still needed, without permission changes.

## Native Linux ARM64 chain

Official Flutter3.47.8 release metadata has no Linux ARM64 SDK archive or upstream bundle SHA256. B4 therefore pins official Flutter Git revision737ba8f885dc774b53b442759999a312c9461e4e and enginecf636b1dfb818dd6a3ff16e390e93b1a036969c8, then bootstraps official native Dart/GTK artifacts on disposable Ubuntu24.04 ARM64. Observed official GCS MD5 transport digests and locally measured SHA256 are recorded honestly, not called signatures/upstream ARM SDK attestations.

The native Ubuntu24.04 ARM64 job actually passed Rust/Flutter release builds, strict Clippy/analysis,89Flutter tests, the native Rust suites listed above and complete bundle/hash/package verification. All four product ELFs are AArch64; interpreters, RPATH, dependencies and imported symbol versions are recorded. This is an Ubuntu24.04 host baseline, **not** an Ubuntu22.04/Mint21 compatibility claim.

Upstream engine artifacts contained an x86-64 frontend-server AOT component despite the ARM64 directory. The same pinned official Dart ARM SDK contains its native counterpart. The bootstrap explicitly assembles that exact official target component, verifies both component SHA256s and the full Dart archive MD5, rereads the result and retains the all-native ELF guard. It changes no Flutter source and accepts/executes no x64 compiler. See [Flutter#156309](https://github.com/flutter/flutter/issues/156309) and the upstream copy rule. This assembled SDK is not an official ARM SDK archive/signature/attestation. Original and native component hashes/provenance are retained.

No desktop ARM app was launched by the packaging helper. Native widget tests are not SNI/graphics runtime, GPU acceleration, physical SBC or capture-control validation. Those remain unverified.

## Performance method

The only B4 optimization so far reuses a lazy32768-unit UTF16 executable-path buffer across one camera forensic pass, instead of allocating/zeroing it for each process. Process generation/path/module/signature/trust checks remain fresh and in the same order. No verdict cache or cadence change.

A complete clean Windows comparison used immutable B3 `7e794d2` against B4 engine d7f778a plus rebuilt frontend7e47596, whose only payload change from the frozen d7f package is the native runner's timing protocol5 fix. Two AB/BA rounds,60seconds per closed/open interval, isolated app-data and equivalent normalized boundary observations passed. The earlier interrupted receipt remains separate and is never spliced into this comparison.

| Windows measurement, two-round descriptive mean | B3 closed | B4 closed | B3 open | B4 open |
|---|---:|---:|---:|---:|
| CPU, percent of one logical core |3.333|3.177|7.894|8.155|
| Aggregate RSS, MiB |17.94|18.00|134.30|135.23|
| Process count |1|1|2|2|

Closed round deltas have opposite signs (+0.052 and−0.365percentage points); open deltas are +0.228 and+0.293. **No established CPU speedup**: the reusable buffer avoids redundant allocation but these measurements do not show a general performance improvement. Different B1/B2/B3 workloads must not be compared as an apparent B4 gain. No verdict cache, relaxed trust check or cadence reduction was introduced.

Fresh native runner_started/engine_created events are accepted at protocol5. Protocol availability, first framework frame, mapped window and actual displayed pixels remain different measurements. Inclusive collector profiling is a separate experiment, never summed or mixed with clean whole-process CPU. No capture-active/device/trust-mutation detection-latency experiment was authorized.

The separate30-second profile observed56 completed snapshots per B3 interval and57 per B4 interval, with comparable sparse-sequence cadence1.88–1.90cycles/second. Inclusive outer snapshot CPU was2.08–2.44% of one core; native microphone enumeration/control-status observation was a substantial sampled scope. These nested scopes must not be added. This inactive workload emitted no signature/WinVerifyTrust/module-scan completion records, so their current cost is **not measured**, not proven zero or resolved. A signature-heavy/capture-active comparison and detection latency remain unverified; no real device/trust mutation was authorized.

In the clean new-runner experiment, positive-sequence protocol availability was623/541ms and child process exit343/366ms across the two B4 owners; this is not first-pixel timing or a large-sample benchmark. The separate consultation lifecycle continuation measured burst-reopen protocol readiness412–570ms. Its products differ only in the native timing emitter, and its measurements are kept distinct, not combined as a single performance distribution. The complete B4 private Windows bundle is39.59MiB, versus39.11MiB for its B3 reference.

## Windows actual UI and remaining qualification

Owned-PID MSAA exercised real History30→60→30 rows, matching recorded applications/timestamps/order, and detailed native Doctor in both themes. Four actual owned-HWND WGC captures were examined. A later WGC attempt refused a blank body; its failure is retained without fallback or replacement. A separate continuation omitted new captures explicitly and completed two burst reopen cycles per theme, reloaded actual history after each new child, observed Rust progress with Flutter absent and preserved the copied history bytes.

Startup known/unknown native readback and the development read-only explanation were exercised, never host startup writes. Native Windows accessibility could not verify the disabled button: MSAA omitted its unavailable state and UIA exposed only the FLUTTERVIEW pane. The continuation receipt is intentionally `exercised_with_unverified_accessibility`, not a fabricated accessibility pass. Dart gating and Rust rejection tests are separate proof, not manual screen-reader validation.

Final evidence is privately indexed in `B4-EVIDENCE-INDEX.json`; do not publish genuine history, local paths or raw diagnostics without privacy review. Full parity, camera/lock/maintenance and Unix microphone commands, persisted language/theme APIs, installed-package update qualification, accelerated Linux/Wayland and ARM graphics runtime are not delivered by B4. Host startup writes/device actions remain unqualified here, and native Windows disabled-button accessibility remains unverified. Keep native fallbacks. No stable replacement, main/work push, release/tag or deployment was performed.
