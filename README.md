# MicCamWatch

**Free, open-source microphone and camera activity monitoring for Windows, Linux, and macOS.**

[![Official Website](https://img.shields.io/badge/website-miccamwatch.pages.dev-126b50)](https://miccamwatch.pages.dev/)
[![Build](https://github.com/Roman-Cuisset/miccamwatch/actions/workflows/ci.yml/badge.svg)](https://github.com/Roman-Cuisset/miccamwatch/actions/workflows/ci.yml)
[![Latest Release](https://img.shields.io/github/v/release/Roman-Cuisset/miccamwatch)](https://github.com/Roman-Cuisset/miccamwatch/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/Roman-Cuisset/miccamwatch/total.svg)](https://github.com/Roman-Cuisset/miccamwatch/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

MicCamWatch (`mcw`) helps you understand which applications are using your
microphone or camera. It reads operating-system metadata, reports available
process identities, and offers a terminal dashboard, native tray/menu-bar
controls, notifications, and scoped privacy actions.

- **Official website:** [miccamwatch.pages.dev](https://miccamwatch.pages.dev/)
- **Downloads:** [GitHub Releases](https://github.com/Roman-Cuisset/miccamwatch/releases)
- **Privacy:** [official Privacy Policy](https://miccamwatch.pages.dev/privacy)
- **Local monitoring:** no application telemetry, background analytics, or automatic cloud reporting.
- **No media recording:** MicCamWatch does not record audio or capture camera frames.

Detection and controls depend on the operating system and available evidence.
An unknown or incomplete observation is **not** an all-clear.

## Overview & Screenshots

| Interactive Terminal Dashboard (`mcw top`) | Native Desktop Tray (`mcw tray`) |
| :---: | :---: |
| ![mcw top terminal dashboard](site/assets/top-windows.png) | ![MicCamWatch system tray](site/assets/tray-windows.png) |
| *Access events, process identities, and keyboard controls* | *Tray status, profiles, and quick controls* |

These are authentic historical Windows **0.17.1** captures, not mock-ups or
screenshots of the 0.21.0 windows. Their
[provenance and limits](site/assets/captures.json) are retained.

[Install](#quick-start) · [Features](#features) · [Platform support](#supported-operating-systems) ·
[Usage](#everyday-usage) · [Safety](#understanding-status-and-controls) · [Documentation](#documentation)

## Release status

| Channel | What is available |
| --- | --- |
| **Public stable: [v0.22.0](https://github.com/Roman-Cuisset/miccamwatch/releases/tag/v0.22.0)** | Complete hybrid packages for Windows x64, Linux x64/ARM64 and macOS Apple Silicon/Intel. |
| **Previous release: [v0.21.0](https://github.com/Roman-Cuisset/miccamwatch/releases/tag/v0.21.0)** | Native Win32, GTK and AppKit consultation windows. |

The consultation window runs in a separate Flutter process. Rust owns
monitoring, controls, the tray and local data. Closing the window leaves the
tray and monitor running. When upgrading from v0.21.0, use the per-user MSI
or replace the complete archive or application bundle for your system.
[Release notes](docs/RELEASE_0.22.0_NOTES.md) ·
[Qualification and source provenance](docs/HYBRID_STAGE5_RELEASE_CANDIDATE.md).

## Features

- **Live observations:** microphone and camera activity, with process/PID attribution when the platform can prove it.
- **Readable interfaces:** a live terminal dashboard, a Flutter consultation window, native tray or menu bar, notifications and optional system sounds.
- **Scoped controls:** supported microphone mute and owned camera controls, with explicit permission and restoration handling.
- **Profiles and preferences:** Balanced, Private, Meeting, and Development; saved settings and opt-in autostart.
- **Diagnostics:** collector health, permissions, incomplete coverage, and requested versus effective protection.
- **Automation:** versioned JSON snapshots, JSONL events, local history, and TOML policy rules.
- **Seven languages:** English, French, German, Spanish, Japanese, Simplified Chinese, and Russian.

The core is Rust with native platform integration, not a web application or
media recorder. Runtime requirements and permissions differ by platform.

## Supported Operating Systems

The following platforms are supported in v0.22.0:

| Platform | Monitoring | Controls and prerequisites |
| --- | --- | --- |
| **Windows 10 / 11, x64** | Native audio sessions and camera-activity evidence; verified process attribution where available. | Capture-endpoint mute; administrator-approved camera device control. |
| **Desktop Linux, x64 / ARM64** | Running PipeWire capture links and authenticated process identity; limited direct V4L2 evidence. | glibc 2.35+, PipeWire, GTK3 and EGL/OpenGL ES for Flutter. USB camera control requires the optional Polkit-authorized helper. |
| **macOS 15+, Apple Silicon / Intel** | CoreAudio input activity; CoreMediaIO camera running-state with unknown camera client/PID. | Writable input mute; manually approved MCW-owned camera restriction profile. |

Linux desktop effects require the appropriate notification service and a
StatusNotifierItem host. macOS notification and profile authorization remain
controlled by the operating system. Monitoring does not require running the
whole application as administrator/root.

**Linux ARM64 qualification is hosted-runner software evidence only.** No physical
ARM board, Raspberry Pi or ARM USB-device compatibility has been validated.
See [platform limits](docs/USAGE.md#observation-and-control-limits).

## Quick Start

### Windows — MSI or portable ZIP

Download the [v0.22.0 MSI](https://github.com/Roman-Cuisset/miccamwatch/releases/download/v0.22.0/miccamwatch-hybrid-windows-x86_64.msi)
for a per-user installation, PATH registration, and a Start Menu entry.
It upgrades an existing v0.21.0 MSI installation. Alternatively, extract the
[complete portable ZIP](https://github.com/Roman-Cuisset/miccamwatch/releases/download/v0.22.0/miccamwatch-hybrid-windows-x86_64.zip)
in a separate user-owned folder.
Open a new terminal after MSI installation; for a ZIP, run the executables from
its extracted directory or configure PATH explicitly.

```powershell
mcw --version
mcw doctor
mcw status --lang en
mcw top
```

The Windows package contains the CLI, the native tray and the Flutter window.
Start Menu activation, `mcw-tray.exe --open` or `mcw tray open` opens the
consultation. Autostart launches the tray in the background.
[MSI recovery](docs/WINDOWS_MSI_MAINTENANCE.md).

### macOS — native application

Choose the matching v0.22.0 DMG.
[Apple Silicon](https://github.com/Roman-Cuisset/miccamwatch/releases/download/v0.22.0/miccamwatch-hybrid-macos-aarch64.dmg) ·
[Intel](https://github.com/Roman-Cuisset/miccamwatch/releases/download/v0.22.0/miccamwatch-hybrid-macos-x86_64.dmg).
Mount it, drag `MicCamWatch.app` to Applications, then open the copied application.
The menu-bar icon provides access to the native consultation and controls.

The DMG does not install the CLI on PATH or enable autostart implicitly.
Bundles are ad-hoc sealed, **not Developer ID signed or notarized**;
[security and signing guidance](docs/SIGNING.md) explains the limits.
Do not bypass Gatekeeper or quarantine protections.

### Linux — managed hybrid bundle

Choose the matching archive and verify it against the release's
[`SHA256SUMS`](https://github.com/Roman-Cuisset/miccamwatch/releases/download/v0.22.0/SHA256SUMS).
[x64](https://github.com/Roman-Cuisset/miccamwatch/releases/download/v0.22.0/miccamwatch-hybrid-linux-x86_64.tar.gz) ·
[ARM64](https://github.com/Roman-Cuisset/miccamwatch/releases/download/v0.22.0/miccamwatch-hybrid-linux-aarch64.tar.gz).

Use the reviewed Python installer from the tagged source to install the complete
bundle under your user account. The legacy one-line Unix installer serves the
previous release. [Installation and rollback](docs/HYBRID_DISTRIBUTION.md#opérations).

Windows EXEs and MSI are **unsigned**. The macOS app has an ad-hoc seal.
Follow platform signing and security rules when installing.

## Everyday usage

| Command | Purpose |
| --- | --- |
| `mcw status` | Show the current observation and health. |
| `mcw status --json` | Produce a versioned machine-readable snapshot. |
| `mcw watch --json` | Stream access events and health changes. |
| `mcw top` | Open the interactive terminal dashboard. |
| `mcw tray` | Run the native tray/menu-bar frontend. |
| `mcw doctor` | Explain monitoring capabilities, prerequisites, and failures. |
| `mcw config settings-path` | Locate saved application preferences. |
| `mcw history path` | Locate local event history. |

Use `mcw --help` and each command's `--help` for the options in your installed
version. For example, select French with `mcw status --lang fr`.

Desktop profiles select policy/preferences; they do **not** grant capture
permissions or automatically mute/block devices. Balanced and Meeting currently
share the same policy. An explicit `--config` takes precedence over saved profiles.

Use `mcw tray open` on Windows, Linux or macOS to open/reopen the owned native
consultation window without creating another resident frontend.
See [interfaces and preferences](docs/USAGE.md#desktop-and-terminal-interfaces)
and the [full command guide](docs/USAGE.md#commands).

## Understanding status and controls

- **Active** is observed activity within the collector's scope, not a universal guarantee of frame flow or attribution.
- **Ready** is a possible capture pipeline/handle, not confirmed recording.
- **Unknown / unavailable / partial** means evidence is missing or incomplete; it must not be read as idle or safe.
- **Requested protection** and **effective control** are separate from observed activity.
- **SDK mute is not capture permission denial.** Exclusive, ASIO, direct-driver, ALSA, or other unsupported paths may bypass the supported scope.

Windows camera changes request administrator approval. Linux USB control needs
separate reviewed root-helper setup and explicit Polkit authorization. macOS
camera profile approval/removal is manual; it does not grant camera TCC access
or identify the camera client.

Before uninstalling or replacing an installation, explicitly restore owned
changes and resolve pending restoration. Closing a UI is **not** release.
Do not delete ownership journals, force-stop protection owners, or recover
quarantined files as a shortcut.

[Control limits and recovery](docs/USAGE.md#observation-and-control-limits) ·
[Safe updates and uninstall](docs/INSTALLATION.md#updates)

## Privacy

Monitoring uses local OS metadata rather than microphone samples or camera frames.
Settings and history remain in application-data/XDG directories.
Local history is enabled by default and rotates by size; it has no automatic
age-based expiration. Manage it explicitly when sharing a computer or diagnostics.

MicCamWatch has no automatic telemetry/crash upload. Explicit installation and
update commands contact release hosts; an explicitly online trust policy can
also perform network checks. Cloudflare and GitHub have their own hosting and
download processing, separate from the local application.

Read the [official Privacy Policy](https://miccamwatch.pages.dev/privacy) and
[local storage documentation](docs/USAGE.md#logs-and-privacy).

## Documentation

| Read this for… | Guide |
| --- | --- |
| Downloads, verification, updates, uninstall, source builds | [Installation](docs/INSTALLATION.md) |
| Commands, profiles, controls, policies, diagnostics | [Usage and safety](docs/USAGE.md) |
| Public packages, source-specific qualification, hardware limits | [Validation](docs/VALIDATION.md) |
| Versioned changes and historical CI/provenance evidence | [Release history](docs/RELEASE_HISTORY.md) |
| Collector contracts, ownership, platform implementation | [Architecture](docs/ARCHITECTURE.md) |
| Authenticode, Defender, provenance, security warnings | [Signing and safe handling](docs/SIGNING.md) |
| Static-site build, languages, Cloudflare configuration | [Website maintenance](docs/WEBSITE.md) |
| Development priorities and handoff records | [Roadmap](docs/ROADMAP.md) |

The [documentation index](docs/README.md) separates user guides, technical
references, current validation, and historical records.

## Contributing and support

[Report a reproducible problem](https://github.com/Roman-Cuisset/miccamwatch/issues)
with your installed version, OS/architecture, relevant `doctor` output, and the
steps that trigger it. Redact usernames, paths, application identities, and
other private information before posting diagnostics.

For development, follow the [platform-specific build instructions](docs/INSTALLATION.md#build-from-source)
and [architecture contracts](docs/ARCHITECTURE.md). Do not treat a successful build
or hosted CI run as proof of hardware enforcement or universal monitoring coverage.
The full release/CI narrative lives in the documentation rather than this overview.

## License

Free software under the [MIT License](LICENSE).
