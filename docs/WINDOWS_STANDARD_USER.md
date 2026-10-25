# Windows interactive standard-user qualification

This is an **operator-provisioned disposable VM** qualification, not permission to run installation, uninstallation, microphone/camera controls, or account provisioning on a personal computer. It does not create a VM, accept a Windows license, register a runner, generate registration secrets, or dispatch itself. A suitable authorized guest must actually exist. Choose either the explicit local-guest route below or the existing GitHub runner route; never fake GitHub markers to admit a workstation.

Both routes use the same [PowerShell orchestrator](../installer/tests/windows-standard-user.ps1), [native uninstall cases](../installer/tests/native_uninstall.py), and installed MSI application consumer. They build the real tray-enabled binaries and per-user MSI and genuinely install the Cargo package as the logged-in standard account into a dedicated root. The optional manual [native-uninstall workflow](../.github/workflows/native-uninstall.yml), subset `windows-standard`, calls the reusable [VM workflow](../.github/workflows/windows-standard-user.yml). Neither route uses the hosted `windows_ordinary.py` batch launcher as interactive proof.

## Why a separate fixture exists

A genuine standard-account batch logon can still be on a noninteractive `Service-…` window station. It cannot qualify an installed application's Explorer shortcut, notification area, input desktop, or reopen/concurrent-owner behavior. Previously attempted hosted RDP sessions also failed during Windows logon initialization; do not blindly repeat them or manufacture a session/token to get a green report. These fixture limitations do not establish a product GUI failure or a product qualification PASS.

This path requires an actual console or normal RDP logon, the account's normal loaded profile, its own real Explorer shell, and the visible `WinSta0` input desktop. Filtered administrator tokens, restricted tokens, scheduled/batch/service runners, moved tokens, and fake profile/AppData values are rejected. Ordinary hosted-runner admission remains separate and is not weakened.

## 1. Provision a disposable guest, not the workstation

Use an approved hypervisor and a properly licensed/activated Windows 11 Pro/Enterprise x64 guest (or an evaluation image within its permitted license). Allocate at least 4 virtual CPUs, 12 GB RAM, and enough disposable disk for two release builds and retained native evidence; 80 GB free is recommended. Keep the guest on a separate disposable virtual disk, with no personal files, host filesystem shares, saved personal credentials, or OneDrive/cloud-account sign-in. Use a local provisioning administrator only inside this guest.

The admission helper recognizes genuine Hyper-V (`Microsoft Corporation` / `Virtual Machine`), VMware (`VMware Virtual Platform`), VirtualBox, and QEMU/KVM guest metadata and also requires `HypervisorPresent`. A physical host running VBS/Hyper-V is not admitted. Do not spoof manufacturer/model values or change the helper to admit a different environment. Metadata is an operator safety gate, not cryptographic VM attestation.

At VM creation, omit virtual sound/capture hardware, camera/USB passthrough, physical-device forwarding, shared folders/drives, and host microphone/camera access. Do not disable protection, UAC, Defender, privacy controls, or physical devices on the host or guest as a workaround. The VM must have no usable capture endpoints or cameras; the real production `doctor` command verifies this before destructive native cases.

Prefer a basic hypervisor console with no enhanced-session redirection. If using ordinary RDP, disable all device/audio/camera/drive/clipboard/printer forwarding in the connection before login, including both playback and recording. A dedicated `.rdp` file can contain:

```text
audiomode:i:2
audiocapturemode:i:0
camerastoredirect:s:
devicestoredirect:s:
usbdevicestoredirect:s:
redirectdrives:i:0
redirectclipboard:i:0
redirectprinters:i:0
redirectsmartcards:i:0
```

Also inspect the client's Local Resources settings and the hypervisor's device attachments; an RDP file alone does not prove hardware isolation. Do not use restricted-admin mode, an administrative logon with a filtered token, `runas`, a batch task, or a service to substitute for the required login. Leave the actual standard-account desktop logged in and unlocked for the whole job; do not disconnect the RDP session or log off during qualification.

### Guest prerequisites

Inside the disposable guest only, complete Windows updates and install:

