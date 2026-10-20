"""Deterministically contend the real worker's installation lock.

Windows DEBUG_PROCESS stops the actual child at CREATE_PROCESS_DEBUG_EVENT,
before its first instruction. The fixture takes the production byte-range lock
there, then resumes the worker normally. No fake worker, production test hook,
process termination, device effect, privilege adjustment or timing guess.
"""
import ctypes as C
from ctypes import wintypes as W
import json
import msvcrt
from pathlib import Path
import subprocess
import time


class ExceptionRecord(C.Structure):
    _fields_ = [("code", W.DWORD), ("flags", W.DWORD), ("record", C.c_void_p),
                ("address", C.c_void_p), ("count", W.DWORD), ("information", C.c_size_t * 15)]


class ExceptionInfo(C.Structure):
    _fields_ = [("record", ExceptionRecord), ("first_chance", W.DWORD)]


class CreateProcessInfo(C.Structure):
    _fields_ = [("file", W.HANDLE), ("process", W.HANDLE), ("thread", W.HANDLE),
                ("base", C.c_void_p), ("debug_offset", W.DWORD), ("debug_size", W.DWORD),
                ("tls", C.c_void_p), ("start", C.c_void_p), ("image_name", C.c_void_p),
                ("unicode", W.WORD)]


class LoadDllInfo(C.Structure):
    _fields_ = [("file", W.HANDLE), ("base", C.c_void_p), ("debug_offset", W.DWORD),
                ("debug_size", W.DWORD), ("image_name", C.c_void_p), ("unicode", W.WORD)]


class EventData(C.Union):
    _fields_ = [("exception", ExceptionInfo), ("create", CreateProcessInfo),
                ("load_dll", LoadDllInfo), ("exit_code", W.DWORD), ("storage", C.c_byte * 160)]


class DebugEvent(C.Structure):
    _fields_ = [("code", W.DWORD), ("pid", W.DWORD), ("tid", W.DWORD), ("data", EventData)]


def busy_worker(cli, env, private_output):
    assert C.sizeof(C.c_void_p) == 8 and C.sizeof(DebugEvent) == 176, "Native fixture requires Windows x64 SDK layout"
    native = C.WinDLL("kernel32", use_last_error=True)
    native.WaitForDebugEvent.argtypes = [C.POINTER(DebugEvent), W.DWORD]
    native.WaitForDebugEvent.restype = W.BOOL
    native.ContinueDebugEvent.argtypes = [W.DWORD, W.DWORD, W.DWORD]
    native.ContinueDebugEvent.restype = W.BOOL
    native.DebugSetProcessKillOnExit.argtypes = [W.BOOL]
    native.DebugSetProcessKillOnExit.restype = W.BOOL
    native.DebugActiveProcessStop.argtypes = [W.DWORD]
    native.DebugActiveProcessStop.restype = W.BOOL
    native.GetFinalPathNameByHandleW.argtypes = [W.HANDLE, W.LPWSTR, W.DWORD, W.DWORD]
    native.GetFinalPathNameByHandleW.restype = W.DWORD
    native.CloseHandle.argtypes = [W.HANDLE]
    native.CloseHandle.restype = W.BOOL
    live = set()
    worker = None
    worker_exit = None
    result = None
    lock = None
    with private_output.open("wb") as output:
        parent = subprocess.Popen([str(cli), "uninstall"], env=env, stdin=subprocess.DEVNULL,
                                  stdout=output, stderr=output, creationflags=1)  # DEBUG_PROCESS, descendants included
        live.add(parent.pid)
        assert native.DebugSetProcessKillOnExit(False), C.WinError(C.get_last_error())
        try:
            deadline = time.monotonic() + 45
            while live:
                assert time.monotonic() < deadline, "Worker waited on a busy installation lock instead of refusing"
                event = DebugEvent()
                if not native.WaitForDebugEvent(C.byref(event), 250):
                    assert C.get_last_error() == 121, C.WinError(C.get_last_error())
                    continue
                disposition = 0x00010002  # DBG_CONTINUE
                try:
                    if event.code == 3:  # CREATE_PROCESS_DEBUG_EVENT
                        live.add(event.pid)
                        information = event.data.create
                        try:
                            if event.pid != parent.pid:
                                path = C.create_unicode_buffer(32768)
                                length = native.GetFinalPathNameByHandleW(information.file, path, len(path), 0)
                                assert 0 < length < len(path), "Native worker image identity unavailable"
                                image = path.value.removeprefix("\\\\?\\")
                                helper = Path(image)
                                assert helper.name == "worker.exe" and helper.parent.name.startswith(".mcw-uninstall-"), "Unexpected debugged descendant"
                                assert helper.parent.parent.resolve() == Path(env["TEMP"]).resolve(), "Worker is outside isolated temporary state"
                                assert worker is None, "Unexpected second uninstall worker"
                                worker = event.pid
                                result = helper.parent / "result.json"
                                # The caller released this permanent lock before CreateProcess.
                                # Stop-at-create guarantees the child has not tried to acquire it.
                                lock = (cli.parent / ".mcw-install.lock").open("r+b")
                                msvcrt.locking(lock.fileno(), msvcrt.LK_NBLCK, 1)
                        finally:
                            if information.file:
                                native.CloseHandle(information.file)
                    elif event.code == 5:  # EXIT_PROCESS_DEBUG_EVENT
                        live.discard(event.pid)
                        if event.pid == worker:
                            worker_exit = event.data.exit_code
                    elif event.code == 6:  # LOAD_DLL_DEBUG_EVENT
                        if event.data.load_dll.file:
                            native.CloseHandle(event.data.load_dll.file)
                    elif event.code == 1 and event.data.exception.record.code != 0x80000003:
                        disposition = 0x80010001  # DBG_EXCEPTION_NOT_HANDLED, do not hide real faults
                finally:
                    assert native.ContinueDebugEvent(event.pid, event.tid, disposition), C.WinError(C.get_last_error())
            assert worker is not None and worker_exit is not None and worker_exit != 0, "Actual worker did not refuse its busy lock"
            assert parent.wait(timeout=5) != 0, "Caller incorrectly acknowledged deferred success"
            assert result is not None and result.is_file(), "Worker refusal did not produce a completion record"
            record = json.loads(result.read_text())
            assert record["status"] == "refused_or_incomplete" and "installation lock is busy" in record["error"], "Worker completion does not truthfully identify its lock refusal"
        finally:
            # Never terminate debuggees; detach only retained fixture process IDs.
            for pid in live:
                native.DebugActiveProcessStop(pid)
            if lock is not None:
                lock.seek(0)
                msvcrt.locking(lock.fileno(), msvcrt.LK_UNLCK, 1)
                lock.close()
    # Both actual parent and worker exited before the lock was released. No
    # delayed worker can later mutate the frontend/autostart after caller failure.
    return record
