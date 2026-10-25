param(
    [switch]$Observe,
    [switch]$ValidateHost,
    [Alias('RequireStandardUser')][switch]$ValidateFixtureStandardUser,
    [Alias('LocalDisposableVm')][switch]$ValidateFixtureLocalVm,
    [Alias('FixtureRoot')][string]$ValidateFixtureRoot
)
# Read-only context observation and explicit disposable-VM admission.
# No account/token/ACL/service/device/desktop changes are performed here.
# CLI parameter variables are distinct from caller switches because this file
# is also dot-sourced by the real application and qualification consumers.
function Initialize-McwFixtureContext {
    if ('McwAppContext' -as [type]) { return }
    Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text;
public static class McwAppContext {
    [StructLayout(LayoutKind.Sequential)] struct LUID { public uint Low; public int High; }
    [StructLayout(LayoutKind.Sequential)] struct Statistics {
        public LUID Token, Authentication; public long Expiration; public uint Type, Impersonation;
        public uint Charged, Available, Groups, Privileges; public LUID Modified;
    }
    [StructLayout(LayoutKind.Sequential)] struct UnicodeText { public ushort Length, Maximum; public IntPtr Buffer; }
    [StructLayout(LayoutKind.Sequential)] struct Logon {
        public uint Size; public LUID Id; public UnicodeText User, Domain, Package; public uint Type, Session;
    }
    [StructLayout(LayoutKind.Sequential)] struct UserInfo {
        public IntPtr Name, Password; public uint PasswordAge, Privilege; public IntPtr Home, Comment;
        public uint Flags; public IntPtr Script;
    }
    [StructLayout(LayoutKind.Sequential)] struct UserSid {
        public IntPtr Name, FullName, Comment; public uint Flags; public IntPtr Sid;
    }
    [DllImport("advapi32.dll", SetLastError=true)] static extern bool OpenProcessToken(IntPtr process,uint access,out IntPtr token);
    [DllImport("advapi32.dll", SetLastError=true)] static extern bool GetTokenInformation(IntPtr token,int kind,IntPtr data,uint length,out uint needed);
    [DllImport("advapi32.dll", EntryPoint="GetTokenInformation", SetLastError=true)] static extern bool GetTokenInteger(IntPtr token,int kind,ref int data,uint length,out uint needed);
    [DllImport("advapi32.dll")] static extern bool IsTokenRestricted(IntPtr token);
    [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr handle);
    [DllImport("kernel32.dll")] static extern uint GetCurrentThreadId();
    [DllImport("user32.dll")] static extern IntPtr GetProcessWindowStation();
    [DllImport("user32.dll")] static extern IntPtr GetThreadDesktop(uint thread);
    [DllImport("user32.dll", SetLastError=true)] static extern IntPtr OpenInputDesktop(uint flags,bool inherit,uint access);
    [DllImport("user32.dll")] static extern bool CloseDesktop(IntPtr desktop);
    [DllImport("user32.dll", CharSet=CharSet.Unicode, SetLastError=true)] static extern bool GetUserObjectInformationW(IntPtr handle,int index,StringBuilder name,uint bytes,out uint needed);
    [DllImport("userenv.dll", CharSet=CharSet.Unicode, SetLastError=true)] static extern bool GetUserProfileDirectoryW(IntPtr token,StringBuilder path,ref uint length);
    [DllImport("secur32.dll")] static extern uint LsaGetLogonSessionData(ref LUID id,out IntPtr data);
    [DllImport("secur32.dll")] static extern uint LsaFreeReturnBuffer(IntPtr data);
    [DllImport("netapi32.dll", CharSet=CharSet.Unicode)] static extern uint NetUserGetInfo(string server,string name,uint level,out IntPtr data);
    [DllImport("netapi32.dll", CharSet=CharSet.Unicode)] static extern uint NetUserGetLocalGroups(string server,string name,uint level,uint flags,out IntPtr data,uint maximum,out uint count,out uint total);
    [DllImport("netapi32.dll")] static extern uint NetApiBufferFree(IntPtr data);
    [StructLayout(LayoutKind.Sequential)] struct ObjectFlags { public int Inherit, Reserved; public uint Flags; }
    [DllImport("user32.dll", EntryPoint="GetUserObjectInformationW", SetLastError=true)] static extern bool GetObjectFlags(IntPtr handle,int index,out ObjectFlags flags,uint bytes,out uint needed);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern IntPtr FindWindowW(string className,string title);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr window,out uint pid);
    [DllImport("kernel32.dll", SetLastError=true)] static extern IntPtr OpenProcess(uint access,bool inherit,uint pid);
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)] static extern bool QueryFullProcessImageNameW(IntPtr process,uint flags,StringBuilder image,ref uint length);
    static void ObserveShell(Dictionary<string,object> result) {
        var window=FindWindowW("Shell_TrayWnd",null);
        result["shellFound"]=window!=IntPtr.Zero;
        if(window==IntPtr.Zero) return;
        uint pid; GetWindowThreadProcessId(window,out pid);
        var process=OpenProcess(0x1000,false,pid);
        if(process==IntPtr.Zero) throw new Win32Exception();
        IntPtr token=IntPtr.Zero;
        try {
            var image=new StringBuilder(32768); uint length=(uint)image.Capacity;
            if(!QueryFullProcessImageNameW(process,0,image,ref length)) throw new Win32Exception();
            if(!OpenProcessToken(process,8,out token)) throw new Win32Exception();
            using(var identity=new System.Security.Principal.WindowsIdentity(token)) result["shellSid"]=identity.User.Value;
            result["shellPid"]=pid; result["shellImage"]=image.ToString();
            result["shellSessionId"]=TokenInt(token,12); result["shellElevated"]=TokenInt(token,20)!=0;
        } finally { if(token!=IntPtr.Zero) CloseHandle(token); CloseHandle(process); }
    }
    static IntPtr TokenData(IntPtr token,int kind) {
        uint size; GetTokenInformation(token,kind,IntPtr.Zero,0,out size);
        if(size==0) throw new Win32Exception();
        var data=Marshal.AllocHGlobal((int)size);
        if(!GetTokenInformation(token,kind,data,size,out size)) { Marshal.FreeHGlobal(data); throw new Win32Exception(); }
        return data;
    }
    static int TokenInt(IntPtr token,int kind) {
        // TokenHasRestrictions can return a one-byte BOOLEAN, not a DWORD.
        // ref (not out) preserves zeroed upper bytes when Windows writes only one byte.
        int value=0; uint needed;
        if(!GetTokenInteger(token,kind,ref value,sizeof(int),out needed)) throw new Win32Exception();
        if(needed!=sizeof(int) && !(kind==21 && needed==sizeof(byte)))
            throw new InvalidOperationException("Unexpected token scalar width: "+needed);
        return value;
    }
    static string ObjectName(IntPtr handle) {
        var text=new StringBuilder(1024); uint needed;
        if(handle==IntPtr.Zero || !GetUserObjectInformationW(handle,2,text,(uint)text.Capacity*2,out needed)) throw new Win32Exception();
        return text.ToString();
    }
    public static Dictionary<string,object> Observe() {
        var result=new Dictionary<string,object>();
        using(var identity=System.Security.Principal.WindowsIdentity.GetCurrent())
        using(var process=Process.GetCurrentProcess()) {
            result["sid"]=identity.User.Value; result["name"]=identity.Name;
            result["authenticationType"]=identity.AuthenticationType;
            result["administratorRole"]=new System.Security.Principal.WindowsPrincipal(identity).IsInRole(System.Security.Principal.WindowsBuiltInRole.Administrator);
            var groups=new List<string>(); foreach(var group in identity.Groups) groups.Add(group.Value);
            result["tokenGroupSids"]=groups.ToArray();
            result["accountDomain"]=Environment.UserDomainName; result["computer"]=Environment.MachineName;
            result["sessionId"]=process.SessionId;
            result["windowStation"]=ObjectName(GetProcessWindowStation());
            ObjectFlags stationFlags; uint neededFlags;
            if(!GetObjectFlags(GetProcessWindowStation(),1,out stationFlags,(uint)Marshal.SizeOf(typeof(ObjectFlags)),out neededFlags)) throw new Win32Exception();
            result["windowStationVisible"]=(stationFlags.Flags&1)!=0;
            result["desktop"]=ObjectName(GetThreadDesktop(GetCurrentThreadId()));
            var input=OpenInputDesktop(0,false,1);
            result["inputDesktopError"]=input==IntPtr.Zero ? Marshal.GetLastWin32Error() : 0;
            result["inputDesktop"]=null;
            if(input!=IntPtr.Zero) { try { result["inputDesktop"]=ObjectName(input); } finally { CloseDesktop(input); } }
            IntPtr token;
            if(!OpenProcessToken(process.Handle,8,out token)) throw new Win32Exception();
            try {
                result["elevated"]=TokenInt(token,20)!=0; result["elevationType"]=TokenInt(token,18);
                result["hasRestrictions"]=TokenInt(token,21)!=0; result["restricted"]=IsTokenRestricted(token);
                result["tokenSessionId"]=TokenInt(token,12);
                var data=TokenData(token,10); Statistics statistics;
                try { statistics=(Statistics)Marshal.PtrToStructure(data,typeof(Statistics)); } finally { Marshal.FreeHGlobal(data); }
                result["tokenType"]=statistics.Type;
                result["authenticationId"]=statistics.Authentication.High.ToString("X8")+":"+statistics.Authentication.Low.ToString("X8");
                IntPtr logon; uint status=LsaGetLogonSessionData(ref statistics.Authentication,out logon);
                if(status!=0 || logon==IntPtr.Zero) throw new InvalidOperationException("Cannot inspect actual logon session: "+status);
                try {
                    var session=(Logon)Marshal.PtrToStructure(logon,typeof(Logon));
                    result["logonType"]=session.Type; result["logonSessionId"]=session.Session;
                } finally { LsaFreeReturnBuffer(logon); }
                var profile=new StringBuilder(32768); uint length=(uint)profile.Capacity;
                if(!GetUserProfileDirectoryW(token,profile,ref length)) throw new Win32Exception();
                result["profileDirectory"]=profile.ToString();
            } finally { CloseHandle(token); }
            // Query the underlying local account, not merely the possibly
            // filtered token. Reject an admin whose token was stripped.
            IntPtr account; uint code=NetUserGetInfo(null,Environment.UserName,1,out account);
            if(code!=0) throw new InvalidOperationException("Cannot inspect local account privilege: "+code);
            try { result["accountPrivilege"]=((UserInfo)Marshal.PtrToStructure(account,typeof(UserInfo))).Privilege; }
            finally { NetApiBufferFree(account); }
            code=NetUserGetInfo(null,Environment.UserName,23,out account);
            if(code!=0) throw new InvalidOperationException("Cannot inspect local account SID: "+code);
            try { result["accountSid"]=new System.Security.Principal.SecurityIdentifier(((UserSid)Marshal.PtrToStructure(account,typeof(UserSid))).Sid).Value; }
            finally { NetApiBufferFree(account); }
            uint count,total; IntPtr localGroups;
            code=NetUserGetLocalGroups(null,Environment.UserName,0,1,out localGroups,0xffffffff,out count,out total);
            if(code!=0 || count!=total) throw new InvalidOperationException("Cannot inspect complete underlying account groups: "+code);
            try {
                var sids=new List<string>();
                for(int index=0;index<count;index++) {
                    string name=Marshal.PtrToStringUni(Marshal.ReadIntPtr(localGroups,index*IntPtr.Size));
                    sids.Add(new System.Security.Principal.NTAccount(name).Translate(typeof(System.Security.Principal.SecurityIdentifier)).Value);
                }
                result["accountLocalGroupSids"]=sids.ToArray();
            } finally { if(localGroups!=IntPtr.Zero) NetApiBufferFree(localGroups); }
        }
        ObserveShell(result);
        return result;
    }
}
'@
}
function Get-McwStandardUserContext {
    Initialize-McwFixtureContext
    $context=[McwAppContext]::Observe()
    $context['environmentProfile']=$env:USERPROFILE
    $context['environmentAppData']=$env:APPDATA
    $context['environmentLocalAppData']=$env:LOCALAPPDATA
    $context['userInteractive']=[Environment]::UserInteractive
    $profileKey="HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\ProfileList\$($context.sid)"
    $context['registeredProfile']=[Environment]::ExpandEnvironmentVariables([string](Get-ItemProperty -LiteralPath $profileKey -Name ProfileImagePath -ErrorAction Stop).ProfileImagePath)
    $context['profileHiveLoaded']=Test-Path -LiteralPath "Registry::HKEY_USERS\$($context.sid)"
    return $context
}
function Assert-McwStandardUserContext {
    param([Parameter(Mandatory=$true)]$Context)
    if ($Context.elevated -or $Context.administratorRole -or $Context.elevationType -ne 1 -or
        $Context.hasRestrictions -or $Context.restricted -or $Context.tokenType -ne 1 -or
        $Context.tokenGroupSids -contains 'S-1-5-32-544' -or $Context.accountLocalGroupSids -contains 'S-1-5-32-544' -or
        $Context.accountPrivilege -ne 1 -or $Context.accountSid -cne $Context.sid -or
        $Context.accountDomain -ine $Context.computer) {
        throw 'A genuine local nonadmin account and unrestricted default primary token are required; filtered/restricted administrators are refused.'
    }
    if ($Context.sessionId -le 0 -or $Context.tokenSessionId -ne $Context.sessionId -or
        $Context.logonSessionId -ne $Context.sessionId -or $Context.logonType -notin @(2,10) -or
        $Context.windowStation -cne 'WinSta0' -or -not $Context.windowStationVisible -or
        [string]::IsNullOrEmpty($Context.inputDesktop) -or $Context.desktop -cne $Context.inputDesktop -or
        -not $Context.userInteractive) {
        throw 'A real interactive/remote-interactive logon on the visible WinSta0 input desktop is required; batch/session-zero or moved-token approximations are refused.'
    }
    foreach ($actual in @($Context.environmentProfile,$Context.registeredProfile)) {
        if (-not [string]::Equals([IO.Path]::GetFullPath($actual).TrimEnd('\'),$Context.profileDirectory.TrimEnd('\'),[StringComparison]::OrdinalIgnoreCase)) {
            throw 'Actual token profile, registered profile and inherited USERPROFILE must agree.'
        }
    }
    if (-not $Context.profileHiveLoaded -or -not (Test-Path -LiteralPath $Context.profileDirectory -PathType Container) -or
        -not [string]::Equals($env:APPDATA,[Environment]::GetFolderPath('ApplicationData'),[StringComparison]::OrdinalIgnoreCase) -or
        -not [string]::Equals($env:LOCALAPPDATA,[Environment]::GetFolderPath('LocalApplicationData'),[StringComparison]::OrdinalIgnoreCase)) {
        throw 'The genuine account profile and its actual application-data folders must be loaded.'
    }
    $expectedExplorer=Join-Path $env:SystemRoot 'explorer.exe'
    if (-not $Context.shellFound -or $Context.shellSid -cne $Context.sid -or
        $Context.shellSessionId -ne $Context.sessionId -or $Context.shellElevated -or
        -not [string]::Equals($Context.shellImage,$expectedExplorer,[StringComparison]::OrdinalIgnoreCase)) {
        throw 'The same standard account/session must own the real unelevated Explorer notification-area window.'
    }
}
function Assert-McwFixtureRoot {
    param([string]$Path)
    if (-not [IO.Path]::IsPathRooted($Path) -or -not (Test-Path -LiteralPath $Path -PathType Container)) {
        throw 'The fixture directory must already exist at an absolute path.'
    }
    $item=Get-Item -LiteralPath $Path -Force
    $sid=[Security.Principal.WindowsIdentity]::GetCurrent().User.Value
    $owner=(Get-Acl -LiteralPath $item.FullName).GetOwner([Security.Principal.SecurityIdentifier]).Value
    if ($owner -cne $sid) { throw 'The fixture root must actually be owned by the logged-in standard account.' }
    for ($ancestor=$item; $null -ne $ancestor; $ancestor=$ancestor.Parent) {
        if (($ancestor.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw 'Fixture paths must not traverse a junction or other reparse point.'
        }
    }
    return $item.FullName
}
function Get-McwVirtualGuest {
    $machine=Get-CimInstance Win32_ComputerSystem -ErrorAction Stop
    $manufacturer=[string]$machine.Manufacturer; $model=[string]$machine.Model
    $recognized=($manufacturer -eq 'Microsoft Corporation' -and $model -eq 'Virtual Machine') -or
        ($manufacturer -match 'VMware' -and $model -match '^VMware Virtual Platform$') -or
        ($manufacturer -match 'innotek|Oracle' -and $model -eq 'VirtualBox') -or
        ($manufacturer -match 'QEMU|KVM' -and $model -match '^Standard PC|^KVM$')
    if (-not $machine.HypervisorPresent -or -not $recognized) {
        throw 'Recognized virtual-guest metadata is required; HypervisorPresent alone does not admit a physical VBS/Hyper-V host.'
    }
    return @{manufacturer=$manufacturer;model=$model}
}
function Assert-McwDisposableFixtureHost {
    param([switch]$RequireStandardUser,[switch]$LocalDisposableVm,[string]$FixtureRoot)
    if ($env:OS -cne 'Windows_NT') { throw 'A Windows fixture host is required.' }
    if ($LocalDisposableVm) {
        if (-not $RequireStandardUser -or $env:GITHUB_ACTIONS -ceq 'true' -or $env:RUNNER_ENVIRONMENT) {
            throw 'Local disposable-guest mode requires strict standard-user proof and must not impersonate or mix with GitHub runner modes.'
        }
        $guest=Get-McwVirtualGuest
        $context=Get-McwStandardUserContext
        Assert-McwStandardUserContext -Context $context
        if (($context.name -split '\\')[-1] -cnotmatch '^mcw_qa_[A-Za-z0-9]{4,12}$') {
            throw 'The actual local guest token must identify a fresh mcw_qa_* fixture account.'
        }
        $root=Assert-McwFixtureRoot -Path $FixtureRoot
        return @{kind='local-disposable-vm';standardUserContext=$context;fixtureRoot=$root
            manufacturer=$guest.manufacturer;model=$guest.model
            disposalAuthority='Explicit operator-authorized local disposable guest; VM metadata is not cryptographic attestation.'}
    }
    if ($FixtureRoot) { throw 'An explicit fixture root is only accepted in local disposable-guest mode.' }
    if ($env:GITHUB_ACTIONS -cne 'true') {
        throw 'A genuine GitHub Actions Windows fixture host is required; never fake hosted markers on a personal PC.'
    }
    if ($env:RUNNER_ENVIRONMENT -ceq 'github-hosted') {
        $context=$null
        if ($RequireStandardUser) { $context=Get-McwStandardUserContext; Assert-McwStandardUserContext -Context $context }
        return @{kind='github-hosted';standardUserContext=$context;fixtureRoot=$env:RUNNER_TEMP}
    }
    if ($env:RUNNER_ENVIRONMENT -cne 'self-hosted' -or -not $RequireStandardUser -or
        $env:MCW_DISPOSABLE_STANDARD_VM -cne 'true' -or
        $env:RUNNER_NAME -cnotmatch '^mcw-standard-disposable-[A-Za-z0-9._-]+$') {
        throw 'Self-hosted qualification requires an explicitly disposable standard-user VM, dedicated runner identity and fresh mcw_qa_* account.'
    }
    $guest=Get-McwVirtualGuest
    $context=Get-McwStandardUserContext
    Assert-McwStandardUserContext -Context $context
    if (($context.name -split '\\')[-1] -cnotmatch '^mcw_qa_[A-Za-z0-9]{4,12}$') {
        throw 'The account name resolved from the actual token must be a fresh mcw_qa_* fixture identity; USERNAME is not authority.'
    }
    return @{kind='dedicated-standard-vm';standardUserContext=$context;fixtureRoot=$env:RUNNER_TEMP
        manufacturer=$guest.manufacturer;model=$guest.model
        disposalAuthority='Explicit operator-provisioned ephemeral guest; metadata is not cryptographic attestation.'}
}
if ($Observe) { Get-McwStandardUserContext | ConvertTo-Json -Depth 12 -Compress }
if ($ValidateHost) { Assert-McwDisposableFixtureHost -RequireStandardUser:$ValidateFixtureStandardUser -LocalDisposableVm:$ValidateFixtureLocalVm -FixtureRoot $ValidateFixtureRoot | ConvertTo-Json -Depth 12 -Compress }