- Git for Windows, supported x64 Python 3 (a real interpreter on `PATH`, not the Microsoft Store alias), and Windows PowerShell 5.1.
- Current stable Rust/Cargo for `x86_64-pc-windows-msvc`, with the account's ordinary usable toolchain/cache. The workflow uses the repository lockfile, not an elevated toolchain wrapper.
- Visual Studio 2022 Build Tools with **Desktop development with C++**, MSVC x64 tools, and the Windows SDK. Make their supported command-line build environment available to the standard account's runner terminal.
- The .NET SDK needed to execute the repository's local WiX tool. The workflow runs `dotnet tool restore`; the repository `.config/dotnet-tools.json` pins WiX 5.0.2, with roll-forward disabled. Do not install an unpinned global `wix` as a substitute.
- **GitHub route only:** a current official GitHub Actions x64 Windows runner archive, obtained using the repository's **Settings → Actions → Runners → New self-hosted runner** instructions and published checksum. Do not install it as a Windows service. The local route needs no runner registration.

COM, Media Foundation, Windows Installer, Task Scheduler observation, and the Windows audio enumeration prerequisites must work normally. **Before taking the clean snapshot**, the guest provisioning administrator must ensure `AudioEndpointBuilder` and `Audiosrv` are running. For example, in an elevated PowerShell terminal **inside the guest**, not on its hypervisor host:

```powershell
foreach ($name in @('AudioEndpointBuilder', 'Audiosrv')) {
    Set-Service -Name $name -StartupType Automatic
    Start-Service -Name $name -ErrorAction Stop
}
```

A running audio service does not authorize adding a microphone/audio device. The standard-user workflow only reads these services' status and fails if either is stopped; it never starts or reconfigures them. Resolve provisioning failures in the guest before dispatch, without broad ACL grants, UAC/Defender bypasses, or privileged runtime helpers.

Create a **fresh local standard account** in the guest. Its name must match `mcw_qa_` followed by 4–12 ASCII letters/digits; `mcw_qa_20261024` is one example. A reproducible guest-administrator setup is:

```powershell
$password = Read-Host 'New disposable guest-user password' -AsSecureString
New-LocalUser -Name 'mcw_qa_20261024' -Password $password -Description 'Disposable MCW qualification only'
$users = Get-LocalGroup -SID 'S-1-5-32-545'
Add-LocalGroupMember -Group $users -Member 'mcw_qa_20261024'
```

Do not add this user to Administrators, grant it an administrator token, or relax product/profile ACLs. Log out of the provisioning administrator and actually log in as the new account using the VM console or configured RDP connection. Complete first-login setup and let its own Explorer desktop load. Finish per-user Rust/Python setup under this account without elevation. Keep runner/toolchain directories outside Desktop, Documents, Downloads, Pictures, Music, and Videos; those folders must have no personal files (system `desktop.ini` files and empty default folders are not personal data). Do not install or run MCW in this preparation profile. There must be no existing MCW MSI/product registration, binaries on PATH, processes, settings/history, shortcut, autostart task/Run entry, or application identity.

Take a powered-off **clean disposable snapshot before qualification** (and before runner registration for the GitHub route), then resume and log in as the same standard account. Each attempt needs fresh guest/account state from that snapshot. A previous failed run's installed product or retained state must not be manually removed to make a retry appear clean.

## Local VM — no GitHub runner

**Implementation prepared; a full local standard-user guest run remains unexecuted.** A parser/negative-host safety PASS is not MSI, GUI, Cargo or portable qualification. Do not execute the following commands on the physical host, through PowerShell Direct/remoting, or as the guest administrator.

The local route requires explicit `-LocalDisposableVm`, `-AcknowledgeDisposableVm true`, an existing root actually owned by the standard account, and an independently reviewed full source SHA. It rejects GitHub-mode environment markers, physical/VBS hosts, administrative/restricted/batch tokens, wrong profile/desktop/Explorer, reparse ancestry and preexisting MCW products/data. Recognized guest metadata plus operator authorization is a safety check, **not cryptographic proof that a VM is disposable**. The operator must verify the new VM identity and absence of host shares/device forwarding before use.

First complete Windows installation and all legally required registration/license/activation steps yourself, inside the new guest only. Obtain media only from Microsoft and compare its exact edition/language/version SHA-256 before booting. A publicly reachable ISO URL does not waive Microsoft's stated registration or license conditions; no script here accepts them for you. Normal Windows 11 Hyper-V requirements, including generation 2, Secure Boot and virtual TPM, remain enabled.

