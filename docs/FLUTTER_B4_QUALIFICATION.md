# Flutter B4 — reliability, consultation and platform qualification

Development branch: `work/flutter-hybrid-0211`. This is **not** a public0.21.1 release, full functional parity, or permission to replace stable0.21.0. GTK/AppKit fallbacks remain.

## Source and evidence boundaries

- Initial B3 baseline: `14ae566edc44047533f4423ef30d75fcab429898`.
- B4 preservation checkpoint: `79b35db28537c06f692dd7b4d855f3dd35b8744a` (intentionally WIP).
- Reviewed command/qualification code: `d7f778a0b5199913d2bdc0596b9ff812f4c7efd9`.
- Only the previously authorized CI ref `ci/flutter-b3-20261030-081807` was pushed, with `[CF-Pages-Skip] [skip ci]`. Fresh [CI41120973121](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/41120973121) passed Ubuntu22.04 x64 including hosted runtime. Both Mac jobs stopped at a Linux-only field in the new qualification view; ARM64 stopped at archive-level Dart license metadata. Those preparation defects are corrected without relaxing path guards; only the three failed targets will be rebuilt. Their new results remain pending at this checkpoint.
- Private evidence root: `%LOCALAPPDATA%\MicCamWatchQualification\flutter-b4-20261030-202725`. It contains local application/process paths and genuine history data; do not publish raw evidence without privacy review.
- Main/origin73369ab, stable tag06d800b→3d74686, installed MSI,8worktrees and stash are preserved. Initial file and startup-registry receipts were taken before local UI tests.

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
| Flutter suite |88passed; keyboard fixture initially failed due an outstanding request timer | Fixture cleanup was corrected; affected keyboard test then passed separately. No failed case converted to success. Fresh full suites remain in CI |
| Windows optimized Rust/Flutter builds | Passed, exact d7f778a products privately pinned | MSBuild warns about incremental builds under Temp; warning retained |
| Python/YAML parsing | Passed | Not execution of platform helpers |

The new Unix ownership/CAS and native menu mutation-refusal tests await their actual Unix CI execution. Source review alone is not a passing test.

The first ARM64 bootstrap verified the official source and native Dart archive's pinned MD5 before refusing `LICENSE.dart_sdk_archive.md`. The corrected extractor validates original ZIP names (before host normalization), rejects absolute/traversal/NUL/backslash/symlink members, and extracts **only** `dart-sdk/*`. Archive-level metadata/license stays in the retained original archive. Private malicious-layout smoke passed; the actual native chain must still pass on the ARM64 runner.

The first local CPU run was interrupted after three valid individual intervals, before a complete paired round. Its original receipt remains unchanged and a separate interruption receipt records the missing completion. No paired gain is derived from those partial measurements; no owned B4 process remained on resume.

## macOS experiment and qualification criteria

The B3 Intel failure remains open until actual B4 evidence. B3's window-only development owner differed from production's permanent AppKit menu bar host. B4 now runs the **existing production DesktopHost** continuously in a read-only isolated mode. Ready, native helper identity/hash, real NSStatusItem visibility and registration are checked before/after every Flutter close. Reopening must dispatch through the real native Status action into the same Rust supervisor, create a new Flutter child, establish positive-sequence IPC and display an owned NSWindow. Three close/reopen cycles, singleton activation, native Exit and release owner-death cleanup are required.

Owned signals exercise actual enabled native target/actions; no physical mouse-click claim is made. Capture is owned-window-only and requires preexisting authorization; no global fallback or grant. Product Ready15s, pipe5s, sandbox/signatures and permissions are unchanged. Both Intel and Apple Silicon are requested from hosted macOS15/Xcode26.3, not a physical/manual Mac. Packaging/seals/architectures, unit tests, graphical automation and physical sensor functionality must be reported separately. No physical Mac is a development gate.

## Linux graphics facts

Read-only inventory of the real Mint22.2 machine confirms the authorized user session is XRDP/XFCE/X11 `:10`, GLX llvmpipe/Mesa25.2.8, acceleration=false. An Intel i915 GPU exists but the user cannot read/write its render nodes. Seat0 is a LightDM greeter, **not** an available authorized accelerated user desktop. Do not change groups, render-node permissions, drivers, session security or logins to obtain a passing GPU result. B3 software-renderer measurements remain historical, not accelerated B4 results. A future accelerated comparison requires an already-authorized user session whose actual Flutter renderer is verified.

## Native Linux ARM64 chain

Official Flutter3.47.8 release metadata has no Linux ARM64 SDK archive or upstream bundle SHA256. B4 therefore pins official Flutter Git revision737ba8f885dc774b53b442759999a312c9461e4e and enginecf636b1dfb818dd6a3ff16e390e93b1a036969c8, then bootstraps official native Dart/GTK artifacts on disposable Ubuntu24.04 ARM64. Observed official GCS MD5 transport digests and locally measured SHA256 are recorded honestly, not called signatures/upstream ARM SDK attestations.

The job must actually compile/test Rust and Flutter release, inspect native AArch64 ELF/interpreters/RPATH/dependencies/imported symbol floors, hash and package the complete bundle. It does not claim Ubuntu22.04/Mint21 compatibility, graphics runtime, GPU acceleration or physical SBC qualification merely because the native build succeeds. SDK/build execution is pending in CI at this checkpoint.

## Performance method

The only B4 optimization so far reuses a lazy32768-unit UTF16 executable-path buffer across one camera forensic pass, instead of allocating/zeroing it for each process. Process generation/path/module/signature/trust checks remain fresh and in the same order. No verdict cache or cadence change.

A clean Windows comparison runs the immutable B3 `7e794d2` baseline against d7f778a, two AB/BA rounds,60seconds per closed/open interval, isolated app-data, release builds and normalized before/after native CLI observations. RSS/private commit/USS/shareable RSS, process count, native readiness, close time and sparse observer cadence are recorded. Inclusive profiling/counts are a **separate** instrumented experiment, never added together or mixed with clean whole-process CPU. No capture-active/device/trust-mutation latency experiment is authorized in B4. Measurements are pending; no gain is claimed.

## Remaining qualification at this checkpoint

- Read the actual CI logs/receipts and classify each native target without inferring runtime from build success.
- Exercise real Windows History/Diagnostics/startup read-only UI, paging, errors, lifecycle and light/dark owned-window captures with private copied real history, not simulated production events.
- Complete the controlled CPU comparison before other local GUI workloads.
- Qualify a new Linux x64 bundle in the already-authorized Mint session if produced; retain the software-renderer limitation.
- Update this report, the migration matrix and ROADMAP with outcomes, commits and exact next gates. No native-interface removal, stable replacement, work/main push, release/tag or deployment.
