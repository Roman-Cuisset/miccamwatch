using System;
using System.IO;
using System.Runtime.InteropServices;

[assembly: DefaultDllImportSearchPaths(DllImportSearchPath.System32)]

// The caller admits one disposable, current-user text-only MSI probe.
public static class McwMsiTransactionProbe {
    const uint InstallUiNone = 2;
    const uint LogModesIncludingVerbose = 0x1fff;
    const uint LogAppendAndFlush = 3;
    const int InstallStateAbsent = 2;
    const uint TransactionRollback = 0;
    const string NoRestart = "REBOOT=ReallySuppress";

    [DllImport("msi.dll", ExactSpelling=true)]
    static extern uint MsiSetInternalUI(uint level, IntPtr window);
    [DllImport("msi.dll", CharSet=CharSet.Unicode, ExactSpelling=true)]
    static extern uint MsiEnableLogW(uint mode, string log, uint attributes);
    [DllImport("msi.dll", CharSet=CharSet.Unicode, ExactSpelling=true)]
    static extern uint MsiBeginTransactionW(string name, uint attributes, out uint transaction, out IntPtr ownerChange);
    [DllImport("msi.dll", CharSet=CharSet.Unicode, ExactSpelling=true)]
    static extern uint MsiConfigureProductExW(string code, int level, int state, string properties);
    [DllImport("msi.dll", CharSet=CharSet.Unicode, ExactSpelling=true)]
    static extern uint MsiInstallProductW(string path, string properties);
    [DllImport("msi.dll", ExactSpelling=true)]
    static extern uint MsiEndTransaction(uint state);
    [DllImport("msi.dll", ExactSpelling=true)]
    static extern uint MsiCloseHandle(uint handle);
    [DllImport("kernel32.dll", ExactSpelling=true)]
    static extern bool CloseHandle(IntPtr handle);

    public sealed class Result {
        public string FirstUtc { get; set; }
        public string LastUtc { get; set; }
        public uint LoggingCode { get; set; }
        public uint? BeginCode { get; set; }
        public uint? RemoveCode { get; set; }
        public bool? OldMarkerPresentAfterRemoval { get; set; }
        public uint? InstallCode { get; set; }
        public uint? EndCode { get; set; }
    }

    public static Result FailUpgrade(string oldCode, string faultPackage, string marker, string log) {
        var result = new Result { FirstUtc = DateTime.UtcNow.ToString("O") };
        uint previousUi = MsiSetInternalUI(InstallUiNone, IntPtr.Zero);
        uint transaction = 0;
        IntPtr ownerChange = IntPtr.Zero;
        try {
            result.LoggingCode = MsiEnableLogW(LogModesIncludingVerbose, log, LogAppendAndFlush);
            if (result.LoggingCode != 0) return result;
            result.BeginCode = MsiBeginTransactionW("McwMsiProbe-" + Guid.NewGuid().ToString("N"),
                0, out transaction, out ownerChange);
            if (result.BeginCode != 0) return result;
            try {
                // This is a top-level uninstall owned by the same standard process,
                // not RemoveExistingProducts inside the candidate's installation.
                result.RemoveCode = MsiConfigureProductExW(oldCode, 0, InstallStateAbsent, NoRestart);
                if (result.RemoveCode == 0) {
                    result.OldMarkerPresentAfterRemoval = File.Exists(marker);
                    result.InstallCode = MsiInstallProductW(faultPackage, NoRestart);
                }
            } finally {
                // The probe never commits a provisional removal, even if the intended
                // source failure is not observed.
                result.EndCode = MsiEndTransaction(TransactionRollback);
            }
            return result;
        } finally {
            if (transaction != 0) MsiCloseHandle(transaction);
            if (ownerChange != IntPtr.Zero) CloseHandle(ownerChange);
            MsiEnableLogW(0, null, 0);
            MsiSetInternalUI(previousUi, IntPtr.Zero);
            result.LastUtc = DateTime.UtcNow.ToString("O");
        }
    }
}
