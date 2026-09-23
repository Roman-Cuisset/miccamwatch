# Architecture

MicCamWatch is a library-first Windows application. `src/lib.rs` owns the reusable modules; `src/main.rs` is a thin command dispatcher.

## Layers

- `model`: versioned observations, evidence, risk, enforcement decisions, and diagnostics.
- `collector`: platform-neutral `CaptureScope`, `CaptureCollector`, and capability contracts.
- `platform`: Windows implementation using WASAPI, Media Foundation, ConsentStore, process inspection, Authenticode, session state, and privacy controls.
- `config`, `settings`, `history`: policy, persistent user preferences, and rotating event storage.
- `watcher`, `notify`, `output`, `updater`: application services.
- `frontends/cli`, `frontends/tui`, `frontends/tray`: independent user interfaces consuming the same monitor and model.

The observable activity state, heuristic risk, confidence, and enforcement decision remain independent. A collector must not convert incomplete evidence into an enforcement denial.

## Collector contract

A platform backend implements `CaptureCollector`:

1. `snapshot(CaptureScope)` returns observations plus explicit collector health.
2. `devices()` enumerates physical capture devices when the platform exposes them.
3. `diagnostics()` explains backend availability and degraded behavior.

`CollectorContract` records the evidence classes required from each future backend. These constants are design requirements, not claims that non-Windows collectors already exist.

## Platform boundaries

- **Windows**: implemented and release-gated. WASAPI provides microphone session attribution; Capability Access Manager and capture-module inspection provide camera evidence.
- **Linux**: requires PipeWire session attribution, V4L2 device correlation, and process identity from `/proc`. Privacy-control semantics depend on the desktop portal and are not declared available.
- **macOS**: requires CoreAudio, AVFoundation, and TCC evidence. Process attribution must remain unavailable unless a stable supported API proves it.
- **Android**: requires a separate application and permission architecture. Android does not expose a general third-party per-process capture collector, so the desktop contract must not be simulated.

Unsupported backends must report unavailable capabilities rather than emit synthetic activity or confidence.

## Frontend invariants

CLI, TUI, and tray may format, filter, and initiate explicit user controls. They do not collect evidence or reinterpret enforcement. New frontends consume the public library contract instead of importing Windows internals.
