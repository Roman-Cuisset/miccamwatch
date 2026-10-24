# Usage and safety reference

This guide describes **published stable v0.20.0**, not the unpublished 0.21.0 candidate. Commands are backed by the [stable CLI source](https://github.com/Roman-Cuisset/miccamwatch/blob/a26522028eafe02591f58c03ceb5c367f2d5d60b/src/frontends/cli.rs). Platform-specific commands are labelled; do not copy a macOS-only command onto Windows/Linux.

[Documentation index](README.md) · [Installation, update and removal](INSTALLATION.md) · [Architecture](ARCHITECTURE.md) · [Validation boundaries](VALIDATION.md)

## Commands

Start without changing device controls:

```console
mcw --version
mcw doctor
mcw status
mcw top
```

Use `mcw --help` or `mcw <command> --help` for local command help. `--lang en|fr|de|es|ja|zh|ru` and `--config PATH` are global options and may precede the subcommand. Use the exact installed executable path when several installations exist; Windows portable PowerShell commands use `.\mcw.exe`.

### Observe and diagnose

```console
mcw status --microphone
mcw status --camera --json
mcw status --risk suspicious
mcw status --include-ready
mcw status --lang fr
mcw watch
mcw watch --notify
mcw watch --interval 250
mcw watch --log events.jsonl
mcw watch --eventlog
mcw watch --no-color
mcw watch --sound
mcw watch --json
mcw devices
mcw devices --json
mcw explain 1234
mcw explain 1234 --json
mcw doctor --json
```

| Option | Availability and meaning |
| --- | --- |
| `--microphone` / `--camera` | `status` and `watch`; mutually exclusive scope filters. Without either, both resources are observed. |
| `--risk LEVEL` | `status` and `watch`; minimum `expected`, `unexplained`, `suspicious` or `blocked` risk to report, not enforcement authorization. |
| `--include-ready` | `status` and `watch`; include unconfirmed/low-confidence ready pipelines. It does not prove frame flow. |
| `--json`, `--no-color` | `status`, `watch`, `devices`, `explain`, `doctor`; macOS `camera inspect` also supports them. |
| `--interval MS` | `watch`; default 750 ms, minimum 100 ms. |
| `--notify` | `watch`; request native desktop notifications, subject to desktop services/authorization. |
| `--sound` | `watch`; request a chime on confirmed capture initiation. Sound preference may also enable it. |
| `--log PATH` | `watch`; append event JSONL to the chosen file, separately from rotating application history. |
| `--eventlog` | `watch`; opt in to native system journal delivery. Unix also accepts `mcw tray --eventlog`. |
| `--kill-unauthorized` | `watch`; opt in to termination only under the explicit-policy safeguards below. |
| `--no-kill` | `watch`; disable automatic termination even if policy sets `action = "kill"`; conflicts with `--kill-unauthorized`. |

Ctrl+C stops `watch`. `status` normally excludes ready-only observations. Exit status is **0** for no matching access, **1** for a matching access, **2** for error/unavailable coverage; read collector health as well as accesses. A 0 does not certify that no unobservable capture exists. `explain` returns 1 if the requested PID has no current observation. It is not a historical PID lookup.

### Explicit controls and lifecycle

Choose an action deliberately; this is a reference, **not a sequence to paste into
a terminal**. State-changing actions have platform-specific permission and
restoration requirements.

| Command | Effect |
| --- | --- |
| `mcw mute --status` | Query supported microphone control without starting protection. |
| `mcw mute`, `mcw mute --toggle`, `mcw unmute` | Request supported microphone protection/mute or explicit release. |
| `mcw camera status` | Query camera control and ownership state. |
| `mcw camera block`, `mcw camera allow`, `mcw camera toggle` | Explicitly change supported camera controls; may request approval. |
| `mcw tray`, `mcw tray run` | Start native desktop monitoring. |
| `mcw tray status`, `mcw tray stop` | Inspect or cooperatively stop the owned frontend; stopping is not restoration. |
| `mcw autostart status`, `mcw autostart enable`, `mcw autostart disable` | Inspect or explicitly change owned login startup. |
| `mcw notifications status`, `mcw notifications pause 60`, `mcw notifications resume` | Inspect or change notification pause, not monitoring/protection. |
| `mcw profile`, `mcw profile private` | Query or select a saved privacy profile. |
| `mcw lock-policy disable` | Clear opted-in automatic lock actions. |
| `mcw history path` | Locate local application history. |
| `mcw history clear` | **Delete** current/rotated application history; separate from restoration. |
| `mcw config path`, `mcw config settings-path` | Locate policy and preferences. |
| `mcw --config policy.toml config validate` | Validate a policy without enabling its actions. |
| `mcw update`, `mcw uninstall` | Follow the installation owner's safe maintenance/removal procedure. |

`mute --status` (`-s`) queries without starting protection; `mute --toggle` (`-t`) toggles Windows protection intent or supported Unix input mute. Camera commands affect **all applications within the supported platform scope**, not one selected application's permission. Read the [control limits](#observation-and-control-limits) before changing state. Notification pause accepts **1–10080 minutes**; pausing notifications is not pausing observation, history or protection. `update`/`uninstall` must follow [installation ownership and explicit restoration](INSTALLATION.md#updates), not be used as restoration commands.

Platform-specific examples:

```console
# Windows only; camera UAC cannot reliably be approved after locking:
mcw lock-policy enable --microphone --camera
mcw lock-policy status

# Linux: microphone lock actions only, with a local graphical logind session:
mcw lock-policy enable --microphone

# macOS only: native consultation and read-only profile inspection:
mcw tray open
mcw camera inspect --json
mcw camera inspect --admin
```

macOS has no supported public lock-state signal: enabling lock policy and its status query are refused; `mcw lock-policy disable` remains available to clear settings. Linux camera-on-lock is unsupported and refused; explicitly block before locking. Do not infer supported controls merely because a subcommand name is shared.

## Desktop and terminal interfaces

### Terminal dashboard

`mcw top` is the shared live terminal dashboard. It supports keyboard and left-click controls, compact/narrow layouts, high-contrast colors and case-insensitive action shortcuts:

| Key | Action |
| --- | --- |
| `m` | Toggle supported microphone control |
| `b` | Request camera block in the supported scope |
| `a` | Restore MCW-owned camera changes |
| `k` | Request termination of the selected live process; a second confirmation is required for the same process identity |
| `r` | Refresh |
| `q` / Esc | Quit; Esc cancels a pending termination confirmation first, without quitting |

Selection changes or process-identity changes must not turn a confirmation into permission to terminate another process. Camera approval runs off the UI thread; pending controls prevent unsafe quit and failures remain visible. macOS camera actions still require manual System Settings approval/removal; Linux USB controls require the separate matching root setup and explicit Polkit authorization. A dashboard button does not broaden those scopes.

### Desktop mode

`mcw tray` is a single-instance per-user desktop frontend with `tray status` and `tray stop` lifecycle IPC. Running it interactively on Unix stays in the foreground. A compact status line/icon is a summary; open **Status details/Details** for complete collector, ownership and control errors. Known accesses remain visible during partial coverage with a warning badge; a partial/unavailable collector is not an all-clear.

- **Windows:** native Notification Area icon and popup menu; left/right click opens the menu, double-click requests the current-summary toast. Packages include the windowless `mcw-tray.exe`, used by the Start Menu and autostart; `mcw.exe tray` remains useful for diagnostics. Stable 0.20.0 has status-details consultation, not the candidate's redesigned full application interface.
- **Linux:** native StatusNotifierItem on a desktop with a supporting host. Details use the native status notification; its visible height depends on the notification daemon. A missing tray host is a capability limit, not proof the observer failed or a reason to fabricate an icon.
- **macOS:** AppKit menu bar and, in stable 0.20.0, a singleton native consultation window with observed microphone/camera state, access details and diagnostics. Close the window to leave the tray running; reopen the app or use **macOS-only** `mcw tray open` to reuse it. Protection, notification pause, four profiles and localized About remain in the menu.

The Win32/GTK/AppKit redesign and unified activation in **candidate 0.21.0 are unpublished**, not instructions or promised stable features. See [Validation](VALIDATION.md#candidate-0210).

### Opt-in autostart

`mcw autostart enable` explicitly registers desktop-session startup; no installer enables it implicitly.

- Windows first uses a limited per-user Task Scheduler task, falling back to that user's `Run` key if task creation is denied, without elevation. CLI/companion versions must match. Startup Apps/Task Manager-disabled intent is respected; MCW never rewrites `StartupApproved` to re-enable it.
- Linux uses a native desktop `.desktop` registration; macOS uses a per-user LaunchAgent in an Aqua login session.
- Disable removes only an owned registration. Refresh/update preserves owned enabled, dormant and disabled intent, does not recreate absent intent, and refuses unrelated/externally changed registrations. Unix's maintenance `mcw autostart refresh` is documented for app/package replacement in [Installation](INSTALLATION.md#updates).

Stopping a frontend or disabling autostart does **not** release Windows background protection or restore device/profile changes.

## Profiles and preferences

The desktop menus have four direct, checked privacy profile choices; the CLI queries with `mcw profile` and saves with `mcw profile balanced|private|meeting|development`. This preference survives restart and is **not** an application allowlist, capture permission or device switch.

| Saved privacy profile | Default assessment profile when no policy file overrides it |
| --- | --- |
| Balanced | `balanced` |
| Private | `strict` |
| Meeting | `balanced` |
| Development | `conservative` |

The TOML policy's `profile = "conservative"|"balanced"|"strict"` is the assessment setting; do not confuse it with the four saved privacy profile names. The default policy file is loaded when present. An explicit `--config` takes authority; tray profile choices are disabled and a saved preference is not shown as the effective override. Selecting a profile neither grants permissions nor releases owned protection.

Locate configuration instead of guessing paths:

```console
mcw config path
mcw config settings-path
mcw history path
```

| Data | Windows | Linux and macOS |
| --- | --- | --- |
| Policy/settings | `%APPDATA%\MicCamWatch\policy.toml` and `settings.toml` | Absolute `$XDG_CONFIG_HOME/MicCamWatch`, otherwise `$HOME/.config/MicCamWatch` |
| History/native user data | `%LOCALAPPDATA%\MicCamWatch` | Absolute `$XDG_DATA_HOME/MicCamWatch`, otherwise `$HOME/.local/share/MicCamWatch` |

Persistent preferences include `notifications_enabled` (default true), `sound_enabled` (false), `show_ready` (false), `history_enabled` (true), the saved profile, notification pause expiry, `mute_on_lock`/`block_camera_on_lock` (false), and `restore_on_unlock` (true). If manually editing `settings.toml`, stop the frontend normally first and preserve valid TOML/other settings; prefer the CLI/menu for supported changes. Disabling `history_enabled` controls automatic application history, not an explicitly selected `--log` file or system journal. Do not delete adjacent persistent lock/ownership files while processes may use them.

### Session lock actions

Lock policy is opt-in and applied by the tray, not a replacement for manual protection. Windows/Linux use a three-state lock signal; **unknown never triggers enforcement**. Windows's first known state is a passive startup baseline, not a new lock event. Existing protection and pending restoration are not reset at startup. Unlock restoration only restores changes actually owned by successful automatic protection; a newer manual request takes precedence.

On Windows, UAC cannot reliably be approved on a locked desktop, so camera-on-lock is not a dependable automatic safeguard. Explicitly block before locking when that supported isolation is required. Linux supports microphone actions in a local graphical logind session, not automatic camera authorization; SSH/headless lock state can remain unknown. macOS public console metadata is not lock proof and automatic lock actions are refused.

## Observation and control limits

### Read activity, risk, confidence and health separately

| Field | Values | Interpretation |
| --- | --- | --- |
| `activity` | `active`, `ready` | Live OS activity evidence vs capture-capable pipeline with frame flow unproven |
| `risk` | `expected`, `unexplained`, `suspicious`, `blocked` | Interpretation of evidence; `blocked` means Windows permission is denied, not proof frames bypassed it |
| `confidence` | `high`, `medium`, `low` | Strength of activity evidence, not probability or a threat score |
| `enforcement` | `allow`, `alert`, `deny`, `unknown` | Policy decision, independent of risk |
| Collector health | Including degraded/unavailable | What can actually be observed; an empty access list does not repair an observation gap |

Three further facts are independent: **requested protection intent**, **effective SDK/PnP/profile readback**, and **observed capture**. A requested Block is not proof an owner is running; software mute is not capture-permission denial; no currently discovered camera is not itself an error. Read unknown/partial results rather than converting them to Allowed or inactive. Unsupported actions are disabled/refused, never reported as successful.

| Platform | Microphone evidence | Camera evidence and limits |
| --- | --- | --- |
| Windows 10/11 x64 | Active WASAPI capture session; validated process identity, high confidence | Media Foundation sensor streaming where available; validated PID only when attributable. ConsentStore/CAM intervals are a medium-confidence fallback; loaded capture modules are low-confidence `ready` only. |
| Linux desktop | Running PipeWire input/source nodes **and active source-to-stream link**; authenticated Client PID validated through `/proc` | Analogous PipeWire video evidence; idle/unlinked streams or direct V4L2 open handles are at most `ready`. Direct V4L2 frame flow is not reliably observable. |
| macOS 15+ | CoreAudio `isRunningInput` for a process exposed by the public process API, with validated PID if readable | Passive AVFoundation inventory plus public CoreMediaIO device running-state; medium confidence, **unknown client/PID**, degraded attribution; no frame-flow proof or per-app camera enforcement. |
| WSL/virtual/headless | Physical devices/session not guaranteed | Inventory/hosted CI is not physical capture proof. Android needs a separate application/permission architecture. |

Linux client-supplied process properties alone are not trusted: PID must match server-authenticated `pipewire.sec.pid` and stable executable/start time. Forwarded portal/PulseAudio identity stays unknown when this cannot be established. A failed/restarting PipeWire socket reports unavailable (`status` exits 2), not safe inactivity.

macOS camera discovery does not imply `ready`. Failed/empty discovery or unknown CoreMediaIO/CoreAudio queries create observation gaps; the watcher suppresses invented STOP and rebaselines on recovery. Known active devices remain visible during partial errors. CoreAudio output-device lists do not identify the microphone used by Sound or Telegram; incomplete input-device identity stays unknown. Elevation does not repair these limits.

Windows CAM values may be historical/delayed/unavailable. NonPackaged camera records match full executable paths; inaccessible paths, short-name aliases, junctions or other path representations remain unattributed rather than borrowing another process's signature. Loaded Media Foundation/DirectShow modules and CPU workload alone never establish active capture. Protected/higher-privilege processes can prevent path, command-line, ancestry, module or signature inspection. Output is diagnostic evidence, **not forensic proof that frames were captured**.

### Windows controls

`mcw mute` requests an owned native microphone guard that persists after `top`/tray exit until explicit `mcw unmute`. It is not an implicitly installed login service. Effective SDK endpoint mute and observed activity remain separate: exclusive, ASIO, direct-driver or hardware paths may bypass/differ from software mute. Same-value external mute changes can be invisible; ownership safeguards are not an absolute security boundary. Automatic restoration tracks original endpoint state/generation/ownership and does not undo newer manual intent. Failed restoration stays pending.

`mcw camera block` sets **global intent**, even with zero cameras, and starts a temporary explicitly UAC-approved native owner. It covers supported currently connected cameras and future arrivals until Allow, with a brief arrival-enforcement window. It targets the Camera setup class and legacy Image devices with positive video-camera/capture membership, not scanners merely because they belong to Image. Inventory failure stops the action. Restart requirements, vetoes and partial/unknown readback stay visible, not guaranteed denial.

`mcw camera allow` clears global intent and restores **only MCW-owned changes**, not all disabled devices. If an owned blocked camera is unplugged, restoration can remain pending (`system_managed`), not fully allowed. Reconnect and allow with the matching version if needed; failed or declined authorization retains evidence for retry. Do not delete restoration state. Unsigned legacy records need the explicit reviewed [legacy recovery](INSTALLATION.md#legacy-migrations), not automatic adoption.

If Block remains requested but its guard is unavailable, the tray offers **Restart camera protection (UAC)…**, separately from Allow. Restart revalidates intent and preserves restoration ownership; it does not insert an Allow step. Do not infer from an IPC failure that the guard certainly exited. Status never launches helpers or changes controls. Native ownership is scoped to authenticated user/session/data namespace, not permission to operate on another installation.

### Linux controls

Microphone actions use native PipeWire Props for **session-source mute**, retaining/restoring only MCW-owned changes in the same live server cookie/node serial. They do not revoke direct ALSA access. Linux/macOS controls do not inherit Windows's persistent background owner.

Optional camera control requires the [administrator setup and ordered lifecycle](INSTALLATION.md#optional-usb-camera-administrator-setup). Explicit Polkit actions detach supported USB video-class interfaces from `uvcvideo` using generation-pinned USBFS descriptors and restore recorded bindings. This affects direct V4L2 and PipeWire on those interfaces, not non-USB/CSI cameras or every capture path. No global module unload, device-node permission change, USB reset, audio/storage change or reusable sysfs-path write is performed.

The supported envelope is conservative: exactly one USB configuration, selected interface alternates remain video class `0x0e` with the same control/streaming subclass, and original video siblings are all bound to `uvcvideo`. Mixed bound/unbound siblings are externally managed/refused before intent; originally all-unbound devices remain untouched. Multi-configuration, role-changing, damaged/ambiguous descriptors and configuration mismatches are unsupported. Single-configuration composite cameras can keep unrelated audio/storage interfaces untouched.

Restoration is bound to recorded descriptor/serial/topology/boot/device-generation identity. Reboot, replacement, unplug/replug, denial or partial kernel changes can leave honest incomplete/stale state. Explicit Allow may retire prior-boot or demonstrably vanished/replaced **parent USB generation** records without touching a replacement; changed interface inodes alone do not authorize retirement. Never delete the journal or bind a different camera as recovery. New arrivals are **not automatically blocked** on Linux. One authorized host's supported UVC proof is not universal hardware certification; see [Validation](VALIDATION.md#hardware-and-manual-limits).

### macOS controls and inspection

Microphone controls apply only to **writable CoreAudio input-mute properties**. Missing/read-only properties are unavailable, not muted; there is no universal microphone deny.

`mcw camera block` prepares MCW's owned Restrictions profile and opens manual System Settings approval. **Pending approval is not blocked.** `mcw camera allow` requests removal of that exact owned profile, with manual completion as needed. Profile installation metadata is not physical capture-denial proof; global/MDM restrictions are not adopted as MCW ownership. User mute/profile records survive uninstall so restoration remains possible.

`mcw camera inspect --json` reads current-user profile metadata without changing controls, ownership or permission. If scope is insufficient, explicit `mcw camera inspect --admin` offers confirmation and administrator authorization for only a fixed bounded read-only system-wide `profiles` query, not elevation of the tray/observer. Results carry observation time, completeness and authorization outcome. Cancellation/refusal leaves effective restriction **unknown**, not Allowed or failed control. Administrator access is **not camera TCC permission**, cannot identify CoreMediaIO camera clients or prove frame flow, and does not establish screen-lock state. Do not remove TCC/SIP/quarantine or unrelated profiles/registrations.

## Policy and enforcement

Create a TOML policy, tailoring executable/publisher/path values to your actual application. This Windows-oriented example explicitly opts into online trust but **not termination**:

```toml
language = "fr"             # en | fr | de | es | ja | zh | ru
profile = "strict"          # conservative | balanced | strict
trust_policy = "online"     # offline (default) | online
action = "alert"            # alert (default) | kill

[[applications]]
executable = "zoom.exe"
publishers = ["Zoom Video Communications"]
paths = ['C:\Program Files\Zoom']
```

```console
mcw --config policy.toml config validate
mcw --config policy.toml status
mcw --config policy.toml watch --no-kill
```

Unknown fields, duplicate/empty executable rules and empty publisher/path values are rejected. Application-name heuristics add context only; they are not allowlists and cannot authorize termination. `strict` escalates heuristic unexplained risk to suspicious but does not create a deny decision.

### Trust boundaries

- Default trust policy is **offline**; monitoring never implicitly enables certificate/key retrieval. Explicit online policy permits native trust-network behavior.
- Windows publisher rules match a verified Authenticode signer. Online mode enables revocation checks. Signatures are reverified for each observed capture; path and modification time alone cannot safely cache identity across replacement. Chain acceptance proves neither benign behavior nor an antivirus verdict, and catalog signer identity can be unavailable even if WinVerifyTrust accepts integrity.
- Linux publisher rules pin `openpgp:<FULL_UPPERCASE_PRIMARY_FINGERPRINT>` for the verified detached `<executable>.sig`; key retrieval is permitted only by explicit online configuration. A process name or arbitrary fingerprint string is not verified publisher identity.
- macOS uses native code-signing validation against trusted certificate identity. Ad-hoc integrity is not publisher trust; online mode uses Apple's configured validation without a guarantee of freshly fetched revocation data.
- When publisher and path groups are both configured, **both** must match. Missing identity evidence gives `unknown`, never permission to terminate. Path prefixes end on a directory-component boundary, but these are lexical checks, not protection against symlink/reparse redirection.

### Termination is opt-in and policy-driven

There is no heuristic `unauthorized` assessment. Only an explicit publisher/path policy mismatch produces `enforcement = "deny"`; suspicious risk alone is insufficient. `watch --kill-unauthorized` or `action = "kill"` additionally requires confirmed **active** capture, an attributable PID, two consecutive observations, stable process-instance revalidation and a non-protected target. `--no-kill` overrides policy. Unknown identity/authority is a refusal, not a reason to elevate monitoring.

```console
# Potentially destructive: enable only after validating a deliberate policy.
mcw --config policy.toml watch --kill-unauthorized
```

Linux termination needs stable `pidfd` authority; macOS needs retained task/audit-token authority and refuses protected/inaccessible targets. An unattributed macOS camera observation has unknown enforcement and **cannot trigger termination**. TUI manual termination separately requires confirmation against the same current process identity.

## Logs and privacy

### JSON and event contract

Status JSON is a versioned object containing `schema_version`, `tool_version`, `collectors` and `accesses` (current schema **3**). Watch emits newline-delimited START/UPDATE/STOP access events with `schema_version`, `action`, `observed_at` and flattened access assessment; Unix watch also emits a status document with `collectors` on health changes. Consumers must distinguish document types and **reject unsupported schema versions**, not guess meanings from a sample tool version.

Deduplication uses stable observations/process identities, not PID alone. STOP carries retained **last-observation** evidence: an event can retain `activity = "active"` after capture ended. Human STOP output labels this accordingly. Missing observations during a collector gap do not establish a real stop.

### Local retention

Automatic application history is `history.jsonl` in the reported data directory. Before the next append after the current file reaches **5 MiB**, it rotates to `history.jsonl.1`; up to **three rotated files plus the current file** are retained. This is an approximate bounded retention policy, not an exact total-byte ceiling, because individual event size can cross the threshold. CLI/tray coordinate writes, rotation and clear through persistent locks.

History is enabled by default and has **no automatic age-based expiry**. Disabling
future history does not erase files already retained.

`mcw history clear` deletes current/rotated application history only. It does not delete arbitrary `watch --log` destinations, system journals, policy/settings, backups or native control/restoration records. An explicitly chosen `--log events.jsonl` is an append log, **not** the automatically rotated history; manage its retention separately. Journals have their own operating-system retention:

| Explicit `--eventlog` destination | Platform |
| --- | --- |
| Application Event Log | Windows |
| Native syslog/journal delivery | Linux |
| Unified Logging | macOS |

Logs can contain sensitive local metadata such as executable paths, process identity, ancestry, user/session, devices, signatures and evidence. Inspect/redact before sharing diagnostics. Uninstallation intentionally retains user data and restoration evidence; deleting data is separate from safely restoring owned controls.

### Offline-first, not never-network

MCW records **no audio samples, webcam frames or media payloads**. It reads operating-system capture/process metadata. Monitoring has **no analytics, cloud reporting or outbound product telemetry**. A UI label such as **Telemetry**, “telemetry unavailable” or “diagnostics” means local collector/health information, not a remote analytics service.

Explicit installation/update downloads contact GitHub (or the reviewed installer delivery endpoint), and an explicitly online trust policy permits the native network behavior above. Do not describe those opt-in operations as never networking. Native notifications and journals deliver to local OS services, subject to their configuration. macOS extracts its embedded helper into a private temporary directory and removes it on normal exit; that live runtime helper is not an installer-download leak. See the [repository privacy notes](../PRIVACY.md) and [official Privacy Policy](https://miccamwatch.pages.dev/privacy).

## Troubleshooting

| Symptom | Safe next action |
| --- | --- |
| `mcw` not found after installation | Open a new terminal, check the exact installation path and opted-in PATH settings. If Defender quarantined the binary, PATH is not a fix: preserve evidence and follow [SIGNING.md](SIGNING.md#defender-incident-and-safe-handling). |
| Empty accesses with warning/error | Read `mcw doctor --json`, status collector health and native Details. Unavailable/degraded coverage is not proof of inactivity. Do not elevate or loosen device permissions to manufacture success. |
| Tray says Telemetry unavailable | Treat this as a local collector/IPC diagnostic, not analytics. Read Details and `doctor`; retain the actual error. |
| Requested Windows Block but stopped/error guard | Requested intent is not effective protection. Inspect details; use the explicit **Restart camera protection (UAC)…** action when offered, or restore explicitly. Do not kill the guard or delete receipts. |
| Update/uninstall refuses owner, pending operation or restoration | Use the matching installed version to release protection and finish restoration; follow [maintenance procedures](INSTALLATION.md#updates). Closing the UI first is not a safe workaround. |
| Response timeout during a control | The operation may have applied. Inspect actual status/ownership before taking another explicit action; do not blindly replay the command. |
| Linux PipeWire unavailable or unattributed PID | Check the real user's socket/environment and `pw-dump` availability. Client-supplied/forwarded identities may legitimately remain unknown. Direct V4L2 may bypass observation. |
| Linux USB control setup/version/authorization error | Read matching-helper instructions, verify kernel support and a working Polkit agent. Status never authorizes a block; `pkexec` being installed is not enough. |
| Linux SSH Polkit reports `No session for cookie` | A real registered authorization agent is needed. An authorized host used standard unprivileged `pkttyagent` registered for the CLI process; do not weaken Polkit/PAM/account permissions or restart services as a presumed fix. |
| No notification/chime/icon | Check the actual graphical-session services, authorization, StatusNotifier host and sound backend. Successful API acceptance does not prove visible notification or speaker audibility. Windows toast identity is per-user `MicCamWatch.MicCamWatch`; errors are reported. |
| macOS camera active, client unknown | Expected CoreMediaIO attribution limit, not fixed by administrator/TCC access. Running-state is not frame-flow proof. |
| macOS profile restriction unknown/pending | Read inspect completeness/authorization and complete your explicitly owned profile approval/removal in System Settings. Do not adopt unrelated/MDM profiles or interpret cancelled inspection as Allowed. |
| macOS app update refused | Replace the whole matching app after explicit restoration and normal quit, not `Contents/MacOS/mcw` alone; preserve owned autostart intent. |

For support, record MCW version, OS/architecture, installation owner, relevant `doctor`/status collector diagnostics and exact errors, redacting sensitive paths/identities. Distinguish software/virtual-session evidence from physical microphone/camera behavior. See [Validation](VALIDATION.md) for confirmed evidence and remaining manual/hardware limits rather than treating a successful build or hosted test as hardware certification.

## Languages

Display text and command help support **English (`en`), French (`fr`), German (`de`), Spanish (`es`), Japanese (`ja`), Simplified Chinese (`zh`) and Russian (`ru`)**. Locale detection is native; choose explicitly when needed:

```console
mcw status --lang fr
mcw --lang ja --help
mcw --lang en doctor --json
```

Explicit `--lang` overrides the policy language, then native detection supplies the default. Detailed evidence, stable event codes and machine-readable diagnostics remain in English for consistent tooling. Translation does not change capabilities, trust, authorization or the JSON schema.
