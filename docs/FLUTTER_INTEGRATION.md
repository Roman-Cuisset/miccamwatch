# 0.21.1 hybrid desktop — first functional read-only slice

Historical read-only/protocol1 design and measurements, preserved for the pinned first-slice commits. The current development branch uses **protocol2**, with Windows stage A microphone commands; see [the current command contract](FLUTTER_COMMANDS_STAGE_A.md) and [migration matrix](FLUTTER_MIGRATION_MATRIX.md). Do not use the protocol1 example as a current wire schema or relabel its measurements as interactive results.

Decision27October2026: retain hybrid Rust/tray + disposable Flutter presentation. No repeat of comparative study; source experiment/ui-native-flutter-20261027 at b8e1d7f. Integration worktree mcw-flutter-hybrid-0211, branch work/flutter-hybrid-0211, base main73369ab. No push/tag/release/install or existing MSI modification authorized.

## Ownership and boundaries

Rust retains collectors, control/restoration authorization, CLI, settings/history, native tray/notifications and lifecycle. Flutter only displays data. Existing native windows/control paths and seven-language implementations remain available; Linux/macOS native interfaces are not removed. The UI integration is opt-in via Cargo feature `flutter-desktop`; Windows package UI path is fixed relative to the current executable: `desktop/miccamwatch_desktop.exe`. No user-supplied executable/endpoint/command paths. Parent prepares that layout in an isolated qualification prefix, never the MSI prefix.

Local execution alongside stable uses explicit **read-only development tray** (`mcw-tray --read-only-development`, only available with flutter-desktop). Distinct fixed read-only singleton/class, real PlatformMonitor snapshots and passive status queries, no control workers, no requested-protection resume, no automatic lock/restoration, no settings/history/autostart writes or notifications. It is not a second operational namespace allowed to mutate hardware. Normal tray code retains all existing control/security paths. This mode is a genuine collector/tray integration, not a simulator; tests must name its limits.

## Wire protocol1 — inherited private pipes only

Parent creates anonymous stdin/out pipes for its one owned child, with a restricted inherited-handle list. Child starts suspended, is enclosed in kill-on-close Job Object before execution, then resumed. stdout may contain native engine logging: accept only bounded JSON protocol event lines (max4096bytes); never accept arbitrary commands. Flutter receives newline-delimited JSON snapshots on stdin, max262144bytes per frame, UTF8. Rust keeps at most the latest pending presentation snapshot; slow/closed UI cannot block collection, retain unbounded history or stop Rust. No TCP/listening pipe/global endpoint, token/secret in argv, or disk snapshot files. EOF marks disconnected and terminates UI; child crash/close does not release protection. Fixed trusted bundle path validation rejects links/reparse points, nonregular/missing executable; only that owned child process handle/PID may be activated/closed. Read-only UI cannot request any device, permission, settings/history mutation or shell command.

### Snapshot JSON

```
{
 "type":"snapshot", "protocol_version":1, "sequence":42,
 "observed_at":"2026-10-27T08:00:00Z",
 "engine":{"version":"0.21.0","platform":"windows","mode":"read_only_development","state":"degraded","detail":null},
 "locale":"fr",
 "resources":[
  {"resource":"microphone","activity":"active","coverage":"complete","confidence":"high","active_count":1,"ready_count":0},
  {"resource":"camera","activity":"unknown","coverage":"incomplete","confidence":null,"active_count":0,"ready_count":0}
 ],
 "accesses":[{"resource":"microphone","activity":"active","confidence":"high","application":"application name from engine","pid":123,"device":null,"started_at":null}],
 "collectors":[{"collector":"wasapi","state":"healthy","detail":null}],
 "observation_gaps":["camera"],
 "protection":{"microphone":{"state":"unknown","requested":null,"service_active":null,"detail":null},"camera":{"state":"unknown","requested":null,"service_active":null,"detail":null}},
 "diagnostics":[], "truncated":false
}
```

