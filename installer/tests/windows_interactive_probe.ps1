# Disposable hosted experiment: real fresh-user RDP desktop, never a token substitute.
# Strict server authentication remains enabled; only the exact local RDP leaf is
# provisioned into this disposable VM's machine Root for CredSSP validation.
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$StageDirectory,
    [Parameter(Mandatory = $true)][string]$FixtureScript,
    [Parameter(Mandatory = $true)][string]$PythonExe,
    [Parameter(Mandatory = $true)][string]$ReportPath,
    [ValidateRange(60, 86400)][int]$TimeoutSeconds = 900
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted') {
    throw 'Interactive provider is restricted to disposable GitHub-hosted Windows runners'
}
if ([Threading.Thread]::CurrentThread.ApartmentState -ne 'STA') { throw 'Run this provider with powershell.exe -STA' }
$stage = [IO.Path]::GetFullPath($StageDirectory)
$public = [IO.Path]::GetFullPath($env:PUBLIC).TrimEnd('\') + '\'
if (-not $stage.StartsWith($public, [StringComparison]::OrdinalIgnoreCase)) { throw 'StageDirectory must be below the disposable public directory' }
$null = New-Item -ItemType Directory -Force -Path $stage
$fixture = (Resolve-Path -LiteralPath $FixtureScript).Path
$python = (Resolve-Path -LiteralPath $PythonExe).Path
$reportFile = [IO.Path]::GetFullPath($ReportPath)
$null = New-Item -ItemType Directory -Force -Path ([IO.Path]::GetDirectoryName($reportFile))
$report = [ordered]@{ status = 'starting'; no_process_was_killed = $true; authentication_level = 1; trust_scope = 'Disposable hosted VM LocalMachine Root exact existing local RDP leaf only' }
$rootStore = $null
$addedLeaf = $false
$leaf = $null
$rdp = $null
$form = $null
$task = $null
$account = $null
$code = 1
$clock = [Diagnostics.Stopwatch]::StartNew()
function Save-Report { [IO.File]::WriteAllText($reportFile, ($report | ConvertTo-Json -Depth 8), (New-Object Text.UTF8Encoding($false))) }
function Stage([string]$value) { $report.stage = $value; Save-Report; Write-Output "Interactive provider: $value" }
function Assert-Deadline { if ($clock.Elapsed.TotalSeconds -ge $TimeoutSeconds) { throw 'Bounded interactive-provider deadline expired; no task/session was stopped' } }
function Quote-Argument([string]$value) {
    if ($value.Contains('"') -or $value.EndsWith('\')) { throw 'Unexpected quote/trailing slash in scheduled file argument' }
    return '"' + $value + '"'
}
try {
    Stage 'compile-native-bridge'
    Add-Type -AssemblyName System.Windows.Forms
    Add-Type -ReferencedAssemblies System.Windows.Forms -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Windows.Forms;
public sealed class McwRdpHost : AxHost {
    public McwRdpHost(string clsid) : base(clsid) {}
}
[ComImport, Guid("302D8188-0052-4807-806A-362B628F9AC5"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface McwExtendedSettings {
    void Put([MarshalAs(UnmanagedType.BStr)] string name, [In, MarshalAs(UnmanagedType.Struct)] ref object value);
    [return: MarshalAs(UnmanagedType.Struct)] object Get([MarshalAs(UnmanagedType.BStr)] string name);
}
public static class McwRdpNative {
    public static void SoftwareRendering(object control) {
        var settings=(McwExtendedSettings)control;
        object disabled=false;
        settings.Put("EnableFrameBufferRedirection",ref disabled);
        disabled=false;
        settings.Put("EnableHardwareMode",ref disabled);
        if((bool)settings.Get("EnableFrameBufferRedirection"))
            throw new InvalidOperationException("Native loopback framebuffer redirection remained enabled");
    }
    [StructLayout(LayoutKind.Sequential)] struct Session { public int Id; public IntPtr Station; public int State; }
    public sealed class Login { public int Id; public int State; public string User; public string Domain; public int Protocol; }
    [DllImport("wtsapi32.dll", SetLastError=true)] static extern bool WTSEnumerateSessionsW(IntPtr server, int reserved, int version, out IntPtr sessions, out int count);
    [DllImport("wtsapi32.dll", SetLastError=true)] static extern bool WTSQuerySessionInformationW(IntPtr server, int session, int kind, out IntPtr buffer, out int bytes);
    [DllImport("wtsapi32.dll")] static extern void WTSFreeMemory(IntPtr buffer);
    static string Text(int id, int kind) {
        IntPtr p; int bytes;
        if (!WTSQuerySessionInformationW(IntPtr.Zero, id, kind, out p, out bytes)) throw new Win32Exception(Marshal.GetLastWin32Error());
        try { return Marshal.PtrToStringUni(p) ?? ""; } finally { WTSFreeMemory(p); }
    }
    static int Protocol(int id) {
        IntPtr p; int bytes;
        if (!WTSQuerySessionInformationW(IntPtr.Zero, id, 16, out p, out bytes)) throw new Win32Exception(Marshal.GetLastWin32Error());
        try { if (bytes < 2) throw new InvalidOperationException("Incomplete WTS protocol"); return (ushort)Marshal.ReadInt16(p); } finally { WTSFreeMemory(p); }
    }
    public static Login[] Logins() {
        IntPtr p; int count;
        if (!WTSEnumerateSessionsW(IntPtr.Zero, 0, 1, out p, out count)) throw new Win32Exception(Marshal.GetLastWin32Error());
        try {
            var result = new List<Login>(); int size = Marshal.SizeOf(typeof(Session));
            for (int i = 0; i < count; i++) {
                var s = (Session)Marshal.PtrToStructure(IntPtr.Add(p, i * size), typeof(Session));
                string user = Text(s.Id, 5);
                if (user.Length != 0) result.Add(new Login { Id=s.Id, State=s.State, User=user, Domain=Text(s.Id, 7), Protocol=Protocol(s.Id) });
            }
            return result.ToArray();
        } finally { WTSFreeMemory(p); }
    }
    public static int DisconnectReason = -1;
    public static int FatalError = -1;
    public static int LogonError = -1;
    public static bool LoginComplete;
    public static void Observe(object control) {
        var events = new Guid("336d5562-efa8-482e-8cb3-c5c0fc7a7db6");
        ComEventsHelper.Combine(control, events, 3, new Action(() => LoginComplete = true));
        ComEventsHelper.Combine(control, events, 4, new Action<int>(value => DisconnectReason = value));
        ComEventsHelper.Combine(control, events, 10, new Action<int>(value => FatalError = value));
        ComEventsHelper.Combine(control, events, 22, new Action<int>(value => LogonError = value));
    }
}
'@
    # Read the actual configured listener and public local server certificate.
    Stage 'read-existing-listener'
    $listener = Get-CimInstance -Namespace root/cimv2/terminalservices -ClassName Win32_TSGeneralSetting -Filter "TerminalName='RDP-tcp'"
    if ($listener.UserAuthenticationRequired -ne 1) { throw 'Existing RDP listener does not require NLA; no policy will be changed' }
    $machine = [Environment]::MachineName
    $configuredHash = ([string]$listener.SSLCertificateSHA1Hash -replace '\s', '').ToUpperInvariant()
    Stage 'read-existing-server-leaf'
    $certificates = @(Get-ChildItem -LiteralPath 'Cert:\LocalMachine\Remote Desktop' | Where-Object {
        $_.Subject -eq "CN=$machine" -and $_.Issuer -eq $_.Subject -and
        $_.NotBefore -le (Get-Date) -and $_.NotAfter -gt (Get-Date)
    })
    if ($configuredHash.Length -ne 0) {
        $certificates = @($certificates | Where-Object { $_.Thumbprint -eq $configuredHash })
    } elseif ($listener.SSLCertificateSHA1HashType -ne 1) {
        throw 'Listener does not identify its default self-signed certificate'
    }
    if ($certificates.Count -ne 1) { throw 'Cannot uniquely bind the current machine RDP certificate to its existing listener' }
    # Public-only clone: never copy or export the server private key.
    $leaf = New-Object Security.Cryptography.X509Certificates.X509Certificate2 -ArgumentList @(,$certificates[0].RawData)
    $report.server_certificate_thumbprint = $leaf.Thumbprint
    $report.configured_certificate_hash = $configuredHash
    $report.server_hostname = $machine
    Stage 'open-disposable-machine-root'
    $rootStore = New-Object Security.Cryptography.X509Certificates.X509Store('Root', 'LocalMachine')
    $rootStore.Open([Security.Cryptography.X509Certificates.OpenFlags]::ReadWrite)
    $present = @($rootStore.Certificates | Where-Object { [Convert]::ToBase64String($_.RawData) -ceq [Convert]::ToBase64String($leaf.RawData) })
    Stage 'provision-exact-client-leaf'
    if ($present.Count -eq 0) { $rootStore.Add($leaf); $addedLeaf = $true }
    $report.exact_client_leaf_added = $addedLeaf
    $report.exact_client_leaf_present = @($rootStore.Certificates | Where-Object { [Convert]::ToBase64String($_.RawData) -ceq [Convert]::ToBase64String($leaf.RawData) }).Count -eq 1
    $chain = New-Object Security.Cryptography.X509Certificates.X509Chain
    try {
        $report.current_user_chain_trusted = $chain.Build($leaf)
        $report.current_user_chain_status = @($chain.ChainStatus | ForEach-Object { $_.Status.ToString() })
    } finally { $chain.Dispose() }
    Stage 'read-back-exact-client-trust'
    if (-not $report.exact_client_leaf_present -or -not $report.current_user_chain_trusted) { throw 'Exact disposable machine leaf enrollment did not establish native client chain trust' }
    Stage 'create-fresh-account'

    $name = 'mcw_' + [Guid]::NewGuid().ToString('N').Substring(0, 12)
    $random = New-Object byte[] 24
    $rng = [Security.Cryptography.RandomNumberGenerator]::Create()
    try { $rng.GetBytes($random) } finally { $rng.Dispose() }
    $password = 'Aa1!' + [Convert]::ToBase64String($random)
    [Array]::Clear($random, 0, $random.Length)
    $secure = ConvertTo-SecureString -String $password -AsPlainText -Force
    try { $account = New-LocalUser -Name $name -Password $secure -Description 'Disposable native interactive qualification' }
    finally { $secure.Dispose() }
    Stage 'assign-fresh-account-groups'
    foreach ($groupSid in @('S-1-5-32-545', 'S-1-5-32-555')) {
        $group = Get-LocalGroup -SID $groupSid
        if (@(Get-LocalGroupMember -Group $group | Where-Object { $_.SID -eq $account.SID }).Count -eq 0) {
            Add-LocalGroupMember -Group $group -Member $account
        }
    }
    Stage 'verify-fresh-account-memberships'
    $memberships = @(Get-LocalGroup | Where-Object {
        @(Get-LocalGroupMember -Group $_ | Where-Object { $_.SID -eq $account.SID }).Count -ne 0
    } | ForEach-Object { $_.SID.Value })
    if (@($memberships | Where-Object { $_ -notin @('S-1-5-32-545', 'S-1-5-32-555') }).Count -ne 0) { throw 'Fresh account has unexpected local group membership' }
    $report.fixture_account = $name
    $report.fixture_sid = $account.SID.Value
    $report.fixture_groups = $memberships
    Save-Report

    Stage 'create-native-rdp-client'
    # The registered v11 coclass is sufficient; no generated interop assemblies.
    $controlType = [Type]::GetTypeFromProgID('MsTscAx.MsTscAx.11', $true)
    $report.control_clsid = $controlType.GUID.ToString()
    if ($controlType.GUID -ne [Guid]'A0C63C30-F08D-4AB4-907C-34905D770C7D') { throw 'Unexpected registered RDP v11 class' }
    $form = New-Object Windows.Forms.Form
    $form.Text = 'Disposable MicCamWatch ordinary desktop'
    $form.Width = 1024; $form.Height = 768
    $hostControl = New-Object McwRdpHost($controlType.GUID.ToString())
    ([ComponentModel.ISupportInitialize]$hostControl).BeginInit()
    $hostControl.Dock = [Windows.Forms.DockStyle]::Fill
    $form.Controls.Add($hostControl)
    ([ComponentModel.ISupportInitialize]$hostControl).EndInit()
    $form.Show()
    [Windows.Forms.Application]::DoEvents()
    $rdp = $hostControl.GetOcx()
    [McwRdpNative]::Observe($rdp)
    [McwRdpNative]::SoftwareRendering($rdp)
    $report.rendering = 'Software decoding; native loopback framebuffer redirection disabled and read back'
    $report.control_version = $rdp.Version
    $rdp.Server = $machine
    $rdp.Domain = $machine
    $rdp.UserName = $name
    $rdp.DesktopWidth = 1024; $rdp.DesktopHeight = 768
    $report.initial_color_depth = $rdp.ColorDepth
    $rdp.ColorDepth = 32
    $report.configured_color_depth = $rdp.ColorDepth
    $rdp.AdvancedSettings5.AuthenticationLevel = 1
    $rdp.AdvancedSettings7.EnableCredSspSupport = $true
    $rdp.AdvancedSettings2.RedirectDrives = $false
    $rdp.AdvancedSettings2.RedirectPrinters = $false
    $rdp.AdvancedSettings2.RedirectPorts = $false
    $rdp.AdvancedSettings2.RedirectSmartCards = $false
    $rdp.AdvancedSettings6.RedirectClipboard = $false
    $rdp.AdvancedSettings8.AudioCaptureRedirectionMode = $false
    $rdp.SecuredSettings2.AudioRedirectionMode = 2
    $rdp.AdvancedSettings2.ClearTextPassword = $password
    $password = $null
    $report.effective_authentication_level = $rdp.AdvancedSettings5.AuthenticationLevel
    Stage 'connect-strict-native-rdp'
    $rdp.Connect()
    do {
        Assert-Deadline
        [Windows.Forms.Application]::DoEvents()
        if ([McwRdpNative]::DisconnectReason -ge 0 -or [McwRdpNative]::FatalError -ge 0) { throw 'Native RDP connection failed; inspect original control error evidence' }
        $sessions = @([McwRdpNative]::Logins() | Where-Object { $_.User -eq $name -and $_.Domain -eq $machine -and $_.State -eq 0 -and $_.Protocol -eq 2 })
        if ($sessions.Count -eq 1 -and [McwRdpNative]::LoginComplete) { break }
        Start-Sleep -Milliseconds 100
    } while ($true)
    $session = $sessions[0]
    if ($session.Id -eq 0) { throw 'RDP did not establish a real interactive session' }
    $report.interactive_session_id = $session.Id
    $report.authentication_type = [int]$rdp.AdvancedSettings7.AuthenticationType
    if ($report.authentication_type -notin @(1, 3)) { throw 'RDP did not report certificate-authenticated connection' }

    $sidBytes = New-Object byte[] $account.SID.BinaryLength
    $account.SID.GetBinaryForm($sidBytes, 0)
    $sidHex = -join ($sidBytes | ForEach-Object { $_.ToString('x2') })
    $bootstrap = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot 'windows_ordinary.py')).Path
    $bootstrapReport = Join-Path $stage ("private-interactive-$name-result.json")
    $configuration = Join-Path $stage ("private-interactive-$name-config.json")
    $workflowEnvironment = @{}
    foreach ($key in @('GITHUB_ACTIONS', 'RUNNER_ENVIRONMENT', 'GITHUB_WORKSPACE', 'PATH')) {
        $value = [Environment]::GetEnvironmentVariable($key)
        if ($null -ne $value) { $workflowEnvironment[$key] = $value }
    }
    $bootstrapConfiguration = @{ script=$fixture; working_directory=$stage; timeout=[Math]::Max(1, $TimeoutSeconds - [int]$clock.Elapsed.TotalSeconds - 10);
       report=$bootstrapReport; expected_account_sid=$sidHex; expected_session_id=$session.Id;
       existing_interactive_shell=$true; workflow_environment=$workflowEnvironment }
    [IO.File]::WriteAllText($configuration, ($bootstrapConfiguration | ConvertTo-Json -Depth 5), (New-Object Text.UTF8Encoding($false)))
    $scheduler = New-Object -ComObject 'Schedule.Service'
    $scheduler.Connect()
    $folder = $scheduler.GetFolder('\')
    $definition = $scheduler.NewTask(0)
    $definition.Principal.UserId = $account.SID.Value
    $definition.Principal.LogonType = 3
    $definition.Principal.RunLevel = 0
    $definition.Settings.Enabled = $true
    $definition.Settings.AllowDemandStart = $true
    $definition.Settings.DisallowStartIfOnBatteries = $false
    $definition.Settings.StopIfGoingOnBatteries = $false
    $definition.Settings.AllowHardTerminate = $false
    $definition.Settings.ExecutionTimeLimit = 'PT0S'
    $action = $definition.Actions.Create(0)
    $action.Path = $python
    $action.Arguments = (Quote-Argument $bootstrap) + ' --bootstrap-config ' + (Quote-Argument $configuration)
    $action.WorkingDirectory = $stage
    $taskName = 'MicCamWatchInteractiveProbe-' + [Guid]::NewGuid().ToString('N')
    $task = $folder.RegisterTaskDefinition($taskName, $definition, 2, $account.SID.Value, $null, 3, $null)
    $null = $task.RunEx($null, 4, $session.Id, $null)
    $report.task_name = $taskName
    $report.task_logon_type = 3
    $report.bootstrap_report = $bootstrapReport
    $report.status = 'running'
    Save-Report
    do {
        Assert-Deadline
        [Windows.Forms.Application]::DoEvents()
        if ([McwRdpNative]::DisconnectReason -ge 0 -or [McwRdpNative]::FatalError -ge 0) { throw 'Native interactive connection ended while fixture task was running' }
        if (Test-Path -LiteralPath $bootstrapReport) {
            $result = Get-Content -LiteralPath $bootstrapReport -Raw | ConvertFrom-Json
            if ($result.status -in @('completed', 'refused_or_failed', 'incomplete')) {
                $report.bootstrap_status = $result.status
                if ($result.status -ne 'completed') { throw 'Genuine interactive bootstrap did not complete; inspect its original stderr' }
                $code = [int]$result.script_exit_code
                break
            }
        }
        Start-Sleep -Milliseconds 100
    } while ($true)
    # Wait for the real task process to finish, not just its final JSON write.
    while ($task.State -eq 4) { Assert-Deadline; [Windows.Forms.Application]::DoEvents(); Start-Sleep -Milliseconds 100 }
    $report.last_task_result = $task.LastTaskResult
    if ($task.LastTaskResult -ne $code) { throw 'Scheduler result differs from the actual bootstrap result' }
    $rdp.Disconnect()
    $report.status = if ($code -eq 0) { 'completed' } else { 'fixture_failed' }
} catch {
    $report.status = 'refused_or_failed'
    $report.exception = $_.Exception.ToString()
    $report.exception_hresult = $_.Exception.HResult
    $code = 1
} finally {
    $password = $null
    if ($null -ne $rdp) {
        $report.disconnect_reason = [McwRdpNative]::DisconnectReason
        $report.fatal_error = [McwRdpNative]::FatalError
        $report.logon_error = [McwRdpNative]::LogonError
        try {
            $report.extended_disconnect_reason = [int]$rdp.ExtendedDisconnectReason
            if ([McwRdpNative]::DisconnectReason -ge 0) { $report.native_error_description = $rdp.GetErrorDescription([uint32][McwRdpNative]::DisconnectReason, [uint32]$rdp.ExtendedDisconnectReason) }
        } catch { $report.native_error_read_failure = $_.Exception.ToString() }
    }
    if ($report.status -eq 'refused_or_failed') {
        & secedit.exe /export /areas USER_RIGHTS /cfg (Join-Path $stage 'native-logon-rights.inf') /log (Join-Path $stage 'native-logon-rights-export.log') | Out-Null
        $since = (Get-Date).AddSeconds(-$clock.Elapsed.TotalSeconds - 5)
        $channels = @('Security','Application','System','Microsoft-Windows-TerminalServices-RemoteConnectionManager/Operational','Microsoft-Windows-TerminalServices-LocalSessionManager/Operational','Microsoft-Windows-RemoteDesktopServices-RdpCoreTS/Operational')
        foreach ($channel in $channels) {
            $file = Join-Path $stage ('native-events-' + ($channel -replace '[^A-Za-z0-9]','_') + '.json')
            try {
                $filter = @{LogName=$channel;StartTime=$since}
                if ($channel -eq 'Security') { $filter.Id=4625 }
                $events = @(Get-WinEvent -FilterHashtable $filter -MaxEvents 40 -ErrorAction Stop | Select-Object TimeCreated,Id,LevelDisplayName,Message)
                $events | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $file -Encoding UTF8
            } catch { @{query_error=$_.Exception.Message} | ConvertTo-Json | Set-Content -LiteralPath $file -Encoding UTF8 }
        }
    }
    $live = $true
    if ($null -ne $task) {
        try { $report.task_state = $task.State; $report.last_task_result = $task.LastTaskResult }
        catch { $report.task_query_error = $_.Exception.ToString() }
    }
    try {
        $sessionsLeft = @(if ($null -ne $account) { [McwRdpNative]::Logins() | Where-Object { $_.User -eq $account.Name } })
        $report.fixture_sessions_retained = @($sessionsLeft | ForEach-Object { @{ id=$_.Id; state=$_.State; protocol=$_.Protocol } })
        $live = $sessionsLeft.Count -ne 0 -or ($null -ne $task -and $task.State -eq 4)
    } catch { $report.retention_query_error = $_.Exception.ToString() }
    if ($addedLeaf -and $report.status -eq 'completed' -and -not $live) {
        try { $rootStore.Remove($leaf); $report.owned_leaf_removed = $true }
        catch { $report.trust_cleanup_error = $_.Exception.ToString(); $code = 1; $report.status = 'refused_or_failed' }
    } elseif ($addedLeaf) { $report.owned_leaf_retention = 'Retained with live/uncertain session or failed task until disposable VM teardown' }
    if ($null -ne $rootStore) { $rootStore.Close() }
    if ($null -ne $form -and -not $live) { $form.Dispose() }
    $report.account_task_retention = 'Fresh account, task and profile retained until disposable VM teardown; never killed, stopped, logged off or deleted'
    Save-Report
}
exit $code
