param(
    [Parameter(Mandatory = $true)][string]$MsiPath,
    [Parameter(Mandatory = $true)][string]$PayloadDirectory,
    [Parameter(Mandatory = $true)][string]$OutputDirectory,
    [switch]$IsolatedRunner
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
# Real package operations are restricted to a disposable hosted VM. Never invoke
# this fixture on a user's PC; the production UpgradeCode can replace real MCW.
if (-not $IsolatedRunner -or $env:OS -ne 'Windows_NT' -or
    $env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted') {
    throw 'Explicitly isolated GitHub-hosted Windows VM required.'
}
$repository = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$msi = [IO.Path]::GetFullPath($MsiPath)
$payload = [IO.Path]::GetFullPath($PayloadDirectory)
$proof = [IO.Path]::GetFullPath($OutputDirectory)
$temp = [IO.Path]::GetFullPath($env:RUNNER_TEMP).TrimEnd('\') + '\'
if (-not $proof.StartsWith($temp, [StringComparison]::OrdinalIgnoreCase) -or (Test-Path -LiteralPath $proof)) {
    throw 'Proof directory must be new and beneath RUNNER_TEMP.'
}
if (-not $msi.StartsWith(($repository + '\target\'), [StringComparison]::OrdinalIgnoreCase) -or
    [IO.Path]::GetExtension($msi) -ne '.msi' -or -not (Test-Path -LiteralPath $msi -PathType Leaf)) {
    throw 'Only this private checkout target-directory MSI may be qualified.'
}
foreach ($path in @($msi, $payload, $proof)) {
    if ($path.Contains('"') -or $path.Contains("`r") -or $path.Contains("`n")) { throw 'Unsafe fixture path.' }
    $ancestor = $path
    while ($ancestor) {
        if ((Test-Path -LiteralPath $ancestor) -and
            ((Get-Item -LiteralPath $ancestor).Attributes -band [IO.FileAttributes]::ReparsePoint)) {
            throw "Reparse-point fixture ancestry: $ancestor"
        }
        $ancestor = Split-Path -Parent $ancestor
    }
}
[void](New-Item -ItemType Directory -Path $proof)
$installDirectory = Join-Path $proof 'installation'
$shortcut = Join-Path ([Environment]::GetFolderPath('Programs')) 'MicCamWatch.lnk'
$expectedPackage = 'F37927EA1681738F0FDF49377F210812F1C0B8BF80489FB01900A3C59F5CCB66'
$expectedPayload = @{
    'mcw.exe' = '402CD89BD19E8F1853E4436A884A2BE47D8A3B15F08E954A084EA8B1BDF92CE1'
    'mcw-tray.exe' = 'FAD3610978867902EB7EA8846EF789128B66D035FDB626D667EF1DB5E6630714'
}
$report = [ordered]@{
    status = 'running'; scope = 'Actual perUser MSI; OS-restricted same-user console consumer, not ordinary desktop'
    artifactRun = 39625449451; packageSha256 = $null; productCode = $null; registration = $null
    installation = $installDirectory; observerSid = [Security.Principal.WindowsIdentity]::GetCurrent().User.Value
    consumer = $null; cleanup = $null; error = $null
}
$installer = $null
$productCode = $null
$installAttempted = $false
$script:pendingInstaller = $false
$failure = $null
function Save-Report { $report | ConvertTo-Json -Depth 16 | Set-Content -LiteralPath (Join-Path $proof 'result.json') -Encoding UTF8 }
function Get-UserPathSnapshot {
    $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $false)
    $present = $false; $kind = $null; $raw = $null
    try {
        if ($null -ne $key) {
            foreach ($name in $key.GetValueNames()) {
                if ([string]::Equals($name, 'PATH', [StringComparison]::OrdinalIgnoreCase)) {
                    $present = $true
                    $kind = $key.GetValueKind($name).ToString()
                    $raw = $key.GetValue($name, $null, [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
                    break
                }
            }
        }
        return @{ observerSid = [Security.Principal.WindowsIdentity]::GetCurrent().User.Value
                  keyExists = ($null -ne $key); valueExists = $present; valueKind = $kind; rawValue = $raw }
    } finally { if ($null -ne $key) { $key.Dispose() } }
}
function Invoke-OwnedMsi([string]$Operation, [string]$Subject, [string]$Log, [string]$Properties = '') {
    $arguments = "$Operation `"$Subject`" /qn /norestart /L*V `"$Log`" $Properties"
    $process = Start-Process -FilePath (Join-Path $env:WINDIR 'System32/msiexec.exe') -ArgumentList $arguments -PassThru
    $null = $process.Handle
    try {
        if (-not $process.WaitForExit(180000)) {
            $script:pendingInstaller = $true
            $report['pendingInstaller'] = @{ pid = $process.Id; log = $Log }
            throw 'Owned msiexec deadline; left alive, no cleanup or forced stop.'
        }
        $process.Refresh()
        if ($process.ExitCode -notin @(0, 3010)) { throw "Owned msiexec exited $($process.ExitCode); inspect $Log" }
        return $process.ExitCode
    } finally { $process.Dispose() }
}
$report['userPathBeforeSnapshot'] = Get-UserPathSnapshot
Save-Report
try {
    $report.packageSha256 = (Get-FileHash -LiteralPath $msi -Algorithm SHA256).Hash
    if ($report.packageSha256 -cne $expectedPackage) { throw 'Retained genuine MSI hash mismatch; refusing install.' }
    foreach ($name in $expectedPayload.Keys) {
        if ((Get-FileHash -LiteralPath (Join-Path $payload $name) -Algorithm SHA256).Hash -cne $expectedPayload[$name]) {
            throw "Retained genuine payload hash mismatch: $name"
        }
    }
    $installer = New-Object -ComObject WindowsInstaller.Installer
    $database = $installer.OpenDatabase($msi, 0)
    $view = $database.OpenView('SELECT `Property`, `Value` FROM `Property`')
    $properties = @{}
    try {
        $view.Execute()
        while ($null -ne ($record = $view.Fetch())) {
            try { $properties[$record.StringData(1)] = $record.StringData(2) }
            finally { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($record) }
        }
    } finally {
        $view.Close()
        [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($view)
        [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($database)
    }
    $productCode = $properties['ProductCode']
    if ($properties['ProductName'] -ne 'MicCamWatch' -or $properties['ProductVersion'] -ne '0.20.0' -or
        $productCode -notmatch '^\{[0-9A-Fa-f-]{36}\}$' -or
        $properties['UpgradeCode'] -ne '{8FC9317B-4517-4E83-9F52-CF3257584BF6}') {
        throw 'Actual package identity is not the retained owned v0.20.0 MSI.'
    }
    $report.productCode = $productCode
    $related = $installer.RelatedProducts($properties['UpgradeCode'])
    try {
        $relatedCount = $related.GetType().InvokeMember('Count', [Reflection.BindingFlags]::GetProperty, $null, $related, $null)
        if ($relatedCount -isnot [int]) { throw 'Related MSI product count unreadable; refusing install.' }
    } finally { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($related) }
    if ($installer.ProductState($productCode) -ne -1 -or $relatedCount -ne 0 -or
        (Test-Path -LiteralPath $shortcut) -or
        (Test-Path -LiteralPath (Join-Path ([Environment]::GetFolderPath('CommonPrograms')) 'MicCamWatch.lnk'))) {
        throw 'Existing MCW registration/shortcut: refusing fixture installation.'
    }
    $installAttempted = $true
    # Preserve authored perUser scope: do not force ALLUSERS or substitute machine registration.
    $report['installExitCode'] = Invoke-OwnedMsi '/i' $msi (Join-Path $proof 'install-msi.log') "INSTALLFOLDER=`"$installDirectory`""
    if ($installer.ProductState($productCode) -ne 5) { throw 'Real MSI is not locally installed.' }
    $location = [string]$installer.ProductInfo($productCode, 'InstallLocation')
    $assignment = [string]$installer.ProductInfo($productCode, 'AssignmentType')
    if ($assignment -cne '0' -or
        -not [string]::Equals($location.TrimEnd('\'), $installDirectory, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Actual registration is not exact-location perUser assignment0.'
    }
    $report.registration = @{ productCode = $productCode; assignmentType = $assignment; installLocation = $location
                              displayName = $installer.ProductInfo($productCode, 'ProductName')
                              version = $installer.ProductInfo($productCode, 'VersionString')
                              localPackage = $installer.ProductInfo($productCode, 'LocalPackage')
                              observerSid = $report.observerSid }
    foreach ($name in $expectedPayload.Keys) {
        if ((Get-FileHash -LiteralPath (Join-Path $installDirectory $name) -Algorithm SHA256).Hash -cne $expectedPayload[$name]) {
            throw "Actual installed payload differs from retained genuine artifact: $name"
        }
    }
    if (-not (Test-Path -LiteralPath $shortcut -PathType Leaf)) { throw 'Real MSI user shortcut missing.' }
    $report['userPathInstalledSnapshot'] = Get-UserPathSnapshot
    Save-Report
    $python = (Get-Command python).Source
    $helper = Join-Path $PSScriptRoot 'windows_msi_restricted.py'
    & $python -B $helper --binary (Join-Path $installDirectory 'mcw.exe') --product $productCode --proof $proof
    if ($LASTEXITCODE -ne 0) { throw 'Restricted same-user MSI consumer failed; inspect tokens.json/consumer.json.' }
    $consumer = Get-Content -LiteralPath (Join-Path $proof 'consumer.json') -Raw | ConvertFrom-Json
    if ($consumer.status -ne 'passed' -or $consumer.actual_process_token.token_elevation -ne 0 -or
        $consumer.actual_process_token.admin_group_enabled -or $consumer.actual_process_token.sid -cne $report.observerSid) {
        throw 'Actual consumer did not prove eligible SDK token and exact product delegation.'
    }
    $report.consumer = $consumer
    # The consumer must not have removed/reassigned the package or its PATH/shortcut.
    if ($installer.ProductState($productCode) -ne 5 -or
        [string]$installer.ProductInfo($productCode, 'AssignmentType') -cne '0' -or
        -not (Test-Path -LiteralPath $shortcut)) { throw 'Consumer changed manager-owned registration/shortcut.' }
    $pathAfterConsumer = Get-UserPathSnapshot
    foreach ($field in @('observerSid', 'keyExists', 'valueExists', 'valueKind', 'rawValue')) {
        if ($pathAfterConsumer[$field] -cne $report.userPathInstalledSnapshot[$field]) {
            throw "Consumer changed user PATH field: $field"
        }
    }
    $report.status = 'consumer-passed-manager-removal-pending'
} catch {
    $failure = $_
    $report.status = 'failed'
    $report.error = $_.ToString() + "`n" + $_.ScriptStackTrace
} finally {
    # Scoped cleanup follows the branding fixture: only our absent-before product,
    # never a running/unknown MSI or still-live consumer. No force-kills or repairs.
    if ($script:pendingInstaller -or (Test-Path -LiteralPath (Join-Path $proof 'pending-consumer.json'))) {
        $report.cleanup = @{ status = 'pending-child-retained'; productCode = $productCode }
        $report.status = 'failed'
    } elseif ($installAttempted -and $productCode -and $null -ne $installer) {
        try {
            if ((Get-FileHash -LiteralPath $msi -Algorithm SHA256).Hash -cne $expectedPackage) { throw 'Package changed; refusing cleanup.' }
            if (Test-Path -LiteralPath $installDirectory) {
                Copy-Item -LiteralPath $installDirectory -Destination (Join-Path $proof 'installed-before-cleanup') -Recurse
            }
            if ($installer.ProductState($productCode) -ne -1) {
                $exitCode = Invoke-OwnedMsi '/x' $productCode (Join-Path $proof 'uninstall-msi.log')
                $report.cleanup = @{ status = 'uninstalled'; productCode = $productCode; exitCode = $exitCode }
            } else { $report.cleanup = @{ status = 'not-registered'; productCode = $productCode } }
            if ($installer.ProductState($productCode) -ne -1 -or (Test-Path -LiteralPath $shortcut) -or
                (Test-Path -LiteralPath (Join-Path $installDirectory 'mcw.exe')) -or
                (Test-Path -LiteralPath (Join-Path $installDirectory 'mcw-tray.exe'))) {
                throw 'Real manager removal retained product registration, shortcut or payload.'
            }
            $after = Get-UserPathSnapshot
            $report['userPathAfterSnapshot'] = $after
            $before = $report.userPathBeforeSnapshot
            $beforeEntries = @(([string]$before.rawValue).Split(';') | Where-Object { $_.Length -gt 0 })
            $afterEntries = @(([string]$after.rawValue).Split(';') | Where-Object { $_.Length -gt 0 })
            if ($beforeEntries.Count -ne $afterEntries.Count) { throw 'Manager removal lost/retained user PATH entries.' }
            for ($i = 0; $i -lt $beforeEntries.Count; $i++) {
                if ($beforeEntries[$i] -cne $afterEntries[$i]) { throw 'Manager removal changed/reordered raw user PATH.' }
            }
            if ($beforeEntries.Count -gt 0 -and $before.valueKind -cne $after.valueKind) { throw 'Manager removal changed PATH interpretation.' }
            $report['userPathEntriesPreserved'] = $true
            if ($null -ne $report.consumer) {
                foreach ($item in $report.consumer.data) {
                    if ((Get-FileHash -LiteralPath $item.path -Algorithm SHA256).Hash -ine $item.sha256) {
                        throw 'Actual MSI removal changed retained user data.'
                    }
                }
                if ($report.status -eq 'consumer-passed-manager-removal-pending') {
                    $report['retainedDataPreserved'] = $true
                    $report.status = 'passed'
                }
            }
        } catch {
            $report.status = 'failed'
            $report.cleanup = @{ status = 'failed'; error = $_.ToString() }
            if ($null -eq $failure) { $failure = $_ }
        }
    }
    if ($null -ne $installer) { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($installer) }
    Save-Report
}
if ($null -ne $failure) { throw $failure }
if ($report.status -ne 'passed') { throw 'MSI qualification incomplete; no pass claimed.' }
Write-Output 'Actual perUser MSI delegation/removal passed for the recorded restricted same-user token mode.'
