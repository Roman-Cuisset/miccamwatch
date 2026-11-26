using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using System.Security.Principal;
using System.Text;
using System.Threading;
using System.Threading.Tasks;

[assembly: DefaultDllImportSearchPaths(DllImportSearchPath.System32)]

// Read-only sampling. No token is duplicated or used, and no privilege is enabled.
public sealed class McwMsiTokenSamples : IDisposable {
    [DllImport("kernel32.dll", SetLastError=true)] static extern IntPtr OpenProcess(uint access, bool inherit, int pid);
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
    static extern bool QueryFullProcessImageName(IntPtr process, int flags, StringBuilder name, ref int length);
    [DllImport("kernel32.dll", SetLastError=true)] static extern IntPtr OpenThread(uint access, bool inherit, uint tid);
    [DllImport("kernel32.dll", SetLastError=true)] static extern bool CloseHandle(IntPtr handle);
    [DllImport("kernel32.dll", SetLastError=true)] static extern bool IsWow64Process2(IntPtr process, out ushort emulated, out ushort native);
    [DllImport("advapi32.dll", SetLastError=true)] static extern bool OpenProcessToken(IntPtr process, uint access, out IntPtr token);
    [DllImport("advapi32.dll", SetLastError=true)] static extern bool OpenThreadToken(IntPtr thread, uint access, bool openAsSelf, out IntPtr token);
    [DllImport("advapi32.dll", SetLastError=true)] static extern bool GetTokenInformation(IntPtr token, int kind, IntPtr data, int size, out int length);
    [DllImport("advapi32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
    static extern bool LookupPrivilegeName(string system, IntPtr luid, StringBuilder name, ref int length);

    readonly CancellationTokenSource stop = new CancellationTokenSource();
    readonly Dictionary<string, Sample> seen = new Dictionary<string, Sample>();
    readonly Task capture;
    readonly string userSid;
    readonly HashSet<string> images = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
    public class Sample {
        public int ProcessId {get; set;}
        public int ThreadId {get; set;}
        public string ProcessTag {get; set;}
        public string ThreadTag {get; set;}
        public string Image {get; set;}
        public string ProcessMachine {get; set;}
        public string NativeMachine {get; set;}
        public string ProcessToken {get; set;}
        public string ThreadToken {get; set;}
        public string FirstUtc {get; set;}
        public string LastUtc {get; set;}
        public int Count {get; set;}
    }
    public McwMsiTokenSamples(string fixtureSid, string windows) {
        userSid = fixtureSid;
        foreach (var folder in new[] {"System32", "SysWOW64", "SysArm32"})
            images.Add(Path.Combine(windows, folder, "msiexec.exe"));
        capture = Task.Run((Action)Capture);
    }

    static IntPtr Query(IntPtr token, int kind) {
        int length;
        GetTokenInformation(token, kind, IntPtr.Zero, 0, out length);
        if (length <= 0 || length > 65536) return IntPtr.Zero;
        var data = Marshal.AllocHGlobal(length);
        if (GetTokenInformation(token, kind, data, length, out length)) return data;
        Marshal.FreeHGlobal(data);
        return IntPtr.Zero;
    }
    static string Sid(IntPtr token, int kind) {
        var data = Query(token, kind);
        if (data == IntPtr.Zero) return "unavailable";
        try { return new SecurityIdentifier(Marshal.ReadIntPtr(data)).Value; }
        finally { Marshal.FreeHGlobal(data); }
    }
    static int Integer(IntPtr token, int kind) {
        var data = Query(token, kind);
        if (data == IntPtr.Zero) return -1;
        try { return Marshal.ReadInt32(data); }
        finally { Marshal.FreeHGlobal(data); }
    }
    static string EnabledPrivileges(IntPtr token) {
        var data = Query(token, 3);
        if (data == IntPtr.Zero) return "unavailable";
        try {
            var names = new List<string>();
            int count = Marshal.ReadInt32(data);
            for (int i = 0; i < count; i++) {
                var entry = IntPtr.Add(data, 4 + 12*i);
                if ((Marshal.ReadInt32(entry, 8) & 2) == 0) continue;
                var buffer = new StringBuilder(128);
                int length = buffer.Capacity;
                if (LookupPrivilegeName(null, entry, buffer, ref length)) names.Add(buffer.ToString());
            }
            return string.Join(",", names);
        } finally { Marshal.FreeHGlobal(data); }
    }
    static string Token(IntPtr handle, string sid) {
        int type = Integer(handle, 8);
        return String.Format("sid={0} integrity={1} type={2} impersonationLevel={3} enabled={4}",
            sid, Sid(handle, 25), type, type == 2 ? Integer(handle, 9) : -1, EnabledPrivileges(handle));
    }
    static string ThreadToken(int id) {
        IntPtr thread = OpenThread(0x0040, false, (uint)id);
        if (thread == IntPtr.Zero) return "OpenThread error " + Marshal.GetLastWin32Error();
        try {
            IntPtr token;
            if (!OpenThreadToken(thread, 8, true, out token))
                return "OpenThreadToken error " + Marshal.GetLastWin32Error();
            try { return Token(token, Sid(token, 1)); }
            finally { CloseHandle(token); }
        } finally { CloseHandle(thread); }
    }
    void Capture() {
        while (!stop.IsCancellationRequested) {
            foreach (var proc in Process.GetProcessesByName("msiexec")) {
                using (proc) {
                    try {
                        IntPtr process = OpenProcess(0x1000, false, proc.Id);
                        if (process == IntPtr.Zero) continue;
                        var path = new StringBuilder(32768);
                        int length = path.Capacity;
                        IntPtr token;
                        ushort emulated, native;
                        bool arch;
                        try {
                            if (!QueryFullProcessImageName(process, 0, path, ref length) || !images.Contains(path.ToString())) continue;
                            arch = IsWow64Process2(process, out emulated, out native);
                            if (!OpenProcessToken(process, 8, out token)) continue;
                        } finally { CloseHandle(process); }
                        string processToken;
                        try {
                            string sid = Sid(token, 1);
                            if (sid != userSid && sid != "S-1-5-18") continue;
                            processToken = Token(token, sid);
                        } finally { CloseHandle(token); }
                        foreach (ProcessThread thread in proc.Threads) {
                            using (thread) {
                                string threadToken = ThreadToken(thread.Id);
                                string key = proc.Id + ":" + thread.Id + ":" + processToken + ":" + threadToken;
                                string time = DateTime.UtcNow.ToString("O");
                                Sample row;
                                if (!seen.TryGetValue(key, out row)) {
                                    row = new Sample {
                                        ProcessId = proc.Id, ThreadId = thread.Id, Image = Path.GetFileName(path.ToString()),
                                        ProcessTag = (proc.Id & 255).ToString("X2"),
                                        ThreadTag = (thread.Id & 255).ToString("X2"),
                                        ProcessMachine = arch ? emulated.ToString("X4") : "unavailable",
                                        NativeMachine = arch ? native.ToString("X4") : "unavailable",
                                        ProcessToken = processToken, ThreadToken = threadToken, FirstUtc = time
                                    };
                                    seen.Add(key, row);
                                }
                                row.Count++;
                                row.LastUtc = time;
                            }
                        }
                    } catch (System.ComponentModel.Win32Exception) {
                        // A terminating process can disappear between inventory and query.
                    } catch (InvalidOperationException) {
                        // The process may no longer have a queryable main module.
                    }
                }
            }
            stop.Token.WaitHandle.WaitOne(10);
        }
    }
    public Sample[] Finish() {
        stop.Cancel();
        capture.GetAwaiter().GetResult();
        var rows = new Sample[seen.Count];
        seen.Values.CopyTo(rows, 0);
        return rows;
    }
    public void Dispose() {
        stop.Cancel();
        try { capture.GetAwaiter().GetResult(); }
        finally { stop.Dispose(); }
    }
}
