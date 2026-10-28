# Flutter preferences B1 — Windows development tranche

28 October 2026. Runtime source: `a0411ea80fd985c9aa319061c4ad3a1830880aeb`, following `65b5ae8` and `3a661e8`. This is not a public release or a replacement for installed MSI 0.21.0.

## Delivered scope

Flutter Settings can change the existing notification, sound and future-history preferences, and the assessment profile. Rust persists them. All other fields are merged from the latest native store under its stable lock. Assessment profiles are not permissions or device-blocking controls.

Saved notification/sound flags do **not** establish capture-alert delivery: the Windows tray does not currently wire these effects. The UI states this explicitly. The settings-only test observer does not record history; its UI reports that limitation. The operational tray integration reports its native history-recording setting separately, but was not launched against the owner's stable installation.

No camera, automatic lock, autostart, permissions, history deletion or installer command was added. No existing native interface was removed. Theme remains local to the disposable UI; no fictitious persisted language/theme API was introduced.

## Protocol 3 and authority

This is a clean protocol cutover: Rust, Dart and Windows runner timing events use version 3; no compatibility shim accepts version 2.

- The existing private inherited pipes and per-child CSPRNG challenge remain the only command channel. No listening network endpoint, command-line/environment token or raw shell facility was added.
- A separate `preferences_command` lane has monotonic IDs, one pending request and a bounded result. The microphone lane and its generation/confirmation checks remain separate.
- Exact request fields: `session`, `id`, `action`, `observed_sequence`, `key`, `value`, `expected_value`. Action is only `set_preference`; keys are only `notifications_enabled`, `sound_enabled`, `history_enabled`, `profile`. Values are booleans for the first three, or `private|meeting|development|balanced` for profile. Unknown fields, wrong types, stale sessions, replay, unavailable capabilities and stale expected values are rejected.
- Shared envelope admission checks authenticated child session, bounded counters, readiness, observation sequence and current native capability freshness. The filesystem remains subject to normal OS permissions, without elevation.
- `Settings::update_checked` compares the expected field value against the latest store **inside the existing stable lock**, mutates only that field and saves only on success. Invalid stores are not replaced with defaults. A native readback must confirm the target field before success.
- A lazy bounded persistence worker belongs to the Rust process. It is not cancelled when Flutter closes. Normal exit/update-stop remains busy while its write/readback is pending. The native collector and tray do not wait on that settings lock.
- Queue failure, permission failure, readback error, timeout and uncertain completion are explicit. A UI timeout is not native cancellation, and never triggers an automatic retry. Replacing the UI cannot attach an old command acknowledgement to its new session.
- Saved `values.profile` and collector-observed `applied_profile` are independent. Explicit configuration disables profile editing, not the three boolean preferences. Effect support and effect-active fields are separate from saved values.

The authority is a child capability boundary, not a claim that an administrator or a debugger with equivalent process access is excluded.

## Real Windows smoke

Private evidence: `%LOCALAPPDATA%\MicCamWatchQualification\flutter-preferences-b1-20261028-142703`.

The valid runtime is `runtime-settings-v3`, not the initial stale prefixes. Its 15-file manifest pins the built engine, CLI and Flutter payload. Test mode `--settings-development` verifies dedicated data/config paths under the qualification root and never creates a microphone worker, resumes protections or applies an automatic device policy.

Observed on Windows 11, French UI, 125% DPI:

1. Opened Flutter from the Rust development tray; real collector/device states were displayed, with device commands disabled.
2. Changed notifications, sound and future-history recording from the actual accessible controls. Native write/readback succeeded; each changed only its selected field.
3. Changed Balanced to Private; saved selection and independently collector-applied Private were observed in the accessible tree and real window.
4. Marked only the isolated test preferences file read-only. The native write failed, the UI exposed an error, and file bytes and values remained unchanged. The test file attribute was restored in `finally`.
5. Held only the isolated settings lock. The UI timed out to uncertain without saving optimistically; observer ticks continued. Closing Flutter did not cancel the pending write. After releasing the lock, the native write/readback succeeded.
6. Repeated the replacement-child portion with the accessibility tree explicitly initialized: three open requests produced one child; the replacement UI reported the pending native write and could not pretend to save it. After unlock, the stored value changed through the original process-owned worker.
7. Restored every test preference through Flutter, including the assessment profile; all original test fields matched. Closed Flutter; only Rust remained and its tick counter advanced. Normal tray exit left no owned process.

The automation used MSAA actions on the exact owned HWND/PID, not global mouse movement. It observed the same Flutter callbacks, native IPC and filesystem as an interactive user. It is not a complete screen-reader or keyboard audit.

### Preserved tooling failures

