# MicCamWatch 0.21.0 — DRAFT release notes and publication checklist

**Status: unpublished; publication is not authorized.** Public stable remains [v0.20.0](https://github.com/Roman-Cuisset/miccamwatch/releases/tag/v0.20.0). This document prepares the integrated `release/0.21.0-candidate` work; it does not create a GitHub draft release, tag, final package hashes, signing verdict or deployment. Development-branch push permission is not permission to change `main`, create/push a tag, publish a release, or deploy the website. Do not run the Release workflow as part of this preparation.

[Validation and evidence](VALIDATION.md) · [Installation and ownership](INSTALLATION.md) · [Signing and safe handling](SIGNING.md) · [Release workflow](../.github/workflows/release.yml)

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
| Windows 10/11 x86_64 | Portable ZIP or per-user MSI; Win32 window, Notification Area and CLI/TUI | Integrated native CI passed on `4b90fd6`, including installed MSI under an elevated runner. The separate ordinary-account uninstall workflow failed before MSI/Cargo coverage; Windows 10, other DPI/multi-monitor configurations, taskbar grouping and screen readers remain unqualified. |
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

The complete [release.yml](../.github/workflows/release.yml) is authoritative; only its release-note body is updated by this preparation.

1. **Triggers and publication:** tag pushes matching `v*` or `workflow_dispatch` can run the workflow. The historical `refs/tags/v0.14.0` exclusion remains. A dispatch on a **non-tag branch** can build/attest/upload a `verified-release-bundle`, but `Publish complete GitHub release` executes only for `refs/tags/`. Dispatching against a tag is therefore not a safe publication-free dry run. There is no `draft: true` publication setting: a successful authorized tag run can publish immediately.
2. **Windows approval:** the Windows job requires `vars.WINDOWS_RELEASE_APPROVED == 'true'` and uses the `release` environment. Operators must inspect the environment's actual protection/approval configuration; its name alone does not prove required reviewers are configured. This repository variable is not the owner's new approval for 0.21.0. A tag name must match Cargo's version in the Windows verification step.
3. **Windows signing:** when the configured certificate is available, both EXEs are SHA-256 Authenticode-signed and timestamped, verified with `signtool` and expected certificate/timestamp checks, then embedded in the MSI; the MSI is independently signed/verified. Temporary certificate files/private keys are removed in `finally` blocks. No private key/secret belongs in notes or source. If no certificate is configured, EXEs/MSI are explicitly reported unsigned; approval does not invent signing identity or antivirus clearance. Final candidate signature status is not yet observed.
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

The fresh integrated run above now passed all five targets; actual lifecycle and GUI receipts were reviewed. Separate [uninstall qualification 40106709656](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40106709656), on the same source, passed the four Unix targets but **failed Windows** after successful portable removal without a frontend. Its genuine ordinary account could not register a tray icon on a noninteractive `Service-…` station; MSI/Cargo cases after that point did not execute. An actual Explorer process does not by itself establish an interactive `WinSta0` desktop. The exact Shell API refusal cause remains unproved. See [the detailed qualification record](VALIDATION.md#integrated-native-qualification--2026-10-24). **Verdict: NOT READY FOR RELEASE.**

Before publication, retain decisions/evidence against the frozen final source for every item:

- [x] Fresh complete CI on `4b90fd6`: all five native jobs passed; actual logs, lifecycle, refusal and cleanup receipts inspected. Later documentation-only evidence commits are not relabelled as that tested revision.
- [x] Automated seven-language/theme/geometry coverage and representative native views examined, including preserved focus/selection/scroll and retained-window behavior. Simulated observations remain labeled. One ARM installed launch capture is a blank initial frame; the cold-launch image and GTK fixture views render, so do not treat every lifecycle screenshot as painted-content proof.
- [ ] **Blocking:** provide a disposable interactive standard-user Windows desktop and complete the currently failed frontend/uninstall/MSI/Cargo qualification without bypassing ownership, UAC or ACL checks.
- [ ] Additional Windows manual coverage: standard-user MSI upgrade/repair, Windows 10, relevant DPI/multi-monitor/global-theme/taskbar/keyboard/screen-reader behavior. Elevated installed-application CI is not proof of those contexts.
- [ ] Linux: real Wayland/other supported desktops, session startup/reboot and accessibility. Xvfb/XFCE/Gio receipts alone do not close these gates.
- [ ] macOS: authorized real-user Finder reopen, menu/consultation lifecycle, notifications and manual OS authorization/profile flows, keeping cancellation and incomplete readback visible. No grant or bypass by automation.
- [ ] Physical ARM: authorized Raspberry Pi/SBC with recorded board/OS/architecture/glibc/GTK/PipeWire/Polkit versions, observable microphone and compatible USB webcam where available; actual desktop, permissions, hotplug and owned restoration/recovery. A native hosted ARM VM is not this evidence; non-USB/CSI control remains out of scope.
- [ ] Physical controls: separately authorized microphone/camera capture, supported mute/block/readback/restoration and real lock transitions on applicable platforms; report exclusive/driver paths and unavailable hardware without guessing. Historical other-version physical reports are not final-candidate certification.
- [x] Resource reports include candidate/helper/guard descendants and sampling/shared-page limits. All candidate functional modes passed; the global 15 MB soft target remains unmet, including Windows/macOS tray. Missing historical ARM baseline stays unqualified. Windows measured `watch` was deliberately terminated by its retained process identity after the full 20-second measurement; its exit code 1 is not a spontaneous startup failure.
- [ ] Record final Windows signing identity/timestamp or explicit unsigned status; macOS ad-hoc/unnotarized limits; no universal antivirus/SmartScreen/Gatekeeper clearance.

The failed Windows ordinary-account workflow is a blocking gate. Other unchecked items remain unvalidated scope, not implicit success or physical support claims. Physical ARM testing is not replaced by native hosted ARM results. Close any additional mandatory gate in the approved release scope or obtain an explicit scope decision; never silently waive acceptance or promise untested hardware compatibility.

### Reproducible Windows standard-user gate

Use [Windows interactive standard-user qualification](WINDOWS_STANDARD_USER.md), not a new run of the known noninteractive batch launcher. Two explicit routes share the same consumers: `native-uninstall.yml` manual subset `windows-standard` on a dedicated runner, or the local orchestrator with `-LocalDisposableVm`, an owned private root and an independently reviewed source SHA. The local route does not impersonate GitHub Actions. A fresh real local standard login in a disposable guest must own the visible `WinSta0` input desktop, real Explorer and loaded profile. Filtered/restricted administrators, moved/session-zero tokens, foreign products/data and personal physical hosts are refused; real device inventory must positively report zero capture endpoints/cameras before native removal cases.

Either route must finish the existing real portable, MSI shortcut/tray/window lifecycle, MSI manager delegation/removal, genuine Cargo uninstall and retained-data/refusal cases. Both native `status=passed` and the **same installed MSI** application's `passed`, `cleanupSafe`, `requireStandardUser` and `standardUserQualified` proof are mandatory. A timeout/failure retains state instead of force-removing it. Local artifacts carry local guest provenance, not an invented GitHub run or runner identity.

No appropriate VM runner was registered when checked on 2026-10-24. The full scenario remains **not executed**. The read-only local smoke and hosted [windows-context run 40127113302](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40127113302) passed on tooling source `5cc515b`; retained context confirms a real interactive **administrator**, correctly refused as a standard account. Parsing/C# compilation and that safety refusal do not close the GUI/MSI/uninstall gate. Keep **NOT READY FOR RELEASE** until authentic interactive evidence is retained and reviewed. No personal-PC installation/device action is an authorized fallback.

On 2026-10-25 the user's scoped Hyper-V controller is recorded **closed with no owned VM**; no renewed UAC, host mutation or Windows license acceptance is authorized during their absence. Microsoft publishes the ISO URL, but its stated registration/download conditions are not waived by public HTTP access. No local ISO download, Windows installation or standard login is claimed. Explicit local admission, orchestrator/native/application entry refusals on the physical PC and parsing/C# safety smoke passed; a positive disposable-guest run remains unexecuted. Targeted hosted MSI/application regression [40168712499](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40168712499) **passed** on exact tooling source `84e3ed73a0d2beecb34aa40ee2918b8114b9e2d2`; actual receipts and a reopened native-window capture were inspected. It proves the existing runner route still works after the shared-root API change, not the local standard-user route: its receipt explicitly has `requireStandardUser=false` and `standardUserQualified=false`.

Later on 2026-10-25 the user supplied the official ISO: exact Microsoft SHA-256, complete length, EnterpriseEval/x64/EN-US WIM metadata and Microsoft Setup signature were actually verified. A newly approved elevated controller created the sole isolated Generation-2 VM with TPM/Secure Boot; Windows now boots its normal OOBE. The user separately accepted the Windows and guest-tool licenses. **No genuine standard desktop or positive MCW qualification yet.** The official local-account option did not advance with the sent keyboard actions; a supplemental guest-only mouse UAC was cancelled, not bypassed or retried automatically. See the latest [Roadmap](ROADMAP.md) checkpoint. The earlier closed controller/media blocker is historical; the current blocker is completing normal OOBE and retaining real standard-user evidence.

Subsequent progress: the real QA console login **was verified** by the unchanged native context guard: session/token/LSA 2, Interactive logon 2, visible WinSta0/Default input desktop, same-user unelevated Explorer, normal loaded profile and an unrestricted nonadministrator primary token. Both guest users' CurrentUser RemoteSigned configuration had separate explicit approval; UAC/Defender remain unrelaxed. This closes the missing **environment prerequisite**, not product qualification. Machine prerequisite execution then exceeded the client's 1800-second wait: its guest outcome is **unknown**, no replay was attempted, and the new read-only diagnostic UAC was cancelled. The prior authority is now unavailable; VM state and installed-tool logs require renewed authorized inspection. Keep **NOT READY FOR RELEASE** until actual MSI/Cargo/portable receipts and safe final VM state are retained.

The subsequently approved one-VM UAC recovered the actual outcome, without replaying installers: all four Machine prerequisite installers exited **0**, final status **provisioned**, no reboot required, after about **41½ minutes**. The 30-minute client timeout was not an installer/product failure. After restarting only the existing guest, actual Git/Python/.NET commands and component-filtered `vswhere` confirmed the installed prerequisites; both required audio services are Running/Automatic. The renewed bounded authority permits the next genuine QA desktop Rust bootstrap and clean powered-off checkpoint. **The positive product scenario remains unexecuted**, so this recovery does not change the release verdict. Keep raw remoting transcripts private and redact their ephemeral transport-token headers from any shared evidence.

A further **qualification-harness** defect was reproduced in the real standard guest: `TokenHasRestrictions` returned **one byte**, while the old C# reader read a DWORD from that one-byte allocation. Independent native-byte observations proved intermittent false restriction/refusal flags; the corrected initialized by-reference scalar reader matched all retained after-fix observations. The permanent read-only smoke now compares with an independent native-byte oracle and passed locally, retaining correct administrative/physical-host refusals. **No standard-user guard condition was relaxed.** Earlier restriction-flag interpretations from the faulty reader are not reliable evidence of actual token filtering. Production Rust/GUI/installer code remains unchanged; the positive MSI/Cargo/portable gate still awaits execution after the corrected guest bootstrap.

The real guest now has a verified standard desktop, completed MSVC/SDK environment and actual **rustc/Cargo 1.99.1 zero-exit execution**, with a retained clean powered-off checkpoint. Its one-shot `5795ed4…` launch stopped **before the orchestrator/build/product actions** on a false private-launcher Python refusal: actual QA inspection proved that `Get-Command` returned both genuine Python first and the Store alias second; an array-valued `.StartsWith` result was incorrectly used as one Boolean. Scalar first-command selection is now corrected in the Windows qualification consumers, and the genuine standard account selected/executed Python 3.13.17 with exit 0 without PATH/profile/ACL changes or a weakened alias guard. The original failed attempt is byte-verified and retained, and the VM is **Off**. A fresh full run still requires restoring only the owned clean guest under valid scoped Hyper-V authority; the current controller does not expose Restore and no new UAC/restoration has been attempted. Do not replay the failed profile or infer success from missing MSI/native/application receipts. **NOT READY FOR RELEASE** remains.

**Latest local attempt, 2026-10-26:** the source-`30051ce…` real standard-user release build completed with **exit 0** at **07:14:59Z**. Its actual observation-only doctor also exited **0**, positively reporting zero capture endpoints and zero cameras. The orchestrator then falsely refused those results because Windows PowerShell 5 boxed the decoded JSON array inside `@(...)`; direct assignment fixes that harness defect without changing any count/status/inventory predicate. The actual source consumer now accepts the retained positive-zero report and rejects six invalid inventory boundaries; existing read-only context/parsing/C# smoke also passed. **No WiX/MSI/Cargo-install/native application or removal case ran.** Eighteen original command/evidence files were exported byte-for-byte with SHA-256 checks, the failed receipt is retained, and the sole guest was stopped with **Off actually read back**; compiled binaries remain on its disk. The current controller's one-shot clean restoration is already consumed: do not replay the failed root or delete its evidence/latch. A new clean restoration requires renewed UAC; any explicitly approved staged continuation must retain the initial failure, successful stage provenance and every remaining strict native/MSI proof requirement. No such continuation is claimed. **NOT READY FOR RELEASE.**

## Ordered publication plan — future approval required

1. **Freeze the development candidate.** Preserve the integrated GUI/ARM/docs work, protected worktrees/untracked exclusions and unchanged production site/privacy. Record the final full commit SHA only after all intended edits land. Development-branch pushes alone are allowed under the current authorization.
2. **Qualify that exact revision.** Run the fresh complete CI and inspect retained native evidence and manual/hardware gates above under the qualification owner's authority. A source change affecting a qualified path invalidates its previous result. Document unresolved evidence and obtain explicit scope decisions, never a blanket invented PASS.
3. **Obtain separate authorization for release-package preparation.** Only then may an operator dispatch Release on the frozen **non-tag development branch** to produce the unpublished bundle. Confirm ref, source SHA, Windows approval variable and protected-environment review first. This document does not authorize that dispatch; do not run it now.
4. **Verify the actual unpublished bundle.** Expect exactly ten uploads/nine checksummed and attested subjects, inspect package/signature/SBOM contents, retain manifest/digests/provenance and compare workflow source. Verify against the branch ref and exact candidate SHA for this dry run. Do not precompute or invent the future tag-run hashes; rebuilding/signing may produce different bytes.
5. **Request explicit owner approval before every production boundary:** proposed `main` integration, tag creation/push, GitHub publication/stable/latest and site changes/deployment. State the exact candidate, evidence, unresolved hardware/manual scope, unsigned/ad-hoc boundaries and expected ten assets. Existing Windows variable/environment settings and permission to push development branches are not substitutes. Approval of one boundary does not imply approval of the others.
6. **After approval, select and qualify the actual release revision.** Integrate to `main` only as authorized; if the final revision differs from the qualified candidate, run the required exact-revision qualification before tagging. Recheck version `0.21.0`, notes, gates and scope. Do not rewrite existing tags or release assets. Create/push `v0.21.0` only after explicit tag/publication approval, recognizing that this triggers the publishing workflow, not a draft-only upload.
7. **Inspect the authorized tag run and public bytes.** Require the expected complete bundle, intended stable/latest status and exact tag/source. Download the actual ten public assets, compare API sizes/digests, named checksums and tag-constrained provenance; inspect actual platform signatures and retain evidence. Failure is a stop condition, not permission to re-tag, overwrite assets or weaken gates.
8. **Qualify real public consumers after publication.** With separate authorization for state-changing disposable-host checks, exercise current installer/updater/removal paths (including public ARM opt-in only once the archive exists), ownership/preserved preferences/autostart and platform limits. Do not claim public-HTTPS migration proof from a local unpublished archive. Replace no live user installation without its separate approval and explicit completed restoration.
9. **Update site/download claims only after separate site approval.** Keep public 0.20.0 instructions and production site intact until authorized and the verified 0.21.0 release really exists. Retain historical screenshot provenance and privacy/security policy; do not add physical-support, signing or success claims unsupported by the evidence. Announce only the verified authorized scope.

### Stop conditions

Stop before the next irreversible boundary if the ref/source/version differs, any required CI job fails or is skipped without an explicit scope decision, a native report is partial/unavailable, an asset is missing/extra/empty, a digest/provenance/signature mismatch occurs, a signing claim lacks the expected valid publisher/timestamp, Windows approval is absent for the ten-asset plan, or owner approval is missing. Unsafe permissions, foreign ownership, active protection/pending restoration, antivirus quarantine/detection and denied OS authorization are refusals to investigate, not reasons to force-kill, broaden ACLs, bypass protections, replay a potentially applied action or publish a narrower release silently. Preserve failed evidence and previously published immutable tags/assets.

## Verification commands for the authorized operator — not executed here

Use a new download directory and a current GitHub CLI. Obtain `EXPECTED_SOURCE` from the independently reviewed frozen full commit SHA and `EXPECTED_REF` from the approved run: `refs/heads/release/0.21.0-candidate` for its unpublished branch bundle, or `refs/tags/v0.21.0` for the actual tag release. Do not derive the expected identity solely from the attestation being checked.

For a future authorized branch dry run, download the `verified-release-bundle` from the recorded Release run with `gh run download RUN_ID --repo Roman-Cuisset/miccamwatch --name verified-release-bundle` (replace `RUN_ID` with the reviewed run identifier). This is a download instruction, not authority to dispatch the workflow.

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

Substitute the actual app path. Successful ad-hoc seal verification is not Developer ID/notarization or Gatekeeper authorization. No command/result above has been exercised by this notes-only preparation.
