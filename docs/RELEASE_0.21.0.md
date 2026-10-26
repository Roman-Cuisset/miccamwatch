# MicCamWatch 0.21.0 — DRAFT release notes and publication checklist

**Status: unpublished; package preparation and verification are authorized, publication is not.** Public stable remains [v0.20.0](https://github.com/Roman-Cuisset/miccamwatch/releases/tag/v0.20.0). The user's explicit 2026-10-26 permission covers preparing and verifying the integrated `release/0.21.0-candidate` packages through a Release workflow dispatch on that **non-tag branch**. It does not authorize changes to `main`, tag creation/push, GitHub release publication or website deployment. Dispatching against a tag is outside this permission because it can publish immediately. Neither this document nor preparation permission creates a public release, final tag-run hashes, a signing verdict or a deployment.

**Qualification update, 2026-10-26: READY FOR RELEASE within the recorded software scope.** The last blocking Windows standard-user native/MSI/Cargo gate is closed by actual local guest evidence. Unpublished release-package preparation and actual byte/signature/provenance verification are now authorized; production boundaries remain separately forbidden. No 0.21.0 tag/release/site update is implied. [Exact evidence and limitations](VALIDATION.md#windows-standard-user-qualification--2026-10-26).

[Validation and evidence](VALIDATION.md) · [Installation and ownership](INSTALLATION.md) · [Signing and safe handling](SIGNING.md) · [Release workflow](../.github/workflows/release.yml)

## Unpublished packages verified — 2026-10-26

- **Release [40316172507](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40316172507), attempt 1**, `workflow_dispatch`, source **`ff0f05ceba6506dc4cffd0ca54f117780b54511e`**, ref **`refs/heads/release/0.21.0-candidate`**. Windows and all four native Unix package jobs passed. The reusable Windows job was intentionally skipped (`native-only`); the separate complete [CI 40315147426](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40315147426) passed all five targets on this same source.
- The assembly job named `publish` succeeded, but its **Publish complete GitHub release step was skipped**. No release/tag/site publication was performed. The [Actions bundle artifact 12647996675](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40316172507/artifacts/12647996675) contains exactly ten nonempty files. Its downloaded ZIP SHA-256 **`03cae6483c66d06fdf6ee6983a25ae41fa4b95807e2b7418158b4e6c8fe7bb6e`** matched the Actions API digest.

| Actual unpublished filename | Observed SHA-256 |
| --- | --- |
| `miccamwatch-windows-x86_64.zip` | `a9cfa96ee1c26a300f7609af29e1d4590f6e0abac74cb7fc6d4dcefde58782f1` |
| `miccamwatch-windows-x86_64.msi` | `081083b1bb559c1570bd00d650238de533e82a5311b6c42e39a286cef39f0c66` |
| `miccamwatch-linux-x86_64.tar.gz` | `9ddf8316f92921ac8481f9d102e7ae05e6cc02dad65d898bc364c4575c4ba9fe` |
| `miccamwatch-linux-aarch64.tar.gz` | `993896c825cc1b4057e5275d2eda4ff890b4dc71256b28335de98811e4ad23cf` |
| `miccamwatch-macos-aarch64.tar.gz` | `d162a64ab8fc90b898452f0630cd6a108f4924e81c7d6e0b254d65364c9afb52` |
| `miccamwatch-macos-x86_64.tar.gz` | `a7c61154db9d287bdc43fe8602f1b23794d8012a1a1665b66daffff0cc455e53` |
| `miccamwatch-macos-aarch64.dmg` | `09739e8ea5b1ca55dfb0cda737bc0dd8de335e67a91dd3c51dbaf183aa6e206b` |
| `miccamwatch-macos-x86_64.dmg` | `53fe47117a5891c26ffb6b2459dd643c087d26580270a6ba6d561f0790857de0` |
| `miccamwatch-sbom.spdx.json` | `212375ef5d51497a3bedf5513795f0bdbeeff83222dd05b76375c0ae00cc3e59` |
| `SHA256SUMS` | `7f1459d874914970fdbec5172f74c552f81b374434f9235a98bd52f1c3a51b34` |

All nine unique manifest entries matched the actual payload/SBOM bytes. The last row is an independently computed file hash, **not a self-entry in the manifest or an attested checksum manifest**. These are branch-run hashes, not predicted future tag-run hashes.

**Platform verification actually observed**

- **Windows:** both downloaded ZIP executables and the MSI independently returned **`NotSigned`**, no signer and no timestamp. The portable receipt has format 1/version 0.21.0 and both actual EXE hashes match it. Independent read-only MSI database/cabinet inspection found ProductVersion 0.21.0, the single `MicCamWatch` shortcut with `--open`, and both embedded EXEs **byte-identical to the ZIP pair**. No `msiexec`, packaged executable or tray was run on the personal host; actual extracted CLI/tray/MSI execution belongs to the recorded hosted package job, not the earlier local standard-user guest.
- **Linux:** exact archive membership and both ELF64/little-endian payloads independently checked; machines 62/x86_64 and 183/aarch64. All four payload hashes match this run's native `linux-elf.json`/`readelf` evidence. Build baseline remains glibc 2.35/PipeWire 0.3.48; the observed maximum GLIBC symbol requirement is 2.34, **not a new lower supported baseline**. Both native GTK receipts contain 21 passed seven-language/light/dark/high-contrast fixture cases; ordinary-user/root-helper install/removal fixture exits were read as 0. Physical ARM/desktop/capture flags remain false.
- **macOS:** thin archive Mach-O architectures checked. Actual native package logs cover `hdiutil verify` and nested/app ad-hoc sealing; both copied-DMG app reports are `passed`, version 0.21.0, and **`codesign --verify --deep --strict` exit 0**. This is **ad-hoc sealing only**, not Developer ID, notarization, a trusted publisher/timestamp or Gatekeeper clearance. No macOS mount/codesign command was run on the Windows host; the native receipts and their API-digest-verified proof archives were inspected.
- **SBOM:** actual SPDX 2.3 generated by Syft 1.42.3; 503 packages/2,246 relationships, including `miccamwatch` version 0.21.0. Repository-generated inventory is **not binary-complete per-package dependency certification**.

**Provenance:** all **nine** subjects passed `gh attestation verify` constrained to this repository, `release.yml`, exact signer/source digest above, branch ref, SLSA v1 and denial of self-hosted runners. Verified certificate metadata identifies GitHub OIDC, the expected workflow/ref/source and run attempt 1; transparency-log evidence is retained. These Actions artifacts/attestations are not a GitHub release, and public-repository provenance is not confidential. Six subjects passed initially; three first failed **Sigstore verifier initialization**, then passed one serialized retry with the **same policy**. Original failures are retained; their initialization cause is not established and no trust guard was weakened.

**Durable evidence:** `%LOCALAPPDATA%\MicCamWatchQualification\0.21.0-packages-20261026-ff0f05c` retains the exact bundle/manifest, five API-digest-verified ZIPs (bundle and four native proofs), actual job logs, signature/MSI/native comparisons, per-subject verification outputs including failures/retries and the signed provenance bundle. Later documentation/release-note-body commits are **not** the source of these bytes or attestations.

**READY for the final authorization decision within the documented software scope.** Physical ARM/capture, Windows 10, MSI upgrade/repair and other untested real desktop/DPI/accessibility contexts remain unqualified. The public release remains 0.20.0. Future tag-run packages must be checked again against their actual tag/source; this branch receipt cannot authorize or certify them.


## Draft release notes

### Native desktop interfaces on three operating systems

- Windows gains a native Win32 consultation interface; Linux gains a native GTK3 interface; macOS gains a redesigned AppKit consultation. Overview, Settings and Diagnostics separate current observations from preferences and detailed evidence, with localized About information. The CLI, live terminal dashboard and compact tray/menu-bar remain available.
- Overview distinguishes microphone/camera observation, current accesses, collector health, last observation and requested versus effective protection. Partial or unavailable coverage remains visible; missing process identity is not invented, and a bounded list of displayed accesses is not a claim that no other access exists.
- Settings expose persistent profiles, notifications, sound and local history, plus notification pause and explicit autostart controls. Platform-specific lock/control settings remain subject to their actual capability checks. Balanced, Private, Meeting and Development select policy preferences, not capture permissions; an explicit `--config` remains authoritative. Simply opening the application does not enable autostart or protection.
- Diagnostics remain selectable/copyable and separate from compact headlines. Refresh preserves the retained view's tab, keyboard focus, selection and scroll where implemented, rather than repeatedly resetting the user's reading position. Seven languages and native light/dark/high-contrast scenarios are covered by the recorded platform-specific proofs, not a promise about every desktop/accessibility configuration.

### One owned desktop application, repeatable activation

- `mcw tray open` opens or reopens the native consultation across Windows, Linux and macOS. Explicit desktop launch reuses the authenticated owner/window for the same installation and data namespace; closing the consultation hides it without stopping monitoring. Background/autostart launches stay hidden. Startup contention waits for authenticated readiness rather than spawning a second set of workers or commandeering another installation.
- Windows continues to ship **two executables for one application**: `mcw.exe` for the CLI and `mcw-tray.exe` for console-free desktop launch. The MSI is an installer, not a third resident application. The Start Menu shortcut opens the retained interface; the process, shortcut and notifications share the application identity. Additional protection-owner processes do not represent extra main windows.
- Managed Linux installation adds an owned XDG application launcher/icon, separate from background autostart. Gio launch quoting handles literal percent signs and other special characters without a shell; the consultation's actual `WM_CLASS` matches `StartupWMClass=MicCamWatch`. The existing post-update refresh path adds launcher assets when migrating an owned 0.20 managed installation locally, without enabling autostart or adopting foreign files.
- macOS Finder activation uses the existing ephemeral app launcher and one Rust frontend/AppKit helper pair. Reopening reuses the retained consultation, including activation while the owner is still publishing its control endpoint.

### Linux ARM64 distribution and safeguards

- Adds `miccamwatch-linux-aarch64.tar.gz` for native `aarch64-unknown-linux-gnu`, alongside x86_64. Both Linux packages retain the Ubuntu 22.04 / glibc 2.35 and PipeWire/SPA 0.3.48 build baseline. ARMv7/32-bit Linux and musl are not supported by these packages.
- Ordinary-user and separate root camera-helper installers check **both** packaged executables before probing them: ELF class, byte order, permitted ABI, executable type and architecture must match. ARM64 requires machine 183; x86_64 requires machine 62. Existing checksums, safe-path/permission checks, receipts, rollback, restoration and explicit privilege boundaries remain enforced.
- Native hosted ARM qualification records the actual host/toolchain/header versions, generated bindings, native C SPA roundtrip and both ELF identities, and exercises extracted CLI/PTY, PipeWire, desktop and installer/helper boundaries. This is not a cross-build, QEMU substitute or x86-header ABI assumption.
- There is no historical public pre-0.21 Linux ARM archive, upgrade baseline or 0.16.1 ARM resource baseline. These comparisons stay explicitly unqualified instead of downloading x86_64 or fabricating evidence. Candidate/helper process-tree resource measurements do not establish the 15,000,000-byte soft target globally.

### Important correctness and lifecycle fixes

- Preference changes are applied from current persisted settings, not rolled back by an older queued observation. Notification delivery requires enabled/unpaused notifications; sound additionally requires enabled sound and a confirmed **Active capture start**, not Ready, Update or Stop. Disabling local history affects automatic history separately from explicit log/journal output. Desktop API acceptance is not guaranteed visible delivery or audible sound.
- Linux camera-action preflight checks the fixed helper and `pkexec` prerequisites without elevating or treating readable inventory as permission to control a camera. It does not prove a running Polkit agent or successful authorization. macOS read-only inspection/preflight continues to distinguish unknown restriction/identity from effective protection.
- Unix control sockets are prepared inside private staging with restrictive permissions before exclusive atomic publication; inode-bound cleanup preserves replacement/foreign endpoints. macOS open delivery no longer waits behind slow collector polling. Authentication, fixed command capabilities, bounded IPC and refusal of foreign or unsafe endpoints remain intact.
- Windows `mcw update` refuses binary self-update of an MSI-owned installation **before** network, staging or stopping the frontend, directing the user to Windows Installer instead. It is not an automated MSI upgrade. Portable updating remains a separate owned transaction; updating/uninstalling never implicitly releases protection or erases pending restoration.
- Documentation is reorganized into installation, usage/safety, validation, history and architecture references, retaining historical evidence and distinguishing public 0.20.0 from this unpublished candidate. Historical screenshots are not relabelled as the new interface; no production site/privacy change is implied.

## Candidate platform matrix

| Target | Package/interface and prerequisites | Qualification boundary |
| --- | --- | --- |
| Windows 10/11 x86_64 | Portable ZIP or per-user MSI; Win32 window, Notification Area and CLI/TUI | Integrated native CI passed on `4b90fd6`. Actual Windows 11 interactive standard-user portable, same-MSI application/cleanup and genuine MSI/Cargo removal passed in the explicitly approved local staged aggregate on 2026-10-26; compiled source `30051ce…`, harness through `bebad88…`, unchanged application/build/installer payload code. The earlier hosted uninstall failure remains failed. Windows 10, standard-user MSI upgrade/repair, other DPI/multi-monitor configurations, taskbar grouping and screen readers remain unqualified. |
| Linux x86_64, GNU/glibc 2.35+ | Archive with CLI and camera helper; user PipeWire service for observation; GTK3 and graphical session for consultation; StatusNotifier host for tray | Hosted Xvfb/XFCE/isolated PipeWire is not every Wayland/login session or physical capture proof. GTK is loaded when consultation opens; absent GTK/session produces a GUI prerequisite error without converting headless CLI into a GUI dependency. |
| Linux aarch64, GNU/glibc 2.35+ | Native ARM64 archive; same PipeWire/GTK/session requirements and optional separately authorized USB camera helper | Native hosted ARM is **not physical Raspberry Pi/SBC validation**. No general Raspberry Pi OS/peripheral compatibility, ARMv7, musl or CSI camera-control claim. |
| macOS 15+ aarch64 | Apple Silicon CLI archive or application DMG; Aqua session for native application/menu | Integrated AppKit/Finder lifecycle and cleanup passed on `4b90fd6`; human TCC/profile approval and physical capture/enforcement remain separate. |
| macOS 15+ x86_64 | Intel CLI archive or application DMG; same native/session requirements | Same limits; ad-hoc app sealing is not Developer ID signing, notarization or Gatekeeper clearance. |

**Safety limits apply to all targets.** Requested protection, effective SDK/device/profile readback and observed capture are different facts. Unknown/partial coverage is not safe inactivity. Software microphone mute does not revoke every capture path. macOS camera client/PID and frame flow remain unknown; automatic lock policy is unsupported there. Linux direct ALSA/V4L2 and non-USB/CSI devices are not covered by USB-helper enforcement. Windows camera control requires explicit UAC and owns only its own changes. Close/Exit, update and uninstall are not substitutes for explicit release and completed owned restoration. Never disable Defender, SmartScreen, Gatekeeper, TCC, SIP, quarantine or organizational controls to qualify/install a package.

## Expected complete release: exactly ten uploaded assets

GitHub-generated source ZIP/tar archives are **not** part of this count. This table is the intended complete coordinated release, not a statement that candidate assets/hashes already exist.

| # | Exact uploaded filename | Purpose |
| --- | --- | --- |
| 1 | `miccamwatch-windows-x86_64.zip` | Portable CLI/console-free desktop pair, ownership receipt, README and license |
| 2 | `miccamwatch-windows-x86_64.msi` | Per-user Windows installer |
| 3 | `miccamwatch-linux-x86_64.tar.gz` | GNU/Linux x86_64 distribution |
| 4 | `miccamwatch-linux-aarch64.tar.gz` | GNU/Linux ARM64 distribution |
| 5 | `miccamwatch-macos-aarch64.tar.gz` | Apple Silicon command-line distribution |
| 6 | `miccamwatch-macos-x86_64.tar.gz` | Intel command-line distribution |
| 7 | `miccamwatch-macos-aarch64.dmg` | Apple Silicon application disk image |
| 8 | `miccamwatch-macos-x86_64.dmg` | Intel application disk image |
| 9 | `miccamwatch-sbom.spdx.json` | SPDX JSON SBOM generated from the checked-out repository |
| 10 | `SHA256SUMS` | Unified manifest for the preceding nine files; no self-hash |

The nine `miccamwatch-*` files are SHA-256 manifest entries and provenance subjects. `SHA256SUMS` is uploaded but **not itself attested** by the current workflow; the installer script is not a release-manifest payload. Do not claim a separately signed checksum manifest or a binary-complete per-package SBOM audit from repository SBOM generation alone.

### Retained CI payload digests — not the future release manifest

The exact integrated source `4b90fd6bdfc2aaddf6218a64ce71f6554e06c789` was exercised by [CI 40106706806](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40106706806). The four extracted Linux payloads below were hashed and matched to `linux-elf.json`; both retained macOS DMGs were hashed locally. These are **unpublished CI bytes**, not final release/archive hashes, a generated `SHA256SUMS`, or release provenance. The ordinary CI did not generate the Release workflow's attestations.

| Retained CI payload | Observed SHA-256 |
| --- | --- |
| `native-proof-linux-x86_64/package/mcw` | `7412540ea705b723a72eb71773961e4c29fcc94a5af671611329d449210e2a1f` |
| `native-proof-linux-x86_64/package/mcw-camera-helper` | `9dbd68f7718f9c1e96569cdcc7762ef4fcf014d1d030b82688f6f0e02a1f6f4b` |
| `native-proof-linux-aarch64/package/mcw` | `7ffbf41a860efcd8f052eb8cbaa16167b3e01d7cef0fcc4e080db370a0078af8` |
| `native-proof-linux-aarch64/package/mcw-camera-helper` | `516acda3ca22d9e7d86ee27215151e44ad2cdd88cedc614da7444f08c5a6ea88` |
| `macos-app-aarch64/miccamwatch-macos-aarch64.dmg` | `2e85b75174734c60e225cb2114c2d2bcf6349fef0eaefe30cf04a3ac55493afd` |
| `macos-app-x86_64/miccamwatch-macos-x86_64.dmg` | `4ac88ee16374e976d0c748ea68b3fb06fbcf43da78b046d77254ccee7b8dd274` |

## What the existing release workflow actually gates

The complete [release.yml](../.github/workflows/release.yml) is authoritative for workflow behavior; these notes distinguish its package-preparation path from its tag-only publication path.

1. **Triggers and publication:** tag pushes matching `v*` or `workflow_dispatch` can run the workflow. The historical `refs/tags/v0.14.0` exclusion remains. A dispatch on a **non-tag branch** can build/attest/upload a `verified-release-bundle`, but `Publish complete GitHub release` executes only for `refs/tags/`. Dispatching against a tag is therefore not a safe publication-free dry run. There is no `draft: true` publication setting: a successful authorized tag run can publish immediately.
2. **Windows approval:** the Windows job requires `vars.WINDOWS_RELEASE_APPROVED == 'true'` and uses the `release` environment. The current variable is true; the environment has a one-minute wait timer, no branch restriction and no required reviewer. Those settings allow packaging but provide no independent publication-authorization barrier. The user's new approval is for unpublished preparation/verification only, not a `main` change, tag or release. A tag name must match Cargo's version in the Windows verification step.
3. **Windows signing:** when the configured certificate is available, both EXEs are SHA-256 Authenticode-signed and timestamped, verified with `signtool` and expected certificate/timestamp checks, then embedded in the MSI; the MSI is independently signed/verified. Temporary certificate files/private keys are removed in `finally` blocks. No private key/secret belongs in notes or source. If no certificate is configured, EXEs/MSI are explicitly reported unsigned; approval does not invent signing identity or antivirus clearance. Actual unpublished run 40316172507 was independently read back **NotSigned** on both EXEs and MSI; see the results above. Future tag-run signatures require their own readback.
4. **Packaging checks:** Windows builds with locked dependencies, packages the pair, checks extracted version/tray startup and exercises MSI install/version/removal. Unix packages come from reusable `ci.yml` with `native-only: true` and `release-packages: true`, running Linux x64/ARM64 and macOS Intel/ARM on their native runners. This release path does **not** substitute for the separate full Windows CI qualification; native reports can also retain explicit skipped/partial GUI/manual results that need inspection.
5. **macOS sealing:** `installer/macos/package.py` ad-hoc seals and verifies the nested executable/application before creating the DMG. There is no Developer ID or notarization step in this path.
6. **Complete bundle:** publishing requires portable success and Windows success or skip. Downloaded `release-*` artifacts are merged; repository SPDX JSON is generated; all six Unix packages and the SBOM must be nonempty. Windows ZIP/MSI must be nonempty when the Windows job succeeded. `sha256sum miccamwatch-*` generates the unified manifest, then `sha256sum --check SHA256SUMS` checks it.
7. **Provenance:** pinned `actions/attest-build-provenance` attests `dist/miccamwatch-*`, using GitHub OIDC/attestation permissions. `dist/*` is retained as `verified-release-bundle`. Verification must constrain repository, release signer workflow, exact source revision/ref and hosted runner; this is build provenance, not platform code signing or malware clearance.
8. **Stable/latest boundary:** the tag-only release action uploads `miccamwatch-*` and `SHA256SUMS`, fails unmatched upload patterns and generates release notes alongside its supplied body. With Windows approval true it requests stable/latest; otherwise it requests a Unix-only prerelease, not latest. That alternate path would contain eight uploads (six Unix packages, SBOM, manifest), **not this planned ten-asset release**. Do not silently treat it as fulfillment of the complete 0.21.0 publication plan.

## Open qualification and manual/hardware gates

Earlier successful platform-source evidence is retained, not rebranded as qualification of the exact integrated root:

| Earlier functional source | Native run | Scope |
| --- | --- | --- |
| `3f52ee4` | [Windows 40062931635](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40062931635) | Windows installed/portable activation, retained owner/HWND, real Explorer popups, MSI self-update refusal |
| `4931e00` | [macOS 40065426316](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40065426316) | Intel/ARM private socket publication, AppKit/Finder activation, retained lifecycle/removal |
| `519dc1c` | [Linux 40078682137](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40078682137) | x64/ARM GTK/Gio launcher, actual WM_CLASS, retained XID/owner, ELF/privilege and removal boundaries |

These are an aggregate on earlier unchanged platform sources, **not one complete green run of the integrated candidate**. See [Validation](VALIDATION.md#candidate-0210) for prior failures and scope. GUI observation fixtures are explicitly simulated; actual native surfaces, preferences and lifecycles are exercised. AppKit mounted/composited view images are not automatically WindowServer screenshots.

The fresh integrated run above passed all five targets; actual lifecycle and GUI receipts were reviewed. **Historical failed qualification, 2026-10-24:** separate [uninstall qualification 40106709656](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40106709656), on the same source, passed the four Unix targets but **failed Windows** after successful portable removal without a frontend. Its genuine ordinary account could not register a tray icon on a noninteractive `Service-…` station; MSI/Cargo cases after that point did not execute. An actual Explorer process does not by itself establish an interactive `WinSta0` desktop. The exact Shell API refusal cause remains unproved. That run's **NOT READY FOR RELEASE** verdict is historical and remains failed; the later actual local standard-user staged PASS closes the gate without rewriting it. See [the detailed qualification record](VALIDATION.md#integrated-native-qualification--2026-10-24).

Before publication, retain decisions/evidence against the frozen final source for every item:

- [x] Fresh complete CI on `4b90fd6`: all five native jobs passed; actual logs, lifecycle, refusal and cleanup receipts inspected. Later documentation-only evidence commits are not relabelled as that tested revision.
- [x] Automated seven-language/theme/geometry coverage and representative native views examined, including preserved focus/selection/scroll and retained-window behavior. Simulated observations remain labeled. One ARM installed launch capture is a blank initial frame; the cold-launch image and GTK fixture views render, so do not treat every lifecycle screenshot as painted-content proof.
- [x] **Former blocker closed 2026-10-26:** real disposable interactive standard-user Windows desktop; actual portable/tray, same-MSI application/cleanup, exact MSI and genuine Cargo removal/data/PATH evidence passed without ownership, UAC or ACL bypass. Explicitly approved staged aggregate, compiled source `30051ce…`, final harness `bebad88…`; original failures retained.
- [ ] Additional Windows manual coverage: standard-user MSI upgrade/repair, Windows 10, relevant DPI/multi-monitor/global-theme/taskbar/keyboard/screen-reader behavior. Elevated installed-application CI is not proof of those contexts.
- [ ] Linux: real Wayland/other supported desktops, session startup/reboot and accessibility. Xvfb/XFCE/Gio receipts alone do not close these gates.
- [ ] macOS: authorized real-user Finder reopen, menu/consultation lifecycle, notifications and manual OS authorization/profile flows, keeping cancellation and incomplete readback visible. No grant or bypass by automation.
- [ ] Physical ARM: authorized Raspberry Pi/SBC with recorded board/OS/architecture/glibc/GTK/PipeWire/Polkit versions, observable microphone and compatible USB webcam where available; actual desktop, permissions, hotplug and owned restoration/recovery. A native hosted ARM VM is not this evidence; non-USB/CSI control remains out of scope.
- [ ] Physical controls: separately authorized microphone/camera capture, supported mute/block/readback/restoration and real lock transitions on applicable platforms; report exclusive/driver paths and unavailable hardware without guessing. Historical other-version physical reports are not final-candidate certification.
- [x] Resource reports include candidate/helper/guard descendants and sampling/shared-page limits. All candidate functional modes passed; the global 15 MB soft target remains unmet, including Windows/macOS tray. Missing historical ARM baseline stays unqualified. Windows measured `watch` was deliberately terminated by its retained process identity after the full 20-second measurement; its exit code 1 is not a spontaneous startup failure.
- [x] Actual branch-package Windows unsigned status recorded for all three files, with macOS ad-hoc/unnotarized limits; no universal antivirus/SmartScreen/Gatekeeper clearance. Recheck the actual future tag-run bytes.

The former Windows ordinary-account gate is **closed by the explicitly approved local staged qualification**, not by relabelling the failed hosted workflow. Other unchecked items remain unvalidated scope, not implicit success or physical support claims. Physical ARM testing is not replaced by native hosted ARM results. Close any additional mandatory gate in the approved release scope or obtain an explicit scope decision; never silently waive acceptance or promise untested hardware compatibility. Both local qualification and the separately verified unpublished branch packages were observed **NotSigned**; a future tag build is still a separate signing/readback boundary.

### Reproducible Windows standard-user gate

The completed qualification used [Windows interactive standard-user qualification](WINDOWS_STANDARD_USER.md), not a replay of the known noninteractive batch launcher. Its documented routes share the same consumers: `native-uninstall.yml` manual subset `windows-standard` on a dedicated runner, or the local orchestrator with `-LocalDisposableVm`, an owned private root and an independently reviewed source SHA. The local route does not impersonate GitHub Actions. The real local standard login had to own the visible `WinSta0` input desktop, real Explorer and loaded profile. Filtered/restricted administrators, moved/session-zero tokens, foreign products/data and personal physical hosts are refused; real device inventory must positively report zero capture endpoints/cameras before native removal cases. The successful guest has since been safely deleted; package preparation does not authorize recreating it or replaying successful qualification stages.

Either route must finish the existing real portable, MSI shortcut/tray/window lifecycle, MSI manager delegation/removal, genuine Cargo uninstall and retained-data/refusal cases. Both native `status=passed` and the **same installed MSI** application's `passed`, `cleanupSafe`, `requireStandardUser` and `standardUserQualified` proof are mandatory. A timeout/failure retains state instead of force-removing it. Local artifacts carry local guest provenance, not an invented GitHub run or runner identity.

#### Historical checkpoints — not current blockers or instructions

The following 2026-10-24–26 checkpoints preserve their original failures, permissions and then-current verdicts. Present-tense statements inside this chronology describe those earlier checkpoints, **not current authority or current release readiness**. They are superseded only by the final staged aggregate below, not rewritten as green runs; none authorizes renewed VM creation, UAC, restoration or installation.

No appropriate VM runner was registered when checked on 2026-10-24. The full scenario remains **not executed**. The read-only local smoke and hosted [windows-context run 40127113302](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40127113302) passed on tooling source `5cc515b`; retained context confirms a real interactive **administrator**, correctly refused as a standard account. Parsing/C# compilation and that safety refusal do not close the GUI/MSI/uninstall gate. Keep **NOT READY FOR RELEASE** until authentic interactive evidence is retained and reviewed. No personal-PC installation/device action is an authorized fallback.

On 2026-10-25 the user's scoped Hyper-V controller is recorded **closed with no owned VM**; no renewed UAC, host mutation or Windows license acceptance is authorized during their absence. Microsoft publishes the ISO URL, but its stated registration/download conditions are not waived by public HTTP access. No local ISO download, Windows installation or standard login is claimed. Explicit local admission, orchestrator/native/application entry refusals on the physical PC and parsing/C# safety smoke passed; a positive disposable-guest run remains unexecuted. Targeted hosted MSI/application regression [40168712499](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40168712499) **passed** on exact tooling source `84e3ed73a0d2beecb34aa40ee2918b8114b9e2d2`; actual receipts and a reopened native-window capture were inspected. It proves the existing runner route still works after the shared-root API change, not the local standard-user route: its receipt explicitly has `requireStandardUser=false` and `standardUserQualified=false`.

Later on 2026-10-25 the user supplied the official ISO: exact Microsoft SHA-256, complete length, EnterpriseEval/x64/EN-US WIM metadata and Microsoft Setup signature were actually verified. A newly approved elevated controller created the sole isolated Generation-2 VM with TPM/Secure Boot; Windows now boots its normal OOBE. The user separately accepted the Windows and guest-tool licenses. **No genuine standard desktop or positive MCW qualification yet.** The official local-account option did not advance with the sent keyboard actions; a supplemental guest-only mouse UAC was cancelled, not bypassed or retried automatically. See the latest [Roadmap](ROADMAP.md) checkpoint. The earlier closed controller/media blocker is historical; the current blocker is completing normal OOBE and retaining real standard-user evidence.

Subsequent progress: the real QA console login **was verified** by the unchanged native context guard: session/token/LSA 2, Interactive logon 2, visible WinSta0/Default input desktop, same-user unelevated Explorer, normal loaded profile and an unrestricted nonadministrator primary token. Both guest users' CurrentUser RemoteSigned configuration had separate explicit approval; UAC/Defender remain unrelaxed. This closes the missing **environment prerequisite**, not product qualification. Machine prerequisite execution then exceeded the client's 1800-second wait: its guest outcome is **unknown**, no replay was attempted, and the new read-only diagnostic UAC was cancelled. The prior authority is now unavailable; VM state and installed-tool logs require renewed authorized inspection. Keep **NOT READY FOR RELEASE** until actual MSI/Cargo/portable receipts and safe final VM state are retained.

The subsequently approved one-VM UAC recovered the actual outcome, without replaying installers: all four Machine prerequisite installers exited **0**, final status **provisioned**, no reboot required, after about **41½ minutes**. The 30-minute client timeout was not an installer/product failure. After restarting only the existing guest, actual Git/Python/.NET commands and component-filtered `vswhere` confirmed the installed prerequisites; both required audio services are Running/Automatic. The renewed bounded authority permits the next genuine QA desktop Rust bootstrap and clean powered-off checkpoint. **The positive product scenario remains unexecuted**, so this recovery does not change the release verdict. Keep raw remoting transcripts private and redact their ephemeral transport-token headers from any shared evidence.

A further **qualification-harness** defect was reproduced in the real standard guest: `TokenHasRestrictions` returned **one byte**, while the old C# reader read a DWORD from that one-byte allocation. Independent native-byte observations proved intermittent false restriction/refusal flags; the corrected initialized by-reference scalar reader matched all retained after-fix observations. The permanent read-only smoke now compares with an independent native-byte oracle and passed locally, retaining correct administrative/physical-host refusals. **No standard-user guard condition was relaxed.** Earlier restriction-flag interpretations from the faulty reader are not reliable evidence of actual token filtering. Production Rust/GUI/installer code remains unchanged; the positive MSI/Cargo/portable gate still awaits execution after the corrected guest bootstrap.

The real guest now has a verified standard desktop, completed MSVC/SDK environment and actual **rustc/Cargo 1.99.1 zero-exit execution**, with a retained clean powered-off checkpoint. Its one-shot `5795ed4…` launch stopped **before the orchestrator/build/product actions** on a false private-launcher Python refusal: actual QA inspection proved that `Get-Command` returned both genuine Python first and the Store alias second; an array-valued `.StartsWith` result was incorrectly used as one Boolean. Scalar first-command selection is now corrected in the Windows qualification consumers, and the genuine standard account selected/executed Python 3.13.17 with exit 0 without PATH/profile/ACL changes or a weakened alias guard. The original failed attempt is byte-verified and retained, and the VM is **Off**. A fresh full run still requires restoring only the owned clean guest under valid scoped Hyper-V authority; the current controller does not expose Restore and no new UAC/restoration has been attempted. Do not replay the failed profile or infer success from missing MSI/native/application receipts. **NOT READY FOR RELEASE** remains.

**Historical local attempt, 2026-10-26:** the source-`30051ce…` real standard-user release build completed with **exit 0** at **07:14:59Z**. Its actual observation-only doctor also exited **0**, positively reporting zero capture endpoints and zero cameras. The orchestrator then falsely refused those results because Windows PowerShell 5 boxed the decoded JSON array inside `@(...)`; direct assignment fixes that harness defect without changing any count/status/inventory predicate. The actual source consumer now accepts the retained positive-zero report and rejects six invalid inventory boundaries; existing read-only context/parsing/C# smoke also passed. **No WiX/MSI/Cargo-install/native application or removal case ran.** Eighteen original command/evidence files were exported byte-for-byte with SHA-256 checks, the failed receipt is retained, and the sole guest was stopped with **Off actually read back**; compiled binaries remain on its disk. The current controller's one-shot clean restoration is already consumed: do not replay the failed root or delete its evidence/latch. A new clean restoration requires renewed UAC; any explicitly approved staged continuation must retain the initial failure, successful stage provenance and every remaining strict native/MSI proof requirement. No such continuation is claimed. **NOT READY FOR RELEASE.**

The user then explicitly approved **staged continuation without replaying the successful build**. This case-specific route retains all eighteen original proof-file hashes and both compiled-binary hashes, uses a new one-shot entry/proof directory, revalidates the real standard desktop/clean source/no preexisting product and fresh positive-zero doctor, then executes the **original AST statements** for WiX, genuine offline Cargo install and the complete native/MSI consumers. The original `failed` orchestrator remains unchanged. A separate continuation receipt must state compiled source **30051ce…**, decoder fix **9baff90…**, reused successful stages, unchanged original evidence and all mandatory native/same-MSI completion flags. **Approval is not execution or a PASS; native results are still pending.**

**Staged outcome:** WiX restore/MSI build and the genuine offline Cargo install each completed with **exit 0**; native execution then independently confirmed the real interactive standard context and positive-zero hardware. It failed at the first portable case's `directory.mkdir()` with **WinError 206**, before copying/launching that installation: the nested temporary layout requested a 248-character directory. The harness now places TEMP/TMP directly under the admitted owned root as `mcw-tmp`, retained separately by the workflow, without changing Windows policy or native safety assertions. Actual source-AST/filesystem smoke reduced the same case to 203 characters and passed; **native qualification has not yet been rerun or passed**. Both failures and successful-stage evidence are retained, including a SHA-verified durable archive outside Temp. The guest remains preserved and stopped; expired finite Hyper-V authority requires renewed explicit UAC before further guest work. The user's new cleanup authorization is conditional: preserve the official ISO and shared/personal resources, and delete the sole disposable VM/disks only after all tests finish, durable evidence exists and no diagnostic/restart is required. **NOT READY FOR RELEASE.**

**Native-only progress:** the real standard desktop then passed the original portable pair/frontend removals, live-tray/autostart contention refusals and native zero-device protection-preservation cases. The same candidate MSI actually installed with **exit 0** as the standard account. Its application consumer stopped before creating a proof directory because plain `Get-Item` could not read hidden `AppData`; adding `-Force` to application/branding ancestry reads preserves all reparse/root/ACL guards. Actual source-AST smoke accepted real hidden folders and still rejected real junctions in both readers. All 175 native/worker/MSI-install evidence files were byte/SHA-verified and saved durably. **The MSI is deliberately retained**: its shortcut/window/owner/update-refusal/cleanup proof and subsequent original manager-removal/Cargo cases remain required. Continue that exact registered product rather than reinstalling it or replaying the successful portable cases; do not waive `cleanupSafe` or rewrite any failed receipt. **NOT READY FOR RELEASE.**

#### Current Windows qualification result

**Final local result, 2026-10-26 09:40Z: PASS.** Tracking and awaiting the actual `.lnk` Shell launchers preserved the strict one-resident/same-HWND requirement and required every secondary launcher to exit 0. The same MSI application report is `passed`, `standardUserQualified=true`, `requireStandardUser=true`, `cleanupSafe=true`; binary self-update correctly refused with exit 2, zero network/staging and no owner/data/registration change. The original remaining MSI/Cargo consumers then passed exact manager removal and preservation, including actual `msiexec /x` and `cargo uninstall` exit 0. All **244 earlier proof files stayed unchanged**; the final result is a labeled staged aggregate, not a rewritten original orchestrator. Targeted read-only [CI 40301860252](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40301860252) also passed on harness `bebad88…`; its downloaded receipt explicitly remains **qualificationStatus=not_executed**. Durable final receipts, original failures, exact tested artifacts and genuine window captures are preserved outside Temp. The guest was stopped with Off readback and its controller closed before the separately authorized conditional cleanup. See [Validation](VALIDATION.md#windows-standard-user-qualification--2026-10-26) and the latest [Roadmap](ROADMAP.md) entries. At that qualification checkpoint, no release workflow, tag, main merge or production deployment had run; later authorized non-tag packaging is a separate operation, not this local qualification.

The separately authorized cleanup subsequently removed only the exact stopped disposable VM, its owned disks/checkpoints and mission-only temporary files, after durable evidence was hash-verified; the official ISO and unrelated resources were preserved. No qualification environment remains to rebuild as part of package preparation.

## Ordered publication plan — future approval required

1. **Freeze the development candidate.** Preserve the integrated GUI/ARM/docs work, protected worktrees/untracked exclusions and unchanged production site/privacy. Record the final full commit SHA only after all intended edits land. Current authority permits development-branch work and unpublished package preparation/verification, not a `main` change or publication.
2. **Qualify that exact revision.** Run the fresh complete CI and inspect retained native evidence and manual/hardware gates above under the qualification owner's authority. A source change affecting a qualified path invalidates its previous result. Document unresolved evidence and obtain explicit scope decisions, never a blanket invented PASS.
3. **Prepare unpublished release packages — authorized 2026-10-26.** The operator may dispatch Release on the frozen **non-tag `release/0.21.0-candidate` branch** to produce the unpublished bundle. Confirm ref, source SHA, Windows approval variable and actual environment configuration first. Never dispatch against a tag under this permission; the tag-only publication action must remain skipped.
4. **Verify the actual unpublished bundle.** Expect exactly ten uploads/nine checksummed and attested subjects, inspect package/signature/SBOM contents, retain manifest/digests/provenance and compare workflow source. Verify against the branch ref and exact candidate SHA for this dry run. Do not precompute or invent the future tag-run hashes; rebuilding/signing may produce different bytes.
5. **Request explicit owner approval before every production boundary:** proposed `main` integration, tag creation/push, GitHub publication/stable/latest and site changes/deployment. State the exact candidate, evidence, unresolved hardware/manual scope, unsigned/ad-hoc boundaries and expected ten assets. Existing Windows variable/environment settings and permission to push development branches are not substitutes. Approval of one boundary does not imply approval of the others.
6. **After approval, select and qualify the actual release revision.** Re-read `origin/main` and the approved candidate SHA. The inspected base `dc40e95…` is an ancestor, so fast-forward integration can preserve every history without a new merge commit; if main has advanced, inspect and preserve those changes before proceeding, never force-push. Before any main push, inspect the effective Cloudflare production-branch/automatic-deployment configuration: a main push that would deploy also needs that boundary's approval; do not silently alter provider settings. Integrate only as authorized, then require exact-revision CI before tagging. Recheck version 0.21.0, notes and scope. Only after **both tag and automatic-publication approval**, create/push `v0.21.0` at the approved release SHA. The tag starts the publishing workflow immediately: do not also create a competing release manually or expect a draft/reviewer pause.
7. **Inspect the authorized tag run and public bytes.** Require the expected complete bundle, intended stable/latest status and exact tag/source. Download the actual ten public assets, compare API sizes/digests, named checksums and tag-constrained provenance; inspect actual platform signatures and retain evidence. Failure is a stop condition, not permission to re-tag, overwrite assets or weaken gates.
8. **Qualify real public consumers after publication.** With separate authorization for state-changing disposable-host checks, exercise current installer/updater/removal paths (including public ARM opt-in only once the archive exists), ownership/preserved preferences/autostart and platform limits. Do not claim public-HTTPS migration proof from a local unpublished archive. Replace no live user installation without its separate approval and explicit completed restoration.
9. **Update site/download claims only after separate site approval and verified public assets.** Set `site/release.json` to 0.21.0, the actual published tag's full source SHA and freshly recomputed `installer/install.sh` SHA-256. The inspected candidate installer hash is `ef5dc62e71659261de0ad79b6b558e7d2446106d3adaed6019c27e651ce454bd`; recompute if its source changes. The current public pins (0.20.0/source `a265220…`/installer `b7348c9…`) remain untouched now. Add the newly available ARM64 archive link beside Linux x64 in `site/templates/index.html` with appropriate seven-language captions and explicit physical-validation limits; never advertise a nonexistent asset. Run the documented localization tests/export, then the separately approved existing Cloudflare publication process. Verify production download/version/installer digests, seven locales/privacy routes, canonical/hreflang/WebSite metadata, favicon, sitemap/robots and desktop/mobile/no-JS behavior. Update public-version documentation only when that release really exists; preserve historical screenshots/privacy/security content.

### Stop conditions

Stop before the next irreversible boundary if the ref/source/version differs, any required CI job fails or is skipped without an explicit scope decision, a native report is partial/unavailable, an asset is missing/extra/empty, a digest/provenance/signature mismatch occurs, a signing claim lacks the expected valid publisher/timestamp, Windows approval is absent for the ten-asset plan, or owner approval is missing. Unsafe permissions, foreign ownership, active protection/pending restoration, antivirus quarantine/detection and denied OS authorization are refusals to investigate, not reasons to force-kill, broaden ACLs, bypass protections, replay a potentially applied action or publish a narrower release silently. Preserve failed evidence and previously published immutable tags/assets.

## Verification commands — branch results recorded, public checks pending

Use a new download directory and a current GitHub CLI. Obtain `EXPECTED_SOURCE` from the independently reviewed frozen full commit SHA and `EXPECTED_REF` from the approved run: `refs/heads/release/0.21.0-candidate` for its unpublished branch bundle, or `refs/tags/v0.21.0` for the actual tag release. Do not derive the expected identity solely from the attestation being checked.

For the authorized unpublished branch run, download the `verified-release-bundle` from the recorded Release run with `gh run download RUN_ID --repo Roman-Cuisset/miccamwatch --name verified-release-bundle` (replace `RUN_ID` with the reviewed run identifier). This instruction applies only to the approved non-tag candidate run, not a future tag dispatch or publication.

For the future public release, inspect metadata and download only after publication is approved and exists:

```sh
gh release view v0.21.0 --repo Roman-Cuisset/miccamwatch \
  --json tagName,isDraft,isPrerelease,publishedAt,assets
gh api repos/Roman-Cuisset/miccamwatch/releases/tags/v0.21.0 \
  --jq '.assets[] | [.name, .size, .digest] | @tsv'
gh release download v0.21.0 --repo Roman-Cuisset/miccamwatch
```

Require the exact ten-name inventory above, nine non-duplicated exact manifest entries, and no unexpected uploaded files. Compare API `sha256:` digests and sizes with actual downloaded files, including `SHA256SUMS`; an absent API digest is not a successful comparison. API digests and the manifest share the release channel and are not an independent publisher signature.

With the **complete** bundle present (do not use `--ignore-missing` for release qualification):

```sh
# Linux:
sha256sum --check SHA256SUMS
# macOS alternative:
shasum -a 256 --check SHA256SUMS
```

PowerShell can compare the full named manifest as follows, after checking its exact expected inventory/count above:

```powershell
Get-Content .\SHA256SUMS | ForEach-Object {
    if ($_ -notmatch '^([0-9a-f]{64})  (miccamwatch-[A-Za-z0-9._-]+)$') {
        throw "Unexpected checksum entry: $_"
    }
    $expected = $Matches[1]
    $file = $Matches[2]
    $actual = (Get-FileHash -LiteralPath (Join-Path (Get-Location) $file) -Algorithm SHA256).Hash
    if ($actual -ine $expected) { throw "SHA-256 mismatch: $file" }
}
```

Verify **all nine payload/SBOM subjects**, not the unattested checksum manifest. This shell example requires the operator to set the independently approved identity variables first; it checks expected filenames explicitly rather than trusting a glob with missing files:

```sh
: "${EXPECTED_SOURCE:?Set the independently approved full source commit SHA}"
: "${EXPECTED_REF:?Set the approved full branch or tag ref}"
set -- \
  miccamwatch-windows-x86_64.zip \
  miccamwatch-windows-x86_64.msi \
  miccamwatch-linux-x86_64.tar.gz \
  miccamwatch-linux-aarch64.tar.gz \
  miccamwatch-macos-aarch64.tar.gz \
  miccamwatch-macos-x86_64.tar.gz \
  miccamwatch-macos-aarch64.dmg \
  miccamwatch-macos-x86_64.dmg \
  miccamwatch-sbom.spdx.json
for asset do
  test -s "$asset" || exit 1
  gh attestation verify "$asset" \
    --repo Roman-Cuisset/miccamwatch \
    --signer-workflow Roman-Cuisset/miccamwatch/.github/workflows/release.yml \
    --signer-digest "$EXPECTED_SOURCE" \
    --source-digest "$EXPECTED_SOURCE" \
    --source-ref "$EXPECTED_REF" \
    --deny-self-hosted-runners \
    --predicate-type https://slsa.dev/provenance/v1 \
    --format json || exit 1
done
```

Retain the complete verification output, signed certificate/identity and source/ref results alongside the reviewed run and manifest. A branch attestation does not satisfy exact-tag verification of later rebuilt public bytes.

Before execution/installation, inspect the actual downloaded Windows pair (after safe extraction) and MSI:

```powershell
Get-AuthenticodeSignature .\mcw.exe | Format-List
Get-AuthenticodeSignature .\mcw-tray.exe | Format-List
Get-AuthenticodeSignature .\miccamwatch-windows-x86_64.msi | Format-List
```

Record unsigned results honestly. If signing is claimed, require `Valid`, the independently expected publisher/certificate and valid timestamp on all three; do not accept an arbitrary valid signer. On macOS, inspect a safely mounted/copied app without launching or bypassing OS checks:

```sh
codesign --verify --deep --strict /path/to/MicCamWatch.app
codesign --display --verbose=4 /path/to/MicCamWatch.app
```

Substitute the actual app path. Successful ad-hoc seal verification is not Developer ID/notarization or Gatekeeper authorization. Actual branch provenance and Windows Authenticode reads are recorded above; all named checksums were independently computed from downloaded bytes. macOS seal/image commands ran on the native runners, not the Windows host, and `codesign --display` was not independently executed here. The future public-release/API/tag-constrained commands remain pending and are not claimed as passed.