Examples above specify shape, not fixture data to ship in app. Exactly two resources, ordered microphone/camera. `activity`: active, ready, none_observed, unknown. `coverage`: complete/incomplete/unavailable. `confidence`: high/medium/low/null. Presence of known Active is retained even when coverage incomplete; confidence describes observed activity only. No accesses plus degraded/unavailable relevant collector or observation gap => unknown, never safe/blocked. Windows relevant groups: wasapi microphone; privacy_store/module_scanner/sensor_activity camera; unknown collector failure affects both. Preserve exact collector names/messages and observation_gaps; no fabricated diagnostics. Empty collector evidence => unknown. No-protection observation => unknown, not off. Successful passive protection readings preserve desired vs actual/service state; microphone mute is not capture-access denial. UI must label all limits.

Only application display name, PID, resource/activity/confidence/device/start time are exported from Access: no executable/command line/ancestry/module/signature/user dump. Bound lists to128accesses,64collectors/diagnostics, string lengths1024UTF8bytes; set truncated when any data clipped. Export fixed settings summary only if explicitly added to protocol; no fictive preferences/history.

### UI events

Child stdout emits JSON `{ "type":"ready", "protocol_version":1, "sequence":42 }` after a rendered frame with first accepted live snapshot, not just a Connecting view. `{ "type":"timing", "protocol_version":1, "stage":"runner_started|engine_created|dart_started|first_frame|snapshot_received", "elapsed_ms":0 }` may report child-relative times; parent separately timestamps event receipt. No mutation request variant exists. Unknown/invalid/oversized UI output is rejected and cannot invoke core behavior. Flutter validates exact protocol/type, bounded frame, monotonic increasing sequence and required types/enums; invalid/version mismatch shows truthful disconnected/error state rather than idle. After5s without fresh snapshot show stale/disconnected observation, not a safety assertion. EOF exits UI (no orphan engine). Timers/subscriptions canceled on close; no persistent animations or prewarming.

## Rust APIs for integration (agent contract)

`src/frontends/desktop.rs` (portable): public schema DTO `DesktopSnapshot`; `DesktopSnapshot::from_snapshot(sequence:u64, observed_at:DateTime<Utc>, locale:&str, snapshot:Result<&Snapshot,&str>, mode:&str) -> Self`; `set_protection(microphone:Result<&MicrophoneProtectionStatus,&str>, camera:Result<&CameraControlObservation,&str>)` Windows-only method; `encode() -> Result<Vec<u8>>` enforces max frame. Constants `PROTOCOL_VERSION`, `MAX_FRAME_BYTES`. DTO tests cover mixed health/unknown/retainedActive, clipping and bounded frames; no real hardware actions in tests.

`src/frontends/desktop/windows.rs`: `Supervisor::new() -> Result<Self>` fixes own sibling bundle path; lazy resources until open. `open(&mut self, initial:&DesktopSnapshot, theme:&str) -> Result<()>`, idempotently activates current child or queues one reopen while old child exits. `publish(&mut self, snapshot:&DesktopSnapshot) -> Result<()>` no-op only if no child (no serialization/allocation when closed). `poll(&mut self)` reaps owned process/nonblocking logs; `is_running(&self)->bool`; `close(&mut self)->Result<()>` requests own child WM_CLOSE; Drop closes owned job and pipes. Structured bounded lifecycle/timing logs to stderr for qualification, without snapshot payload/observed applications. No foreground/window control beyond tracked own child's matching title `MicCamWatch`. Parent owns wiring and Cargo features/imports, agent owns this module only.

## Agent ownership

