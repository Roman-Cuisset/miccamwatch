# Native feature migration inventory — stages A, B1 and B2

Source baseline: `b5cf391c1905cbc391c47220f1347c55ba8e24cc`, 27 October 2026. Inspection only; runtime qualification is recorded separately. This inventory preserves every native/public CLI capability and its actual limitations.

The inventory priorities mean **A = microphone**, **B = presentation/preferences**, **C = sensitive operations**; the owner's cross-platform and visual-finalization gates remain separate. Camera/lock/maintenance actions still require their command-specific gates. Current protocol3 and B1 evidence: [FLUTTER_PREFERENCES_B1.md](FLUTTER_PREFERENCES_B1.md). [Protocol2 stage-A history](FLUTTER_COMMANDS_STAGE_A.md) is retained, not a compatibility contract for the current runtime.

Stage A implementation update: Windows now has explicit desired-state microphone commands through the existing Rust worker, private protocol2 admission and native readback. Real mute/release, UI process exit, retained native guard and reopening without duplicate UI have been exercised. Other controls/platforms remain gated; this inventory's source references and “inspection only” evidence describe the pinned baseline, not the later runtime qualification. Sidebar/Windows branding now uses official assets; macOS/Linux Flutter packaging identity remains unqualified. See the current stage A results in the ROADMAP.

## Current B2 qualification — 29 October 2026

Paired performance checkpoint `d37c8e8`; final code/build and Windows/Linux lifecycle smoke checkpoint `9aa4689`. See [B2 results, methods and remaining gates](FLUTTER_B2_QUALIFICATION.md). This is not full parity, installed-package qualification or a public0.21.1 release.

| Function / platform | B2 state actually established | Remaining gate |
|---|---|---|
| Windows capture notifications and sound | Existing Rust snapshots/transitions connected to a bounded native worker; live master/pause/audio preferences, cooldown and failures. TEST pipeline submitted while Flutter was absent; actual Flutter writes persisted; OS history confirmed silent/sound requests. | Physical audibility/banner, controlled genuine capture and other Windows configurations not tested. Operational path compiled, not installed over stable MSI. |
| Four safe preferences | Windows actual GUI/native CAS/readback retained; same Dart components and typed preferences wired to Unix owner. | Linux GUI mutation not yet exercised; no macOS runtime. History flag controls future recording only and is explicitly ineffective in development mode. |
| Windows background CPU | Fresh kernel generation inventory with immutable birth facts only; matched closed CPU17.53→11.60%, signatures fresh, cadence not reduced. | Non-negligible remaining signature cost; no universal/significance/Windows10 gain claim. Active-capture and trust-mutation latency cases untested. |
| Linux x64 frontend | Rust and Flutter release builds, real WSLg X11 display/IPC snapshots, light/dark captures, six close/reopens, same-child activation and stopped-child parent-death check. Unknown/unavailable media sources displayed honestly. | Real SNI tray/native fallback surface, hardware controls, native Wayland, package lifecycle and physical Linux/GPU not qualified. Software llvmpipe open CPU39.09% remains a performance limitation. |
| Linux ARM64 | Architecture-neutral Unix route prepared; original ELF/installer checks retained. | No new Flutter ARM64 build/package/runtime or physical ARM validation. Earlier native CI is not B2 proof. |
| macOS Intel / Apple Silicon | Unix route, official runner branding, activation and independent native parent monitoring prepared; AppKit retained. | No available macOS/Xcode host: all builds, sandbox/IPC, execution, visuals, controls and signing/package checks remain open. Respect native macOS15 minimum. |
| Common appearance | Same Dart app/protocol3, navigation, cards, preferences, themes and official sidebar asset; seven-language widget checks on Windows/Linux. | Real visuals only French125% Windows and English100% Linux; not seven native visual reviews or full accessibility/DPI qualification. |

Still absent from Flutter: pause/resume UI (native CLI was exercised), autostart, full doctor/history presentation, camera/lock/maintenance flows and platform-specific Unix microphone controls. No browser/export API or persisted language/theme API is invented. Windows explicit release still unmutes supported active inputs rather than restoring each original; B2 does not invoke or generalize it. Bundle-aware installation/update/uninstall is not qualified by private prefixes. GTK/AppKit must remain until replacements pass their real gates.


### Historical B1 handoff

