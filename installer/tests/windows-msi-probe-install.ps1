[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Plan,
    [Parameter(Mandatory)][string]$Proof,
    [Parameter(Mandatory)][string]$StandardAccountSid,
    [switch]$TransactionProbe
)
$ErrorActionPreference = 'Stop'
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
if ($env:RUNNER_ENVIRONMENT -cne 'github-hosted' -or $env:GITHUB_ACTIONS -cne 'true' -or
    $identity.User.Value -cne $StandardAccountSid -or $identity.Name -notmatch '\\mcwqa[0-9a-f]{8}$' -or
    !$PSScriptRoot.StartsWith((Join-Path $env:PUBLIC 'McwStandard-'), [StringComparison]::OrdinalIgnoreCase) -or
    $identity.Groups.Value -contains 'S-1-5-32-544') {
    throw 'Probe requires the hosted wrapper, its genuine fixture SID and disposable prefix'
}
$env:USERPROFILE = [Environment]::GetFolderPath('UserProfile')
$env:APPDATA = [Environment]::GetFolderPath('ApplicationData', 'Create')
$env:LOCALAPPDATA = [Environment]::GetFolderPath('LocalApplicationData', 'Create')
$env:TEMP = (New-Item -ItemType Directory -Path (Join-Path $env:LOCALAPPDATA 'Temp') -Force).FullName
$env:TMP = $env:TEMP
New-Item -ItemType Directory -Path $Proof | Out-Null
. (Join-Path $PSScriptRoot 'windows-fixture-context.ps1')
$context = Get-McwStandardUserContext
if ($context.elevated -or $context.elevationType -ne 1 -or $context.restricted -or
    $context.tokenType -ne 1 -or $context.integritySid -cne 'S-1-16-8192' -or
    !$context.profileHiveLoaded -or $context.accountLocalGroupSids -contains 'S-1-5-32-544') {
    throw 'Probe invoker does not have a medium-integrity standard primary token and loaded profile'
}
$cases = Get-Content -LiteralPath $Plan -Raw | ConvertFrom-Json
if ($TransactionProbe) {
    if (@($cases).Count -ne 1 -or $cases[0].Name -cne 'x64-early-media' -or $cases[0].Failure -cne 'native-media') {
        throw 'Only the existing x64 missing-cabinet fixture is admitted for this experiment'
    }
    Add-Type -Path (Join-Path $PSScriptRoot 'windows-msi-transaction-probe.cs')
}
$report = [ordered]@{ client=$context; processId=$PID; parentProcessId=(Get-CimInstance Win32_Process -Filter "ProcessId=$PID").ParentProcessId; cases=@() }
if ($TransactionProbe) {
    $machine = Get-ItemProperty -LiteralPath 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Installer' -ErrorAction SilentlyContinue
    $userPolicy = Get-ItemProperty -LiteralPath 'HKCU:\SOFTWARE\Policies\Microsoft\Windows\Installer' -ErrorAction SilentlyContinue
    $report.installerPolicy = @{
        machineDisableMsi=$machine.DisableMSI; machineDisableRollback=$machine.DisableRollback;
        userDisableRollback=$userPolicy.DisableRollback
    }
}

function Invoke-ProbeMsi([string]$Operation, [string]$Package, [string]$Log) {
    $args = '{0} "{1}" /qn /norestart /l*v "{2}"' -f $Operation,$Package,$Log
    $info = [Diagnostics.ProcessStartInfo]::new("$env:WINDIR\System32\msiexec.exe", $args)
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $job = [Diagnostics.Process]::Start($info)
    try {
        $start = [DateTime]::UtcNow.ToString('O')
        if (!$job.WaitForExit(90000)) { throw 'Installer timed out; its fixture is retained' }
        return @{ pid=$job.Id; firstUtc=$start; lastUtc=[DateTime]::UtcNow.ToString('O'); exitCode=$job.ExitCode }
    } finally { $job.Dispose() }
}

