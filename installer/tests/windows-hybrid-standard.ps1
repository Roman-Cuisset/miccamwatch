[CmdletBinding(DefaultParameterSetName='Hybrid')]
param(
    [Parameter(Mandatory,ParameterSetName='Hybrid')][string]$Candidate,
    [Parameter(Mandatory,ParameterSetName='Probe')][string]$ProbePlan,
    [Parameter(Mandatory)][string]$Proof,
    [string]$SourceRevision = $env:GITHUB_SHA,
    [Parameter(ParameterSetName='Hybrid')][switch]$PositiveLifecycleOnly,
    [Parameter(ParameterSetName='Hybrid')][switch]$HybridRollbackOnly
)
$ErrorActionPreference = 'Stop'
if ($env:RUNNER_ENVIRONMENT -cne 'github-hosted' -or $env:GITHUB_ACTIONS -cne 'true') {
    throw 'Creating a standard-user fixture requires a disposable GitHub-hosted Windows runner.'
}
if (Test-Path -LiteralPath $Proof) { throw 'Standard-user evidence path must be new' }
$shared = Join-Path $env:PUBLIC ("McwStandard-" + [Guid]::NewGuid().ToString('N'))
$account = "mcwqa" + [Guid]::NewGuid().ToString('N').Substring(0,8)
$random = New-Object byte[] 32
$generator = [Security.Cryptography.RandomNumberGenerator]::Create()
try { $generator.GetBytes($random) }
finally { $generator.Dispose() }
$password = ConvertTo-SecureString ('Qa1!' + [Convert]::ToBase64String($random)) -AsPlainText -Force
[Array]::Clear($random, 0, $random.Length)
$user = New-LocalUser -Name $account -Password $password -Description 'MicCamWatch standard-user fixture'
if (!(@(Get-LocalGroupMember -SID 'S-1-5-32-545').SID.Value -contains $user.SID.Value)) {
    Add-LocalGroupMember -SID 'S-1-5-32-545' -Member $user
}
New-Item -ItemType Directory -Path $shared | Out-Null
$probePhases = @{}
if ($ProbePlan) {
    $plan = Get-Item -LiteralPath $ProbePlan
    $prefix = (Resolve-Path -LiteralPath $env:RUNNER_TEMP).Path.TrimEnd('\') + '\'
    if ($plan.Name -cne 'probes.json' -or !$plan.FullName.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Probe inputs must be built in the disposable runner temp directory'
    }
    $plans = @(Get-Content -LiteralPath $plan.FullName -Raw | ConvertFrom-Json)
    $probes = (New-Item -ItemType Directory -Path (Join-Path $shared 'probes')).FullName
    foreach ($case in $plans) {
        if ($case.Name -cnotmatch '^(x64|arm64)-(early|late)-(error|media)$') { throw 'Unknown probe case' }
        $destination = (New-Item -ItemType Directory -Path (Join-Path $probes $case.Name)).FullName
        foreach ($kind in 'baseline','fault','good') {
            $built = Join-Path $plan.DirectoryName "$($case.Name)\$kind\probe.msi"
            Copy-Item -LiteralPath $built -Destination (Join-Path $destination "$kind.msi")
        }
        foreach ($phase in 'baseline','rollback','retry','remove') { $probePhases["$($case.Name)-$phase"] = $case }
    }
    Copy-Item -LiteralPath $plan.FullName -Destination $probes
    $script = 'windows-msi-probe-install.ps1'
} else {
    $bundle = (New-Item -ItemType Directory -Path (Join-Path $shared 'bundle')).FullName
    $installer = Join-Path $shared 'candidate.msi'
    Copy-Item -LiteralPath $Candidate -Destination $installer
    Copy-Item -LiteralPath (Join-Path (Split-Path $Candidate) 'bundle\miccamwatch-bundle.json') -Destination $bundle
    $script = 'windows-hybrid-install.ps1'
}
Copy-Item -LiteralPath (Join-Path $PSScriptRoot $script) -Destination $shared
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'windows-fixture-context.ps1') -Destination $shared
. (Join-Path $PSScriptRoot 'windows-hybrid-registration.ps1')
$evidence = Join-Path $shared 'evidence'
$observations = Join-Path $shared 'observations'
New-Item -ItemType Directory -Path $evidence,$observations | Out-Null
& "$env:WINDIR\System32\icacls.exe" $shared /grant ('*{0}:(OI)(CI)(RX)' -f $user.SID.Value)
if ($LASTEXITCODE -ne 0) { throw 'Cannot grant the fixture read access' }
& "$env:WINDIR\System32\icacls.exe" $evidence /grant ('*{0}:(OI)(CI)(M)' -f $user.SID.Value)
if ($LASTEXITCODE -ne 0) { throw 'Cannot grant the fixture evidence access' }
$inner = Join-Path $shared $script
# Retain the process handle returned by CreateProcessWithLogonW. PowerShell 5
# Start-Process can return a Process without a readable ExitCode after exit.
$info = [Diagnostics.ProcessStartInfo]::new("$env:WINDIR\System32\WindowsPowerShell\v1.0\powershell.exe")
$info.UserName = $account
$info.Domain = $env:COMPUTERNAME
$info.Password = $password
$info.LoadUserProfile = $true
$info.UseShellExecute = $false
$info.CreateNoWindow = $true
$info.WorkingDirectory = $shared
$info.RedirectStandardOutput = $true
$info.RedirectStandardError = $true
# Alternate-credential launch normally receives a new logon environment.
# Forward the verified runner markers and system entries only. User profile
# paths are resolved by the child for the loaded standard account.
$info.EnvironmentVariables.Clear()
$systemEnvironment = 'WINDIR','SystemRoot','SystemDrive','PUBLIC','ProgramData','ProgramFiles','ProgramFiles(x86)',
    'CommonProgramFiles','CommonProgramFiles(x86)','ALLUSERSPROFILE','COMPUTERNAME','PROCESSOR_ARCHITECTURE','ComSpec','PATHEXT'
