[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Candidate,
    [Parameter(Mandatory)][string]$Proof,
    [string]$SourceRevision = $env:GITHUB_SHA,
    [switch]$PositiveLifecycleOnly
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
$evidence = Join-Path $shared 'evidence'
New-Item -ItemType Directory -Path $evidence | Out-Null
& "$env:WINDIR\System32\icacls.exe" $shared /grant ('*{0}:(OI)(CI)(RX)' -f $user.SID.Value)
if ($LASTEXITCODE -ne 0) { throw 'Cannot grant the fixture read access' }
& "$env:WINDIR\System32\icacls.exe" $evidence /grant ('*{0}:(OI)(CI)(M)' -f $user.SID.Value)
if ($LASTEXITCODE -ne 0) { throw 'Cannot grant the fixture evidence access' }
$inner = Join-Path $shared 'windows-hybrid-install.ps1'
$credential = [Management.Automation.PSCredential]::new("$env:COMPUTERNAME\$account", $password)
$params = @{}
$params.FilePath = "$env:WINDIR\System32\WindowsPowerShell\v1.0\powershell.exe"
$params.Credential = $credential
$params.LoadUserProfile = $true
$params.WorkingDirectory = $shared
$params.PassThru = $true
$params.RedirectStandardOutput = Join-Path $evidence 'standard-stdout.log'
$params.RedirectStandardError = Join-Path $evidence 'standard-stderr.log'
$params.ArgumentList = '-NoProfile -NonInteractive -File "{0}" -Candidate "{1}" -Proof "{2}" -StandardAccountSid "{3}" -SourceRevision "{4}"' -f
    $inner,$installer,(Join-Path $evidence 'result'),$user.SID.Value,$SourceRevision
if ($PositiveLifecycleOnly) { $params.ArgumentList += ' -PositiveLifecycleOnly' }
$process = Start-Process @params
if (!$process.WaitForExit(420000)) { throw 'Standard-user fixture timed out; retain the live test runner state' }
Copy-Item -LiteralPath $evidence -Destination $Proof -Recurse
if ($process.ExitCode -ne 0) { throw "Standard-user fixture failed. See $Proof." }
$report = Get-Content -LiteralPath (Join-Path $Proof 'result\result.json') -Raw | ConvertFrom-Json
if ($report.status -cne 'passed' -or $report.standardUserQualified -ne $true -or $report.administratorToken -ne $false) {
    throw 'The standard-account run has no completed token and MSI proof'
}
