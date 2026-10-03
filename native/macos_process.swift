import CoreGraphics
import Darwin
import Foundation

// Root-helper protocol (all replies live under Output.effect):
//   session-lock [] -> {ok:false,state:"unknown",error:<public API limitation>}
//   terminate [decimal PID, observed instance] -> {ok:true} only after
//       task_terminate accepted the retained task authority; otherwise
//       {ok:false,error:<identity/permission/native failure>}.
// The observed instance remains macos:microphone:<pid>:<birth seconds>:<usec>.
// No mode asks for media, elevated privilege, or protection bypass.
func processEffect(mode: String, args: [String]) -> NativeEffectReply {
    switch mode {
    case "session-lock":
        guard args.isEmpty else {
            return NativeEffectReply(ok: false, error: "session-lock takes no arguments", state: "unknown")
        }
        return publicSessionLockEvidence()
    case "terminate":
        guard args.count == 2, let pid = Int32(args[0]), pid > 1,
              pid != getpid(), pid != getppid(), !args[1].isEmpty else {
            return NativeEffectReply(ok: false, error: "refusing to terminate a protected or unattributed process", state: nil)
        }
        do {
            try terminateObservedTask(pid: pid, expectedInstance: args[1])
            return NativeEffectReply(ok: true, error: nil, state: nil)
        } catch {
            return NativeEffectReply(ok: false, error: String(describing: error), state: nil)
        }
    default:
        return NativeEffectReply(ok: false, error: "unsupported process effect mode: \(mode)", state: nil)
    }
}

private func publicSessionLockEvidence() -> NativeEffectReply {
    guard let session = CGSessionCopyCurrentDictionary() else {
        return NativeEffectReply(ok: false,
            error: "screen lock state is unknown: the public Quartz session API reports no graphical session",
            state: "unknown")
    }
    let dictionary = session as NSDictionary
    guard let uid = dictionary[kCGSessionUserIDKey] as? NSNumber,
          uid.int64Value == Int64(geteuid()),
          let onConsole = dictionary[kCGSessionOnConsoleKey] as? Bool,
          let loginDone = dictionary[kCGSessionLoginDoneKey] as? Bool,
          onConsole, loginDone else {
        return NativeEffectReply(ok: false,
            error: "screen lock state is unknown: public Quartz session evidence is missing or is not this user's active console session",
            state: "unknown")
    }
    // These documented keys establish console/login context, not screen lock.
    // IOConsoleLocked and CGSSessionScreenIsLocked are explicitly defined in
    // Apple's IOKitKeysPrivate.h, not the public SDK contract. Reading their
    // spelling via a public registry function would still rely on private API.
    // A logged-in/on-console session must therefore NEVER imply Unlocked.
    return NativeEffectReply(ok: false,
        error: "screen lock state is unknown: public macOS Quartz/IOKit SDKs expose console context but no documented current screen-lock property; private IOConsoleLocked/CGSSessionScreenIsLocked keys are not used",
        state: "unknown")
}

private struct ProcessEffectError: Error, CustomStringConvertible {
    let description: String
}

private struct ProcessEffectIdentity {
    let info: proc_bsdinfo
    let executable: String

    var instance: String {
        "macos:microphone:\(info.pbi_pid):\(info.pbi_start_tvsec):\(info.pbi_start_tvusec)"
    }
}

private func readEffectIdentity(_ pid: Int32) throws -> ProcessEffectIdentity {
    var info = proc_bsdinfo()
    let size = Int32(MemoryLayout<proc_bsdinfo>.size)
    let result = withUnsafeMutablePointer(to: &info) {
        proc_pidinfo(pid, PROC_PIDTBSDINFO, 0, $0, size)
    }
    guard result == size, info.pbi_pid == UInt32(pid), info.pbi_start_tvsec > 0,
          info.pbi_start_tvusec < 1_000_000 else {
        throw ProcessEffectError(description: "cannot verify target process birth identity with public libproc: \(String(cString: strerror(errno)))")
    }
    var bytes = [CChar](repeating: 0, count: Int(MAXPATHLEN))
    let length = proc_pidpath(pid, &bytes, UInt32(bytes.count))
    guard length > 0 else {
        throw ProcessEffectError(description: "cannot verify target process executable with public libproc: \(String(cString: strerror(errno)))")
    }
    let executable = String(cString: bytes)
    guard executable.hasPrefix("/"), !executable.isEmpty else {
        throw ProcessEffectError(description: "target executable identity is unavailable")
    }
    return ProcessEffectIdentity(info: info, executable: executable)
}