| Priority | Remaining function | Scope and gate |
|---|---|---|
| Delivered B1, Windows development | Notification/sound/history preferences and assessment profile | Four real Flutter commands: native locked CAS/merge, readback, separate applied policy; UI closure/timeout/permission failure exercised. Windows capture alerts remain explicitly unsupported. |
| P1, parallel | Collector CPU | Compare identical workloads, retain signatures and cadence; no stale verdict cache or invented Active capture. |
| P1, next control tranche | Camera and lock/protection options | Platform-specific authority, UAC/helper/profile ownership and restoration; no device-changing test without specific permission. Windows microphone release remains all-supported-input unmute, not Unix owned-original restore. |
| P2 | Notification pause, autostart, complete diagnostics and history presentation | Reuse bounded native operations; host autostart writes need safe qualification. Native history browser/export APIs do not yet exist. |
| P2 | Language/theme/version/update presentation | Local theme and version display already exist. Do not invent persisted language/theme settings or a generic installer-independent updater. |
| Cross-platform preparation | Linux x64/ARM64 and macOS Intel/Apple Silicon Flutter integration | Common app/protocol, authenticated owned process transport, native fallback retained until actual qualification; architecture/build preparation is not runtime or ARM hardware proof. |

B1 runtime `a0411ea` is compiled and exercised on Windows with isolated preferences; [results and limits](FLUTTER_PREFERENCES_B1.md). This is not full parity, installed-MSI qualification or a Unix Flutter port. Baseline line references below remain pinned, not current line numbers.

## Summary

- **Stage A can reuse the existing Windows microphone owner**, without migrating camera authorization, restoration, settings, or maintenance. The native tray microphone worker currently accepts **toggle**, not an explicit desired-state assignment; the underlying Rust API already supports `set_microphone_mute(bool)`.
- **Requested protection, effective SDK/PnP/profile state, and observed capture are independent.** Neither “mute requested” nor “SDK muted” means microphone permission denied.
- Baseline gaps at the pinned source revision: **no export command/history browser**, no tray/native process termination, unused persisted `show_ready`, Linux TUI camera controls deliberately disabled, and Windows tray preferences did **not** implement capture-event alerts/chimes. The current B2 section supersedes that Windows alert gap.

**Baseline evidence scope:** source and asset inspection only. The later B1/B2 runtime results above are separate evidence, not retroactive validation of these pinned line references.

## Files

The following aliases keep the matrices compact; line references below resolve to these paths.

| Alias | File | Holds |
|---|---|---|
| **CLI** | `src/frontends/cli.rs:6-336` | Public command/subcommand definitions and flags |
| **MAIN** | `src/main.rs:46-621` | CLI dispatch, settings/policy precedence, platform differences |
| **WT** | `src/frontends/tray.rs` | Windows tray menus, operation workers, native/Flutter activation, lock/restoration coordination |
| **WN** | `src/frontends/tray/windows_desktop.rs` | Win32 consultation, preferences, diagnostics copying, About |
| **U** | `src/frontends/tray/unix.rs` | Linux/macOS action vocabulary, native views, control dispatch, inspection worker |
| **GTK** | `src/frontends/tray/linux_desktop.rs` | GTK3 consultation and callbacks |
| **APP** | `native/macos_desktop.swift` | AppKit consultation/menu/About, copying, notification/sound effects |
| **SET** | `src/settings.rs:20-74,158-224` | All persisted application preferences, defaults, paths, update API |
| **HIST** | `src/history.rs:68-132` | JSONL history, rotation, clear |
| **GUIDE** | `docs/USAGE.md:130-309` | User-facing scope, permissions, restoration, policy and storage contracts |

Additional backend and branding files are identified precisely in the relevant tables.

## Architecture

CLI, TUI and native frontends ultimately call `PlatformMonitor`, privacy backends, settings/history, autostart and maintenance APIs. Windows microphone control is owned by a separate native COM/MTA guard; Windows camera control by an explicitly elevated coordinator. Unix controls have different scopes and ownership models.

The existing Flutter bridge is **presentation-only**: `UiEvent` accepts only `Ready` and `Timing`, with strict decoding—there is no mutation variant (`src/frontends/desktop.rs:353-401`). Flutter must request typed Rust operations through the parent, not acquire device/control authority itself.

## Report

### 1. Complete public CLI inventory

**Proposed migration labels:** **A** = microphone command slice, preserving current read-only dashboard; **B** = later observation/preferences/history presentation; **C** = later sensitive camera/process/maintenance integration. B/C grouping is a recommendation, not an already implemented protocol or a documented approved staging contract.

All commands below exist on Windows/Linux/macOS unless stated otherwise.

