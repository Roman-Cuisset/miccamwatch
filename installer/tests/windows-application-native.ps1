param(
    [Parameter(Mandatory = $true)][string]$InstallDirectory,
    [Parameter(Mandatory = $true)][string]$ShortcutPath,
    [Parameter(Mandatory = $true)][string]$ProductCode,
    [Parameter(Mandatory = $true)][string]$OutputDirectory,
    [switch]$IsolatedRunner,
    [switch]$RequireStandardUser,
    [switch]$LocalDisposableVm,
    [string]$FixtureRoot
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
# Called after a guarded per-user MSI install by windows-brand-native.ps1 or
# native_uninstall.py, before that owner delegates/removes the same MSI.
# Never set/fake hosted-runner environment variables to run this on a user PC.
if (-not $IsolatedRunner -or -not [Environment]::UserInteractive) {
    throw 'Application integration requires an explicitly isolated interactive Windows fixture.'
}
. (Join-Path $PSScriptRoot 'windows-fixture-context.ps1')
$fixtureHost=Assert-McwDisposableFixtureHost -RequireStandardUser:$RequireStandardUser -LocalDisposableVm:$LocalDisposableVm -FixtureRoot $FixtureRoot
$install = [IO.Path]::GetFullPath($InstallDirectory).TrimEnd('\')
$shortcut = [IO.Path]::GetFullPath($ShortcutPath)
$proof = [IO.Path]::GetFullPath($OutputDirectory)
$temp = [IO.Path]::GetFullPath($fixtureHost.fixtureRoot).TrimEnd('\') + '\'
foreach ($path in @($install, $proof)) {
    if (-not $path.StartsWith($temp, [StringComparison]::OrdinalIgnoreCase) -or $path.Contains('"') -or $path.Contains("`n") -or $path.Contains("`r")) {
        throw 'Application fixture paths must be inside the admitted private fixture root.'
    }
    $ancestor = $path
    while ($ancestor) {
        if ((Test-Path -LiteralPath $ancestor) -and ((Get-Item -LiteralPath $ancestor -Force).Attributes -band [IO.FileAttributes]::ReparsePoint)) {
            throw "Reparse-point fixture ancestry: $ancestor"
        }
        $ancestor = Split-Path -Parent $ancestor
    }
}
if ((Test-Path -LiteralPath $proof) -or $ProductCode -notmatch '^\{[0-9A-Fa-f-]{36}\}$') { throw 'New proof directory and actual MSI product code required.' }
$expectedShortcut = Join-Path ([Environment]::GetFolderPath('Programs')) 'MicCamWatch.lnk'
if (-not [string]::Equals($shortcut, $expectedShortcut, [StringComparison]::OrdinalIgnoreCase)) { throw 'Only the actual installed user Start Menu link may be launched.' }
$cli = Join-Path $install 'mcw.exe'
$gui = Join-Path $install 'mcw-tray.exe'
[void](New-Item -ItemType Directory -Path $proof)
$report = [ordered]@{
    status='running'; cleanupSafe=$false; productCode=$ProductCode; installation=$install; shortcut=$shortcut
    observerSid=[Security.Principal.WindowsIdentity]::GetCurrent().User.Value
    requireStandardUser=[bool]$RequireStandardUser; standardUserQualified=$false
    fixtureHostKind=$fixtureHost.kind; context=$fixtureHost.standardUserContext
    methods=@('actual installed MSI payload and Start Menu .lnk', 'retained process handles and native HWND/class/style observations', 'real Shell launches without CreateNoWindow for GUI', 'loopback proxy tripwire and staging filesystem events for actual MSI update refusal')
    launches=@(); observations=@(); cleanup=@(); error=$null
    limitations=@('No physical device-control or recording validation. Settings disable all protection, notification, sound and history actions. No desktop control/WM_COMMAND, autostart, device, UAC, clipboard or physical pointer action is requested.')
}
function Save-ApplicationReport { $report | ConvertTo-Json -Depth 16 | Set-Content -LiteralPath (Join-Path $proof 'result.json') -Encoding UTF8 }
Save-ApplicationReport
$previous = @{}
foreach ($name in @('APPDATA','LOCALAPPDATA','HOME','USERPROFILE','TEMP','TMP','HTTP_PROXY','HTTPS_PROXY','ALL_PROXY','NO_PROXY')) { $previous[$name] = [Environment]::GetEnvironmentVariable($name, 'Process') }
$processes = [Collections.Generic.List[object]]::new()
$owner = $null
$watcher = $null
$proxy = $null
$monitor = $null
$installer = $null
$failure = $null
try {
    Add-Type -AssemblyName System.Drawing
    Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
public sealed class McwAppWindow {
    public long Hwnd; public uint Pid; public string Class, Title; public bool Visible; public long ExStyle;
}
public static class McwAppNative {
    public delegate bool EnumProc(IntPtr hwnd, IntPtr parameter);
    [DllImport("user32.dll", SetLastError=true)] static extern bool EnumWindows(EnumProc callback, IntPtr parameter);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetClassNameW(IntPtr hwnd, StringBuilder text, int count);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetWindowTextW(IntPtr hwnd, StringBuilder text, int count);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr hwnd);
    [DllImport("user32.dll", EntryPoint="GetWindowLongPtrW")] static extern IntPtr GetWindowLongPtr(IntPtr hwnd, int index);
    [DllImport("user32.dll", SetLastError=true)] public static extern IntPtr SendMessageTimeoutW(IntPtr hwnd, uint message, UIntPtr wparam, IntPtr lparam, uint flags, uint timeout, out UIntPtr result);
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)] static extern bool QueryFullProcessImageNameW(IntPtr process, uint flags, StringBuilder path, ref uint size);
    [DllImport("kernel32.dll")] public static extern uint WaitForSingleObject(IntPtr process, uint timeout);
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode)] static extern int GetApplicationUserModelId(IntPtr process, ref uint length, StringBuilder appid);
    [StructLayout(LayoutKind.Sequential)] struct RECT { public int left, top, right, bottom; }
    [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr hwnd, out RECT rect);
    [DllImport("user32.dll")] static extern bool PrintWindow(IntPtr hwnd, IntPtr dc, uint flags);
    public static string Class(IntPtr hwnd) { var text=new StringBuilder(256); GetClassNameW(hwnd,text,text.Capacity); return text.ToString(); }
    public static McwAppWindow[] Windows() {
        var result=new List<McwAppWindow>();
        if (!EnumWindows((hwnd, ignored) => {
            uint pid; GetWindowThreadProcessId(hwnd,out pid);
            var title=new StringBuilder(1024); GetWindowTextW(hwnd,title,title.Capacity);
            result.Add(new McwAppWindow { Hwnd=hwnd.ToInt64(), Pid=pid, Class=Class(hwnd), Title=title.ToString(), Visible=IsWindowVisible(hwnd), ExStyle=GetWindowLongPtr(hwnd,-20).ToInt64() });
            return true;
        },IntPtr.Zero)) throw new Win32Exception();
        return result.ToArray();
    }
    public static string Image(IntPtr handle) {
        var text=new StringBuilder(32768); uint length=(uint)text.Capacity;
        if(!QueryFullProcessImageNameW(handle,0,text,ref length)) throw new Win32Exception();
        return System.IO.Path.GetFullPath(text.ToString());
    }
    public static string[] AppId(IntPtr handle) {
        try {
            uint length=0; int first=GetApplicationUserModelId(handle,ref length,null);
            // This package identity API does NOT expose the explicit desktop
            // identity set by SetCurrentProcessExplicitAppUserModelID.
            // Record its result only; assert the real Shell shortcut property.
            if(first==15703) return new [] { "unavailable-unpackaged", "15703" };
            if(first!=122) return new [] { "package-query-unavailable", first.ToString() };
            var text=new StringBuilder((int)length);
            int status=GetApplicationUserModelId(handle,ref length,text);
            if(status!=0) return new [] { "package-query-unavailable", status.ToString() };
            return new [] { "available", text.ToString() };
        } catch(EntryPointNotFoundException) { return new [] { "api-unavailable", "" }; }
    }
    public static void Capture(IntPtr hwnd,string path) {
        RECT rect; if(!GetWindowRect(hwnd,out rect)) throw new Win32Exception();
        using(var image=new Bitmap(rect.right-rect.left,rect.bottom-rect.top))
        using(var graphics=Graphics.FromImage(image)) {
            var dc=graphics.GetHdc(); bool ok;
            try { ok=PrintWindow(hwnd,dc,2); } finally { graphics.ReleaseHdc(dc); }
            if(!ok) throw new Win32Exception();
            image.Save(path,ImageFormat.Png);
        }
    }
}
// A real native event observer catches transient GUI-launch console windows;
// unlike polling alone it also records a console that closes during startup.
public sealed class McwAppConsoleMonitor : IDisposable {
    delegate void EventProc(IntPtr hook,uint evt,IntPtr hwnd,int obj,int child,uint thread,uint time);
    [StructLayout(LayoutKind.Sequential)] struct POINT { public int x,y; }
    [StructLayout(LayoutKind.Sequential)] struct MSG { public IntPtr hwnd; public uint msg; public UIntPtr wp; public IntPtr lp; public uint time; public POINT point; public uint privateValue; }
    [DllImport("user32.dll")] static extern IntPtr SetWinEventHook(uint min,uint max,IntPtr module,EventProc callback,uint pid,uint thread,uint flags);
    [DllImport("user32.dll")] static extern bool UnhookWinEvent(IntPtr hook);
    [DllImport("user32.dll")] static extern int GetMessageW(out MSG message,IntPtr hwnd,uint min,uint max);
    [DllImport("user32.dll")] static extern bool PostThreadMessageW(uint thread,uint message,UIntPtr wp,IntPtr lp);
    [DllImport("kernel32.dll")] static extern uint GetCurrentThreadId();
    readonly object gate=new object(); readonly List<string> events=new List<string>();
    readonly ManualResetEvent ready=new ManualResetEvent(false); readonly Thread worker;
    EventProc callback; uint id; string error;
    public McwAppConsoleMonitor() {
        worker=new Thread(Run); worker.IsBackground=true; worker.Start();
        if(!ready.WaitOne(5000) || error!=null) throw new InvalidOperationException(error ?? "Console observer startup timed out");
    }
    void Run() {
        id=GetCurrentThreadId();
        callback=(hook,evt,hwnd,obj,child,thread,time)=> {
            if(hwnd!=IntPtr.Zero && obj==0 && McwAppNative.Class(hwnd)=="ConsoleWindowClass") {
                uint pid; McwAppNative.GetWindowThreadProcessId(hwnd,out pid);
                lock(gate) events.Add("event="+evt+" hwnd="+hwnd+" pid="+pid+" tick="+time);
            }
        };
        var handle=SetWinEventHook(0x8000,0x8002,IntPtr.Zero,callback,0,0,0);
        if(handle==IntPtr.Zero) { error="SetWinEventHook failed"; ready.Set(); return; }
        // Prime the thread message queue before another thread can stop it.
        ready.Set(); MSG msg;
        try { while(GetMessageW(out msg,IntPtr.Zero,0,0)>0) {} }
        finally { UnhookWinEvent(handle); }
    }
    public string[] Events { get { lock(gate) return events.ToArray(); } }
    public void Dispose() { PostThreadMessageW(id,0x12,UIntPtr.Zero,IntPtr.Zero); worker.Join(5000); ready.Dispose(); }
}
public sealed class McwAppRun : IDisposable {
    public readonly Process Process; readonly object gate=new object();
    readonly StringBuilder stdout=new StringBuilder(),stderr=new StringBuilder();
    public McwAppRun(string image,string arguments,bool gui) {
        Process=new Process(); Process.StartInfo=new ProcessStartInfo(image,arguments) { UseShellExecute=gui };
        if(!gui) {
            Process.StartInfo.CreateNoWindow=true;
            Process.StartInfo.RedirectStandardOutput=true; Process.StartInfo.RedirectStandardError=true;
            Process.OutputDataReceived+=(s,e)=> { if(e.Data!=null) lock(gate) stdout.AppendLine(e.Data); };
            Process.ErrorDataReceived+=(s,e)=> { if(e.Data!=null) lock(gate) stderr.AppendLine(e.Data); };
        }
        if(!Process.Start()) throw new InvalidOperationException("Launch failed");
        var handle=Process.Handle;
        if(!gui) { Process.BeginOutputReadLine(); Process.BeginErrorReadLine(); }
    }
    public string Stdout { get { lock(gate) return stdout.ToString(); } }
    public string Stderr { get { lock(gate) return stderr.ToString(); } }
    public void Dispose() { Process.Dispose(); }
}
'@ -ReferencedAssemblies System.Drawing
    # Native callbacks avoid PowerShell handlers on threads without a runspace.
    Add-Type -TypeDefinition @'
