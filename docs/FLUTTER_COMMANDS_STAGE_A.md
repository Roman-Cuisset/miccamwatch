# Flutter command protocol 2 — stage A contract

Decision: extend the existing inherited private-pipe bridge, not the engine. Only `set_microphone_protection` is enabled in this stage. No camera/settings/history/maintenance mutation is admitted. Native UI/CLI remain. No push/main/tag/release/install changes.

## Wire shape

All existing snapshot fields remain; `protocol_version` becomes **2**. New required snapshot field:

```json
"controls": {
  "session": "32 lowercase hexadecimal characters (empty only before a child is attached)",
  "microphone": {
    "can_enable": false,
    "can_release": false,
    "current_requested": null,
    "generation": 0,
    "reason": "Read-only development connection"
  },
  "command": null
}
```

`command` is null or exactly `{ "id":1, "requested":true, "state":"accepted|succeeded|failed|rejected|uncertain", "code":null, "detail":null }`. Code is null or a bounded string; detail/reason are null or at most1024 UTF8 bytes. It is the last outcome of this window session, retained across coalesced snapshots. An accepted operation is not effective protection. Actual protection fields remain authoritative and separate. Failure may be partial; unknown must not imply restored/unmuted.

UI output keeps `ready` and `timing`, now version2, and adds:

```json
{
  "type":"command", "protocol_version":2,
  "request":{
    "session":"32 lowercase hexadecimal characters", "id":1,
    "action":"set_microphone_protection", "requested":true,
    "expected_requested":false, "expected_generation":0,
    "observed_sequence":1,
    "confirmation":"sdk_mute_all_inputs"
  }
}
```

Strict nested decoding, no extra fields, unknown actions, shell commands, device IDs, paths, control endpoints, user IDs or privilege parameters. IDs1..u32MAX, sequence>0, generation/sequence at most signed64 maximum. The client emits an explicit desired boolean, never a toggle. Confirmation is sent only after the native-language UI dialog explaining SDK mute limitations and the scope of release (all active inputs, not restore-original-state).
Mute requires `sdk_mute_all_inputs`; explicit release requires `release_all_active_inputs`. They are target-specific acknowledgements, not proof against a compromised same-user UI. Malformed numeric bounds are decoding errors; do not fabricate a correlated response for an invalid request ID.

## Security and state machine

- Anonymous inherited pipes with existing handle allowlist, owned child image validation and kill-on-close job. A new system-CSPRNG128-bit session challenge per child is delivered only inside the private snapshot pipe, never argv/log/disk. This verifies the owned connection, not resistance to arbitrary same-user process injection.
- Admit only after ready, for the current live child/session, an actually sent positive snapshot, a monotonic request ID, matching generation/current requested state, enabled Rust capability and exact confirmation. One pending command; queue full/unavailable/stale/read-only/unauthorized requests are explicit rejections, not success. Old sessions/results cannot affect a new window.
- Transport returns an authorized command ticket; parent dispatches only through the existing microphone worker. Worker rechecks generation/current requested status before calling `PlatformMonitor::set_microphone_mute(bool)`, invalidates automatic restore ownership exactly like a manual native action, and performs passive post-readback. Existing native broker/ACL/user/session/scope/native-resource locks are unchanged.
- Success requires readback requested==target and SDK muted/unmuted as appropriate, with a live service for requested mute. Partial/error/unavailable readback is failed/uncertain, never optimistic protection. Explicit release is not restoration of previously muted endpoints.
- Accepted work survives UI closure; never terminate native worker/guard to implement a UI timeout. UI timeout says result uncertain and does not automatically retry. New window sees current status and busy capability while accepted native work remains pending.
- Outcomes are retained in snapshots; only presentation snapshots coalesce. Command admission and worker dispatch are bounded and at-most-once within the live session. Observational freshness must also use observed_at age so command-only publications cannot make old capture observations appear live.

## New module interfaces

`src/frontends/desktop/commands.rs` owns portable command types/state only; no device or OS calls:

- `CommandAction::SetMicrophoneProtection`, strict serde snake_case.
- `UiCommand` fields exactly as nested request above.
- `MicrophoneCapability { can_enable:bool, can_release:bool, current_requested:Option<bool>, generation:u64, reason:Option<String> }`, Default disabled.
- `CommandState { Accepted, Succeeded, Failed, Rejected, Uncertain }`, serde snake_case.
- `CommandResult { id:u64, requested:bool, state:CommandState, code:Option<String>, detail:Option<String> }`.
- `Controls { session:String, microphone:MicrophoneCapability, command:Option<CommandResult> }`, Default disabled.
- `CommandSession::new(session:String)->anyhow::Result<Self>` validates32lowerhex; `controls(&self,capability:MicrophoneCapability)->Controls` clones only small bounded control state.
- `admit(&mut self, request:UiCommand, last_written_sequence:u64, ready:bool, capability:&MicrophoneCapability)->Option<UiCommand>` validates as above, stores accepted or explicit rejection; one pending. No device execution. Repeated/wrong/old IDs never execute; do not reset a pending operation when rejecting another request.
- `finish(&mut self,id:u64,state:CommandState,code:Option<&str>,detail:Option<String>)->bool` only matches the pending ID and final states; clears pending, stores bounded outcome; wrong/old completion ignored. `pending(&self)->bool`.

`desktop/flutter/lib/commands.dart` owns command controller/widget only, no hardware/FFI/file/network calls:

- `MicrophoneCommandClient` extends ChangeNotifier, constructed with `Future<void> Function(Map<String,dynamic>) send` and optional timeout duration.
- `update(Map<String,dynamic> controls, int observedSequence, bool connected)` called by parent on snapshots/connection changes. Keep IDs monotonic across navigation; reset only for a genuinely new session. Ignore unrelated/outdated results. No optimistic actual-state field.
- `Future<void> request(bool requested)` after UI confirmation: records pending, sends envelope including protocol_version2 and exact nested shape. Pending timeout marks uncertain without auto-retry or assuming native work stopped. Read-only/disconnected/capability/pending gates fail safely. Errors are visible.
- `MicrophoneCommandPanel({required MicrophoneCommandClient client, required String locale})` uses Material3/theme colors, compact explicit mute/release buttons, disabled explanation, human confirmation, accepted/pending/success/failure/uncertain feedback. Seven language labels; explain SDK mute != permission denial and release != restore-original-state. No new logo or animation/decorative dependency.

Parent owns all integration in existing files: DTO/version/parser, Windows transport/challenge/tickets, native worker/request/result dispatch, tray and isolated controlled-development observer, main.dart/session validation/freshness, CPU probes/docs/builds/smoke. New-module implementers must not edit existing files, build/lint/test/format mid-flight, or act on hardware.

## Qualification boundary

User explicitly selected **Test local contrôlé**: after read-only diagnosis, close only the old experimental preview, isolate the new process APPDATA/LOCALAPPDATA, verify no active native guard and all input endpoints initially unmuted, then brief mute/release with post-readback. Stop before mutation if initial state is mixed/unknown/guarded or restoration cannot be proven. Never change MSI installation, stable preferences, active protections or host rights. Keep recovery evidence and use native release API, never rewrite old ownership journals. Other-platform migration remains gated after stage A review.