| Actual CLI surface | Actual behavior, support and consent | Proposed Flutter equivalent | Evidence |
|---|---|---|---|
| `status` | Current access snapshot. `--microphone`/`--camera`, `--risk expected\|unexplained\|suspicious\|blocked`, `--include-ready`, `--json`, `--no-color`. Filters do not authorize enforcement. Exit 1 for matching access; 2 for unavailable collectors, also degraded collectors on Unix. | Existing live overview; B for full CLI-detail parity | CLI:84-85,236-275,299-336; MAIN:147-177,611-621 |
| `watch` | START/UPDATE/STOP monitoring until Ctrl+C. Same filters/output; `--interval` default 750 ms/minimum 100; `--notify`, `--log PATH`, `--eventlog`, `--sound`, `--kill-unauthorized`, `--no-kill`. Kill flags conflict. Explicit log is independent of automatic history. | Rust continues monitoring; B for presentation/log workflows; C for termination | CLI:86-87,246-275; MAIN:178-209 |
| `devices` | Native microphone/camera inventory; JSON/no-color options. Inventory is not capture or permission proof. | B device inventory | CLI:88-89; MAIN:210-214 |
| `explain PID` | Filters a **current** snapshot to that PID, prints associated evidence; not historical process lookup. JSON/no-color. | B application detail | CLI:90-91,278-286; MAIN:215-232 |
| `doctor` | Native diagnostics, JSON/no-color. Unix returns 2 for diagnostic errors; Windows dispatch returns 0 after printing checks, so inspect check statuses rather than treating exit 0 as universal health. | Existing collector diagnostics are only a subset; B full doctor view | CLI:96-97; MAIN:237-250 |
| `mute`, `mute --toggle/-t`, `mute --status/-s`; `unmute` | Windows persistent requested protection/release; Unix supported scoped mute/restoration. `--status` is passive and takes precedence if also supplied with toggle. No existing extra confirmation dialog/UAC for microphone operations. Platform semantics below. | **A** explicit microphone request/release and actual-state feedback | CLI:98-103,288-297; MAIN:251-274,555-608 |
| `top` | Interactive TUI. `m` microphone; `b` camera block; `a` camera restore; `k` terminate; `r` refresh; `q`/Esc quit; selection keys. **Linux camera buttons/actions are disabled**, despite Linux CLI/tray camera support. | A microphone; B observation/navigation; C camera/process actions | CLI:104-105; `src/frontends/tui.rs:546-554,649-807,1186-1216,2319-2324` |
| `tray` / `tray run` | Background native frontend; optional Unix `--eventlog`. Plain startup does not intentionally open consultation. | Keep Rust/native tray | CLI:106-115,146-156; MAIN:279-296 |
| `tray open` | Opens/reopens owned consultation, reusing the frontend where supported. With `flutter-desktop`, Windows ordinary opening uses Flutter; native controls remain separately accessible. | Existing supervised open/reopen | MAIN:71-87,297-313; WT:2054-2063,2092-2150 |
| `tray stop`, `tray status` | Stop/query frontend, not microphone/camera release. Status prints running/stopped, nonzero if stopped. Windows refuses close while accepted privacy/restoration work remains pending. | Preserve owned lifecycle; never equate window close with protection release | MAIN:314-330; WT:862-886 |
| `camera status\|block\|allow\|toggle` | Same command names, materially different platform controls; see protection matrix. Not a per-app permissions editor. | C; retain native paths during A | CLI:158-180; MAIN:333-360,371-403,528-551 |
| Windows `camera allow --restore-legacy INSTANCE_ID` | Explicit authorization to recover **one** historical camera; may override independent disable. Not blanket adoption of writable legacy records. New UAC authorization when restoration is necessary. | C explicit recovery workflow | CLI:170-176; `src/privacy.rs:418-423`; `src/privacy/windows_guard.rs:2226-2287` |
| macOS `camera inspect [--admin] [--json] [--no-color]` | Read-only profile metadata. `--admin` only requests expanded scope when passive scope is insufficient/failed; confirmation and administrator authorization, not camera TCC permission. Timestamped outcome/completeness; unknown exits 2. | C explicit diagnostic authorization, not a “grant camera permission” button | CLI:161-169; MAIN:374-388; `src/privacy/macos.rs:105-151` |
| `autostart status\|enable\|disable` | Per-user native registration; explicit opt-in, ownership checked. No blanket administrative startup service. Details below. | B | CLI:182-187; MAIN:404-420 |
| `profile [private\|meeting\|development\|balanced]` | Omitted argument queries; supplied argument saves preference. Changes assessment policy, **not device controls or an allowlist**. | B | CLI:123-128,191-198; MAIN:421-434 |
| `notifications status\|pause MINUTES\|resume` | Pause range **1–10080 minutes**. Does not stop monitoring/protection. There is no public CLI enable/disable subcommand for general notifications or sound. | B assignment/expiry UI | CLI:129-133,200-209; MAIN:435-461 |
| `lock-policy status\|enable [--microphone] [--camera]\|disable` | Windows: absent flags means both; enable sets restore-on-unlock true. Linux: microphone only; `--camera` refused; no flags selects microphone. macOS: **status and enable refused**, disable remains available to clear preferences. Applied by tray, not `watch`. | B policy preferences; retain existing native enforcement | CLI:134-138,211-221; MAIN:462-497 |
| `history path\|clear` | Locate/delete current and rotated application history. **Clear has no existing confirmation prompt.** Does not delete restoration records, explicit logs or OS journals. No `history list`, `show`, or `export`. | B real history/clear workflow; any new destructive confirmation would be new UI policy | CLI:223-227; MAIN:499-508; HIST:100-113 |
| `config validate\|path\|settings-path` | Validate selected/default TOML; missing policy rejected for validate. Print policy/preferences locations. No interactive policy editor. | B settings/diagnostic paths; leave policy editor unsupported unless explicitly added | CLI:229-235; MAIN:509-525 |
| `update` | Installation-aware release maintenance, not universal auto-update. See maintenance section. | C; no A maintenance mutation | CLI:92-93; MAIN:233-236 |
| `uninstall` | Exact-install ownership/removal or owner-specific instructions; user data retained. See maintenance section. | C; do not launch arbitrary deletion/elevation from Flutter | CLI:94-95; MAIN:68-70 |
| `help`, `-h/--help`, `-V/--version`; global `--config PATH`, `--lang CODE` | Seven languages: en/fr/de/es/ja/zh/ru. Config supplies TOML policy; language is CLI/policy/detected, not a persisted `Settings` language field. | Existing localization/About; B explicit language handling if desired | CLI:6-57,343-351; MAIN:129-145 |