Bridge agent: only src/frontends/desktop.rs and src/frontends/desktop/**. Flutter agent: only desktop/flutter/**, seeded from experiment. Parent: src/frontends/tray.rs, new tray/windows_observer.rs, src/bin/mcw-tray.rs, src/frontends/mod.rs, Cargo, docs, scripts/builds/qualification/Git and any shared edits. No builds/tests/lint/formatters mid-flight; parent runs after contributions settle. Agents never edit site/installers/stable files or other worktrees.

## Migration matrix — first slice, not full replacement

| Existing function | First Flutter slice | Retained path / next gate |
|---|---|---|
| Observed microphone/camera, application, confidence and coverage | Live read-only dashboard | Existing Rust collectors and native interfaces retained |
| Collector health/diagnostics | Live read-only diagnostics | Existing doctor/native diagnostics retained |
| Desired/actual protection | Passive status only; unknown retained | Native tray/CLI authorizations unchanged |
| Profile/preferences, notifications/sound, pause and autostart | Not migrated; no fake toggles | Native tray/window/CLI retained; later explicit approval |
| Microphone/camera controls, approval/resume/restoration | No Flutter command | Native Rust/privileged boundary and existing tests unchanged |
| History | Not migrated, no demo rows | Existing local history/native path retained |
| About/theme/navigation | Flutter local presentation, no settings writes | System-native conventions/7locale review needed |
| Open/reopen/single child/close | Rust supervisor + real Flutter process exit | All activation/install/upgrade paths need later package qualification |
| Linux/macOS + all5architectures | Native retained, portable DTO/Flutter structure only | Native build/physical/runtime gates, Intel deprecation and ARM toolchain risks remain |

## Stop after this slice

Deliver actual Windows readings/cycles, stage timings and memory, authentic owned-window light/dark captures, safety/preservation evidence, local commits and ROADMAP. Do not migrate full settings/control/history, remove native targets, push, install, tag or publish. Distinguish read-only local qualification from operational protection/installer/other-OS validation. User reviews before next slice.

## Reproduce the Windows slice without touching the stable installation

Use a clean integration checkout, Rust/MSVC, the Windows desktop C++ workload and the pinned study SDK Flutter3.47.8/Dart3.13.7. This is a development prefix, **not an MSI/update package**. Do not copy files into the installed application or Cargo bin directory. Do not run an older/default tray with development arguments: older released versions do not understand the mode; the new default build explicitly refuses it.

From the integration repository in PowerShell:

```powershell
cargo clippy --features windows-tray,flutter-desktop --all-targets -- -D warnings
cargo test --features windows-tray,flutter-desktop --lib frontends::desktop
cargo build --release --features windows-tray,flutter-desktop --bin mcw --bin mcw-tray
$repo = (Get-Location).Path
$source = (git rev-parse HEAD).Trim()
Push-Location desktop/flutter
flutter pub get
flutter analyze
flutter test
flutter build windows --release --no-pub
Pop-Location

$prefix = Join-Path $env:LOCALAPPDATA ("MicCamWatchQualification\first-slice-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $prefix, "$prefix\desktop" | Out-Null
Copy-Item "$repo\target\release\mcw.exe", "$repo\target\release\mcw-tray.exe" $prefix
Copy-Item "$repo\desktop\flutter\build\windows\x64\runner\Release\*" "$prefix\desktop" -Recurse
@{
  mode = "read_only_development"
  feature = "flutter-desktop"
  source = $source
  renderer = "Windows Skia; Impeller disabled"
  engine_sha256 = (Get-FileHash "$prefix\mcw-tray.exe" -Algorithm SHA256).Hash.ToLowerInvariant()
  ui_sha256 = (Get-FileHash "$prefix\desktop\miccamwatch_desktop.exe" -Algorithm SHA256).Hash.ToLowerInvariant()
} | ConvertTo-Json | Set-Content "$prefix\qualification-manifest.json" -Encoding utf8

# Python3 + psutil + Pillow, only for qualification; no Python app dependency.
python desktop/qualification/windows.py --prefix $prefix --out "$prefix\evidence" `
  --trials 3 --cycles 3 --idle-seconds 4 --captures --lifecycle

# Inspect manually afterwards; close this development tray when finished.
& "$prefix\mcw-tray.exe" --read-only-development --desktop-theme dark --open
```

Do not interact with the development tray during timed phases; the runner rejects changed process membership, rather than counting an unexpected opening as idle. It uses only owned PID/HWND messages/captures, never desktop-wide mouse input or process-name termination. `--lifecycle` deliberately terminates its exact owned Flutter child, confirms Rust collection continues, reopens, then shuts the test parent with UI still open to check job containment. It never stops the stable MSI tray.

For offline runtime inspection keep the complete `desktop` directory; the executable alone is insufficient. No Flutter SDK, Python, development console, network service or resident UI engine is needed by that runtime. The test manifest is a local build identity check, **not a publisher signature or cryptographic attestation**. Same-user process injection and arbitrary modification of the user's own executable bundle are not solved by this bridge.
