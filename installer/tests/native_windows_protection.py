#!/usr/bin/env python3
"""Actual guard lifecycle/IPC/RSS proof on a disposable Windows runner with no inputs.

Refuses physical input/camera inventories, non-hosted machines and unelevated tokens.
No audio is captured, no real device is muted/enabled/disabled, no PID-name killing.
"""
import argparse
import ctypes as C
from ctypes import wintypes as W
import hashlib
import json
import os
from pathlib import Path
import struct
import statistics
import shutil
import subprocess
import tempfile
import time

from native_resources import Sampler, WindowsProcesses, sustain, windows_tray_window
from native_tui_controls import NativeTerminal


class NativePipe:
    def __init__(self, binary, directory):
        self.kernel = C.WinDLL("kernel32", use_last_error=True)
        self.advapi = C.WinDLL("advapi32", use_last_error=True)
        self.kernel.GetCurrentProcess.restype = W.HANDLE
        self.kernel.CloseHandle.argtypes = [W.HANDLE]
        self.advapi.OpenProcessToken.argtypes = [W.HANDLE, W.DWORD, C.POINTER(W.HANDLE)]
        self.advapi.GetTokenInformation.argtypes = [W.HANDLE, C.c_int, C.c_void_p, W.DWORD, C.POINTER(W.DWORD)]
        self.advapi.ConvertSidToStringSidW.argtypes = [C.c_void_p, C.POINTER(W.LPWSTR)]
        self.kernel.LocalFree.argtypes = [C.c_void_p]
        token = W.HANDLE()
        assert self.advapi.OpenProcessToken(self.kernel.GetCurrentProcess(), 8, C.byref(token)), C.WinError()
        try:
            elevation, needed = W.DWORD(), W.DWORD()
            assert self.advapi.GetTokenInformation(token, 20, C.byref(elevation), C.sizeof(elevation), C.byref(needed)), C.WinError()
            assert elevation.value, "Already-elevated disposable runner required; refusing an unattended UAC prompt"
            self.advapi.GetTokenInformation(token, 1, None, 0, C.byref(needed))
            assert 0 < needed.value <= 65536
            buffer = C.create_string_buffer(needed.value)
            assert self.advapi.GetTokenInformation(token, 1, buffer, needed, C.byref(needed)), C.WinError()
            sid = C.cast(buffer, C.POINTER(C.c_void_p))[0]
            text = W.LPWSTR()
            assert self.advapi.ConvertSidToStringSidW(sid, C.byref(text)), C.WinError()
            try:
                self.sid = text.value
            finally:
                self.kernel.LocalFree(C.cast(text, C.c_void_p))
        finally:
            self.kernel.CloseHandle(token)
        session = W.DWORD()
        self.kernel.ProcessIdToSessionId.argtypes = [W.DWORD, C.POINTER(W.DWORD)]
        assert self.kernel.ProcessIdToSessionId(os.getpid(), C.byref(session)), C.WinError()
        canonical = str(directory.resolve())
        if not canonical.startswith("\\\\?\\"):
            canonical = "\\\\?\\" + canonical
        scope = hashlib.sha256(canonical.lower().encode("utf-8")).hexdigest()
        self.prefix = f"\\\\.\\pipe\\MicCamWatch-v1-{self.sid}-{session.value}-{scope}-"
        self.binary = str(binary.resolve()).casefold()
        self.kernel.CreateFileW.argtypes = [W.LPCWSTR, W.DWORD, W.DWORD, C.c_void_p, W.DWORD, W.DWORD, W.HANDLE]
        self.kernel.CreateFileW.restype = W.HANDLE
        self.kernel.WaitNamedPipeW.argtypes = [W.LPCWSTR, W.DWORD]
        self.kernel.GetNamedPipeServerProcessId.argtypes = [W.HANDLE, C.POINTER(W.ULONG)]
        self.kernel.QueryFullProcessImageNameW.argtypes = [W.HANDLE, W.DWORD, W.LPWSTR, C.POINTER(W.DWORD)]
        self.kernel.WriteFile.argtypes = [W.HANDLE, C.c_void_p, W.DWORD, C.POINTER(W.DWORD), C.c_void_p]

    def connect(self, resource):
        name = self.prefix + resource
        deadline = time.monotonic() + 8
        while True:
            handle = self.kernel.CreateFileW(name, 0xC0000000, 0, None, 3, 0, None)
            if handle not in (None, C.c_void_p(-1).value):
                return handle
            if time.monotonic() >= deadline:
                raise C.WinError(C.get_last_error())
            self.kernel.WaitNamedPipeW(name, 100)
            time.sleep(.02)

    def track_server(self, resource, backend, sampler, earliest_creation):
        pipe = self.connect(resource)
        try:
            pid = W.ULONG()
            assert self.kernel.GetNamedPipeServerProcessId(pipe, C.byref(pid)), C.WinError()
            process = backend.kernel.OpenProcess(0x1000 | 0x10, False, pid.value)
            assert process, C.WinError()
            try:
                row = backend.read_handle(process, pid.value, name="mcw.exe")
                assert row and not row["has_exited"] and int(row["identity"]) >= earliest_creation, "Unexpected server process lifetime"
                image, size = C.create_unicode_buffer(32768), W.DWORD(32768)
                assert self.kernel.QueryFullProcessImageNameW(process, 0, image, C.byref(size)), C.WinError()
                assert str(Path(image.value).resolve()).casefold() == self.binary, "Unexpected server executable"
                with sampler.lock:
                    sampler.track(row, process)
                    key = (pid.value, row["identity"])
                    sampler.tracked[key]["role"] = resource + "_guard"
                return key
            finally:
                backend.kernel.CloseHandle(process)
        finally:
            self.kernel.CloseHandle(pipe)

    def malformed(self, resource, payload, hold_open=False):
        handle = self.connect(resource)
        started = time.monotonic()
        try:
            transferred = W.DWORD()
            buffer = C.create_string_buffer(payload)
            assert self.kernel.WriteFile(handle, buffer, len(payload), C.byref(transferred), None), C.WinError()
            assert transferred.value == len(payload)
            if hold_open:
                time.sleep(4)  # Exceeds the real server's three-second deadline.
            # Subsequent CLI status proves recovery, not merely client closure.
        finally:
            self.kernel.CloseHandle(handle)
        return time.monotonic() - started


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--proof", type=Path, required=True)
    args = parser.parse_args()
    assert os.name == "nt" and os.environ.get("GITHUB_ACTIONS") == "true" and os.environ.get("RUNNER_ENVIRONMENT") == "github-hosted", "Disposable hosted native Windows runner only"
    args.proof.mkdir(parents=True, exist_ok=True)
    # Operational locks/journals are not publishable evidence and may remain
    # locked during a failed scenario. Keep them outside the uploaded directory.
    root = Path(tempfile.mkdtemp(prefix="mcw-native-guards-"))
    appdata, localdata = root / "roaming", root / "local"
    directory = localdata / "MicCamWatch"
    (appdata / "MicCamWatch").mkdir(parents=True)
    directory.mkdir(parents=True)
    (appdata / "MicCamWatch" / "settings.toml").write_text('notifications_enabled=false\nsound_enabled=false\nshow_ready=false\nhistory_enabled=false\nmute_on_lock=false\nblock_camera_on_lock=false\nrestore_on_unlock=false\n', encoding="utf-8")
    env = dict(os.environ, APPDATA=str(appdata), LOCALAPPDATA=str(localdata), HOME=str(root / "home"), USERPROFILE=str(root / "home"))
    native, backend = NativePipe(args.binary, directory), WindowsProcesses()
    result = dict(scenario="disposable_windows_zero_native_inputs", physical_device_mutations=False, audio_capture=False, proof_complete=False)
    sampler = Sampler(backend, result)
    owned = []
    attempted = {}
    tracked_resources = set()
    terminal = None
    tray = None
    result["frontend_phases"] = []

    def command(arguments, check=True, environment=None, allowed=(0,)):
        child = subprocess.Popen([str(args.binary), *arguments], env=env if environment is None else environment, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        with sampler.lock:
            row = backend.read_handle(child._handle, child.pid, name="mcw.exe")
            if row:
                if sampler.root is None:
                    sampler.root = (child.pid, row["identity"])
                sampler.track(row, child._handle)
                sampler.tracked[(child.pid, row["identity"])]["role"] = "cli_control_or_status"
        try:
            stdout, stderr = child.communicate(timeout=25)
        except BaseException:
            child.terminate()  # Popen owns the exact native process HANDLE.
            child.wait(timeout=10)  # Do not wait again for an inherited pipe's EOF.
            raise
        if check:
            assert child.returncode in allowed, f"CLI {arguments} exited {child.returncode}: {stderr.decode('utf-8', errors='replace')[:8192]}"
        return stdout

    def status(environment=None):
        # Status 1 means observed access, not a command failure; 2 means collector
        # degradation. Complete native protection data below must still be valid.
        data = json.loads(command(["status", "--json", "--no-color"], environment=environment, allowed=(0, 1, 2)))["protection"]
        assert data["microphone_error"] is None and data["camera_error"] is None, "Native observation unavailable"
        assert data["microphone"]["endpoint_count"] == 0, "Refusing microphone mutation on a machine with input endpoints"
        assert data["camera"]["present_total"] == 0 and data["camera"]["unknown_devices"] == 0, "Refusing camera mutation on a machine with present/unknown cameras"
        return data

    def await_state(predicate, description):
        deadline = time.monotonic() + 15
        while True:
            observed = status()
            if predicate(observed):
                return observed
            assert time.monotonic() < deadline, description
            time.sleep(.1)

    try:
        initial = status()
        assert not initial["microphone"]["requested"] and not initial["camera"]["desired_blocked"], "Fresh private scope required"
        # Old/corrupt configuration must not be consulted by hidden guard dispatch.
        (appdata / "MicCamWatch" / "settings.toml").write_text("invalid_toml=[", encoding="utf-8")
        command(["__microphone-guard"])
        (appdata / "MicCamWatch" / "settings.toml").write_text('notifications_enabled=false\nsound_enabled=false\nhistory_enabled=false\nmute_on_lock=false\nblock_camera_on_lock=false\nrestore_on_unlock=false\n', encoding="utf-8")
        earliest = int(time.time() * 10_000_000) + 116444736000000000
        attempted["microphone"] = earliest
        command(["mute"])
        owned.append(native.track_server("microphone", backend, sampler, earliest))
        tracked_resources.add("microphone")
        active = await_state(lambda s: s["microphone"]["requested"] and s["microphone"]["service_active"], "Microphone guard did not activate")
        assert active["microphone"]["mute_state"] == "unavailable", "Zero endpoints must not pretend effective SDK mute"
        payloads = [(struct.pack("<I", 4097), False),
                    (struct.pack("<I", 1) + b"{", False),
                    (struct.pack("<I", 4096) + b"{", True)]
        result["malformed_peers"] = []
        for payload, hold_open in payloads:
            started = time.monotonic()
            native.malformed("microphone", payload, hold_open)
            observed = status()
            assert observed["microphone"]["requested"] and observed["microphone"]["service_active"], "Malformed peer killed the guard"
            elapsed = time.monotonic() - started
            assert elapsed < 10, "Malformed peer exceeded finite transport recovery deadline"
            result["malformed_peers"].append(dict(sent_bytes=len(payload), peer_held_open=hold_open, recovery_seconds=elapsed))
        earliest = int(time.time() * 10_000_000) + 116444736000000000
        attempted["camera"] = earliest
        command(["camera", "block"])
        owned.append(native.track_server("camera", backend, sampler, earliest))
        tracked_resources.add("camera")
        blocked = await_state(lambda s: s["camera"]["desired_blocked"] and s["camera"]["helper_active"], "Zero-camera global Block helper did not activate")
        assert blocked["camera"]["owned_blocked_present"] == 0, "Invented expected camera/owned device"
        other_env = dict(env, LOCALAPPDATA=str(root / "other-local"))
        command(["unmute"], check=False, environment=other_env)
        original = status()
        assert original["microphone"]["requested"] and original["microphone"]["service_active"], "Foreign operational scope released original microphone protection"
        other = status(other_env)
        assert not other["microphone"]["requested"] and not other["microphone"]["service_active"], "Foreign scope adopted another microphone owner's intent/service"
        command(["camera", "allow"], check=False, environment=other_env)
        original = status()
        assert original["camera"]["desired_blocked"] and original["camera"]["helper_active"], "Foreign operational scope cancelled original global camera intent"
        result["foreign_operational_scope_cannot_release_owner"] = True
        result["sustained_start_seconds"] = time.monotonic() - sampler.started
        time.sleep(20)
        result["sustained_end_seconds"] = time.monotonic() - sampler.started
        assert status()["camera"]["desired_blocked"] and status()["microphone"]["requested"]
        previous_environment = {key: os.environ.get(key) for key in ("APPDATA", "LOCALAPPDATA", "HOME", "USERPROFILE")}
        try:
            for key in previous_environment:
                os.environ[key] = env[key]
            terminal = NativeTerminal(args.binary, "en", 120, 30)
        finally:
            for key, value in previous_environment.items():
                if value is None:
                    os.environ.pop(key, None)
                else:
                    os.environ[key] = value
        terminal.drain(2)
        assert terminal.alive(), "Native top exited before interaction"
        with sampler.lock:
            row = backend.get(terminal.process.pid, name="mcw.exe")
            assert row and not row["has_exited"], "Native top resource identity unavailable"
            sampler.track(row)
            sampler.tracked[(row["pid"], row["identity"])]["role"] = "top_frontend"
        phase = dict(frontend_role="top_frontend")
        result["frontend_phases"].append(phase)
        sustain(phase, sampler, terminal.alive, lambda _: terminal.drain(.1))
        terminal.send("q")
        terminal.drain(20)
        assert not terminal.alive(), "Native top did not exit after Quit"
        terminal.close()
        terminal = None
        observed = status()
        assert observed["microphone"]["requested"] and observed["microphone"]["service_active"], "Top exit released microphone protection"
        assert observed["camera"]["desired_blocked"] and observed["camera"]["helper_active"], "Top exit released global camera protection"
        tray = subprocess.Popen([str(args.binary.with_name("mcw-tray.exe"))], env=env,
                                stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        with sampler.lock:
            row = backend.read_handle(tray._handle, tray.pid, name="mcw-tray.exe")
            assert row and not row["has_exited"], "Native tray resource identity unavailable"
            sampler.track(row, tray._handle)
            sampler.tracked[(row["pid"], row["identity"])]["role"] = "tray_frontend"
        deadline = time.monotonic() + 15
        while not windows_tray_window(tray.pid):
            assert tray.poll() is None and time.monotonic() < deadline, "Owned native tray window did not become ready"
            time.sleep(.05)
        phase = dict(frontend_role="tray_frontend")
        result["frontend_phases"].append(phase)
        sustain(phase, sampler, lambda: tray.poll() is None and windows_tray_window(tray.pid))
        assert windows_tray_window(tray.pid, close=True), "Owned tray window unavailable at close"
        tray.wait(timeout=15)
        assert tray.returncode == 0, "Native tray did not exit cleanly"
        tray = None
        observed = status()
        assert observed["microphone"]["requested"] and observed["microphone"]["service_active"], "Tray exit released microphone protection"
        assert observed["camera"]["desired_blocked"] and observed["camera"]["helper_active"], "Tray exit released global camera protection"
        command(["camera", "allow"])
        await_state(lambda s: not s["camera"]["desired_blocked"] and not s["camera"]["helper_active"], "Allow did not clear global intent/stop helper")
        command(["unmute"])
        await_state(lambda s: not s["microphone"]["requested"] and not s["microphone"]["service_active"], "Explicit release did not stop microphone guard")
        deadline = time.monotonic() + 10
        for key in owned:
            handle = backend.owned_handles[key]
            while True:
                row = backend.read_handle(handle, key[0])
                if row is None or row["has_exited"]:
                    break
                assert time.monotonic() < deadline, "Owned guard remained alive after explicit release"
                time.sleep(.05)
        result["proof_complete"] = True
    except BaseException as error:
        result["failure"] = type(error).__name__ + ": " + str(error)
        raise
    finally:
        cleanup_errors = []
        if terminal is not None:
            try:
                terminal.close()
            except Exception as error:
                cleanup_errors.append("top: " + str(error))
        if tray is not None and tray.poll() is None:
            try:
                tray.terminate()  # Exact Popen-owned native handle, not a PID lookup.
                tray.wait(timeout=10)
            except Exception as error:
                cleanup_errors.append("tray: " + str(error))
        # A failed CLI can still have started its broker. Recover custody only
        # from our private pipe, exact executable and bounded creation lifetime.
        for resource, earliest in attempted.items():
            if resource not in tracked_resources:
                try:
                    owned.append(native.track_server(resource, backend, sampler, earliest))
                except Exception as error:
                    cleanup_errors.append(resource + " custody unavailable: " + str(error))
        # Never signal by name or unverified PID. Exact known broker instances
        # can be terminated only as failed-proof cleanup on this disposable host.
        deadline = time.monotonic() + 10
        for key in owned:
            handle = backend.owned_handles[key]
            row = backend.read_handle(handle, key[0])
            if row and not row["has_exited"]:
                backend.terminate(*key)
                while time.monotonic() < deadline:
                    row = backend.read_handle(handle, key[0])
                    if row is None or row["has_exited"]:
                        break
                    time.sleep(.05)
                else:
                    cleanup_errors.append("Owned guard did not stop after failed-proof cleanup")
        sampler.finish()
        for phase in result["frontend_phases"]:
            rows = [row for row in result["samples"]
                    if phase.get("sustained_start_seconds", float("inf")) <= row["seconds"] <= phase.get("sustained_end_seconds", -1)]
            values = [row["product_rss_bytes"] for row in rows]
            required = {"microphone_guard", "camera_guard", phase["frontend_role"]}
            phase.update(
                sample_count=len(rows),
                sustained_rss_median_bytes=statistics.median(values) if values else None,
                sustained_rss_p95_bytes=sorted(values)[int((len(values) - 1) * .95)] if values else None,
                aggregate_concurrent_observed_peak_bytes=max(values, default=None),
                cpu_delta_seconds=(rows[-1]["product_cpu_seconds"] - rows[0]["product_cpu_seconds"]) if len(rows) > 1 else None,
                required_roles_present=bool(rows) and all(
                    required <= {item["role"] for item in row["per_process_memory"]} for row in rows),
            )
            if not phase["required_roles_present"]:
                cleanup_errors.append("Frontend phase did not observe both guards and UI: " + phase["frontend_role"])
        backend.close()
        try:
            shutil.rmtree(root)
        except Exception as error:
            cleanup_errors.append("Private operational scope cleanup: " + str(error))
        result["cleanup_errors"] = cleanup_errors
        had_complete_proof = result["proof_complete"]
        if cleanup_errors:
            result["proof_complete"] = False
        (args.proof / "guards.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        if cleanup_errors and had_complete_proof:
            raise AssertionError("Native proof cleanup failed: " + "; ".join(cleanup_errors))
    print("Native zero-device guard lifecycle, malformed-peer recovery and inclusive RSS proof passed")


if __name__ == "__main__":
    main()