**Other executable entrypoints, not additional public `mcw` subcommands:**

- Windows `mcw-tray.exe [--open]`; development feature-only `--read-only-development` and `--desktop-theme light|dark`. Development mode bypasses operational control startup, not a second operational hardware namespace (`src/bin/mcw-tray.rs:26-62`).
- Hidden Windows `__microphone-guard`, `__camera-guard --bootstrap`, `__uninstall-worker PARENT_PID PARENT_CREATED`, `__portable-receipt`; hidden Unix `autostart refresh`. These are internal/maintenance capabilities, not controls to expose as raw Flutter arguments (`CLI:61-83,182-189`; `MAIN:46-66,95-107`).

### 2. Exhaustive tray/native action matrix

`✓` means an actual exposed action, not merely an available backend. Native observation/diagnostic views also display read-only state.

| Function | Windows tray | Win32 window | Linux tray | GTK window | macOS menu | AppKit window |
|---|---|---|---|---|---|---|
| Open/reopen consultation | Menu/double-click | Retained window | Details/activation | Retained window | Open/double-click/Finder reopen | Retained window |
| Microphone toggle | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Camera toggle | ✓ | ✓ | ✓, prerequisite-gated | ✓, prerequisite-gated | ✓ | ✓ |
| Resume requested camera guard without Allow | Conditional UAC action | Conditional UAC action | — | — | — | — |
| Expanded read-only camera inspection | — | — | — | — | **Not in menu** | Conditional explicit admin inspection |
| Pause/resume notifications | **1 hour** | **1 hour** | **30 minutes** | **30 minutes** | **30 minutes** | **30 minutes** |
| General notification enable/disable | — | ✓ | — | ✓ | — | ✓ |
| Sound enable/disable | — | ✓ | — | ✓ | — | ✓ |
| Automatic history enable/disable | — | ✓ | — | ✓ | — | ✓ |
| Four profile choices | Submenu | Radio choices | Submenu | Combo | Submenu | Popup |
| Autostart enable/disable | ✓ | ✓ | ✓ | ✓ | **Excluded from menu** | ✓ |
| Microphone-on-lock, camera-on-lock, restore-on-unlock preference editing | — | ✓ | — | — | — | — |
| Full diagnostics / copying | Opens consultation | Copy all; selectable text | Opens consultation | Select/Ctrl+A/Ctrl+C | Opens consultation | Copy button; selection/Cmd+A/Cmd+C |
| About | Passive version row only | About page | Passive identity only | About page | About alert | No separate About tab |
| Exit frontend | ✓ | Closing hides window | ✓ | Closing hides window | ✓ | Closing leaves frontend running |
| Process kill; history clear/browser/export; update; uninstall | **None** | **None** | **None** | **None** | **None** | **None** |

**Action and widget evidence:**

- Windows tray dispatch/menu: **WT:806-854,2314-2439**. Feature-enabled tray additionally has **“Native settings and controls…”**, closing Flutter before opening the native window.
- Win32 actions/preferences/copy: **WN:851-896,1337-1388,2249-2320**; setting application **WT:2177-2227**.
- Unix action vocabulary and dispatch: **U:39-92,1538-1759,2084-2154,2239-2260**.
- Linux SNI activation/menu: **U:2297-2330,2396-2417**.
- GTK widgets/actions: **GTK:495-625**; callbacks guarded against render-triggered writes **GTK:291-316**.
- AppKit widgets/actions: **APP:673-713,905-924**. Menu explicitly excludes `camera_inspect` and `autostart`: **APP:987-1022**. About and reopening: **APP:1060-1075,1174-1202**.

Profiles are disabled in native menus/windows when **explicit `--config`** owns policy. CLI `profile` still saves the preference; saving does not override that explicit policy (`WT:1898-1918`; `U:2251-2260`; `MAIN:421-434`).

