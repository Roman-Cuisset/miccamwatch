#!/usr/bin/env python3
"""Run a real PowerShell fixture as the same ordinary Windows user, hosted CI only.

SAFER_LEVELID_NORMALUSER removes Administrator/Power User rights without changing
accounts, UAC, policy, ACLs, the user profile, session, or desktop. No inert-policy
flag, reserved flag, privilege adjustment, process termination, or fallback is
used. Both the computed token and actual child's TokenElevation must be zero.

SDK contracts:
https://learn.microsoft.com/windows/win32/api/winsafer/nf-winsafer-safercreatelevel
https://learn.microsoft.com/windows/win32/api/winsafer/nf-winsafer-safercomputetokenfromlevel
https://learn.microsoft.com/windows/win32/api/processthreadsapi/nf-processthreadsapi-createprocessasuserw
"""
import argparse
import ctypes as C
from ctypes import wintypes as W
import json
import msvcrt
import os
from pathlib import Path
import subprocess
import sys
import traceback


class StartupInfo(C.Structure):
    _fields_ = [("size", W.DWORD), ("reserved", W.LPWSTR), ("desktop", W.LPWSTR),
                ("title", W.LPWSTR), ("x", W.DWORD), ("y", W.DWORD),
                ("width", W.DWORD), ("height", W.DWORD), ("chars_x", W.DWORD),
                ("chars_y", W.DWORD), ("fill", W.DWORD), ("flags", W.DWORD),
                ("show", W.WORD), ("reserved_size", W.WORD),
                ("reserved_data", C.POINTER(W.BYTE)), ("stdin", W.HANDLE),
                ("stdout", W.HANDLE), ("stderr", W.HANDLE)]


class ProcessInfo(C.Structure):
    _fields_ = [("process", W.HANDLE), ("thread", W.HANDLE),
                ("pid", W.DWORD), ("tid", W.DWORD)]


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def checked(result):
    if not result:
        raise C.WinError(C.get_last_error())


def bind(dll, name, result, arguments):
    function = getattr(dll, name)
    function.restype = result
    function.argtypes = arguments
    return function