using System;
using System.Collections.Concurrent;
using System.IO;
public sealed class McwAppStagingObserver : IDisposable {
    public readonly ConcurrentQueue<string> Events=new ConcurrentQueue<string>(); readonly FileSystemWatcher watcher;
    public McwAppStagingObserver(FileSystemWatcher source) {
        watcher=source;
        watcher.Created+=(s,e)=> { if(e.FullPath.IndexOf(".mcw-update-",StringComparison.OrdinalIgnoreCase)>=0) Events.Enqueue("created:"+e.FullPath); };
        watcher.Renamed+=(s,e)=> { if(e.FullPath.IndexOf(".mcw-update-",StringComparison.OrdinalIgnoreCase)>=0) Events.Enqueue("renamed:"+e.FullPath); };
        watcher.Error+=(s,e)=>Events.Enqueue("observer-error:"+e.GetException());
        watcher.EnableRaisingEvents=true;
    }
    public void Dispose() { watcher.EnableRaisingEvents=false; }
}
'@
    if ($RequireStandardUser) {
        # Admission observed the genuine account before private app-data isolation.
        $report.standardUserQualified=$true
        Save-ApplicationReport
    }
    # Refuse even a different-install/user MCW; never take over or stop it.
    foreach ($name in @('mcw','mcw-tray')) {
        foreach ($process in [Diagnostics.Process]::GetProcessesByName($name)) {
            try { throw "Existing $name process PID $($process.Id); no application launch allowed." }
            finally { $process.Dispose() }
        }
    }
    if (@([McwAppNative]::Windows() | Where-Object { $_.Class -in @('MicCamWatchTrayClass','MicCamWatchDesktopClass') }).Count -ne 0) { throw 'Existing MCW native window; refusing launch.' }
    $installer = New-Object -ComObject WindowsInstaller.Installer
    function Get-Registration {
        $related=$installer.RelatedProducts('{8FC9317B-4517-4E83-9F52-CF3257584BF6}')
        try { $count=$related.GetType().InvokeMember('Count',[Reflection.BindingFlags]::GetProperty,$null,$related,$null) }
        finally { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($related) }
        if ($count -isnot [int] -or $count -ne 1 -or $installer.ProductState($ProductCode) -ne 5) { throw 'Exactly one owned MSI product must be installed.' }
        $location=[string]$installer.ProductInfo($ProductCode,'InstallLocation')
        if (-not [string]::Equals($location.TrimEnd('\'),$install,[StringComparison]::OrdinalIgnoreCase) -or [string]$installer.ProductInfo($ProductCode,'AssignmentType') -cne '0') { throw 'MSI installation identity/scope mismatch.' }
        return [ordered]@{ productCode=$ProductCode; name=$installer.ProductInfo($ProductCode,'ProductName'); version=$installer.ProductInfo($ProductCode,'VersionString'); location=$location; cachedMsiSha256=(Get-FileHash -LiteralPath $installer.ProductInfo($ProductCode,'LocalPackage')).Hash }
    }
    $report['registrationBefore']=Get-Registration
    if ($report.registrationBefore.name -cne 'MicCamWatch') { throw 'Unexpected installed product name.' }
    $shell=New-Object -ComObject WScript.Shell
    try {
        $links=@()
        foreach ($root in @([Environment]::GetFolderPath('Programs'),[Environment]::GetFolderPath('CommonPrograms'))) {
            foreach ($file in Get-ChildItem -LiteralPath $root -Filter '*.lnk' -Recurse -File) {
                $link=$shell.CreateShortcut($file.FullName)
                try {
                    if ($file.BaseName -like '*MicCamWatch*' -or [string]::Equals($link.TargetPath,$gui,[StringComparison]::OrdinalIgnoreCase)) { $links+=@{ path=$file.FullName; target=$link.TargetPath; arguments=$link.Arguments } }
                } finally { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($link) }
            }
        }
        if ($links.Count -ne 1 -or -not [string]::Equals($links[0].path,$shortcut,[StringComparison]::OrdinalIgnoreCase) -or -not [string]::Equals($links[0].target,$gui,[StringComparison]::OrdinalIgnoreCase)) { throw 'Expected exactly one visible installed MicCamWatch Start Menu application link.' }
        $report['visibleLinks']=$links
    } finally { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($shell) }
    $shellProperties=New-Object -ComObject Shell.Application
    $folder=$null; $item=$null
    try {
        $folder=$shellProperties.Namespace((Split-Path -Parent $shortcut))
        if ($null -eq $folder) { throw 'Shell cannot resolve the real installed shortcut folder.' }
        $item=$folder.ParseName([IO.Path]::GetFileName($shortcut))
        if ($null -eq $item) { throw 'Shell cannot resolve the actual installed shortcut item.' }
        $shortcutAppId=[string]$item.ExtendedProperty('System.AppUserModel.ID')
        $report['shortcutAppUserModelId']=$shortcutAppId
        if ($shortcutAppId -cne 'MicCamWatch.MicCamWatch') { throw 'Actual installed Shell shortcut AppUserModelID mismatch.' }
    } finally {
        foreach ($com in @($item,$folder,$shellProperties)) { if ($null -ne $com) { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($com) } }
    }
    $report.limitations+='GetApplicationUserModelId is package identity, not a remote getter for explicit Win32 desktop AUMID. Its raw result is informational; actual .lnk Shell property is asserted. Actual taskbar grouping still requires visual review of the hosted capture/session.'
    function Get-AutostartSnapshot {
        $items=[ordered]@{}
        foreach ($hive in @('HKCU:','HKLM:')) {
            foreach ($key in @('Software\Microsoft\Windows\CurrentVersion\Run','Software\Microsoft\Windows\CurrentVersion\RunOnce')) {
                $path="$hive\$key"
                if (Test-Path -LiteralPath $path) {
                    $value=Get-ItemProperty -LiteralPath $path
                    foreach ($p in $value.PSObject.Properties | Where-Object { $_.Name -notlike 'PS*' }) { $items["$path/$($p.Name)"]=[string]$p.Value }
                }
            }
        }
        foreach ($task in @(Get-ScheduledTask | Where-Object { $_.TaskName -like '*MicCamWatch*' -or @($_.Actions | Where-Object { $_.CimClass.CimClassName -eq 'MSFT_TaskExecAction' -and $_.Execute -match 'mcw(-tray)?\.exe' }).Count -ne 0 })) { $items["task:$($task.TaskPath)$($task.TaskName)"]=Export-ScheduledTask -TaskName $task.TaskName -TaskPath $task.TaskPath }
        foreach ($root in @([Environment]::GetFolderPath('Startup'),[Environment]::GetFolderPath('CommonStartup'))) {
            foreach ($file in Get-ChildItem -LiteralPath $root -File -Recurse) { $items["startup:$($file.FullName)"]=(Get-FileHash -LiteralPath $file.FullName).Hash }
        }
        return ($items | ConvertTo-Json -Depth 8 -Compress)
    }
    $autostartBefore=Get-AutostartSnapshot
    $report['autostartBefore']=$autostartBefore
    if ($autostartBefore -match 'MicCamWatch|mcw(-tray)?\.exe') { throw 'Existing MCW autostart registration; refusing application launch.' }
    $scope=Join-Path $proof 'private-user'
    $env:APPDATA=Join-Path $scope 'roaming'; $env:LOCALAPPDATA=Join-Path $scope 'local'
    $env:HOME=Join-Path $scope 'home'; $env:USERPROFILE=$env:HOME
    $env:TEMP=Join-Path $scope 'temp'; $env:TMP=$env:TEMP
    foreach ($directory in @((Join-Path $env:APPDATA 'MicCamWatch'),$env:LOCALAPPDATA,$env:HOME,$env:TEMP)) { [void](New-Item -ItemType Directory -Path $directory -Force) }
    $settings=[IO.Path]::GetFullPath((Join-Path $env:APPDATA 'MicCamWatch/settings.toml'))
    [IO.File]::WriteAllText($settings,"notifications_enabled=false`nsound_enabled=false`nshow_ready=false`nhistory_enabled=false`nmute_on_lock=false`nblock_camera_on_lock=false`nrestore_on_unlock=false`n")
    $policy=[IO.Path]::GetFullPath((Join-Path $env:APPDATA 'MicCamWatch/policy.toml'))
    [IO.File]::WriteAllText($policy,"profile='conservative'`ntrust_policy='offline'`naction='alert'`nlanguage='en'`n")
    $settingsHash=(Get-FileHash -LiteralPath $settings).Hash; $policyHash=(Get-FileHash -LiteralPath $policy).Hash
    function Get-FileSnapshot([string]$Root) {
        return @(@(Get-ChildItem -LiteralPath $Root -File -Recurse -Force | Sort-Object FullName) | ForEach-Object { [ordered]@{ path=$_.FullName.Substring($Root.Length); length=$_.Length; sha256=(Get-FileHash -LiteralPath $_.FullName).Hash } })
    }
    function Start-App([string]$Image,[string]$Arguments,[bool]$IsGui) {
        $run=[McwAppRun]::new($Image,$Arguments,$IsGui)
        $processes.Add($run)
        $report.launches+=@{ image=$Image; arguments=$Arguments; pid=$run.Process.Id; handle=$run.Process.Handle.ToInt64(); shellExecute=$IsGui; createNoWindow=(-not $IsGui); at=[DateTime]::UtcNow.ToString('o') }
        Save-ApplicationReport
        return $run
    }
    function Get-AppWindows { return @([McwAppNative]::Windows() | Where-Object { $_.Class -in @('MicCamWatchTrayClass','MicCamWatchDesktopClass') }) }
    function Retain-Owner {
        $windows=@(Get-AppWindows | Where-Object { $_.Class -eq 'MicCamWatchTrayClass' })
        if ($windows.Count -ne 1) { return $null }
        $process=[Diagnostics.Process]::GetProcessById($windows[0].Pid)
        try {
            $handle=$process.Handle
            if (-not [string]::Equals([McwAppNative]::Image($handle),$gui,[StringComparison]::OrdinalIgnoreCase)) { throw 'Resident tray is not the actual installed image.' }
            return [pscustomobject]@{ Process=$process; Handle=$handle; Pid=$process.Id; Tray=$windows[0].Hwnd; Desktop=0L }
        } catch { $process.Dispose(); throw }
    }
    function Assert-Owner([string]$Phase,[int]$Visible) {
        if ([McwAppNative]::WaitForSingleObject($owner.Handle,0) -ne 258 -or -not [string]::Equals([McwAppNative]::Image($owner.Handle),$gui,[StringComparison]::OrdinalIgnoreCase)) { throw "Exact owner exited/changed during $Phase" }
        $windows=@(Get-AppWindows)
        $trays=@($windows | Where-Object { $_.Class -eq 'MicCamWatchTrayClass' })
        $desktops=@($windows | Where-Object { $_.Class -eq 'MicCamWatchDesktopClass' })
        if ($trays.Count -ne 1 -or $trays[0].Hwnd -ne $owner.Tray -or $trays[0].Pid -ne $owner.Pid) { throw "Duplicate/replaced tray owner during $Phase" }
        if ($desktops.Count -gt 1 -or ($owner.Desktop -ne 0 -and ($desktops.Count -ne 1 -or $desktops[0].Hwnd -ne $owner.Desktop))) { throw "Duplicate/replaced retained desktop HWND during $Phase" }
        if ($desktops.Count -eq 1) {
            if ($desktops[0].Pid -ne $owner.Pid) { throw 'Desktop belongs to a different process.' }
            $owner.Desktop=$desktops[0].Hwnd
            if (($desktops[0].ExStyle -band 0x40000) -eq 0 -or ($desktops[0].ExStyle -band 0x80) -ne 0) { throw 'Main native window is not taskbar-eligible WS_EX_APPWINDOW (or is a tool window).' }
        }
        $shown=$desktops.Count -eq 1 -and $desktops[0].Visible
        if ($Visible -ge 0 -and $shown -ne ($Visible -eq 1)) { throw "Unexpected desktop visibility during $Phase" }
        $residents=@()
        foreach ($name in @('mcw','mcw-tray')) {
            foreach ($process in [Diagnostics.Process]::GetProcessesByName($name)) {
                try { if (-not $process.HasExited) { $residents+=@{ pid=$process.Id; image=[McwAppNative]::Image($process.Handle) } } }
                finally { $process.Dispose() }
            }
        }
        if ($residents.Count -ne 1 -or $residents[0].pid -ne $owner.Pid) { throw "Expected exactly one resident tray process during $Phase" }
        $id=[McwAppNative]::AppId($owner.Handle)
        $report.observations+=@{ phase=$Phase; ownerPid=$owner.Pid; processHandle=$owner.Handle.ToInt64(); trayHwnd=$owner.Tray; desktopHwnd=$owner.Desktop; visible=$shown; windows=$windows; residentProcesses=$residents; packageIdentityApi=@{ status=$id[0]; value=$id[1]; explicitDesktopAppIdGetter=$false } }
        Save-ApplicationReport
    }
    function Wait-Visible([bool]$Visible) {
        $deadline=[DateTime]::UtcNow.AddSeconds(15)
        do {
            $windows=@(Get-AppWindows | Where-Object { $_.Class -eq 'MicCamWatchDesktopClass' })
            if ($windows.Count -gt 1) { throw 'Multiple native desktop windows.' }
            if ($windows.Count -eq 1 -and $windows[0].Visible -eq $Visible) { return }
            Start-Sleep -Milliseconds 25
        } while ([DateTime]::UtcNow -lt $deadline)
        throw "Desktop visibility did not become $Visible"
    }
    function Wait-Launchers {
        foreach ($run in $processes) {
            if ($run.Process.Id -eq $owner.Pid) { continue }
            if (-not $run.Process.WaitForExit(15000)) { throw "Duplicate launcher did not exit: $($run.Process.Id)" }
            if ($run.Process.ExitCode -ne 0) { throw "Launcher failed: $($run.Process.Id): $($run.Stderr)" }
        }
    }
    function Hide-Desktop {
        Assert-Owner 'before-close' 1
        [UIntPtr]$reply=[UIntPtr]::Zero
        if ([McwAppNative]::SendMessageTimeoutW([IntPtr]$owner.Desktop,0x10,[UIntPtr]::Zero,[IntPtr]::Zero,3,5000,[ref]$reply) -eq [IntPtr]::Zero) { throw 'Exact desktop WM_CLOSE timed out.' }
        Wait-Visible $false
        Assert-Owner 'close-hides-retains-HWND' 0
    }
    $monitor=[McwAppConsoleMonitor]::new()
    # GUI subprocess is deliberately not created with CreateNoWindow; this is
    # the actual console-free executable/Windows Shell behavior under test.
    $null=Start-App $gui '' $true
    # Race cold GUI background starts before waiting for the first native HWND.
    # CLI joins the repeated/concurrent mixed-entrypoint races below.
    foreach ($iteration in 1..4) { $null=Start-App $gui '' $true }
    $deadline=[DateTime]::UtcNow.AddSeconds(20)
    do { $owner=Retain-Owner; if ($null -ne $owner) { break }; Start-Sleep -Milliseconds 25 } while ([DateTime]::UtcNow -lt $deadline)
    if ($null -eq $owner -or @($processes | Where-Object { $_.Process.Id -eq $owner.Pid }).Count -ne 1) { throw 'Cold concurrent installed GUI launches did not produce one owned resident tray.' }
    Wait-Launchers
    Start-Sleep -Milliseconds 500
    Assert-Owner 'initial-background-hidden' 0
    # Launch the original installed .lnk, not a copied or synthetic shortcut.
    # Shell resolution is real; its returned process is not ownership authority.
    Start-Process -FilePath $shortcut
    $report.launches+=@{ image=$shortcut; arguments=$null; shellExecute=$true; actualInstalledShortcut=$true; at=[DateTime]::UtcNow.ToString('o') }
    Wait-Visible $true
    Assert-Owner 'actual-StartMenu-link-opens' 1
    [McwAppNative]::Capture([IntPtr]$owner.Desktop,(Join-Path $proof 'installed-start-menu-window.png'))
    Hide-Desktop
    foreach ($phase in @('sequential','concurrent')) {
        if ($phase -eq 'sequential') {
            foreach ($entry in @(@($gui,'--open',$true),@($cli,'tray open',$false),@($gui,'--open',$true),@($cli,'tray open',$false))) {
                $null=Start-App $entry[0] $entry[1] $entry[2]
                Wait-Visible $true; Wait-Launchers; Assert-Owner "repeated-$phase-open" 1; Hide-Desktop
            }
        } else {
            foreach ($iteration in 1..4) {
                $null=Start-App $gui '--open' $true
                $null=Start-App $cli 'tray open' $false
                Start-Process -FilePath $shortcut
                $report.launches+=@{ image=$shortcut; shellExecute=$true; actualInstalledShortcut=$true; phase='concurrent'; iteration=$iteration }
            }
            Wait-Visible $true; Wait-Launchers; Assert-Owner 'concurrent-GUI-CLI-shortcut-one-owner-one-HWND' 1
            Hide-Desktop
        }
        foreach ($iteration in 1..3) { $null=Start-App $gui '' $true; $null=Start-App $cli 'tray run' $false }
        Wait-Launchers
        # Duplicate background runs must not raise an already-retained window.
        Start-Sleep -Milliseconds 500
        Assert-Owner "$phase-background-launch-stays-hidden" 0
    }
    $null=Start-App $cli 'tray open' $false
    Wait-Visible $true; Wait-Launchers; Assert-Owner 'explicit-CLI-open-after-background' 1
    [McwAppNative]::Capture([IntPtr]$owner.Desktop,(Join-Path $proof 'retained-reopened-window.png'))
    # Execute the real installed CLI updater while the retained owner is alive.
    # A loopback proxy is a no-response tripwire, not a release-server mock.
    # Any attempted HTTP(S) fetch fails this scenario before an external request.
    $payloadBefore=Get-FileSnapshot $install
    $dataBefore=Get-FileSnapshot $scope
    $shortcutHashBefore=(Get-FileHash -LiteralPath $shortcut).Hash
    $userPathBefore=[Environment]::GetEnvironmentVariable('PATH','User')
    $watcher=[IO.FileSystemWatcher]::new($install)
    $watcher.IncludeSubdirectories=$true; $watcher.InternalBufferSize=65536
    $stagingObserver=[McwAppStagingObserver]::new($watcher)
    $proxy=[Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback,0); $proxy.Start()
    $proxyAddress='http://127.0.0.1:'+$proxy.LocalEndpoint.Port
    $env:HTTP_PROXY=$proxyAddress; $env:HTTPS_PROXY=$proxyAddress; $env:ALL_PROXY=$proxyAddress; $env:NO_PROXY=''
    $update=[McwAppRun]::new($cli,'update',$false); $processes.Add($update)
    $report.launches+=@{ image=$cli; arguments='update'; pid=$update.Process.Id; handle=$update.Process.Handle.ToInt64(); networkTripwire=$proxyAddress }
    $clock=[Diagnostics.Stopwatch]::StartNew(); $connections=0
    try {
        while (-not $update.Process.HasExited -and $clock.ElapsedMilliseconds -lt 10000) {
            while ($proxy.Pending()) { $connection=$proxy.AcceptTcpClient(); $connections++; $connection.Dispose() }
            Start-Sleep -Milliseconds 10
        }
        while ($proxy.Pending()) { $connection=$proxy.AcceptTcpClient(); $connections++; $connection.Dispose() }
        if (-not $update.Process.HasExited) { throw 'MSI update did not refuse promptly; process retained, no forced stop.' }
        $update.Process.WaitForExit()
        Start-Sleep -Milliseconds 100
        $events=@($stagingObserver.Events.ToArray())
        $text=$update.Stdout+"`n"+$update.Stderr
        [IO.File]::WriteAllText((Join-Path $proof 'msi-update.stdout.txt'),$update.Stdout)
        [IO.File]::WriteAllText((Join-Path $proof 'msi-update.stderr.txt'),$update.Stderr)
        $report['msiUpdate']=@{ exitCode=$update.Process.ExitCode; elapsedMs=$clock.ElapsedMilliseconds; proxyConnections=$connections; stagingEvents=$events; stdout=$update.Stdout; stderr=$update.Stderr; payloadBefore=$payloadBefore; payloadAfter=(Get-FileSnapshot $install); dataBefore=$dataBefore; dataAfter=(Get-FileSnapshot $scope); registrationAfter=(Get-Registration) }
        if ($update.Process.ExitCode -eq 0 -or -not $text.Contains($ProductCode) -or -not $text.Contains('https://github.com/Roman-Cuisset/miccamwatch/releases/latest')) { throw 'Actual installed MSI updater must refuse with its registered product identity and actionable official MSI release link.' }
        if ($connections -ne 0 -or $events.Count -ne 0 -or @(Get-ChildItem -LiteralPath $install -Filter '.mcw-update-*' -Force).Count -ne 0) { throw 'MSI updater attempted networking or staging before refusing.' }
        if (($payloadBefore | ConvertTo-Json -Depth 6 -Compress) -cne ($report.msiUpdate.payloadAfter | ConvertTo-Json -Depth 6 -Compress) -or ($dataBefore | ConvertTo-Json -Depth 6 -Compress) -cne ($report.msiUpdate.dataAfter | ConvertTo-Json -Depth 6 -Compress) -or ($report.registrationBefore | ConvertTo-Json -Compress) -cne ($report.msiUpdate.registrationAfter | ConvertTo-Json -Compress)) { throw 'MSI update refusal changed installed payload, MSI version/metadata, or private user data.' }
        $report.msiUpdate['shortcutSha256Before']=$shortcutHashBefore
        $report.msiUpdate['shortcutSha256After']=(Get-FileHash -LiteralPath $shortcut).Hash
        $report.msiUpdate['userPathBefore']=$userPathBefore
        $report.msiUpdate['userPathAfter']=[Environment]::GetEnvironmentVariable('PATH','User')
        if ($report.msiUpdate.shortcutSha256After -cne $shortcutHashBefore -or $report.msiUpdate.userPathAfter -cne $userPathBefore) { throw 'MSI update refusal changed actual Start Menu shortcut or user PATH.' }
        Assert-Owner 'MSI-update-refusal-preserves-exact-owner-and-HWND' 1
    } finally {
        $stagingObserver.Dispose(); $proxy.Stop(); $proxy=$null
        foreach ($name in @('HTTP_PROXY','HTTPS_PROXY','ALL_PROXY','NO_PROXY')) { [Environment]::SetEnvironmentVariable($name,$previous[$name],'Process') }
    }
    $report['consoleWindowEvents']=$monitor.Events
    if ($monitor.Events.Count -ne 0) { throw 'GUI/Shell launch created or showed a native console window.' }
    if ((Get-FileHash -LiteralPath $settings).Hash -cne $settingsHash -or (Get-FileHash -LiteralPath $policy).Hash -cne $policyHash) { throw 'Application launch changed isolated disabled preferences/policy.' }
    $report['autostartAfter']=Get-AutostartSnapshot
    if ($report.autostartAfter -cne $autostartBefore) { throw 'Application launch requested or changed OS autostart registration.' }
    # Read-only observation currently creates no disk locks: microphone resume
    # uses the native request mutex and File::open/load_intent (NotFound =>
    # default); camera read_journal uses protected_path(identity, false).
    # current_identity/scope_hash canonicalizes existing ancestry without mkdir.
    # Keep the persistence assertion strict, but never label an unexpected lock
    # file as evidence of a device-control request or physical mutation.
    $unexpected=@(Get-ChildItem -LiteralPath $scope -File -Recurse -Force | Where-Object { $_.FullName -ne $settings -and $_.FullName -ne $policy })
    $report['privateUserFiles']=Get-FileSnapshot $scope
    $unexpectedLocks=@($unexpected | Where-Object { $_.Extension -eq '.lock' } | ForEach-Object { $_.FullName })
    $unexpectedState=@($unexpected | Where-Object { $_.Extension -ne '.lock' } | ForEach-Object { $_.FullName })
    $report['unexpectedPersistence']=@{ lockFiles=$unexpectedLocks; otherFiles=$unexpectedState; diskLocksExpectedByCurrentReadOnlyPath=$false }
    if ($unexpected.Count -ne 0) { throw "Read-only launch unexpectedly persisted app data; locks are not device-action evidence. Locks: $($unexpectedLocks -join ', '); other files: $($unexpectedState -join ', ')" }
    $report['actionRequests']=@{ deviceControl=$false; autostart=$false; notifications=$false; history=$false; basis='test-driver launch allowlist: tray run, tray open, GUI background/--open, actual installed shortcut, MSI update refusal; only WM_CLOSE sent; disabled settings/policy hashes unchanged; no persisted protection requests/history; OS autostart snapshot unchanged'; physicalMutationClaimed=$false }
    $report.status='passed'
} catch {
    $failure=$_; $report.status='failed'; $report.error=$_.ToString()+"`n"+$_.ScriptStackTrace
} finally {
    # Cleanup authority is an exact retained process handle + installed image +
    # tray HWND/PID. Never kill by name or terminate an unresponsive process.
    try {
        if ($null -ne $owner -and [McwAppNative]::WaitForSingleObject($owner.Handle,0) -eq 258) {
            [uint32]$currentPid=0
            [void][McwAppNative]::GetWindowThreadProcessId([IntPtr]$owner.Tray,[ref]$currentPid)
            if ($currentPid -ne $owner.Pid -or -not [string]::Equals([McwAppNative]::Image($owner.Handle),$gui,[StringComparison]::OrdinalIgnoreCase)) { throw 'Cooperative cleanup ownership mismatch; no message sent.' }
            [UIntPtr]$reply=[UIntPtr]::Zero
            if ([McwAppNative]::SendMessageTimeoutW([IntPtr]$owner.Tray,0x10,[UIntPtr]::Zero,[IntPtr]::Zero,3,5000,[ref]$reply) -eq [IntPtr]::Zero -or [McwAppNative]::WaitForSingleObject($owner.Handle,15000) -ne 0) { throw 'Owned tray did not cooperatively exit; retained for inspection, no forced stop.' }
            $report.cleanup+=@{ pid=$owner.Pid; handle=$owner.Handle.ToInt64(); trayHwnd=$owner.Tray; cooperativeExit=$true }
        }
        foreach ($run in $processes) {
            if (-not $run.Process.WaitForExit(5000)) { throw "Exact launched PID $($run.Process.Id) remains alive; no forced stop." }
        }
        # Also catch Shell-launched descendants not returned by ShellExecute.
        foreach ($name in @('mcw','mcw-tray')) {
            foreach ($process in [Diagnostics.Process]::GetProcessesByName($name)) {
                try { if (-not $process.HasExited -and [McwAppNative]::Image($process.Handle).StartsWith($install+'\',[StringComparison]::OrdinalIgnoreCase)) { throw "Installed application PID $($process.Id) remains alive; preserve MSI." } }
                finally { $process.Dispose() }
            }
        }
        $report.cleanupSafe=$true
    } catch {
        $report.cleanupSafe=$false; $report.status='failed'; $report.cleanup+=@{ error=$_.ToString(); forcedTermination=$false }
        if ($null -eq $failure) { $failure=$_ }
    }
    if ($null -ne $monitor) { $report['consoleWindowEvents']=$monitor.Events; $monitor.Dispose() }
    if ($null -ne $proxy) { $proxy.Stop() }
    if ($null -ne $watcher) { $watcher.Dispose() }
    foreach ($run in $processes) { $run.Dispose() }
    if ($null -ne $owner) { $owner.Process.Dispose() }
    if ($null -ne $installer) { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($installer) }
    foreach ($name in $previous.Keys) { [Environment]::SetEnvironmentVariable($name,$previous[$name],'Process') }
    Save-ApplicationReport
}
if ($null -ne $failure) { throw $failure }
Write-Output "Installed Windows application integration passed; retained evidence: $proof"