### 3. Protection strength, authorization and restoration

| Platform/control | Actual strength and limitations | Confirmation/permissions and existing APIs |
|---|---|---|
| **Windows microphone — A** | Persistent native owner reasserts **endpoint SDK mute**, including compatible topology changes. Survives TUI/tray/UI exit until explicit release. Hardware-mute capability counts are advertisements, not proof of a universal physical cutoff. Exclusive/ASIO/direct-driver/unsupported paths can bypass software scope. **Explicit release actively unmutes supported active endpoints; it is not “restore each original.”** Original-state restoration belongs to automatic lock policy. | No existing UAC/second confirmation. `PlatformMonitor::{set_microphone_mute(bool), toggle_microphone_mute(), microphone_protection_status()}`: `src/platform/windows.rs:663-672`. Intent/owner startup: `src/platform/windows_protection.rs:280-401`; limitations: `:500-548`; enforce/release: `src/platform/windows_mute.rs:443-550`. |
| **Linux microphone — future platform integration** | Writable capture-source **PipeWire session** mute, including supported virtual/monitor sources. Not ALSA permission denial, other PipeWire sessions or physical isolation. Retains/restores MCW-owned originals pinned to server/node identity; no Windows-style persistent guard. | Requires usable session and writable PipeWire controls; no elevation fallback. `Control::{set_microphone_mute,toggle_microphone_mute,begin_lock_mute,restore_lock_mute}`: `src/platform/linux/control.rs:135-215`; wrapper API `src/platform/linux.rs:39-56`; scope `:276-317`. Unmute can leave independently pre-muted sources muted. |
| **macOS microphone — future platform integration** | Only writable **CoreAudio INPUT mute properties**, master/channel controls independently. Missing/read-only controls unavailable; no universal microphone deny. Restores owned originals; refuses to unmute unrelated pre-muted inputs when no originals exist. No Windows-style persistent owner. | Native writable-property checks/readback, not TCC grant or administrative observation. `src/platform/macos/control.rs:372-482`; `native/macos_controls.swift:111-182`. |
| **Windows camera — C** | Global requested Block, including zero cameras and supported future arrivals while elevated owner runs. Native PnP disable for Camera class and positively identified legacy Image cameras—not arbitrary Image/scanner devices. Arrival-enforcement window, veto/restart/partial/unknown states remain possible. Allow restores **only authenticated MCW-owned disables**, with offline/pending restoration tracked. | Temporary **`runas` UAC** owner; new launch/resume/recovery can require approval. Allow is not guaranteed UAC-free if owner is absent and restoration requires elevation. `src/privacy.rs:368-423`; `src/privacy/windows_guard.rs:1317-1429,1595-1621,2001-2044,2140-2224`. |
| **Linux camera — C** | Optional Linux ≥5.9 **USB/UVC interface unbind**, not global camera permission denial. Supports conservative descriptor/configuration envelope; preserves unrelated composite audio/storage interfaces. No CSI/non-USB support, automatic arrival blocking or camera-on-lock. Restoration tied to recorded USB generation; changed/stale/partial state can remain unresolved. | Separately reviewed root-helper installation, matching version, trusted fixed paths, **explicit Polkit/pkexec authorization**. Preflight checks prerequisites, not consent/effectiveness. `src/privacy/linux.rs:66-163`; envelope/restoration contract `GUIDE:230-240`. Linux TUI intentionally does not expose these controls. |
| **macOS camera — C** | MCW-owned Restrictions profile with `allowCamera=false`. Preparing/opening profile is **pending, not blocked**. Allow removes only exact owned profile; external/MDM restrictions are not adopted. Installed metadata is not physical capture proof or per-app TCC management. | Explicit confirmation to open System Settings, followed by manual installation/removal approval/authentication there. `src/privacy/macos.rs:393-488`; `native/macos_controls.swift:426-475`. |
| **macOS profile inspection — C** | Separate timestamped read-only inventory with complete/incomplete scope and authorization outcome. Cancellation/denial leaves unknown; does not change camera intent/ownership, identify camera clients, establish lock state or grant TCC. | Native Continue/Cancel then fixed bounded administrator `profiles` query. Worker: **U:1450-1527**. API `camera_inspection(bool)`: `src/privacy/macos.rs:105-151`; authorization UI/query: `native/macos_controls.swift:377-424`. |

**Lock policy distinctions:**

- Windows startup’s first known lock state is a **baseline**, not a new enforcement request; unknown never triggers action. Microphone originals use token/generation/per-endpoint ownership; manual requests invalidate automatic ownership. Camera restoration is coordinated with pending manual/block/restore work. **WT:1005-1044**; `src/platform/windows_protection.rs:599-704`; `src/platform/windows_mute.rs:65-154`.
- Windows camera UAC cannot reliably be approved after locking: camera-on-lock is **not a dependable safeguard**; explicitly block before locking. **GUIDE:178-182**.
- Linux uses native graphical-session lock evidence and microphone ownership restoration; unknown is not unlock. Unix enforcement loop: **U:1887-1943**.
- macOS public lock signal is unavailable. The existence of lower-level microphone lock helpers does not make automatic macOS lock policy supported. `src/platform/unix.rs:85-88`; **MAIN:462-477**.