Provision the prerequisites above with a separate guest administrator. Then actually sign in through Windows to the fresh local standard account and let that account's Explorer load on the unlocked console desktop. If later automated, a guest-local interactive-token task may run in that **already logged-in** session; a password/batch task, moved token, `runas` approximation or PowerShell Direct process is not a substitute. Do not switch or lock the host's current personal session to simulate this.

Run this in the standard account's **normal guest desktop terminal**, supplying the full candidate SHA reviewed outside the checkout:

```powershell
$ErrorActionPreference = 'Stop'
$revision = (Read-Host 'Independently reviewed full candidate SHA').Trim().ToLowerInvariant()
if ($revision -cnotmatch '^[0-9a-f]{40}$') { throw 'A full reviewed SHA is required.' }
$root = Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) ('MCWQualification-' + [DateTime]::UtcNow.ToString('yyyyMMdd-HHmmssfff'))
if (Test-Path -LiteralPath $root) { throw 'Fresh guest fixture root required.' }
New-Item -ItemType Directory -Path $root | Out-Null
$repository = Join-Path $root 'source'
git clone --no-checkout https://github.com/Roman-Cuisset/miccamwatch.git $repository
if ($LASTEXITCODE -ne 0) { throw 'Candidate clone failed.' }
git -C $repository checkout --detach $revision
if ($LASTEXITCODE -ne 0) { throw 'Reviewed candidate revision is unavailable.' }
& powershell.exe -NoLogo -NoProfile -NonInteractive -STA -File (Join-Path $repository 'installer/tests/windows-standard-user.ps1') -AcknowledgeDisposableVm true -LocalDisposableVm -FixtureRoot $root -ExpectedSourceCommit $revision
if ($LASTEXITCODE -ne 0) { throw 'Local qualification failed; preserve all existing state and evidence.' }
Get-Content -LiteralPath (Join-Path $root 'mcw-standard-user-proof/orchestrator-result.json') -Raw
```

The orchestrator checks that its own tracked checkout is unchanged and matches the supplied SHA, and that it lies inside the admitted guest root. It does not set `GITHUB_ACTIONS`, `RUNNER_ENVIRONMENT`, `RUNNER_TEMP`, `GITHUB_WORKSPACE`, `GITHUB_SHA`, or disguise the account/profile. Temporary consumer state is confined below this owned root; the existing isolated application-data behavior happens only after genuine context validation.

Artifacts remain **local**, under `$root/mcw-standard-user-proof`, with the genuine Cargo root at `$root/mcw-cargo-uninstall`. Completion still requires both the full native receipt and the **same installed MSI** application receipt, including `requireStandardUser`, `standardUserQualified` and `cleanupSafe`. Local receipt kind is `local-disposable-vm`; no GitHub run or runner provenance is invented. An early admission refusal may leave only terminal output and no proof directory, by design: save that output separately rather than relaxing admission.

On failure, retain the guest and proof paths; do not uninstall an uncertain product, force-stop an unknown process, delete restoration state, or rerun over failed evidence. Once evidence is retained, shut down **only the new test VM** through its authorized hypervisor controls. Do not automatically delete it. A stopped/expired UAC controller means host VM-management work must wait for renewed user authorization; no task/service/token workaround is permitted.


## 2. Register and run the real interactive runner

In the standard account's normal, unelevated desktop terminal, extract the official runner to a new directory owned by this account, for example `%USERPROFILE%\actions-runner`. Do not place its work directory under a personal-data folder, shared host path, junction, or another user's profile. The runner-created `_work\_temp` (`RUNNER_TEMP`) must be owned by this account; the orchestrator rejects a different owner or reparse-point ancestry.

An authorized repository operator obtains a **short-lived registration token** from GitHub's runner settings for this specific disposable runner. Do not put tokens/passwords in source, artifacts, documentation, persistent environment variables, command history, or chat. Enter the registration token at `config.cmd`'s prompt. From the extracted runner directory, replacing the repository/name examples with actual values:

```powershell
.\config.cmd --url https://github.com/OWNER/REPO --name mcw-standard-disposable-20261024a --labels mcw-standard-disposable --work _work --ephemeral
```

