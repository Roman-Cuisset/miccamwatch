param(
    [string]$OutputDirectory = 'artifacts/native-windows-profiles',
    [switch]$IsolatedDesktop
)
$ErrorActionPreference = 'Stop'
if (-not $IsolatedDesktop -or $env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted') {
    throw 'Profile menu proof requires explicit -IsolatedDesktop on a disposable GitHub-hosted Windows runner. Never run against a user tray.'
}
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class McwProfileProof {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
    [StructLayout(LayoutKind.Sequential)] public struct MOUSEINPUT { public int dx, dy; public uint data, flags, time; public UIntPtr extra; }
    [StructLayout(LayoutKind.Sequential)] public struct KEYBDINPUT { public ushort vk, scan; public uint flags, time; public UIntPtr extra; }
    [StructLayout(LayoutKind.Explicit)] public struct UNION { [FieldOffset(0)] public MOUSEINPUT mouse; [FieldOffset(0)] public KEYBDINPUT key; }
    [StructLayout(LayoutKind.Sequential)] public struct INPUT { public uint type; public UNION input; }
    [DllImport("user32.dll", SetLastError=true)] static extern uint SendInput(uint count, INPUT[] input, int size);
    public static void Click() {
        var down = new INPUT { type=0, input=new UNION { mouse=new MOUSEINPUT { flags=2 } } };
        var up = new INPUT { type=0, input=new UNION { mouse=new MOUSEINPUT { flags=4 } } };
        if (SendInput(2, new[] { down, up }, Marshal.SizeOf(typeof(INPUT))) != 2) throw new InvalidOperationException("Mouse input rejected");
    }
    public static void Key(ushort vk) {
        var down = new INPUT { type=1, input=new UNION { key=new KEYBDINPUT { vk=vk } } };
        var up = new INPUT { type=1, input=new UNION { key=new KEYBDINPUT { vk=vk, flags=2 } } };
        if (SendInput(2, new[] { down, up }, Marshal.SizeOf(typeof(INPUT))) != 2) throw new InvalidOperationException("Keyboard input rejected");
    }
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string cls, string title);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindowEx(IntPtr parent, IntPtr after, string cls, string title);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(POINT point);
    [DllImport("user32.dll")] public static extern bool GetMenuItemRect(IntPtr owner, IntPtr menu, uint position, out RECT rect);
    [DllImport("user32.dll")] public static extern uint GetMenuState(IntPtr menu, uint item, uint flags);
    [DllImport("user32.dll")] public static extern int GetMenuItemCount(IntPtr menu);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetMenuString(IntPtr menu, uint item, System.Text.StringBuilder text, int maximum, uint flags);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd, out RECT rect);
    [DllImport("user32.dll", EntryPoint="SendMessageTimeoutW", SetLastError=true)] static extern IntPtr NativeMessage(IntPtr hwnd, uint msg, IntPtr wp, IntPtr lp, uint flags, uint timeout, out IntPtr result);
    public static IntPtr Message(IntPtr hwnd, uint msg, IntPtr wp, IntPtr lp) {
        IntPtr result;
        if (NativeMessage(hwnd, msg, wp, lp, 0x22, 2000, out result) == IntPtr.Zero)
            throw new InvalidOperationException("Owned native message timed out or its target exited");
        return result;
    }
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hwnd, uint msg, IntPtr wp, IntPtr lp);
    [DllImport("user32.dll")] public static extern IntPtr OpenInputDesktop(uint flags, bool inherit, uint access);
    [DllImport("user32.dll")] public static extern bool CloseDesktop(IntPtr desktop);
    [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
    [DllImport("user32.dll")] public static extern IntPtr GetThreadDesktop(uint thread);
    [DllImport("user32.dll", CharSet=CharSet.Unicode, SetLastError=true)] static extern bool GetUserObjectInformation(IntPtr value, int index, System.Text.StringBuilder text, uint bytes, out uint needed);
    public static string DesktopName(IntPtr desktop) {
        var name = new System.Text.StringBuilder(256); uint needed;
        if (!GetUserObjectInformation(desktop, 2, name, 512, out needed)) throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
        return name.ToString();
    }
    [DllImport("user32.dll", SetLastError=true)] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
}
'@
function Assert-Isolation {
    if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or -not [Environment]::UserInteractive) {
        throw 'Hosted interactive isolation was lost; no input sent.'
    }
    if ([McwProfileProof]::FindWindow('MicCamWatchTrayClass', $null) -ne [IntPtr]::Zero -or
        @(Get-Process -Name mcw,mcw-tray -ErrorAction SilentlyContinue).Count -ne 0) {
        throw 'A live product tray/control/protection process exists; no physical native input is permitted.'
    }
    $desktop = [McwProfileProof]::OpenInputDesktop(0, $false, 1)
    if ($desktop -eq [IntPtr]::Zero) { throw 'No accessible input desktop; no unlock or desktop switch attempted.' }
    try { $inputName = [McwProfileProof]::DesktopName($desktop) }
    finally { [void][McwProfileProof]::CloseDesktop($desktop) }
    $threadName = [McwProfileProof]::DesktopName([McwProfileProof]::GetThreadDesktop([McwProfileProof]::GetCurrentThreadId()))
    if ($inputName -ne $threadName -or $inputName -eq 'Winlogon') { throw 'Thread is not on the unlocked input desktop.' }
}
Assert-Isolation
$directory = [IO.Path]::GetFullPath((Join-Path $OutputDirectory ((Get-Date -Format 'yyyyMMdd-HHmmss') + '-' + $PID)))
[void][IO.Directory]::CreateDirectory($directory)
# Build only the test executable. Never launch mcw, mcw-tray, a control or a broker.
function Build-ProfileTest([string]$logDirectory) {
    # Windows PowerShell turns native stderr into errors under ErrorAction=Stop.
    # Capture both streams directly; only the native exit code decides success.
    $start = New-Object System.Diagnostics.ProcessStartInfo
    $start.FileName = (Get-Command cargo -CommandType Application | Select-Object -First 1).Source
    $start.Arguments = 'test --locked --features windows-tray --lib --no-run --message-format=json'
    $start.WorkingDirectory = (Get-Location).Path
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $buildProcess = [System.Diagnostics.Process]::Start($start)
    try {
        $stdout = $buildProcess.StandardOutput.ReadToEndAsync()
        $stderr = $buildProcess.StandardError.ReadToEndAsync()
        if (-not $buildProcess.WaitForExit(180000)) { throw 'Profile test build exceeded its deadline.' }
        if (-not $stdout.Wait(5000) -or -not $stderr.Wait(5000)) { throw 'Profile build streams did not close.' }
        $output = $stdout.GetAwaiter().GetResult()
        [IO.File]::WriteAllText((Join-Path $logDirectory 'build-stdout.jsonl'), $output)
        [IO.File]::WriteAllText((Join-Path $logDirectory 'build-stderr.log'), $stderr.GetAwaiter().GetResult())
        if ($buildProcess.ExitCode -ne 0) { throw 'Profile test executable build failed; see build-stderr.log.' }
        return $output -split '\r?\n'
    } finally {
        if (-not $buildProcess.HasExited) {
            $buildProcess.Kill()
            [void]$buildProcess.WaitForExit(5000)
        }
        $buildProcess.Dispose()
    }
}
$build = @(Build-ProfileTest $directory)
$executables = @($build | ForEach-Object {
    try {
        $item = $_ | ConvertFrom-Json
        if ($item.reason -eq 'compiler-artifact' -and $item.target.name -eq 'miccamwatch' -and $item.profile.test -and $item.executable) { $item.executable }
    } catch { }
} | Select-Object -Unique)
if ($executables.Count -ne 1) { throw 'Cannot uniquely identify the freshly built miccamwatch library-test executable.' }
$executable = [IO.Path]::GetFullPath($executables[0])
$target = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../target')) + [IO.Path]::DirectorySeparatorChar
if (-not $executable.StartsWith($target, [StringComparison]::OrdinalIgnoreCase) -or $executable -notmatch '[\\/]deps[\\/]miccamwatch-[0-9a-f]+\.exe$') {
    throw 'Refusing a non-repository/non-test executable image.'
}
$scope = Join-Path $directory 'scope'
[void][IO.Directory]::CreateDirectory((Join-Path $scope 'roaming/MicCamWatch'))
[void][IO.Directory]::CreateDirectory((Join-Path $scope 'local'))
$settingsPath = Join-Path $scope 'roaming/MicCamWatch/settings.toml'
$configPath = Join-Path $scope 'explicit.toml'
[IO.File]::WriteAllText($configPath, "profile = 'strict'`n")
$script:process = $null
$script:runDirectory = $null
$previousCursor = [System.Windows.Forms.Cursor]::Position
$previousDpi = [McwProfileProof]::SetThreadDpiAwarenessContext([IntPtr](-4))
if ($previousDpi -eq [IntPtr]::Zero) { throw 'Cannot establish native DPI coordinates.' }
function Wait-Record([string]$name) {
    $path = Join-Path $script:runDirectory $name
    $deadline = (Get-Date).AddSeconds(45)
    while ((Get-Date) -lt $deadline) {
        if (Test-Path $path) {
            try {
                $record = Get-Content -Raw -Encoding UTF8 $path | ConvertFrom-Json
                if ($null -ne $record) { return $record }
            } catch { }
        }
        if ($script:process.HasExited) { throw "Fixture exited ($($script:process.ExitCode)) before $name; see $script:runDirectory" }
        Start-Sleep -Milliseconds 25
    }
    throw "Timed out waiting for $name"
}
function Assert-Owner([IntPtr]$window) {
    if ($script:process.HasExited) { throw 'Retained fixture process has exited.' }
    [uint32]$ownerPid = 0
    $thread = [McwProfileProof]::GetWindowThreadProcessId($window, [ref]$ownerPid)
    if ($ownerPid -ne $script:process.Id -or $thread -eq 0 -or
        -not [string]::Equals($script:process.MainModule.FileName, $executable, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Native window/image/PID custody does not match the retained test process handle.'
    }
    return $thread
}
function Wait-Popup([IntPtr]$owner, [IntPtr]$menu) {
    $ownerThread = Assert-Owner $owner
    $deadline = (Get-Date).AddSeconds(15)
    while ((Get-Date) -lt $deadline) {
        $candidate = [IntPtr]::Zero
        while ($true) {
            $candidate = [McwProfileProof]::FindWindowEx([IntPtr]::Zero, $candidate, '#32768', $null)
            if ($candidate -eq [IntPtr]::Zero) { break }
            [uint32]$popupPid = 0
            $thread = [McwProfileProof]::GetWindowThreadProcessId($candidate, [ref]$popupPid)
            if ($popupPid -eq $script:process.Id -and $thread -eq $ownerThread -and
                [McwProfileProof]::Message($candidate, 0x01e1, [IntPtr]::Zero, [IntPtr]::Zero) -eq $menu) {
                return $candidate
            }
        }
        if ($script:process.HasExited) { throw 'Fixture exited while awaiting native menu.' }
        Start-Sleep -Milliseconds 25
    }
    throw 'Owned real Windows popup did not appear.'
}
function Assert-InputTarget([IntPtr]$owner, [IntPtr]$popup) {
    Assert-Isolation
    $null = Assert-Owner $owner
    $null = Assert-Owner $popup
    $foreground = [McwProfileProof]::GetForegroundWindow()
    $null = Assert-Owner $foreground
}
function Focus-RowByKeyboard([IntPtr]$owner, [IntPtr]$popup, [IntPtr]$menu, [uint32]$position, [string]$traceName) {
    Assert-InputTarget $owner $popup
    $count = [McwProfileProof]::GetMenuItemCount($menu)
    if ($count -le 0 -or $position -ge $count) { throw 'Owned keyboard menu target is invalid.' }
    [McwProfileProof]::Key(0x24) # Home; do not assume disabled rows are skipped.
    $trace = @()
    $reached = $false
    for ($move = 0; $move -le $count; $move++) {
        Start-Sleep -Milliseconds 100
        Assert-InputTarget $owner $popup
        $flags = [McwProfileProof]::GetMenuState($menu, $position, 0x400) # MF_BYPOSITION
        if ($flags -eq [uint32]::MaxValue -or ($flags -band 3) -ne 0) { throw 'Owned keyboard target disappeared or became disabled.' }
        $trace += @{ move=$move; position=$position; native_flags=$flags; highlighted=(($flags -band 0x80) -ne 0) }
        if (($flags -band 0x80) -ne 0) { $reached = $true; break } # MF_HILITE
        if ($move -lt $count) { [McwProfileProof]::Key(0x28) } # Down, navigation only.
    }
    ConvertTo-Json -InputObject @($trace) -Depth 5 | Set-Content -Encoding UTF8 (Join-Path $script:runDirectory "$traceName.json")
    if (-not $reached) {
        Capture-Popup $popup "$traceName-failed"
        throw 'Actual keyboard input did not highlight the requested owned menu row.'
    }
}
function Click-Row([IntPtr]$owner, [IntPtr]$popup, [IntPtr]$menu, [uint32]$position) {
    Assert-InputTarget $owner $popup
    $row = New-Object McwProfileProof+RECT
    if (-not [McwProfileProof]::GetMenuItemRect($owner, $menu, $position, [ref]$row) -or $row.Right -le $row.Left -or $row.Bottom -le $row.Top) {
        throw 'Cannot measure a visible owned menu row.'
    }
    $point = New-Object McwProfileProof+POINT
    $point.X = [int](($row.Left + $row.Right) / 2)
    $point.Y = [int](($row.Top + $row.Bottom) / 2)
    [System.Windows.Forms.Cursor]::Position = New-Object System.Drawing.Point -ArgumentList $point.X, $point.Y
    Start-Sleep -Milliseconds 100
    if ([McwProfileProof]::WindowFromPoint($point) -ne $popup) { throw 'Native pointer is not over the exact owned popup; input refused.' }
    [McwProfileProof]::Click()
}
function Capture-Popup([IntPtr]$popup, [string]$name) {
    $null = Assert-Owner $popup
    $rect = New-Object McwProfileProof+RECT
    if (-not [McwProfileProof]::GetWindowRect($popup, [ref]$rect)) { throw 'Cannot capture actual native popup.' }
    $work = [System.Windows.Forms.Screen]::FromHandle($popup).WorkingArea
    if ($rect.Right - $rect.Left -gt 400 -or $rect.Left -lt $work.Left -or $rect.Top -lt $work.Top -or $rect.Right -gt $work.Right -or $rect.Bottom -gt $work.Bottom) {
        throw 'Actual profile popup exceeds its width or monitor work-area bound.'
    }
    $image = Join-Path $script:runDirectory "$name.png"
    $start = New-Object System.Diagnostics.ProcessStartInfo
    $start.FileName = Join-Path $PSHOME 'powershell.exe'
    $renderer = Join-Path $PSScriptRoot 'windows-native-menu-render.ps1'
    $birth = $script:process.StartTime.ToUniversalTime().Ticks
    $start.Arguments = "-NoProfile -File `"$renderer`" -Window $($popup.ToInt64()) -OwnerPid $($script:process.Id) -OwnerBirthTicks $birth -OwnerExecutable `"$executable`" -OutputPath `"$image`" -IsolatedDesktop"
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $renderProcess = [System.Diagnostics.Process]::Start($start)
    try {
        $stdout = $renderProcess.StandardOutput.ReadToEndAsync()
        $stderr = $renderProcess.StandardError.ReadToEndAsync()
        if (-not $renderProcess.WaitForExit(15000)) { throw 'Owned native menu rendering exceeded its deadline.' }
        if (-not $stdout.Wait(2000) -or -not $stderr.Wait(2000)) { throw 'Native render streams did not close.' }
        [IO.File]::WriteAllText((Join-Path $script:runDirectory "$name-render.stdout.log"), $stdout.GetAwaiter().GetResult())
        [IO.File]::WriteAllText((Join-Path $script:runDirectory "$name-render.stderr.log"), $stderr.GetAwaiter().GetResult())
        if ($renderProcess.ExitCode -ne 0 -or -not (Test-Path $image)) { throw 'Owned native menu rendering failed; inspect render logs.' }
    } finally {
        if (-not $renderProcess.HasExited) { $renderProcess.Kill(); [void]$renderProcess.WaitForExit(5000) }
        $renderProcess.Dispose()
    }
}
function Inspect-Choices($record, [IntPtr]$profiles, [int]$checked) {
    if (-not $record.inert -or $record.pid -ne $script:process.Id) { throw 'Fixture is not the private inert profile surface.' }
    $actual = @()
    for ($index = 0; $index -lt 4; $index++) {
        $flags = [McwProfileProof]::GetMenuState($profiles, [uint32](109 + $index), 0)
        if ($flags -eq [uint32]::MaxValue -or (($flags -band 8) -ne 0) -ne ($index -eq $checked) -or
            (($flags -band 3) -ne 0) -ne [bool]$record.explicit_policy) {
            throw 'Actual native menu checkmark/disabled state contradicts the persisted profile/config authority.'
        }
        $label = New-Object System.Text.StringBuilder -ArgumentList 256
        if ([McwProfileProof]::GetMenuString($profiles, [uint32](109 + $index), $label, $label.Capacity, 0) -le 0) {
            throw 'Cannot read the actual localized native profile label.'
        }
        $actual += @{ command=(109 + $index); label=$label.ToString(); checked=(($flags -band 8) -ne 0); disabled=(($flags -band 3) -ne 0) }
    }
    return $actual
}
function Invoke-Phase([string]$language, [string]$mode) {
    $script:runDirectory = Join-Path $scope "$language-$mode"
    [void][IO.Directory]::CreateDirectory($script:runDirectory)
    $start = New-Object System.Diagnostics.ProcessStartInfo
    $start.FileName = $executable
    $start.Arguments = 'frontends::tray::tests::native_windows_tray_profiles --ignored --exact --nocapture --test-threads=1'
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    # Child environment only: the PowerShell driver's and user's appdata are untouched.
    $start.EnvironmentVariables['APPDATA'] = Join-Path $scope 'roaming'
    $start.EnvironmentVariables['LOCALAPPDATA'] = Join-Path $scope 'local'
    $start.EnvironmentVariables['MCW_WINDOWS_PROFILE_ISOLATED'] = 'true'
    $start.EnvironmentVariables['MCW_WINDOWS_PROFILE_SCOPE'] = $scope
    $start.EnvironmentVariables['MCW_WINDOWS_PROFILE_PROOF_DIR'] = $script:runDirectory
    $start.EnvironmentVariables['MCW_WINDOWS_PROFILE_MODE'] = $mode
    $start.EnvironmentVariables['MCW_WINDOWS_PROFILE_LANGUAGE'] = $language
    $start.EnvironmentVariables.Remove('MCW_WINDOWS_PROFILE_CONFIG')
    if ($mode -eq 'explicit') { $start.EnvironmentVariables['MCW_WINDOWS_PROFILE_CONFIG'] = $configPath }
    Assert-Isolation
    $script:process = [System.Diagnostics.Process]::Start($start)
    $null = $script:process.Handle # Retain exact process custody through cleanup, never reacquire by name/PID.
    $stdout = $script:process.StandardOutput.ReadToEndAsync()
    $stderr = $script:process.StandardError.ReadToEndAsync()
    $selections = @(3, 1, 0, 2) # Deliberately not cycle order; mouse and keyboard each select twice.
    $steps = if ($mode -eq 'select') { 5 } else { 1 }
    $checked = if ($mode -eq 'select') { 0 } elseif ($mode -eq 'explicit') { -1 } else { 2 }
    $measurements = @()
    try {
        for ($step = 0; $step -lt $steps; $step++) {
            $record = Wait-Record "menu-$step.json"
            if ($record.language -ne $language) { throw 'Native fixture language does not match the requested locale.' }
            $owner = [IntPtr][long]$record.owner
            $menu = [IntPtr][long]$record.menu
            $profiles = [IntPtr][long]$record.profiles
            $popup = Wait-Popup $owner $menu
            $choices = @(Inspect-Choices $record $profiles $checked)
            Capture-Popup $popup "root-$step"
            # Only the profile submenu is enabled on this inert surface.
            if ($mode -eq 'select' -and $step -lt 4 -and $step % 2 -eq 1) {
                Focus-RowByKeyboard $owner $popup $menu ([uint32]$record.profile_position) "keyboard-root-$step"
                [McwProfileProof]::Key(0x27) # Right: real native submenu.
            } else {
                Click-Row $owner $popup $menu ([uint32]$record.profile_position)
            }
            $submenu = Wait-Popup $owner $profiles
            Capture-Popup $submenu "choices-$step"
            if ($mode -eq 'select' -and $step -lt 4) {
                $selected = $selections[$step]
                if ($step % 2 -eq 0) {
                    Click-Row $owner $submenu $profiles ([uint32]$selected)
                } else {
                    Focus-RowByKeyboard $owner $submenu $profiles ([uint32]$selected) "keyboard-choice-$step"
                    [McwProfileProof]::Key(0x0d)
                }
                $checked = $selected
            } else {
                if ($mode -eq 'explicit') {
                    $before = [IO.File]::ReadAllBytes($settingsPath)
                    foreach ($command in @(109, 110, 111, 112)) {
                        $null = Assert-Owner $owner
                        [void][McwProfileProof]::Message($owner, 0x0111, [IntPtr]$command, [IntPtr]::Zero)
                    }
                    $result = Wait-Record 'command.json'
                    if ($result.accepted -ne 0 -or $result.rejected -ne 4) { throw 'Forged WM_COMMAND bypassed explicit-config refusal.' }
                    $after = [IO.File]::ReadAllBytes($settingsPath)
                    if ([Convert]::ToBase64String($before) -ne [Convert]::ToBase64String($after)) { throw 'Explicit-config commands changed persisted settings.' }
                }
                [void][McwProfileProof]::PostMessage($owner, 0x001f, [IntPtr]::Zero, [IntPtr]::Zero) # WM_CANCELMODE, no command.
            }
            $inputKind = if ($mode -eq 'select' -and $step -lt 4) { if ($step % 2 -eq 0) { 'mouse' } else { 'keyboard' } } else { 'inspect/cancel' }
            $measurements += @{ language=$language; mode=$mode; step=$step; choices=$choices; selected_index=$checked; input=$inputKind }
        }
        if (-not $script:process.WaitForExit(15000)) { throw 'Profile proof did not finish within the finite cleanup deadline.' }
        if ($script:process.ExitCode -ne 0 -or -not (Test-Path (Join-Path $script:runDirectory 'complete'))) { throw "Profile fixture failed: $script:runDirectory" }
        ConvertTo-Json -InputObject @($measurements) -Depth 8 | Set-Content -Encoding UTF8 (Join-Path $script:runDirectory 'measurements.json')
    } finally {
        if (-not $script:process.HasExited) { $script:process.Kill(); [void]$script:process.WaitForExit(10000) }
        if ($stdout.Wait(2000)) { [IO.File]::WriteAllText((Join-Path $script:runDirectory 'stdout.log'), $stdout.GetAwaiter().GetResult()) }
        if ($stderr.Wait(2000)) { [IO.File]::WriteAllText((Join-Path $script:runDirectory 'stderr.log'), $stderr.GetAwaiter().GetResult()) }
        $script:process.Dispose()
        $script:process = $null
    }
}
try {
    $languages = @('en', 'fr', 'de', 'es', 'ja', 'zh', 'ru')
    foreach ($language in $languages) {
        [IO.File]::WriteAllText($settingsPath, "profile = 'balanced'`nsound_enabled = true`n")
        Invoke-Phase $language 'select'
        Invoke-Phase $language 'reload'
        Invoke-Phase $language 'explicit'
    }
    @{ passed=$true; executable=$executable; languages=$languages; selection_order=@('development', 'private', 'balanced', 'meeting'); mouse_and_keyboard=$true; process_restart_reload=$true; explicit_disabled_and_forged_commands_rejected=$true; protection_or_hardware_qualified=$false; collector_policy_refresh_qualified=$false } |
        ConvertTo-Json -Depth 5 | Set-Content -Encoding UTF8 (Join-Path $directory 'qualification.json')
    Write-Output "Qualified actual native profile menu selection, persistence/restart and explicit-config refusal (no protection/hardware/collector claim): $directory"
} finally {
    if ($script:process) {
        if (-not $script:process.HasExited) { $script:process.Kill(); [void]$script:process.WaitForExit(10000) }
        $script:process.Dispose()
    }
    [System.Windows.Forms.Cursor]::Position = $previousCursor
    [void][McwProfileProof]::SetThreadDpiAwarenessContext($previousDpi)
}