- The first build command specified `flutter-desktop` without `windows-tray`. Cargo legitimately skipped the tray binary, and the packaging step copied a stale stage-A tray. Startup did not produce the expected development HWND. Those prefixes and their source-commit claim are invalid and excluded. The constructor stopped only its owned process. Stable non-history files remained identical. Correct builds explicitly use both features.
- Initial coordinate-based synthetic gestures were not reliable; no preference save occurred. Their captures are not command-success evidence. They were replaced by exact accessible actions, without moving the system pointer.
- A first replacement-child accessibility action ran before its new semantic tree was ready. The lock was released safely in `finally`; native completion survived UI closure, but the replacement gate was not claimed from that attempt. The separate semantic-ready scenario proved the gate.
- MSBuild MSB8029 warns that this isolated worktree resides under Temp. Builds succeeded. This is not an installer qualification.

## Automated/build evidence

- 42 Rust desktop admission/protocol/state tests and 4 native settings persistence/concurrency/error tests passed.
- Flutter analysis passed; 47 Flutter tests passed, including 20 preference tests. An initial dropdown hit-target warning was corrected and those 20 tests passed again without that warning.
- `cargo check --all-targets --features windows-tray,flutter-desktop`, matching strict Clippy `-D warnings`, and explicit release builds of both native binaries passed.
- The check without Flutter passed. Flutter Windows release build passed.
- Python qualification modules compiled/imported. Actual settings Host lifecycle and strict manifest validation were exercised.

Reproduction builds:

```powershell
cargo check --all-targets --features windows-tray,flutter-desktop
cargo clippy --all-targets --features windows-tray,flutter-desktop -- -D warnings
cargo test --lib --features windows-tray,flutter-desktop frontends::desktop
cargo test --lib --features windows-tray,flutter-desktop settings::tests
cargo build --release --features windows-tray,flutter-desktop --bins
# Use the already configured Flutter SDK, within desktop/flutter:
flutter analyze
flutter test
flutter build windows --release
```

Do not copy a pre-existing `target/release/mcw-tray.exe` after a build that skipped its required feature. Verify producer flags and binary identity before making a qualification manifest.

## CPU and resource results

The previously instrumented camera path spent most measured CPU in repeated signature verification; the 47.79% sample had two Ready pipelines and is not comparable to the later one-pipeline sample. A pinned file or unchanged path/mtime does not freeze Windows trust policy, revocation state or process identity. No signature-verdict cache was added. Process inventories from different collection phases were not silently merged.

The only optimization skips the sensor's process inventory when **all freshly collected sensor reports are empty**. Sensor collection, nonempty-report attribution order, signature checks and the 500 ms observer pause remain unchanged. This is not a claim to have solved dominant signature costs.

`desktop/qualification/compare_cpu.py` ran manifest-pinned stage-A and B1 read-only builds in alternating AB/BA order: two rounds, 20 seconds per open/closed interval. All paired CLI boundaries agreed after declared normalization: no confirmed microphone capture, one low-confidence camera Ready row, the same process identity and healthy collector reports. This is not proof of universal inactivity or unchanged unobserved activity between boundaries. CPU is percent of one logical core; process sums can double-count shared pages; GPU/DWM are excluded.

No controlled microphone/camera capture, synthetic camera DLL, permission/registry mutation or multiple-application capture fixture was authorized or run in B1. Unmeasured scenarios must not be filled with projections.

| Measure | Stage A `34e4e94` | B1 `a0411ea` |
|---|---:|---:|
| Rust alone, CPU (% one logical core) | 16.73 | 16.21 |
| Rust + Flutter, CPU (% one logical core) | 19.04 | 16.37 |
| Rust alone, RSS (MiB) | 20.73 | 20.66 |
| Rust + Flutter, RSS (MiB) | 131.05 | 135.25 |
| Rust alone, private commit (MiB) | 4.25 | 4.11 |
| Rust + Flutter, private commit (MiB) | 116.83 | 119.76 |
| Rust alone, unique resident memory (MiB) | 3.28 | 3.17 |
| Rust + Flutter, unique resident memory (MiB) | 74.83 | 77.82 |

These are descriptive medians from a small matched sample, not a statistically established speedup. Closed-window paired CPU deltas were +0.46 and −1.50 percentage points. **The dominant CPU problem is not resolved.** Sparse observer sequences indicate approximately 0.81–0.86 cycles/second with this real workload; the pause/capture window were not reduced. New per-operation profiler counts are unavailable in this uninstrumented build, not zero; the prior signature-attribution evidence remains separately revision-pinned. Hardware detection latency and capture-active scenarios were not measured.

Separate B1 settings-mode lifecycle run: six opens/closes, three concurrent opening requests in one cycle, one child when open and zero after every close. First protocol-ready was 541 ms; five reopens had median 562 ms (504–619); process exit median 423 ms. These are protocol-ready and process lifecycle measurements, **not first usable raster**, and caches were not purged. They do not demonstrate an opening-speed gain over stage A.

In that separate context, median RSS was 19.53 MiB before, 131.73 MiB open, 20.85 MiB after six closes; private commit 4.09/119.13/4.23 MiB. A further real-write probe included the lazy persistence worker: after saving/restoring preferences and closing Flutter, Rust alone used 20.27 MiB RSS, 5.12 MiB private commit and one process. Its 19.26% one-core CPU is an unpaired sample in a different interval, not a causal worker-overhead comparison.

