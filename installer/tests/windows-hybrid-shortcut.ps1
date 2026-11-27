param(
    [Parameter(Mandatory)][string]$PackageDirectory,
    [Parameter(Mandatory)][string]$Proof
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
. (Join-Path $PSScriptRoot 'windows-fixture-context.ps1')
$null = Assert-McwDisposableFixtureHost
if ($env:RUNNER_ENVIRONMENT -cne 'github-hosted' -or -not [Environment]::UserInteractive) {
    throw 'An interactive GitHub-hosted Windows desktop is required.'
}

$package = (Resolve-Path -LiteralPath $PackageDirectory).Path
$msi = Join-Path $package 'miccamwatch-hybrid-windows-x86_64.msi'
$install = Join-Path $env:LOCALAPPDATA 'Programs/MicCamWatch'
$shortcut = Join-Path ([Environment]::GetFolderPath('Programs')) 'MicCamWatch.lnk'
$settingsDir = Join-Path $env:APPDATA 'MicCamWatch'
$dataDir = Join-Path $env:LOCALAPPDATA 'MicCamWatch'
$tray = Join-Path $install 'mcw-tray.exe'
$cli = Join-Path $install 'mcw.exe'
$frontend = Join-Path $install 'desktop/miccamwatch_desktop.exe'
foreach ($path in @($install, $shortcut, $Proof, $settingsDir, $dataDir)) {
    if (Test-Path -LiteralPath $path) { throw "Existing fixture path: $path" }
}
if (Get-Process 'mcw-tray','miccamwatch_desktop' -ErrorAction SilentlyContinue) {
    throw 'Another MicCamWatch process is already running.'
}
$null = New-Item -ItemType Directory -Path $Proof,$settingsDir,$dataDir
$proofRoot = (Resolve-Path -LiteralPath $Proof).Path
$settings = Join-Path $settingsDir 'settings.toml'
$history = Join-Path $dataDir 'history.jsonl'
@'
notifications_enabled = false
sound_enabled = false
history_enabled = false
mute_on_lock = false
block_camera_on_lock = false
'@ | Set-Content -LiteralPath $settings
'{"fixture":"persisted across upgrade"}' | Set-Content -LiteralPath $history
$state = @(Get-FileHash -LiteralPath $settings,$history | ForEach-Object Hash)

function Install-McwFixture([string]$Path, [string]$Name) {
    $log = Join-Path $proofRoot "$Name.log"
    $proc = Start-Process -FilePath "$env:WINDIR/System32/msiexec.exe" -ArgumentList @(
        '/i', ('"{0}"' -f $Path), '/qn', '/norestart', '/l*v', ('"{0}"' -f $log)
    ) -WindowStyle Hidden -PassThru
    if (!$proc.WaitForExit(120000) -or $proc.ExitCode -ne 0) { throw "MSI transaction failed: $Name" }
}

function Find-OwnedProcess([string]$Name, [string]$Path) {
    @(Get-Process -Name $Name -ErrorAction SilentlyContinue | Where-Object Path -EQ $Path)
}

$receipt = [ordered]@{status='failed'; msi_sha256=(Get-FileHash -LiteralPath $msi).Hash.ToLowerInvariant(); windows=@()}
try {
    $baseline = Join-Path $proofRoot 'baseline-0.22.1.msi'
    Invoke-WebRequest 'https://github.com/Roman-Cuisset/miccamwatch/releases/download/v0.22.1/miccamwatch-hybrid-windows-x86_64.msi' -OutFile $baseline
    if ((Get-FileHash -LiteralPath $baseline).Hash -ine 'ef9ca1371ef803e36ab08acde7659d811d7cb531c5e0c025db783ea4b1e2f2e0') {
        throw 'Wrong public v0.22.1 baseline digest.'
    }
    Install-McwFixture $baseline 'install-0.22.1'
    Install-McwFixture $msi 'upgrade-candidate'
    $installed = Get-Content -LiteralPath (Join-Path $install 'miccamwatch-bundle.json') -Raw | ConvertFrom-Json -AsHashtable
    $expected = Get-Content -LiteralPath (Join-Path $package 'bundle/miccamwatch-bundle.json') -Raw | ConvertFrom-Json -AsHashtable
    if ($installed.version -ne '0.22.2' -or $installed.revision -cne $env:GITHUB_SHA -or $installed.ipc -ne 10 -or
        (Get-FileHash -LiteralPath (Join-Path $install 'miccamwatch-bundle.json')).Hash -ne
        (Get-FileHash -LiteralPath (Join-Path $package 'bundle/miccamwatch-bundle.json')).Hash) {
        throw 'Wrong installed manifest identity.'
    }
    foreach ($entry in $expected.files.GetEnumerator()) {
        if ((Get-FileHash -LiteralPath (Join-Path $install $entry.Key)).Hash -ine $entry.Value) {
            throw "Installed bytes changed: $($entry.Key)"
        }
    }
    $shell = New-Object -ComObject WScript.Shell
    if (!(Test-Path -LiteralPath $shortcut -PathType Leaf)) { throw 'Installed shortcut is missing.' }
    $link = $shell.CreateShortcut($shortcut)
    if ($link.TargetPath -ine $tray -or $link.Arguments -cne '--open') { throw 'Wrong installed shortcut.' }
    for ($cycle = 1; $cycle -le 2; $cycle++) {
        # ShellExecute uses the original per-user .lnk without stdio redirection.
        $info = [Diagnostics.ProcessStartInfo]::new($shortcut)
        $info.UseShellExecute = $true
        $null = [Diagnostics.Process]::Start($info)
        $end = [DateTime]::UtcNow.AddSeconds(40)
        do {
            $owners = @(Find-OwnedProcess 'mcw-tray' $tray)
            $windows = @(Find-OwnedProcess 'miccamwatch_desktop' $frontend | Where-Object MainWindowHandle -NE ([IntPtr]::Zero))
            if ($owners.Count -eq 1 -and $windows.Count -eq 1) { break }
            Start-Sleep -Milliseconds 250
        } while ([DateTime]::UtcNow -lt $end)
        if ($owners.Count -ne 1 -or $windows.Count -ne 1 -or !$windows[0].Responding) {
            throw "Installed shortcut did not display one responding Flutter window in cycle $cycle."
        }
        $parent = Get-CimInstance Win32_Process -Filter "ProcessId=$($windows[0].Id)"
        if ($parent.ParentProcessId -ne $owners[0].Id) { throw 'Unexpected owner of the Flutter window.' }
        if ($cycle -eq 2 -and $owners[0].Id -ne $receipt.windows[0].tray_pid) { throw 'Resident tray was replaced.' }
        $receipt.windows += @{cycle=$cycle; tray_pid=$owners[0].Id; flutter_pid=$windows[0].Id; visible=$true}
        if (!$windows[0].CloseMainWindow() -or !$windows[0].WaitForExit(10000)) { throw 'Flutter did not close normally.' }
    }
    if (Compare-Object $state @(Get-FileHash -LiteralPath $settings,$history | ForEach-Object Hash)) {
        throw 'Upgrade or opening changed fixture user data.'
    }
    $receipt.status = 'passed'
} catch {
    $receipt.error = $_.Exception.Message
    throw
} finally {
    try {
        if ((Test-Path -LiteralPath $cli) -and @(Find-OwnedProcess 'mcw-tray' $tray).Count -eq 1) {
            & $cli tray stop
            if ($LASTEXITCODE -ne 0) { throw 'Owned tray did not stop.' }
        }
    } finally {
        $receipt | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $proofRoot 'result.json')
    }
}
