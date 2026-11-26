[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Candidate,
    [Parameter(Mandatory)][string]$Proof,
    [string]$StandardAccountSid,
    [string]$SourceRevision = $env:GITHUB_SHA,
    [switch]$LocalDisposableVm,
    [string]$FixtureRoot,
    [switch]$PositiveLifecycleOnly,
    [switch]$HybridRollbackOnly,
    [switch]$ObserveRegistration,
    [string]$ExpectedMsiSha256,
    [string]$RustMsiTest,
    [string]$BaselineMsi,
    [switch]$SampleDesktopResources
)
$ErrorActionPreference = 'Stop'
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
$isAdmin = $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
$standard = [bool]($StandardAccountSid -or $LocalDisposableVm)
$interactive = !$standard -or $LocalDisposableVm
if ($LocalDisposableVm) {
    if ($StandardAccountSid -or $ExpectedMsiSha256 -cnotmatch '^[0-9a-f]{64}$') {
        throw 'Disposable-guest mode requires an independent MSI SHA-256 and the actual logged-in user.'
    }
    . (Join-Path $PSScriptRoot 'windows-fixture-context.ps1')
    $guest = Assert-McwDisposableFixtureHost -RequireStandardUser -LocalDisposableVm -FixtureRoot $FixtureRoot
    $rootPrefix = $guest.fixtureRoot.TrimEnd('\') + '\'
    $candidatePath = (Resolve-Path -LiteralPath $Candidate).Path
    $proofParent = Assert-McwFixtureRoot -Path (Split-Path ([IO.Path]::GetFullPath($Proof)))
    $candidateParent = Assert-McwFixtureRoot -Path (Split-Path $candidatePath)
    foreach ($folder in @($candidateParent, $proofParent)) {
        if ($folder -ine $guest.fixtureRoot -and !$folder.StartsWith($rootPrefix, [StringComparison]::OrdinalIgnoreCase)) {
            throw 'The MSI and proof must be under the admitted guest root.'
        }
    }
    $file = Get-Item -LiteralPath $candidatePath -Force
    if (($file.Attributes -band [IO.FileAttributes]::ReparsePoint) -or
        (Get-FileHash -LiteralPath $candidatePath -Algorithm SHA256).Hash -ine $ExpectedMsiSha256) {
        throw 'The guest MSI does not match its independently checked hash.'
    }
    $null = Assert-McwFixtureRoot -Path (Join-Path $candidateParent 'bundle')
    $policy = Get-ItemProperty -LiteralPath 'HKLM:\Software\Policies\Microsoft\Windows\Installer' -ErrorAction SilentlyContinue
    if ($policy.DisableMSI -in 1,2) { throw "The guest policy DisableMsi=$($policy.DisableMSI) disallows this fixture." }
    $Candidate = $candidatePath
    if ($ObserveRegistration) { $StandardAccountSid = $identity.User.Value }
} elseif ($FixtureRoot -or $ExpectedMsiSha256) {
    throw 'FixtureRoot and ExpectedMsiSha256 require LocalDisposableVm mode.'
} elseif ($standard) {
    $shared = Join-Path $env:PUBLIC 'McwStandard-'
    if ($identity.User.Value -ne $StandardAccountSid -or $isAdmin -or
        $identity.Groups.Value -contains 'S-1-5-32-544' -or
        !$Candidate.StartsWith($shared, [StringComparison]::OrdinalIgnoreCase) -or
        $identity.Name -notmatch '\\mcwqa[0-9a-f]{8}$') {
        throw 'The process token, account or isolated prefix does not qualify as a standard-user fixture.'
    }
    # Resolve folders for the loaded standard profile rather than consuming any
    # application paths inherited from the runner administrator.
    $profileKey = "HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\ProfileList\$StandardAccountSid"
    $profile = [Environment]::ExpandEnvironmentVariables((Get-ItemProperty -LiteralPath $profileKey).ProfileImagePath)
    if (!$profile -or $profile -ine [Environment]::GetFolderPath('UserProfile')) {
        throw 'The loaded standard account does not own this fixture profile.'
    }
    $env:USERPROFILE = $profile
    $env:HOMEDRIVE = Split-Path -Qualifier $profile
    $env:HOMEPATH = Split-Path -NoQualifier $profile
    $local = [Environment]::GetFolderPath('LocalApplicationData', 'Create')
    $roaming = [Environment]::GetFolderPath('ApplicationData', 'Create')
    if (!$local.StartsWith($profile.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase) -or
        !$roaming.StartsWith($profile.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase)) {
        throw 'The loaded standard account does not own this fixture profile.'
    }
    $env:LOCALAPPDATA = $local
    $env:APPDATA = $roaming
    $temp = Join-Path $local 'Temp'
    New-Item -ItemType Directory -Path $temp -Force | Out-Null
    $env:TEMP = $temp
    $env:TMP = $temp
} elseif ($env:RUNNER_ENVIRONMENT -cne 'github-hosted' -or $env:GITHUB_ACTIONS -cne 'true') {
    throw 'This scenario requires a disposable GitHub-hosted Windows runner.'
}
if ($BaselineMsi -and !$LocalDisposableVm) { throw 'An offline baseline is confined to the admitted local VM.' }
if ($PositiveLifecycleOnly -and ($HybridRollbackOnly -or !$ObserveRegistration -or
    (!$LocalDisposableVm -and $env:PROCESSOR_ARCHITECTURE -cne 'ARM64'))) {
    throw 'The positive-only replay requires an independently observed standard-account fixture.'
}
if ($HybridRollbackOnly -and !$LocalDisposableVm -and (!$ObserveRegistration -or $env:PROCESSOR_ARCHITECTURE -cne 'ARM64')) {
    throw 'The separate hybrid rollback needs a disposable Windows-on-ARM runner or an admitted local VM.'
}
if ($ObserveRegistration -and (!$StandardAccountSid -or (!$LocalDisposableVm -and
    ($env:RUNNER_ENVIRONMENT -cne 'github-hosted' -or $env:GITHUB_ACTIONS -cne 'true')))) {
    throw 'The independent observer requires the disposable standard-account wrapper.'
}
if ($SourceRevision -notmatch '^[0-9a-f]{40}$') { throw 'Exact source revision required' }
if ($SampleDesktopResources -and !$LocalDisposableVm -and ($standard -or !$isAdmin -or
    $env:RUNNER_ENVIRONMENT -cne 'github-hosted' -or $env:GITHUB_ACTIONS -cne 'true')) {
    throw 'Desktop resource sampling requires the disposable native administrator runner.'
}
if ($SampleDesktopResources) {
    Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class McwInstalledBaselineWindow {
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern IntPtr FindWindow(string className, string title);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr window);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr window, out uint pid);
    public static bool IsVisibleFor(uint owner) {
        var window = FindWindow("MicCamWatchDesktopClass", null);
        uint actual;
        return window != IntPtr.Zero && GetWindowThreadProcessId(window, out actual) != 0 &&
               actual == owner && IsWindowVisible(window);
    }
}
'@
}
$base = Join-Path $env:LOCALAPPDATA 'Programs\MicCamWatch'
if ($LocalDisposableVm) {
    Add-Type @'
using System;
using System.Runtime.InteropServices;
using System.Threading;
public static class McwLabFrameProof {
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] struct MonitorInfo { public uint Size; public Rect Monitor, Work; public uint Flags; }
    [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr window, out Rect rect);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr window, out uint pid);
    [DllImport("user32.dll")] static extern IntPtr MonitorFromWindow(IntPtr window, uint flags);
    [DllImport("user32.dll")] static extern bool GetMonitorInfo(IntPtr monitor, ref MonitorInfo info);
    [DllImport("user32.dll")] static extern bool SetWindowPos(IntPtr window, IntPtr after, int x, int y, int width, int height, uint flags);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern IntPtr SendMessageTimeoutW(IntPtr window, uint message, UIntPtr wparam, IntPtr lparam, uint flags, uint timeout, out UIntPtr result);
    public static object Check(IntPtr window, uint expectedPid) {
        uint pid;
        if (window==IntPtr.Zero || GetWindowThreadProcessId(window,out pid)==0 || pid!=expectedPid) throw new Exception("Not the owned Flutter window");
        Rect before;
        if (!GetWindowRect(window,out before)) throw new Exception("Cannot read owned frame");
        if (!SetWindowPos(window,IntPtr.Zero,-30000,-30000,0,0,0x15)) throw new Exception("Cannot position owned frame");
        UIntPtr response;
        if (SendMessageTimeoutW(window,0x007e,UIntPtr.Zero,IntPtr.Zero,2,3000,out response)==IntPtr.Zero) throw new Exception("WM_DISPLAYCHANGE timed out");
        Thread.Sleep(200);
        Rect after;
        var info=new MonitorInfo {Size=(uint)Marshal.SizeOf(typeof(MonitorInfo))};
        if (!GetWindowRect(window,out after) || !GetMonitorInfo(MonitorFromWindow(window,2),ref info)) throw new Exception("Cannot verify recovered frame");
        if (after.Left<info.Work.Left || after.Top<info.Work.Top || after.Right>info.Work.Right || after.Bottom>info.Work.Bottom || after.Right<=after.Left || after.Bottom<=after.Top) throw new Exception("Display change left the frame outside the work area");
        return new {before=before, after=after, workArea=info.Work, stimulus="owned window moved off-screen; WM_DISPLAYCHANGE; no physical monitor removed"};
    }
}
'@
}
$configuration = Join-Path $env:APPDATA 'MicCamWatch'
$data = Join-Path $env:LOCALAPPDATA 'MicCamWatch'
$runKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$programs = [Environment]::GetFolderPath('Programs')
if ($StandardAccountSid) {
    # The hosted fixture account is new. Resolve its unredirected per-user
    # Start menu relative to its verified Roaming folder in the loaded profile.
    $programs = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs'
}
$shortcut = Join-Path $programs 'MicCamWatch.lnk'
$upgradeCode = '{8FC9317B-4517-4E83-9F52-CF3257584BF6}'
$installer = New-Object -ComObject WindowsInstaller.Installer
$related = $installer.RelatedProducts($upgradeCode)
$count = $related.GetType().InvokeMember('Count', [Reflection.BindingFlags]::GetProperty, $null, $related, $null)
[void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($related)
if ($count -isnot [int] -or $count -ne 0 -or
    (Get-Process mcw,mcw-tray,miccamwatch_desktop -ErrorAction SilentlyContinue)) {
    throw 'Existing MicCamWatch product or process. Refusing install fixture.'
}
foreach ($path in @($base,$configuration,$data,$Proof,$shortcut)) {
    if (Test-Path -LiteralPath $path) { throw "Existing fixture path: $path" }
}
New-Item -ItemType Directory -Path $Proof | Out-Null
$Proof = (Resolve-Path -LiteralPath $Proof).Path
$Candidate = (Resolve-Path -LiteralPath $Candidate).Path
$report = [ordered]@{status='running'; source=$SourceRevision; uacExercised=$false; standardUserQualified=$false;
    administratorToken=$isAdmin; standardTokenVerified=$standard; interactiveFlutterRequested=$interactive;
    rollbackInjected=!$PositiveLifecycleOnly; localDisposableVm=[bool]$LocalDisposableVm; sessionId=[Diagnostics.Process]::GetCurrentProcess().SessionId}

function Invoke-Msi([string]$Operation, [string]$Payload, [string]$Label, [int]$ExpectedCode = 0) {
    if ($LocalDisposableVm) {
        $null = Assert-McwDisposableFixtureHost -RequireStandardUser -LocalDisposableVm -FixtureRoot $FixtureRoot
        $service = Get-CimInstance Win32_Service -Filter "Name='msiserver'"
        if (@(Get-CimInstance Win32_Process -Filter "Name='msiexec.exe'" |
              Where-Object ProcessId -ne $service.ProcessId).Count) {
            throw 'Another MSI client remains active in the guest.'
        }
        foreach ($name in 'Global\_MSIExecute','_MSIExecute') {
            $mutex=$null; $acquired=$false
            try {
                $mutex=[Threading.Mutex]::OpenExisting($name)
                $acquired=$mutex.WaitOne(0)
                if (!$acquired) { throw 'Another MSI transaction owns the execute mutex.' }
            } catch [Threading.WaitHandleCannotBeOpenedException] {
                # An absent mutex is idle; access denial or abandonment fails closed.
            } finally {
                if ($acquired) { $mutex.ReleaseMutex() }
                if ($mutex) { $mutex.Dispose() }
            }
        }
    }
    $log = Join-Path $Proof "$Label.log"
    # The real standard-account desktop can receive legitimate UAC prompts.
    $ui = if ($LocalDisposableVm) { '/qb' } else { '/qn' }
    $arguments = '{0} "{1}" {2} /norestart /l*v "{3}"' -f $Operation,$Payload,$ui,$log
    $process = Start-Process -FilePath "$env:WINDIR\System32\msiexec.exe" -WindowStyle Hidden -PassThru -ArgumentList $arguments
    $transaction = [ordered]@{ pid=$process.Id; parent=$PID; firstUtc=[DateTime]::UtcNow.ToString('O') }
    $report["transaction-$Label"] = $transaction
    $timeout = if ($LocalDisposableVm) { 600000 } else { 180000 }
    if (!$process.WaitForExit($timeout)) { throw "MSI timed out. Retained live fixture; inspect $log." }
    $transaction.lastUtc = [DateTime]::UtcNow.ToString('O')
    $transaction.exitCode = $process.ExitCode
    if ($process.ExitCode -ne $ExpectedCode) { throw "MSI $Label returned $($process.ExitCode), expected $ExpectedCode" }
}

function Read-MsiValue($Database, [string]$Query) {
    $view = $Database.OpenView($Query)
    [void]$view.Execute()
    try {
        $row = $view.Fetch()
        if ($null -eq $row) { throw "MSI query returned no value: $Query" }
        try { return $row.StringData(1) }
        finally { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($row) }
    } finally {
        [void]$view.Close()
        [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($view)
    }
}

function Observe-Registration([string]$Phase, [string]$Current, [string]$Other) {
    if (!$ObserveRegistration) { return }
    $request = Join-Path $Proof "observe-$Phase.json"
    @{ current = $Current; other = $Other } | ConvertTo-Json | Set-Content -LiteralPath "$request.tmp" -Encoding utf8
    Move-Item -LiteralPath "$request.tmp" -Destination $request
    $receipt = Join-Path $PSScriptRoot "observations\$Phase.json"
    $timer = [Diagnostics.Stopwatch]::StartNew()
    while (!(Test-Path -LiteralPath $receipt -PathType Leaf)) {
        if ($timer.Elapsed.TotalSeconds -gt 30) { throw "Independent observer did not answer $Phase" }
        Start-Sleep -Milliseconds 200
    }
    $result = Get-Content -LiteralPath $receipt -Raw | ConvertFrom-Json
    $report["independent-$Phase"] = $result
    if ($result.error -or $result.phase -cne $Phase -or $result.targetSid -cne $StandardAccountSid -or
        !$result.observerSid -or $result.observerSid -eq $StandardAccountSid) {
        throw "Independent observation failed for $Phase"
    }
}

function Assert-Registration([string]$Phase, [string]$Version) {
    if (!$ObserveRegistration) { return }
    $result = $report["independent-$Phase"]
    $entry = $result.current
    $p = $entry.properties
    if ($result.other.properties.State.status -ne 1605 -or $result.other.upgradeIndexError -or $result.other.productInUserUpgradeIndex) {
        throw "Independent observer could not establish that the other product is absent at $Phase"
    }
    if ($Phase -ceq 'removed-hybrid') {
        if ($p.State.status -ne 1605 -or $entry.upgradeIndexError -or $entry.productInUserUpgradeIndex) {
            throw 'Independent observer could not establish removal of the hybrid product'
        }
        return
    }
    # MsiGetProductInfoEx returns "5" for an installed product and "1" for advertised.
    if ($p.State.status -ne 0 -or $p.State.value -cne '5' -or
        $p.VersionString.status -ne 0 -or $p.VersionString.value -cne $Version -or
        $p.InstallLocation.status -ne 0 -or ([string]$p.InstallLocation.value).TrimEnd('\') -ine $base -or
        !$entry.productInUserUpgradeIndex -or $entry.cacheError -or $entry.cacheIdentity.ProductCode -ine $entry.productCode -or
        $entry.cacheIdentity.UpgradeCode -ine $upgradeCode -or $entry.cacheIdentity.ProductVersion -cne $Version) {
        throw "Installed MSI registration could not be verified for $Phase"
    }
}

function Identity([string]$Path, [string]$Version) {
    $db = $installer.OpenDatabase($Path, 0)
    try {
        if ((Read-MsiValue $db "SELECT ``Value`` FROM ``Property`` WHERE ``Property``='ProductName'") -cne 'MicCamWatch' -or
            (Read-MsiValue $db "SELECT ``Value`` FROM ``Property`` WHERE ``Property``='ProductVersion'") -cne $Version -or
            (Read-MsiValue $db "SELECT ``Value`` FROM ``Property`` WHERE ``Property``='UpgradeCode'") -cne $upgradeCode) {
            throw 'Unexpected MSI product identity'
        }
        $code = Read-MsiValue $db "SELECT ``Value`` FROM ``Property`` WHERE ``Property``='ProductCode'"
        if ($code -notmatch '^\{[0-9A-Fa-f-]{36}\}$') { throw 'Invalid MSI product code' }
        return $code
    } finally { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($db) }
}

function Check-Installed([string]$Code, [string]$Version) {
    $state = $installer.ProductState($Code)
    $location = ([string]$installer.ProductInfo($Code, 'InstallLocation')).TrimEnd('\')
    $assignment = [string]$installer.ProductInfo($Code, 'AssignmentType')
    $report["msiProduct$Version"] = [ordered]@{state=$state; assignmentType=$assignment;
        actualLocation=$location; expectedLocation=$base; processArchitecture=$env:PROCESSOR_ARCHITECTURE}
    if ($state -ne 5 -or $location -ine $base -or $assignment -ne '0') {
        throw "Invalid per-user MSI status for $Version (state=$state, assignment=$assignment, locationMatch=$($location -ieq $base))"
    }
    $cliVersion = & "$base\mcw.exe" --version
    $cliExit = $LASTEXITCODE
    $trayPresent = Test-Path -LiteralPath "$base\mcw-tray.exe" -PathType Leaf
    $shortcutPresent = Test-Path -LiteralPath $shortcut -PathType Leaf
    $report["installedComponents$Version"] = @{ cliVersion=$cliVersion; cliExit=$cliExit; tray=$trayPresent;
        shortcut=$shortcutPresent; shortcutPath=$shortcut }
    if ($cliExit -ne 0 -or $cliVersion -cne "mcw $Version" -or !$trayPresent -or !$shortcutPresent) {
        throw "Installed executables or shortcut version mismatch for $Version"
    }
    if ($Version -ne '0.21.0') {
        # Keep the process handle. PowerShell 5 Start-Process can lose the
        # exit status of a GUI program that exits before the first wait.
        $info = [Diagnostics.ProcessStartInfo]::new("$base\mcw-tray.exe", '--version')
        $info.UseShellExecute = $false
        $info.CreateNoWindow = $true
        $info.RedirectStandardOutput = $true
        $process = [Diagnostics.Process]::Start($info)
        try {
            $output = $process.StandardOutput.ReadToEndAsync()
            if (!$process.WaitForExit(10000) -or $process.ExitCode -ne 0 -or
                !$output.Wait(1000) -or $output.Result.Trim() -cne "mcw-tray $Version") {
                throw 'Hybrid tray version mismatch'
            }
        } finally { $process.Dispose() }
    }
}

function Copy-FailingFixture([string]$Source, [string]$Destination, [string]$FixtureCode, [string]$FixtureVersion) {
    Copy-Item -LiteralPath $Source -Destination $Destination
    $db = $installer.OpenDatabase($Destination, 1)
    try {
        if ($FixtureCode) {
            $summary = $db.SummaryInformation(1)
            try {
                $summary.Property(9) = ([Guid]::NewGuid().ToString('B').ToUpperInvariant())
                [void]$summary.Persist()
            } finally { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($summary) }
        }
        $seq = [int](Read-MsiValue $db "SELECT ``Sequence`` FROM ``InstallExecuteSequence`` WHERE ``Action``='RemoveExistingProducts'")
        if ($seq -le 1500 -or $seq -ge 4000) { throw 'Upgrade removal is outside the expected rollback window' }
        $p = $seq + 1
        $statements = @(
            "INSERT INTO ``CustomAction`` (``Action``, ``Type``, ``Target``) VALUES ('McwFixtureFailure', 19, 'Expected MicCamWatch rollback fixture failure')",
            "INSERT INTO ``InstallExecuteSequence`` (``Action``, ``Condition``, ``Sequence``) VALUES ('McwFixtureFailure', '1', $p)"
        )
        if ($FixtureCode) {
            # A higher test-only product code/version lets MSI remove the
            # installed hybrid as an older product before the injected failure.
            $statements += @(
                "UPDATE ``Property`` SET ``Value``='$FixtureCode' WHERE ``Property``='ProductCode'",
                "UPDATE ``Property`` SET ``Value``='$FixtureVersion' WHERE ``Property``='ProductVersion'",
                "DELETE FROM ``Upgrade`` WHERE ``ActionProperty``='WIX_UPGRADE_DETECTED'",
                "DELETE FROM ``Upgrade`` WHERE ``ActionProperty``='WIX_DOWNGRADE_DETECTED'",
                "INSERT INTO ``Upgrade`` (``UpgradeCode``, ``VersionMax``, ``Attributes``, ``ActionProperty``) VALUES ('$upgradeCode', '$FixtureVersion', 1, 'WIX_UPGRADE_DETECTED')",
                "INSERT INTO ``Upgrade`` (``UpgradeCode``, ``VersionMin``, ``Attributes``, ``ActionProperty``) VALUES ('$upgradeCode', '$FixtureVersion', 2, 'WIX_DOWNGRADE_DETECTED')"
            )
        }
        foreach ($statement in $statements) {
            $view = $db.OpenView($statement)
            try { [void]$view.Execute() }
            finally { [void]$view.Close(); [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($view) }
        }
        [void]$db.Commit()
    } finally { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($db) }
}

function Check-Rollback([string]$Label, [string[]]$Files) {
    $rollback = Get-Content -LiteralPath (Join-Path $Proof "$Label.log") -Raw
    $rollbackScript = [regex]::Match($rollback,
        '(?s)Action ended [^\r\n]+RemoveExistingProducts\. Return value 1\..*?Action ended [^\r\n]+McwFixtureFailure\. Return value 3\..*?Executing op: Header\([^\r\n]*ScriptType=2,[^\r\n]*\)(?<restore>.*)')
    if (!$rollbackScript.Success -or $rollback -notmatch 'Expected MicCamWatch rollback fixture failure') {
        throw 'Expected MSI removal, injected failure and rollback script were not observed in order'
    }
    foreach ($name in $Files) {
        $destination = [regex]::Escape((Join-Path $base $name))
        if ($rollbackScript.Groups['restore'].Value -notmatch
            ('(?im)^.*Executing op: FileCopy\(SourceName=[^,\r\n]+\.rbf,[^\r\n]*DestName=' + $destination + ',')) {
            throw "Rollback log has no backup restore for $name"
        }
    }
}

function Check-Payload($Manifest) {
    $manifestPath = 'miccamwatch-bundle.json'
    $source = Join-Path (Split-Path $Candidate) "bundle\$manifestPath"
    if ((Get-FileHash -LiteralPath (Join-Path $base $manifestPath) -Algorithm SHA256).Hash -cne
        (Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash) {
        throw 'Installed manifest bytes differ'
    }
    $actual = @(Get-ChildItem -LiteralPath $base -Recurse -File -Force | ForEach-Object {
        $_.FullName.Substring($base.Length + 1).Replace('\','/')
    })
    # Autostart in the baseline fixture creates this exact native lock. It
    # belongs to the runtime, while the MSI owns the manifest and payload.
    $lock = Get-Item -LiteralPath $installationLock -Force
    if ($lock.PSIsContainer -or $lock.Length -ne 0 -or $lock.Attributes -band [IO.FileAttributes]::ReparsePoint) {
        throw 'Unexpected native installation lock'
    }
    $names = @($Manifest.files.PSObject.Properties.Name) + $manifestPath + $lock.Name
    $differences = @(Compare-Object $names $actual -CaseSensitive)
    if ($differences.Count) {
        $report.installedLayoutDifferences = @($differences | Select-Object InputObject,SideIndicator)
    }
    foreach ($file in $Manifest.files.PSObject.Properties) {
        if ((Get-FileHash -LiteralPath (Join-Path $base $file.Name) -Algorithm SHA256).Hash -ine $file.Value) {
            throw "Installed payload digest mismatch: $($file.Name)"
        }
    }
}

function Read-Autostart {
    $task = Get-ScheduledTask -TaskName 'MicCamWatch Tray' -TaskPath '\' -ErrorAction SilentlyContinue
    if ($null -ne $task) {
        if (@($task.Actions).Count -ne 1) { throw 'Unexpected task action count' }
        "Task: $($task.Actions.Execute) $($task.Actions.Arguments)"
    }
    $key = Get-Item -LiteralPath $runKey -ErrorAction SilentlyContinue
    if ($null -eq $key) { return }
    $key.GetValueNames() | Where-Object { $_ -match 'MicCamWatch' } | ForEach-Object { [string]$key.GetValue($_) }
}

function Owned([string]$Name, [string]$Image) {
    $all = @(Get-Process -Name $Name -ErrorAction SilentlyContinue)
    foreach ($item in $all) {
        if ($item.Path -ine $Image) { throw "Unexpected $Name process path" }
    }
    $all
}

function Read-ResourceCoverage {
    $status = (& "$base\mcw.exe" status --json --no-color) -join "`n" | ConvertFrom-Json
    if (!$status.collectors) { throw 'No native collector status for resource sample' }
    [ordered]@{
        collectorStates = @($status.collectors | Select-Object collector,state)
        observationGaps = @($status.observation_gaps | Where-Object { $_ })
        observedActive = @($status.accesses | Where-Object activity -eq 'active').Count
    }
}

function Measure-DesktopResources([string]$Label, [Diagnostics.Process[]]$Processes) {
    try {
        $beforeCoverage = Read-ResourceCoverage
        $timer = [Diagnostics.Stopwatch]::StartNew()
        $samples = @(foreach ($index in 0..15) {
            if ($index) { Start-Sleep -Seconds 1 }
            foreach ($item in $Processes) {
                $item.Refresh()
                if ($item.HasExited) { throw "Process exited during sample: $($item.Id)" }
            }
            [ordered]@{
                seconds = $timer.Elapsed.TotalSeconds
                processCount = $Processes.Count
                cpuSeconds = ($Processes | ForEach-Object { $_.TotalProcessorTime.TotalSeconds } | Measure-Object -Sum).Sum
                rssBytes = ($Processes | Measure-Object WorkingSet64 -Sum).Sum
                privateCommitBytes = ($Processes | Measure-Object PrivateMemorySize64 -Sum).Sum
                threadCount = ($Processes | ForEach-Object { $_.Threads.Count } | Measure-Object -Sum).Sum
            }
        })
        $afterCoverage = Read-ResourceCoverage
        $report.desktopResources[$Label] = [ordered]@{
            coverageBefore = $beforeCoverage
            coverageAfter = $afterCoverage
            samples = $samples
        }
    } catch {
        $report.desktopResources[$Label] = [ordered]@{error = $_.ToString()}
    }
}

function Measure-BaselineDesktop {
    $image = "$base\mcw-tray.exe"
    $launch = Start-Process -FilePath $image -PassThru
    try {
        $timer = [Diagnostics.Stopwatch]::StartNew()
        do {
            Start-Sleep -Milliseconds 200
            & "$base\mcw.exe" tray status | Out-Null
            $ready = $LASTEXITCODE -eq 0
        } until ($ready -or $launch.HasExited -or $timer.Elapsed.TotalSeconds -ge 30)
        $owner = @(Owned 'mcw-tray' $image)
        if (!$ready -or $launch.HasExited -or $owner.Count -ne 1 -or $owner[0].Id -ne $launch.Id) {
            throw 'Native v0.21.0 tray owner was not verified'
        }
        Measure-DesktopResources 'baseline-closed' @($launch)
        $timer.Restart()
        & "$base\mcw.exe" tray open | Out-Null
        if ($LASTEXITCODE -ne 0) { throw 'Native v0.21.0 open request failed' }
        do {
            Start-Sleep -Milliseconds 200
            $visible = [McwInstalledBaselineWindow]::IsVisibleFor([uint32]$launch.Id)
        } until ($visible -or $launch.HasExited -or $timer.Elapsed.TotalSeconds -ge 30)
        if (!$visible -or $launch.HasExited) {
            throw 'Native v0.21.0 window was not observed'
        }
        $report.desktopResources.baselineWindowVisibleMs = $timer.ElapsedMilliseconds
        Measure-DesktopResources 'baseline-open' @($launch)
    } catch {
        $report.desktopResources.baselineError = $_.ToString()
    } finally {
        if (!$launch.HasExited) {
            $owner = @(Owned 'mcw-tray' $image)
            if ($owner.Count -ne 1 -or $owner[0].Id -ne $launch.Id) { throw 'Cannot prove baseline tray identity for shutdown' }
            & "$base\mcw.exe" tray stop
            if ($LASTEXITCODE -ne 0 -or !$launch.WaitForExit(15000)) { throw 'Baseline owner did not stop cooperatively' }
        }
    }
}

try {
    if ($ObserveRegistration) {
        . (Join-Path $PSScriptRoot 'windows-fixture-context.ps1')
        $context = Get-McwStandardUserContext
        $report.clientToken = $context
        if ($context.elevated -or $context.administratorRole -or $context.elevationType -ne 1 -or
            $context.restricted -or $context.tokenType -ne 1 -or $context.integritySid -cne 'S-1-16-8192' -or
            $context.accountLocalGroupSids -contains 'S-1-5-32-544' -or
            $context.sid -cne $StandardAccountSid -or !$context.profileHiveLoaded) {
            throw 'The MSI invoker lacks the expected medium-integrity standard primary token'
        }
    }
    $stable = Join-Path $Proof 'baseline-v0.21.0.msi'
    if ($BaselineMsi) {
        $offline = Get-Item -LiteralPath $BaselineMsi -Force
        if ($offline.PSIsContainer -or ($offline.Attributes -band [IO.FileAttributes]::ReparsePoint) -or
            !$offline.FullName.StartsWith($rootPrefix, [StringComparison]::OrdinalIgnoreCase)) {
            throw 'Offline baseline escaped the admitted guest fixture root.'
        }
        Copy-Item -LiteralPath $offline.FullName -Destination $stable
    } else {
        Invoke-WebRequest -UseBasicParsing -Uri 'https://github.com/Roman-Cuisset/miccamwatch/releases/download/v0.21.0/miccamwatch-windows-x86_64.msi' -OutFile $stable
    }
    $stableSha = (Get-FileHash -LiteralPath $stable -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($stableSha -cne '4e6145a4e48bff47f08c8341c24c14e7fab563146e36b8338f46441f7f4da3df') { throw 'Public v0.21.0 digest changed' }
    $oldCode = Identity $stable '0.21.0'
    $bundle = Get-Content -LiteralPath (Join-Path (Split-Path $Candidate) 'bundle\miccamwatch-bundle.json') -Raw | ConvertFrom-Json
    if ($bundle.revision -cne $SourceRevision -or $bundle.ipc -ne 10) { throw 'The candidate is not this run source' }
    $newCode = Identity $Candidate $bundle.version
    $report.baselineSha256 = $stableSha
    $report.candidateSha256 = (Get-FileHash -LiteralPath $Candidate -Algorithm SHA256).Hash.ToLowerInvariant()
    Invoke-Msi '/i' $stable 'install-baseline'
    Observe-Registration 'installed-baseline' $oldCode $newCode
    Check-Installed $oldCode '0.21.0'
    $baselineHashes = @(Get-FileHash -LiteralPath "$base\mcw.exe","$base\mcw-tray.exe" -Algorithm SHA256 | ForEach-Object Hash)
    New-Item -ItemType Directory -Path $configuration,$data | Out-Null
    $settings = Join-Path $configuration 'settings.toml'
    $history = Join-Path $data 'history.jsonl'
    $preferences = @('notifications_enabled = false','sound_enabled = false','history_enabled = false',
      'mute_on_lock = false','block_camera_on_lock = false','restore_on_unlock = true','profile = "meeting"')
    [IO.File]::WriteAllLines($settings, [string[]]$preferences)
    '{"test_fixture":"existing history"}' | Set-Content -LiteralPath $history -Encoding utf8
    $before = @(Get-FileHash -LiteralPath $settings,$history -Algorithm SHA256 | ForEach-Object Hash)
    & "$base\mcw.exe" autostart enable
    if ($LASTEXITCODE -ne 0) { throw 'Baseline autostart setup failed' }
    $installationLock = Join-Path $base '.mcw-install.lock'
    if (!(Test-Path -LiteralPath $installationLock -PathType Leaf)) { throw 'Baseline autostart did not create its native lock' }
    $runEntry = @(Read-Autostart)
    if ($runEntry.Count -ne 1 -or !$runEntry[0].Contains($base)) { throw 'Baseline autostart registration missing' }

    if (!$PositiveLifecycleOnly -and !$HybridRollbackOnly) {
        $faulty = Join-Path $Proof 'rollback-fixture.msi'
        Copy-FailingFixture $Candidate $faulty
        Invoke-Msi '/i' $faulty 'upgrade-with-rollback' 1603
        Observe-Registration 'restored-baseline' $oldCode $newCode
        $report.legacyRollbackFiles = @(
            Get-FileHash -LiteralPath "$base\mcw.exe","$base\mcw-tray.exe" -Algorithm SHA256 -ErrorAction SilentlyContinue
        )
        $report.legacyRollbackRustBytes = @(Compare-Object $baselineHashes @($report.legacyRollbackFiles.Hash)).Count -eq 0
        $report.legacyRollbackShortcut = Test-Path -LiteralPath $shortcut -PathType Leaf
        $report.legacyRollbackData = @(Compare-Object $before @(Get-FileHash -LiteralPath $settings,$history -Algorithm SHA256 | ForEach-Object Hash)).Count -eq 0
        $report.legacyRollbackAutostart = @(Compare-Object $runEntry @(Read-Autostart)).Count -eq 0
        $report.legacyRollbackCandidateFilesAbsent = !(Test-Path -LiteralPath "$base\desktop")
        if (!$report.legacyRollbackRustBytes) {
            throw 'Rollback did not restore both original Rust executables'
        }
        if (!$report.legacyRollbackData -or !$report.legacyRollbackAutostart) {
            throw 'Legacy rollback changed the fixture data or autostart'
        }
        Check-Rollback 'upgrade-with-rollback' @('mcw.exe','mcw-tray.exe')
        Assert-Registration 'installed-baseline' '0.21.0'
        Assert-Registration 'restored-baseline' '0.21.0'
        Check-Installed $oldCode '0.21.0'
        if ($installer.ProductState($newCode) -eq 5 -or (Test-Path -LiteralPath "$base\desktop")) {
            throw 'Failed upgrade left the candidate installed'
        }
        $report.wholeMsiRollback = $true
    }
    Assert-Registration 'installed-baseline' '0.21.0'
    if ($SampleDesktopResources) {
        $report.desktopResources = [ordered]@{
            scope = 'Single hosted sample, observed tray and visible UI only, no capture injection'
            processAccounting = 'Exact tray/UI handles, with no OS process or transient helper accounting'
        }
        Measure-BaselineDesktop
    }

    if ($RustMsiTest) {
        if ($standard -or $env:RUNNER_ENVIRONMENT -cne 'github-hosted' -or $env:GITHUB_ACTIONS -cne 'true') {
            throw 'Rust MSI fixture mode requires the disposable administrator runner.'
        }
        $env:MCW_TEST_HYBRID_MSI = $Candidate
        $env:MCW_TEST_MSI_PROOF = $Proof
        $env:MCW_TEST_MSI_SHA256 = $report.candidateSha256
        & $RustMsiTest 'updater::msi::tests::install_exact_wix_artifact_on_hosted_fixture' --exact --ignored
        if ($LASTEXITCODE -ne 0) { throw 'The Rust installer transfer fixture failed.' }
        $report.rustMsiInstall = $true
    } else {
        Invoke-Msi '/i' $Candidate 'upgrade-hybrid'
    }
    Observe-Registration 'installed-hybrid' $newCode $oldCode
    Assert-Registration 'installed-hybrid' $bundle.version
    Check-Installed $newCode $bundle.version
    if ($installer.ProductState($oldCode) -eq 5 -or (Compare-Object $runEntry @(Read-Autostart))) {
        throw 'Upgrade left the baseline registered or changed autostart'
    }
    $report.msiUpgrade = $true
    Check-Payload $bundle
    $report.installedPayloadVerified = $true
    if ($interactive) {
        $trayImage = "$base\mcw-tray.exe"
        $flutterImage = "$base\desktop\miccamwatch_desktop.exe"
        $info = [Diagnostics.ProcessStartInfo]::new($trayImage, '--open')
        $info.UseShellExecute = $false
        $info.WorkingDirectory = $base
        $info.RedirectStandardError = $true
        $openTimer = [Diagnostics.Stopwatch]::StartNew()
        $launch = [Diagnostics.Process]::Start($info)
        $diagnostics = [Collections.Generic.List[string]]::new()
        $readyPids = [Collections.Generic.HashSet[int]]::new()
        $pendingDiagnostic = $launch.StandardError.ReadLineAsync()
        $drainDiagnostics = {
            param([int]$WaitMilliseconds)
            while ($pendingDiagnostic -and $pendingDiagnostic.Wait($WaitMilliseconds)) {
                $line = $pendingDiagnostic.GetAwaiter().GetResult()
                if ($null -eq $line) {
                    $pendingDiagnostic = $null
                    break
                }
                $diagnostics.Add($line)
                if ($line -match '^desktop event=ready endpoint=parent_received pid=([1-9][0-9]*) ') {
                    [void]$readyPids.Add([int]$Matches[1])
                }
                $pendingDiagnostic = $launch.StandardError.ReadLineAsync()
            }
        }
        try {
            $limit = [DateTime]::UtcNow.AddSeconds(45)
            do {
                Start-Sleep -Milliseconds 200
                $tray = @(Owned 'mcw-tray' $trayImage)
                $flutter = @(Owned 'miccamwatch_desktop' $flutterImage)
                $ui = @($flutter | Where-Object { $_.MainWindowHandle -ne [IntPtr]::Zero })
                . $drainDiagnostics 0
                $ready = $ui.Count -eq 1 -and $readyPids.Contains($ui[0].Id)
            } until (($tray.Count -eq 1 -and $ready) -or $launch.HasExited -or
                [DateTime]::UtcNow -gt $limit)
            $report.installedProcessObservation = [ordered]@{trayCount=$tray.Count; flutterCount=$flutter.Count;
                flutterWindowCount=$ui.Count; startedOwnerExited=$launch.HasExited; trayWindowTitles=@($tray.MainWindowTitle)}
            if ($launch.HasExited) { $report.installedProcessObservation.ownerExitCode = $launch.ExitCode }
            if ($tray.Count -ne 1 -or $ui.Count -ne 1 -or $tray[0].Id -ne $launch.Id) {
                throw 'Installed tray/Flutter did not open exactly once'
            }
            if (!$ready) { throw 'Missing installed IPC ready receipt before closing Flutter' }
            $report.installedIpcReady = $true
            if ($LocalDisposableVm) {
                if ($ui[0].HasExited) { throw 'Owned UI exited before frame qualification' }
                $report.displayChangeRecovery = [McwLabFrameProof]::Check($ui[0].MainWindowHandle, [uint32]$ui[0].Id)
            }
            if ($SampleDesktopResources) {
                $report.desktopResources.candidateWindowVisibleAndIpcMs = $openTimer.ElapsedMilliseconds
                Measure-DesktopResources 'candidate-open' @($launch, $ui[0])
            }
            $closeTimer = [Diagnostics.Stopwatch]::StartNew()
            if (!$ui[0].CloseMainWindow() -or !$ui[0].WaitForExit(15000)) { throw 'Installed Flutter window did not close' }
            if ($SampleDesktopResources) {
                $report.desktopResources.candidateCloseMs = $closeTimer.ElapsedMilliseconds
            }
            $owner = @(Owned 'mcw-tray' $trayImage)
            if ($owner.Count -ne 1 -or $owner[0].Id -ne $launch.Id -or @(Owned 'miccamwatch_desktop' $flutterImage).Count) {
                throw 'Close changed the native owner or retained a Flutter process'
            }
            & "$base\mcw.exe" tray status
            if ($LASTEXITCODE -ne 0) { throw 'Native status after window close failed' }
            $report.installedFlutterClosedTrayContinued = $true
            if ($SampleDesktopResources) {
                Measure-DesktopResources 'candidate-closed' @($launch)
            }
        } catch {
            $report.guiError = $_.ToString()
        } finally {
            if (!$launch.HasExited) {
                $owner = @(Owned 'mcw-tray' $trayImage)
                if ($owner.Count -ne 1 -or $owner[0].Id -ne $launch.Id) { throw 'Cannot prove exact tray identity for shutdown' }
                & "$base\mcw.exe" tray stop
                if ($LASTEXITCODE -ne 0 -or !$launch.WaitForExit(15000)) { throw 'Installed owner did not stop cooperatively' }
            }
            . $drainDiagnostics 1000
            [IO.File]::WriteAllLines((Join-Path $Proof 'installed-tray-stderr.log'), $diagnostics)
            $launch.Dispose()
        }
        if (!$report.installedIpcReady -and !$report.guiError) { $report.guiError = 'Missing installed IPC ready receipt' }
        if (@(Owned 'miccamwatch_desktop' $flutterImage).Count) { throw 'Flutter remains after its owner stopped' }
    }
    if (!$PositiveLifecycleOnly) {
        # Simulate a future MSI failing after removing this hybrid package. Verify
        # rollback against every installed file, including Flutter and the manifest.
        $future = Join-Path $Proof 'rollback-hybrid-fixture.msi'
        $futureCode = [Guid]::NewGuid().ToString('B').ToUpperInvariant()
        $currentVersion = [Version]::Parse($bundle.version)
        $futureVersion = '{0}.{1}.{2}' -f $currentVersion.Major,$currentVersion.Minor,($currentVersion.Build + 1)
        Copy-FailingFixture $Candidate $future $futureCode $futureVersion
        if ((Identity $future $futureVersion) -cne $futureCode) { throw 'Invalid future fixture identity' }
        Invoke-Msi '/i' $future 'hybrid-upgrade-with-rollback' 1603
        Observe-Registration 'restored-hybrid' $newCode $futureCode
        $report.hybridRollbackData = @(Compare-Object $before @(Get-FileHash -LiteralPath $settings,$history -Algorithm SHA256 | ForEach-Object Hash)).Count -eq 0
        $report.hybridRollbackAutostart = @(Compare-Object $runEntry @(Read-Autostart)).Count -eq 0
        Check-Payload $bundle
        $report.hybridRollbackPayload = !$report.installedLayoutDifferences
        Check-Rollback 'hybrid-upgrade-with-rollback' @('mcw.exe','mcw-tray.exe','miccamwatch-bundle.json',
            'desktop\miccamwatch_desktop.exe','desktop\flutter_windows.dll','desktop\data\app.so')
        Assert-Registration 'restored-hybrid' $bundle.version
        Check-Installed $newCode $bundle.version
        if ($installer.ProductState($futureCode) -eq 5 -or (Compare-Object $runEntry @(Read-Autostart))) {
            throw 'Failed hybrid upgrade left the newer fixture installed or changed autostart'
        }
        $report.wholeHybridMsiRollback = $true
    }

    & "$base\mcw.exe" autostart disable
    if ($LASTEXITCODE -ne 0 -or @(Read-Autostart).Count -ne 0) { throw 'Fixture autostart removal failed' }
    Invoke-Msi '/x' $newCode 'uninstall-hybrid'
    Observe-Registration 'removed-hybrid' $newCode $oldCode
    Assert-Registration 'removed-hybrid'
    if ($installer.ProductState($newCode) -eq 5 -or (Test-Path -LiteralPath "$base\desktop") -or
        (Test-Path -LiteralPath "$base\mcw.exe") -or (Test-Path -LiteralPath $shortcut)) {
        throw 'Uninstall left hybrid payload or product registrations'
    }
    $left = @(Get-ChildItem -LiteralPath $base -Recurse -File -Force -ErrorAction SilentlyContinue)
    if ($left.Count -gt 1 -or ($left.Count -eq 1 -and $left[0].FullName -ine $installationLock)) {
        throw 'The MSI uninstall left unexpected files'
    }
    $report.retainedNativeInstallationLock = $left.Count -eq 1
    if (Compare-Object $before @(Get-FileHash -LiteralPath $settings,$history -Algorithm SHA256 | ForEach-Object Hash)) {
        throw 'Upgrade/rollback/uninstall did not preserve fixture user data'
    }
    $report.userDataPreserved = $true
    $report.msiUninstall = $true
    $report.standardUserQualified = $standard
    if ($report.installedLayoutDifferences) { throw 'Installed payload differs from the exact file list' }
    if ($report.guiError) { throw $report.guiError }
    $report.status = 'passed'
} catch {
    $report.status = 'failed'
    $report.error = $_.ToString()
    throw
} finally {
    $report | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $Proof 'result.json') -Encoding utf8
}