function Read-Registration([string]$Phase, $Case) {
    $label = "$($Case.Name)-$Phase"
    $request = Join-Path $Proof "observe-$label.json"
    @{ current=$Case.OldCode; other=$Case.NewCode } | ConvertTo-Json | Set-Content -LiteralPath "$request.tmp" -Encoding utf8
    Move-Item -LiteralPath "$request.tmp" -Destination $request
    $answer = Join-Path $PSScriptRoot "observations\$label.json"
    $timer = [Diagnostics.Stopwatch]::StartNew()
    while (!(Test-Path -LiteralPath $answer -PathType Leaf)) {
        if ($timer.Elapsed.TotalSeconds -gt 30) { throw "Independent observer did not answer $label" }
        Start-Sleep -Milliseconds 100
    }
    $receipt = Get-Content -LiteralPath $answer -Raw | ConvertFrom-Json
    if ($receipt.error -or $receipt.phase -cne $label -or $receipt.targetSid -cne $StandardAccountSid -or
        !$receipt.observerSid -or $receipt.observerSid -eq $StandardAccountSid) { throw "Invalid receipt for $label" }
    return $receipt
}

function Test-Registered($Entry, [string]$Version, [string]$Location, [string]$UpgradeCode) {
    return ($Entry.properties.State.status -eq 0 -and $Entry.properties.State.value -ceq '5' -and
        $Entry.properties.VersionString.value -ceq $Version -and
        ([string]$Entry.properties.InstallLocation.value).TrimEnd('\') -ieq $Location -and
        $Entry.productInUserUpgradeIndex -and !$Entry.upgradeIndexError -and !$Entry.cacheError -and
        $Entry.cacheIdentity.ProductCode -ieq $Entry.productCode -and $Entry.cacheIdentity.UpgradeCode -ieq $UpgradeCode -and
        $Entry.cacheIdentity.ProductVersion -ceq $Version -and
        @($Entry.contexts | Where-Object status -eq 0).Count -eq 1 -and
        @($Entry.contexts | Where-Object { $_.status -eq 0 -and $_.context -eq 2 -and $_.sid -ceq $StandardAccountSid }).Count -eq 1)
}

function Test-Absent($Entry) {
    return ($Entry.properties.State.status -eq 1605 -and !$Entry.productInUserUpgradeIndex -and
        !$Entry.upgradeIndexError -and @($Entry.contexts | Where-Object status -eq 259).Count -eq 3)
}

foreach ($case in $cases) {
    $item = [ordered]@{ name=$case.Name; platform=$case.Arch; schedule=$case.Schedule; failure=$case.Failure; status='failed' }
    $report.cases += $item
    $work = (New-Item -ItemType Directory -Path (Join-Path $Proof $case.Name)).FullName
    try {
        if ($TransactionProbe -and ($machine.DisableMSI -in 1,2 -or
            $machine.DisableRollback -eq 1 -or $userPolicy.DisableRollback -eq 1)) {
            throw 'Machine MSI policy or rollback policy refuses this per-user experiment'
        }
        $source = Join-Path $PSScriptRoot "probes\$($case.Name)"
        $base = Join-Path $env:LOCALAPPDATA $case.TargetFolder
        $marker = Join-Path $base 'marker.txt'
        if (Test-Path -LiteralPath $base) { throw 'Unexpected existing probe component path' }
        $old = Join-Path $source 'baseline.msi'
        $fault = Join-Path $source 'fault.msi'
        $good = Join-Path $source 'good.msi'
        foreach ($pair in @(@($old,$case.OldHash),@($fault,$case.FaultHash),@($good,$case.GoodHash))) {
            if ((Get-FileHash -LiteralPath $pair[0] -Algorithm SHA256).Hash -cne $pair[1]) { throw 'Unexpected probe package bytes' }
        }
        $item.install = Invoke-ProbeMsi '/i' $old (Join-Path $work 'baseline.log')
        if ($item.install.exitCode -ne 0) { throw "Baseline returned $($item.install.exitCode)" }
        $item.before = Read-Registration 'baseline' $case
        if (!(Test-Registered $item.before.current '1.0.0' $base $case.UpgradeCode) -or !(Test-Absent $item.before.other)) {
            throw 'Baseline did not register exclusively in the expected fixture context'
        }
        $hash = (Get-FileHash -LiteralPath $marker -Algorithm SHA256).Hash
        $log = Join-Path $work 'failure.log'
        if ($TransactionProbe) {
            $item.transaction = [McwMsiTransactionProbe]::FailUpgrade($case.OldCode, $fault, $marker, $log)
        } else {
            $item.fault = Invoke-ProbeMsi '/i' $fault $log
        }
        $item.after = Read-Registration 'rollback' $case
        $item.registered = Test-Registered $item.after.current '1.0.0' $base $case.UpgradeCode
        $item.candidateAbsent = Test-Absent $item.after.other
        $item.markerRestored = (Test-Path -LiteralPath $marker -PathType Leaf) -and (Get-FileHash -LiteralPath $marker -Algorithm SHA256).Hash -ceq $hash
        $journal = if (Test-Path -LiteralPath $log -PathType Leaf) { Get-Content -LiteralPath $log -Raw } else { '' }
        $item.deniedRegistry = @($journal -split '\r?\n' | Where-Object { $_ -match 'Note: 1: 1401 .* 3: (5|1307)\s*$' }).Count
        $failure = if ($case.Failure -eq 'native-media') { '(?:Note: 1: 1311|Error 1311)[^\r\n]*probe-media\.cab' } else { 'Action ended [^\r\n]+ProbeFailure\. Return value 3' }
        if ($TransactionProbe) {
            $item.qualifiedFault = $item.transaction.BeginCode -eq 0 -and $item.transaction.RemoveCode -eq 0 -and
                $item.transaction.OldMarkerPresentAfterRemoval -eq $false -and $item.transaction.InstallCode -eq 1603 -and
                $journal -match $failure
        } else {
            $item.qualifiedFault = $item.fault.exitCode -eq 1603 -and $journal -match
                ("(?s)Action ended [^\r\n]+RemoveExistingProducts\. Return value 1\..*?$failure.*?Executing op: Header\([^\r\n]*ScriptType=2,")
        }
        if (!$item.qualifiedFault) { throw 'Expected native rollback after previous-product removal was not observed' }
        if ($TransactionProbe -and $item.transaction.EndCode -ne 0) { throw 'The native transaction did not confirm rollback completion' }
        $item.rollbackVerified = $item.registered -and $item.markerRestored -and $item.candidateAbsent
        if (!$item.rollbackVerified) {
            if (!$TransactionProbe -or !$item.markerRestored -or !$item.candidateAbsent -or !(Test-Absent $item.after.current)) {
                throw 'Rollback did not restore the complete old product'
            }
            # An independent recovery experiment, confined to this text fixture.
            # Preserve the failed rollback receipt and keep the case failed even
            # when reinstalling the exact baseline restores normal maintenance.
            $item.reinstall = Invoke-ProbeMsi '/i' $old (Join-Path $work 'reinstall-baseline.log')
            $item.afterReinstall = Read-Registration 'reinstalled' $case
            if ($item.reinstall.exitCode -ne 0 -or
                !(Test-Registered $item.afterReinstall.current '1.0.0' $base $case.UpgradeCode) -or
                !(Test-Absent $item.afterReinstall.other) -or
                (Get-FileHash -LiteralPath $marker -Algorithm SHA256).Hash -cne $hash) {
                throw 'Baseline reinstall did not restore complete registration and payload'
            }
        }
        $item.retry = Invoke-ProbeMsi '/i' $good (Join-Path $work 'retry.log')
        $item.afterRetry = Read-Registration 'retry' $case
        if ($item.retry.exitCode -ne 0 -or !(Test-Absent $item.afterRetry.current) -or
            !(Test-Registered $item.afterRetry.other '2.0.0' $base $case.UpgradeCode) -or
            (Get-Content -LiteralPath $marker -Raw) -cnotmatch 'version 2\.0\.0$') { throw 'Retry did not install the candidate product' }
        $item.remove = Invoke-ProbeMsi '/x' $case.NewCode (Join-Path $work 'uninstall.log')
        $item.afterRemoval = Read-Registration 'remove' $case
        if ($item.remove.exitCode -ne 0 -or !(Test-Absent $item.afterRemoval.current) -or
            !(Test-Absent $item.afterRemoval.other) -or (Test-Path -LiteralPath $marker)) { throw 'Retry product did not uninstall cleanly' }
        if (!$item.rollbackVerified) {
            $item.maintenanceAfterReinstall = $true
            throw 'Native rollback failed; the separate baseline reinstall, retry and removal passed'
        }
        $item.status = 'passed'
    } catch { $item.error = $_.ToString() }
}
$report.status = if (@($report.cases | Where-Object status -ne 'passed').Count) { 'failed' } else { 'passed' }
$report | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath (Join-Path $Proof 'result.json') -Encoding utf8
if ($report.status -ne 'passed') { exit 1 }
