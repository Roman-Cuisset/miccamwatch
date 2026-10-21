#!/usr/bin/env python3
"""Run real Windows fixtures in a genuine ordinary account, hosted CI only.

A fresh local Users-only account gets a Windows-loaded profile and a private
window station/desktop with their documented default security. A real Explorer
shell must become ready there. No existing ACL, UAC, security policy, antivirus,
privilege, token flag, or desktop is modified. No process is forcibly stopped;
accounts/profiles and live shells remain on the disposable host until teardown.

SDK contracts:
https://learn.microsoft.com/windows/win32/api/lmaccess/nf-lmaccess-netuseradd
https://learn.microsoft.com/windows/win32/api/winuser/nf-winuser-createwindowstationw
https://learn.microsoft.com/windows/win32/api/winuser/nf-winuser-createdesktopw
https://learn.microsoft.com/windows/win32/api/winbase/nf-winbase-createprocesswithlogonw
"""
import argparse
import ctypes as C
from ctypes import wintypes as W
import json
import os
from pathlib import Path
import secrets
import shutil
import subprocess
import sys
import time
import traceback
import uuid


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


class UserInfo(C.Structure):
    _fields_ = [("name", W.LPWSTR), ("password", W.LPWSTR), ("password_age", W.DWORD),
                ("privilege", W.DWORD), ("home", W.LPWSTR), ("comment", W.LPWSTR),
                ("flags", W.DWORD), ("script", W.LPWSTR)]


class GroupMember(C.Structure):
    _fields_ = [("name", W.LPWSTR)]


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def checked(result):
    if not result:
        raise C.WinError(C.get_last_error())
    return result


def bind(dll, name, result, arguments):
    function = getattr(dll, name)
    function.restype = result
    function.argtypes = arguments
    return function


def save(path, report):
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    temporary.replace(path)


class Windows:
    def __init__(self):
        self.native = C.WinDLL("advapi32", use_last_error=True)
        self.kernel = C.WinDLL("kernel32", use_last_error=True)
        self.user = C.WinDLL("user32", use_last_error=True)
        self.open_token = bind(self.native, "OpenProcessToken", W.BOOL,
                               [W.HANDLE, W.DWORD, C.POINTER(W.HANDLE)])
        self.token_info = bind(self.native, "GetTokenInformation", W.BOOL,
                               [W.HANDLE, C.c_int, C.c_void_p, W.DWORD, C.POINTER(W.DWORD)])
        self.sid_length = bind(self.native, "GetLengthSid", W.DWORD, [C.c_void_p])
        self.current_process = bind(self.kernel, "GetCurrentProcess", W.HANDLE, [])
        self.close = bind(self.kernel, "CloseHandle", W.BOOL, [W.HANDLE])
        self.wait = bind(self.kernel, "WaitForSingleObject", W.DWORD, [W.HANDLE, W.DWORD])
        self.exit_code = bind(self.kernel, "GetExitCodeProcess", W.BOOL,
                              [W.HANDLE, C.POINTER(W.DWORD)])
        self.find_window = bind(self.user, "FindWindowW", W.HWND, [W.LPCWSTR, W.LPCWSTR])
        self.station = bind(self.user, "GetProcessWindowStation", W.HANDLE, [])
        self.object_info = bind(self.user, "GetUserObjectInformationW", W.BOOL,
                                [W.HANDLE, C.c_int, C.c_void_p, W.DWORD, C.POINTER(W.DWORD)])

    def scalar(self, token, kind):
        value, needed = W.DWORD(), W.DWORD()
        checked(self.token_info(token, kind, C.byref(value), C.sizeof(value), C.byref(needed)))
        return value.value

    def identity(self, token):
        needed = W.DWORD()
        self.token_info(token, 1, None, 0, C.byref(needed))  # TokenUser
        require(needed.value > 0, "cannot determine token user buffer size")
        storage = C.create_string_buffer(needed.value)
        checked(self.token_info(token, 1, storage, len(storage), C.byref(needed)))
        sid = C.cast(storage, C.POINTER(C.c_void_p)).contents.value
        length = self.sid_length(sid)
        require(length > 0, "cannot determine token user SID size")
        return C.string_at(sid, length).hex(), self.scalar(token, 12)  # TokenSessionId

    def object_name(self, handle):
        needed = W.DWORD()
        storage = C.create_unicode_buffer(1024)
        checked(self.object_info(handle, 2, storage, C.sizeof(storage), C.byref(needed)))  # UOI_NAME
        return storage.value

    def profile_directory(self, token):
        userenv = C.WinDLL("userenv", use_last_error=True)
        query = bind(userenv, "GetUserProfileDirectoryW", W.BOOL,
                     [W.HANDLE, W.LPWSTR, C.POINTER(W.DWORD)])
        storage, length = C.create_unicode_buffer(32768), W.DWORD(32768)
        checked(query(token, storage, C.byref(length)))
        return Path(storage.value).resolve()


