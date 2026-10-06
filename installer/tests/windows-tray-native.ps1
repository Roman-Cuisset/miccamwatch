param(
    [string]$OutputDirectory = 'artifacts/native-windows-tray',
    [switch]$IsolatedClipboard
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class McwNativeProof {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string cls, string title);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd, out RECT rect);
    [DllImport("user32.dll")] public static extern bool GetMenuItemRect(IntPtr hwnd, IntPtr menu, uint item, out RECT rect);
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
$fixturePid = 0
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
function Capture-NativeWindow([IntPtr]$window, [string]$name) {
    $rect = New-Object McwNativeProof+RECT
    if (-not [McwNativeProof]::GetWindowRect($window, [ref]$rect)) { throw "Cannot measure $name" }
    $width = $rect.Right - $rect.Left
    $height = $rect.Bottom - $rect.Top
    if ($width -le 0 -or $height -le 0) { throw "Empty native $name rectangle" }
    $bitmap = New-Object System.Drawing.Bitmap -ArgumentList $width, $height
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    try {
        $graphics.CopyFromScreen($rect.Left, $rect.Top, 0, 0, $bitmap.Size)
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
    foreach ($name in @('long-error', 'unicode-controls')) {
        $menu = Wait-ProofFile "$name-menu.json"
        $owner = [IntPtr][long]$menu.owner
        [uint32]$ownerPid = 0
        $ownerThread = [McwNativeProof]::GetWindowThreadProcessId($owner, [ref]$ownerPid)
        $fixturePid = $ownerPid
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
        $limit = [Math]::Min(600 * [double]$menu.dpi / 96, [double]$menu.work_width * 0.75)
        if ($bounds.width -gt $limit) { throw "Native popup width $($bounds.width) exceeds compact bound $limit" }
        [void][McwNativeProof]::PostMessage($owner, 0x001f, [IntPtr]::Zero, [IntPtr]::Zero) # WM_CANCELMODE; no privacy action
        $details = Wait-ProofFile "$name-details.json"
        Start-Sleep -Milliseconds 300
        $detailsBounds = Capture-NativeWindow ([IntPtr][long]$details.window) "$name-details"
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
    Write-Output "Verified native Windows popup bounds and full wrapping details (clipboard exercised: $([bool]$IsolatedClipboard)): $directory"
} finally {
    $env:MCW_WINDOWS_NATIVE_PROOF_DIR = $previous
    if ($fixturePid -ne 0 -and (Get-Process -Id $fixturePid -ErrorAction SilentlyContinue)) {
        Stop-Process -Id $fixturePid
    }
    if ($process -and -not $process.HasExited) { Stop-Process -Id $process.Id }
    if ($previousDpi -and $previousDpi -ne [IntPtr]::Zero) {
        [void][McwNativeProof]::SetThreadDpiAwarenessContext($previousDpi)
    }
}
