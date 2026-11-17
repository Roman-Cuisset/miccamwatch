[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Candidate,
    [Parameter(Mandatory)][string]$Proof,
    [string]$StandardAccountSid,
    [string]$SourceRevision = $env:GITHUB_SHA
)
$ErrorActionPreference = 'Stop'
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
$isAdmin = $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
$standard = [bool]$StandardAccountSid
if ($standard) {
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
    $local = [Environment]::GetFolderPath('LocalApplicationData', 'Create')
    $roaming = [Environment]::GetFolderPath('ApplicationData', 'Create')
    if (!$profile -or $profile -ine [Environment]::GetFolderPath('UserProfile') -or
        !$local.StartsWith($profile.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase) -or
        !$roaming.StartsWith($profile.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase)) {
        throw 'The loaded standard account does not own this fixture profile.'
    }
    $env:LOCALAPPDATA = $local
    $env:APPDATA = $roaming
} elseif ($env:RUNNER_ENVIRONMENT -cne 'github-hosted' -or $env:GITHUB_ACTIONS -cne 'true') {
    throw 'This scenario requires a disposable GitHub-hosted Windows runner.'
}
if ($SourceRevision -notmatch '^[0-9a-f]{40}$') { throw 'Exact source revision required' }
$base = Join-Path $env:LOCALAPPDATA 'Programs\MicCamWatch'
$configuration = Join-Path $env:APPDATA 'MicCamWatch'
$data = Join-Path $env:LOCALAPPDATA 'MicCamWatch'
$runKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$shortcut = Join-Path ([Environment]::GetFolderPath('Programs')) 'MicCamWatch.lnk'
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
    administratorToken=$isAdmin; standardTokenVerified=$standard; interactiveFlutterRequested=!$standard; sessionId=[Diagnostics.Process]::GetCurrentProcess().SessionId}

