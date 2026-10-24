param(
    [string]$AcknowledgeDisposableVm = 'false'
)

# This script never provisions accounts, changes services, elevates, or launches
# a batch-token worker. Its caller is the real logged-in disposable guest user.
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

function Write-FixtureJson {
    param([string]$Path, $Value)
    [IO.File]::WriteAllText($Path, (($Value | ConvertTo-Json -Depth 30) + "`n"), (New-Object Text.UTF8Encoding($false)))
}

function Assert-OwnedDirectory {
    param([string]$Path)
    if (-not [IO.Path]::IsPathRooted($Path) -or -not (Test-Path -LiteralPath $Path -PathType Container)) {
        throw 'The real runner directory must already exist at an absolute path.'
    }
    $item = Get-Item -LiteralPath $Path -Force
    $sid = [Security.Principal.WindowsIdentity]::GetCurrent().User.Value
    $owner = (Get-Acl -LiteralPath $item.FullName).GetOwner([Security.Principal.SecurityIdentifier]).Value
    if ($owner -cne $sid) { throw 'RUNNER_TEMP must actually be owned by the logged-in standard account.' }
    for ($ancestor = $item; $null -ne $ancestor; $ancestor = $ancestor.Parent) {
        if (($ancestor.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw 'Runner directories must not traverse a junction or other reparse point.'
        }
    }
    return $item.FullName
}

function Assert-NoExistingProductOrData {
    if (@(Get-Process -Name 'mcw', 'mcw-tray' -ErrorAction SilentlyContinue).Count -ne 0) {
        throw 'Existing MCW processes: refusing to touch this guest.'
    }
    $paths = @(
        (Join-Path $env:APPDATA 'MicCamWatch'),
        (Join-Path $env:LOCALAPPDATA 'MicCamWatch'),
        (Join-Path $env:LOCALAPPDATA 'Programs/MicCamWatch'),
        (Join-Path $env:ProgramData 'MicCamWatch'),
        (Join-Path $env:ProgramFiles 'MicCamWatch'),
        (Join-Path ([Environment]::GetFolderPath('Programs')) 'MicCamWatch.lnk'),
        (Join-Path ([Environment]::GetFolderPath('CommonPrograms')) 'MicCamWatch.lnk'),
        (Join-Path $env:USERPROFILE '.cargo/bin/mcw.exe'),
        (Join-Path $env:USERPROFILE '.cargo/bin/mcw-tray.exe')
    )
    if (${env:ProgramFiles(x86)}) { $paths += Join-Path ${env:ProgramFiles(x86)} 'MicCamWatch' }
    if ($env:CARGO_HOME) {
        $paths += Join-Path $env:CARGO_HOME 'bin/mcw.exe'
        $paths += Join-Path $env:CARGO_HOME 'bin/mcw-tray.exe'
    }
    foreach ($path in $paths) {
        if (Test-Path -LiteralPath $path) { throw "Existing MCW product/data/shortcut: $path" }
    }
    if (@(Get-Command mcw, mcw-tray -CommandType Application -ErrorAction SilentlyContinue).Count -ne 0) {
        throw 'An existing MCW executable is on PATH.'
    }
    foreach ($root in @('HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall',
                        'HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall',
                        'HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall')) {
        if (Test-Path -LiteralPath $root) {
            foreach ($key in Get-ChildItem -LiteralPath $root) {
                $values = Get-ItemProperty -LiteralPath $key.PSPath
                if (@($values.PSObject.Properties | Where-Object {
                    $_.Name -match '(?i)miccamwatch' -or [string]$_.Value -match '(?i)miccamwatch|\bmcw(?:-tray)?\.exe'
                }).Count -ne 0) {
                    throw 'Existing MCW uninstall registration: refusing qualification.'
                }
            }
        }
    }
    foreach ($root in @('HKCU:\Software\Microsoft\Windows\CurrentVersion\Run',
                        'HKCU:\Software\Microsoft\Windows\CurrentVersion\RunOnce',
                        'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run',
                        'HKLM:\Software\Microsoft\Windows\CurrentVersion\Run',
                        'HKLM:\Software\Microsoft\Windows\CurrentVersion\RunOnce',
                        'HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Run')) {
        if ((Test-Path -LiteralPath $root) -and
            ((Get-ItemProperty -LiteralPath $root | Out-String) -match '(?i)miccamwatch|\bmcw(?:-tray)?\.exe')) {
            throw 'Existing MCW autostart registration: refusing qualification.'
        }
    }
    if (Test-Path -LiteralPath 'HKCU:\Software\Classes\AppUserModelId\MicCamWatch.MicCamWatch') {
        throw 'Existing MCW application identity: refusing qualification.'
    }
    $installer = New-Object -ComObject WindowsInstaller.Installer
    try {
        $related = $installer.RelatedProducts('{8FC9317B-4517-4E83-9F52-CF3257584BF6}')
        try {
            $count = $related.GetType().InvokeMember('Count', [Reflection.BindingFlags]::GetProperty, $null, $related, $null)
            if ($count -ne 0) { throw 'An existing related MCW MSI product is registered.' }
        } finally { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($related) }
    } finally { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($installer) }
    $tasks = @(Get-ScheduledTask -ErrorAction Stop | Where-Object {
        $_.TaskName -match '(?i)miccamwatch' -or (($_.Actions | Out-String) -match '(?i)miccamwatch|\bmcw(?:-tray)?\.exe')
    })
    if ($tasks.Count -ne 0) { throw 'Existing MCW scheduled autostart task.' }
    foreach ($folder in @([Environment]::GetFolderPath('Startup'), [Environment]::GetFolderPath('CommonStartup'))) {
        if (@(Get-ChildItem -LiteralPath $folder -Force | Where-Object { $_.Name -match '(?i)miccamwatch|\bmcw\b' }).Count -ne 0) {
            throw 'Existing MCW startup-folder entry.'
        }
    }
    # The disposable identity must have no personal documents or synchronized
    # cloud data. Toolchains/runner live outside these ordinary user folders.
    $personalFolders = @([Environment]::GetFolderPath('Desktop'), [Environment]::GetFolderPath('MyDocuments'),
        [Environment]::GetFolderPath('MyPictures'), [Environment]::GetFolderPath('MyMusic'),
        [Environment]::GetFolderPath('MyVideos'), (Join-Path $env:USERPROFILE 'Downloads'))
    if ($env:OneDrive) { $personalFolders += $env:OneDrive }
    foreach ($folder in $personalFolders) {
        if ($folder -and (Test-Path -LiteralPath $folder)) {
            if (@(Get-ChildItem -LiteralPath $folder -Recurse -File -Force | Where-Object { $_.Name -ne 'desktop.ini' }).Count -ne 0) {
                throw "The fresh fixture account has personal-folder contents: $folder"
            }
        }
    }
}

function Invoke-FixtureCommand {
    param([string]$Name, [string]$Executable, [string[]]$Arguments)
    $stdout = Join-Path $proof ("private-$Name-stdout.log")
    $stderr = Join-Path $proof ("private-$Name-stderr.log")
    $record = [ordered]@{ executable = $Executable; arguments = $Arguments; workingDirectory = (Get-Location).Path;
        startedUtc = [DateTime]::UtcNow.ToString('o'); stdout = $stdout; stderr = $stderr; exitCode = $null }
    $commandPath = Join-Path $proof ("private-$Name-command.json")
    Write-FixtureJson $commandPath $record
    $previousPreference = $ErrorActionPreference
    try {
        # Windows PowerShell 5 must not stop at native stderr and lose the
        # actual native exit status or Python traceback. Never eval a command.
        $ErrorActionPreference = 'Continue'
        & $Executable @Arguments 1> $stdout 2> $stderr
        $code = $LASTEXITCODE
    } finally { $ErrorActionPreference = $previousPreference }
    $record['exitCode'] = $code
    $record['finishedUtc'] = [DateTime]::UtcNow.ToString('o')
    Write-FixtureJson $commandPath $record
    if ($code -ne 0) { throw "$Name failed (exit $code); inspect its private command/stdout/stderr artifacts." }
    return [pscustomobject]@{ stdout = $stdout; stderr = $stderr; exitCode = $code }
}

# Do not accept an arbitrary output path, fake profile, or alternate account.
$runnerTemp = Assert-OwnedDirectory $env:RUNNER_TEMP
$proof = Join-Path $runnerTemp 'mcw-standard-user-proof'
$cargoRoot = Join-Path $runnerTemp 'mcw-cargo-uninstall'
if (Test-Path -LiteralPath $proof) { throw 'A prior qualification directory exists; retain it and revert/reprovision the guest.' }
[void](New-Item -ItemType Directory -Path $proof)
$resultPath = Join-Path $proof 'orchestrator-result.json'
$report = [ordered]@{ status = 'running'; platform = 'Windows'; standardUserQualified = $false;
    startedUtc = [DateTime]::UtcNow.ToString('o'); nativeResult = 'result.json'; firstFailure = $null }
Write-FixtureJson $resultPath $report
$exitCode = 1
$oldTemp = $env:TEMP
$oldTmp = $env:TMP
$oldLocation = Get-Location
try {
    Write-FixtureJson (Join-Path $proof 'private-admission-context.json') ([ordered]@{
        githubActions = $env:GITHUB_ACTIONS; runnerEnvironment = $env:RUNNER_ENVIRONMENT; runnerName = $env:RUNNER_NAME;
        disposableVm = $env:MCW_DISPOSABLE_STANDARD_VM; acknowledged = $AcknowledgeDisposableVm;
        userName = $env:USERNAME; sid = [Security.Principal.WindowsIdentity]::GetCurrent().User.Value;
        sessionId = (Get-Process -Id $PID).SessionId; userProfile = $env:USERPROFILE;
        appData = $env:APPDATA; localAppData = $env:LOCALAPPDATA; runnerTemp = $runnerTemp
    })
    if ($AcknowledgeDisposableVm -cne 'true') { throw 'Explicit disposable-VM acknowledgement must be true; default/false is a failure, not a skip.' }
    if ($env:GITHUB_ACTIONS -cne 'true' -or $env:RUNNER_ENVIRONMENT -cne 'self-hosted' -or
        $env:MCW_DISPOSABLE_STANDARD_VM -cne 'true' -or $env:RUNNER_NAME -cnotlike 'mcw-standard-disposable-*') {
        throw 'Only a genuine explicitly provisioned GitHub self-hosted disposable standard-user VM is admitted.'
    }
    . (Join-Path $PSScriptRoot 'windows-fixture-context.ps1')
    $observedContext = Get-McwStandardUserContext
    Write-FixtureJson (Join-Path $proof 'private-standard-user-context.json') $observedContext
    $hostContext = Assert-McwDisposableFixtureHost -RequireStandardUser
    Write-FixtureJson (Join-Path $proof 'private-host-context.json') $hostContext
    Assert-McwStandardUserContext -Context $hostContext.standardUserContext
    if ($hostContext.kind -cne 'dedicated-standard-vm') { throw 'This workflow requires dedicated-standard-vm admission, never the hosted batch path.' }
    if (Test-Path -LiteralPath $cargoRoot) { throw 'Dedicated Cargo uninstall root already exists; refuse reuse.' }
    Assert-NoExistingProductOrData
    foreach ($name in @('AudioEndpointBuilder', 'Audiosrv')) {
        if ((Get-Service -Name $name -ErrorAction Stop).Status -ne 'Running') {
            throw "Provisioning prerequisite $name is not running. Stop: the standard account must not start/change services."
        }
    }
    $repository = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '../..')).Path
    if (-not [string]::Equals($repository, (Resolve-Path -LiteralPath $env:GITHUB_WORKSPACE).Path, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'The actual checked-out GitHub workspace must contain this orchestrator.'
    }
    Set-Location -LiteralPath $repository
    $git = (Get-Command git -CommandType Application -ErrorAction Stop).Source
    $sourceCommand = Invoke-FixtureCommand 'source-revision' $git @('rev-parse', 'HEAD')
    $sourceCommit = (Get-Content -LiteralPath $sourceCommand.stdout -Raw).Trim()
    if ($sourceCommit -cnotmatch '^[0-9a-f]{40}$' -or $sourceCommit -cne $env:GITHUB_SHA) {
        throw 'Checked-out source must exactly match the selected GitHub Actions revision.'
    }
    $report['sourceCommit'] = $sourceCommit
    $cargo = (Get-Command cargo -CommandType Application -ErrorAction Stop).Source
    $dotnet = (Get-Command dotnet -CommandType Application -ErrorAction Stop).Source
    $python = (Get-Command python -CommandType Application -ErrorAction Stop).Source
    $fixtureTemp = Join-Path $proof 'temporary-state'
    [void](New-Item -ItemType Directory -Path $fixtureTemp)
    $env:TEMP = $fixtureTemp
    $env:TMP = $fixtureTemp
    Write-FixtureJson (Join-Path $proof 'private-paths.json') ([ordered]@{
        repository = $repository; runnerTemp = $runnerTemp; fixtureTemp = $fixtureTemp; cargoInstall = $cargoRoot;
        userProfile = $env:USERPROFILE; appData = $env:APPDATA; localAppData = $env:LOCALAPPDATA
    })
    $null = Invoke-FixtureCommand 'cargo-build' $cargo @('build', '--release', '--locked', '--features', 'windows-tray', '--bin', 'mcw', '--bin', 'mcw-tray')
    $binary = (Resolve-Path -LiteralPath 'target/release/mcw.exe').Path
    $null = Resolve-Path -LiteralPath 'target/release/mcw-tray.exe'
    # This is an observation-only production command, before any destructive
    # fixture case. A missing/failed enumeration is not proof of zero devices.
    $doctor = Invoke-FixtureCommand 'doctor' $binary @('doctor', '--lang', 'en', '--json', '--no-color')
    $checks = @(Get-Content -LiteralPath $doctor.stdout -Raw | ConvertFrom-Json)
    foreach ($expected in @(
        @{ name = 'microphone_devices'; detail = '0 capture device(s) detected' },
        @{ name = 'camera_devices'; detail = '0 camera device(s) detected' }
    )) {
        $observed = @($checks | Where-Object { $_.name -ceq $expected.name })
        if ($observed.Count -ne 1 -or $observed[0].status -cne 'warning' -or $observed[0].detail -cne $expected.detail) {
            throw 'Doctor did not positively enumerate zero capture endpoints and zero cameras. Remove guest hardware/redirection during provisioning; do not bypass this gate.'
        }
    }
    $null = Invoke-FixtureCommand 'wix-restore' $dotnet @('tool', 'restore')
    $metadataCommand = Invoke-FixtureCommand 'cargo-metadata' $cargo @('metadata', '--locked', '--no-deps', '--format-version', '1')
    $metadata = Get-Content -LiteralPath $metadataCommand.stdout -Raw | ConvertFrom-Json
    $packages = @($metadata.packages | Where-Object { $_.name -ceq 'miccamwatch' })
    if ($packages.Count -ne 1 -or -not $packages[0].version) { throw 'Cannot resolve actual MicCamWatch package version.' }
    $msi = Join-Path $repository 'target/mcw-standard-user-fixture.msi'
    $null = Invoke-FixtureCommand 'wix-build' $dotnet @('wix', 'build', 'installer/miccamwatch.wxs',
        '-d', ('Version=' + $packages[0].version), '-d', 'Executable=target/release/mcw.exe',
        '-d', 'TrayExecutable=target/release/mcw-tray.exe', '-o', $msi)
    $null = Resolve-Path -LiteralPath $msi
    Copy-Item -LiteralPath $msi -Destination (Join-Path $proof 'mcw-standard-user-fixture.msi')
    $null = Invoke-FixtureCommand 'cargo-install' $cargo @('install', '--locked', '--offline', '--path', $repository,
        '--root', $cargoRoot, '--features', 'windows-tray')
    $null = Resolve-Path -LiteralPath (Join-Path $cargoRoot 'bin/mcw.exe')
    # Re-observe context immediately before the existing native cases. There is
    # no windows_ordinary.py, task, token swap, or environment/profile disguise.
    $currentContext = Get-McwStandardUserContext
    Write-FixtureJson (Join-Path $proof 'private-context-before-native.json') $currentContext
    Assert-McwStandardUserContext -Context $currentContext
    $null = Invoke-FixtureCommand 'native-uninstall' $python @('-u', '-B',
        (Join-Path $repository 'installer/tests/native_uninstall.py'), '--binary', $binary,
        '--repository', $repository, '--cargo-install', $cargoRoot, '--msi', $msi,
        '--proof', (Join-Path $proof 'result.json'))
    $native = Get-Content -LiteralPath (Join-Path $proof 'result.json') -Raw | ConvertFrom-Json
    $application = Get-Content -LiteralPath (Join-Path $proof 'private-msi-application/result.json') -Raw | ConvertFrom-Json
    if ($native.status -cne 'passed' -or $application.status -cne 'passed' -or
        $application.standardUserQualified -ne $true -or $application.requireStandardUser -ne $true -or
        $application.cleanupSafe -ne $true) {
        throw 'Full PASS requires completed native cases and the exact MSI application standardUserQualified/cleanupSafe proof.'
    }
    $report['status'] = 'passed'
    $report['standardUserQualified'] = $true
    $exitCode = 0
} catch {
    $report['status'] = 'failed'
    $report['firstFailure'] = $_.Exception.Message
    [IO.File]::WriteAllText((Join-Path $proof 'private-orchestrator-error.log'), (($_ | Out-String) + "`n" + $_.ScriptStackTrace))
    Write-Error 'Disposable standard-user qualification failed. Retain the guest and inspect the private artifacts; no failure cleanup was attempted.' -ErrorAction Continue
} finally {
    $report['finishedUtc'] = [DateTime]::UtcNow.ToString('o')
    Write-FixtureJson $resultPath $report
    $env:TEMP = $oldTemp
    $env:TMP = $oldTmp
    Set-Location -LiteralPath $oldLocation.Path
}
if ($exitCode -eq 0) { Write-Output 'PASS real interactive standard-user native uninstall and installed MSI application qualification' }
exit $exitCode
