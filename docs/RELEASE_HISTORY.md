# Release history and evidence archive

[Documentation](README.md) · [Installation](INSTALLATION.md) · [Usage](USAGE.md) · [Current validation](VALIDATION.md) · [Technical roadmap](ROADMAP.md)

This archive is chronological, oldest first. **Public stable is [v0.20.0](https://github.com/Roman-Cuisset/miccamwatch/releases/tag/v0.20.0).** “Stable/latest”, publication holds, candidate status and local installation observations quoted below describe their historical release, not today's installation or download target. Tags and previously published assets remain immutable.

**0.21.0 is not published.** Its Linux ARM64 packages, redesigned Win32/GTK/AppKit consultation and unified activation belong to [candidate validation](VALIDATION.md#candidate-0210), not the published release history. A successful candidate CI run is not a release or physical-device certification.

## Contents

- [0.14.0 and unshipped 0.15.0](#0140-and-unshipped-0150)
- [0.15.1 prerelease](#0151-prerelease)
- [0.16.0](#0160--native-unix-controls-and-stable-publication)
- [0.16.1](#0161--standalone-macos-updates-and-compact-frontends)
- [0.17.0](#0170--windows-first-protection-baseline)
- [0.17.1](#0171--corrective-tray-and-distribution-release)
- [0.17.2](#0172--camera-attribution-and-partial-coverage)
- [0.18.0](#0180--unix-diagnostics-and-macos-application)
- [0.18.1](#0181--zip-dependency-security-maintenance)
- [0.19.0](#0190--direct-profiles-and-windows-lifecycle-fixes)
- [0.19.1](#0191--windows-cli-hosted-tray-update-correction)
- [0.20.0](#0200--native-consultation-and-scoped-uninstall)
- [Website evidence](#historical-website-evidence)
- [Original README mapping](#original-readme-mapping)

## 0.14.0 and unshipped 0.15.0

The public 0.14.0 Unix packages were monitoring-only; later native controls must not be attributed to them. [Public installer verification 36827199694](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/36827199694) exercised the commit-pinned installer and genuine 0.14.0 packages on Linux x64 and both macOS architectures. The immutable 0.15.0 tag was **unshipped**, with no release/assets.

The Windows Defender incident and exact public 0.14.0 ZIP/EXE hashes remain canonical in [Signing: Defender incident and safe handling](SIGNING.md#defender-incident-and-safe-handling). The quarantined CLI's original bytes/hash were unavailable; a matching surviving tray does not establish the quarantined file's provenance. Neither a false positive nor Microsoft clearance was established. Do not restore quarantine, add exclusions or disable security protections.

## 0.15.1 prerelease

[Published prerelease v0.15.1](https://github.com/Roman-Cuisset/miccamwatch/releases/tag/v0.15.1), source `7d1e3dc17a6171b521a2f53b3cfebe663425ceb0`, shipped three Unix archives, SPDX SBOM and checksum manifest. At that time stable remained 0.14.0 and Windows publication was held; these are historical channel boundaries, not a current Windows hold.

| Evidence | What it established |
| --- | --- |
| [Native release dry run 37299191864](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37299191864) | Ubuntu 22.04/PipeWire 0.3.48 and macOS 15.7.9 ARM/Intel native builds, tests and extracted CLI/TUI/desktop lifecycle; SDK 15.5, Swift 6.1.2 in Swift 5 mode, macOS deployment target 15.0 |
| [Tag publication 37299861471](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37299861471) | Three Unix packages and release metadata, not Windows publication |
| [Public installation/migration 37301377449](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37301377449) | Real HTTPS fresh install and 0.14.0 → 0.15.1 on Linux x64/macOS ARM/Intel × bash/zsh/fish; PATH consent/idempotence, running-target refusal, failed-upgrade preservation and owned uninstall |
| [Windows CI 37299189617](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37299189617) | 99 tests, dependency audit and per-user MSI smoke; source qualification did not lift the publication hold |

The three archives and SBOM matched API digests, named `SHA256SUMS` entries and attestations constrained to release workflow, exact source and `refs/tags/v0.15.1`. The manifest was not separately attested. A managed Linux update retained the prerelease instead of downgrading to stable 0.14.0; unsafe shared-writable ancestor directories were refused, not silently repaired.

On `serveur-asus`, PipeWire 1.0.5 virtual-source capture, authenticated PID, mute/readback/restoration, already-muted preservation, PTY cancellation and labeled START/STOP were observed. This was session-flow evidence, **not physical microphone denial or audibility**. The SSH user had no physical microphone and could not open `/dev/video0`; permissions were unchanged. Native sound API acceptance did not prove audible speakers, and macOS status-item/LaunchAgent tests did not prove notification authorization.

Canonical detail: [Architecture: native 0.15.1 boundaries](ARCHITECTURE.md#native-0151-verification-boundaries). The separately approved local Windows build/replacement, hashes and observed Defender scan interval are in [Signing: approved local v0.15.1 replacement](SIGNING.md#approved-local-windows-v0151-replacement); this was not a public Windows release or a Microsoft verdict.

## 0.16.0 — Native Unix controls and stable publication

[Stable v0.16.0](https://github.com/Roman-Cuisset/miccamwatch/releases/tag/v0.16.0) fixed source `9515fb664ac0c75601ebf6b8321b6954666faec9` and was published on 2026-10-06. Windows publication was explicitly authorized; authorization was not a signing certificate.

- [Dry run 37425122074](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37425122074) passed the revised macOS CoreMediaIO helper and native Unix lifecycle/recovery. Both macOS system and MCW camera inventories were empty; it did not prove macOS 27 hardware parity.
- [Dry run 37429480145](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37429480145), source `8be1f1b78a907fbeaa47c60e3995369974212588`, passed the socket-activated PipeWire correction. It produced an attested candidate bundle, not itself a public release.
- [Main CI 37435307905](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37435307905) and [release/package smoke 37437092551](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37437092551) passed Windows, Linux x64 and macOS ARM/Intel. Seven public assets matched API digests; six payload/SBOM files matched named checksums and exact-tag attestations.
- Current-installer migrations from [0.14.0: 37438076601](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37438076601) and [0.15.1: 37438076504](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37438076504) passed the three Unix targets with bash/zsh/fish. [Earlier public installer run 37422717465](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37422717465) separately exercised 0.14.0 → 0.15.1.
- A genuine Windows 0.15.1 portable pair updated to the exact public 0.16.0 pair in a private prefix; the second call was up-to-date and existing startup values were preserved. EXEs/MSI were `NotSigned`; local Defender observations did not imply universal clearance.

**Reported macOS hardware evidence:** [@repentandliveholy](https://github.com/repentandliveholy) reported Apple Silicon/macOS 27 Telegram CoreMediaIO **0 → 1 → 0**, then FaceTime HD Camera START/STOP with a locally patched release. This is attributed friend-reported evidence, not a maintainer or hosted-CI physical run. The correction was integrated without rewriting 0.15.1. Camera client/PID remained unknown; running-state did not prove frame flow or authorize per-application enforcement.

**Authorized Linux hardware evidence:** one USB UVC webcam (`13d3:5a11`) on kernel `7.0.0-28-generic`, PipeWire 1.0.5, passed genuine Polkit-authorized block/allow. Detaching only its original UVC interfaces interrupted ongoing V4L2 capture, removed nodes, refused new root capture and restored original bindings/capture. Cancellation changed no bindings. A registered `pkttyagent` worked; the host's internal text agent failed independently. This was not universal camera, hotplug, non-USB/CSI or physical microphone certification. Exact archive hashes, original interfaces, authorization and cleanup are canonical in [Authorized Linux physical verification](ARCHITECTURE.md#authorized-linux-physical-verification).

For systemd socket activation, the socket creator was the user manager, not PipeWire. The correction verified UID/process generation without requiring its executable; capture PID/executable checks stayed strict. Independent virtual-source readback proved mute/restoration and retention of an already-muted original, not physical denial. See [socket-activated PipeWire proof](ARCHITECTURE.md#socket-activated-pipewire-mute-verification), [native software boundaries](ARCHITECTURE.md#native-0160-software-verification-boundaries), [public consumers](ARCHITECTURE.md#stable-v0160-publication-and-public-consumers) and [Windows signing evidence](SIGNING.md#published-stable-windows-v0160-verification).

**Legacy boundary:** the immutable Linux 0.15.1 updater embeds the old three-member installer and safely refuses the six-member 0.16.0 archive. Migration uses the reviewed current installer at the same managed prefix, not an unsafe parser bypass. See [Installation: legacy migrations](INSTALLATION.md#legacy-migrations).

## 0.16.1 — Standalone macOS updates and compact frontends

[Stable v0.16.1](https://github.com/Roman-Cuisset/miccamwatch/releases/tag/v0.16.1), source `7b134e3500860bd185ddc3955107b8082d414e78`, added verified in-place standalone macOS updates outside `bin`, preserving permissions/quarantine without inventing receipts, and rejecting modified, running or unsafe targets. Native details and compact menus/TUI improved; control scope did not expand.

[Complete native CI 37487806861](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37487806861), [release-package smoke 37493346742](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37493346742) and [public Unix migration 37495128116](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37495128116) passed. Unix evidence includes 0.16.0 → 0.16.1 with bash/zsh/fish, genuine standalone bootstrap and public HTTPS updater paths. A private genuine Windows 0.16.0 pair also updated to the public pair and then no-op'd.

Seven public assets matched API digests; six payload/SBOM entries matched checksums and exact workflow/tag/source/hosted-runner attestations. [Signing: v0.16.1](SIGNING.md#published-stable-windows-v0161-verification) retains exact Windows package hashes and `NotSigned` results. No new Defender clearance or physical-device support follows from these software checks.

## 0.17.0 — Windows-first protection baseline

[Published v0.17.0](https://github.com/Roman-Cuisset/miccamwatch/releases/tag/v0.17.0), source `d1b8557f827e102df20c33dbac4ec99badba7d1a`, delivered P1-RAM/P1-RESTORE with **software/native qualification, not hardware certification**. Linux/macOS kept one-shot controls; they did not acquire the Windows background owner.

The Windows native broker separated requested intent, effective SDK endpoint mute and observed capture; manual intent survived UI exit until explicit release. Global camera Block established an explicitly UAC-approved temporary owner, including future arrivals, while Allow restored only owned changes. Zero cameras was not an error. Fixed-capability IPC retained SID/session/scope/process-birth authentication and bounded requests; timeout did not prove a command was unapplied and did not authorize replay. Legacy unsigned camera records were not imported as privileged ownership; recovery remained an explicit single-target UAC action.

The complete contract remains canonical in [Architecture: Windows-first protection](ARCHITECTURE.md#candidate-0170-windows-first-protection-contract). Its historical “candidate” heading does not mean 0.17.0 is unpublished. Software mute is not capture denial; exclusive/ASIO/direct-driver paths and invisible same-value external intent remain limitations. Closing a UI is not release; external MSI/manual replacement is outside portable updater reservations.

| Evidence | Result and boundary |
| --- | --- |
| [32f87cd / 38051530685](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38051530685) | Four native targets and resource comparisons passed; actual zero-device Windows owners survived sustained top/tray and UI exit, then retired on explicit release |
| [29db092 / 38055262096](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38055262096) | Four targets passed after a Unix/AppKit stop-readiness correction; real Unix tray stop/exit returned 0; composed Windows ownership proof passed |
| [Exact tagged-source CI 38056837640](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38056837640) and [release 38058344725](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38058344725) | Four native targets; seven public assets and six payload/SBOM attestations verified against exact tag/source/workflow |
| [Public Unix migration 38059262974](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38059262974) | Managed 0.16.1 → 0.17.0 and macOS public-0.16.0 standalone bootstrap; not physical capture, enabled autostart or Gatekeeper approval |

The private Windows public-0.16.1 updater installed the exact 0.17.0 pair, preserved preferences, and then preserved bytes/mtimes on no-op; an unrelated user tray remained alive. Local strict suites, eight native IPC cases and seven-language/width ConPTY checks were recorded separately from hardware evidence. A non-inheriting launch fixture released captured pipes in 0.282 seconds while its child remained alive for five seconds.

**Resource target not met:** the composed runner measured median **42.820 MB with top**, **43.516 MB with tray**, guards-only **27.922 MB**, and sampled whole-scenario peak **56.041 MB**. The 15,000,000-byte soft target was unmet; approximately 30 MB was a reference, not a hard cutoff. Shared RSS and sampling gaps preclude unique-physical-memory or complete-transient-cost claims. Detailed local/baseline tables, failed earlier runs, native cleanup and sampling limits remain in [pre-publication qualification](ARCHITECTURE.md#pre-publication-qualification-at-32f87cd) and [historical native qualification](ARCHITECTURE.md#historical-native-candidate-qualification), rather than copied here.

Physical microphone enforcement/restoration remained unqualified. A Linux virtual fixture leaving an external unmute in place after five seconds was not physical audio evidence; macOS gained no new physical validation. Exact publication/migration/DNS checks remain in [post-publication evidence](ARCHITECTURE.md#post-publication-v0170-evidence) and [Signing: v0.17.0](SIGNING.md#published-stable-windows-v0170-verification).

## 0.17.1 — Corrective tray and distribution release

[Published v0.17.1](https://github.com/Roman-Cuisset/miccamwatch/releases/tag/v0.17.1), source `dd35f7ddc1bb36c079017f235703175c52d77da5`, kept Windows status compact with complete native Status details. A stopped approved camera guard remained a warning, and operation recovery did not erase unrelated failures. Notification API acceptance was not proof of visibility.

| Evidence | Scope |
| --- | --- |
| [Exact native CI 38152020817](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38152020817) | Windows/Linux/macOS ARM/Intel, native tray/details and MSI consumers |
| [Unix installation 38152020898](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38152020898) | Three platforms and actual HTTPS curl installation |
| [Native Arch 38152020811](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38152020811) | makepkg/pacman install/reinstall/pkgrel-upgrade/remove, native SPA C/Rust roundtrip, private PipeWire and PTY, preserved user state |
| [Release 38210407613](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38210407613) | Seven public assets/digests and six constrained attestations |
| [Public Unix migrations 38212488807](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38212488807) and [release-pinned Arch 38212488566](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38212488566) | Actual 0.17.0 → 0.17.1 installation lifecycles, not physical capture or external package publication |

The reviewed DNS installer at that time had SHA-256 `b7348c9a6f37a3d4f89faa2b767fd4880162d90c8f5d7c151185e0823384d1bb`. Its colored terminal installation and owned removal ran in a disposable HOME/prefix. This is a historical digest, not a substitute for today's selected release verification.

Arch's Clang 22+/bindgen 0.69.5 opaque SPA builder problem was addressed by upstream bindgen 0.72.1, not a handwritten ABI. At that time Homebrew qualification was blocked by the ARM runner's group-writable Cellar and Intel's upstream z3 build; AUR publication lacked authorized SSH access. Later recipe tests do not rewrite those results or establish a public tap/AUR listing. Package-manager ownership and the explicit Homebrew refresh-before-cleanup procedure remain in [Installation](INSTALLATION.md#updates).

An explicitly approved workstation UI handoff preserved mute/camera intent: the legacy CLI-hosted tray required a PID-bound cooperative close, then the 0.17.1 companion menu was visibly compact. No physical protection or reproduction of a prior notification failure was claimed. An occluded earlier details screenshot was explicitly **not GUI evidence**. Full candidate, delivered-installer and authorized deployment records remain canonical in [Architecture](ARCHITECTURE.md#corrective-0171-candidate-qualification), [final source evidence](ARCHITECTURE.md#final-source-and-delivered-installer-evidence), [post-publication](ARCHITECTURE.md#post-publication-v0171) and [authorized deployment](ARCHITECTURE.md#authorized-workstation-deployment).

## 0.17.2 — Camera attribution and partial coverage

[Published v0.17.2](https://github.com/Roman-Cuisset/miccamwatch/releases/tag/v0.17.2), source `a58c8b9f3a967589a576879d6d27bfdbc40f9dff`, retained detected accesses under partial collector coverage with a distinct warning badge and full reasons. Native camera report times missing/zero remained **unattributed**; raw PID metadata was not kill authorization and freshness/process-generation checks were not relaxed to guess Zoom.

[Exact pre-tag CI 38361454701](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38361454701) and [native release 38364052525](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38364052525) passed four targets. Seven assets matched API digests; six payload/SBOM files matched named checksums and exact workflow/tag/source attestations. Windows EXEs/MSI were `NotSigned`. A private public-0.17.1 → 0.17.2 Windows updater and same-version no-op passed. [Public Unix migrations 38366922327](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38366922327) passed nine managed **installer** migrations across bash/zsh/fish and three Unix targets; these were not nine cross-version managed CLI updater proofs. [Native Arch 38362707189](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38362707189) qualified the immutable release archive, not AUR/tap publication.

A user-started real Zoom session and native C++ SDK probe reported activity and a matching raw PID but a successful zero report time; attribution correctly remained unknown. Ktalk later had a valid timestamp and verified attribution. After explicit “Maintenant” permission, the workstation pair matched the public ZIP and the actual **319×262** native menu showed Ktalk microphone/camera accesses and an activity icon, with unchanged protection snapshot. This did not certify physical enforcement or automatic unmute prevention.

Two residual observations were retained: Explorer's accessible icon name contradicted active capture, and captured updater output did not reach EOF after restart. The workstation updater exit/stdout was unavailable after timeout; read-only verification of final files/menu/protections did not turn that missing result into success or rerun the update. The mapped-old-CLI cleanup warning was separate. These fed the 0.19 corrections, not a rewrite of 0.17.2. Canonical detail: [coordinated public release/handoff](ARCHITECTURE.md#coordinated-public-0172-and-authorized-workstation-handoff-2026-10-13) and [native Zoom qualification](ARCHITECTURE.md#p0-cam-tray-source-correction-and-native-zoom-qualification-2026-10-13).

The former README's version-pinned installer example remains traceable at [immutable 0.17.2 installer source](https://raw.githubusercontent.com/Roman-Cuisset/miccamwatch/a58c8b9f3a967589a576879d6d27bfdbc40f9dff/installer/install.sh). Its archive table covered Windows x64 ZIP/MSI, Linux x64 tarball and macOS ARM/Intel tarballs; macOS DMGs arrived in 0.18.0. For current pins, verification, standalone bootstrap and Linux helper ordering use [Installation](INSTALLATION.md), not archived commands.

## 0.18.0 — Unix diagnostics and macOS application

[Published v0.18.0](https://github.com/Roman-Cuisset/miccamwatch/releases/tag/v0.18.0), source `a2b19b6226ded2f32debb3147d3a3e9ef99116a4`, added read-only macOS camera-profile inspection, clearer partial/unknown diagnostics, direct macOS profiles, and native Apple Silicon/Intel app/DMG packages. Desktop IPC v2 required the Rust frontend and embedded helper to be built together.

`camera inspect --json` was passive/current-user; explicit administrator inspection elevated only a fixed bounded system-wide query, not the observer. Refusal/cancellation left restriction unknown and never adopted foreign profiles. Administrator authorization was not TCC camera permission; CoreMediaIO still could not identify camera clients or prove frame flow, and Quartz console metadata did not prove screen lock.

The copied app opened a native menu without Terminal, implicit autostart/protection or permission grants. Bundles were ad-hoc sealed, not Developer ID signed/notarized. App updates required whole-app replacement; binary-only `mcw update` refused. Native app lifecycle/keyboard/Cancel/preauthorized-query proofs did not establish human OS approval, physical capture or an inter-version app upgrade that had not occurred.

[Exact CI 38863239649](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38863239649) and [release 38865573731](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38865573731) passed four targets; nine public assets and eight payload/SBOM checksums/attestations were verified. See [Architecture: 0.18.0 contract](ARCHITECTURE.md#0180-scoped-macos-inspection-menu-and-app) and [public 0.18.0](ARCHITECTURE.md#public-v0180) for source, actual app tests, failed prior qualifications and public migrations.

## 0.18.1 — ZIP dependency security maintenance

[Published v0.18.1](https://github.com/Roman-Cuisset/miccamwatch/releases/tag/v0.18.1), source `034d2c1e5cd5a2f06bf40fc69d0b15bed83defcc`, upgraded `self_update` to 1.3.1 and ZIP to 9.0.4 following [RUSTSEC-2026-0367](https://rustsec.org/advisories/RUSTSEC-2026-0367), [0368](https://rustsec.org/advisories/RUSTSEC-2026-0368) and [0369](https://rustsec.org/advisories/RUSTSEC-2026-0369), issued 2026-10-17. Those advisories concerned upstream extraction/Unix permission/symlink handling. MCW selected two named Windows executable entries through single-file extraction; no MCW exploit was demonstrated. The dependency was updated, not ignored; 0.18.0 tags/assets stayed immutable.

[CI 39027055886](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/39027055886), [release 39028452282](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/39028452282), [public Windows migration 39029450035](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/39029450035) and [public Unix migrations 39029450053](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/39029450053) passed their documented paths. Nine public files, eight named sums/attestations and the public SBOM were verified. Canonical dependency/migration boundaries remain in [Architecture: ZIP maintenance](ARCHITECTURE.md#0181-zip-dependency-security-maintenance) and [public v0.18.1](ARCHITECTURE.md#public-v0181).

## 0.19.0 — Direct profiles and Windows lifecycle fixes

[Published v0.19.0](https://github.com/Roman-Cuisset/miccamwatch/releases/tag/v0.19.0), source `ac7a6e18306c7cf3944ec3e681aa204b68474e59`, followed the explicitly nonpublished candidate packages. Windows/Linux gained direct checked Balanced, Private, Meeting and Development profile choices; explicit `--config` remained authoritative. Windows corrected neutral-before-first-snapshot tooltip identity, broker retirement versus admitted commands, non-inheriting tray restart, passive lock-state startup and preservation of real ownership errors. Camera-protection restart remained a separate UAC action and did not insert Allow or overturn newer intent.

The bounded busy-pipe open followed Microsoft's [Win32 named-pipe admission pattern](https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-client): only connection opening was retried, never possibly applied request bytes. Missing/denied endpoints and separate ownership reservations remained refusals.

- [Native product qualification 39312572930](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/39312572930) passed functional scenarios on four targets but the Windows job **failed strict Explorer popup observation**.
- [Pre-publication packaging 39379984562](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/39379984562), candidate source `535bfc60d768976b98486c327fe7d1cca816980d`, passed packaging on all four targets **without publishing**. Nine files/eight checksums/eight provenance subjects were verified; local ZIP execution was version-only and MSI inspection read-only.
- [Stable release 39383443795](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/39383443795) subsequently published the exact tagged source, with nine public API digests and eight named sums/provenance subjects verified.

The user reported globally positive Windows behavior after the candidate, but did not identify every scenario/version/path. It was not specific popup qualification or Linux/macOS/hardware acceptance. The historical Explorer/UIA limitation remained open for 0.19; later candidate success does not retroactively qualify it.

Hosted native old-tray replacement/mixed-pair repair proved transactions with healthy **inactive protection**, not transfer of active protection. Immutable public 0.18.0/0.18.1 invoking updaters could complete file transactions while still retaining captured EOF: a new file does not repair already-running old restart code. No implicit microphone unmute, camera Allow or forced guard termination was introduced. [ROADMAP](ROADMAP.md) retains the transaction layouts, candidate deliveries, native reports and historical failures.

## 0.19.1 — Windows CLI-hosted tray update correction

[Published v0.19.1](https://github.com/Roman-Cuisset/miccamwatch/releases/tag/v0.19.1), source `3dd70fc`, recognized both exact-installation frontend hosts, `mcw-tray.exe` and `mcw.exe tray`, retaining native process identity through cooperative shutdown. Restart preserved raw Windows arguments, working directory, Unicode environment, relative config paths and language overrides; unreadable context or a foreign canonical data namespace refused before stop.

[Five-layout native qualification 39394812502](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/39394812502) exercised the missing CLI-hosted case. [Release 39397946877](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/39397946877) succeeded on its unchanged-source second attempt; nine public assets and eight named checksums/provenance subjects were verified. The first macOS Intel tray-stop EINVAL remained recorded and unresolved **at that release**, not erased by the retry. Its later accepted-stream blocking correction shipped in 0.20.0.

Public Windows [CLI-hosted 39419044223](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/39419044223) and [companion-hosted 39419044530](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/39419044530) repair/no-op proofs passed with exact downloaded pairs, preserved preferences/context, EOF while the successor lived, and retained PID/HWND for no-op. These used the current 0.19.1 invoking client; they did not claim the older invoking updater was retroactively repaired. [Public Unix 39419045377](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/39419045377) passed real HTTPS, three shells, managed 0.19.0 → 0.19.1 migrations and macOS standalone bootstrap/updater. Detailed limits remain in [ROADMAP](ROADMAP.md).

An old frontend already renamed into a backup was not automatically adopted or killed. It needed an explicitly authorized one-time normal exit/start of the matching frontend; **never execute or restore the backup**. Windows EXEs/MSI remained unsigned; macOS apps ad-hoc and unnotarized. No user-installation replacement or device-control change followed merely from publishing this fix.

## 0.20.0 — Native consultation and scoped uninstall

[Public v0.20.0](https://github.com/Roman-Cuisset/miccamwatch/releases/tag/v0.20.0), source `a26522028eafe02591f58c03ceb5c367f2d5d60b`, annotated tag object `547f6185a0579876041d1ae7ebcdd6feccdcb5cf`, was published **2026-10-22 at 16:13:39Z**, release ID `420936561`. [Native release 39807585260](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/39807585260) passed; nine public assets matched size/API digests, eight named checksum entries and eight signed SLSA v1 provenance subjects were verified.

- **macOS consultation:** singleton native window for observations, access details and diagnostics; close kept the tray alive and reopening reused it. Simplified menu, localized About and larger undistorted menu-bar mark retained distinct badges. This is the 0.20 consultation, not the unpublished 0.21 redesign on three operating systems.
- **Scoped uninstall:** installation provenance selected managed Unix/verified portable removal, package-manager delegation or explicit manual instructions for unowned app/standalone bundles. Settings/history/restoration data stayed retained. Windows deferred command acceptance was not final deletion. Foreign ownership, corrupt records, requested protection and incomplete/unknown owned restoration remained refusals. No macOS owned camera receipt meant no recorded restoration obligation, **not proof of globally allowed cameras**.
- **Lifecycle/branding:** macOS refresh preserved disabled intent across native/legacy launchctl forms; Unix accepted sockets explicitly entered blocking mode, correcting BSD listener-flag inheritance rather than increasing deadlines. Windows executables/MSI/shortcuts used the official icon.
- **TLS maintenance:** `rustls-webpki 0.103.16`, `webpki-roots 1.0.10` and `webpki-root-certs 1.0.10` addressed [RUSTSEC-2026-0382](https://rustsec.org/advisories/RUSTSEC-2026-0382), [0381](https://rustsec.org/advisories/RUSTSEC-2026-0381) and [0383](https://rustsec.org/advisories/RUSTSEC-2026-0383). Three Linux D-Bus-stack maintenance warnings remained visible; no advisory was ignored/suppressed.

[Current validation](VALIDATION.md#public-release-0200) separates native release qualification, authorized isolated workstation tests and missing hardware/manual evidence. Windows EXEs/MSI remain unsigned, and macOS bundles ad-hoc sealed without notarization. Checksums and signed build provenance do not replace platform signing, SmartScreen/Gatekeeper decisions, antivirus clearance or user authorization.

## Historical website evidence

These are **0.17-era website observations**, not a statement of today's production analytics configuration or current installer digest. Current build/deployment procedures belong to [Website](WEBSITE.md); the public [privacy policy](https://miccamwatch.pages.dev/privacy) is the current policy reference.

The seven-language website was exercised in real Wrangler Pages desktop/mobile sessions: OS keyboard tabs, direct language links, no-JS content, nearest-path localized 404s, secure headers, clipboard-denial and manual-selection feedback. Successful system clipboard writing was intentionally not exercised. The repaired production `/install.sh` installed/versioned/uninstalled 0.17.2 through real HTTPS in a disposable Linux HOME.

Source `07eb1734dd177a5319969da5645983a6b94097d5` passed [static-site CI 38530338888](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38530338888) and Cloudflare deployment. Every locale, canonical, localized deep-path 404, same-origin command and the exact **51,249-byte installer** was verified. The local direct-upload ZIP `miccamwatch-pages-0.17.2-multilingual.zip` contained **34 files, 490,598 bytes**, SHA-256 `1747056b969ea5ca017895743af6d6b05eeb6f925c391af4eba8b6caa720bf65`; this local historical artifact is not advertised as a current download. A pre-existing `site-dist/` returned Windows EPERM and was preserved without ACL/antivirus bypass; fresh exact-source exports passed.

At that historical verification Cloudflare injected a Web Analytics beacon that restrictive CSP blocked. The recorded remedy was to disable the project setting, **not relax CSP**. Do not carry that observation forward as an assertion that the current site injects analytics. Canonical incident detail remains in [Architecture: static multilingual site and production repair](ARCHITECTURE.md#static-multilingual-site-and-production-repair-2026-10-14).

The earlier alternative [ASUS static endpoint](https://serveur-asus.duckdns.org:9443/mcw/) and [installer URL](https://serveur-asus.duckdns.org:9443/mcw/install.sh) served documentation/reviewed installer, **not a binary mirror**; payload/checksum transport still used GitHub. This archive does not newly attest that endpoint's current contents or availability. TLS checks must not be disabled; downloading/reviewing a script is safer than piping remote bytes into a shell.

## Original README mapping

The immutable [pre-reorganization README at dc40e95](https://github.com/Roman-Cuisset/miccamwatch/blob/dc40e95bc9c1c6a14386eb5e773f01b62b47aa71/README.md) is the source of the removed historical blocks. This map makes their destinations explicit. Canonical references intentionally retain detailed duplicate tables, hashes and fixture boundaries without copying the same evidence ledger into every guide.

| Original heading/block | Archive destination and canonical detail |
| --- | --- |
| macOS camera activity; two 0.16 dry-run paragraphs | [0.16.0](#0160--native-unix-controls-and-stable-publication); [reported macOS evidence](ARCHITECTURE.md#native-0151-verification-boundaries), [physical Linux](ARCHITECTURE.md#authorized-linux-physical-verification), [socket mute](ARCHITECTURE.md#socket-activated-pipewire-mute-verification) |
| v0.17.0 published protection baseline; local/native/resources; publication/migrations | [0.17.0](#0170--windows-first-protection-baseline); [full protection contract](ARCHITECTURE.md#candidate-0170-windows-first-protection-contract), [resource tables](ARCHITECTURE.md#pre-publication-qualification-at-32f87cd), [publication](ARCHITECTURE.md#post-publication-v0170-evidence), [Windows hashes/signing](SIGNING.md#published-stable-windows-v0170-verification) |
| v0.17.1 corrective release; recipes/installer/Arch; authorized handoff | [0.17.1](#0171--corrective-tray-and-distribution-release); [qualification](ARCHITECTURE.md#corrective-0171-candidate-qualification), [publication](ARCHITECTURE.md#post-publication-v0171), [deployment](ARCHITECTURE.md#authorized-workstation-deployment); current manager procedure in [Installation](INSTALLATION.md#updates) |
| v0.17.2 attribution/partial tray; publication and residual observations | [0.17.2](#0172--camera-attribution-and-partial-coverage); [canonical native/handoff evidence](ARCHITECTURE.md#coordinated-public-0172-and-authorized-workstation-handoff-2026-10-13) |
| 0.20.0 consultation/uninstall/TLS/publication | [0.20.0](#0200--native-consultation-and-scoped-uninstall); [current validation](VALIDATION.md#public-release-0200), [ROADMAP](ROADMAP.md) |
| 0.19.1 CLI-hosted tray correction/publication/Intel limitation | [0.19.1](#0191--windows-cli-hosted-tray-update-correction); [ROADMAP](ROADMAP.md) transaction and release journal |
| v0.19.0 profiles/lifecycle; native candidate/package proof and user feedback | [0.19.0](#0190--direct-profiles-and-windows-lifecycle-fixes); [ROADMAP](ROADMAP.md) records failed popup and old-invoker EOF separately |
| v0.18.1 ZIP security maintenance | [0.18.1](#0181--zip-dependency-security-maintenance); [canonical dependency/publication records](ARCHITECTURE.md#0181-zip-dependency-security-maintenance) |
| v0.18.0 diagnostics/application; macOS application (DMG) | [0.18.0](#0180--unix-diagnostics-and-macos-application); [canonical proof boundaries](ARCHITECTURE.md#0180-scoped-macos-inspection-menu-and-app); current installation/replacement in [Installation](INSTALLATION.md#macos) |
| Install / v0.17.2 coordinated native packages / Unix installer: versioned historical notes | [0.16.0](#0160--native-unix-controls-and-stable-publication), [0.16.1](#0161--standalone-macos-updates-and-compact-frontends), [0.17.2](#0172--camera-attribution-and-partial-coverage); current procedures and legacy bootstraps in [Installation](INSTALLATION.md#legacy-migrations) |
| Install: public 0.16.1, 0.16.0 and prerelease 0.15.1 evidence paragraphs | Corresponding chronological sections above, retaining all distinct release/CI/migration URLs; Windows exact hashes and local Defender history remain in [Signing](SIGNING.md) |
| Static product website: historical runtime/deployment/bundle/EPERM/beacon observations | [Historical website evidence](#historical-website-evidence); [canonical incident record](ARCHITECTURE.md#static-multilingual-site-and-production-repair-2026-10-14); current development/deployment in [Website](WEBSITE.md) |
| Platform scope: 0.15.1 dry run/Windows CI and real-host virtual-flow paragraphs | [0.15.1](#0151-prerelease); [canonical native boundaries](ARCHITECTURE.md#native-0151-verification-boundaries); current [hardware/manual limits](VALIDATION.md#hardware-and-manual-limits) |
| Later GUI-branch-only 0.21.0 ARM64/privacy-center development paragraphs (not in dc40e95 README) | [Candidate validation](VALIDATION.md#candidate-0210), not a published release; [immutable development README](https://github.com/Roman-Cuisset/miccamwatch/blob/cafd98daf7541f317b44aee98edb3b6efa3af561/README.md#L355-L374) |

Durable safety rules from these historical sections are also retained as current guidance in [Installation](INSTALLATION.md) and [Usage](USAGE.md): release/restore before replacement, preserve package-manager ownership, unknown/partial is not safe, SDK mute is not capture permission, and operating-system authorization/signing boundaries remain intact.