### 4. Every application preference and policy type

`Settings` has **nine fields**; unknown fields are rejected. Updates merge against latest settings under a stable process lock, so Flutter should reuse `Settings::update`, not rewrite a stale snapshot.

| Preference | Type / default | Existing user entrypoints and actual meaning | Flutter |
|---|---|---|---|
| `notifications_enabled` | `bool`, true | Win32/GTK/AppKit checkboxes; no CLI enable/disable | B |
| `sound_enabled` | `bool`, false | Native checkboxes; `watch --sound` requests playback, with platform distinctions below | B |
| `show_ready` | `bool`, false | **Persisted but unused in production source.** Tray collection explicitly includes ready independently; CLI uses `--include-ready`. No actual preference control to migrate. | Do not invent effective toggle |
| `history_enabled` | `bool`, true | Native checkboxes; controls future automatic history, not existing files/explicit log/system journal | B |
| `profile` | `PrivacyProfile`, Balanced | Four saved choices. Private→Strict; Development→Conservative; Meeting/Balanced→Balanced assessment policy | B |
| `pause_notifications_until` | `Option<DateTime<Utc>>`, None | CLI bounded minutes; native Windows 1h/Unix 30m pause/resume | B |
| `mute_on_lock` | `bool`, false | CLI lock-policy, Win32 checkbox; supported Windows/Linux execution only | B |
| `block_camera_on_lock` | `bool`, false | CLI lock-policy and Win32 checkbox; Windows only | B/C boundary |
| `restore_on_unlock` | `bool`, true | CLI enable forces true; Win32 editable checkbox; no separate CLI setter | B |

Evidence: **SET:20-74,88-147**; **WT:2177-2227**; **U:2118-2140,2239-2260**. Repository-wide `show_ready` references were limited to its schema/default and tests, not production consumption.

**Separate TOML policy types**, not additional `Settings` fields:

- `profile`: Conservative/Balanced/Strict.
- `trust_policy`: Offline(default)/Online.
- `language`: optional string.
- `action`: Alert(default)/Kill.
- `applications`: rules containing executable plus publisher and path lists.

`src/config.rs:5-94`. Publisher/path rules are policy evidence, not an OS per-app privacy permission editor. Online trust explicitly permits native trust-network behavior; no GUI rule editor currently exists.

**Baseline alert-delivery discrepancies — Windows capture dispatch superseded by B2 above:**

- Windows tray records access history but does **not dispatch capture-event notifications/chimes**. Its asynchronous toasts report operations/errors and are not gated by notification-enable/pause preferences. Evidence: **WT:1061-1114,1142-1247,1248-1315**; all `notify_async` callers are control/settings/lifecycle paths.
- Windows `watch` computes notification gating at startup; sound is independent of that notification gate. **MAIN:184-208**; `src/watcher/windows.rs:332-375`.
- Unix dispatcher reloads settings during event delivery; sound requires `sound_enabled`, notification enable/unpaused, and **Active START**. Consequently **`watch --sound` alone does not bypass saved `sound_enabled=false`**. `src/watcher/unix.rs:95-125,154-170`.

### 5. Process actions, history/export and maintenance