Retain the default `self-hosted`, `Windows`, and `X64` labels and add `mcw-standard-disposable`. The actual runner name must start with `mcw-standard-disposable-` and contain only letters, digits, dots, underscores, and hyphens after the prefix. Configure a runner group/repository restriction that permits only trusted qualification workflows. Do not run untrusted pull-request/fork code on this runner. When asked about running as a service, choose **No**; do not execute `svc.cmd`.

In that same unelevated, genuinely logged-in standard-user terminal, declare the explicit disposable-guest authority and start the foreground runner:

```powershell
$env:MCW_DISPOSABLE_STANDARD_VM = 'true'
.\run.cmd
```

This marker alone is insufficient: the workflow also checks real GitHub Actions values, the actual runner identity, virtual guest metadata, the local standard account/token, loaded profile, interactive session/input desktop, and same-user Explorer ownership. Do not set or override `GITHUB_ACTIONS`, `RUNNER_ENVIRONMENT`, `RUNNER_NAME`, `USERNAME`, `USERPROFILE`, `APPDATA`, or `LOCALAPPDATA`. GitHub supplies its real runner values during the job. `TEMP`/`TMP` are isolated only after admission, beneath the actual owned `RUNNER_TEMP`; profile/AppData are never redirected to pass admission.

The guest/runner must appear **online and idle with all four required labels** before dispatch. If there is no actual suitable runner, stop; a queued job is not qualification evidence. The ephemeral runner handles one job and unregisters afterward. It must not be reused for an additional attempt.

## 3. Explicit manual dispatch

The existing `native-uninstall.yml` workflow is already registered on the default branch. Its candidate-only manual subset `windows-standard` calls the new workflow from the **same selected candidate revision**, so no `main` adoption is needed to register the new file. The new file accepts that trusted reusable call and a direct manual dispatch if separately registered; its native job requires the original event to be `workflow_dispatch`. It has `contents: read` and exact runner labels `[self-hosted, Windows, X64, mcw-standard-disposable]`. It does not qualify through a push, pull request, schedule, or release event.

Once a suitable VM is actually online, select the reviewed trusted qualification ref explicitly. An authorized operator can use **Native installation-scoped uninstall → Run workflow**, select `release/0.21.0-candidate`, choose `windows-standard`, and check the disposable-VM acknowledgement. Equivalent CLI:

```text
gh workflow run native-uninstall.yml --repo Roman-Cuisset/miccamwatch --ref release/0.21.0-candidate -f platform=windows-standard -f acknowledge_disposable_vm=true
```

Record the resulting run URL, selected commit, runner name, guest snapshot identity, and evidence artifacts. Review the selected commit before running; a read-only `GITHUB_TOKEN` does not sandbox self-hosted workflow code. The acknowledgement defaults to **false**. Omitting it or submitting false **fails the script/job**; it is never a green skip. There is no hosted fallback if the custom runner is unavailable.

For the separate **read-only** parser/C# compilation/context-admission smoke on a normal hosted runner, use `-f platform=windows-context` instead. It runs no MCW binary, MSI, uninstall or device action. Its receipt must remain `qualificationStatus=not_executed` even when the safety smoke passes. This is not a substitute for `windows-standard`.

## 4. Executed cases and evidence

Before building, the script dot-sources [the shared admission helper](../installer/tests/windows-fixture-context.ps1), observes the actual standard-user context, and calls `Assert-McwDisposableFixtureHost -RequireStandardUser` with the explicitly chosen route. It requires `dedicated-standard-vm` for GitHub or `local-disposable-vm` for local execution. It rejects preexisting MCW products/processes/autostart/data and personal-folder files, and checks already-running audio prerequisites. It then:

