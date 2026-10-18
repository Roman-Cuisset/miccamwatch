param(
    [string]$OutputDirectory = 'artifacts/native-windows-tray',
    [switch]$IsolatedClipboard,
    [switch]$IsolatedDesktop
)
$ErrorActionPreference = 'Stop'
if (-not $IsolatedDesktop -or $env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted') {
    throw 'Shell tooltip proof is restricted to an explicitly isolated GitHub-hosted Windows runner (-IsolatedDesktop). Never run against an installed user tray.'
}
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class McwNativeProof {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct NOTIFYICONIDENTIFIER { public uint cbSize; public IntPtr hWnd; public uint uID; public Guid guidItem; }
    [DllImport("shell32.dll")] public static extern int Shell_NotifyIconGetRect(ref NOTIFYICONIDENTIFIER icon, out RECT rect);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string cls, string title);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd, out RECT rect);
    [DllImport("user32.dll")] public static extern bool GetMenuItemRect(IntPtr hwnd, IntPtr menu, uint item, out RECT rect);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr hwnd, IntPtr hdc, uint flags);
    [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr hwnd, uint msg, IntPtr wparam, IntPtr lparam);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hwnd, uint msg, IntPtr wparam, IntPtr lparam);
    [DllImport("user32.dll")] public static extern IntPtr OpenInputDesktop(uint flags, bool inherit, uint access);
    [DllImport("user32.dll")] public static extern bool CloseDesktop(IntPtr desktop);
    [DllImport("user32.dll", SetLastError=true)] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
}
'@
if (-not [Environment]::UserInteractive) { throw 'Native proof requires an interactive, unlocked Windows desktop.' }
$desktop = [McwNativeProof]::OpenInputDesktop(0, $false, 1)
if ($desktop -eq [IntPtr]::Zero) { throw 'No accessible input desktop: native geometry/screenshots cannot be verified.' }
[void][McwNativeProof]::CloseDesktop($desktop)
$directory = [IO.Path]::GetFullPath((Join-Path $OutputDirectory ((Get-Date -Format 'yyyyMMdd-HHmmss') + '-' + $PID)))
[void][IO.Directory]::CreateDirectory($directory)
$previous = $env:MCW_WINDOWS_NATIVE_PROOF_DIR
$env:MCW_WINDOWS_NATIVE_PROOF_DIR = $directory
$process = $null
$fixtureProcess = $null
$previousCursor = [System.Windows.Forms.Cursor]::Position
$shellMeasurements = @()
function Wait-ProofFile([string]$name) {
    $deadline = (Get-Date).AddMinutes(10)
    $path = Join-Path $directory $name
    while (-not (Test-Path $path)) {
        if ($process.HasExited) { throw "Native fixture exited ($($process.ExitCode)); see $directory/cargo-stderr.log and cargo-stdout.log" }
        if ((Get-Date) -gt $deadline) { throw "Native fixture timed out waiting for $name" }
        Start-Sleep -Milliseconds 50
    }
    # The producer writes small JSON atomically from its single UI thread, but
    # the file can be visible before its write completes.
    $deadline = (Get-Date).AddSeconds(10)
    while ($true) {
        try { return (Get-Content -Raw -Encoding UTF8 $path | ConvertFrom-Json) }
        catch { if ((Get-Date) -gt $deadline) { throw }; Start-Sleep -Milliseconds 20 }
    }
}
function Capture-NativeWindow([IntPtr]$window, [string]$name, [switch]$RenderWindow) {
    $rect = New-Object McwNativeProof+RECT
    if (-not [McwNativeProof]::GetWindowRect($window, [ref]$rect)) { throw "Cannot measure $name" }
    $width = $rect.Right - $rect.Left
    $height = $rect.Bottom - $rect.Top
    if ($width -le 0 -or $height -le 0) { throw "Empty native $name rectangle" }
    $bitmap = New-Object System.Drawing.Bitmap -ArgumentList $width, $height
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    try {
        if ($RenderWindow) {
            $dc = $graphics.GetHdc()
            try {
                if (-not [McwNativeProof]::PrintWindow($window, $dc, 2)) { throw "Native PrintWindow failed for $name" }
            } finally { $graphics.ReleaseHdc($dc) }
        } else {
            $graphics.CopyFromScreen($rect.Left, $rect.Top, 0, 0, $bitmap.Size)
        }
        $bitmap.Save((Join-Path $directory "$name.png"), [System.Drawing.Imaging.ImageFormat]::Png)
        $first = $bitmap.GetPixel(0, 0).ToArgb()
        $different = $false
        for ($x = 0; $x -lt $width; $x += 7) {
            for ($y = 0; $y -lt $height; $y += 7) {
                if ($bitmap.GetPixel($x, $y).ToArgb() -ne $first) { $different = $true; break }
            }
            if ($different) { break }
        }
        if (-not $different) { throw "Blank native screenshot for $name; desktop is unavailable/locked." }
    } finally { $graphics.Dispose(); $bitmap.Dispose() }
    return @{ left=$rect.Left; top=$rect.Top; right=$rect.Right; bottom=$rect.Bottom; width=$width; height=$height }
}
function Get-FixtureShellButton([string]$expected) {
    $condition = New-Object System.Windows.Automation.PropertyCondition -ArgumentList ([System.Windows.Automation.AutomationElement]::ControlTypeProperty), ([System.Windows.Automation.ControlType]::Button)
    $buttons = [System.Windows.Automation.AutomationElement]::RootElement.FindAll([System.Windows.Automation.TreeScope]::Descendants, $condition)
    $matches = @()
    foreach ($button in $buttons) {
        if ($button.Current.Name.StartsWith('miccamwatch', [StringComparison]::Ordinal)) {
            $hostProcess = Get-Process -Id $button.Current.ProcessId -ErrorAction SilentlyContinue
            if ($hostProcess -and $hostProcess.ProcessName -eq 'explorer') { $matches += $button }
        }
    }
    if ($matches.Count -gt 1) { throw 'More than one miccamwatch shell button exists on the isolated runner.' }
    if ($matches.Count -eq 1) {
        $name = $matches[0].Current.Name
        # Shell may retain NIM_ADD's accessible identity even after NIM_MODIFY.
        # A neutral product identity is fine; an old idle/status assertion is not.
        if ($name -ne 'miccamwatch' -and -not [string]::Equals($name, $expected, [StringComparison]::Ordinal)) {
            throw "Shell accessible identity contradicts current fixture status: $name (expected $expected or neutral product name)"
        }
        return $matches[0]
    }
    # Hosted English Windows images may hide newly added notification icons.
    # Invoke only Explorer's overflow button, never a product/control command.
    foreach ($button in $buttons) {
        if ($button.Current.Name -in @('Show hidden icons', 'Show hidden icons.', 'Hidden icons menu')) {
            $hostProcess = Get-Process -Id $button.Current.ProcessId -ErrorAction SilentlyContinue
            if ($hostProcess -and $hostProcess.ProcessName -eq 'explorer') {
                $pattern = $button.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
                $pattern.Invoke()
                return $null
            }
        }
    }
    return $null
}
function Confirm-FixtureShellTooltip([string]$name) {
    $shell = Wait-ProofFile "$name-shell.json"
    [uint32]$ownerPid = 0
    [void][McwNativeProof]::GetWindowThreadProcessId([IntPtr][long]$shell.owner, [ref]$ownerPid)
    if ($ownerPid -eq 0) { throw 'Fixture shell owner is no longer alive.' }
    if ($null -eq $script:fixtureProcess) {
        $candidate = Get-Process -Id $ownerPid -ErrorAction Stop
        $null = $candidate.Handle
        [uint32]$confirmedPid = 0
        [void][McwNativeProof]::GetWindowThreadProcessId([IntPtr][long]$shell.owner, [ref]$confirmedPid)
        if ($confirmedPid -ne $ownerPid -or $candidate.HasExited) {
            $candidate.Dispose()
            throw 'Fixture owner changed while retaining its process handle.'
        }
        $script:fixtureProcess = $candidate
    } elseif ($script:fixtureProcess.Id -ne $ownerPid -or $script:fixtureProcess.HasExited) {
        throw 'Fixture shell owner changed between observations.'
    }
    $identity = New-Object McwNativeProof+NOTIFYICONIDENTIFIER
    $identity.cbSize = [Runtime.InteropServices.Marshal]::SizeOf($identity)
    $identity.hWnd = [IntPtr][long]$shell.owner
    $identity.uID = $shell.id
    $rect = New-Object McwNativeProof+RECT
    if ([McwNativeProof]::Shell_NotifyIconGetRect([ref]$identity, [ref]$rect) -ne 0) {
        throw 'Explorer did not register the actual fixture notification icon.'
    }
    if ($shell.capacity -ne 128 -or $shell.utf16_units -gt 127) { throw 'Invalid native tooltip capacity.' }
    $deadline = (Get-Date).AddSeconds(30)
    $button = $null
    while (-not $button -and (Get-Date) -lt $deadline) {
        $button = Get-FixtureShellButton $shell.tooltip
        if (-not $button) { Start-Sleep -Milliseconds 200 }
    }
    if (-not $button) { throw 'Fixture shell icon is not UIA-accessible, including overflow. Requires an unlocked English hosted Explorer desktop.' }
    $bounds = $button.Current.BoundingRectangle
    if ($bounds.IsEmpty -or $bounds.Width -le 0 -or $bounds.Height -le 0) { throw 'Fixture shell icon has no visible bounds.' }
    # Leave the last tooltip before entering the next icon/status. This forces
    # Explorer to show freshly updated native text rather than a stale hover.
    [System.Windows.Forms.Cursor]::Position = New-Object System.Drawing.Point -ArgumentList 0, 0
    Start-Sleep -Milliseconds 200
    [System.Windows.Forms.Cursor]::Position = New-Object System.Drawing.Point -ArgumentList ([int]($bounds.X + $bounds.Width / 2)), ([int]($bounds.Y + $bounds.Height / 2))
    $condition = New-Object System.Windows.Automation.PropertyCondition -ArgumentList ([System.Windows.Automation.AutomationElement]::ControlTypeProperty), ([System.Windows.Automation.ControlType]::ToolTip)
    $deadline = (Get-Date).AddSeconds(20)
    $observed = @()
    $tooltip = $null
    while (-not $tooltip -and (Get-Date) -lt $deadline) {
        $tips = [System.Windows.Automation.AutomationElement]::RootElement.FindAll([System.Windows.Automation.TreeScope]::Descendants, $condition)
        foreach ($tip in $tips) {
            $text = $tip.Current.Name
            $observed += $text
            $hostProcess = Get-Process -Id $tip.Current.ProcessId -ErrorAction SilentlyContinue
            if ($hostProcess -and $hostProcess.ProcessName -eq 'explorer' -and [string]::Equals($text, $shell.tooltip, [StringComparison]::Ordinal)) { $tooltip = $tip; break }
        }
        if (-not $tooltip) { Start-Sleep -Milliseconds 100 }
    }
    if (-not $tooltip) {
        $observed | Select-Object -Unique | ConvertTo-Json | Set-Content -Encoding UTF8 (Join-Path $directory "$name-shell-observed.json")
        throw "Actual Explorer tooltip did not match current fixture text ($($shell.tooltip)); see $name-shell-observed.json"
    }
    $tipWindow = [IntPtr]$tooltip.Current.NativeWindowHandle
    if ($tipWindow -eq [IntPtr]::Zero) { throw 'Explorer tooltip has no native window for screenshot proof.' }
    $tipBounds = Capture-NativeWindow $tipWindow "$name-shell-tooltip"
    $script:shellMeasurements += @{
        case=$name; accessible_name=$button.Current.Name; tooltip=$tooltip.Current.Name;
        tooltip_bounds=$tipBounds; native_utf16_units=$shell.utf16_units;
        shell_rect=@{left=$rect.Left; top=$rect.Top; right=$rect.Right; bottom=$rect.Bottom}
    }
    [System.Windows.Forms.Cursor]::Position = New-Object System.Drawing.Point -ArgumentList 0, 0
    [IO.File]::WriteAllText((Join-Path $directory "$name-shell-checked"), 'actual Explorer UIA tooltip and screenshot checked')
}
try {
    # GetWindowRect must return physical pixels, matching CopyFromScreen.
    # A DPI-unaware PowerShell host otherwise crops wallpaper at scaled offsets.
    $previousDpi = [McwNativeProof]::SetThreadDpiAwarenessContext([IntPtr](-4))
    if ($previousDpi -eq [IntPtr]::Zero) { throw 'Cannot establish per-monitor-aware native measurement coordinates.' }
    $process = Start-Process cargo -ArgumentList @('test', '--locked', '--features', 'windows-tray', 'frontends::tray::tests::native_windows_tray_visual_smoke', '--', '--ignored', '--exact', '--nocapture', '--test-threads=1') -PassThru -NoNewWindow -RedirectStandardOutput (Join-Path $directory 'cargo-stdout.log') -RedirectStandardError (Join-Path $directory 'cargo-stderr.log')
    # Cache the native process handle while it is alive. Windows PowerShell
    # otherwise can report a null ExitCode for a completed Start-Process.
    $null = $process.Handle
    $measurements = @()
    Confirm-FixtureShellTooltip 'startup'
    foreach ($name in @('partial-active', 'error-active', 'recovered-active', 'long-error', 'unicode-controls', 'idle-protected')) {
        Confirm-FixtureShellTooltip $name
        $menu = Wait-ProofFile "$name-menu.json"
        $owner = [IntPtr][long]$menu.owner
        [uint32]$ownerPid = 0
        $ownerThread = [McwNativeProof]::GetWindowThreadProcessId($owner, [ref]$ownerPid)
        # Explorer may return the overflow-chevron rectangle for a hidden icon.
        # Rasterize the actual native HICON; do not mislabel the chevron as our glyph.
        $nativeIcon = [System.Drawing.Icon]::FromHandle([IntPtr][long]$menu.icon)
        $icon = $nativeIcon.ToBitmap()
        try {
            $icon.Save((Join-Path $directory "$name-native-icon.png"), [System.Drawing.Imaging.ImageFormat]::Png)
        } finally { $icon.Dispose() }
        $deadline = (Get-Date).AddSeconds(30)
        $popup = [IntPtr]::Zero
        while ((Get-Date) -lt $deadline) {
            $candidate = [McwNativeProof]::FindWindow('#32768', $null)
            [uint32]$popupPid = 0
            $popupThread = [McwNativeProof]::GetWindowThreadProcessId($candidate, [ref]$popupPid)
            if ($candidate -ne [IntPtr]::Zero -and $popupThread -eq $ownerThread -and $popupPid -eq $ownerPid) { $popup = $candidate; break }
            Start-Sleep -Milliseconds 20
        }
        if ($popup -eq [IntPtr]::Zero) { throw 'Real Windows popup did not appear on the input desktop.' }
        Start-Sleep -Milliseconds 300
        $bounds = Capture-NativeWindow $popup "$name-menu"
        $row = New-Object McwNativeProof+RECT
        if (-not [McwNativeProof]::GetMenuItemRect($owner, [IntPtr][long]$menu.menu, 1, [ref]$row)) { throw 'Native diagnostic menu row was not measurable.' }
        $limit = [Math]::Min(400 * [double]$menu.dpi / 96, [double]$menu.work_width * 0.75)
        if ($bounds.width -gt $limit) { throw "Native popup width $($bounds.width) exceeds compact bound $limit" }
        [void][McwNativeProof]::PostMessage($owner, 0x001f, [IntPtr]::Zero, [IntPtr]::Zero) # WM_CANCELMODE; no privacy action
        $details = Wait-ProofFile "$name-details.json"
        Start-Sleep -Milliseconds 300
        $detailsBounds = Capture-NativeWindow ([IntPtr][long]$details.window) "$name-details" -RenderWindow
        if ($detailsBounds.width -gt $menu.work_width -or $detailsBounds.height -gt $menu.work_height) {
            throw 'Native details window exceeds the monitor work area.'
        }
        $expected = [IO.File]::ReadAllText((Join-Path $directory "$name-diagnostic.txt"))
        $copiedUnits = $null
        # A local user's clipboard may contain arbitrary delayed-render/OLE
        # formats that cannot be snapshotted reliably. Never replace it.
        # Only a disposable runner's explicitly isolated clipboard is exercised.
        if ($IsolatedClipboard) {
            [System.Windows.Forms.Clipboard]::Clear()
            [void][McwNativeProof]::SendMessage([IntPtr][long]$details.window, 0x0111, [IntPtr]202, [IntPtr]::Zero) # actual Copy all command
            $copied = [System.Windows.Forms.Clipboard]::GetText()
            if (-not [string]::Equals($expected, $copied, [StringComparison]::Ordinal)) { throw 'Native Copy all did not preserve the complete diagnostic.' }
            [IO.File]::WriteAllText((Join-Path $directory "$name-copied.txt"), $copied)
            $copiedUnits = $copied.Length
        }
        $measurements += @{ case=$name; menu=$bounds; menu_width_limit=$limit; diagnostic_row=@{ left=$row.Left; top=$row.Top; right=$row.Right; bottom=$row.Bottom }; details=$detailsBounds; clipboard_verified=[bool]$IsolatedClipboard; copied_utf16_units=$copiedUnits; original_utf16_units=$details.document_utf16_units; text_width_limit=$menu.text_width_limit; dpi=$menu.dpi }
        [void][McwNativeProof]::PostMessage([IntPtr][long]$details.window, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)
    }
    if (-not $process.WaitForExit(30000)) { throw 'Native fixture did not finish after closing details.' }
    if ($process.ExitCode -ne 0 -or -not (Test-Path (Join-Path $directory 'complete'))) { throw "Native fixture failed ($($process.ExitCode)); see fixture logs." }
    $measurements | ConvertTo-Json -Depth 8 | Set-Content -Encoding UTF8 (Join-Path $directory 'measurements.json')
    $shellMeasurements | ConvertTo-Json -Depth 8 | Set-Content -Encoding UTF8 (Join-Path $directory 'shell-measurements.json')
    Write-Output "Verified actual Explorer tooltip/UIA updates, native popup bounds and full wrapping details (clipboard exercised: $([bool]$IsolatedClipboard)): $directory"
} finally {
    $env:MCW_WINDOWS_NATIVE_PROOF_DIR = $previous
    [System.Windows.Forms.Cursor]::Position = $previousCursor
    if ($fixtureProcess) {
        if (-not $fixtureProcess.HasExited) { $fixtureProcess.Kill(); $fixtureProcess.WaitForExit() }
        $fixtureProcess.Dispose()
    }
    if ($process) {
        if (-not $process.HasExited) { $process.Kill(); $process.WaitForExit() }
        $process.Dispose()
    }
    if ($previousDpi -and $previousDpi -ne [IntPtr]::Zero) {
        [void][McwNativeProof]::SetThreadDpiAwarenessContext($previousDpi)
    }
}
