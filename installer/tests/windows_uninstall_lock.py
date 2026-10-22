"""Deterministically contend the real worker's installation lock.

Windows DEBUG_PROCESS stops the actual child at CREATE_PROCESS_DEBUG_EVENT,
before its first instruction. The fixture takes the production byte-range lock
there, then resumes the worker normally. No fake worker, production test hook,
process termination, device effect, privilege adjustment or timing guess.
Every created image and its kernel-reported parent are logged before the strict
worker identity checks. Only the native system-directory console host, parented
by this caller or its verified worker, is accepted as an additional descendant;
it remains debugged until exit, with native exceptions handled normally.
"""
import ctypes as C
from ctypes import wintypes as W
import hashlib
import json
import msvcrt
from pathlib import Path
import subprocess
import sys
import time
import traceback


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


class ProcessBasicInformation(C.Structure):
    _fields_ = [("reserved1", C.c_void_p), ("peb", C.c_void_p),
                ("reserved2", C.c_void_p * 2), ("pid", C.c_size_t),
                ("parent_pid", C.c_size_t)]


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
    native.GetSystemDirectoryW.argtypes = [W.LPWSTR, W.UINT]
    native.GetSystemDirectoryW.restype = W.UINT
    native.CloseHandle.argtypes = [W.HANDLE]
    native.CloseHandle.restype = W.BOOL
    ntdll = C.WinDLL("ntdll")
    ntdll.NtQueryInformationProcess.argtypes = [
        W.HANDLE, W.DWORD, C.c_void_p, W.ULONG, C.POINTER(W.ULONG)]
    ntdll.NtQueryInformationProcess.restype = C.c_long
    system_directory = C.create_unicode_buffer(32768)
    system_length = native.GetSystemDirectoryW(system_directory, len(system_directory))
    assert 0 < system_length < len(system_directory), C.WinError(C.get_last_error())
    console_image = (Path(system_directory.value) / "conhost.exe").resolve(strict=True)
    with cli.open("rb") as installed:
        cli_hash = hashlib.file_digest(installed, "sha256").digest()
    live = set()
    console_hosts = set()
    console_exits = {}
    worker = None
    worker_exit = None
    result = None
    lock = None
    locked = False
    with private_output.open("wb") as output, private_output.with_suffix(".debug.log").open("w", encoding="utf-8") as debug:
        parent = subprocess.Popen([str(cli), "uninstall"], env=env, stdin=subprocess.DEVNULL,
                                  stdout=output, stderr=output, creationflags=1)  # DEBUG_PROCESS, descendants included
        live.add(parent.pid)
        print(f"DEBUG_PROCESS caller pid={parent.pid}; ordinary-token installation-lock contention", file=debug, flush=True)
        try:
            assert native.DebugSetProcessKillOnExit(False), C.WinError(C.get_last_error())
            deadline = time.monotonic() + 45
            while live:
                assert time.monotonic() < deadline, "Worker waited on a busy installation lock instead of refusing"
                event = DebugEvent()
                if not native.WaitForDebugEvent(C.byref(event), 250):
                    assert C.get_last_error() == 121, C.WinError(C.get_last_error())
                    continue
                disposition = 0x00010002  # DBG_CONTINUE
                print(f"event={event.code} pid={event.pid} tid={event.tid}", file=debug, flush=True)
                try:
                    if event.code == 3:  # CREATE_PROCESS_DEBUG_EVENT
                        live.add(event.pid)
                        information = event.data.create
                        try:
                            path = C.create_unicode_buffer(32768)
                            length = native.GetFinalPathNameByHandleW(information.file, path, len(path), 0)
                            assert 0 < length < len(path), f"Debugged image identity unavailable: {C.WinError(C.get_last_error())}"
                            image = path.value.removeprefix("\\\\?\\")
                            basic, returned = ProcessBasicInformation(), W.ULONG()
                            status = ntdll.NtQueryInformationProcess(
                                information.process, 0, C.byref(basic), C.sizeof(basic), C.byref(returned))
                            print(f"created pid={event.pid} image={image!r} "
                                  f"kernel_parent_pid={basic.parent_pid} kernel_pid={basic.pid} "
                                  f"query_status=0x{status & 0xffffffff:08x} returned={returned.value}",
                                  file=debug, flush=True)
                            assert status >= 0 and returned.value == C.sizeof(basic), "Cannot inspect debugged process kernel parent"
                            assert basic.pid == event.pid, "Debug event and kernel process identity disagree"
                            if event.pid != parent.pid and Path(image).resolve(strict=True) == console_image:
                                assert basic.parent_pid in {parent.pid, worker} and basic.parent_pid in live, (
                                    f"System console host has an unauthenticated parent: {basic.parent_pid}")
                                console_hosts.add(event.pid)
                                print(f"authenticated system console host pid={event.pid}; "
                                      f"kernel_parent_pid={basic.parent_pid}", file=debug, flush=True)
                            elif event.pid != parent.pid:
                                helper = Path(image)
                                assert helper.name == "worker.exe" and helper.parent.name.startswith(".mcw-uninstall-"), (
                                    f"Unexpected debugged descendant: image={image!r}, pid={event.pid}, "
                                    f"kernel_parent_pid={basic.parent_pid}")
                                assert helper.parent.parent.resolve() == Path(env["TEMP"]).resolve(), "Worker is outside isolated temporary state"
                                assert worker is None, "Unexpected second uninstall worker"
                                assert basic.parent_pid == parent.pid and parent.pid in live, "Worker is not a direct child of the live caller"
                                with helper.open("rb") as staged:
                                    assert hashlib.file_digest(staged, "sha256").digest() == cli_hash, "Worker differs from the installed CLI"
                                worker = event.pid
                                result = helper.parent / "result.json"
                                # The caller released this permanent lock before CreateProcess.
                                # Stop-at-create guarantees the child has not tried to acquire it.
                                lock = (cli.parent / ".mcw-install.lock").open("r+b")
                                msvcrt.locking(lock.fileno(), msvcrt.LK_NBLCK, 1)
                                locked = True
                                print(f"real worker pid={worker}; installation byte-range lock acquired", file=debug, flush=True)
                        finally:
                            for handle in (information.file, information.process, information.thread):
                                if handle:
                                    native.CloseHandle(handle)
                    elif event.code == 5:  # EXIT_PROCESS_DEBUG_EVENT
                        live.discard(event.pid)
                        if event.pid == worker:
                            worker_exit = event.data.exit_code
                            print(f"worker exit={worker_exit}", file=debug, flush=True)
                        elif event.pid in console_hosts:
                            console_exits[event.pid] = event.data.exit_code
                            print(f"system console host pid={event.pid} exit={event.data.exit_code}", file=debug, flush=True)
                    elif event.code == 6:  # LOAD_DLL_DEBUG_EVENT
                        if event.data.load_dll.file:
                            native.CloseHandle(event.data.load_dll.file)
                    elif event.code == 1 and event.data.exception.record.code != 0x80000003:
                        disposition = 0x80010001  # DBG_EXCEPTION_NOT_HANDLED, do not hide real faults
                finally:
                    failing = sys.exc_info()[0] is not None
                    if not native.ContinueDebugEvent(event.pid, event.tid, disposition):
                        error = C.WinError(C.get_last_error())
                        print(f"ContinueDebugEvent failed: {error}", file=debug, flush=True)
                        if not failing:
                            raise error
            assert console_hosts == console_exits.keys(), "System console host remains alive"
            assert worker is not None and worker_exit is not None and worker_exit != 0, "Actual worker did not refuse its busy lock"
            caller_exit = parent.wait(timeout=5)
            print(f"caller exit={caller_exit}", file=debug, flush=True)
            assert caller_exit != 0, "Caller incorrectly acknowledged deferred success"
            assert result is not None and result.is_file(), "Worker refusal did not produce a completion record"
            record = json.loads(result.read_text())
            print(json.dumps(record), file=debug, flush=True)
            assert record["status"] == "refused_or_incomplete" and "installation lock is busy" in record["error"], "Worker completion does not truthfully identify its lock refusal"
        except BaseException:
            traceback.print_exc(file=debug)
            debug.flush()
            raise
        finally:
            # Never terminate debuggees; detach only retained fixture process IDs.
            failing = sys.exc_info()[0] is not None
            cleanup_error = None
            for pid in live:
                if not native.DebugActiveProcessStop(pid):
                    error = C.WinError(C.get_last_error())
                    print(f"debugger detach failed for pid={pid}: {error}", file=debug, flush=True)
                    if cleanup_error is None:
                        cleanup_error = error
            if lock is not None:
                try:
                    if locked:
                        lock.seek(0)
                        msvcrt.locking(lock.fileno(), msvcrt.LK_UNLCK, 1)
                except OSError as error:
                    print(f"fixture lock release failed: {error}", file=debug, flush=True)
                    if cleanup_error is None:
                        cleanup_error = error
                finally:
                    lock.close()
            if cleanup_error is not None and not failing:
                raise cleanup_error
    # Both actual parent and worker exited before the lock was released. No
    # delayed worker can later mutate the frontend/autostart after caller failure.
    return record