1. Runs real `cargo build --release --locked --features windows-tray --bin mcw --bin mcw-tray` as the standard account.
2. Runs that actual `mcw.exe doctor --lang en --json --no-color`. Admission requires exactly one `microphone_devices` check reporting `warning` / `0 capture device(s) detected` and one `camera_devices` check reporting `warning` / `0 camera device(s) detected`. Enumeration errors, unknown schema, or any devices fail before destructive cases. Zero active capture endpoints is the production doctor's scope; provisioning must also prevent inactive/forwarded hardware from appearing later.
3. Restores pinned local WiX, resolves the locked package version, and builds the genuine per-user MSI from `installer/miccamwatch.wxs` with both actual binaries.
4. Runs real `cargo install --locked --offline --path <repository> --root <actual RUNNER_TEMP>/mcw-cargo-uninstall --features windows-tray` as the same standard account. The preceding real build populates the normal Cargo cache; no copied executable masquerades as a Cargo install.
5. Rechecks the real interactive standard-user context and calls `native_uninstall.py` with its unchanged `--binary`, `--repository`, `--cargo-install`, `--msi`, and `--proof` arguments. It executes the existing real native contracts, including the actual per-user MSI install, installed Start Menu application/reopen/concurrent-owner qualification, exact MSI manager delegation/removal, data preservation, and genuine Cargo uninstall cases.
6. Requires native `result.json` status `passed` **and** installed MSI `private-msi-application/result.json` status `passed`, `standardUserQualified = true`, `requireStandardUser = true`, and `cleanupSafe = true` before printing a full PASS. A successful binary build, a doctor report, or a partial application result is not sufficient.

The workflow always attempts artifact upload, including hidden files, with seven-day retention. Artifact name: `windows-standard-user-<run_id>-<run_attempt>`. Its source locations are:

- `<RUNNER_TEMP>/mcw-standard-user-proof/orchestrator-result.json`: top-level completion/failure authority.
- `private-admission-context.json`, `private-standard-user-context.json`, `private-host-context.json`, and `private-context-before-native.json`: actual admission/context observations, when reached.
- `private-*-command.json`, `private-*-stdout.log`, `private-*-stderr.log`, `private-orchestrator-error.log`: exact executable/argument arrays, working directories, timestamps, exit codes, private output, and precise orchestration failure.
- `result.json`, native command evidence/private logs, `private-msi-application/`, and MSI logs: existing native fixture reports and installed application evidence.
- `temporary-state/`: the real fixture `TEMP`/`TMP`, including retained sandboxes if native cases fail.
- `mcw-standard-user-fixture.msi`: the actual package used for qualification.
- `<RUNNER_TEMP>/mcw-cargo-uninstall`: the dedicated genuine Cargo install/root and any retained failure state.

These are diagnostic artifacts, not release assets. **A `private-*` filename is not access control:** artifacts in a public GitHub repository can be downloadable by other users with repository read access. Use only the fresh disposable identity and synthetic fixture data, never personal data or credentials; download evidence before its seven-day expiry. The script does not dump GitHub registration tokens or intentionally archive the runner installation/credential files. If GitHub cancels the job, the runner is lost, or upload fails, artifacts may be incomplete: retain the disposable guest disk/snapshot and recover the existing proof paths out of band without claiming PASS. If even `RUNNER_TEMP` is invalid/unowned, the script refuses to write there and upload may contain no evidence; that is an admission failure, not permission to relax ownership.

## Stop conditions and disposal

Stop on any failed admission, noninteractive/session-zero context, administrator/restricted token, missing Explorer/input desktop, preexisting product/data, service prerequisite failure, device detection/enumeration failure, native timeout/failure, MSI cleanup uncertainty, or missing/mismatched final proof. Never enable an audio service, terminate an unknown product/process, uninstall an existing installation, clear autostart, remove locks/state, or change camera/microphone/protection settings to continue a failed run.

There is no script-level failure cleanup: do not add an `always()` uninstall/process-kill/reset step. The existing native cases perform their own cooperative cleanup only when their safety proofs authorize it; failed state stays available for diagnosis. After evidence is retained and investigated, stop the interactive runner and discard/revert the whole disposable guest through its hypervisor. Do not run cleanup against the host. A new attempt starts from the clean pre-registration snapshot and fresh ephemeral registration, never from manually repaired failure state.

### Shared implementation dependencies

The orchestrator depends on shared `Get-McwStandardUserContext`, `Assert-McwStandardUserContext`, `Assert-McwFixtureRoot`, and `Assert-McwDisposableFixtureHost`, returning `kind`, `standardUserContext` and `fixtureRoot`. The native and installed application consumers propagate explicit local flags/root without changing the runner route. Runner mode confines paths to real `RUNNER_TEMP`; local mode confines source/payload/Cargo/proof paths to the admitted guest root. Neither this implementation nor the read-only safety smoke proves that a guest exists or that full Windows standard-user qualification has passed.
