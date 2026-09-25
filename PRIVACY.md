# Privacy Policy

**MicCamWatch** (`mcw`) is an open-source, offline-first security and privacy monitor for Windows.

## Data Collection and Storage

- **Offline-only**: MicCamWatch never transmits diagnostics, telemetry, crash reports, or user analytics to any external server. It makes no network calls.
- **No Audio or Video Recording**: MicCamWatch never records audio streams, captures webcam frames, or inspects media payloads. It only reads OS-level metadata:
  - Windows Core Audio / WASAPI session information (PID, session state, capture device name).
  - Windows Capability Access Manager registry timestamps (`LastUsedTimeStart`, `LastUsedTimeStop`).
  - Loaded DLL modules associated with capture frameworks (DirectShow, Media Foundation) to correlate capability.
  - Process metadata (PID, executable path, process tree ancestry, Authenticode digital signatures) to identify which application accessed the device.
- **Local Storage Only**: All generated files remain exclusively on your local machine under your Windows user profile:
  - Settings: `%APPDATA%\MicCamWatch\settings.toml`
  - Activity history: `%LOCALAPPDATA%\MicCamWatch\history.jsonl`
  - Camera block state: `%LOCALAPPDATA%\MicCamWatch\blocked-camera-devices.json`
- **Camera Privacy Controls**: The camera block and allow features use Windows Plug and Play APIs (`pnputil`) locally to enable or disable capture devices. Device state modifications require explicit Windows administrator approval (UAC) and operate entirely on the local machine.
- **No Background Surveillance**: The background tray component (`mcw-tray.exe`) operates solely as a local notification area icon and status monitor. It communicates only with local CLI commands via local Windows messages (`WM_COPYDATA` / registered window messages).

## Contact and Source Code

The complete source code is public, auditable, and licensed under the MIT license:
<https://github.com/Roman-Cuisset/miccamwatch>