def create_account(api, report):
    net = C.WinDLL("netapi32", use_last_error=True)
    add_user = bind(net, "NetUserAdd", W.DWORD,
                    [W.LPCWSTR, W.DWORD, C.POINTER(UserInfo), C.POINTER(W.DWORD)])
    add_member = bind(net, "NetLocalGroupAddMembers", W.DWORD,
                      [W.LPCWSTR, W.LPCWSTR, W.DWORD, C.POINTER(GroupMember), W.DWORD])
    string_sid = bind(api.native, "ConvertStringSidToSidW", W.BOOL,
                      [W.LPCWSTR, C.POINTER(C.c_void_p)])
    lookup_sid = bind(api.native, "LookupAccountSidW", W.BOOL,
                      [W.LPCWSTR, C.c_void_p, W.LPWSTR, C.POINTER(W.DWORD),
                       W.LPWSTR, C.POINTER(W.DWORD), C.POINTER(W.DWORD)])
    free = bind(api.kernel, "LocalFree", C.c_void_p, [C.c_void_p])
    name = "mcw_" + uuid.uuid4().hex[:12]
    password = "Aa1!" + secrets.token_hex(24)
    # USER_PRIV_USER, UF_SCRIPT | UF_NORMAL_ACCOUNT: never an administrator.
    information = UserInfo(name, password, 0, 1, None, "Disposable native qualification", 0x201, None)
    parameter = W.DWORD()
    status = add_user(None, 1, C.byref(information), C.byref(parameter))
    require(status == 0, f"NetUserAdd failed ({status}, parameter {parameter.value})")
    report["fixture_account"] = name
    report["account_retention"] = "retained until disposable host teardown"
    sid = C.c_void_p()
    try:
        checked(string_sid("S-1-5-32-545", C.byref(sid)))  # Builtin Users, localized name
        group, domain = C.create_unicode_buffer(256), C.create_unicode_buffer(256)
        group_size, domain_size, use = W.DWORD(len(group)), W.DWORD(len(domain)), W.DWORD()
        checked(lookup_sid(None, sid, group, C.byref(group_size), domain, C.byref(domain_size), C.byref(use)))
        member = GroupMember(os.environ["COMPUTERNAME"] + "\\" + name)
        status = add_member(None, group.value, 3, C.byref(member), 1)
        require(status in (0, 1378), f"ordinary Users membership failed ({status})")
    finally:
        if sid:
            free(sid)
    return name, password


