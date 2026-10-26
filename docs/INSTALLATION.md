# Installation and maintenance

This guide covers the **published stable v0.21.0** packages, including Linux ARM64 and native Win32/GTK3/AppKit consultation windows. See [validation and limits](VALIDATION.md#public-release-0210) and [release history](RELEASE_HISTORY.md) for exact public-package evidence and source-specific qualification; publication does not certify physical capture hardware.

[Documentation index](README.md) · [Usage and safety](USAGE.md) · [Signing and safe incident handling](SIGNING.md)

## Choose the right package

Use the [official website](https://miccamwatch.pages.dev/#installer), [stable v0.21.0 release](https://github.com/Roman-Cuisset/miccamwatch/releases/tag/v0.21.0), or [latest stable release](https://github.com/Roman-Cuisset/miccamwatch/releases/latest). All links in this table pin the published 0.21.0 assets.

| System | Package | Requirements and intended use |
| --- | --- | --- |
| Windows x64 | [MSI](https://github.com/Roman-Cuisset/miccamwatch/releases/download/v0.21.0/miccamwatch-windows-x86_64.msi) | Windows 10/11; per-user installation, PATH and Start Menu entry |
| Windows x64 | [Portable ZIP](https://github.com/Roman-Cuisset/miccamwatch/releases/download/v0.21.0/miccamwatch-windows-x86_64.zip) | Matching CLI/tray pair in a user-owned directory |
| Linux x64 | [tar.gz](https://github.com/Roman-Cuisset/miccamwatch/releases/download/v0.21.0/miccamwatch-linux-x86_64.tar.gz) | glibc 2.35+, user PipeWire service, PipeWire/SPA libraries and `pw-dump`; managed installer recommended |
| Linux ARM64 (`aarch64`) | [tar.gz](https://github.com/Roman-Cuisset/miccamwatch/releases/download/v0.21.0/miccamwatch-linux-aarch64.tar.gz) | Same Linux prerequisites; native hosted-runner qualification only, no physical ARM board/peripheral validation |
| macOS Apple Silicon | [DMG](https://github.com/Roman-Cuisset/miccamwatch/releases/download/v0.21.0/miccamwatch-macos-aarch64.dmg) or [tar.gz](https://github.com/Roman-Cuisset/miccamwatch/releases/download/v0.21.0/miccamwatch-macos-aarch64.tar.gz) | macOS 15+; DMG for the native app, archive for CLI/managed installation |
| macOS Intel | [DMG](https://github.com/Roman-Cuisset/miccamwatch/releases/download/v0.21.0/miccamwatch-macos-x86_64.dmg) or [tar.gz](https://github.com/Roman-Cuisset/miccamwatch/releases/download/v0.21.0/miccamwatch-macos-x86_64.tar.gz) | macOS 15+; Aqua session for the menu bar |

Linux consultation needs a graphical session and GTK3 runtime; headless CLI use does not load GTK3. Linux ARM64 publication is not Raspberry Pi/SBC or USB/CSI compatibility certification. Unsupported architectures are rejected, not silently substituted. WSL, headless hosts and virtual runners do not guarantee a graphical session or physical capture hardware. Installing software never proves that a device control works on your hardware.

Repository recipes under [`distribution/`](../distribution/) are not evidence of publication to winget, Homebrew/core, a tap or the AUR. If a package manager owns your installation, retain that ownership: do not overwrite its files with the Unix installer or `mcw update`.

## Verify downloads

Download [SHA256SUMS](https://github.com/Roman-Cuisset/miccamwatch/releases/download/v0.21.0/SHA256SUMS) from the **same release** as the selected package. Compare the exact filename's digest before opening or extracting it:

```powershell
# Windows PowerShell: compare Hash with the matching SHA256SUMS entry.
Get-FileHash .\miccamwatch-windows-x86_64.msi -Algorithm SHA256
```

```sh
# Linux, in the directory containing the selected archive and SHA256SUMS:
sha256sum --check --ignore-missing SHA256SUMS

# macOS: compare the result with the exact named entry in SHA256SUMS.
shasum -a 256 miccamwatch-macos-aarch64.dmg
```

Substitute your chosen ZIP, DMG or archive filename. Do not accept a checksum for another architecture or version. Release assets also include an [SPDX SBOM](https://github.com/Roman-Cuisset/miccamwatch/releases/download/v0.21.0/miccamwatch-sbom.spdx.json); build-provenance evidence and its qualification boundaries are in [Validation](VALIDATION.md#public-release-0210).

**The installer script is not a release-manifest payload.** `SHA256SUMS` checks release archives, installers, DMGs and the SBOM, not the downloaded `mcw-install.sh`. The Unix installer below checks the selected archive's exact named entry itself. Review the script separately, preferably from the immutable published source URL shown below. Checksums/API digests share a release channel; neither they nor build attestations are an independent publisher signature, antivirus verdict or permission grant.

For 0.21.0, **Windows EXE/MSI files are unsigned**; macOS archive CLI binaries are also **unsigned**. DMG app packaging logs report **ad-hoc seals only, not Developer ID signing or notarization**. Windows signature inspection and exact public-byte evidence are documented in [SIGNING.md](SIGNING.md#published-stable-0210-signature-and-provenance-evidence). Respect Gatekeeper, TCC, SIP, quarantine, Defender and organizational controls; do not disable or bypass them to install MCW.

## Windows

### MSI

Open the verified `miccamwatch-windows-x86_64.msi` as the intended ordinary user. It installs under `%LOCALAPPDATA%\Programs\MicCamWatch`, adds that directory to the **user** PATH, and creates a Start Menu shortcut to the windowless `mcw-tray.exe --open`. Open a new terminal to pick up PATH changes:

```powershell
mcw --version
mcw doctor
mcw status --lang en
mcw top
```

Installation does not enable login startup or change microphone/camera controls. Observation runs without administrator privileges; an explicit Windows camera control requests UAC separately. Keep MSI registration intact and use MSI for upgrades/removal.

### Portable ZIP

Extract the **whole** verified ZIP into a private user-owned directory. Keep `mcw.exe`, its matching `mcw-tray.exe` and the supplied portable ownership receipt together; do not mix versions. In PowerShell in that directory:

```powershell
.\mcw.exe --version
.\mcw.exe doctor --json
.\mcw.exe top
# Start desktop mode without a persistent terminal window:
.\mcw-tray.exe
```

Portable extraction does not automatically add PATH. Use the exact executable path when more than one MCW installation exists. `mcw.exe tray` is also supported for interactive diagnostics.

The MSI Start Menu shortcut passes `--open` to `mcw-tray.exe`; `mcw-tray.exe --open` or `mcw tray open` explicitly opens/reopens the retained consultation window. Plain `mcw-tray.exe` and opt-in autostart remain background-only. These launchers reuse the owned frontend rather than start another resident application. See [native consultation and activation](USAGE.md#native-consultation-and-activation) and [public package verification](VALIDATION.md#public-release-0210) for behavior and qualification boundaries.

If Defender quarantines a file, a PATH edit will not recover it. Preserve detection name, affected path/version/hash where available and Protection History. Follow [safe incident handling](SIGNING.md#defender-incident-and-safe-handling); do not add exclusions, disable protection, restore quarantined bytes or automatically reinstall a detected executable. A detection is not established as a false positive.

## macOS

### Native application (DMG)

1. Choose the Apple Silicon (`aarch64`) or Intel (`x86_64`) DMG and verify it.
2. Mount it and drag **MicCamWatch.app** onto the Applications shortcut.
3. Open the copied application, not a binary inside the mounted disk image.

No Terminal, Rust, Swift compiler or development script is needed at runtime. Opening the app starts menu-bar monitoring; it does not implicitly enable autostart, protect devices or grant permissions. Gatekeeper approval is subject to the user's and organization's policy; an ad-hoc seal/checksum is not notarization.

The native AppKit consultation window closes without stopping the tray; reopening the app or using `mcw tray open` reuses the existing frontend. The bundled CLI remains available:

```sh
"/Applications/MicCamWatch.app/Contents/MacOS/mcw" --version
"/Applications/MicCamWatch.app/Contents/MacOS/mcw" config settings-path
"/Applications/MicCamWatch.app/Contents/MacOS/mcw" history path
```

### Managed Unix installation (macOS and Linux)

Download and inspect the installer pinned to the [published 0.21.0 source](https://github.com/Roman-Cuisset/miccamwatch/tree/3d74686ace7b04728794765f9eb57c0c0a8bc84c):

```sh
curl --proto '=https' --tlsv1.2 -fsSL \
  https://raw.githubusercontent.com/Roman-Cuisset/miccamwatch/3d74686ace7b04728794765f9eb57c0c0a8bc84c/installer/install.sh \
  -o mcw-install.sh
less mcw-install.sh
sh mcw-install.sh --version v0.21.0 --add-path
```

Run as the ordinary user, **without sudo**. The default prefix is `$HOME/.local`, with executable `$HOME/.local/bin/mcw`. `--add-path` is a supported, explicit request for managed shell integration; use `--no-modify-path` instead to leave shell configuration untouched. They are mutually exclusive. Without either, an interactive terminal is asked (default **no**); noninteractive installation neither waits for input nor silently edits PATH. Bash, zsh (including `ZDOTDIR`) and fish are supported. Open a new terminal afterward: a child installer cannot change its parent's environment.

```sh
# Custom absolute prefix; executable becomes PREFIX/bin/mcw:
sh mcw-install.sh --version v0.21.0 \
  --prefix "$HOME/Applications/MicCamWatch" --no-modify-path
"$HOME/Applications/MicCamWatch/bin/mcw" --version
"$HOME/Applications/MicCamWatch/bin/mcw" doctor --json
```

Omit `--version` to resolve **latest stable** to a concrete release. An explicitly chosen prerelease is not stable latest. The installer selects native macOS CPU architecture, including from a Rosetta-translated shell. It stages and validates the selected archive before replacement; failed download, integrity or executable validation preserves a working managed installation.

The [production website installer](https://miccamwatch.pages.dev/install.sh) is a convenient delivery endpoint, not an immutable script URL. Download and inspect it before execution. A `curl … | sh` pipeline executes remote code and is less reviewable; it is a Unix command, **not a PowerShell installer**. Never disable TLS verification. Earlier alternative endpoints are preserved only as [historical evidence](RELEASE_HISTORY.md#historical-website-evidence), not recommended current installation sources.

Keep `PREFIX/.miccamwatch-install` receipts. Normal installation refuses unmanaged/externally modified, running, symlinked or hardlinked targets and unsafe shared writable ancestors; choose another prefix or deliberately relocate the old installation instead of weakening permissions or fabricating receipts. Installation never enables autostart, changes devices, approves a camera profile or bypasses platform protections.

### Direct CLI archive

After verifying and extracting the matching archive into a private directory:

```sh
./mcw --version
./mcw doctor --json
./mcw watch --json
```

Ctrl+C stops watching. The macOS helper is embedded. Unmanaged Linux archive installs require manual replacement; macOS supports safe owned standalone updates outside `bin` from 0.16.1 onward. A `bin/mcw` layout with missing/damaged receipts must not be reclassified as standalone to evade ownership checks. Prefer managed installation for an owned uninstall path.

## Linux

Use the [managed Unix procedure](#managed-unix-installation-macos-and-linux) with the public x64 archive. Monitoring needs an accessible user PipeWire socket (`XDG_RUNTIME_DIR`, optionally `PIPEWIRE_REMOTE`), `pw-dump` and readable `/proc/<pid>/stat` and `/proc/<pid>/exe` for attributable processes. Detached OpenPGP publisher rules additionally need GnuPG. Do not run monitoring as root to work around session or device permissions.

Desktop mode needs a StatusNotifierItem host, native notification daemon with `notify-send`, and a graphical session. Optional sound needs `canberra-gtk-play`, a sound theme and a backend for the actual sound session. On Ubuntu, [`libcanberra-pulse`](https://launchpad.net/ubuntu/+source/libcanberra) uses PulseAudio or PipeWire's PulseAudio server; the GTK player alone is insufficient. API playback acceptance does not establish audible sound.

### Optional USB camera administrator setup

Ordinary installation places matching, reviewable payloads in `PREFIX/share/miccamwatch/linux-camera`; it does **not** activate root control. Optional USB `uvcvideo` controls require:

- USBFS generation metadata (`USBDEVFS_CONNINFO_EX`, kernel 5.9+; documented Linux baseline 5.15+).
- `/usr/bin/pkexec`, Polkit and a usable authorization agent.
- A matching root-installed helper; administrator setup also needs `/usr/bin/python3`.
- A supported USB camera arrangement; non-USB/CSI cameras are not covered. See [control limits](USAGE.md#linux-controls).

Download the same-version Linux archive and `SHA256SUMS`, verify its digest, and review `install-camera-helper.sh` and `com.roman-cuisset.miccamwatch.camera.policy`. An administrator must independently decide to trust that release and installer. Then run the **separate reviewed installer**, substituting absolute paths and the actual reviewed digest:

```sh
sudo /bin/sh /absolute/path/install-camera-helper.sh \
  --archive /absolute/path/miccamwatch-linux-x86_64.tar.gz \
  --sha256 '<reviewed 64-character archive SHA-256>'
mcw camera status
mcw camera block    # Explicit Polkit approval; supported current USB cameras only.
mcw camera allow    # Restore only exact identities recorded by MCW.
```

The installer makes a private root-owned copy, verifies its digest and protocol/version pairing, and installs `/usr/local/libexec/miccamwatch/mcw-camera-helper` and its fixed Polkit policy. It does not start a service or change devices. Polkit authorizes only that fixed root helper, never `mcw`, a user-prefix helper, curl pipeline or arbitrary shell. Status/startup/polling never prompts. Root restoration records are under `/var/lib/miccamwatch`; the readable status cache cannot authorize changes.

**Required root-helper upgrade/removal order:**

1. Use the **old matching CLI/helper**: explicitly run `mcw camera allow` to restore owned changes and retire eligible vanished-generation records.
2. Run the reviewed administrator script: `sudo /bin/sh /absolute/path/install-camera-helper.sh --uninstall`. Nonempty/malformed restoration evidence or modified/unmanaged files refuse removal.
3. Update/reinstall/remove the ordinary user installation. Any root installation/transaction remnants, even untrusted ones or an apparently `allowed` state, refuse this step.
4. If still wanted, explicitly install the new matching root helper using the verified same-version archive.

Do not perform concurrent administrator reinstallation during this sequence. User updates refresh only the user payload transactionally, never root files. User uninstall never deletes privileged state. Successful explicit root removal deletes only verified empty journal/cache files and retains the permanent operation-lock inode. Never delete a journal to bypass restoration.

## Updates

### Release protection before changing binaries

With the **currently installed matching version**, inspect `mcw mute --status` and `mcw camera status`. Disable opted-in lock actions (`mcw lock-policy disable`), explicitly release microphone protection (`mcw unmute`) and camera intent (`mcw camera allow`) as applicable, and finish pending approval/restoration. Inspect the resulting state; a command timeout does not prove a change was unapplied and is not permission to replay it blindly.

Closing the frontend or killing a guard is **not** release. Requested/active protection, foreign ownership, busy UAC operations, or pending/unknown/corrupt ownership records are refusals, not obstacles to work around. Windows portable updater readiness reservations do **not** protect external MSI, manual or DMG replacement. Updates never implicitly unmute, perform camera Allow, or adopt another installation's controls. Linux root-helper installations must follow the order above first.

| Installation owner | Update procedure |
| --- | --- |
| Windows MSI | Use the verified new MSI; `mcw update` cannot update MSI registration. |
| Verified Windows portable pair | Run its exact `mcw update`; it stages a matching CLI/tray pair and cooperatively stops only that installation's frontend when safe. |
| Managed Unix | After restoration, stop the installed tray/watcher normally, then run its exact `mcw update`, or rerun the reviewed installer with the **same prefix**. |
| macOS standalone outside `bin` | From 0.16.1, stop its tray/watcher and use `./mcw update`; see bootstrap below for older executables. |
| macOS `.app` | Quit after restoration, then replace the **whole app** from the matching verified DMG. Binary-only `mcw update` is refused. |
| Package-manager installation | Upgrade with that manager, not the public installer/updater. |

Managed Unix/macOS standalone `mcw update` follows stable and refuses downgrades. The Windows updater checks the exact ZIP against both its GitHub API digest and exact `SHA256SUMS` entry, validates both executable versions before shutdown, and waits for real frontend exit. No force-kill/elevation frees executable locks. Tray replacement is atomic; the running CLI uses recoverable same-volume renames with a brief pathname gap. Keep backups for inspection when rollback is refused by concurrent changes or renewed protection; never execute or restore a retained backup blindly.

Updates preserve settings/history and existing owned autostart intent; absent, disabled or unrelated registrations are not enabled/adopted. For a replaced macOS app with an **existing owned** registration:

```sh
"/Applications/MicCamWatch.app/Contents/MacOS/mcw" autostart refresh
```

Before moving an app, disable its owned registration from the old bundled CLI; re-enable from the new location only if desired. Refresh is not an enable action.

For a Homebrew-owned installation with existing autostart, preserve the old keg until refresh succeeds:

```sh
HOMEBREW_NO_INSTALL_CLEANUP=1 brew upgrade miccamwatch &&
mcw autostart refresh &&
brew cleanup miccamwatch
```

Stop on upgrade/refresh failure; do not clean up early. Without existing autostart, ordinary `brew upgrade miccamwatch` needs no refresh. This ownership procedure does not claim a publicly published formula. Arch recipes install reviewable bootstrap material under `/usr/share/miccamwatch/linux-camera`, not an active privileged helper/policy; root setup remains separate.

## Uninstall

First use the [restoration procedure](#release-protection-before-changing-binaries). Uninstall is **not** a device-restoration command. It preserves policy, preferences, history and native restoration data. Do not delete ownership evidence, persistent locks or whole application-data directories as a recovery shortcut.

`mcw uninstall`, introduced in 0.20.0, must be run through the exact installed executable as its ordinary owner. It determines installation provenance before removal:

- **Managed Unix:** validates receipts, ownership, inactive controls and owned autostart, stops this installation's tray and removes unchanged owned user payload/PATH entries.
- **Verified Windows portable pair:** cooperatively reserves protection and stops the exact frontend, then delegates deletion to an authenticated worker. Acceptance/exit code 0 is **not proof of completed deletion**: inspect the printed staging directory's `result.json` (`removed` or `refused_or_incomplete`) and `worker.log`. Remove that private recovery directory only after completion/inspection.
- **MSI/package manager:** prints the owning manager's removal procedure and returns a non-success result rather than pretending removal succeeded. Use Windows Settings **Apps → Installed apps → MicCamWatch → Uninstall** for MSI; Homebrew uses `brew uninstall --formula miccamwatch`; Arch uses the actual owning package (`sudo pacman -R -- miccamwatch` or `miccamwatch-bin`); Cargo follows its reported `cargo uninstall --root … miccamwatch` command.
- **macOS app or unowned standalone:** preserves it, prints manual instructions and returns non-success. Disable owned autostart from that exact binary, quit normally, then delete only that whole app (Finder/Trash) or exact standalone file, respecting an administrator/MDM owner.

Foreign owners and incomplete restoration refuse automated removal before frontend shutdown. On macOS, no owned camera receipt means no **recorded MCW restoration obligation**, not proof that cameras are globally allowed. Uninstall does not require a privileged inventory of unrelated management profiles; corrupt records or pending/unknown restoration for an owned receipt still refuse it.

For explicit managed-script removal after restoration and normal frontend exit:

```sh
sh mcw-install.sh --uninstall
# Or, for the original custom prefix:
sh mcw-install.sh --uninstall --prefix "$HOME/Applications/MicCamWatch"
```

This removes only unchanged installer-owned files/managed PATH blocks; independently modified blocks/files remain for manual review. Retained user data is intentional. `history clear` is a separate explicit action and does not clear restoration records or arbitrary logs.

## Legacy migrations

Old executables retain their old updater code; downloading a newer package cannot retroactively repair a running old client's handoff.

- **Windows v0.14.0 tray:** cannot acknowledge the safe shutdown protocol. Finish pending controls, explicitly restore through the matching version, exit normally and retry. Do not force-kill it.
- **Windows pre-0.19.1 CLI-hosted tray:** an already-renamed old process running from a retained backup is not adopted or killed by the corrected matcher. After explicit release, perform one normal Exit and start the matching installed frontend. Never execute/restore the backup. See [historical evidence](RELEASE_HISTORY.md).
- **Windows legacy camera ownership:** old unsigned ownership files are not imported as privileged receipts. `mcw camera allow --restore-legacy INSTANCE_ID` is an explicit single-target recovery with fresh UAC and camera-class validation; it may override an independent disable. Use only after reviewing the exact device identity. Preserve evidence; do not delete `blocked-camera-devices.json` or protected journals.
- **Linux v0.15.1:** its old three-file updater refuses the newer six-file package safely. Download/review the current installer and rerun with the same prefix, pinned to `v0.21.0` or stable latest. This installs matching user camera payloads without root setup or changes to preferences/history; remove any root-helper installation in the required order first.
- **macOS standalone v0.16.0 or older:** the old embedded updater cannot bootstrap the new standalone contract. After restoration and normal tray/watcher exit, download/review the current script and run once **from the directory containing the existing `mcw`**:

```sh
target="$(pwd -P)/mcw"
digest="$(/usr/bin/shasum -a 256 "$target")"
sh mcw-install.sh --version v0.21.0 --no-modify-path \
  --update-portable "$target" --current-sha256 "${digest%% *}"
./mcw --version
./mcw update
```

This selects that exact executable, not another copy on PATH. It needs an owned single-link regular file outside `bin` and safe private ancestors; relocate a shared-writable extraction first. It preserves executable permissions/quarantine, creates no managed receipt, changes no PATH/preferences/history and never enables autostart. Interrupted replacement rolls back only unchanged owned files where safe; concurrent changes/quarantine preserve the target/backup for inspection. Do not disguise a broken managed/package installation as portable.

Old v0.14.0 Unix packages are monitoring-only. Their immutable assets do not acquire newer controls or safety checks. Version-scoped release/migration proof belongs in [Release history](RELEASE_HISTORY.md) and [Validation](VALIDATION.md).

## Build from source

For reproducible stable work, use the [immutable published 0.21.0 source](https://github.com/Roman-Cuisset/miccamwatch/tree/3d74686ace7b04728794765f9eb57c0c0a8bc84c), not an unpublished branch. A build does not install a package, enable autostart, establish signing trust or qualify physical controls.

| Platform | Prerequisites | Build command |
| --- | --- | --- |
| Windows | Stable Rust MSVC toolchain; Visual Studio C++ Build Tools | `cargo build --release --locked --features windows-tray` |
| Linux | Stable Rust; `pkg-config`; PipeWire/SPA development headers; libclang | `cargo build --release --locked --features linux-camera-helper` |
| macOS 15+ | Stable Rust; Xcode Command Line Tools, macOS 15 SDK and Swift compiler | `cargo build --release --locked` |

Windows produces `target/release/mcw.exe` and `mcw-tray.exe`; without `windows-tray` only the CLI is built. Linux's feature also builds the optional camera helper, but privileged setup remains a separate reviewed operation. Ubuntu 22.04's stock PipeWire 0.3.48 headers are supported. macOS embeds a private ad-hoc-sealed helper; no Swift compiler is needed at runtime and ad-hoc integrity is not Developer ID trust/notarization.

The implementation contracts are in [Architecture](ARCHITECTURE.md), especially the [Unix installer boundary](ARCHITECTURE.md#unix-installer-boundary). Source-backed options: [stable CLI](https://github.com/Roman-Cuisset/miccamwatch/blob/3d74686ace7b04728794765f9eb57c0c0a8bc84c/src/frontends/cli.rs) and [stable installer](https://github.com/Roman-Cuisset/miccamwatch/blob/3d74686ace7b04728794765f9eb57c0c0a8bc84c/installer/install.sh).
