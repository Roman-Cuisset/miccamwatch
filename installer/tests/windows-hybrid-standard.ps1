[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Candidate,
    [Parameter(Mandatory)][string]$Proof,
    [string]$SourceRevision = $env:GITHUB_SHA,
    [switch]$PositiveLifecycleOnly,
    [switch]$HybridRollbackOnly
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
New-Item -ItemType Directory -Path $shared,(Join-Path $shared 'bundle') | Out-Null
$installer = Join-Path $shared 'candidate.msi'
Copy-Item -LiteralPath $Candidate -Destination $installer
Copy-Item -LiteralPath (Join-Path (Split-Path $Candidate) 'bundle\miccamwatch-bundle.json') -Destination (Join-Path $shared 'bundle')
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'windows-hybrid-install.ps1') -Destination $shared
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'windows-fixture-context.ps1') -Destination $shared
. (Join-Path $PSScriptRoot 'windows-hybrid-registration.ps1')
$evidence = Join-Path $shared 'evidence'
$observations = Join-Path $shared 'observations'
New-Item -ItemType Directory -Path $evidence,$observations | Out-Null
& "$env:WINDIR\System32\icacls.exe" $shared /grant ('*{0}:(OI)(CI)(RX)' -f $user.SID.Value)
if ($LASTEXITCODE -ne 0) { throw 'Cannot grant the fixture read access' }
& "$env:WINDIR\System32\icacls.exe" $evidence /grant ('*{0}:(OI)(CI)(M)' -f $user.SID.Value)
if ($LASTEXITCODE -ne 0) { throw 'Cannot grant the fixture evidence access' }
$inner = Join-Path $shared 'windows-hybrid-install.ps1'
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
$info.Arguments = '-NoProfile -NonInteractive -File "{0}" -Candidate "{1}" -Proof "{2}" -StandardAccountSid "{3}" -SourceRevision "{4}"' -f
    $inner,$installer,(Join-Path $evidence 'result'),$user.SID.Value,$SourceRevision
if ($PositiveLifecycleOnly) { $info.Arguments += ' -PositiveLifecycleOnly' }
if ($HybridRollbackOnly) { $info.Arguments += ' -HybridRollbackOnly' }
$info.Arguments += ' -ObserveRegistration'
$phases = 'installed-baseline','restored-baseline','installed-hybrid','restored-hybrid','removed-hybrid'
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
                $receipt.current = Get-McwMsiRegistration $ids.current $user.SID.Value
                $receipt.other = Get-McwMsiRegistration $ids.other $user.SID.Value
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
}
if ($exitCode -ne 0) { throw "Standard-user fixture returned $exitCode. See $Proof." }
$report = Get-Content -LiteralPath (Join-Path $Proof 'result\result.json') -Raw | ConvertFrom-Json
if ($report.status -cne 'passed' -or $report.standardUserQualified -ne $true -or $report.administratorToken -ne $false) {
    throw 'The standard-account run has no completed token and MSI proof'
}
