# miccamwatch

`mcw` is a lightweight Windows command-line monitor that shows which applications are using the microphone or camera.

## Current capabilities

- Enumerates active microphone sessions through Windows Core Audio/WASAPI.
- Reports the owning PID, executable path, and capture device for microphone sessions.
- Infers camera activity from the Windows Capability Access Manager privacy data.
- Emits start/stop events continuously.
- Supports human-readable and JSON output.
- Runs without administrator privileges.

## Commands

```console
mcw status
mcw status --microphone
mcw status --camera --json
mcw watch
mcw watch --interval 250
mcw devices
```

`status` exits with code `0` when no access is detected, `1` when access is active, and `2` on error.

## Build

Install the stable Rust MSVC toolchain and Visual Studio C++ Build Tools, then run:

```console
cargo build --release
```

The executable is created at `target/release/mcw.exe`.

## Detection guarantees

Microphone attribution uses the documented Windows audio-session API and is marked `confirmed`.

Windows has no supported public API that enumerates every camera consumer by PID. Camera attribution uses privacy activity data and is therefore marked `inferred`. A browser can usually be identified as the consumer, but not its individual tab or website. The current camera backend also cannot identify the physical camera device.

## Platform scope

The first release targets Windows 10 and Windows 11. Linux, macOS, and Android are possible future targets after the event model and Windows backend are stable.