The complete tested bundle is 38.22 MiB/15 files, including a 25.88 MiB Flutter payload; no new production Dart package was added. Evidence: `cpu-abba/comparison.json`, `resource-cycles.json`, `post-write-resources.json`, `interactive-preferences-final.json`, and the two `runtime-*-v3/qualification-manifest.json` files.

Real captures examined: `preferences-restored-light.png` and `preferences-confirmed-dark.png`, French/125% DPI, exact owned HWND/PID via Windows.Graphics.Capture. No global desktop capture. Other DPI, keyboard/screen readers and seven-language desktop visual qualification remain open.

### Safely opening this development UI again

Use `desktop/qualification/windows.py`'s `Host(prefix, theme, environment=isolated_environment, settings_development=True)` with the valid `runtime-settings-v3` prefix, **not** the MSI or an arbitrary old binary. Its docstring gives the isolated APPDATA/LOCALAPPDATA setup. Create both `MicCamWatch` subdirectories before launch. The user can then use the actual Settings controls; finish with `host.stop()` in `finally`. No device action is available in this mode. The local manifest is a reproducibility identity check, not a publisher signature.

## Preservation and saved state

Final checks compared nine accessible stable-installation/data files. Stable binaries and every non-history file were byte-identical; the original history prefix was preserved. The installed MSI tray remained PID 2888, identified by its executable path. No B1 test process remained. All isolated test preferences were restored. Only the two invalid runtime copies created by this mission were removed, after preserving their manifests; the valid v3 runtimes and immutable stage-A runtime remain available.

The eight worktrees and existing stash are preserved. `main` and local `origin/main` remain `73369abda09bdadce8c779a9559b3e2334a32ecb`; annotated `v0.21.0` remains object `06d800ba536b5e6945abe4a4345957ba85c79cbf`, target `3d74686ace7b04728794765f9eb57c0c0a8bc84c`. No push, merge, tag, release, site deployment or installed application update occurred.

## Camera and microphone preservation gates

The existing Windows manual release deliberately unmutes supported active inputs; it does not restore individual pre-existing mute values. Its labels/confirmation remain explicit. Windows automatic lock restoration has a separate owned-original journal; it was not changed. Unix owned-original release must not be projected as Windows all-input unmute. Any future ownership-preserving manual restoration is a separately designed/approved native contract, not an unannounced Dart change.

Future camera commands must reuse native mechanisms and report intent, present-device state, pending restoration and arrival-guard health separately:

- Windows: explicit UAC through the existing fixed helper; resume a requested guard is not Allow→Block. Restore only authenticated owned disables; legacy recovery remains a separate one-target authorization. No device/unknown target state is not global blocking proof.
- Linux: existing trusted Polkit/helper flow and generation-bound original bindings; USB/UVC and kernel constraints remain. No non-USB/global capture denial, hotplug guard or camera-on-lock is invented. UI timeout cannot cancel or disguise a native authorization operation.
- macOS: opening the owned restrictions profile is pending manual System Settings approval, not effective blocking. Read-only administrator profile inspection does not grant TCC. External/MDM restrictions remain independent.

No camera command is enabled by B1.

## Linux/macOS preparation, not qualification

The shared Dart app, strict DTOs and safe preference domain can be reused. There is no Unix Flutter producer/supervisor in this tranche. Preserve native GTK3/AppKit fallback until these gates pass:

1. Add an owned child lifecycle with private inherited transport, per-child challenge, strict framing and parent/child EOF/error handling; retain native instance activation and authority. Linux native peer activation currently requires `SO_PEERPIDFD`/kernel 6.5+, independent of future Flutter pipe transport. Do not weaken that authentication to make a port appear to work.
2. Qualify open/reopen/close/crash/parent-exit on Linux x64/ARM64 and macOS Intel/Apple Silicon. No resident engine after close, no orphan or second UI, no Dart device authority.
3. Wire platform-specific microphone capability/restore semantics and effect support before enabling those actions; current Windows requested-guard projection is not a generic Unix implementation.
4. Own the complete architecture-matched payload in installation/update/uninstall receipts. Current native CI/package passes do not build or qualify Flutter. No new remote CI was run because no push is authorized.
5. Resolve Linux/macOS runner identifiers and stock macOS app icons against the existing official installation identity. Do not rename native installed identities blindly. Windows title/sidebar assets are official and were visually observed; Unix Flutter assets/packaging are not certified.
6. Ship the actual OS-version intersection: the inspected Flutter macOS target is 12.0, but current native package/engine/helper require 15.0. App Sandbox, inherited pipes and signing/sealing require real checks. An ad-hoc seal is not Developer ID notarization.

No Linux ARM physical validation, new Unix Flutter build, macOS visual test, installer upgrade or stable installation mutation is claimed. Seven-language widget behavior is covered; real B1 desktop captures are French only.

## Next tranche

After owner review: notification pause/resume and explicit autostart inspection/commands with isolated registration tests; existing device/doctor detail presentation; then separately gated camera/lock commands and Unix child lifecycle/packaging. A history browser/export is not an existing native API to claim as migrated. No publication or full migration is authorized by this checkpoint.
