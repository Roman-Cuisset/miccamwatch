# miccamwatch

`mcw` is a lightweight Windows command-line monitor that shows which applications are using the microphone or camera.

## Current capabilities

- Enumerates active microphone sessions through Windows Core Audio/WASAPI.
- Reports the owning PID, executable path, and capture device for microphone sessions.
- Enumerates physical camera devices through Windows Media Foundation.
- Infers camera activity and its application from Windows privacy activity data.
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

Physical cameras are enumerated through Windows Media Foundation. Windows has no supported public API that enumerates every camera consumer by PID. Camera attribution therefore uses Capability Access Manager activity data and is marked `inferred`. It identifies applications that Windows records—including browsers, OBS, Telegram, and the Windows Camera app—but legacy or privacy-bypassing capture paths may remain invisible. A browser can be identified as the consumer, but not its individual tab or website.

## Platform scope

The first release targets Windows 10 and Windows 11. Linux, macOS, and Android are possible future targets after the event model and Windows backend are stable.

## License

MIT
