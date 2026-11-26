[CmdletBinding()]
param([Parameter(Mandatory)][string]$FixtureRoot,[Parameter(Mandatory)][string]$CandidateCode,
      [Parameter(Mandatory)][string]$BaselineCode,[switch]$Preflight)
$ErrorActionPreference='Stop'
$sid='S-1-5-21-1745080491-1157776599-3695790684-1002'
if ([guid](Get-CimInstance Win32_ComputerSystemProduct).UUID -ne [guid]'4B01C3A6-C913-41AC-B759-66148DFCAB64' -or
    [Security.Principal.WindowsIdentity]::GetCurrent().User.Value -cne 'S-1-5-21-1745080491-1157776599-3695790684-1001' -or
    !$FixtureRoot.StartsWith('C:\Users\mcw_qa_lab01\AppData\Local\MCWStage4-',[StringComparison]::OrdinalIgnoreCase)) {
    throw 'Unexpected observer guest, identity or QA fixture root.'
}
# PSScriptRoot is created by the controller with administrator/SYSTEM write
# rights and QA read-only rights; never execute code copied into QA's profile.
. (Join-Path $PSScriptRoot 'windows-hybrid-registration.ps1')
$allowed=@($CandidateCode,$BaselineCode)
$output=Join-Path $PSScriptRoot 'observations'
New-Item -ItemType Directory -Path $output | Out-Null
if ($Preflight) {
    $receipt=[ordered]@{scope='read-only preflight; no installation or uninstall performed';
        effectiveProcessPolicy=[string](Get-ExecutionPolicy);observerSid=[Security.Principal.WindowsIdentity]::GetCurrent().User.Value;
        candidate=(Get-McwMsiRegistration $CandidateCode $sid);baseline=(Get-McwMsiRegistration $BaselineCode $sid);
        candidateContexts=@(Get-McwMsiContexts $CandidateCode $sid);baselineContexts=@(Get-McwMsiContexts $BaselineCode $sid)}
    $receipt | ConvertTo-Json -Depth 14 | Set-Content -LiteralPath (Join-Path $output 'preflight.json') -Encoding utf8
    exit 0
}
try {
    $deadline=[DateTime]::UtcNow.AddMinutes(35)
    while ([DateTime]::UtcNow -lt $deadline -and !(Test-Path -LiteralPath (Join-Path $PSScriptRoot 'observer.stop'))) {
        foreach ($file in Get-ChildItem -LiteralPath "$FixtureRoot\proof" -Filter 'observe-*.json' -ErrorAction SilentlyContinue) {
            $phase=$file.BaseName.Substring(8)
            if ($phase -cnotin 'installed-baseline','installed-hybrid','removed-hybrid') {throw 'Unexpected observation phase.'}
            $destination=Join-Path $output "$phase.json"
            if (Test-Path -LiteralPath $destination) {continue}
            if ($file.Attributes -band [IO.FileAttributes]::ReparsePoint) {throw 'Redirected request file.'}
            $request=Get-Content -LiteralPath $file.FullName -Raw | ConvertFrom-Json
            if ($request.current -cnotin $allowed -or $request.other -cnotin $allowed -or $request.current -ceq $request.other) {throw 'Unexpected observation ProductCode.'}
            $receipt=[ordered]@{phase=$phase;targetSid=$sid;observerSid=[Security.Principal.WindowsIdentity]::GetCurrent().User.Value;
                current=(Get-McwMsiRegistration $request.current $sid);other=(Get-McwMsiRegistration $request.other $sid)}
            $receipt.current.contexts=@(Get-McwMsiContexts $request.current $sid)
            $receipt.other.contexts=@(Get-McwMsiContexts $request.other $sid)
            if (@($receipt.other.contexts | Where-Object status -eq 259).Count -ne 3 -or
                ($phase -ceq 'removed-hybrid' -and @($receipt.current.contexts | Where-Object status -eq 259).Count -ne 3) -or
                ($phase -cne 'removed-hybrid' -and
                  (@($receipt.current.contexts | Where-Object status -eq 0).Count -ne 1 -or
                   @($receipt.current.contexts | Where-Object { $_.status -eq 0 -and $_.context -eq 2 -and $_.sid -ceq $sid }).Count -ne 1))) {
                throw 'Unexpected Windows Installer context.'
            }
            $receipt | ConvertTo-Json -Depth 14 | Set-Content -LiteralPath "$destination.tmp" -Encoding utf8
            Move-Item -LiteralPath "$destination.tmp" -Destination $destination
        }
        Start-Sleep -Milliseconds 200
    }
} catch {
    $_ | Out-String | Set-Content -LiteralPath (Join-Path $output 'observer-error.txt') -Encoding utf8
    exit 1
}