private func effectAuditToken(_ task: mach_port_t) throws -> audit_token_t {
    var token = audit_token_t()
    let capacity = MemoryLayout<audit_token_t>.size / MemoryLayout<integer_t>.size
    var count = mach_msg_type_number_t(capacity)
    let status = withUnsafeMutablePointer(to: &token) { pointer in
        pointer.withMemoryRebound(to: integer_t.self, capacity: capacity) {
            task_info(task, task_flavor_t(TASK_AUDIT_TOKEN), $0, &count)
        }
    }
    guard status == KERN_SUCCESS, count == mach_msg_type_number_t(capacity) else {
        throw ProcessEffectError(description: "cannot obtain retained task audit identity (Mach \(status))")
    }
    return token
}

private func terminateObservedTask(pid: Int32, expectedInstance: String) throws {
    var task = mach_port_t(MACH_PORT_NULL)
    let authority = task_for_pid(mach_task_self_, pid, &task)
    guard authority == KERN_SUCCESS, task != mach_port_t(MACH_PORT_NULL) else {
        throw ProcessEffectError(description: "macOS denied stable task termination authority (task_for_pid Mach \(authority)); protected/hardened targets require authority allowed by the OS; no elevation, entitlement fabrication, SIP bypass or kill(pid) fallback is attempted")
    }
    defer { _ = mach_port_deallocate(mach_task_self_, task) }

    let auditBefore = try effectAuditToken(task)
    // Audit tokens are opaque: use public libbsm accessors, not val[] offsets.
    let version = audit_token_to_pidversion(auditBefore)
    guard audit_token_to_pid(auditBefore) == pid, version != 0 else {
        throw ProcessEffectError(description: "retained task audit PID/PID-version identity is unavailable or mismatched")
    }
    let before = try readEffectIdentity(pid)
    guard before.instance == expectedInstance else {
        throw ProcessEffectError(description: "process birth identity changed or the observed instance is unavailable")
    }
    guard before.info.pbi_uid != 0, before.info.pbi_uid == geteuid(),
          (before.info.pbi_flags & UInt32(PROC_FLAG_SYSTEM)) == 0 else {
        throw ProcessEffectError(description: "refusing to terminate a system process or another user's process")
    }
    let name = URL(fileURLWithPath: before.executable).lastPathComponent
    let protected = ["launchd", "loginwindow", "WindowServer", "kernel_task", "securityd",
                     "taskgated", "taskgated-helper", "trustd", "notifyd", "opendirectoryd"]
    guard !protected.contains(name) else {
        throw ProcessEffectError(description: "refusing to terminate a protected session/system executable")
    }
    var resolvedPID: pid_t = 0
    let resolved = pid_for_task(task, &resolvedPID)
    guard resolved == KERN_SUCCESS, resolvedPID == pid else {
        throw ProcessEffectError(description: "retained task no longer belongs to the expected process (Mach \(resolved))")
    }
    let after = try readEffectIdentity(pid)
    let auditAfter = try effectAuditToken(task)
    guard after.instance == before.instance, after.executable == before.executable,
          after.info.pbi_uid == before.info.pbi_uid,
          (after.info.pbi_flags & UInt32(PROC_FLAG_SYSTEM)) == 0,
          audit_token_to_pid(auditAfter) == pid,
          audit_token_to_pidversion(auditAfter) == version,
          withUnsafeBytes(of: auditBefore, { first in
              withUnsafeBytes(of: auditAfter, { second in first.elementsEqual(second) })
          }) else {
        throw ProcessEffectError(description: "process birth, executable or retained task audit identity changed before termination")
    }
    // A recycled numeric PID cannot redirect this call: task is the retained
    // Mach authority, not a PID-based signal. An exited task reports an error.
    let terminated = task_terminate(task)
    guard terminated == KERN_SUCCESS else {
        throw ProcessEffectError(description: "retained task termination failed (Mach \(terminated))")
    }
}
