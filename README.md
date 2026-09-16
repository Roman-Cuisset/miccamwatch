# miccamwatch

`mcw` is a lightweight Windows command-line monitor that shows which applications are using the microphone or camera.

## Current capabilities

- Enumerates active microphone sessions through Windows Core Audio/WASAPI.
- Reports the owning PID, executable path, and capture device for microphone sessions.
- Enumerates physical camera devices through Windows Media Foundation.
- Multi-signal camera forensic analysis: classifies camera activity into `ACTIVE`, `READY`, `SUSPECT`, and `UNAUTHORIZED`.
- Emits start/stop events continuously.
- Supports human-readable and JSON output.
- Updates itself from signed-by-checksum GitHub release assets.
- Runs without administrator privileges.

## Commands

```console
mcw status
mcw status --microphone
mcw status --camera --json
mcw watch
mcw watch --interval 250
mcw devices
mcw update
```

`status` exits with code `0` when no access is detected, `1` when access is active, and `2` on error.

## Install

Download `miccamwatch-windows-x86_64.zip` from the [latest release](https://github.com/Roman-Cuisset/miccamwatch/releases/latest), extract `mcw.exe`, and place it in a directory listed in `PATH`.

Upgrade later with one command:

```console
mcw update
```

The updater downloads the latest Windows release and verifies its SHA-256 checksum before replacing the running executable.

## Build from source

Install the stable Rust MSVC toolchain and Visual Studio C++ Build Tools, then run:

```console
cargo build --release
```

The executable is created at `target/release/mcw.exe`.

## Detection guarantees

Microphone attribution uses the documented Windows audio-session API and is marked `confirmed`.

Physical cameras are enumerated through Windows Media Foundation. Camera attribution combines Windows Capability Access Manager activity data with a forensic capture-pipeline scanner.

Access is classified into four distinct operational states:

| State | Confidence | Meaning |
|---|---|---|
| `ACTIVE` | `confirmed` / `inferred` | Camera access is verified by live system APIs or active Windows privacy tracking. |
| `READY` | `forensic` | Camera capture pipeline is loaded, expected for the application type (video-call apps, browsers without dedicated capture processes). No frame flow is proven. |
| `SUSPECT` | `forensic` | Camera capture pipeline is active without correlated Windows privacy activity and with anomalous signals (e.g. browser `VideoCaptureService` running without an active user session, unknown binary, temporary path). |
| `UNAUTHORIZED` | `forensic` | Camera capture pipeline is active while the Windows privacy permission is explicitly set to `Deny`. |

## Platform scope

The first release targets Windows 10 and Windows 11. Linux, macOS, and Android are possible future targets after the event model and Windows backend are stable.

## License

MIT