| Function | Actual implementation / safeguards | Migration |
|---|---|---|
| Manual process termination | **TUI only**; second `k`/click within **3 seconds**, same access key/PID/process instance; Esc cancels. Protected/unverified identities refused. No public `kill PID` command, tray button or native-GUI kill. `src/frontends/tui.rs:562-576,692-807`. | C with identity-bound confirmation—not a raw PID action |
| Automatic termination | `watch --kill-unauthorized` or policy Kill; `--no-kill` overrides. Requires explicit policy **Deny**, Active evidence, attributable stable process, two observations, protected-target safeguards. Suspicious heuristic risk alone is insufficient. `src/watcher/windows.rs:379-439`; `src/watcher/unix.rs:233-280`. | C; do not activate from profile selection |
| Native termination authority | Windows retained process handle/creation-time revalidation, no elevation fallback; Linux pidfd/stable identity/same-user restrictions; macOS retained native process authority. `src/platform/windows.rs:2091-2118`; `src/platform/unix.rs:122-225`. | Retain Rust authority, never make Flutter PID sufficient |
| History retention/clear | `history.jsonl`, **5 MiB rotation threshold**, **three rotations plus current**, no age expiry. `history::path/append/clear`; stable locking coordinates writers/rotation/clear. **HIST:68-132**. | B real data access; clear only designated history |
| Export | **No dedicated export/CSV/history-export command or GUI.** Existing outputs are JSON snapshots, JSONL watch/explicit logs/history, and copied diagnostics. A history viewer/export API would be new functionality, not “migrating an existing native action.” CLI enum **CLI:61-240**; native action enumerations above; **GUIDE:283-309**. | B only if explicitly added; no fake export |
| Windows update | Downloads/verifies matching CLI/tray package, transactionally replaces eligible pair, cooperative owned frontend handling and rollback; protection reservations refuse requested/pending ownership rather than releasing it. Registered MSI/manager installs **refuse binary-only update**, provide official installer guidance; no automatic MSI/UAC upgrade. `src/updater.rs:31-173`; `src/uninstall/windows.rs:644-654`; `src/platform/windows_protection.rs:230-239`; `src/privacy/windows_guard.rs:1721-1742`. | C installation-aware action |
| Unix update | Ordinary-user, installer/receipt-aware path; Linux root camera-helper installation blocks user update/removal until ordered administrator lifecycle. macOS `.app` binary-only update refused; replace whole app. `src/updater/unix.rs:94-163,280-367`. | C; no generic self-update promise |
| Uninstall | Windows verified portable pair: authenticated deferred worker, reservations before frontend shutdown, result receipt—not immediate completion claim. MSI/Cargo produce owner commands rather than executing them. Unix managed removal validates ownership/reserves controls; app/Homebrew/Cargo/Arch/standalone cases provide appropriate instructions. Settings/history/policy retained; active automatic protection/restoration blocks removal. `src/uninstall/windows.rs:43-109,145-215,532-570`; `src/uninstall.rs:40-87,101-215`. | C with exact-install provenance and truthful result |
| Autostart | `state/enable/disable`; Windows limited per-user task, Run-key fallback, no elevation, respects native-disabled intent; Unix `.desktop` or Aqua LaunchAgent, owned registrations only. Hidden refresh preserves existing intent, not installation-wide enable. `src/autostart.rs:98-156,215-229`; `src/autostart/unix.rs:73-78,214-250,285-303,423-507`. | B; separate from startup of a guard |
| About/theme/diagnostics | Native About and diagnostic copying as matrix above. Current Flutter has Overview/Diagnostics/About and local light/dark selection, **not persisted theme preference**. Its diagnostics are snapshot collector/protection details, not full doctor/process evidence. `desktop/flutter/lib/main.dart:135-178,283-327`. | Keep local in A; B for deeper diagnostics |

### 6. Reuse points for Stage A and later stages

| Existing boundary | Precise reuse recommendation |
|---|---|
| Windows microphone tray worker | `src/frontends/tray/windows_audio.rs:18-93`: bounded request/result queues; `Manual`, `Lock`, `Unlock`; COM references stay on worker apartment. **Manual currently toggles** and invalidates automatic journal after success. For an explicit Flutter desired-state command, reuse this owner/worker path and the existing `set_microphone_mute(bool)` backend; do not infer a toggle from stale SDK mute. |
| Microphone result/actual-state reporting | `MicrophoneProtectionStatus { requested, service_active, mute_state, endpoint_count, hardware_mute_count, corrections, detail }`, `src/platform/windows_protection.rs:38-47`. Preserve operation failure plus subsequent observation; absence of endpoints is not proof of protection. |
| Passive Flutter projection | `src/frontends/desktop.rs:261-310` retains requested/service/actual separation and maps zero microphone endpoints/zero cameras to unknown. Counts currently live in backend status, not dedicated fields in Flutter’s `ProtectionStatus` (`:74-95`). |
| Windows authorization | Fixed typed endpoints, same user/session/data-scope; peer checks and version checks. `src/windows_control.rs:276-324,353-382,903-976`; `src/platform/windows_protection.rs:289-305`. This is not authority over another installation’s namespace. |
| Camera workers/restoration | **WT:889-952,1563-1593** already separate camera operations and arrival restoration; `privacy::{camera_observation,set_camera_state,toggle_camera,arrived_restore_targets,restore_arrived,restore_legacy_camera}` at `src/privacy.rs:381-423`. Resume is a separate intent-preserving operation, not Allow→Block. |
| Settings/history | `Settings::update` (**SET:65-68,88-147**) and `history::{path,append,clear}` (**HIST:71-113**). No history-reader/export API currently present. |
| Unix actions | **U:2084-2154** routes native actions to existing platform controls; macOS inspection alone has a dedicated inspection worker (**U:1450-1527**). Do not assume Windows worker architecture already exists identically on Unix. |

### 7. Official logo assets and resource identity mismatches

