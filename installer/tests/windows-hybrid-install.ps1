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
    $installationLock = Join-Path $base '.mcw-install.lock'
    if (!(Test-Path -LiteralPath $installationLock -PathType Leaf)) { throw 'Baseline autostart did not create its native lock' }
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
    Check-Rollback 'upgrade-with-rollback' @('mcw.exe','mcw-tray.exe')
    $report.wholeMsiRollback = $true

    Invoke-Msi '/i' $Candidate 'upgrade-hybrid'
    Check-Installed $newCode $bundle.version
    if ($installer.ProductState($oldCode) -eq 5 -or (Compare-Object $runEntry @(Read-Autostart))) {
        throw 'Upgrade left the baseline registered or changed autostart'
    }
    $report.msiUpgrade = $true
    Check-Payload $bundle
    $report.installedPayloadVerified = $true
    if (!$standard) {
        $trayImage = "$base\mcw-tray.exe"
        $flutterImage = "$base\desktop\miccamwatch_desktop.exe"
        $info = [Diagnostics.ProcessStartInfo]::new($trayImage, '--open')
        $info.UseShellExecute = $false
        $info.WorkingDirectory = $base
        $info.RedirectStandardError = $true
        $launch = [Diagnostics.Process]::Start($info)
        $diagnostics = $launch.StandardError.ReadToEndAsync()
        try {
            $limit = [DateTime]::UtcNow.AddSeconds(45)
            do {
                Start-Sleep -Milliseconds 200
                $tray = @(Owned 'mcw-tray' $trayImage)
                $flutter = @(Owned 'miccamwatch_desktop' $flutterImage)
                $ui = @($flutter | Where-Object { $_.MainWindowHandle -ne [IntPtr]::Zero })
            } until (($tray.Count -eq 1 -and $ui.Count -eq 1) -or $launch.HasExited -or
                [DateTime]::UtcNow -gt $limit)
            $report.installedProcessObservation = [ordered]@{trayCount=$tray.Count; flutterCount=$flutter.Count;
                flutterWindowCount=$ui.Count; startedOwnerExited=$launch.HasExited; trayWindowTitles=@($tray.MainWindowTitle)}
            if ($launch.HasExited) { $report.installedProcessObservation.ownerExitCode = $launch.ExitCode }
            if ($tray.Count -ne 1 -or $ui.Count -ne 1 -or $tray[0].Id -ne $launch.Id) {
                throw 'Installed tray/Flutter did not open exactly once'
            }
            if (!$ui[0].CloseMainWindow() -or !$ui[0].WaitForExit(15000)) { throw 'Installed Flutter window did not close' }
            $owner = @(Owned 'mcw-tray' $trayImage)
            if ($owner.Count -ne 1 -or $owner[0].Id -ne $launch.Id -or @(Owned 'miccamwatch_desktop' $flutterImage).Count) {
                throw 'Close changed the native owner or retained a Flutter process'
            }
            & "$base\mcw.exe" tray status
            if ($LASTEXITCODE -ne 0) { throw 'Native status after window close failed' }
            $report.installedFlutterClosedTrayContinued = $true
        } catch {
            $report.guiError = $_.ToString()
        } finally {
            if (!$launch.HasExited) {
                $owner = @(Owned 'mcw-tray' $trayImage)
                if ($owner.Count -ne 1 -or $owner[0].Id -ne $launch.Id) { throw 'Cannot prove exact tray identity for shutdown' }
                & "$base\mcw.exe" tray stop
                if ($LASTEXITCODE -ne 0 -or !$launch.WaitForExit(15000)) { throw 'Installed owner did not stop cooperatively' }
            }
            if ($diagnostics.Wait(1000)) {
                [IO.File]::WriteAllText((Join-Path $Proof 'installed-tray-stderr.log'), $diagnostics.Result)
                $report.installedIpcReady = $diagnostics.Result -match '(?m)^desktop event=ready endpoint=parent_received pid=[1-9][0-9]* '
            }
            $launch.Dispose()
        }
        if (!$report.installedIpcReady -and !$report.guiError) { $report.guiError = 'Missing installed IPC ready receipt' }
        if (@(Owned 'miccamwatch_desktop' $flutterImage).Count) { throw 'Flutter remains after its owner stopped' }
    }
    # Simulate a future MSI failing after removing this hybrid package. Verify
    # rollback against every installed file, including Flutter and the manifest.
    $future = Join-Path $Proof 'rollback-hybrid-fixture.msi'
    $futureCode = [Guid]::NewGuid().ToString('B').ToUpperInvariant()
    $currentVersion = [Version]::Parse($bundle.version)
    $futureVersion = '{0}.{1}.{2}' -f $currentVersion.Major,$currentVersion.Minor,($currentVersion.Build + 1)
    Copy-FailingFixture $Candidate $future $futureCode $futureVersion
    if ((Identity $future $futureVersion) -cne $futureCode) { throw 'Invalid future fixture identity' }
    Invoke-Msi '/i' $future 'hybrid-upgrade-with-rollback' 1603
    Check-Rollback 'hybrid-upgrade-with-rollback' @('mcw.exe','mcw-tray.exe','miccamwatch-bundle.json',
        'desktop\miccamwatch_desktop.exe','desktop\flutter_windows.dll','desktop\data\app.so')
    Check-Installed $newCode $bundle.version
    Check-Payload $bundle
    if ($installer.ProductState($futureCode) -eq 5 -or (Compare-Object $runEntry @(Read-Autostart))) {
        throw 'Failed hybrid upgrade left the newer fixture installed or changed autostart'
    }
    $report.wholeHybridMsiRollback = $true

    & "$base\mcw.exe" autostart disable
    if ($LASTEXITCODE -ne 0 -or @(Read-Autostart).Count -ne 0) { throw 'Fixture autostart removal failed' }
    Invoke-Msi '/x' $newCode 'uninstall-hybrid'
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