def launch(args, report):
    api = Windows()
    logon = bind(api.native, "LogonUserW", W.BOOL,
                 [W.LPCWSTR, W.LPCWSTR, W.LPCWSTR, W.DWORD, W.DWORD, C.POINTER(W.HANDLE)])
    create_process = bind(api.native, "CreateProcessWithLogonW", W.BOOL,
                          [W.LPCWSTR, W.LPCWSTR, W.LPCWSTR, W.DWORD, W.LPCWSTR,
                           W.LPWSTR, W.DWORD, C.c_void_p, W.LPCWSTR,
                           C.POINTER(StartupInfo), C.POINTER(ProcessInfo)])
    create_station = bind(api.user, "CreateWindowStationW", W.HANDLE,
                          [W.LPCWSTR, W.DWORD, W.DWORD, C.c_void_p])
    set_station = bind(api.user, "SetProcessWindowStation", W.BOOL, [W.HANDLE])
    close_station = bind(api.user, "CloseWindowStation", W.BOOL, [W.HANDLE])
    create_desktop = bind(api.user, "CreateDesktopW", W.HANDLE,
                          [W.LPCWSTR, W.LPCWSTR, C.c_void_p, W.DWORD, W.DWORD, C.c_void_p])
    close_desktop = bind(api.user, "CloseDesktop", W.BOOL, [W.HANDLE])
    resume_thread = bind(api.kernel, "ResumeThread", W.DWORD, [W.HANDLE])
    source, expected, actual = W.HANDLE(), W.HANDLE(), W.HANDLE()
    station, desktop = None, None
    child = ProcessInfo()
    try:
        checked(api.open_token(api.current_process(), 8, C.byref(source)))
        source_identity = api.identity(source)
        report["source_token_elevated"] = api.scalar(source, 20) != 0
        account, password = create_account(api, report)
        checked(logon(account, ".", password, 2, 0, C.byref(expected)))  # INTERACTIVE, DEFAULT provider
        expected_sid, _ = api.identity(expected)
        report["expected_token_elevated"] = api.scalar(expected, 20) != 0
        require(not report["expected_token_elevated"], "genuine standard account is unexpectedly elevated")
        report["expected_account_sid"] = expected_sid
        report["expected_session_id"] = source_identity[1]
        report["fixture_is_runner_account"] = expected_sid == source_identity[0]
        require(not report["fixture_is_runner_account"], "fixture did not receive its own account identity")

        station_name = "mcw-ordinary-" + uuid.uuid4().hex
        # Only NEW fixture-owned objects use documented default security; no
        # existing window station/desktop/file DACL or system policy is edited.
        original_station = checked(api.station())
        station = checked(create_station(station_name, 1, 0x037F, None))  # CWF_CREATE_ONLY
        try:
            checked(set_station(station))
            desktop = checked(create_desktop("default", None, None, 0, 0x01FF, None))
        finally:
            failing = sys.exc_info()[0] is not None
            if not set_station(original_station):
                report["station_restore_error"] = C.get_last_error()
                if not failing:
                    raise C.WinError(C.get_last_error())
        report["private_window_station"] = station_name
        report["private_desktop"] = "default"
        bootstrap_result = args.report.with_suffix(".bootstrap-result.json")
        configuration = args.report.with_suffix(".bootstrap.json")
        save(configuration, {"script": str(args.script), "working_directory": str(args.working_directory),
                             "timeout": args.timeout, "report": str(bootstrap_result),
                             "expected_account_sid": expected_sid, "expected_session_id": source_identity[1],
                             "expected_station": station_name,
                             "workflow_environment": {name: os.environ[name] for name in
                                                      ("GITHUB_ACTIONS", "RUNNER_ENVIRONMENT", "GITHUB_WORKSPACE", "PATH")
                                                      if name in os.environ}})
        report["bootstrap_result"] = bootstrap_result.name
        helper = args.report.with_suffix(".bootstrap-helper.py")
        shutil.copy2(Path(__file__).resolve(), helper)
        command = C.create_unicode_buffer(subprocess.list2cmdline(
            [sys.executable, "-u", "-B", str(helper), "--bootstrap-config", str(configuration)]))
        require(len(command.value) < 1024, "CreateProcessWithLogonW command exceeds its SDK limit")
        startup = StartupInfo()
        startup.size = C.sizeof(startup)
        startup.desktop = station_name + "\\default"
        # LOGON_WITH_PROFILE loads the real user hive and supplies that user's
        # environment. CREATE_SUSPENDED lets us inspect the ACTUAL primary token
        # before any Python/script instruction. No privilege adjustment is used.
        checked(create_process(account, ".", password, 1, sys.executable, command,
                               0x00000004, None, str(args.working_directory), C.byref(startup), C.byref(child)))
        report["child_pid"] = child.pid
        report["child_suspended"] = True
        profile = api.profile_directory(expected)
        report["profile_directory"] = str(profile)
        checked(api.open_token(child.process, 8, C.byref(actual)))
        report["child_token_elevated"] = api.scalar(actual, 20) != 0
        report["child_expected_user_and_session"] = api.identity(actual) == (expected_sid, source_identity[1])
        require(not report["child_token_elevated"], "actual bootstrap token is elevated")
        require(report["child_expected_user_and_session"], "actual bootstrap has a different account or native session")
        previous = resume_thread(child.thread)
        if previous == 0xFFFFFFFF:
            raise C.WinError(C.get_last_error())
        report["child_suspended"] = previous > 1
        require(previous == 1, "bootstrap thread had an unexpected suspend count")
        save(args.report, report)
        waited = api.wait(child.process, round(args.timeout * 1000))
        if waited == 258:
            report["status"] = "incomplete"
            report["reason"] = "bounded wait expired; account/profile/desktop/processes retained, nothing killed"
            return 124
        require(waited == 0, "waiting for the genuine-account bootstrap failed")
        code = W.DWORD()
        checked(api.exit_code(child.process, C.byref(code)))
        early_failure = profile / "mcw-ordinary-bootstrap-failure.log"
        if early_failure.is_file():
            evidence = args.report.with_suffix(".bootstrap-early-failure.log")
            shutil.copy2(early_failure, evidence)
            report["details_file"] = evidence.name
        report["status"] = "completed"
        report["exit_code"] = code.value
        return code.value
    finally:
        # References held by live genuine processes retain their own objects.
        # Never delete an account/profile, unload a live hive, or kill a process.
        for handle in (actual, child.thread, child.process, expected, source):
            if handle:
                api.close(handle)
        if desktop:
            close_desktop(desktop)
        if station:
            close_station(station)


