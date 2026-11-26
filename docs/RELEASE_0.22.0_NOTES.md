# MicCamWatch v0.22.0

MicCamWatch now opens its consultation window in Flutter, in a separate
process. The Rust monitor, terminal tools and native tray or menu bar remain
available. Closing the window ends its Flutter process and leaves monitoring
running. The interface has two themes and seven languages, with improvements
to keyboard navigation, text scaling and window placement.

## Changes since v0.21.0

- The window displays collector health and distinguishes confirmed observations
  from missing or partial data. It preserves local preferences, profiles,
  history and the application's autostart settings across the supported upgrade.
- On macOS, the new **Close application** action requests normal closure
  after confirmation and checks that the identified app exited. macOS can
  refuse forced termination of protected applications. The reported Vorssaint
  case has not been verified on the actual app.
- macOS camera monitoring reports gaps if a known device disappears. An active
  camera may have no attributable application. Notification permission can be
  inspected, and the installed notification helper has a stable bundle path.
- Windows and Linux remember window geometry and keep the next window within
  the available desktop area where the window system permits it.
- The Windows MSI and portable ZIP have separate maintenance flows. Update and
  recovery check the package identity, file hashes and, for MSI, Windows
  Installer registration. Linux managed archives exchange complete bundles.

## Install or upgrade

Choose the complete hybrid package for your operating system and architecture.
Use the checksum for that exact filename. Leave all the Rust and Flutter files
in their supplied layout.

- **Windows x64.** Use the per-user MSI for an existing MSI installation.
  Close the tray normally and handle any protections it owns before replacing
  the installation. The v0.21.0 updater cannot migrate the hybrid bundle.
  For portable use, extract the full ZIP to a new user-owned directory outside
  the MSI path. Use the maintenance guide if Windows Installer reports an
  interruption or leaves an ambiguous registration. A downgrade is not an MSI
  rollback. [MSI recovery](WINDOWS_MSI_MAINTENANCE.md).
- **Linux x64 and ARM64.** Use the matching GNU/Linux archive with glibc
  2.35 or newer and the documented PipeWire and GTK dependencies. The Flutter
  window also requires EGL/OpenGL ES libraries. Ubuntu 22.04 minimal images
  may need `libgles2`. Use the
  managed hybrid installer and retain its rollback slot. Preserve ownership of
  existing package-manager installations. [Installation and rollback](HYBRID_DISTRIBUTION.md#opérations).
- **macOS 15 or later.** Select the Intel or Apple Silicon DMG or archive.
  Quit the old owner and replace the entire application bundle. Keep the
  previous app until the new one starts correctly. [Installation and rollback](HYBRID_DISTRIBUTION.md#opérations).

## Signing, privacy and qualification limits

Windows candidate executables and MSI are prepared without Authenticode. Check
the signature status on the final bytes. The macOS application has an ad-hoc
seal and has no Developer ID signature or notarization. Neither a checksum nor
build provenance grants OS authorization. Respect the permissions and security
rules on your system.

MicCamWatch observes operating-system metadata. It does not record microphone
audio or camera frames. History and preferences stay on the device. An
unidentified app or an unavailable collector does not establish inactivity.

The qualification does not cover physical macOS camera transitions after
sleep, notification banner display or persistent permission, complete screen
reader workflows, physical monitor removal, Linux hardware GPU acceleration
or resource use with physical microphone and camera capture. See the exact
evidence and scope in the [candidate report](HYBRID_STAGE5_RELEASE_CANDIDATE.md).
