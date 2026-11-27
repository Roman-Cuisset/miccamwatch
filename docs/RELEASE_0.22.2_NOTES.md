## Flutter opens from the macOS menu and Finder

MicCamWatch now opens its Flutter interface from Finder and the native menu on Intel and Apple Silicon Macs. The earlier menu opened the native panel directly. The Finder launcher could also reject the normal permissions of the system Applications folder. The fix recognizes the root-owned, admin-group writable `/Applications` ancestor for the packaged frontend while preserving the other path, ownership and ACL checks.

Windows retains the v0.22.1 Flutter startup fix and localized tray actions. Linux x64 and ARM64 packages were exercised with their system graphics libraries installed. Flutter opened from the native tray menu on both architectures.

## Downloads and updates

All five targets now share this stable version. Windows MSI users can upgrade using Windows Installer. Portable Windows, Linux and macOS users should follow the documented whole-package update procedure. Linux needs GTK 3, EGL and OpenGL ES, including `libgles2` on Ubuntu, and PipeWire for observation.

Use `SHA256SUMS` to check downloads. GitHub attestations describe the assembly of the eight previously qualified packages and the SPDX SBOM. The packages came from [build 45957355443](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/45957355443) at `bfb699b2a86f8610d785112c1ffdbfdf06485d7d`.

## Signing and qualification limits

Windows installers and executables are unsigned. macOS apps are ad-hoc sealed, without an Apple Developer ID signature or notarization. Gatekeeper admission after a browser download was outside this qualification. Keep platform security protections enabled.

The targeted tests exercised the Windows MSI upgrade from v0.22.1, macOS Finder and menu routes under the system Applications directory, and Linux native menus using hosted XFCE sessions under Xvfb. They did not requalify physical capture devices, macOS sleep and notification permissions, full screen-reader coverage, physical display removal, Linux GPU acceleration or Wayland.