def bootstrap(configuration):
    config = json.loads(configuration.read_text(encoding="utf-8"))
    report_path = Path(config["report"])
    report = {"status": "starting", "no_process_was_killed": True}
    output = report_path.with_suffix(".stdout.log").open("w", encoding="utf-8", buffering=1)
    errors = report_path.with_suffix(".stderr.log").open("w", encoding="utf-8", buffering=1)
    sys.stdout, sys.stderr = output, errors
    api = Windows()
    token = W.HANDLE()
    try:
        checked(api.open_token(api.current_process(), 8, C.byref(token)))
        expected = config["expected_account_sid"], config["expected_session_id"]
        report["token_elevated"] = api.scalar(token, 20) != 0
        report["expected_user_and_session"] = api.identity(token) == expected
        require(not report["token_elevated"] and report["expected_user_and_session"], "bootstrap identity is not the expected ordinary account/session")
        report["window_station"] = api.object_name(checked(api.station()))
        if config["expected_station"] is not None:
            require(report["window_station"].casefold() == config["expected_station"].casefold(), "bootstrap did not enter its private window station")
        profile = api.profile_directory(token)
        require(profile == Path(os.environ["USERPROFILE"]).resolve(), "environment is not the genuine Windows-loaded profile")
        import winreg
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, "Software"):
            pass
        report["windows_profile_loaded"] = True
        report["profile_directory"] = str(profile)
        os.environ.update(config["workflow_environment"])
        require(os.environ.get("GITHUB_ACTIONS") == "true" and os.environ.get("RUNNER_ENVIRONMENT") == "github-hosted", "bootstrap is not a disposable hosted fixture")
        save(report_path, report)
        require(not api.find_window("Shell_TrayWnd", None), "foreign Explorer shell found on the new private desktop")
        explorer_image = Path(os.environ["SystemRoot"]) / "explorer.exe"
        with report_path.with_suffix(".explorer.log").open("wb") as shell_output:
            shell = subprocess.Popen([str(explorer_image)], stdin=subprocess.DEVNULL,
                                     stdout=shell_output, stderr=shell_output)
        report["explorer_launch_pid"] = shell.pid
        save(report_path, report)
        deadline = time.monotonic() + min(config["timeout"], 45)
        while not (window := api.find_window("Shell_TrayWnd", None)):
            require(time.monotonic() < deadline, f"real Explorer did not establish Shell_TrayWnd (launch exit {shell.poll()}); no shell was mocked or killed")
            time.sleep(.1)
        window_pid = bind(api.user, "GetWindowThreadProcessId", W.DWORD, [W.HWND, C.POINTER(W.DWORD)])
        open_process = bind(api.kernel, "OpenProcess", W.HANDLE, [W.DWORD, W.BOOL, W.DWORD])
        image_name = bind(api.kernel, "QueryFullProcessImageNameW", W.BOOL,
                          [W.HANDLE, W.DWORD, W.LPWSTR, C.POINTER(W.DWORD)])
        pid, shell_token = W.DWORD(), W.HANDLE()
        checked(window_pid(window, C.byref(pid)))
        shell_process = checked(open_process(0x1000, False, pid.value))
        try:
            image, length = C.create_unicode_buffer(32768), W.DWORD(32768)
            checked(image_name(shell_process, 0, image, C.byref(length)))
            require(Path(image.value).resolve() == explorer_image.resolve(), "private shell window is not the actual Windows Explorer image")
            checked(api.open_token(shell_process, 8, C.byref(shell_token)))
            require(api.scalar(shell_token, 20) == 0 and api.identity(shell_token) == expected,
                    "actual Explorer is not the genuine ordinary account/session")
        finally:
            if shell_token:
                api.close(shell_token)
            api.close(shell_process)
        report["explorer_pid"] = pid.value
        report["real_ordinary_explorer_ready"] = True
        powershell = Path(os.environ["SystemRoot"]) / "System32/WindowsPowerShell/v1.0/powershell.exe"
        with report_path.with_suffix(".script.stdout.log").open("wb") as script_output, report_path.with_suffix(".script.stderr.log").open("wb") as script_errors:
            script = subprocess.Popen([str(powershell), "-NoProfile", "-STA", "-File", config["script"]],
                                      cwd=config["working_directory"], stdin=subprocess.DEVNULL,
                                      stdout=script_output, stderr=script_errors)
            report["script_pid"] = script.pid
            report["status"] = "running"
            save(report_path, report)
            try:
                code = script.wait(timeout=config["timeout"])
            except subprocess.TimeoutExpired:
                report["status"] = "incomplete"
                report["reason"] = "script wait expired; real script/shell/profile left untouched"
                return 124
        report["status"] = "completed"
        report["script_exit_code"] = code
        return code
    except BaseException as error:
        report["status"] = "refused_or_failed"
        report["error_type"] = type(error).__name__
        traceback.print_exc(file=errors)
        return 1
    finally:
        report["retention"] = "fixture account/profile/private desktop and any real Explorer retained until host teardown"
        save(report_path, report)
        if token:
            api.close(token)
        output.flush()
        errors.flush()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--script", type=Path)
    parser.add_argument("--working-directory", type=Path)
    parser.add_argument("--timeout", type=float)
    parser.add_argument("--report", type=Path)
    parser.add_argument("--bootstrap-config", type=Path)
    args = parser.parse_args()
    if args.bootstrap_config is not None:
        require(os.name == "nt", "genuine-account bootstrap requires Windows")
        try:
            return bootstrap(args.bootstrap_config.resolve(strict=True))
        except BaseException:
            # Even an unreadable config/output directory must leave the first
            # traceback in the genuine profile; the parent retains this file.
            failure = Path(os.environ["USERPROFILE"]) / "mcw-ordinary-bootstrap-failure.log"
            failure.write_text(traceback.format_exc(), encoding="utf-8")
            return 1
    require(os.environ.get("GITHUB_ACTIONS") == "true" and os.environ.get("RUNNER_ENVIRONMENT") == "github-hosted",
            "ordinary-account launcher is restricted to disposable GitHub-hosted runners")
    require(os.name == "nt", "ordinary-account launcher requires Windows")
    require(all(value is not None for value in (args.script, args.working_directory, args.timeout, args.report)),
            "script, working directory, timeout and report are required")
    require(0 < args.timeout <= 86400, "a bounded timeout of at most one day is required")
    args.script = args.script.resolve(strict=True)
    args.working_directory = args.working_directory.resolve(strict=True)
    require(args.script.is_file() and args.script.suffix.lower() == ".ps1", "a real PowerShell script is required")
    require(args.working_directory.is_dir(), "working directory is not a directory")
    args.report = args.report.resolve()
    args.report.parent.mkdir(parents=True, exist_ok=True)
    report = {"status": "starting", "token_method": "genuine_standard_account_LOGON_WITH_PROFILE",
              "no_process_was_killed": True}
    try:
        return launch(args, report)
    except BaseException as error:
        report["status"] = "refused_or_failed"
        report["error_type"] = type(error).__name__
        details = args.report.with_suffix(".exception.log")
        details.write_text(traceback.format_exc(), encoding="utf-8")
        report["details_file"] = details.name
        print("Ordinary-account launcher failed; inspect its private report and exception log", file=sys.stderr)
        return 1
    finally:
        save(args.report, report)


if __name__ == "__main__":
    sys.exit(main())
