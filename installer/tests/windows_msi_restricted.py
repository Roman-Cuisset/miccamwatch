#!/usr/bin/env python3
"""Hosted-only, OS-restricted SAME-user MSI consumer; never alters token fields/policy.

Evidence scope is a restricted console process, not an ordinary interactive desktop.
SAFER opens built-in levels only; no inert flag, ACL, account or UAC changes.
"""
import argparse
import ctypes as c
from ctypes import wintypes as w
import hashlib
import json
import msvcrt
import os
from pathlib import Path
import subprocess
import sys
import traceback


def require(ok, message):
    if not ok:
        raise RuntimeError(message)


def save(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


class SidAttributes(c.Structure):
    _fields_ = [("sid", c.c_void_p), ("attributes", w.DWORD)]


class Groups(c.Structure):
    _fields_ = [("count", w.DWORD), ("groups", SidAttributes * 1)]


class Startup(c.Structure):
    _fields_ = [("cb", w.DWORD), ("reserved", w.LPWSTR), ("desktop", w.LPWSTR),
                ("title", w.LPWSTR), ("x", w.DWORD), ("y", w.DWORD),
                ("xs", w.DWORD), ("ys", w.DWORD), ("xc", w.DWORD), ("yc", w.DWORD),
                ("fill", w.DWORD), ("flags", w.DWORD), ("show", w.WORD),
                ("reserved2size", w.WORD), ("reserved2", c.c_void_p),
                ("stdin", w.HANDLE), ("stdout", w.HANDLE), ("stderr", w.HANDLE)]


class Process(c.Structure):
    _fields_ = [("process", w.HANDLE), ("thread", w.HANDLE),
                ("pid", w.DWORD), ("tid", w.DWORD)]


adv = c.WinDLL("advapi32", use_last_error=True)
kernel = c.WinDLL("kernel32", use_last_error=True)
userenv = c.WinDLL("userenv", use_last_error=True)
# Explicit pointer-sized prototypes are required on 64-bit Windows.
adv.OpenProcessToken.argtypes = [w.HANDLE, w.DWORD, c.POINTER(w.HANDLE)]
adv.GetTokenInformation.argtypes = [w.HANDLE, c.c_int, c.c_void_p, w.DWORD, c.POINTER(w.DWORD)]
adv.ConvertSidToStringSidW.argtypes = [c.c_void_p, c.POINTER(w.LPWSTR)]
adv.SaferCreateLevel.argtypes = [w.DWORD, w.DWORD, w.DWORD, c.POINTER(w.HANDLE), c.c_void_p]
adv.SaferComputeTokenFromLevel.argtypes = [w.HANDLE, w.HANDLE, c.POINTER(w.HANDLE), w.DWORD, c.c_void_p]
adv.SaferCloseLevel.argtypes = [w.HANDLE]
adv.CreateRestrictedToken.argtypes = [w.HANDLE, w.DWORD, w.DWORD, c.c_void_p,
                                    w.DWORD, c.c_void_p, w.DWORD, c.c_void_p, c.POINTER(w.HANDLE)]
adv.CreateProcessAsUserW.argtypes = [w.HANDLE, w.LPCWSTR, w.LPWSTR, c.c_void_p,
                                   c.c_void_p, w.BOOL, w.DWORD, c.c_void_p,
                                   w.LPCWSTR, c.POINTER(Startup), c.POINTER(Process)]
kernel.GetCurrentProcess.restype = w.HANDLE
kernel.CloseHandle.argtypes = [w.HANDLE]
kernel.LocalFree.argtypes = [c.c_void_p]
kernel.WaitForSingleObject.argtypes = [w.HANDLE, w.DWORD]
kernel.GetExitCodeProcess.argtypes = [w.HANDLE, c.POINTER(w.DWORD)]
userenv.GetUserProfileDirectoryW.argtypes = [w.HANDLE, w.LPWSTR, c.POINTER(w.DWORD)]


def checked(ok):
    if not ok:
        raise c.WinError(c.get_last_error())


def token_info(token, kind):
    size = w.DWORD()
    adv.GetTokenInformation(token, kind, None, 0, c.byref(size))
    require(size.value > 0, "Cannot determine kernel token information size")
    value = c.create_string_buffer(size.value)
    checked(adv.GetTokenInformation(token, kind, value, size, c.byref(size)))
    return value


def sid_string(sid):
    text = w.LPWSTR()
    checked(adv.ConvertSidToStringSidW(sid, c.byref(text)))
    try:
        return text.value
    finally:
        kernel.LocalFree(c.cast(text, c.c_void_p))


def snapshot(token):
    def number(kind):
        return w.DWORD.from_buffer(token_info(token, kind)).value
    user = token_info(token, 1)
    groups = token_info(token, 2)
    count = w.DWORD.from_buffer(groups).value
    entries = (SidAttributes * count).from_address(c.addressof(groups) + Groups.groups.offset)
    admins = [{"sid": sid_string(g.sid), "attributes": g.attributes,
               "enabled": bool(g.attributes & 4), "deny_only": bool(g.attributes & 16)}
              for g in entries if sid_string(g.sid) == "S-1-5-32-544"]
    size = w.DWORD(32768)
    profile = c.create_unicode_buffer(size.value)
    checked(userenv.GetUserProfileDirectoryW(token, profile, c.byref(size)))
    return {"sid": sid_string(SidAttributes.from_buffer(user).sid),
            "session": number(12), "token_elevation": number(20),
            "token_elevation_type": number(18),
            "has_restrictions": bool(c.c_ubyte.from_buffer(token_info(token, 21)).value),
            "profile": profile.value, "userprofile_env": os.environ.get("USERPROFILE"),
            "appdata_env": os.environ.get("APPDATA"), "localappdata_env": os.environ.get("LOCALAPPDATA"),
            "administrators": admins, "admin_group_enabled": any(g["enabled"] for g in admins)}


def current_token():
    token = w.HANDLE()
    checked(adv.OpenProcessToken(kernel.GetCurrentProcess(), 0x02000000, c.byref(token)))
    return token


def same_identity(left, right):
    return all(left[key] == right[key] for key in
               ("sid", "session", "profile", "userprofile_env", "appdata_env", "localappdata_env"))


def eligible(value, original):
    return value["token_elevation"] == 0 and not value["admin_group_enabled"] and same_identity(value, original)


def invoke(binary, args, proof, label):
    stdout = proof / (label + ".stdout")
    stderr = proof / (label + ".stderr")
    with stdout.open("wb") as out, stderr.open("wb") as err:
        process = subprocess.Popen([str(binary), *args], stdin=subprocess.DEVNULL, stdout=out, stderr=err)
        try:
            process.wait(timeout=90)
        except subprocess.TimeoutExpired:
            save(proof / "pending-consumer.json", {"pid": process.pid, "args": args,
                                                   "reason": "deadline; process not killed; no MSI cleanup permitted"})
            raise
    return process.returncode, stdout.read_text(encoding="utf-8", errors="replace"), stderr.read_text(encoding="utf-8", errors="replace")


def consumer(args):
    report = {"status": "running", "scope": "OS-restricted same-user console consumer; not ordinary desktop"}
    path = args.proof / "consumer.json"
    try:
        token = current_token()
        try:
            report["actual_process_token"] = snapshot(token)
        finally:
            kernel.CloseHandle(token)
        original = json.loads((args.proof / "tokens.json").read_text(encoding="utf-8"))["original"]
        require(eligible(report["actual_process_token"], original), "Actual child token is not eligible same-user TokenElevation0")
        save(path, report)
        data = []
        for label, command, content in (
            ("settings", ["config", "settings-path"], b"history_enabled = false\nnotifications_enabled = false\nshow_ready = false\nmute_on_lock = false\nblock_camera_on_lock = false\n"),
            ("policy", ["config", "path"], b'language = "en"\n'),
            ("history", ["history", "path"], b'{"fixture":"existing-user-history-preserved"}\n')):
            code, out, err = invoke(args.binary, command, args.proof, label)
            require(code == 0 and out.strip(), f"Actual {label} path command failed; inspect retained output")
            target = Path(out.strip())
            require(target.is_absolute() and not target.exists(), f"Existing {label} data: refusing overwrite")
            require(target.is_relative_to(Path(original["profile"])), "Actual application data must remain beneath registered user's profile")
            target.parent.mkdir(parents=True, exist_ok=True)
            with target.open("xb") as output:
                output.write(content)
            data.append({"path": str(target), "sha256": sha(target)})
        payload = {name: sha(args.binary.parent / name) for name in ("mcw.exe", "mcw-tray.exe")}
        report.update(data=data, payload_before=payload)
        save(path, report)
        code, out, err = invoke(args.binary, ["uninstall"], args.proof, "uninstall")
        guidance = f"msiexec.exe /x {args.product}"
        require(code == 2 and guidance in err, "Real SDK consumer did not delegate exact MSI ProductCode with exit2")
        require(payload == {name: sha(args.binary.parent / name) for name in payload}, "Delegation modified installed payload")
        require(all(sha(Path(item["path"])) == item["sha256"] for item in data), "Delegation changed user data")
        report.update(status="passed", exit_code=code, exact_guidance=guidance, payload_and_data_unchanged=True)
    except BaseException:
        report.update(status="failed", error=traceback.format_exc())
        raise
    finally:
        save(path, report)


def launch(args):
    report = {"status": "running", "original": None, "candidates": [], "selected_mode": None,
              "scope": "restricted same-SID/session/profile process; no ordinary desktop claim"}
    path = args.proof / "tokens.json"
    token = current_token()
    selected = None
    streams = []
    try:
        original = report["original"] = snapshot(token)
        for mode, level_id in (("CreateRestrictedToken_LUA_TOKEN_DISABLE_MAX_PRIVILEGE", None),
                               ("SAFER_CONSTRAINED", 0x10000), ("SAFER_UNTRUSTED", 0x1000)):
            candidate = {"mode": mode, "scope_id": 2 if level_id else None,
                         "level_id": level_id, "flags": 0 if level_id else 5}
            restricted = w.HANDLE()
            try:
                if level_id is None:
                    checked(adv.CreateRestrictedToken(token, 5, 0, None, 0, None, 0, None, c.byref(restricted)))
                else:
                    level = w.HANDLE()
                    checked(adv.SaferCreateLevel(2, level_id, 1, c.byref(level), None))
                    try:
                        checked(adv.SaferComputeTokenFromLevel(level, token, c.byref(restricted), 0, None))
                    finally:
                        adv.SaferCloseLevel(level)
                candidate["kernel_token"] = value = snapshot(restricted)
                candidate["eligible"] = eligible(value, original)
                if candidate["eligible"] and selected is None:
                    selected = restricted
                    restricted = w.HANDLE()
                    report["selected_mode"] = mode
            except OSError as error:
                candidate["error"] = str(error)
            finally:
                if restricted:
                    kernel.CloseHandle(restricted)
                report["candidates"].append(candidate)
                save(path, report)
        require(selected is not None,
                "No OS-derived same-user TokenElevation0/admin-disabled token. Required: disposable Windows VM with genuine standard-user MSI service access, or split-token same MSI user with UAC enabled by VM provisioning; do not alter this runner's policy or SDK guard.")
        command = [sys.executable, "-B", str(Path(__file__).resolve()), "--consumer", "--binary", str(args.binary),
                   "--product", args.product, "--proof", str(args.proof)]
        startup = Startup()
        startup.cb = c.sizeof(startup)
        streams = [open(os.devnull, "rb"), (args.proof / "restricted-launch.stdout").open("wb"),
                   (args.proof / "restricted-launch.stderr").open("wb")]
        for stream in streams:
            os.set_inheritable(stream.fileno(), True)
        startup.flags = 0x100  # STARTF_USESTDHANDLES; retain real native child errors.
        startup.stdin, startup.stdout, startup.stderr = (msvcrt.get_osfhandle(stream.fileno()) for stream in streams)
        # No GUI is started or needed; retain caller's same-user session/profile.
        process = Process()
        checked(adv.CreateProcessAsUserW(selected, sys.executable,
                                       c.create_unicode_buffer(subprocess.list2cmdline(command)),
                                       None, None, True, 0x08000000, None,
                                       str(args.proof), c.byref(startup), c.byref(process)))
        kernel.CloseHandle(process.thread)
        try:
            wait = kernel.WaitForSingleObject(process.process, 360000)
            if wait == 258:
                save(args.proof / "pending-consumer.json", {"pid": process.pid,
                     "reason": "restricted consumer deadline; not killed; do not remove MSI"})
                raise TimeoutError("Restricted child still alive; no forced termination")
            require(wait == 0, "Restricted child wait failed")
            code = w.DWORD()
            checked(kernel.GetExitCodeProcess(process.process, c.byref(code)))
            report["consumer_process_exit_code"] = code.value
            require(code.value == 0, "Actual restricted consumer failed; inspect consumer.json and native outputs")
        finally:
            kernel.CloseHandle(process.process)
        report["status"] = "passed"
    except BaseException:
        report.update(status="failed", error=traceback.format_exc())
        raise
    finally:
        for stream in streams:
            stream.close()
        if selected:
            kernel.CloseHandle(selected)
        kernel.CloseHandle(token)
        save(path, report)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--consumer", action="store_true")
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--product", required=True)
    parser.add_argument("--proof", type=Path, required=True)
    args = parser.parse_args()
    require(os.name == "nt" and os.environ.get("GITHUB_ACTIONS") == "true" and
            os.environ.get("RUNNER_ENVIRONMENT") == "github-hosted", "Actual disposable hosted Windows runner required")
    args.proof = args.proof.resolve(strict=True)
    args.binary = args.binary.resolve(strict=True)
    (consumer if args.consumer else launch)(args)


if __name__ == "__main__":
    main()
