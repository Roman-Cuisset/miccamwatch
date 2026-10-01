# Privacy Policy

**MicCamWatch** (`mcw`) is an open-source, offline-first security and privacy monitor for Windows, Linux and macOS. Linux/macOS observation remains experimental and does not yet provide the Windows device controls.

## Data Collection and Storage

- **Offline by default**: Monitoring sends no telemetry, analytics, crash reports or activity history. Explicitly running the Unix curl installer contacts GitHub for the installer source, release location, native archive and checksum; Windows `mcw update` requests release metadata, a ZIP and its checksum from GitHub. These requests do not upload capture observations or local policy. Setting `trust_policy = "online"` allows Windows Authenticode verification to query certificate-revocation services. The default offline trust policy disables revocation checks and online URL retrieval.
- **No Audio or Video Recording**: MicCamWatch never records audio streams, captures webcam frames, or inspects media payloads. It only reads OS-level metadata:
  - Windows Core Audio / WASAPI session information (PID, session state, capture device name).
  - Windows Capability Access Manager registry timestamps (`LastUsedTimeStart`, `LastUsedTimeStop`).
  - Loaded DLL modules associated with capture frameworks (DirectShow, Media Foundation) to correlate capability.
  - Process metadata (PID, executable path, process tree ancestry, Authenticode digital signatures) to identify which application accessed the device.
  - Linux PipeWire graph metadata, authenticated client/process identity, `/proc` metadata and V4L2 device inventory/open-handle observations.
  - macOS CoreAudio process input-activity metadata, AVFoundation device-level camera-use metadata and process identity from `libproc`. The embedded native helper does not start a media capture session.
- **Local Storage**: By default, generated files are under your Windows user profile:
  - Settings: `%APPDATA%\MicCamWatch\settings.toml`
  - Activity history: `%LOCALAPPDATA%\MicCamWatch\history.jsonl` (and rotated files, when enabled)
  - Camera block state: `%LOCALAPPDATA%\MicCamWatch\blocked-camera-devices.json`
  - Optional `mcw watch --log` writes to the path you choose; `--eventlog` also writes entries to the local Windows Event Log.
- **Unix Local Storage and Installation**: Settings and history use the configured XDG/HOME locations. The Unix installer defaults to `$HOME/.local/bin`; it stores local ownership information under the selected installation prefix and modifies only explicitly approved shell PATH entries. Uninstallation preserves policy, settings and activity history. The macOS observation helper is extracted to a private temporary directory and removed on normal exit.
- **Camera Privacy Controls**: The camera block and allow features use Windows Plug and Play APIs (`pnputil`) locally to enable or disable capture devices. Device state modifications require explicit Windows administrator approval (UAC) and operate entirely on the local machine.
- **Local Background Monitoring**: When started, `mcw-tray.exe` observes capture metadata, records local activity history if enabled, and displays notifications in the Windows notification area. Tray lifecycle commands use local Windows window messages; no remote tray service is used.

## Contact and Source Code

The complete source code is public, auditable, and licensed under the MIT license:
<https://github.com/Roman-Cuisset/miccamwatch>
