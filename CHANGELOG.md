# Changelog

## v0.22.0 — 2026-11-26

- Added a Flutter consultation window in seven languages and two themes.
  Rust retains the monitor, CLI, tray, privacy controls and local data.
- Improved keyboard navigation, scaling and window placement. Windows and
  Linux restore the previous window geometry within the available desktop.
- Added normal app closure on macOS with confirmation and observed exit.
  Improved camera gap reporting and stabilized the installed notification
  helper's bundle path.
- Added complete hybrid packages for Windows x64, Linux x64 and ARM64, and
  macOS Intel and Apple Silicon. Windows MSI and portable maintenance verify
  the complete bundle. MSI checks Windows Installer registration separately
  from files on disk.
- Updated the Rust dependency lock and reduced redundant Flutter updates.

[Release notes and limits](docs/RELEASE_0.22.0_NOTES.md).

## v0.21.0 — 2026-10-26

Native consultation windows, unified desktop and tray activation, and native
GNU/Linux ARM64 packages. [Full release history](docs/RELEASE_HISTORY.md).
