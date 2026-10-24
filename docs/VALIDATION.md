# Validation and evidence boundaries

[Documentation](README.md) · [Installation](INSTALLATION.md) · [Usage](USAGE.md) · [Release history](RELEASE_HISTORY.md) · [Architecture](ARCHITECTURE.md) · [Roadmap](ROADMAP.md)

**Evidence is version-, source-, platform- and scenario-specific.** A build is not a release; a hosted native desktop is not physical hardware; a screenshot with simulated observations is not a microphone/camera capture test. This guide separates recorded qualification from the pending qualification of the exact integrated candidate; no new checks were run by these documentation edits.

## Public release 0.20.0

The public baseline is [v0.20.0](https://github.com/Roman-Cuisset/miccamwatch/releases/tag/v0.20.0), source [`a26522028eafe02591f58c03ceb5c367f2d5d60b`](https://github.com/Roman-Cuisset/miccamwatch/tree/a26522028eafe02591f58c03ceb5c367f2d5d60b), annotated tag object `547f6185a0579876041d1ae7ebcdd6feccdcb5cf`. It was published on 2026-10-22, release ID `420936561`. [Native release run 39807585260](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/39807585260) passed and the public files were subsequently downloaded/verified: **nine uploaded assets**, eight named SHA-256 entries and eight signed SLSA v1 provenance subjects. GitHub's automatically generated source archives are separate from those uploaded assets.

| Public target | Recorded native/software evidence | Not established by that evidence |
| --- | --- | --- |
| Windows x64 | Native CLI/TUI/tray, scoped owners/IPC, portable transactions and ZIP/MSI package smoke; official Shell/installed icon checks | Universal device enforcement, antivirus clearance, a full three-pane consultation window on public 0.20, or transfer of active protection during update |
| Linux x64, glibc 2.35+ | Native PipeWire/SPA builds and regression checks, extracted CLI/PTY, StatusNotifier lifecycle, managed installation/removal and optional helper privilege boundaries | Every desktop/session, direct ALSA/V4L2 capture attribution, physical microphone denial, non-USB/CSI control, or any public Linux ARM64 package |
| macOS 15+, Apple Silicon and Intel | Native CoreAudio/CoreMediaIO helper, CLI/TUI/menu, **0.20 singleton consultation**, app/DMG, Finder reopen, autostart intent and accepted-socket correction | Camera client PID/frame flow, human TCC/profile approval, notarization, physical microphone/camera transitions, or the redesigned 0.21 consultation |

### Supporting scenarios and their limits

- [Integrated native CI 39768698678](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/39768698678), product source `201e`, passed four targets. Both AppKit reports covered normal menu Exit, close/reopen singleton consultation, ordinary Finder activation, duplicate-owner preservation, readiness and moved bundle. Hardware/OS authorization remained outside that proof.
- [Mac lifecycle 39579584237](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/39579584237), source `24303fe`, passed eight start/readiness/stop cycles per architecture and three reopens per cycle. The fix explicitly made accepted streams blocking on BSD; it did not enlarge deadlines. The failed 0.19.1 Intel attempt remains historical evidence, not an unresolved claim that the unchanged 0.20 scenario still fails.
- [Windows branding 39655798674](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/39655798674) exercised the actual per-user MSI and installed CLI/tray/Start/Apps icon surfaces at 16–256 pixels, 36 comparisons, followed by real removal. This did not manufacture Authenticode identity or PE version metadata.
- Homebrew [ARM 39768723173](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/39768723173) and [Intel 39774103433](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/39774103433) qualified six loaded/dormant/disabled autostart paths on the same product source `201e`: genuine 0.16.1 → 0.20 source-package upgrade, explicit ordinary-user refresh before cleanup, no-op, test/removal/reinstall and retained user data. This is recipe qualification, **not publication of a tap/Homebrew-core package**. A repository AUR recipe likewise does not prove an external AUR submission.
- Authorized Windows workstation tests used isolated payloads and state under a real ordinary-user token. Companion- and CLI-hosted portable removal reached actual worker `removed` results with retained data; command acceptance alone was not counted as deletion. Six refusal cases preserved bytes/state; actual Cargo delegation/removal and a separate test-family per-user MSI routing/removal passed. The original pair, running user tray, two existing 0.19 MSI registrations and device-control state were preserved. **The private-family MSI test was not a migration/removal of the unchanged public MSI on that workstation.** These local reports are recorded, not publicly downloadable CI artifacts; see the [immutable 0.20 qualification journal](https://github.com/Roman-Cuisset/miccamwatch/blob/dc40e95bc9c1c6a14386eb5e773f01b62b47aa71/docs/ROADMAP.md#L1359-L1366).

Windows EXEs/MSI are **unsigned**; macOS bundles are **ad-hoc sealed, not Developer ID signed or notarized**. A valid checksum or GitHub attestation is not platform signing, malware clearance or SmartScreen/Gatekeeper approval. [Signing](SIGNING.md) preserves exact historical Windows hashes, observed Defender scan limits and safe incident handling. The TLS dependency audit addressed the versioned advisories listed in [0.20 history](RELEASE_HISTORY.md#0200--native-consultation-and-scoped-uninstall); three Linux D-Bus dependency maintenance warnings remained visible, with no suppressed advisory.

## Candidate 0.21.0

**Unpublished development only.** Current integration is on [`release/0.21.0-candidate`](https://github.com/Roman-Cuisset/miccamwatch/tree/release/0.21.0-candidate), combining the GUI/ARM work with the main baseline and reorganized guides. The preserved GUI/ARM source snapshot is [`cafd98daf7541f317b44aee98edb3b6efa3af561`](https://github.com/Roman-Cuisset/miccamwatch/tree/cafd98daf7541f317b44aee98edb3b6efa3af561); its earlier development-branch name and run results are historical, not pointers to the current integration. Neither branch is a public tag, release asset set, stable installation instruction or deployment. Public 0.20.0 remains the consumer baseline.

**Exact integrated-candidate qualification: pending.** No fresh complete five-target CI result for this integrated candidate is recorded here. Release qualification must identify the exact integrated commit and retain the complete Windows x64, Linux x64/ARM64 and macOS Intel/Apple Silicon results, artifacts and cleanup evidence. The earlier aggregate below does not qualify the new repository root or authorize publication.

The candidate adds native Linux ARM64, redesigned native consultation on Win32/GTK3/AppKit and unified owned activation. Its **previous five-target qualification was an aggregate of platform-specific runs on unchanged platform sources**, not one all-green run of the final GUI/ARM snapshot or the new integrated candidate:

| Historical targets | Previously qualified functional source | Successful native run and actual scope |
| --- | --- | --- |
| Windows x64 | [`3f52ee4`](https://github.com/Roman-Cuisset/miccamwatch/commit/3f52ee4) | [40062931635](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40062931635): installed MSI/CLI/GUI/shortcut concurrent activation, retained HWND/owner, eight genuine Explorer popups, portable update and MSI double-ownership refusal |
| macOS Intel / Apple Silicon | [`4931e00`](https://github.com/Roman-Cuisset/miccamwatch/commit/4931e00) | [40065426316](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40065426316): securely published socket, AppKit, cold Finder opening, retained window/owner, bundle/LaunchAgent lifecycle and normal removal |
| Linux x64 / ARM64 | [`519dc1c`](https://github.com/Roman-Cuisset/miccamwatch/commit/519dc1c) | [40078682137](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40078682137): installed GTK/Gio launcher, exact WM_CLASS, retained XID/owner through activation and normal close/reopen, ELF and privilege boundaries, profiles/menu and owned removal |

The Windows run above also contained then-failing Unix jobs; the macOS-qualified run still had Linux WM_CLASS failures. Later targeted Unix/Linux runs closed those failures without rerunning unchanged successful platform sources. Do not relabel every job in every linked run as successful. The [immutable prior GUI/ARM qualification record](https://github.com/Roman-Cuisset/miccamwatch/blob/cafd98daf7541f317b44aee98edb3b6efa3af561/docs/ROADMAP.md#L1698-L1718) and [integrated roadmap](ROADMAP.md) preserve that historical sequence and its limits; fresh exact-candidate CI is a separate gate.

Linux reports covered **21 GTK cases per architecture**, seven languages and Adwaita/light-dark/HighContrast. Installed receipts confirmed exact `MicCamWatch` WM_CLASS, repeated activation, focused Alt+F4 hiding and reopening the same owned window, hidden background startup and real refusal of foreign namespaces/modified assets. The x64 report covered the real 0.20 launcher migration; ARM's historical-migration field stayed `null` because no old public ARM package exists.

Earlier native ARM qualification [39832624207](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/39832624207), source `2998946`, passed five platforms; [39828453493](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/39828453493) exercised managed/Cargo removal across four Unix targets. The ARM runner established native Ubuntu 22.04/glibc 2.35/PipeWire-SPA 0.3.48, ELF64 little-endian machine 183 on **both** executables, native ABI roundtrip, virtual capture, bounded PipeWire faults and recovery without false STOP, installation/root rollback/refusals and a VM panel. These earlier runs are not substitutes for the later GUI/launcher qualification above.

**Candidate boundaries:**

- No published Linux ARM64 asset, no historical pre-0.21 ARM migration/resource baseline, and no Raspberry Pi/SBC physical certification. A native hosted ARM VM is not a physical board; ARMv7/32-bit/musl and CSI camera control are not promised.
- The Windows integration runner was an administrator. It did not qualify MSI installation with a standard-user token, every Windows 10/DPI configuration, real taskbar grouping or all accessibility readers.
- Linux GUI checks used a hosted graphical environment/Xvfb; they did not qualify every real Wayland desktop, login/reboot path or ARM peripheral. GTK3 is a candidate consultation prerequisite, not a new dependency claim for stable headless CLI use.
- GUI observation fixtures explicitly marked simulated data validate native presentation and real preferences/geometry, not physical capture. AppKit mounted/composited PNGs are not automatically WindowServer screenshots. Actual installed-window/activation proofs and simulated-observation views are distinct evidence classes.
- No physical microphone/camera enforcement certification, new OS authorization, antivirus clearance, signing/notarization or global 15 MB resource-budget success follows from these runs. Earlier failed fixtures/results remain recorded rather than rewritten.

## Hardware and manual limits

| Evidence class | What exists | Boundary that must remain visible |
| --- | --- | --- |
| Authorized physical Linux USB camera | One checksum/provenance-verified UVC package on one host; real Polkit cancellation/approval, ongoing-capture revocation, refusal of new root capture, exact driver restoration and resumed capture | Versioned 0.16-era support envelope, not every webcam/kernel, future candidate, non-USB/CSI, hotplug reblock or physical microphone protection; [exact device/archive/agent record](ARCHITECTURE.md#authorized-linux-physical-verification) |
| Friend-reported macOS physical camera | [@repentandliveholy](https://github.com/repentandliveholy), Apple Silicon/macOS 27: Telegram 0 → 1 → 0 and locally patched FaceTime HD Camera START/STOP | Reported, not maintainer/CI hardware evidence; unknown camera client PID, medium confidence/degraded coverage, no frame-flow or per-app enforcement proof; [attribution and empty-runner limits](ARCHITECTURE.md#native-0151-verification-boundaries) |
| Real Linux PipeWire virtual source | Authenticated capture PID, active/idle/stop, independent mute/restoration, already-muted preservation and systemd socket activation | Real session graph/readback, not a physical microphone, global access denial or audible output; [socket proof](ARCHITECTURE.md#socket-activated-pipewire-mute-verification) |
| Windows native owners on zero-device runners | Ownership/IPC/isolation, UI-exit persistence, explicit release, refusal/cleanup and measured process-tree costs | No physical exclusive/ASIO/direct-driver protection or multi-endpoint/hotplug hardware certification; [protection contract](ARCHITECTURE.md#candidate-0170-windows-first-protection-contract) |
| Native notifications/sound/UI | OS API acceptance, selected visible notifications/menu/window checks, isolated keyboard/clipboard-denial scenarios | API success is not every notification's visibility, granted macOS notification authorization, physical speaker audibility or all accessibility behavior |

Requested protection, effective SDK mute/device restriction and observed capture are **three separate facts**. SDK mute is not capture permission; camera running-state is not frame flow. Partial, unavailable or unknown observation/control never means safe, idle, muted or globally allowed. macOS profile administrator inspection is not TCC permission; Linux camera helper Polkit authorization is separate from user capture permission. Camera PID unknown on macOS cannot authorize process termination.

Physical Linux/macOS microphone mute/capture effects, macOS approved-profile effectiveness, real lock transitions, authorization dialogs and other hardware/session combinations need separately authorized manual evidence. Do not run controls on a user's live installation merely to reproduce CI. Explicitly release protection using the matching version and finish owned restoration before external replacement/removal; closing the frontend or killing a guard is not a release. See [operational limits](USAGE.md#observation-and-control-limits) and [safe updates](INSTALLATION.md#updates).

**Resources:** the soft target is **15,000,000 bytes**; approximately 30 MB is a reference, not a hard cutoff. Neither target success nor complete Linux budget coverage is established merely by bounded code or low sampled RSS. Include every MCW helper/guard/tray and transient child, distinguish root peak from concurrent/sustained tree values, and report shared-page double counting, missing brief children and sampling gaps. Historical populated-host and zero-device measurements, failed reference profiles and comparison qualification remain in [Architecture's native resource evidence](ARCHITECTURE.md#historical-native-candidate-qualification), not recast as current hardware guarantees.

## Historical evidence

[Release history](RELEASE_HISTORY.md) preserves each removed README historical/version block, distinct release/CI/migration proof links and an [original-to-destination map](RELEASE_HISTORY.md#original-readme-mapping). The immutable [original main README](https://github.com/Roman-Cuisset/miccamwatch/blob/dc40e95bc9c1c6a14386eb5e773f01b62b47aa71/README.md) provides the pre-reorganization source; the GUI-branch README is a different, candidate-containing snapshot.

Use [Architecture](ARCHITECTURE.md) as the canonical detailed contract/fixture ledger and [Signing](SIGNING.md) for exact historical Windows hashes, signature results and Defender limits. Some unchanged canonical sections say “candidate”, “stable/latest” or “publication held” in their original version context. Read their subsequent publication sections and the chronological archive; those phrases do not override public 0.20.0 or make 0.21.0 public. [ROADMAP](ROADMAP.md) keeps the chronological decisions, failed runs and qualification closure. Historical site beacon observations are [dated website evidence](RELEASE_HISTORY.md#historical-website-evidence), not a current analytics assertion; current policy is [publicly available](https://miccamwatch.pages.dev/privacy).

## Reproducing checks

### Match the source before interpreting results

1. Choose the public tag or the **exact recorded candidate source commit**, not a moving branch tip. The platform commits above reproduce historical evidence only; new integrated qualification must record its own exact commit. Inspect that revision's workflow, runner architecture, dependencies, feature flags and artifact reports. A cross-target `check`, newer source tree or green job summary cannot replace that native evidence.
2. Public verification starts with release metadata, exact asset bytes and the named checksum entry. Follow [Installation: verify downloads](INSTALLATION.md#verify-downloads), then verify signed build provenance against the exact repository, release workflow, tag/ref, source digest and signer, refusing self-hosted runners where the original proof required hosted runners. `SHA256SUMS` and API digests share the release channel; they are not an independent publisher signature. The checksum manifest is not itself one of the eight attested payload/SBOM subjects.
3. Inspect the actual retained `app-smoke.json`, `resources.json`, installation/removal receipts and screenshots appropriate to the run, not just workflow color. `comparison_qualified`, baseline/candidate functional success, cleanup, absent hardware and skipped cases have distinct meanings. Private local reports cited by the roadmap are not magically public/downloadable.

### Native developer checks

These are documented workflow commands, **not checks run by this documentation change**. Install native build prerequisites from [Build from source](INSTALLATION.md#build-from-source), use an isolated checkout/test account and choose features for the actual host:

```sh
cargo fmt --check
# Windows, including native tray:
cargo clippy --all-targets --features windows-tray --locked -- -D warnings
cargo test --all-targets --features windows-tray --locked
# Linux, including the optional camera-helper build:
cargo clippy --all-targets --features linux-camera-helper --locked -- -D warnings
cargo test --all-targets --features linux-camera-helper --locked
# macOS:
cargo clippy --all-targets --locked -- -D warnings
cargo test --all-targets --locked
# With cargo-audit installed:
cargo audit
```

Do not run all host-specific blocks indiscriminately. The Linux native ABI regression is `cargo test --locked -p libspa --lib native_headers_builder_roundtrip_after_growth -- --nocapture`. Windows CI also tests the basic CLI without `windows-tray`. A normal test pass does not exercise every ignored native/visual/privileged scenario.

Canonical workflow entry points in this checkout:

| Purpose | Source |
| --- | --- |
| Native CLI/TUI/tray, owner/IPC, resource and extracted-package evidence | [ci.yml](../.github/workflows/ci.yml) |
| Public packaging/provenance/publication gates | [release.yml](../.github/workflows/release.yml) |
| Public Unix installer/shell/upgrade paths | [unix-install.yml](../.github/workflows/unix-install.yml) |
| Public Windows updater transactions | [windows-update.yml](../.github/workflows/windows-update.yml) |
| Native scoped removal/delegation | [native-uninstall.yml](../.github/workflows/native-uninstall.yml) |
| Native Homebrew and Arch recipe lifecycle | [homebrew-native.yml](../.github/workflows/homebrew-native.yml), [arch-package-native.yml](../.github/workflows/arch-package-native.yml) |

For current candidate drivers/options inspect [the integrated candidate's CI workflow](https://github.com/Roman-Cuisset/miccamwatch/blob/release/0.21.0-candidate/.github/workflows/ci.yml), then pin the exact source commit before running or interpreting qualification. The [historical GUI/ARM workflow at cafd98d](https://github.com/Roman-Cuisset/miccamwatch/blob/cafd98daf7541f317b44aee98edb3b6efa3af561/.github/workflows/ci.yml) remains available to reproduce the earlier evidence. Native platform selection and fresh five-target ARM/GUI qualification must not be inferred from the older stable workflow matrix or the prior aggregate.

**Safety gate:** native integration drivers can install/remove packages, alter a disposable account's autostart/clipboard/desktop or exercise controls. Use their documented disposable-host prerequisites and retained ownership/cleanup checks; never strip isolation guards, fix permissions broadly or use the installed user application as a fixture. Hosted prerequisites are not recommended production ACL changes. Publication/deployment workflows require separate explicit authorization; documenting a reproduction path does not authorize a release, tag, device action or website deployment.
