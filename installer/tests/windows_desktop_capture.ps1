# Temporary read-only screenshot of the disposable provider's existing desktop.
param([Parameter(Mandatory=$true)][string]$OutputDirectory, [int]$DelaySeconds=60)
$ErrorActionPreference='Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted') { throw 'Disposable hosted runner required' }
Start-Sleep -Seconds $DelaySeconds
Add-Type -AssemblyName System.Windows.Forms,System.Drawing
Add-Type @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class McwVisibleWindows {
    public delegate bool Callback(IntPtr window, IntPtr parameter);
    [DllImport("user32.dll")] static extern bool EnumWindows(Callback callback, IntPtr parameter);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr window);
    [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int GetWindowTextW(IntPtr window,StringBuilder text,int maximum);
    public static string[] Titles() {
        var titles=new List<string>();
        EnumWindows((w,p)=> { if(IsWindowVisible(w)) { var s=new StringBuilder(1024); GetWindowTextW(w,s,s.Capacity); if(s.Length>0) titles.Add(s.ToString()); } return true; },IntPtr.Zero);
        return titles.ToArray();
    }
}
'@
$bounds=[Windows.Forms.SystemInformation]::VirtualScreen
$image=New-Object Drawing.Bitmap($bounds.Width,$bounds.Height)
$graphics=[Drawing.Graphics]::FromImage($image)
try {
    $graphics.CopyFromScreen($bounds.Location,[Drawing.Point]::Empty,$bounds.Size)
    $image.Save((Join-Path $OutputDirectory 'provider-desktop.png'),[Drawing.Imaging.ImageFormat]::Png)
    [McwVisibleWindows]::Titles() | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $OutputDirectory 'provider-window-titles.json') -Encoding UTF8
} finally { $graphics.Dispose(); $image.Dispose() }