foreach ($name in ('RUNNER_ENVIRONMENT','GITHUB_ACTIONS' + $systemEnvironment)) {
    $value = [Environment]::GetEnvironmentVariable($name)
    if ($value) { $info.EnvironmentVariables[$name] = $value }
}
$info.EnvironmentVariables['Path'] = "$env:WINDIR\System32;$env:WINDIR;$env:WINDIR\System32\Wbem;$env:WINDIR\System32\WindowsPowerShell\v1.0"
$info.EnvironmentVariables['USERNAME'] = $account
$info.EnvironmentVariables['USERDOMAIN'] = $env:COMPUTERNAME
$result = Join-Path $evidence 'result'
$sampler = $null
if ($ProbePlan) {
    $info.Arguments = '-NoProfile -NonInteractive -File "{0}" -Plan "{1}" -Proof "{2}" -StandardAccountSid "{3}"' -f
        $inner,(Join-Path $probes 'probes.json'),$result,$user.SID.Value
    $phases = $probePhases.Keys
    $serviceBefore = Get-CimInstance Win32_Service -Filter "Name='msiserver'" | Select-Object Name,StartName,State,ProcessId
    Add-Type -Path (Join-Path $PSScriptRoot 'windows-msi-token-samples.cs')
    $sampler = [McwMsiTokenSamples]::new($user.SID.Value, $env:WINDIR)
} else {
    $info.Arguments = '-NoProfile -NonInteractive -File "{0}" -Candidate "{1}" -Proof "{2}" -StandardAccountSid "{3}" -SourceRevision "{4}" -ObserveRegistration' -f
        $inner,$installer,$result,$user.SID.Value,$SourceRevision
    if ($PositiveLifecycleOnly) { $info.Arguments += ' -PositiveLifecycleOnly' }
    if ($HybridRollbackOnly) { $info.Arguments += ' -HybridRollbackOnly' }
    $phases = 'installed-baseline','restored-baseline','installed-hybrid','restored-hybrid','removed-hybrid'
}
$observed = @{}
$process = [Diagnostics.Process]::Start($info)
try {
    $stdout = $process.StandardOutput.ReadToEndAsync()
    $stderr = $process.StandardError.ReadToEndAsync()
    $deadline = [Diagnostics.Stopwatch]::StartNew()
    while (!$process.WaitForExit(200)) {
        if ($deadline.Elapsed.TotalMinutes -gt 10) { throw 'Standard-user fixture timed out; retain the live test runner state' }
        foreach ($phase in $phases) {
            if ($observed.ContainsKey($phase)) { continue }
            $request = Get-Item -LiteralPath (Join-Path $evidence "result\observe-$phase.json") -ErrorAction SilentlyContinue
            if (!$request) { continue }
            $receipt = [ordered]@{ phase = $phase; targetSid = $user.SID.Value; observerSid = [Security.Principal.WindowsIdentity]::GetCurrent().User.Value }
            try {
                if ($request.PSIsContainer -or $request.Length -gt 512 -or
                    ($request.Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Invalid observation request file' }
                $ids = Get-Content -LiteralPath $request.FullName -Raw | ConvertFrom-Json
                if ($ProbePlan) {
                    $case = $probePhases[$phase]
                    if ($ids.current -cne $case.OldCode -or $ids.other -cne $case.NewCode) { throw 'Probe identity mismatch' }
                    $receipt.current = Get-McwMsiRegistration $case.OldCode $user.SID.Value $case.UpgradeCode
                    $receipt.other = Get-McwMsiRegistration $case.NewCode $user.SID.Value $case.UpgradeCode
                    $receipt.current.contexts = @(Get-McwMsiContexts $case.OldCode $user.SID.Value)
                    $receipt.other.contexts = @(Get-McwMsiContexts $case.NewCode $user.SID.Value)
                } else {
                    $receipt.current = Get-McwMsiRegistration $ids.current $user.SID.Value
                    $receipt.other = Get-McwMsiRegistration $ids.other $user.SID.Value
                }
            } catch { $receipt.error = $_.ToString() }
            $file = Join-Path $observations "$phase.json"
            $receipt | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath "$file.tmp" -Encoding utf8
            Move-Item -LiteralPath "$file.tmp" -Destination $file
            $observed[$phase] = $true
        }
    }
    $exitCode = $process.ExitCode
    if (!$stdout.Wait(5000) -or !$stderr.Wait(5000)) { throw 'Standard-user fixture streams did not close' }
    [IO.File]::WriteAllText((Join-Path $evidence 'standard-stdout.log'), $stdout.Result)
    [IO.File]::WriteAllText((Join-Path $evidence 'standard-stderr.log'), $stderr.Result)
} finally {
    $process.Dispose()
    Copy-Item -LiteralPath $evidence -Destination $Proof -Recurse
    Copy-Item -LiteralPath $observations -Destination $Proof -Recurse
    if ($sampler) {
        try {
            [ordered]@{
                source=$SourceRevision; fixtureSid=$user.SID.Value; observerPid=$PID;
                serviceBefore=$serviceBefore; timeZone=[TimeZoneInfo]::Local.Id;
                osBuild=(Get-CimInstance Win32_OperatingSystem).BuildNumber;
                serviceAfter=(Get-CimInstance Win32_Service -Filter "Name='msiserver'" | Select-Object Name,StartName,State,ProcessId);
                samples=@($sampler.Finish())
            } | ConvertTo-Json -Depth 7 | Set-Content -LiteralPath (Join-Path $Proof 'msi-token-samples.json') -Encoding utf8
        } finally { $sampler.Dispose() }
    }
}
if ($exitCode -ne 0) { throw "Standard-user fixture returned $exitCode. See $Proof." }
$report = Get-Content -LiteralPath (Join-Path $Proof 'result\result.json') -Raw | ConvertFrom-Json
if ($ProbePlan) {
    if ($report.status -cne 'passed') { throw 'The probe completed without a passing receipt' }
    return
}
if ($report.status -cne 'passed' -or $report.standardUserQualified -ne $true -or $report.administratorToken -ne $false) {
    throw 'The standard-account run has no completed token and MSI proof'
}
