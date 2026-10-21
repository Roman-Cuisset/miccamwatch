#!/usr/bin/env python3
"""Run native fixtures as a genuine ordinary account, on disposable hosted CI.

Only the fresh fixture identity receives SeBatchLogonRight. Task Scheduler loads
its real profile in its own noninteractive session; the bootstrap verifies its
actual SID/session/elevation and actual Explorer before running the fixture.
The experimental interactive provider instead validates the existing real login
Explorer using the same image/SID/session/elevation checks; it creates no shell.
Existing accounts, namespace ACLs, UAC and device permissions are untouched.
No token privilege is adjusted and no task/process/account is forcibly removed.

SDK contracts:
https://learn.microsoft.com/windows/win32/taskschd/security-contexts-for-running-tasks
https://learn.microsoft.com/windows/win32/api/ntsecapi/nf-ntsecapi-lsaaddaccountrights
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



class UserInfo(C.Structure):
    _fields_ = [("name", W.LPWSTR), ("password", W.LPWSTR), ("password_age", W.DWORD),
                ("privilege", W.DWORD), ("home", W.LPWSTR), ("comment", W.LPWSTR),
                ("flags", W.DWORD), ("script", W.LPWSTR)]


class GroupMember(C.Structure):
    _fields_ = [("name", W.LPWSTR)]


class LsaAttributes(C.Structure):
    _fields_ = [("length", W.DWORD), ("root", W.HANDLE), ("name", C.c_void_p),
                ("flags", W.DWORD), ("descriptor", C.c_void_p), ("quality", C.c_void_p)]


class LsaString(C.Structure):
    _fields_ = [("length", W.USHORT), ("maximum", W.USHORT), ("buffer", C.c_void_p)]

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


def grant_fixture_batch_logon(api, sid):
    """Authorize only the newly authenticated fixture SID, never an existing user."""
    open_policy = bind(api.native, "LsaOpenPolicy", W.LONG,
                       [C.c_void_p, C.POINTER(LsaAttributes), W.DWORD, C.POINTER(W.HANDLE)])
    add_right = bind(api.native, "LsaAddAccountRights", W.LONG,
                     [W.HANDLE, C.c_void_p, C.POINTER(LsaString), W.ULONG])
    win_error = bind(api.native, "LsaNtStatusToWinError", W.ULONG, [W.LONG])
    close_policy = bind(api.native, "LsaClose", W.LONG, [W.HANDLE])
    attributes, policy = LsaAttributes(), W.HANDLE()
    attributes.length = C.sizeof(attributes)
    status = open_policy(None, C.byref(attributes), 0x810, C.byref(policy))
    if status:
        raise C.WinError(win_error(status))
    try:
        sid_buffer = C.create_string_buffer(bytes.fromhex(sid))
        text = C.create_unicode_buffer("SeBatchLogonRight")
        right = LsaString(len("SeBatchLogonRight") * 2, C.sizeof(text),
                          C.cast(text, C.c_void_p))
        status = add_right(policy, sid_buffer, C.byref(right), 1)
        if status:
            raise C.WinError(win_error(status))
    finally:
        close_policy(policy)


SCHEDULER_SCRIPT = r"""
$ErrorActionPreference = 'Stop'
$configuration = [Console]::In.ReadToEnd() | ConvertFrom-Json
$scheduler = New-Object -ComObject 'Schedule.Service'
$scheduler.Connect()
$folder = $scheduler.GetFolder('\')
$definition = $scheduler.NewTask(0)
$definition.Principal.UserId = $configuration.user
$definition.Principal.LogonType = 1
$definition.Principal.RunLevel = 0
$definition.Settings.Enabled = $true
$definition.Settings.AllowDemandStart = $true
$definition.Settings.DisallowStartIfOnBatteries = $false
$definition.Settings.StopIfGoingOnBatteries = $false
$definition.Settings.AllowHardTerminate = $false
$definition.Settings.ExecutionTimeLimit = 'PT0S'
$action = $definition.Actions.Create(0)
$action.Path = $configuration.python
$action.Arguments = $configuration.arguments
$action.WorkingDirectory = $configuration.directory
$task = $folder.RegisterTaskDefinition($configuration.name,$definition,2,$configuration.user,$configuration.password,1,$null)
$configuration.password = $null
Start-ScheduledTask -TaskName $configuration.name -TaskPath '\'
@{task_name=$configuration.name;state=$task.State;last_task_result=$task.LastTaskResult;principal=$task.Definition.Principal.UserId} |
    ConvertTo-Json | Set-Content -LiteralPath $configuration.report -Encoding UTF8
"""


def launch(args, report):
    api = Windows()
    logon = bind(api.native, "LogonUserW", W.BOOL,
                 [W.LPCWSTR, W.LPCWSTR, W.LPCWSTR, W.DWORD, W.DWORD, C.POINTER(W.HANDLE)])
    source, interactive, batch = W.HANDLE(), W.HANDLE(), W.HANDLE()
    try:
        checked(api.open_token(api.current_process(), 8, C.byref(source)))
        source_identity = api.identity(source)
        report["source_token_elevated"] = api.scalar(source, 20) != 0
        account, password = create_account(api, report)
        checked(logon(account, ".", password, 2, 0, C.byref(interactive)))
        expected_sid, _ = api.identity(interactive)
        require(api.scalar(interactive, 20) == 0, "fresh fixture identity is elevated")
        report["fixture_is_runner_account"] = expected_sid == source_identity[0]
        require(not report["fixture_is_runner_account"], "fixture must have its own identity")
        if not logon(account, ".", password, 4, 0, C.byref(batch)):
            error = C.get_last_error()
            report["initial_batch_logon_error"] = error
            if error != 1385:
                raise C.WinError(error)
            grant_fixture_batch_logon(api, expected_sid)
            report["fresh_fixture_logon_right"] = "SeBatchLogonRight"
            checked(logon(account, ".", password, 4, 0, C.byref(batch)))
        batch_sid, authentication_session = api.identity(batch)
        report["batch_authentication_session_id"] = authentication_session
        require(batch_sid == expected_sid, "batch authentication has an unexpected account SID")
        report["expected_token_elevated"] = api.scalar(batch, 20) != 0
        require(not report["expected_token_elevated"], "batch authentication is elevated")
        report.update(expected_account_sid=expected_sid, expected_session_id=0)
        bootstrap_result = args.report.with_suffix(".bootstrap-result.json")
        configuration = args.report.with_suffix(".bootstrap.json")
        save(configuration, {"script": str(args.script), "working_directory": str(args.working_directory),
                             "timeout": max(1, args.timeout - 60), "report": str(bootstrap_result),
                             "expected_account_sid": expected_sid, "expected_session_id": 0,
                             "workflow_environment": {name: os.environ[name] for name in
                                                      ("GITHUB_ACTIONS", "RUNNER_ENVIRONMENT", "GITHUB_WORKSPACE", "PATH")
                                                      if name in os.environ}})
        helper = args.report.with_suffix(".bootstrap-helper.py")
        shutil.copy2(Path(__file__).resolve(), helper)
        broker_script = args.report.with_suffix(".scheduler.ps1")
        broker_script.write_text(SCHEDULER_SCRIPT, encoding="utf-8")
        scheduler_result = args.report.with_suffix(".scheduler.json")
        task_name = "MicCamWatch-qualification-" + uuid.uuid4().hex
        report.update(bootstrap_result=bootstrap_result.name, task_name=task_name,
                      scheduler_result=scheduler_result.name)
        save(args.report, report)
        parameters = {"user": os.environ["COMPUTERNAME"] + "\\" + account,
                      "password": password, "python": sys.executable,
                      "arguments": subprocess.list2cmdline(["-u", "-B", str(helper), "--bootstrap-config", str(configuration)]),
                      "directory": str(args.working_directory), "name": task_name,
                      "report": str(scheduler_result)}
        powershell = Path(os.environ["SystemRoot"]) / "System32/WindowsPowerShell/v1.0/powershell.exe"
        deadline = time.monotonic() + args.timeout
        with args.report.with_suffix(".scheduler.stdout.log").open("wb") as output, args.report.with_suffix(".scheduler.stderr.log").open("wb") as errors:
            broker = subprocess.Popen([str(powershell), "-NoProfile", "-NonInteractive", "-File", str(broker_script)],
                                      stdin=subprocess.PIPE, stdout=output, stderr=errors,
                                      cwd=args.working_directory)
            try:
                broker.communicate(json.dumps(parameters).encode("utf-8"), timeout=min(60, args.timeout))
            except subprocess.TimeoutExpired:
                report.update(status="incomplete", reason="scheduler request wait expired; broker/task/account retained")
                return 124
        require(broker.returncode == 0, "native scheduler request failed; inspect scheduler stderr")
        while time.monotonic() < deadline:
            if bootstrap_result.is_file():
                result = json.loads(bootstrap_result.read_text(encoding="utf-8"))
                if result["status"] in ("completed", "refused_or_failed", "incomplete"):
                    report.update(child_token_elevated=result.get("token_elevated"),
                                  child_expected_user_and_session=result.get("expected_user_and_session"),
                                  status=result["status"], exit_code=result.get("script_exit_code"))
                    return int(result.get("script_exit_code", 124 if result["status"] == "incomplete" else 1))
            time.sleep(.1)
        report.update(status="incomplete", reason="bootstrap wait expired; task/profile/processes retained, nothing stopped")
        return 124
    finally:
        for handle in (batch, interactive, source):
            if handle:
                api.close(handle)


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
        explorer_image = Path(os.environ["SystemRoot"]) / "explorer.exe"
        if config.get("existing_interactive_shell", False):
            require(expected[1] > 0 and report["window_station"].casefold() == "winsta0",
                    "existing interactive shell requires the real nonzero WinSta0 login")
            deadline = time.monotonic() + min(config["timeout"], 45)
            while not (window := api.find_window("Shell_TrayWnd", None)):
                require(time.monotonic() < deadline,
                        "real interactive logon did not establish Explorer; no replacement shell was launched")
                time.sleep(.1)
        else:
            require(not api.find_window("Shell_TrayWnd", None), "foreign Explorer shell found on the new private desktop")
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
        report["retention"] = "fixture account/task/profile and any real Explorer retained until hosted VM teardown"
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
    report = {"status": "starting", "token_method": "genuine_standard_account_TASK_LOGON_PASSWORD",
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
