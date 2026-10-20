param(
    [Parameter(Mandatory = $true)][string]$MsiPath,
    [Parameter(Mandatory = $true)][string]$OutputDirectory,
    [switch]$IsolatedRunner
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
# This qualification never runs the application or touches device/privacy controls.
# Only an explicitly disposable GitHub-hosted Windows VM may install this fixture.
if (-not $IsolatedRunner -or $env:OS -ne 'Windows_NT' -or
    $env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted') {
    throw 'Brand qualification requires an explicitly isolated GitHub-hosted Windows runner.'
}
$repository = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$msi = [IO.Path]::GetFullPath($MsiPath)
$proof = [IO.Path]::GetFullPath($OutputDirectory)
$temp = [IO.Path]::GetFullPath($env:RUNNER_TEMP).TrimEnd('\') + '\'
if (-not $proof.StartsWith($temp, [StringComparison]::OrdinalIgnoreCase) -or (Test-Path -LiteralPath $proof)) {
    throw 'Proof directory must be a new, retained directory beneath RUNNER_TEMP.'
}
if (-not $msi.StartsWith(($repository + '\target\'), [StringComparison]::OrdinalIgnoreCase) -or
    [IO.Path]::GetExtension($msi) -ne '.msi' -or -not (Test-Path -LiteralPath $msi -PathType Leaf)) {
    throw 'Only this checkout target-directory MSI may be qualified.'
}
foreach ($path in @($msi, $proof)) {
    if ($path.Contains('"') -or $path.Contains("`r") -or $path.Contains("`n")) { throw 'Unsafe fixture path.' }
    $ancestor = Split-Path -Parent $path
    while ($ancestor) {
        if ((Get-Item -LiteralPath $ancestor).Attributes -band [IO.FileAttributes]::ReparsePoint) {
            throw "Reparse-point fixture ancestry: $ancestor"
        }
        $ancestor = Split-Path -Parent $ancestor
    }
}
[void](New-Item -ItemType Directory -Path $proof)
$images = Join-Path $proof 'images'
[void](New-Item -ItemType Directory -Path $images)
$installDirectory = Join-Path $proof 'installation'
$canonical = Join-Path $repository 'installer/miccamwatch.ico'
$shortcut = Join-Path ([Environment]::GetFolderPath('Programs')) 'MicCamWatch.lnk'
$report = [ordered]@{
    status = 'running'; method = 'native Shell item image factory and SHDefExtractIcon bitmaps'
    dpi = 'per-monitor-v2; requested sizes are physical pixels'
    msi = $msi; installation = $installDirectory; shortcut = $shortcut
    productCode = $null; packageSha256 = $null; registration = $null
    versions = @(); comparisons = @(); limitations = @(); cleanup = $null; error = $null
}
$failure = $null
$installAttempted = $false
$productCode = $null
$installer = $null
$userPathBefore = [Environment]::GetEnvironmentVariable('PATH', 'User')
function Save-Report { $report | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath (Join-Path $proof 'result.json') -Encoding UTF8 }
function Invoke-OwnedMsi([string]$Operation, [string]$Subject, [string]$Log, [string]$Properties = '') {
    $arguments = "$Operation `"$Subject`" /qn /norestart /L*V `"$Log`" $Properties"
    $process = Start-Process -FilePath (Join-Path $env:WINDIR 'System32/msiexec.exe') -ArgumentList $arguments -PassThru
    try {
        if (-not $process.WaitForExit(180000)) {
            # Retain the precise child process handle, never kill by executable name.
            $process.Kill()
            throw "Owned msiexec exceeded 180 seconds; inspect $Log"
        }
        $process.Refresh()
        if ($process.ExitCode -notin @(0, 3010)) { throw "Owned msiexec exited $($process.ExitCode); inspect $Log" }
        return $process.ExitCode
    } finally { $process.Dispose() }
}
Save-Report
try {
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
public static class McwBrandNative {
    [StructLayout(LayoutKind.Sequential)] public struct SIZE { public int cx, cy; }
    [ComImport, Guid("bcc18b79-ba16-442f-80c4-8a59c30c463b"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IShellItemImageFactory {
        [PreserveSig] int GetImage(SIZE size, uint flags, out IntPtr bitmap);
    }
    [DllImport("shell32.dll", CharSet=CharSet.Unicode, PreserveSig=true)]
    static extern int SHCreateItemFromParsingName(string path, IntPtr context, ref Guid iid, [MarshalAs(UnmanagedType.Interface)] out IShellItemImageFactory item);
    [DllImport("shell32.dll", CharSet=CharSet.Unicode)]
    static extern int SHDefExtractIcon(string file, int index, uint flags, out IntPtr large, out IntPtr small, uint size);
    [DllImport("user32.dll")] static extern bool DestroyIcon(IntPtr icon);
    [DllImport("user32.dll", SetLastError=true)] static extern bool DrawIconEx(IntPtr dc, int x, int y, IntPtr icon, int width, int height, uint step, IntPtr brush, uint flags);
    [DllImport("gdi32.dll")] static extern bool DeleteObject(IntPtr value);
    [DllImport("user32.dll", SetLastError=true)] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
    public static void ShellImage(string source, int size, string output) {
        Guid iid = typeof(IShellItemImageFactory).GUID;
        IShellItemImageFactory item;
        Marshal.ThrowExceptionForHR(SHCreateItemFromParsingName(source, IntPtr.Zero, ref iid, out item));
        IntPtr bitmap = IntPtr.Zero;
        try {
            // ICONONLY, no thumbnail, link overlay, or synthetic registry image.
            Marshal.ThrowExceptionForHR(item.GetImage(new SIZE { cx=size, cy=size }, 4, out bitmap));
            if (bitmap == IntPtr.Zero) throw new Exception("Shell returned no image: " + source);
            using (Bitmap native = Bitmap.FromHbitmap(bitmap)) {
                if (native.Width != size || native.Height != size) throw new Exception("Shell returned unexpected dimensions for " + source);
                native.Save(output, ImageFormat.Png);
            }
        } finally {
            if (bitmap != IntPtr.Zero) DeleteObject(bitmap);
            Marshal.FinalReleaseComObject(item);
        }
    }
    public static void ExtractImage(string source, int index, int size, string output) {
        IntPtr large = IntPtr.Zero, small = IntPtr.Zero;
        try {
            Marshal.ThrowExceptionForHR(SHDefExtractIcon(source, index, 0, out large, out small, (uint)(size | (size << 16))));
            if (large == IntPtr.Zero) throw new Exception("Native extractor returned no icon: " + source);
            using (Bitmap image = new Bitmap(size, size, PixelFormat.Format32bppArgb))
            using (Graphics graphics = Graphics.FromImage(image)) {
                // FromHbitmap's RGB surface flattens transparent Shell pixels
                // onto black; use the same matte for native HICON comparison.
                graphics.Clear(Color.Black);
                IntPtr dc = graphics.GetHdc();
                try {
                    if (!DrawIconEx(dc, 0, 0, large, size, size, 0, IntPtr.Zero, 3)) throw new Exception("DrawIconEx failed");
                } finally { graphics.ReleaseHdc(dc); }
                image.Save(output, ImageFormat.Png);
            }
        } finally {
            if (large != IntPtr.Zero) DestroyIcon(large);
            if (small != IntPtr.Zero && small != large) DestroyIcon(small);
        }
    }
    public static double[] Difference(string expected, string actual) {
        using (Bitmap a = new Bitmap(expected))
        using (Bitmap b = new Bitmap(actual)) {
            if (a.Size != b.Size) throw new Exception("Bitmap dimensions differ");
            long total = 0, changed = 0;
            int minimum = 255, maximum = 0;
            for (int y=0; y<a.Height; ++y) for (int x=0; x<a.Width; ++x) {
                Color p = a.GetPixel(x,y), q = b.GetPixel(x,y);
                int delta = Math.Abs(p.R-q.R) + Math.Abs(p.G-q.G) + Math.Abs(p.B-q.B);
                total += delta;
                if (delta > 18) ++changed;
                minimum = Math.Min(minimum, Math.Min(p.R, Math.Min(p.G,p.B)));
                maximum = Math.Max(maximum, Math.Max(p.R, Math.Max(p.G,p.B)));
            }
            if (maximum - minimum < 32) throw new Exception("Canonical rendered image is blank");
            return new double[] { (double)total/(a.Width*a.Height*3), (double)changed/(a.Width*a.Height) };
        }
    }
    public static void ContactSheet(string[] paths, string[] labels, string output) {
        const int columns = 9, cellWidth = 290, cellHeight = 310;
        int rows = (paths.Length + columns - 1) / columns;
        using (Bitmap sheet = new Bitmap(columns*cellWidth, rows*cellHeight))
        using (Graphics graphics = Graphics.FromImage(sheet))
        using (Font font = new Font("Segoe UI", 10)) {
            graphics.Clear(Color.FromArgb(232,232,232));
            for (int i=0; i<paths.Length; ++i) {
                int x=(i%columns)*cellWidth, y=(i/columns)*cellHeight;
                graphics.DrawString(labels[i], font, Brushes.Black, x+6, y+4);
                using (Bitmap image = new Bitmap(paths[i])) graphics.DrawImageUnscaled(image, x+12, y+40);
            }
            sheet.Save(output, ImageFormat.Png);
        }
    }
}
'@ -ReferencedAssemblies System.Drawing
} catch {
    $report.status = 'failed'
    $report.error = $_.ToString()
    Save-Report
    throw
}
$sheetPaths = [Collections.Generic.List[string]]::new()
$sheetLabels = [Collections.Generic.List[string]]::new()
function Compare-Image([string]$Surface, [string]$Method, [string]$Source, [int]$Index, [int]$Size, [string]$Expected) {
    $output = Join-Path $images "$Surface-$Method-$Size.png"
    if ($Method -eq 'shell') { [McwBrandNative]::ShellImage($Source, $Size, $output) }
    else { [McwBrandNative]::ExtractImage($Source, $Index, $Size, $output) }
    $sheetPaths.Add($output)
    $sheetLabels.Add("$Surface / $Method / $Size px")
    $difference = [McwBrandNative]::Difference($Expected, $output)
    $matches = $difference[0] -le 2.0 -and $difference[1] -le 0.03
    $report.comparisons += [ordered]@{
        surface = $Surface; method = $Method; size = $Size; source = $Source; index = $Index
        expected = $Expected; actual = $output; meanRgbError = $difference[0]
        fractionPixelsOverSixLevels = $difference[1]; matchesCanonical = $matches
    }
    Save-Report
    if (-not $matches) { throw "$Surface $Method $Size px differs from official icon; inspect retained PNGs." }
}
try {
    $report.packageSha256 = (Get-FileHash -LiteralPath $msi -Algorithm SHA256).Hash
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
    if ($properties['ProductName'] -ne 'MicCamWatch' -or $productCode -notmatch '^\{[0-9A-Fa-f-]{36}\}$' -or
        $properties['UpgradeCode'] -ne '{8FC9317B-4517-4E83-9F52-CF3257584BF6}') {
        throw 'MSI identity does not match the expected owned qualification product.'
    }
    $report.productCode = $productCode
    # Refuse rather than upgrade/remove any existing MCW or replace a shortcut.
    $related = $installer.RelatedProducts($properties['UpgradeCode'])
    try { $relatedCount = $related.Count }
    finally { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($related) }
    if ($installer.ProductState($productCode) -ne -1 -or $relatedCount -ne 0 -or
        (Test-Path -LiteralPath $shortcut) -or (Test-Path -LiteralPath (Join-Path ([Environment]::GetFolderPath('CommonPrograms')) 'MicCamWatch.lnk'))) {
        throw 'Existing MCW registration/shortcut: refusing fixture installation.'
    }
    $arpPath = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\$productCode"
    if (Test-Path -LiteralPath $arpPath) { throw 'Fixture Apps registration already exists.' }
    Save-Report
    $installAttempted = $true
    $report.installExitCode = Invoke-OwnedMsi '/i' $msi (Join-Path $proof 'install-msi.log') "INSTALLFOLDER=`"$installDirectory`" ALLUSERS= MSIINSTALLPERUSER=1"
    if ($installer.ProductState($productCode) -ne 5) { throw 'Fixture MSI is not registered as locally installed.' }
    $registeredLocation = $installer.ProductInfo($productCode, 'InstallLocation')
    if (-not [string]::Equals($registeredLocation.TrimEnd('\'), $installDirectory, [StringComparison]::OrdinalIgnoreCase)) {
        throw "MSI InstallLocation did not resolve to the retained fixture path: $registeredLocation"
    }
    $arp = Get-ItemProperty -LiteralPath $arpPath
    if ($arp.DisplayName -ne 'MicCamWatch' -or $arp.WindowsInstaller -ne 1 -or
        -not [string]::Equals($arp.InstallLocation.TrimEnd('\'), $installDirectory, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Actual installed Apps registration does not identify this fixture installation.'
    }
    $displayIcon = [Environment]::ExpandEnvironmentVariables([string]$arp.DisplayIcon)
    $iconIndex = 0
    $iconFile = $displayIcon.Trim('"')
    if ($displayIcon -match '^\s*"?(.+?)"?\s*,\s*(-?\d+)\s*$') {
        $iconFile = $Matches[1].Trim('"'); $iconIndex = [int]$Matches[2]
    }
    if (-not [IO.Path]::IsPathRooted($iconFile) -or -not (Test-Path -LiteralPath $iconFile -PathType Leaf)) {
        throw "Apps DisplayIcon cannot be resolved to an actual installed icon: $displayIcon"
    }
    $report.registration = [ordered]@{
        registryPath = $arpPath; displayName = $arp.DisplayName; displayVersion = $arp.DisplayVersion
        installLocation = $arp.InstallLocation; displayIcon = $displayIcon; resolvedIcon = $iconFile
        iconIndex = $iconIndex; uninstallString = $arp.UninstallString
        msiLocalPackage = $installer.ProductInfo($productCode, 'LocalPackage')
    }
    if (-not (Test-Path -LiteralPath $shortcut -PathType Leaf)) { throw 'Actual user Start Menu shortcut was not installed.' }
    $shell = New-Object -ComObject WScript.Shell
    try {
        $link = $shell.CreateShortcut($shortcut)
        try {
            $expectedTray = Join-Path $installDirectory 'mcw-tray.exe'
            $report.shortcutDetails = @{ target = $link.TargetPath; arguments = $link.Arguments; iconLocation = $link.IconLocation; workingDirectory = $link.WorkingDirectory }
            if (-not [string]::Equals($link.TargetPath, $expectedTray, [StringComparison]::OrdinalIgnoreCase) -or $link.Arguments) {
                throw 'Real Start Menu shortcut does not target the owned installed tray executable.'
            }
        } finally { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($link) }
    } finally { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($shell) }
    Copy-Item -LiteralPath $shortcut -Destination (Join-Path $proof 'installed-MicCamWatch.lnk')
    Copy-Item -LiteralPath $iconFile -Destination (Join-Path $proof ('registered-Apps-icon' + [IO.Path]::GetExtension($iconFile)))
    foreach ($name in @('mcw.exe', 'mcw-tray.exe')) {
        $installed = Join-Path $installDirectory $name
        $built = Join-Path $repository "target/release/$name"
        if ((Get-FileHash -LiteralPath $installed).Hash -ne (Get-FileHash -LiteralPath $built).Hash) { throw "Installed $name differs from actual build." }
        $version = [Diagnostics.FileVersionInfo]::GetVersionInfo($installed)
        $report.versions += @{ file = $installed; fileVersion = $version.FileVersion; productVersion = $version.ProductVersion; hasVersionResource = ($version.FileMajorPart -ne 0 -or $version.FileMinorPart -ne 0 -or [bool]$version.FileVersion) }
        if (-not $version.FileVersion) { $report.limitations += "$name has no FileVersion/VERSIONINFO; Explorer version metadata is not qualified by this icon-only proof." }
    }
    $previousDpi = [McwBrandNative]::SetThreadDpiAwarenessContext([IntPtr](-4))
    if ($previousDpi -eq [IntPtr]::Zero) { throw 'Cannot establish per-monitor-v2 native icon rendering.' }
    try {
        foreach ($size in @(16, 20, 24, 32, 40, 48, 64, 128, 256)) {
            $expectedShell = Join-Path $images "official-shell-$size.png"
            $expectedNative = Join-Path $images "official-extract-$size.png"
            [McwBrandNative]::ShellImage($canonical, $size, $expectedShell)
            [McwBrandNative]::ExtractImage($canonical, 0, $size, $expectedNative)
            $sheetPaths.Add($expectedShell); $sheetLabels.Add("official / shell / $size px")
            $sheetPaths.Add($expectedNative); $sheetLabels.Add("official / extract / $size px")
            # A generic Shell icon for the .ico itself must never become our
            # expected image. Cross-check it against native ICO extraction.
            Compare-Image 'official-native-crosscheck' 'shell' $canonical 0 $size $expectedNative
            foreach ($name in @('mcw', 'mcw-tray')) {
                $installed = Join-Path $installDirectory "$name.exe"
                Compare-Image $name 'shell' $installed 0 $size $expectedShell
                Compare-Image $name 'extract' $installed 0 $size $expectedNative
            }
            # This is the actual installed Start Menu link, not a synthesized link
            # or a copied registry value. Shell resolves its icon on the consumer path.
            Compare-Image 'start-menu' 'shell' $shortcut 0 $size $expectedShell
            # Apps consumes DisplayIcon as an icon-location string: resolve the
            # registered file/index through Shell's native icon extractor itself.
            Compare-Image 'installed-apps' 'extract' $iconFile $iconIndex $size $expectedNative
        }
    } finally { [void][McwBrandNative]::SetThreadDpiAwarenessContext($previousDpi) }
    $report.status = 'passed'
} catch {
    $failure = $_
    $report.status = 'failed'
    $report.error = $_.ToString() + "`n" + $_.ScriptStackTrace
} finally {
    # Keep the MSI, receipt metadata, native PNGs, logs, and partial installation
    # evidence. Uninstall only the product absent before our own /i attempt.
    if ($installAttempted -and $productCode -and $null -ne $installer) {
        try {
            if (Test-Path -LiteralPath $installDirectory -PathType Container) {
                Copy-Item -LiteralPath $installDirectory -Destination (Join-Path $proof 'installed-before-cleanup') -Recurse
            }
            if (Test-Path -LiteralPath $arpPath) {
                Get-ItemProperty -LiteralPath $arpPath | ConvertTo-Json -Depth 8 |
                    Set-Content -LiteralPath (Join-Path $proof 'installed-Apps-registration.json') -Encoding UTF8
            }
        } catch {
            $report.status = 'failed'
            $report.limitations += "Retaining installer receipt/state failed: $_"
            if ($null -eq $failure) { $failure = $_ }
        }
        try {
            if ((Get-FileHash -LiteralPath $msi -Algorithm SHA256).Hash -ne $report.packageSha256) { throw 'Retained fixture MSI changed; refusing cleanup.' }
            $state = $installer.ProductState($productCode)
            if ($state -ne -1) {
                $exit = Invoke-OwnedMsi '/x' $productCode (Join-Path $proof 'uninstall-msi.log')
                if ($installer.ProductState($productCode) -ne -1 -or (Test-Path -LiteralPath $shortcut) -or (Test-Path -LiteralPath $arpPath)) {
                    throw 'Owned MSI uninstall did not remove its registration/shortcut.'
                }
                $report.cleanup = @{ status = 'uninstalled'; productCode = $productCode; exitCode = $exit }
            } else { $report.cleanup = @{ status = 'not-registered'; productCode = $productCode } }
            if ([Environment]::GetEnvironmentVariable('PATH', 'User') -cne $userPathBefore) { throw 'MSI cleanup did not restore the pre-fixture user PATH.' }
        } catch {
            $report.cleanup = @{ status = 'failed'; productCode = $productCode; error = $_.ToString() }
            $report.status = 'failed'
            if ($null -eq $failure) { $failure = $_ }
        }
    }
    if ($null -ne $installer) { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($installer) }
    try {
        if ($sheetPaths.Count -gt 0) { [McwBrandNative]::ContactSheet($sheetPaths.ToArray(), $sheetLabels.ToArray(), (Join-Path $proof 'shell-icons-contact-sheet.png')) }
    } catch {
        $report.status = 'failed'
        $report.limitations += "Contact sheet failed: $_"
        if ($null -eq $failure) { $failure = $_ }
    }
    Save-Report
}
if ($null -ne $failure) { throw $failure }
Write-Output "Native Shell branding proof passed; retained evidence: $proof"
