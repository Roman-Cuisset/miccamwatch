param(
    [long]$Window,
    [int]$OwnerPid,
    [long]$OwnerBirthTicks,
    [string]$OwnerExecutable,
    [string]$OutputPath,
    [switch]$IsolatedDesktop
)
$ErrorActionPreference = 'Stop'
if (-not $IsolatedDesktop -or $env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted') {
    throw 'Native menu rendering requires explicitly isolated GitHub-hosted qualification.'
}
$expectedImage = [IO.Path]::GetFullPath($OwnerExecutable)
$target = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../target')) + [IO.Path]::DirectorySeparatorChar
if (-not $expectedImage.StartsWith($target, [StringComparison]::OrdinalIgnoreCase) -or $expectedImage -notmatch '[\\/]deps[\\/]miccamwatch-[0-9a-f]+\.exe$') {
    throw 'Only the repository library-test image can grant native menu rendering.'
}
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class McwOwnedMenuRender {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr window, IntPtr dc, uint flags);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr window, out RECT rect);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr window, out uint pid);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr window);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr window, StringBuilder name, int maximum);
    [DllImport("user32.dll", SetLastError=true)] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
}
'@
# PrintWindow is synchronous. The parent retains this worker's exact process
# handle and kills only this worker if rendering exceeds its finite deadline.
$owner = [System.Diagnostics.Process]::GetProcessById($OwnerPid)
try {
    $null = $owner.Handle
    if ($owner.HasExited -or $owner.StartTime.ToUniversalTime().Ticks -ne $OwnerBirthTicks -or
        -not [string]::Equals($owner.MainModule.FileName, $expectedImage, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Retained native menu owner lifetime/image does not match.'
    }
    $windowHandle = [IntPtr]$Window
    [uint32]$windowPid = 0
    $thread = [McwOwnedMenuRender]::GetWindowThreadProcessId($windowHandle, [ref]$windowPid)
    $class = New-Object System.Text.StringBuilder -ArgumentList 256
    if ($thread -eq 0 -or $windowPid -ne $OwnerPid -or
        [McwOwnedMenuRender]::GetClassName($windowHandle, $class, $class.Capacity) -le 0 -or
        $class.ToString() -ne '#32768' -or -not [McwOwnedMenuRender]::IsWindowVisible($windowHandle)) {
        throw 'Only the exact owned visible native menu can be rendered.'
    }
    $previousDpi = [McwOwnedMenuRender]::SetThreadDpiAwarenessContext([IntPtr](-4))
    if ($previousDpi -eq [IntPtr]::Zero) { throw 'Cannot establish native menu pixel coordinates.' }
    try {
        $rect = New-Object McwOwnedMenuRender+RECT
        if (-not [McwOwnedMenuRender]::GetWindowRect($windowHandle, [ref]$rect)) { throw 'Native menu rectangle unavailable.' }
        $width = $rect.Right - $rect.Left
        $height = $rect.Bottom - $rect.Top
        if ($width -le 0 -or $width -gt 400 -or $height -le 0 -or $height -gt 2000) { throw 'Native menu rectangle exceeds its bounds.' }
        $bitmap = New-Object System.Drawing.Bitmap -ArgumentList $width, $height, ([System.Drawing.Imaging.PixelFormat]::Format24bppRgb)
        $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
        try {
            $dc = $graphics.GetHdc()
            try {
                if (-not [McwOwnedMenuRender]::PrintWindow($windowHandle, $dc, 2)) { throw 'Native menu PrintWindow failed.' }
            } finally { $graphics.ReleaseHdc($dc) }
            if ($owner.HasExited) { throw 'Native menu owner exited during rendering.' }
            $bitmap.Save([IO.Path]::GetFullPath($OutputPath), [System.Drawing.Imaging.ImageFormat]::Png)
            @{ method='PrintWindow'; window=$Window; pid=$OwnerPid; width=$width; height=$height } | ConvertTo-Json -Compress
        } finally { $graphics.Dispose(); $bitmap.Dispose() }
    } finally { [void][McwOwnedMenuRender]::SetThreadDpiAwarenessContext($previousDpi) }
} finally { $owner.Dispose() }