function Invoke-Msi([string]$Operation, [string]$Payload, [string]$Label, [int]$ExpectedCode = 0) {
    $log = Join-Path $Proof "$Label.log"
    $arguments = '{0} "{1}" /qn /norestart /l*v "{2}"' -f $Operation,$Payload,$log
    $process = Start-Process -FilePath "$env:WINDIR\System32\msiexec.exe" -WindowStyle Hidden -PassThru -ArgumentList $arguments
    if (!$process.WaitForExit(180000)) { throw "MSI timed out. Retained live fixture; inspect $log." }
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
    if ($installer.ProductState($Code) -ne 5 -or
        ([string]$installer.ProductInfo($Code, 'InstallLocation')).TrimEnd('\') -ine $base -or
        ([string]$installer.ProductInfo($Code, 'AssignmentType')) -ne '0') { throw "Invalid per-user MSI status for $Version" }
    if ((& "$base\mcw.exe" --version) -cne "mcw $Version" -or
        !(Test-Path -LiteralPath "$base\mcw-tray.exe" -PathType Leaf) -or
        !(Test-Path -LiteralPath $shortcut)) { throw "Installed executables or shortcut version mismatch for $Version" }
    if ($Version -ne '0.21.0') {
        # PowerShell does not wait for a GUI executable invoked with &.
        $output = Join-Path $Proof "installed-tray-$Version.txt"
        $process = Start-Process -FilePath "$base\mcw-tray.exe" -ArgumentList '--version' -WindowStyle Hidden `
            -RedirectStandardOutput $output -PassThru
        if (!$process.WaitForExit(5000) -or $process.ExitCode -ne 0 -or
            (Get-Content -LiteralPath $output -Raw).Trim() -cne "mcw-tray $Version") {
            throw 'Hybrid tray version mismatch'
        }
    }
}

function Copy-FailingFixture([string]$Source, [string]$Destination) {
    Copy-Item -LiteralPath $Source -Destination $Destination
    $db = $installer.OpenDatabase($Destination, 1)
    try {
        $seq = [int](Read-MsiValue $db "SELECT ``Sequence`` FROM ``InstallExecuteSequence`` WHERE ``Action``='RemoveExistingProducts'")
        if ($seq -le 1500 -or $seq -ge 4000) { throw 'Upgrade removal is outside the expected rollback window' }
        $p = $seq + 1
        $statements = @(
            "INSERT INTO ``CustomAction`` (``Action``, ``Type``, ``Target``) VALUES ('McwFixtureFailure', 19, 'Expected MicCamWatch rollback fixture failure')",
            "INSERT INTO ``InstallExecuteSequence`` (``Action``, ``Condition``, ``Sequence``) VALUES ('McwFixtureFailure', '1', $p)"
        )
        foreach ($statement in $statements) {
            $view = $db.OpenView($statement)
            try { [void]$view.Execute() }
            finally { [void]$view.Close(); [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($view) }
        }
        [void]$db.Commit()
    } finally { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($db) }
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

try {
    $stable = Join-Path $Proof 'baseline-v0.21.0.msi'
    Invoke-WebRequest -UseBasicParsing -Uri 'https://github.com/Roman-Cuisset/miccamwatch/releases/download/v0.21.0/miccamwatch-windows-x86_64.msi' -OutFile $stable
    $stableSha = (Get-FileHash -LiteralPath $stable -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($stableSha -cne '4e6145a4e48bff47f08c8341c24c14e7fab563146e36b8338f46441f7f4da3df') { throw 'Public v0.21.0 digest changed' }
    $oldCode = Identity $stable '0.21.0'
    $bundle = Get-Content -LiteralPath (Join-Path (Split-Path $Candidate) 'bundle\miccamwatch-bundle.json') -Raw | ConvertFrom-Json
    if ($bundle.revision -cne $SourceRevision -or $bundle.ipc -ne 10) { throw 'The candidate is not this run source' }
    $newCode = Identity $Candidate $bundle.version
    $report.baselineSha256 = $stableSha
    $report.candidateSha256 = (Get-FileHash -LiteralPath $Candidate -Algorithm SHA256).Hash.ToLowerInvariant()
    Invoke-Msi '/i' $stable 'install-baseline'
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
    $runEntry = @(Read-Autostart)
    if ($runEntry.Count -ne 1 -or !$runEntry[0].Contains($base)) { throw 'Baseline autostart registration missing' }

    $faulty = Join-Path $Proof 'rollback-fixture.msi'
    Copy-FailingFixture $Candidate $faulty
    Invoke-Msi '/i' $faulty 'upgrade-with-rollback' 1603
    Check-Installed $oldCode '0.21.0'
    if (Compare-Object $baselineHashes @(Get-FileHash -LiteralPath "$base\mcw.exe","$base\mcw-tray.exe" -Algorithm SHA256 | ForEach-Object Hash)) {
        throw 'Rollback did not restore both original Rust executables'
    }
    if ($installer.ProductState($newCode) -eq 5 -or (Test-Path -LiteralPath "$base\desktop")) {
        throw 'Failed upgrade left the candidate installed'
    }
    $rollback = Get-Content -LiteralPath (Join-Path $Proof 'upgrade-with-rollback.log') -Raw
    $rollbackScript = [regex]::Match($rollback,
        '(?s)Action ended [^\r\n]+RemoveExistingProducts\. Return value 1\..*?Action ended [^\r\n]+McwFixtureFailure\. Return value 3\..*?Executing op: Header\([^\r\n]*ScriptType=2,[^\r\n]*\)(?<restore>.*)')
    if (!$rollbackScript.Success -or $rollback -notmatch 'Expected MicCamWatch rollback fixture failure') {
        throw 'Expected MSI removal, injected failure and rollback script were not observed in order'
    }
    foreach ($name in @('mcw.exe','mcw-tray.exe')) {
        $destination = [regex]::Escape((Join-Path $base $name))
        if ($rollbackScript.Groups['restore'].Value -notmatch
            ('(?im)^.*Executing op: FileCopy\(SourceName=[^,\r\n]+\.rbf,[^\r\n]*DestName=' + $destination + ',')) {
            throw "Rollback log has no backup restore for $name"
        }
    }
    $report.wholeMsiRollback = $true

    Invoke-Msi '/i' $Candidate 'upgrade-hybrid'
    Check-Installed $newCode $bundle.version
    if ($installer.ProductState($oldCode) -eq 5 -or (Compare-Object $runEntry @(Read-Autostart))) {
        throw 'Upgrade left the baseline registered or changed autostart'
    }
    $report.msiUpgrade = $true
    foreach ($file in $bundle.files.PSObject.Properties) {
        if ((Get-FileHash -LiteralPath (Join-Path $base $file.Name) -Algorithm SHA256).Hash -ine $file.Value) {
            throw "Installed payload digest mismatch: $($file.Name)"
        }
    }
    if ((Get-Item -LiteralPath "$base\desktop\miccamwatch_desktop.exe").VersionInfo.ProductVersion -notmatch '^0\.22\.0(?:\D|$)') {
        throw 'Flutter Windows version resource mismatch'
    }
    $report.installedPayloadVerified = $true
    if (!$standard) {
        $trayImage = "$base\mcw-tray.exe"
        $flutterImage = "$base\desktop\miccamwatch_desktop.exe"
        Start-Process -FilePath $trayImage -ArgumentList '--open'
        $limit = [DateTime]::UtcNow.AddSeconds(45)
        do {
            Start-Sleep -Milliseconds 200
            $tray = @(Owned 'mcw-tray' $trayImage)
            $ui = @(Owned 'miccamwatch_desktop' $flutterImage | Where-Object { $_.MainWindowHandle -ne [IntPtr]::Zero })
        } until (($tray.Count -eq 1 -and $ui.Count -eq 1) -or [DateTime]::UtcNow -gt $limit)
        if ($tray.Count -ne 1 -or $ui.Count -ne 1) { throw 'Installed tray/Flutter did not open exactly once' }
        $trayId = $tray[0].Id
        if (!$ui[0].CloseMainWindow() -or !$ui[0].WaitForExit(15000)) { throw 'Installed Flutter window did not close' }
        $owner = @(Owned 'mcw-tray' $trayImage)
        if ($owner.Count -ne 1 -or $owner[0].Id -ne $trayId -or @(Owned 'miccamwatch_desktop' $flutterImage).Count) {
            throw 'Close changed the native owner or retained a Flutter process'
        }
        & "$base\mcw.exe" tray status
        if ($LASTEXITCODE -ne 0) { throw 'Native status after window close failed' }
        $report.installedFlutterClosedTrayContinued = $true
        & "$base\mcw.exe" tray stop
        if ($LASTEXITCODE -ne 0 -or !$owner[0].WaitForExit(15000)) { throw 'Installed owner did not stop cooperatively' }
    }
    & "$base\mcw.exe" autostart disable
    if ($LASTEXITCODE -ne 0 -or @(Read-Autostart).Count -ne 0) { throw 'Fixture autostart removal failed' }
    Invoke-Msi '/x' $newCode 'uninstall-hybrid'
    if ($installer.ProductState($newCode) -eq 5 -or (Test-Path -LiteralPath "$base\desktop") -or
        (Test-Path -LiteralPath "$base\mcw.exe") -or (Test-Path -LiteralPath $shortcut)) {
        throw 'Uninstall left hybrid payload or product registrations'
    }
    if (Compare-Object $before @(Get-FileHash -LiteralPath $settings,$history -Algorithm SHA256 | ForEach-Object Hash)) {
        throw 'Upgrade/rollback/uninstall did not preserve fixture user data'
    }
    $report.userDataPreserved = $true
    $report.msiUninstall = $true
    $report.standardUserQualified = $standard
    $report.status = 'passed'
} catch {
    $report.status = 'failed'
    $report.error = $_.ToString()
    throw
} finally {
    $report | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $Proof 'result.json') -Encoding utf8
}