def launch(args, report):
    native = C.WinDLL("advapi32", use_last_error=True)
    kernel = C.WinDLL("kernel32", use_last_error=True)
    open_token = bind(native, "OpenProcessToken", W.BOOL,
                      [W.HANDLE, W.DWORD, C.POINTER(W.HANDLE)])
    token_info = bind(native, "GetTokenInformation", W.BOOL,
                      [W.HANDLE, C.c_int, C.c_void_p, W.DWORD, C.POINTER(W.DWORD)])
    sid_length = bind(native, "GetLengthSid", W.DWORD, [C.c_void_p])
    safer_create = bind(native, "SaferCreateLevel", W.BOOL,
                        [W.DWORD, W.DWORD, W.DWORD, C.POINTER(W.HANDLE), C.c_void_p])
    safer_compute = bind(native, "SaferComputeTokenFromLevel", W.BOOL,
                         [W.HANDLE, W.HANDLE, C.POINTER(W.HANDLE), W.DWORD, C.c_void_p])
    safer_close = bind(native, "SaferCloseLevel", W.BOOL, [W.HANDLE])
    create_process = bind(native, "CreateProcessAsUserW", W.BOOL,
                          [W.HANDLE, W.LPCWSTR, W.LPWSTR, C.c_void_p, C.c_void_p,
                           W.BOOL, W.DWORD, C.c_void_p, W.LPCWSTR,
                           C.POINTER(StartupInfo), C.POINTER(ProcessInfo)])
    current_process = bind(kernel, "GetCurrentProcess", W.HANDLE, [])
    close_handle = bind(kernel, "CloseHandle", W.BOOL, [W.HANDLE])
    wait_process = bind(kernel, "WaitForSingleObject", W.DWORD, [W.HANDLE, W.DWORD])
    exit_code = bind(kernel, "GetExitCodeProcess", W.BOOL, [W.HANDLE, C.POINTER(W.DWORD)])
    resume_thread = bind(kernel, "ResumeThread", W.DWORD, [W.HANDLE])

    def scalar(token, kind):
        value, needed = W.DWORD(), W.DWORD()
        checked(token_info(token, kind, C.byref(value), C.sizeof(value), C.byref(needed)))
        return value.value

    def identity(token):
        needed = W.DWORD()
        token_info(token, 1, None, 0, C.byref(needed))  # TokenUser, variable-length SID
        require(needed.value > 0, "cannot determine token user buffer size")
        storage = C.create_string_buffer(needed.value)
        checked(token_info(token, 1, storage, len(storage), C.byref(needed)))
        sid = C.cast(storage, C.POINTER(C.c_void_p)).contents.value
        length = sid_length(sid)
        require(length > 0, "cannot determine token user SID size")
        return C.string_at(sid, length), scalar(token, 12)  # TokenSessionId

    source, restricted, level, child_token = W.HANDLE(), W.HANDLE(), W.HANDLE(), W.HANDLE()
    child = ProcessInfo()
    try:
        checked(open_token(current_process(), 0x000B, C.byref(source)))
        original_identity = identity(source)
        report["source_token_elevated"] = scalar(source, 20) != 0  # TokenElevation
        # Documented USER scope, NORMALUSER level, LEVEL_OPEN; no policy writes.
        checked(safer_create(2, 0x20000, 1, C.byref(level), None))
        checked(safer_compute(level, source, C.byref(restricted), 0, None))
        report["restricted_token_elevated"] = scalar(restricted, 20) != 0
        report["restricted_same_user_and_session"] = identity(restricted) == original_identity
        require(not report["restricted_token_elevated"], "SAFER did not produce TokenIsElevated=0; no script was launched")
        require(report["restricted_same_user_and_session"], "SAFER changed the user or session; no script was launched")

        powershell = Path(os.environ["SystemRoot"]) / "System32/WindowsPowerShell/v1.0/powershell.exe"
        command = C.create_unicode_buffer(subprocess.list2cmdline(
            [str(powershell), "-NoProfile", "-STA", "-File", str(args.script)]))
        startup = StartupInfo()
        startup.size = C.sizeof(startup)
        startup.flags = 0x00000100  # STARTF_USESTDHANDLES; no desktop override
        # Files, not pipes: an incomplete child can keep writing after timeout.
        stdout_path = args.report.with_suffix(".stdout.log")
        stderr_path = args.report.with_suffix(".stderr.log")
        report["stdout_file"], report["stderr_file"] = stdout_path.name, stderr_path.name
        with open(os.devnull, "rb") as input_file, stdout_path.open("wb") as output, stderr_path.open("wb") as errors:
            handles = [msvcrt.get_osfhandle(stream.fileno()) for stream in (input_file, output, errors)]
            for handle in handles:
                os.set_handle_inheritable(handle, True)
            startup.stdin, startup.stdout, startup.stderr = handles
            try:
                # A restricted version of our own primary token does not need
                # SE_ASSIGNPRIMARYTOKEN. Environment/session/desktop are inherited.
                checked(create_process(restricted, str(powershell), command, None, None,
                                       True, 0x00000004, None, str(args.working_directory),  # CREATE_SUSPENDED
                                       C.byref(startup), C.byref(child)))
            finally:
                for handle in handles:
                    os.set_handle_inheritable(handle, False)
            report["child_pid"] = child.pid
            report["child_suspended"] = True
            checked(open_token(child.process, 0x0008, C.byref(child_token)))
            report["child_token_elevated"] = scalar(child_token, 20) != 0
            report["child_same_user_and_session"] = identity(child_token) == original_identity
            require(not report["child_token_elevated"], "actual script process has an elevated token")
            require(report["child_same_user_and_session"], "actual script process changed the user or session")
            # No script instruction executes before the actual child token is
            # observed. On verification failure leave it suspended, never kill.
            previous_suspend_count = resume_thread(child.thread)
            if previous_suspend_count == 0xFFFFFFFF:
                raise C.WinError(C.get_last_error())
            report["child_suspended"] = previous_suspend_count > 1
            require(previous_suspend_count == 1, "script thread had an unexpected suspend count")
            waited = wait_process(child.process, round(args.timeout * 1000))
            if waited == 258:  # WAIT_TIMEOUT: leave the actual child untouched.
                report["status"] = "incomplete"
                report["reason"] = "bounded wait expired; no process was killed"
                return 124
            require(waited == 0, "waiting for the script process failed")
            code = W.DWORD()
            checked(exit_code(child.process, C.byref(code)))
            report["status"] = "completed"
            report["exit_code"] = code.value
            return code.value
    finally:
        # Closing retained handles never terminates a child or any descendant.
        for handle in (child_token, child.thread, child.process, restricted, source):
            if handle:
                close_handle(handle)
        if level:
            safer_close(level)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--script", type=Path, required=True)
    parser.add_argument("--working-directory", type=Path, required=True)
    parser.add_argument("--timeout", type=float, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    require(os.environ.get("GITHUB_ACTIONS") == "true" and os.environ.get("RUNNER_ENVIRONMENT") == "github-hosted",
            "ordinary-token launcher is restricted to disposable GitHub-hosted runners")
    require(os.name == "nt", "ordinary-token launcher requires Windows")
    require(0 < args.timeout <= 86400, "a bounded timeout of at most one day is required")
    args.script = args.script.resolve(strict=True)
    args.working_directory = args.working_directory.resolve(strict=True)
    require(args.script.is_file() and args.script.suffix.lower() == ".ps1", "a real PowerShell script is required")
    require(args.working_directory.is_dir(), "working directory is not a directory")
    args.report = args.report.resolve()
    args.report.parent.mkdir(parents=True, exist_ok=True)
    report = {"status": "starting", "token_method": "SAFER_LEVELID_NORMALUSER", "no_process_was_killed": True}
    try:
        return launch(args, report)
    except BaseException as error:
        report["status"] = "refused_or_failed"
        report["error_type"] = type(error).__name__
        details = args.report.with_suffix(".exception.log")
        details.write_text(traceback.format_exc(), encoding="utf-8")
        report["details_file"] = details.name
        print("Ordinary-token launcher failed; inspect its private report and exception log", file=sys.stderr)
        return 1
    finally:
        temporary = args.report.with_suffix(".tmp")
        temporary.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        temporary.replace(args.report)


if __name__ == "__main__":
    sys.exit(main())