| Surface | Inspected evidence / finding |
|---|---|
| Canonical identity | **`site/assets/mark.svg:1`**: rounded green tile, eye outline containing microphone; official **`site/assets/favicon.png`** is the 96×96 raster, visually inspected. This is not the generic Material eye icon. |
| Windows native | `installer/miccamwatch.rc:1-2` explicitly ties `installer/miccamwatch.ico` to canonical SVG. `build.rs:40-49` embeds it into CLI/tray; **WN:1474-1516** decodes the same asset for the native window. Windows tray’s semantic status icons are separately drawn status glyphs (**WT:2551-2633**), not canonical-logo copies. |
| GTK/native Linux | **GTK:858-878** embeds `site/assets/favicon.png` for window/header. Native program identity is `com.roman-cuisset.miccamwatch` (**GTK:1031-1035**). Installer desktop/icon paths use that identity (`installer/install.sh:445-455,517-523`). |
| AppKit/native macOS | `native/macos_brand.swift:1-55` renders canonical SVG geometry; menu-bar template removes tile and adds separate status badge (**APP:277-310**). Packaging generates ICNS from that renderer, identity **`com.roman-cuisset.miccamwatch`** (`installer/macos/package.py:62-81`); notification helper identity is **`com.roman-cuisset.miccamwatch.helper`** (`build.rs:103-107`; **APP:1277-1281**). |
| Flutter in-window branding — **confirmed mismatch** | `desktop/flutter/lib/main.dart:151-155` uses **`Icons.visibility_outlined`**, not official eye-plus-microphone asset. `desktop/flutter/pubspec.yaml:1-18` declares Material icons but no branded asset. |
| Flutter macOS app icon — **confirmed mismatch** | Visually inspected `desktop/flutter/macos/Runner/Assets.xcassets/AppIcon.appiconset/app_icon_128.png`: **stock blue Flutter logo**. Catalog names/sizes are referenced by `Contents.json:1-65`. |
| Flutter macOS/Linux identity — **different from native** | macOS product `miccamwatch_desktop`, bundle **`dev.miccamwatch.desktop`** (`desktop/flutter/macos/Runner/Configs/AppInfo.xcconfig:7-14`); Linux uses same binary/application ID (`desktop/flutter/linux/CMakeLists.txt:3-10`). Linux runner sets MicCamWatch title but has no canonical-icon assignment in its source (`linux/runner/my_application.cc:48-55,146-151`). These are real identity differences; changing installed identities/notification ownership is **not** part of this investigation. |
| Flutter Windows resources | `desktop/flutter/windows/runner/Runner.rc:55,92-99` uses its separate `resources/app_icon.ico`; product/company/title say MicCamWatch, internal/executable name `miccamwatch_desktop`. The existing first-slice document reports the official Windows logo (`docs/FLUTTER_0.21.1_FIRST_SLICE.md:12`). **ICO byte equality was not established by this inspection; no confirmed Windows icon mismatch is asserted.** |
| Version identity | Flutter manifest/About advertise **0.21.1 development**, while engine version is separately reported (`desktop/flutter/pubspec.yaml:4`; `lib/main.dart:314-327`). Keep development shell and actual engine version distinct, rather than implying an installed 0.21.1 release. |

**Unsupported claims to avoid:** universal physical microphone/camera cutoff; per-app privacy permission editing; camera TCC grant via administrator inspection; macOS automatic lock protection; Linux hotplug camera blocking; existing full history/export UI; existing tray/native kill/update/uninstall actions; or treating saved Windows tray sound/notification toggles as proof those capture alerts are implemented.

## B1 parity and next gates — 28 October 2026

| Function | Current Flutter state | Next gate |
|---|---|---|
| Native snapshot/collector diagnostics, About, local light/dark theme | Existing shared presentation retained | Full doctor/device detail, keyboard/screen-reader audit and seven-language desktop visual checks |
| Windows explicit SDK microphone request/release | Stage A retained unchanged | Manual release is all-supported-input unmute, not per-input original restoration; no new hardware test in B1 |
| Notifications, sound, future-history preference | B1 native persistence/readback works | Saved flags are not proof of alert delivery; history effect remains separate from preference; no deletion/viewer API invented |
| Assessment profile | B1 saved selection and collector-applied policy work | Managed native configuration disables only profile editing; no permission, allowlist or blocking side effect |
| Pause/resume notifications, autostart | Native only | Typed existing bounds/ownership; isolated registration tests before any host startup write |
| Camera controls, requested guard resume, automatic lock policies | Native only | Explicit platform authorization, effective-state/arrival health and owned restoration; no blind toggles or new guarantees |
| History path/clear, application details, complete diagnostics | Native CLI/interfaces retained | Read APIs before destructive clear; browser/export is new API work, not missing migration wiring |
| Language/theme persistence, update/uninstall | Existing limited/native behavior retained | No imaginary settings fields or generic installation-independent updater |
| Linux x64/ARM64, macOS Intel/Apple Silicon | Native fallback retained; no B1 Flutter runtime qualification | Owned Unix transport/lifecycle, platform restore semantics, full payload/identity/signing gates; ARM physical validation remains absent |
