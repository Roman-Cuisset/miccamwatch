param([string]$OutputDirectory = 'artifacts/native-windows-desktop')
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class McwDesktopProof {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr window, out RECT rect);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr window, IntPtr dc, uint flags);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr window, out uint pid);
    [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
    [DllImport("user32.dll")] public static extern IntPtr GetThreadDesktop(uint thread);
    [DllImport("user32.dll")] public static extern IntPtr OpenInputDesktop(uint flags, bool inherit, uint access);
    [DllImport("user32.dll")] public static extern bool CloseDesktop(IntPtr desktop);
    [DllImport("user32.dll", CharSet=CharSet.Unicode, SetLastError=true)] private static extern bool GetUserObjectInformation(IntPtr handle, int index, StringBuilder buffer, uint bytes, out uint needed);
    public static string DesktopName(IntPtr handle) {
        var text = new StringBuilder(256); uint needed;
        if (!GetUserObjectInformation(handle, 2, text, 512, out needed)) throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
        return text.ToString();
    }
    [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr window);
    [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr window);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr window, StringBuilder name, int count);
}
'@
if (-not [Environment]::UserInteractive) { throw 'This proof needs an interactive unlocked Windows desktop; no unlock or desktop switch is attempted.' }
$inputDesktop = [McwDesktopProof]::OpenInputDesktop(0, $false, 1)
if ($inputDesktop -eq [IntPtr]::Zero) { throw 'No accessible input desktop.' }
try { $inputName = [McwDesktopProof]::DesktopName($inputDesktop) }
finally { [void][McwDesktopProof]::CloseDesktop($inputDesktop) }
$threadName = [McwDesktopProof]::DesktopName([McwDesktopProof]::GetThreadDesktop([McwDesktopProof]::GetCurrentThreadId()))
if ($inputName -ne $threadName -or $inputName -eq 'Winlogon') { throw 'Thread is not on the unlocked input desktop.' }
$scope = [IO.Path]::GetFullPath((Join-Path $OutputDirectory ((Get-Date -Format 'yyyyMMdd-HHmmss') + '-' + $PID)))
$directory = Join-Path $scope 'proof'
foreach ($path in @($directory, (Join-Path $scope 'roaming'), (Join-Path $scope 'local'))) { [void][IO.Directory]::CreateDirectory($path) }
$names = @('APPDATA', 'LOCALAPPDATA', 'MCW_WINDOWS_DESKTOP_SCOPE', 'MCW_WINDOWS_DESKTOP_PROOF_DIR')
$previous = @{}
foreach ($name in $names) { $previous[$name] = [Environment]::GetEnvironmentVariable($name, 'Process') }
$foreground = [McwDesktopProof]::GetForegroundWindow()
$previousDpi = [IntPtr]::Zero
$process = $null
$fixturePid = $null
$records = @()
try {
    $previousDpi = [McwDesktopProof]::SetThreadDpiAwarenessContext([IntPtr](-4))
    if ($previousDpi -eq [IntPtr]::Zero) { throw 'Cannot establish physical-pixel capture coordinates.' }
    $env:APPDATA = Join-Path $scope 'roaming'
    $env:LOCALAPPDATA = Join-Path $scope 'local'
    $env:MCW_WINDOWS_DESKTOP_SCOPE = $scope
    $env:MCW_WINDOWS_DESKTOP_PROOF_DIR = $directory
    # No product process, installed tray, SDK, camera, autostart or clipboard API
    # is contacted. The production HWND is hosted by a dedicated inert test owner.
    # On Windows cargo.exe in .cargo/bin can be the rustup shim: it spawns a
    # second Cargo process. Launch the selected toolchain Cargo directly so the
    # strict test-executable parent-PID check measures the actual owner.
    $cargoExecutable = (& rustup which cargo)
    if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $cargoExecutable -PathType Leaf)) { throw 'Cannot resolve the selected toolchain Cargo executable.' }
    $process = Start-Process $cargoExecutable -ArgumentList @('test', '--locked', '--features', 'windows-tray', 'frontends::tray::windows_desktop::tests::native_windows_desktop', '--', '--ignored', '--exact', '--nocapture', '--test-threads=1') -PassThru -NoNewWindow -RedirectStandardOutput (Join-Path $scope 'cargo-stdout.log') -RedirectStandardError (Join-Path $scope 'cargo-stderr.log')
    $null = $process.Handle
    $deadline = (Get-Date).AddMinutes(20)
    $step = 0
    while (-not (Test-Path (Join-Path $directory 'complete.json'))) {
        if ((Get-Date) -gt $deadline) { throw 'Native desktop proof exceeded bounded deadline; see retained logs.' }
        $stage = Join-Path $directory ('stage-{0:d3}.json' -f $step)
        if (-not (Test-Path $stage)) {
            if ($process.HasExited) { throw "Native proof exited ($($process.ExitCode)) before completion; see $scope" }
            Start-Sleep -Milliseconds 50
            continue
        }
        # Writer closes each small JSON before waiting for us; retry only a
        # transient incomplete read rather than trusting a partial HWND record.
        try { $record = Get-Content -Raw -Encoding UTF8 $stage | ConvertFrom-Json }
        catch { Start-Sleep -Milliseconds 25; continue }
        $window = [IntPtr][long]$record.window
        [uint32]$windowPid = 0
        [void][McwDesktopProof]::GetWindowThreadProcessId($window, [ref]$windowPid)
        if ($windowPid -ne [uint32]$record.pid -or $windowPid -eq 0) { throw 'Capture HWND does not belong to the reported proof process.' }
        if ($null -eq $fixturePid) {
            $fixture = Get-CimInstance Win32_Process -Filter "ProcessId=$windowPid"
            if (-not $fixture -or $fixture.ParentProcessId -ne $process.Id -or -not $fixture.Name.StartsWith('miccamwatch-')) { throw 'Proof process is not the cargo-owned Rust test executable.' }
            $fixturePid = $windowPid
        }
        if ($windowPid -ne $fixturePid) { throw 'Proof process unexpectedly changed.' }
        $class = New-Object Text.StringBuilder 128
        [void][McwDesktopProof]::GetClassName($window, $class, 128)
        if ($class.ToString() -ne 'MicCamWatchDesktopClass') { throw 'Capture is not the production retained desktop class.' }
        $rect = New-Object McwDesktopProof+RECT
        if (-not [McwDesktopProof]::GetWindowRect($window, [ref]$rect)) { throw 'Cannot measure production HWND.' }
        $width = $rect.Right - $rect.Left
        $height = $rect.Bottom - $rect.Top
        if ($width -le 0 -or $height -le 0 -or $width -gt 8192 -or $height -gt 8192) { throw 'Invalid capture dimensions.' }
        Start-Sleep -Milliseconds 150
        $bitmap = New-Object Drawing.Bitmap $width, $height
        $graphics = [Drawing.Graphics]::FromImage($bitmap)
        $dc = $graphics.GetHdc()
        try { if (-not [McwDesktopProof]::PrintWindow($window, $dc, 2)) { throw 'PrintWindow rejected the production HWND; no wallpaper/foreign-window fallback.' } }
        finally { $graphics.ReleaseHdc($dc); $graphics.Dispose() }
        try { $bitmap.Save((Join-Path $directory ($record.name + '.png')), [Drawing.Imaging.ImageFormat]::Png) }
        finally { $bitmap.Dispose() }
        $records += @{ case=$record.name; dpi=$record.dpi; tab=$record.tab; scroll=$record.scroll; width=$width; height=$height; simulation=$record.simulation; background=$record.background; foreground=$record.foreground; screenshot=($record.name + '.png') }
        [IO.File]::WriteAllText((Join-Path $directory ('stage-{0:d3}.captured' -f $step)), 'captured production HWND only')
        $step++
    }
    if (-not $process.WaitForExit(30000) -or $process.ExitCode -ne 0) { throw 'Native proof failed after captures; see retained cargo logs.' }
    $complete = Get-Content -Raw -Encoding UTF8 (Join-Path $directory 'complete.json') | ConvertFrom-Json
    if ($complete.captures -ne $records.Count -or $complete.languages -ne 7 -or $complete.device_actions_executed -ne 0 -or $complete.autostart_actions_executed -ne 0) { throw 'Native proof completion record does not satisfy the isolated contract.' }
    ConvertTo-Json -InputObject @($records) -Depth 5 | Set-Content -Encoding UTF8 (Join-Path $directory 'captures.json')
    @{ input_desktop=$inputName; current_theme='Actual OS theme is captured in native cases; explicit light/dark palette cases are simulated rendering, not global theme validation.'; screens=@([Windows.Forms.Screen]::AllScreens | ForEach-Object { @{ device=$_.DeviceName; bounds=$_.Bounds.ToString(); work_area=$_.WorkingArea.ToString() } }); settings_scope=$scope; installed_tray_contacted=$false; clipboard_accessed=$false; hardware_validation=$false } | ConvertTo-Json -Depth 5 | Set-Content -Encoding UTF8 (Join-Path $directory 'environment.json')
    Write-Output "Native production desktop behavior passed; inspect screenshots before claiming visual quality: $directory"
} finally {
    foreach ($name in $names) { [Environment]::SetEnvironmentVariable($name, $previous[$name], 'Process') }
    if ($foreground -ne [IntPtr]::Zero -and [McwDesktopProof]::IsWindow($foreground)) { [void][McwDesktopProof]::SetForegroundWindow($foreground) }
    if ($previousDpi -ne [IntPtr]::Zero) { [void][McwDesktopProof]::SetThreadDpiAwarenessContext($previousDpi) }
    if ($process) {
        if (-not $process.HasExited) { Write-Warning "Proof cargo process $($process.Id) is still running; bounded Rust capture timeout will end it. No force-kill or foreign-window cleanup attempted. Evidence: $scope" }
        $process.Dispose()
    }
}
