param(
    [switch]$ActiveTray,
    [switch]$CurrentRepairOnly,
    [ValidateSet('companion','cli')][string]$Frontend = 'companion'
)

$ErrorActionPreference = 'Stop'
# This fixture must never start products or native probes on a developer/user PC.
if ($env:GITHUB_ACTIONS -cne 'true' -or $env:RUNNER_ENVIRONMENT -cne 'github-hosted' -or $env:RUNNER_OS -cne 'Windows') {
    throw 'Public update proof is restricted to disposable GitHub-hosted Windows runners'
}
$proof = Join-Path $env:RUNNER_TEMP 'public-update-proof'
[void](New-Item -ItemType Directory $proof)
$report = [ordered]@{
    status = 'failed'; from = $env:UPGRADE_FROM; to = $env:RELEASE_VERSION
    active_tray_requested = [bool]$ActiveTray; tray_started = $false
    frontend = $Frontend
    launch_context_tested = $false
    policy_semantics_tested = $false
    mode = $(if ($CurrentRepairOnly) { 'current-repair-only' } else { 'upgrade-and-independent-repair' })
    requested_phases = $(if ($CurrentRepairOnly) { @('repair','noop') } else { @('upgrade','noop','repair') })
    skipped_phases = @(if ($CurrentRepairOnly) {
        @{ name = 'upgrade'; reason = 'Current-repair-only mode excludes legacy-client upgrade; no old-client upgrade coverage is claimed' }
    })
    device_control_tested = $false; user_pc_touched = $false
    phases = @(); errors = @(); cleanup = @()
}
$captures = [Collections.Generic.List[object]]::new()
$owners = [Collections.Generic.List[object]]::new()
$settings = $null
$policyFile = $null
function Save-Report {
    $report | ConvertTo-Json -Depth 12 | Set-Content (Join-Path $proof 'result.json')
}
Save-Report
try {
    if ($CurrentRepairOnly -and -not $ActiveTray) { throw 'Current-repair-only requires an active selected frontend and verified older companion' }
    if ($Frontend -eq 'cli' -and (-not $CurrentRepairOnly -or -not $ActiveTray)) {
        throw 'CLI frontend qualification requires current-repair-only and active-tray; legacy-client coverage is unchanged'
    }
    . (Join-Path $PSScriptRoot 'windows-verified-package.ps1')
    $repo = 'Roman-Cuisset/miccamwatch'
    $env:APPDATA = Join-Path $proof 'appdata'
    $env:LOCALAPPDATA = Join-Path $proof 'localappdata'
    [void](New-Item -ItemType Directory $env:APPDATA,$env:LOCALAPPDATA)
    $settingsDirectory = Join-Path $env:APPDATA 'MicCamWatch'
    [void](New-Item -ItemType Directory $settingsDirectory)
    $settings = Join-Path $settingsDirectory 'settings.toml'
    @'
notifications_enabled = false
sound_enabled = false
show_ready = false
history_enabled = false
mute_on_lock = false
block_camera_on_lock = false
restore_on_unlock = false
'@ | Set-Content -Encoding utf8NoBOM $settings
    $settingsHash = (Get-FileHash $settings).Hash
    Copy-Item $settings (Join-Path $proof 'preferences-before.toml')
    # Class inspection and process handles, not tray status text or basename kills.
    Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text;
public static class PublicUpdateNative {
    public delegate bool EnumProc(IntPtr window, IntPtr parameter);
    [DllImport("user32.dll", SetLastError=true)] static extern bool EnumWindows(EnumProc callback, IntPtr parameter);
    [DllImport("user32.dll", CharSet=CharSet.Unicode, SetLastError=true)] static extern int GetClassNameW(IntPtr window, StringBuilder name, int count);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr window, out uint pid);
    [DllImport("user32.dll", SetLastError=true)] public static extern IntPtr SendMessageTimeoutW(IntPtr window, uint message, UIntPtr wparam, IntPtr lparam, uint flags, uint timeout, out UIntPtr result);
    [DllImport("user32.dll", SetLastError=true)] public static extern bool PostMessageW(IntPtr window, uint message, UIntPtr wparam, IntPtr lparam);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetMenuStringW(IntPtr menu, uint item, StringBuilder text, int count, uint flags);
    [DllImport("user32.dll")] static extern int GetMenuItemCount(IntPtr menu);
    [DllImport("user32.dll")] static extern IntPtr GetSubMenu(IntPtr menu, int position);
    [DllImport("user32.dll")] static extern uint GetMenuState(IntPtr menu, uint item, uint flags);
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)] static extern bool QueryFullProcessImageNameW(IntPtr process, uint flags, StringBuilder path, ref uint size);
    [DllImport("kernel32.dll", SetLastError=true)] public static extern uint WaitForSingleObject(IntPtr process, uint timeout);
    [DllImport("kernel32.dll", SetLastError=true)] public static extern bool TerminateProcess(IntPtr process, uint code);
    public static IntPtr[] Windows() {
        var windows = new List<IntPtr>();
        int failure = 0;
        if (!EnumWindows((window, parameter) => {
            var name = new StringBuilder(256);
            if (GetClassNameW(window, name, name.Capacity) == 0) {
                failure = Marshal.GetLastWin32Error();
                if (failure == 1400) { failure = 0; return true; }
                return false;
            }
            if (name.ToString() == "MicCamWatchTrayClass") windows.Add(window);
            return true;
        }, IntPtr.Zero)) throw new System.ComponentModel.Win32Exception(failure == 0 ? Marshal.GetLastWin32Error() : failure);
        return windows.ToArray();
    }
    public static string Image(IntPtr process) {
        var path = new StringBuilder(32768); uint size = (uint)path.Capacity;
        if (!QueryFullProcessImageNameW(process, 0, path, ref size)) throw new System.ComponentModel.Win32Exception();
        return System.IO.Path.GetFullPath(path.ToString());
    }
    // Read the actual native popup of the authenticated process, without selecting
    // any command. MN_GETHMENU retrieves its menu; no remote memory is accessed.
    public static IntPtr PopupWindow(uint expectedPid) {
        IntPtr popup = IntPtr.Zero;
        bool multiple = false;
        if (!EnumWindows((window, parameter) => {
            var name = new StringBuilder(256);
            GetClassNameW(window, name, name.Capacity);
            uint pid; GetWindowThreadProcessId(window, out pid);
            if (pid == expectedPid && name.ToString() == "#32768") {
                if (popup != IntPtr.Zero) multiple = true;
                popup = window;
            }
            return true;
        }, IntPtr.Zero)) throw new System.ComponentModel.Win32Exception();
        if (multiple) throw new InvalidOperationException("Multiple owned native popups");
        return popup;
    }
    public static string[] ProfileMenu(uint expectedPid) {
        var popup = PopupWindow(expectedPid);
        if (popup == IntPtr.Zero) return null;
        UIntPtr result;
        if (SendMessageTimeoutW(popup, 0x01e1, UIntPtr.Zero, IntPtr.Zero, 3, 1000, out result) == IntPtr.Zero)
            throw new InvalidOperationException("Owned native menu query timed out");
        var menu = new IntPtr(unchecked((long)result.ToUInt64()));
        if (menu == IntPtr.Zero) throw new InvalidOperationException("Owned popup has no menu");
        for (int index = 0; index < GetMenuItemCount(menu); index++) {
            var profiles = GetSubMenu(menu, index);
            if (profiles == IntPtr.Zero) continue;
            var label = new StringBuilder(1024);
            if (GetMenuStringW(menu, (uint)index, label, label.Capacity, 0x400) == 0)
                throw new InvalidOperationException("Profile label unavailable");
            var observed = new List<string> { label.ToString() };
            foreach (uint command in new uint[] { 109, 110, 111, 112 }) {
                uint flags = GetMenuState(profiles, command, 0);
                if (flags == uint.MaxValue || (flags & 3) != 3)
                    throw new InvalidOperationException("Explicit policy profile choice is not disabled");
                var choice = new StringBuilder(1024);
                if (GetMenuStringW(profiles, command, choice, choice.Capacity, 0) == 0)
                    throw new InvalidOperationException("Profile choice unavailable");
                observed.Add(choice.ToString());
            }
            return observed.ToArray();
        }
        throw new InvalidOperationException("Owned menu has no profile submenu");
    }
}
// Event readers retain partial logs even when a published restart inherits pipes.
// No PowerShell callback runs on a thread without a runspace.
public sealed class PublicUpdateCapture : IDisposable {
    public readonly Process Process;
    readonly object gate = new object();
    readonly StringBuilder stdout = new StringBuilder(), stderr = new StringBuilder();
    public volatile bool StdoutEof, StderrEof;
    public PublicUpdateCapture(string image, string arguments) : this(image, arguments, null) {}
    public PublicUpdateCapture(string image, string arguments, string directory) {
        Process = new Process();
        Process.StartInfo = new ProcessStartInfo(image, arguments) {
            UseShellExecute = false, CreateNoWindow = true,
            RedirectStandardOutput = true, RedirectStandardError = true
        };
        if (directory != null) Process.StartInfo.WorkingDirectory = directory;
        Process.OutputDataReceived += (sender, value) => { lock(gate) { if (value.Data == null) StdoutEof = true; else stdout.AppendLine(value.Data); } };
        Process.ErrorDataReceived += (sender, value) => { lock(gate) { if (value.Data == null) StderrEof = true; else stderr.AppendLine(value.Data); } };
        if (!Process.Start()) throw new InvalidOperationException("Process did not start");
        var retained = Process.Handle;
        Process.BeginOutputReadLine(); Process.BeginErrorReadLine();
    }
    public string Stdout { get { lock(gate) return stdout.ToString(); } }
    public string Stderr { get { lock(gate) return stderr.ToString(); } }
    public void Dispose() { Process.Dispose(); }
}
'@
    function Test-MCWProcesses {
        $found = $false
        foreach ($name in @('mcw','mcw-tray')) {
            foreach ($process in [Diagnostics.Process]::GetProcessesByName($name)) {
                $found = $true
                $process.Dispose()
            }
        }
        return $found
    }
    if (@([PublicUpdateNative]::Windows()).Count -ne 0 -or (Test-MCWProcesses)) {
        throw 'Runner is not isolated from an existing MCW process'
    }
    function Assert-Preferences {
        if ((Get-FileHash $settings).Hash -cne $settingsHash) { throw 'Public operation changed private preferences' }
        if ($null -ne $policyFile -and (Get-FileHash $policyFile).Hash -cne $policyHash) { throw 'Public operation changed explicit policy' }
    }
    function Get-PairHashes([string]$Install) {
        $hashes = [ordered]@{}
        foreach ($name in @('mcw.exe','mcw-tray.exe')) {
            $path = Join-Path $Install $name
            $hashes[$name] = if (Test-Path $path) { (Get-FileHash $path).Hash } else { $null }
        }
        return $hashes
    }
    function Save-Capture($Capture, [string]$Name) {
        $Capture.Stdout | Set-Content (Join-Path $proof "$Name.stdout.txt")
        $Capture.Stderr | Set-Content (Join-Path $proof "$Name.stderr.txt")
    }
    function Invoke-Captured([string]$Image, [string]$Arguments, [string]$Name, [int]$Seconds = 180) {
        $capture = [PublicUpdateCapture]::new($Image, $Arguments)
        $captures.Add([pscustomobject]@{ Capture = $capture; Name = $Name; Image = [IO.Path]::GetFullPath($Image); Role = 'command' })
        $actualImage = [PublicUpdateNative]::Image($capture.Process.Handle)
        if (-not [string]::Equals($actualImage, [IO.Path]::GetFullPath($Image), [StringComparison]::OrdinalIgnoreCase)) { throw 'Captured child image differs from exact executable' }
        $exited = $capture.Process.WaitForExit($Seconds * 1000)
        $deadline = [DateTime]::UtcNow.AddSeconds(10)
        while ($exited -and -not ($capture.StdoutEof -and $capture.StderrEof) -and [DateTime]::UtcNow -lt $deadline) {
            Start-Sleep -Milliseconds 50
        }
        Save-Capture $capture $Name
        $evidence = [ordered]@{
            pid = $capture.Process.Id; image = $actualImage
            process_exit_deadline_seconds = $Seconds; eof_deadline_seconds_after_exit = 10
            process_exited_within_deadline = $exited
            exit_code = $(if ($exited) { $capture.Process.ExitCode } else { $null })
            stdout_eof_at_deadline = $capture.StdoutEof; stderr_eof_at_deadline = $capture.StderrEof
        }
        $evidence | ConvertTo-Json | Set-Content (Join-Path $proof "$Name.process.json")
        return [pscustomobject]@{ Capture = $capture; Evidence = $evidence }
    }
    function Assert-Version([string]$Image, [string]$Tag, [string]$Name) {
        $run = Invoke-Captured $Image '--version' $Name 15
        if (-not $run.Evidence.process_exited_within_deadline -or $run.Evidence.exit_code -ne 0 -or -not $run.Evidence.stdout_eof_at_deadline -or -not $run.Evidence.stderr_eof_at_deadline -or $run.Capture.Stdout.Trim() -cne ('mcw ' + $Tag.Substring(1))) {
            throw "Public version probe failed: $Name"
        }
    }
    function Get-Owner([string]$Image) {
        $windows = @([PublicUpdateNative]::Windows())
        if ($windows.Count -eq 0) { return $null }
        if ($windows.Count -ne 1) { throw 'Multiple tray HWNDs; ownership is not unique' }
        $window = $windows[0]
        [uint32]$ownerId = 0
        [void][PublicUpdateNative]::GetWindowThreadProcessId($window, [ref]$ownerId)
        $process = [Diagnostics.Process]::GetProcessById($ownerId)
        # Force a retained kernel handle BEFORE revalidating HWND/PID/image.
        $handle = $process.Handle
        $actual = [PublicUpdateNative]::Image($handle)
        [uint32]$again = 0
        [void][PublicUpdateNative]::GetWindowThreadProcessId($window, [ref]$again)
        if ($again -ne $ownerId -or [PublicUpdateNative]::WaitForSingleObject($handle, 0) -ne 258 -or -not [string]::Equals($actual, [IO.Path]::GetFullPath($Image), [StringComparison]::OrdinalIgnoreCase)) {
            $process.Dispose()
            throw 'Tray HWND/PID/exact image ownership mismatch; no stop sent'
        }
        $owner = [pscustomobject]@{ Process = $process; Handle = $handle; Pid = $ownerId; Window = $window; Image = $actual }
        $owners.Add($owner)
        return $owner
    }
    function Owner-Evidence($Owner) {
        if ($null -eq $Owner) { return $null }
        return @{ pid = $Owner.Pid; hwnd = $Owner.Window.ToInt64(); image = $Owner.Image; alive = ([PublicUpdateNative]::WaitForSingleObject($Owner.Handle, 0) -eq 258) }
    }
    function Wait-Owner([string]$Image) {
        $deadline = [DateTime]::UtcNow.AddSeconds(20)
        do {
            $owner = Get-Owner $Image
            if ($null -ne $owner) { return $owner }
            Start-Sleep -Milliseconds 100
        } while ([DateTime]::UtcNow -lt $deadline)
        throw 'Exact-image tray did not reach native HWND readiness'
    }
    function Assert-OneTray($Owner) {
        $matching = 0
        foreach ($process in [Diagnostics.Process]::GetProcessesByName([IO.Path]::GetFileNameWithoutExtension($Owner.Image))) {
            try {
                $image = [PublicUpdateNative]::Image($process.Handle)
                if ([string]::Equals($image, $Owner.Image, [StringComparison]::OrdinalIgnoreCase)) { $matching++ }
            } finally { $process.Dispose() }
        }
        if ($matching -ne 1) { throw "Expected one exact-image tray process, found $matching" }
    }
    function Assert-LaunchContext($Owner) {
        # Observe effective language and explicit-policy behavior in the real UI;
        # neither a copied argv string nor unchanged TOML alone qualifies this.
        $current = Get-Owner $Owner.Image
        if ($null -eq $current -or $current.Pid -ne $Owner.Pid -or $current.Window -ne $Owner.Window) { throw 'Launch-context owner changed' }
        if ([PublicUpdateNative]::PopupWindow($Owner.Pid) -ne [IntPtr]::Zero) { throw 'Unexpected existing owned popup' }
        if (-not [PublicUpdateNative]::PostMessageW($Owner.Window, 0x8001, [UIntPtr]::Zero, [IntPtr]0x205)) { throw 'Could not open owned tray menu' }
        try {
            $deadline = [DateTime]::UtcNow.AddSeconds(5)
            do {
                $observed = [PublicUpdateNative]::ProfileMenu($Owner.Pid)
                if ($null -ne $observed) { break }
                Start-Sleep -Milliseconds 50
            } while ([DateTime]::UtcNow -lt $deadline)
            $expected = @('Profil — Défini par --config','Équilibré','Privé','Réunion','Développement')
            if ($null -eq $observed -or ($observed | ConvertTo-Json -Compress) -cne ($expected | ConvertTo-Json -Compress)) {
                throw 'Effective French language / explicit-policy disabled profile menu differs'
            }
            return @{ profile_label = $observed[0]; choices = @($observed[1..4]); all_choices_disabled = $true; language = 'fr'; explicit_policy = $true }
        } finally {
            [UIntPtr]$reply = [UIntPtr]::Zero
            if ([PublicUpdateNative]::SendMessageTimeoutW($Owner.Window, 0x1f, [UIntPtr]::Zero, [IntPtr]::Zero, 3, 5000, [ref]$reply) -eq [IntPtr]::Zero) {
                throw 'Owned menu cancellation timed out'
            }
            $deadline = [DateTime]::UtcNow.AddSeconds(5)
            while ([PublicUpdateNative]::PopupWindow($Owner.Pid) -ne [IntPtr]::Zero) {
                if ([DateTime]::UtcNow -ge $deadline) { throw 'Owned menu remained open after cancellation' }
                Start-Sleep -Milliseconds 50
            }
        }
    }
    function Assert-LiveEof($Run, $Owner) {
        if (-not $Run.Evidence.stdout_eof_at_deadline -or -not $Run.Evidence.stderr_eof_at_deadline) { throw 'Captured streams missed EOF deadline' }
        $current = Get-Owner $Owner.Image
        if ($null -eq $current -or $current.Pid -ne $Owner.Pid -or $current.Window -ne $Owner.Window -or [PublicUpdateNative]::WaitForSingleObject($Owner.Handle, 0) -ne 258) {
            throw 'EOF must be observed with the exact successor still alive, before any cleanup'
        }
        return @{ owner = (Owner-Evidence $Owner); stdout_eof = $Run.Capture.StdoutEof; stderr_eof = $Run.Capture.StderrEof; before_cleanup = $true }
    }
    function Retain-ImageProcesses([string]$Image) {
        # Retain exact private-install children even if failed startup never made
        # a HWND. These handles are cleanup authority, never a readiness signal.
        foreach ($process in [Diagnostics.Process]::GetProcessesByName([IO.Path]::GetFileNameWithoutExtension($Image))) {
            $keep = $false
            try {
                $handle = $process.Handle
                $actual = [PublicUpdateNative]::Image($handle)
                if ([string]::Equals($actual, [IO.Path]::GetFullPath($Image), [StringComparison]::OrdinalIgnoreCase) -and @($owners | Where-Object { $_.Pid -eq $process.Id -and [PublicUpdateNative]::WaitForSingleObject($_.Handle, 0) -eq 258 }).Count -eq 0) {
                    $owners.Add([pscustomobject]@{ Process = $process; Handle = $handle; Pid = $process.Id; Window = [IntPtr]::Zero; Image = $actual })
                    $keep = $true
                }
            } finally { if (-not $keep) { $process.Dispose() } }
        }
    }
    function Stop-Owner($Owner) {
        if ([PublicUpdateNative]::WaitForSingleObject($Owner.Handle, 0) -eq 0) { return }
        [uint32]$currentId = 0
        [void][PublicUpdateNative]::GetWindowThreadProcessId($Owner.Window, [ref]$currentId)
        if ($currentId -ne $Owner.Pid -or -not [string]::Equals([PublicUpdateNative]::Image($Owner.Handle), $Owner.Image, [StringComparison]::OrdinalIgnoreCase)) {
            throw 'Cleanup ownership changed; no window message sent'
        }
        [UIntPtr]$reply = [UIntPtr]::Zero
        $sent = [PublicUpdateNative]::SendMessageTimeoutW($Owner.Window, 0x10, [UIntPtr]::Zero, [IntPtr]::Zero, 3, 5000, [ref]$reply)
        if ($sent -eq [IntPtr]::Zero) { throw 'Owned cooperative WM_CLOSE timed out' }
        if ([PublicUpdateNative]::WaitForSingleObject($Owner.Handle, 15000) -ne 0) { throw 'Owned tray did not exit after cooperative WM_CLOSE' }
        $report.cleanup += @{ pid = $Owner.Pid; image = $Owner.Image; cooperative_exit = $true }
    }
    $old = Get-VerifiedWindowsPackage $repo $env:UPGRADE_FROM $env:UPGRADE_SOURCE (Join-Path $proof 'old')
    $new = Get-VerifiedWindowsPackage $repo $env:RELEASE_VERSION $env:RELEASE_SOURCE (Join-Path $proof 'new')
    if ($CurrentRepairOnly -and [version]($env:UPGRADE_FROM.Substring(1)) -ge [version]($env:RELEASE_VERSION.Substring(1))) {
        throw 'Current-repair-only requires a verified upgrade-from release older than the current release'
    }
    $latest = gh api "repos/$repo/releases/latest" --jq .tag_name
    if ($LASTEXITCODE -ne 0 -or $latest -cne $env:RELEASE_VERSION) { throw 'Selected release must be public stable latest' }
    $report.verified_public_pairs = @{ older = (Get-PairHashes $old); current = (Get-PairHashes $new) }
    if ($Frontend -eq 'cli') {
        $launchDirectory = Join-Path $proof 'cli launch context'
        $policyDirectory = Join-Path $launchDirectory 'policy files'
        [void](New-Item -ItemType Directory $policyDirectory -Force)
        $policyFile = Join-Path $policyDirectory 'custom policy.toml'
        @'
profile = "conservative"
language = "de"
action = "alert"
'@ | Set-Content -Encoding utf8NoBOM $policyFile
        $policyHash = (Get-FileHash $policyFile).Hash
        Copy-Item $policyFile (Join-Path $proof 'explicit-policy-before.toml')
        $frontendArguments = '--config "policy files/custom policy.toml" --lang fr tray'
        $report.launch_context = @{
            arguments = $frontendArguments; working_directory = $launchDirectory
            updater_working_directory = [Environment]::CurrentDirectory
            policy_path = $policyFile; policy_sha256 = $policyHash
            policy_language = 'de'; explicit_language_override = 'fr'
            coverage = 'Actual localized native profile menu and disabled explicit-policy controls; policy classification semantics are not exercised'
        }
    }
    Assert-Version (Join-Path $old 'mcw.exe') $env:UPGRADE_FROM 'old-version'
    Assert-Version (Join-Path $new 'mcw.exe') $env:RELEASE_VERSION 'new-version'
    # Legacy clients do not consume GH_TOKEN. Wait only BEFORE any updater attempt.
    $quotaHeaders = @{ 'User-Agent' = 'MicCamWatch-public-migration-proof'; 'Cache-Control' = 'no-cache' }
    $quota = Invoke-RestMethod 'https://api.github.com/rate_limit' -Headers $quotaHeaders
    if ($quota.resources.core.remaining -lt 4) {
        $delay = [Math]::Max(1, $quota.resources.core.reset - [DateTimeOffset]::UtcNow.ToUnixTimeSeconds() + 2)
        if ($delay -gt 600) { throw "Anonymous API prerequisite unavailable for $delay seconds; no update attempted" }
        Start-Sleep -Seconds $delay
        $quota = Invoke-RestMethod 'https://api.github.com/rate_limit' -Headers $quotaHeaders
    }
    if ($quota.resources.core.remaining -lt 4) { throw 'Anonymous API prerequisite unavailable; no update attempted' }
    # Opt-in current-client qualification reuses mixed-pair repair, then no-op
    # with its live successor; it does not rerun the captured legacy EOF defect.
    $scenarios = if ($CurrentRepairOnly) { @('repair') } else { @('upgrade','repair') }
    foreach ($scenario in $scenarios) {
        $install = Join-Path $proof "$scenario-install"
        [void](New-Item -ItemType Directory $install)
        if ($scenario -eq 'upgrade') {
            Copy-Item (Join-Path $old '*.exe') $install
        } else {
            Copy-Item (Join-Path $new 'mcw.exe') $install
            # Active repair is a real mixed public pair. Never move/delete a live image.
            if ($ActiveTray) { Copy-Item (Join-Path $old 'mcw-tray.exe') $install }
        }
        $cli = Join-Path $install 'mcw.exe'
        $tray = Join-Path $install 'mcw-tray.exe'
        $frontendImage = if ($Frontend -eq 'cli') { $cli } else { $tray }
        $original = $null
        try {
            if ($ActiveTray) {
                $trayCapture = if ($Frontend -eq 'cli') {
                    [PublicUpdateCapture]::new($frontendImage, $frontendArguments, $launchDirectory)
                } else {
                    [PublicUpdateCapture]::new($frontendImage, '')
                }
                $started = $trayCapture.Process
                $captures.Add([pscustomobject]@{ Capture = $trayCapture; Name = "$scenario-original-tray"; Image = $frontendImage; Role = 'frontend' })
                $report.tray_started = $true
                $original = Wait-Owner $frontendImage
                if ($original.Pid -ne $started.Id) { throw 'Ready tray is not the exact child started by this fixture' }
                Assert-OneTray $original
                $originalContext = if ($Frontend -eq 'cli') { Assert-LaunchContext $original } else { $null }
            }
            $before = Get-PairHashes $install
            $phase = [ordered]@{
                name = $scenario; status = 'failed'; before = $before
                original_tray = (Owner-Evidence $original); core_success = $false
                frontend = $Frontend; original_effective_context = $originalContext
                refusal_preserves_pair_and_original = $false
            }
            $report.phases += $phase
            if ($Frontend -eq 'cli') {
                if ($before['mcw.exe'] -cne (Get-FileHash (Join-Path $new 'mcw.exe')).Hash -or $before['mcw-tray.exe'] -cne (Get-FileHash (Join-Path $old 'mcw-tray.exe')).Hash) {
                    throw 'CLI-hosted repair must use the real current caller/frontend and verified older companion'
                }
                $phase.current_cli_hosts_original_and_calls_update = $true
                $phase.older_companion_requires_repair = $true
            }
            $run = Invoke-Captured $cli 'update' $scenario
            $phase.process = $run.Evidence
            $phase.after = Get-PairHashes $install
            Assert-Preferences
            $phase.preferences_unchanged = $true
            if (-not $run.Evidence.process_exited_within_deadline) { throw "$scenario updater process did not exit within deadline" }
            if ($run.Evidence.exit_code -ne 0) {
                if (($before | ConvertTo-Json -Compress) -cne ($phase.after | ConvertTo-Json -Compress)) { throw "$scenario refusal changed installed bytes" }
                if ($ActiveTray) {
                    $still = Get-Owner $frontendImage
                    if ($null -eq $still -or $still.Pid -ne $original.Pid -or [PublicUpdateNative]::WaitForSingleObject($original.Handle, 0) -ne 258) { throw "$scenario refusal lost the original tray" }
                    Assert-OneTray $still
                    $phase.tray_after = Owner-Evidence $still
                }
                $phase.refusal_preserves_pair_and_original = $true
                throw "$scenario public updater refused (exit $($run.Evidence.exit_code)); active success is NOT qualified"
            }
            Assert-VerifiedWindowsPair $install $new
            $phase.public_pair_hashes_exact = $true
            if ($ActiveTray) {
                $phase.old_child_gone = ([PublicUpdateNative]::WaitForSingleObject($original.Handle, 15000) -eq 0)
                if (-not $phase.old_child_gone) { throw "$scenario old tray child did not exit" }
                $successor = Wait-Owner $frontendImage
                if ($successor.Pid -eq $original.Pid) { throw "$scenario did not create a distinct successor" }
                Assert-OneTray $successor
                $phase.tray_after = Owner-Evidence $successor
                if ($Frontend -eq 'cli') {
                    $phase.effective_context_after = Assert-LaunchContext $successor
                    $phase.effective_context_unchanged = $true
                    $report.launch_context_tested = $true
                }
            } elseif (@([PublicUpdateNative]::Windows()).Count -ne 0 -or (Test-MCWProcesses)) {
                throw "$scenario unexpectedly left a resident frontend in no-tray mode"
            }
            Assert-Preferences
            $phase.core_success = $true
            if (-not $run.Evidence.stdout_eof_at_deadline -or -not $run.Evidence.stderr_eof_at_deadline) {
                throw "$scenario core transaction succeeded but captured stdout/stderr EOF failed; no retry or manufactured EOF"
            }
            if ($ActiveTray) { $phase.eof_with_live_successor = Assert-LiveEof $run $successor }
            $phase.status = 'passed'
            if ($scenario -eq 'upgrade' -or $CurrentRepairOnly) {
                $noop = [ordered]@{ name = 'noop'; status = 'failed'; core_success = $false; before = (Get-PairHashes $install) }
                $report.phases += $noop
                $existing = if ($ActiveTray) { Get-Owner $frontendImage } else { $null }
                if ($ActiveTray -and ($null -eq $existing -or $existing.Pid -ne $successor.Pid -or $existing.Window -ne $successor.Window -or [PublicUpdateNative]::WaitForSingleObject($successor.Handle, 0) -ne 258)) {
                    throw 'Same-version no-op must begin with the exact live update successor'
                }
                $noop.original_tray = Owner-Evidence $existing
                $run = Invoke-Captured $cli 'update' 'noop'
                $noop.process = $run.Evidence
                $noop.after = Get-PairHashes $install
                Assert-Preferences
                Assert-VerifiedWindowsPair $install $new
                if (-not $run.Evidence.process_exited_within_deadline -or $run.Evidence.exit_code -ne 0) { throw 'Same-version no-op failed' }
                if ($ActiveTray) {
                    $after = Get-Owner $frontendImage
                    if ($null -eq $after -or $after.Pid -ne $existing.Pid -or $after.Window -ne $existing.Window -or [PublicUpdateNative]::WaitForSingleObject($existing.Handle, 0) -ne 258) { throw 'Same-version no-op replaced the matching tray' }
                    Assert-OneTray $after
                    $noop.tray_after = Owner-Evidence $after
                    $noop.existing_matching_tray_preserved = $true
                    if ($Frontend -eq 'cli') {
                        $noop.effective_context_after = Assert-LaunchContext $after
                        $noop.effective_context_unchanged = $true
                    }
                } elseif (@([PublicUpdateNative]::Windows()).Count -ne 0 -or (Test-MCWProcesses)) { throw 'No-op unexpectedly started a frontend' }
                Assert-Preferences
                $noop.preferences_unchanged = $true
                $noop.core_success = $true
                if (-not $run.Evidence.stdout_eof_at_deadline -or -not $run.Evidence.stderr_eof_at_deadline) { throw 'No-op captured stdout/stderr EOF failed' }
                if ($ActiveTray) { $noop.eof_with_same_live_successor = Assert-LiveEof $run $after }
                $noop.status = 'passed'
            }
        } catch {
            $report.errors += "$scenario`: $($_.Exception.Message)"
            $_ | Out-String | Add-Content (Join-Path $proof 'failures.txt')
            if (($scenario -eq 'upgrade' -or $CurrentRepairOnly) -and @($report.phases | Where-Object { $_.name -eq 'noop' }).Count -eq 0) {
                $report.phases += @{ name = 'noop'; status = 'skipped'; reason = "$scenario did not pass, including EOF; never relaunch a failed updater" }
            }
        } finally {
            # Evidence was frozen before this cleanup. Closing a successor here can
            # release inherited pipes, but NEVER changes eof_at_deadline or status.
            Save-Report
            # A timed-out updater cannot race cleanup or the independent repair.
            foreach ($entry in $captures) {
                if ($entry.Role -eq 'command' -and $null -ne $entry.Capture -and $entry.Image -eq $cli -and [PublicUpdateNative]::WaitForSingleObject($entry.Capture.Process.Handle, 0) -eq 258) {
                    $handle = $entry.Capture.Process.Handle
                    if (-not [string]::Equals([PublicUpdateNative]::Image($handle), $entry.Image, [StringComparison]::OrdinalIgnoreCase)) { throw 'Timed-out updater cleanup image changed' }
                    if (-not [PublicUpdateNative]::TerminateProcess($handle, 1) -or [PublicUpdateNative]::WaitForSingleObject($handle, 15000) -ne 0) { throw 'Timed-out updater owned-handle cleanup failed' }
                    $report.cleanup += @{ pid = $entry.Capture.Process.Id; image = $entry.Image; forced_exact_handle = $true }
                }
            }
            try {
                Retain-ImageProcesses $frontendImage
                $resident = Get-Owner $frontendImage
                if ($null -ne $resident) { Stop-Owner $resident }
            } catch {
                $report.errors += "cleanup $scenario`: $($_.Exception.Message)"
                Save-Report
                throw
            }
        }
    }
    if ($report.errors.Count -ne 0 -or @($report.phases | Where-Object { $_.status -ne 'passed' }).Count -ne 0) { throw 'Public update proof failed; inspect phase and EOF evidence' }
    $report.status = 'passed'
} catch {
    $report.errors += $_.Exception.Message
    $_ | Out-String | Add-Content (Join-Path $proof 'failures.txt')
    throw
} finally {
    # Fallback termination uses only previously retained, exact-image handles.
    # It is cleanup, never a way to turn an EOF/transaction failure into a pass.
    foreach ($owner in $owners) {
        try {
            if ([PublicUpdateNative]::WaitForSingleObject($owner.Handle, 0) -eq 258) {
                if (-not [string]::Equals([PublicUpdateNative]::Image($owner.Handle), $owner.Image, [StringComparison]::OrdinalIgnoreCase)) { throw 'Owned cleanup image changed' }
                if (-not [PublicUpdateNative]::TerminateProcess($owner.Handle, 1) -or [PublicUpdateNative]::WaitForSingleObject($owner.Handle, 15000) -ne 0) { throw 'Owned handle cleanup failed' }
                $report.errors += "Forced cleanup of owned tray $($owner.Pid)"
                $report.status = 'failed'
                $report.cleanup += @{ pid = $owner.Pid; image = $owner.Image; forced_exact_handle = $true }
            }
        } catch { $report.errors += $_.Exception.Message; $report.status = 'failed' }
    }
    foreach ($entry in $captures) {
        try {
            $process = if ($null -ne $entry.Capture) { $entry.Capture.Process } else { $entry.Process }
            if ([PublicUpdateNative]::WaitForSingleObject($process.Handle, 0) -eq 258) {
                if (-not [string]::Equals([PublicUpdateNative]::Image($process.Handle), $entry.Image, [StringComparison]::OrdinalIgnoreCase)) { throw 'Child cleanup image changed' }
                if (-not [PublicUpdateNative]::TerminateProcess($process.Handle, 1) -or [PublicUpdateNative]::WaitForSingleObject($process.Handle, 15000) -ne 0) { throw 'Retained child handle cleanup failed' }
                $report.errors += "Forced cleanup of owned child $($entry.Name)"
                $report.status = 'failed'
            }
            if ($null -ne $entry.Capture) { Save-Capture $entry.Capture "$($entry.Name)-after-cleanup"; $entry.Capture.Dispose() } else { $process.Dispose() }
        } catch { $report.errors += $_.Exception.Message; $report.status = 'failed' }
    }
    foreach ($owner in $owners) { $owner.Process.Dispose() }
    if ($null -ne $settings -and (Test-Path $settings)) { Copy-Item $settings (Join-Path $proof 'preferences-after.toml') }
    if ($null -ne $policyFile -and (Test-Path $policyFile)) { Copy-Item $policyFile (Join-Path $proof 'explicit-policy-after.toml') }
    Save-Report
}
if ($report.status -ne 'passed') { throw 'Public update proof cleanup failed; inspect retained evidence' }
