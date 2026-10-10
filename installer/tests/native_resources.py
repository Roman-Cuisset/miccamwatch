#!/usr/bin/env python3
"""Real-process resource qualification, not a physical-device latency test.

Private proof dependencies: pyte, plus pywinpty on Windows (NativeTerminal).
Local: --proof DIR [--binary MCW]; baseline is always downloaded and hash-pinned.
Tray requires --runner-desktop on a disposable, logged-in native desktop runner.
No raw status, terminal, event, process arguments or clipboard are persisted.
"""
import argparse
import ctypes as C
import hashlib
import json
import os
from pathlib import Path
import platform
import signal
import statistics
import subprocess
import sys
import tarfile
import tempfile
import threading
import time
import urllib.request
import zipfile

from native_tui_controls import NativeTerminal

BASELINE_VERSION = "v0.16.1"
BASELINE_SOURCE = "7b134e3500860bd185ddc3955107b8082d414e78"
HASHES = {
    "linux-x86_64": "07f2527002eb07a01cf612c014d76b6eeaea4316bd589028ba5dac5c3425f805",
    "macos-aarch64": "ec1ecb4554cc14da5f1cbdc5e15af71766506ebe0d98643ed8137b56c052a5af",
    "macos-x86_64": "9000d6353c98bc5ada162b5df13afe4b9477748ecfbe1c35245c019ea0d04bad",
    "windows-x86_64": "276f0719b595e30b171ab1010e418de7bc4defcfc05e87b7e048a11406981e70",
}
BUDGET = 15_000_000
INTERVAL = .025
WARMUP = 2.0
DURATION = 20.0


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        while chunk := stream.read(65536):
            digest.update(chunk)
    return digest.hexdigest()


def record(pid, parent, identity, name, rss, cpu, highwater=None):
    return dict(pid=pid, parent=parent, identity=str(identity), name=name,
                rss=int(rss), cpu_seconds=cpu, os_highwater_bytes=highwater)


class LinuxProcesses:
    method = "procfs stat/status; RSS pages, VmHWM, utime+stime"

    def __init__(self):
        self.ticks = os.sysconf("SC_CLK_TCK")
        self.page = os.sysconf("SC_PAGE_SIZE")
        self.owned_pidfds = {}
        self.pidfd_errors = {}

    def get(self, pid):
        try:
            text = Path(f"/proc/{pid}/stat").read_text()
            closing = text.rindex(")")
            fields = text[closing + 2:].split()
            status = Path(f"/proc/{pid}/status").read_text().splitlines()
            high = next((int(line.split()[1]) * 1024 for line in status if line.startswith("VmHWM:")), None)
            check = Path(f"/proc/{pid}/stat").read_text()
            if check[check.rindex(")") + 2:].split()[19] != fields[19]:
                return None
            row = record(pid, int(fields[1]), fields[19], text[text.index("(") + 1:closing],
                         max(0, int(fields[21])) * self.page,
                         (int(fields[11]) + int(fields[12])) / self.ticks, high)
            row["has_exited"] = fields[0] == "Z"
            values = {line.split(":", 1)[0]: int(line.split()[1]) * 1024 for line in status
                      if line.startswith(("RssAnon:", "RssFile:", "RssShmem:"))}
            row["os_memory_breakdown"] = dict(rss_anon_bytes=values.get("RssAnon"), rss_file_bytes=values.get("RssFile"),
                                              rss_shmem_bytes=values.get("RssShmem"), private_resident_bytes=None,
                                              proportional_resident_bytes=None, private_commit_bytes=None)
            return row
        except (OSError, ValueError, IndexError):
            return None

    def private_rollup(self, row):
        result = dict(row["os_memory_breakdown"])
        try:
            lines = Path(f"/proc/{row['pid']}/smaps_rollup").read_text().splitlines()
            values = {line.split(":", 1)[0]: int(line.split()[1]) * 1024 for line in lines
                      if line.startswith(("Private_Clean:", "Private_Dirty:", "Pss:"))}
            # Confirm identity after reading mappings so reused PIDs cannot
            # contribute an unrelated process's residency.
            current = self.get(row["pid"])
            if not current or current["identity"] != row["identity"]:
                return None
            result["private_resident_bytes"] = values["Private_Clean"] + values["Private_Dirty"]
            result["proportional_resident_bytes"] = values.get("Pss")
            result["smaps_rollup_available"] = True
        except (OSError, KeyError, ValueError):
            result["smaps_rollup_available"] = False
        return result

    def all(self):
        return [row for entry in Path("/proc").iterdir() if entry.name.isdigit()
                for row in [self.get(int(entry.name))] if row]

    def retain(self, key):
        before = self.get(key[0])
        if not before or before["identity"] != key[1]:
            return False
        fd = None
        try:
            if not hasattr(os, "pidfd_open") or not hasattr(signal, "pidfd_send_signal"):
                self.pidfd_errors[key] = "Python/kernel pidfd API unavailable"
                return False
            fd = os.pidfd_open(key[0], 0)
            after = self.get(key[0])
            if not after or after["identity"] != key[1]:
                os.close(fd)
                return False
            self.owned_pidfds[key] = fd
            return True
        except OSError as error:
            if fd is not None:
                os.close(fd)
            self.pidfd_errors[key] = "pidfd acquisition errno=" + str(error.errno)
            return False

    def alive_owned(self, key):
        import select
        fd = self.owned_pidfds.get(key)
        if fd is None:
            return None
        poller = select.poll()
        poller.register(fd, select.POLLIN)
        return not poller.poll(0)

    def signal_owned(self, key, sig):
        fd = self.owned_pidfds.get(key)
        if fd is None:
            return False
        try:
            signal.pidfd_send_signal(fd, sig, None, 0)
            return True
        except ProcessLookupError:
            return False

    def release(self, key):
        fd = self.owned_pidfds.pop(key, None)
        if fd is not None:
            os.close(fd)
        self.pidfd_errors.pop(key, None)

    def close(self):
        for fd in self.owned_pidfds.values():
            os.close(fd)
        self.owned_pidfds.clear()
        self.pidfd_errors.clear()


class WindowsProcesses:
    method = "Toolhelp32 parent tree; GetProcessTimes start identity/CPU; GetProcessMemoryInfo working set/peak"

    def __init__(self):
        from ctypes import wintypes as W
        self.W = W
        self.kernel = C.WinDLL("kernel32", use_last_error=True)
        self.psapi = C.WinDLL("psapi", use_last_error=True)
        class Entry(C.Structure):
            _fields_ = [("size", W.DWORD), ("usage", W.DWORD), ("pid", W.DWORD),
                        ("heap", C.c_size_t), ("module", W.DWORD), ("threads", W.DWORD),
                        ("parent", W.DWORD), ("priority", W.LONG), ("flags", W.DWORD), ("name", W.WCHAR * 260)]
        class Memory(C.Structure):
            _fields_ = [("cb", W.DWORD), ("faults", W.DWORD)] + [(name, C.c_size_t) for name in
                        ("peak", "rss", "quota_peak_paged", "quota_paged", "quota_peak_nonpaged", "quota_nonpaged", "pagefile", "peak_pagefile", "private_commit")]
        self.Entry, self.Memory = Entry, Memory
        self.kernel.CreateToolhelp32Snapshot.argtypes = [W.DWORD, W.DWORD]
        self.kernel.CreateToolhelp32Snapshot.restype = W.HANDLE
        self.kernel.Process32FirstW.argtypes = [W.HANDLE, C.POINTER(Entry)]
        self.kernel.Process32NextW.argtypes = [W.HANDLE, C.POINTER(Entry)]
        self.kernel.OpenProcess.argtypes = [W.DWORD, W.BOOL, W.DWORD]
        self.kernel.OpenProcess.restype = W.HANDLE
        self.kernel.GetProcessTimes.argtypes = [W.HANDLE] + [C.POINTER(W.FILETIME)] * 4
        self.kernel.CloseHandle.argtypes = [W.HANDLE]
        self.psapi.GetProcessMemoryInfo.argtypes = [W.HANDLE, C.POINTER(Memory), W.DWORD]
        self.kernel.TerminateProcess.argtypes = [W.HANDLE, W.UINT]
        self.kernel.GetCurrentProcess.restype = W.HANDLE
        self.kernel.DuplicateHandle.argtypes = [W.HANDLE, W.HANDLE, W.HANDLE, C.POINTER(W.HANDLE), W.DWORD, W.BOOL, W.DWORD]
        self.owned_handles = {}

    def get(self, pid, parent=0, name="unknown"):
        handle = self.kernel.OpenProcess(0x1000 | 0x10, False, pid)
        if not handle:
            return None
        try:
            return self.read_handle(handle, pid, parent, name)
        finally:
            self.kernel.CloseHandle(handle)

    def read_handle(self, handle, pid, parent=0, name="unknown"):
        W = self.W
        times = [W.FILETIME() for _ in range(4)]
        if not self.kernel.GetProcessTimes(handle, *[C.byref(item) for item in times]):
            return None
        values = [(item.dwHighDateTime << 32) | item.dwLowDateTime for item in times]
        memory = self.Memory()
        memory.cb = C.sizeof(memory)
        if not self.psapi.GetProcessMemoryInfo(handle, C.byref(memory), memory.cb):
            return None
        row = record(pid, parent, values[0], name, memory.rss, (values[2] + values[3]) / 10_000_000, memory.peak)
        row["has_exited"] = values[1] != 0
        row["os_total_cpu_seconds"] = row["cpu_seconds"]
        row["os_memory_breakdown"] = dict(private_commit_bytes=memory.private_commit, private_resident_bytes=None,
                                          shared_private_resident_split_available=False)
        return row

    def retain(self, key, source_handle=None):
        if source_handle is not None:
            handle = self.W.HANDLE()
            owner = self.kernel.GetCurrentProcess()
            if not self.kernel.DuplicateHandle(owner, source_handle, owner, C.byref(handle), 0, False, 2):
                return
        else:
            handle = self.kernel.OpenProcess(0x1000 | 0x10, False, key[0])
            if not handle:
                return
        row = self.read_handle(handle, key[0])
        if row and row["identity"] == key[1]:
            self.owned_handles[key] = handle
        else:
            self.kernel.CloseHandle(handle)

    def final(self, key):
        handle = self.owned_handles.pop(key, None)
        if not handle:
            return None
        try:
            return self.read_handle(handle, key[0])
        finally:
            self.kernel.CloseHandle(handle)

    def all(self):
        snapshot = self.kernel.CreateToolhelp32Snapshot(2, 0)
        if snapshot in (None, C.c_void_p(-1).value):
            raise C.WinError(C.get_last_error())
        rows = []
        try:
            entry = self.Entry()
            entry.size = C.sizeof(entry)
            valid = self.kernel.Process32FirstW(snapshot, C.byref(entry))
            while valid:
                row = self.get(entry.pid, entry.parent, entry.name)
                if row:
                    rows.append(row)
                valid = self.kernel.Process32NextW(snapshot, C.byref(entry))
        finally:
            self.kernel.CloseHandle(snapshot)
        return rows

    def terminate(self, pid, identity):
        # Verify creation time on the SAME handle used for termination.
        W = self.W
        handle = self.kernel.OpenProcess(0x1000 | 1, False, pid)
        if not handle:
            return
        try:
            times = [W.FILETIME() for _ in range(4)]
            if self.kernel.GetProcessTimes(handle, *[C.byref(item) for item in times]):
                created = (times[0].dwHighDateTime << 32) | times[0].dwLowDateTime
                if str(created) == identity:
                    self.kernel.TerminateProcess(handle, 1)
        finally:
            self.kernel.CloseHandle(handle)

    def close(self):
        for handle in self.owned_handles.values():
            self.kernel.CloseHandle(handle)
        self.owned_handles.clear()


class MacProcesses:
    method = "libproc start identity, resident_size and total CPU; OS RSS highwater unavailable"

    def __init__(self):
        self.lib = C.CDLL("/usr/lib/libproc.dylib", use_errno=True)
        class Bsd(C.Structure):
            _fields_ = [(name, C.c_uint32) for name in ("flags", "status", "xstatus", "pid", "parent", "uid", "gid", "ruid", "rgid", "svuid", "svgid", "reserved")]
            _fields_ += [("comm", C.c_char * 16), ("name", C.c_char * 32)]
            _fields_ += [(name, C.c_uint32) for name in ("nfiles", "pgid", "jobc", "tdev", "tpgid", "nice")]
            _fields_ += [("start_sec", C.c_uint64), ("start_usec", C.c_uint64)]
        class Task(C.Structure):
            _fields_ = [(name, C.c_uint64) for name in ("virtual", "resident", "user", "system", "threads_user", "threads_system")]
            _fields_ += [(name, C.c_int32) for name in ("policy", "faults", "pageins", "cow", "messages_sent", "messages_received", "syscalls_mach", "syscalls_unix", "switches", "thread_count", "running", "priority")]
        # Public Darwin rusage_info_v4 ABI from bsd/sys/resource.h:
        # https://github.com/apple/darwin-xnu/blob/main/bsd/sys/resource.h
        class Usage(C.Structure):
            _fields_ = [("uuid", C.c_uint8 * 16)] + [(name, C.c_uint64) for name in (
                "user", "system", "pkg_idle", "interrupts", "pageins", "wired", "resident", "footprint",
                "start", "exit", "child_user", "child_system", "child_pkg_idle", "child_interrupts",
                "child_pageins", "child_elapsed", "disk_read", "disk_write", "qos_default", "qos_maintenance",
                "qos_background", "qos_utility", "qos_legacy", "qos_initiated", "qos_interactive",
                "billed_system", "serviced_system", "logical_writes", "lifetime_peak_footprint",
                "instructions", "cycles", "billed_energy", "serviced_energy", "interval_peak_footprint", "runnable")]
        self.Usage = Usage
        self.lib.proc_pid_rusage.argtypes = [C.c_int, C.c_int, C.c_void_p]
        # Resident size is current RSS, not an OS lifetime highwater.
        self.Bsd, self.Task = Bsd, Task
        self.lib.proc_pidinfo.argtypes = [C.c_int, C.c_int, C.c_uint64, C.c_void_p, C.c_int]
        self.lib.proc_listallpids.argtypes = [C.c_void_p, C.c_int]
        self.method = "libproc PROC_PIDTBSDINFO/PROC_PIDTASKINFO start identity, resident_size and total CPU; OS RSS highwater unavailable"

    def get(self, pid):
        bsd, task = self.Bsd(), self.Task()
        if self.lib.proc_pidinfo(pid, 3, 0, C.byref(bsd), C.sizeof(bsd)) != C.sizeof(bsd):
            return None
        if self.lib.proc_pidinfo(pid, 4, 0, C.byref(task), C.sizeof(task)) != C.sizeof(task):
            return None
        usage = self.Usage()
        has_usage = self.lib.proc_pid_rusage(pid, 4, C.byref(usage)) == 0
        check = self.Bsd()
        if self.lib.proc_pidinfo(pid, 3, 0, C.byref(check), C.sizeof(check)) != C.sizeof(check):
            return None
        if (check.start_sec, check.start_usec) != (bsd.start_sec, bsd.start_usec):
            return None
        row = record(pid, bsd.parent, f"{bsd.start_sec}:{bsd.start_usec}",
                     bytes(bsd.name or bsd.comm).decode("utf-8", "replace"), task.resident,
                     (task.user + task.system) / 1_000_000_000)
        row["os_peak_physical_footprint_bytes"] = usage.lifetime_peak_footprint if has_usage else None
        row["os_memory_breakdown"] = dict(physical_footprint_bytes=usage.footprint if has_usage else None,
                                          private_resident_bytes=None, private_commit_bytes=None)
        return row

    def all(self):
        capacity = self.lib.proc_listallpids(None, 0) + 256
        pids = (C.c_int * capacity)()
        count = self.lib.proc_listallpids(pids, C.sizeof(pids))
        if count <= 0:
            raise RuntimeError("libproc process enumeration failed")
        return [row for pid in pids[:min(count, capacity)] for row in [self.get(pid)] if row]

    def close(self):
        pass


class Sampler:
    def __init__(self, backend, output):
        self.backend, self.output = backend, output
        self.started = time.monotonic()
        self.root = None
        self.tracked = {}
        self.rows = []
        self.error = None
        self.lock = threading.Lock()
        self.done = threading.Event()
        self.thread = threading.Thread(target=self.run, daemon=True)
        self.thread.start()

    def mark_launch(self):
        with self.lock:
            self.started = time.monotonic()
            self.output["timing"] = dict(origin="immediately_before_native_spawn", preflight_excluded=True)

    def attach(self, pid, process_handle=None, allow_unobserved=False, name=None):
        with self.lock:
            # Popen owns this exact retained Windows handle even if the child
            # already exited. No PID/name lookup substitutes for that identity.
            row = self.backend.read_handle(process_handle, pid, name=name or "unknown") if (
                isinstance(self.backend, WindowsProcesses) and process_handle is not None) else self.backend.get(pid)
            if not row:
                if allow_unobserved:
                    self.output["measurement_limitation"] = "Root exited before native resource identity/metrics could be observed; functional status JSON is still validated"
                    return False
                raise RuntimeError("Product exited or resource query failed before root identity capture")
            if name:
                row["name"] = name
            self.root = (pid, row["identity"])
            self.track(row, process_handle)
            self.sample()
            return True

    def track(self, row, process_handle=None):
        key = (row["pid"], row["identity"])
        if key not in self.tracked:
            if isinstance(self.backend, WindowsProcesses):
                self.backend.retain(key, process_handle)
            if isinstance(self.backend, LinuxProcesses):
                self.backend.retain(key)
            # Only OS executable basename, never arguments or access/application names.
            name = Path(row["name"]).name
            self.tracked[key] = dict(pid=key[0], start_identity=key[1], executable_basename=name,
                                     role="root" if key == self.root else "helper_descendant",
                                     observed_peak_rss_bytes=row["rss"], os_highwater_bytes=row["os_highwater_bytes"],
                                     os_peak_physical_footprint_bytes=row.get("os_peak_physical_footprint_bytes"),
                                     final_highwater_read_after_exit=False,
                                     first_observation_after_exit=bool(row.get("has_exited")),
                                     os_total_cpu_seconds=row.get("os_total_cpu_seconds"),
                                     termination_authority=("linux_retained_pidfd" if key in self.backend.owned_pidfds else "linux_pidfd_unavailable") if isinstance(self.backend, LinuxProcesses) else (
                                         "windows_same_handle_birth_verified" if isinstance(self.backend, WindowsProcesses) else "macos_descendant_wait_only_no_stable_signal_authority"),
                                     first_cpu_seconds=row["cpu_seconds"], last_cpu_seconds=row["cpu_seconds"],
                                     first_observed_seconds=time.monotonic() - self.started,
                                     last_observed_seconds=None)

    def sample(self):
        if self.root is None:
            return
        all_rows = self.backend.all()
        live = {(row["pid"], row["identity"]): row for row in all_rows}
        # Persist identities after reparenting. Discover children only of live owned
        # identities, never of a recycled PID. The child must not predate its parent.
        changed = True
        while changed:
            changed = False
            parents = {key[0]: live[key] for key in self.tracked if key in live}
            for row in all_rows:
                key = (row["pid"], row["identity"])
                if key not in self.tracked and row["parent"] in parents:
                    parent = parents[row["parent"]]
                    child_start = tuple(int(n) for n in row["identity"].split(":"))
                    parent_start = tuple(int(n) for n in parent["identity"].split(":"))
                    if child_start >= parent_start:
                        self.track(row)
                        changed = True
        now = time.monotonic() - self.started
        root_rss = helper_rss = 0
        per_process_memory = []
        for key, item in self.tracked.items():
            if key not in live:
                continue
            row = live[key]
            breakdown = row.get("os_memory_breakdown", {})
            if isinstance(self.backend, LinuxProcesses):
                breakdown = self.backend.private_rollup(row)
                if breakdown is None:
                    continue
            item["last_os_memory_breakdown"] = breakdown
            for metric in ("private_commit_bytes", "private_resident_bytes", "proportional_resident_bytes"):
                if breakdown.get(metric) is not None:
                    item["observed_peak_" + metric] = max(item.get("observed_peak_" + metric, 0), breakdown[metric])
            per_process_memory.append(dict(pid=key[0], start_identity=key[1], role=item["role"],
                                           rss_bytes=row["rss"], os_memory_breakdown=breakdown))
            if item["executable_basename"] == "unknown":
                item["executable_basename"] = Path(row["name"]).name
            item["observed_peak_rss_bytes"] = max(item["observed_peak_rss_bytes"], row["rss"])
            high = row["os_highwater_bytes"]
            if high is not None:
                item["os_highwater_bytes"] = max(item["os_highwater_bytes"] or 0, high)
            item["last_cpu_seconds"] = row["cpu_seconds"]
            footprint_peak = row.get("os_peak_physical_footprint_bytes")
            if footprint_peak is not None:
                item["os_peak_physical_footprint_bytes"] = max(item["os_peak_physical_footprint_bytes"] or 0, footprint_peak)
            item["last_observed_seconds"] = now
            if key == self.root:
                root_rss += row["rss"]
            else:
                helper_rss += row["rss"]
        self.rows.append(dict(seconds=now, root_rss_bytes=root_rss,
                              helper_rss_bytes=helper_rss, product_rss_bytes=root_rss + helper_rss,
                              per_process_memory=per_process_memory,
                              product_cpu_seconds=sum(v["last_cpu_seconds"] - v["first_cpu_seconds"] for v in self.tracked.values())))

    def run(self):
        deadline = time.monotonic()
        try:
            while not self.done.is_set():
                with self.lock:
                    self.sample()
                deadline += INTERVAL
                self.done.wait(max(0, deadline - time.monotonic()))
        except Exception as error:
            self.error = type(error).__name__ + ": " + str(error)

    def finish(self):
        self.done.set()
        self.thread.join(timeout=10)
        if self.thread.is_alive():
            self.error = "OS sampler did not stop within 10 seconds"
        rows = self.rows
        sustained = [row for row in rows if self.output.get("sustained_start_seconds", float("inf")) <= row["seconds"] <= self.output.get("sustained_end_seconds", -1)]
        gaps = [b["seconds"] - a["seconds"] for a, b in zip(rows, rows[1:])]
        peak = max((row["product_rss_bytes"] for row in rows), default=0)
        stable = [row["product_rss_bytes"] for row in sustained]
        process_rows = []
        for key, item in self.tracked.items():
            if isinstance(self.backend, WindowsProcesses):
                final = self.backend.final(key)
                if final and final["identity"] == key[1]:
                    if final["os_highwater_bytes"] is not None:
                        item["os_highwater_bytes"] = max(item["os_highwater_bytes"] or 0, final["os_highwater_bytes"])
                    item["last_cpu_seconds"] = max(item["last_cpu_seconds"], final["cpu_seconds"])
                    item["final_highwater_read_after_exit"] = final["has_exited"]
                    item["os_total_cpu_seconds"] = final["os_total_cpu_seconds"]
            row = dict(item)
            row["cpu_delta_seconds"] = row.pop("last_cpu_seconds") - row.pop("first_cpu_seconds")
            if row["first_observation_after_exit"]:
                row["cpu_delta_seconds"] = None
            process_rows.append(row)
            if isinstance(self.backend, LinuxProcesses):
                self.backend.release(key)
        observed_product_samples = any(row["product_rss_bytes"] > 0 for row in rows)
        if not observed_product_samples:
            peak = None
        highwater_values = [row["os_highwater_bytes"] for row in process_rows if row["os_highwater_bytes"] is not None]
        known_peak_lower_bound = max(peak or 0, max(highwater_values, default=0)) if (
            peak is not None or highwater_values) else None
        self.output["measurement_scope"] = "sampled_product_tree" if observed_product_samples else (
            "root_or_observed_descendant_highwater_only" if highwater_values else "unobserved_not_proven")
        self.output.update(samples=rows, sample_count=len(rows), sustained_sample_count=len(sustained),
                           sampling=dict(requested_interval_seconds=INTERVAL, maximum_observed_gap_seconds=max(gaps, default=None),
                                         median_observed_gap_seconds=statistics.median(gaps) if gaps else None,
                                         gaps_over_50ms=sum(gap > .05 for gap in gaps), method=self.backend.method,
                                         observed_50ms_requirement_met=bool(gaps) and max(gaps) <= .05,
                                         error=self.error), processes=process_rows,
                           memory=dict(aggregate_concurrent_observed_peak_bytes=peak,
                                       root_observed_peak_bytes=max((r["root_rss_bytes"] for r in rows), default=0) if observed_product_samples else None,
                                       helper_concurrent_observed_peak_bytes=max((r["helper_rss_bytes"] for r in rows), default=0),
                                       sustained_rss_median_bytes=statistics.median(stable) if stable else None,
                                       sustained_rss_p95_bytes=sorted(stable)[int((len(stable) - 1) * .95)] if stable else None,
                                       sustained_rss_min_bytes=min(stable, default=None), sustained_rss_max_bytes=max(stable, default=None)),
                           cpu_delta_seconds=sum(row["cpu_delta_seconds"] or 0 for row in process_rows) if any(
                               row["cpu_delta_seconds"] is not None for row in process_rows) else None,
                           sustained_cpu_delta_seconds=(sustained[-1]["product_cpu_seconds"] - sustained[0]["product_cpu_seconds"]) if len(sustained) > 1 else None,
                           budget=dict(target_bytes=BUDGET, policy="soft_optimization_target",
                                       acceptable_reference_bytes=30_000_000, reference_is_hard_cutoff=False,
                                       known_peak_lower_bound_bytes=known_peak_lower_bound,
                                       target_met=False if known_peak_lower_bound is not None and known_peak_lower_bound > BUDGET else (
                                           True if observed_product_samples and not self.error else None),
                                       qualification="unmet" if known_peak_lower_bound is not None and known_peak_lower_bound > BUDGET else (
                                           "observed_target_met_not_proof_of_unobserved_peaks" if observed_product_samples and not self.error else "not_proven")))

    def cleanup_descendants(self):
        failed = []
        methods = set()
        for key in reversed(list(self.tracked)):
            if key == self.root:
                continue
            if isinstance(self.backend, LinuxProcesses):
                methods.add("retained_birth_verified_pidfd_signals")
                alive = self.backend.alive_owned(key)
                if alive is None:
                    current = self.backend.get(key[0])
                    if current and current["identity"] == key[1] and not current.get("has_exited"):
                        failed.append(dict(pid=key[0], start_identity=key[1], reason="No stable pidfd authority; refusing numeric PID signal"))
                    continue
                if not alive:
                    continue
                self.backend.signal_owned(key, signal.SIGTERM)
                deadline = time.monotonic() + 2
                while self.backend.alive_owned(key) and time.monotonic() < deadline:
                    time.sleep(.02)
                if self.backend.alive_owned(key):
                    self.backend.signal_owned(key, signal.SIGKILL)
                    deadline = time.monotonic() + 2
                    while self.backend.alive_owned(key) and time.monotonic() < deadline:
                        time.sleep(.02)
                if self.backend.alive_owned(key):
                    failed.append(dict(pid=key[0], start_identity=key[1], reason="Owned pidfd remains alive after bounded shutdown"))
            else:
                current = self.backend.get(key[0])
                if not current or current["identity"] != key[1] or current.get("has_exited"):
                    continue
                if isinstance(self.backend, WindowsProcesses):
                    methods.add("same_handle_birth_verified_windows_termination")
                    self.backend.terminate(*key)
                else:
                    # No unprivileged stable macOS descendant signal authority
                    # is retained. Parent shutdown closes helper pipes; wait for
                    # actual EOF/normal exit, never signal a numeric descendant PID.
                    methods.add("macos_parent_shutdown_eof_wait_only")
                deadline = time.monotonic() + 2
                while time.monotonic() < deadline:
                    current = self.backend.get(key[0])
                    if not current or current["identity"] != key[1] or current.get("has_exited"):
                        break
                    time.sleep(.02)
                else:
                    failed.append(dict(pid=key[0], start_identity=key[1], reason="Helper still alive; no unsafe numeric PID signal fallback"))
        self.output["descendant_cleanup"] = dict(methods=sorted(methods), failed=failed)
        if failed:
            raise RuntimeError("Owned helper cleanup incomplete; see descendant_cleanup evidence; refusing unsafe PID signals")


def collectors(document):
    assert isinstance(document, dict) and isinstance(document.get("collectors"), list) and document["collectors"], "Missing status collectors"
    result = []
    for row in document["collectors"]:
        assert row["state"] in ("healthy", "degraded", "unavailable"), "Unknown collector state"
        if row["state"] != "healthy":
            assert row.get("detail"), "Nonhealthy collector must explicitly explain degradation"
        result.append(dict(collector=row["collector"], state=row["state"], explicit_detail=bool(row.get("detail"))))
    return result


class Reader:
    def __init__(self, stream, started, json_lines=False):
        self.first = None
        self.valid = 0
        self.invalid = False
        self.collector_states = []
        self.data = bytearray()
        self.error = None
        def drain():
            pending = bytearray()
            try:
                while True:
                    chunk = stream.read1(65536)
                    if not chunk:
                        break
                    if self.first is None:
                        self.first = time.monotonic() - started
                    if json_lines:
                        pending.extend(chunk)
                        while b"\n" in pending:
                            line, _, remainder = pending.partition(b"\n")
                            pending = bytearray(remainder)
                            if line.strip():
                                self.parse(line)
                        if len(pending) > 16_777_216:
                            raise RuntimeError("Watch emitted >16MiB unterminated JSON record")
                    elif len(self.data) + len(chunk) <= 64_000_000:
                        self.data.extend(chunk)
                    else:
                        raise RuntimeError("Status exceeded qualification transport limit 64MB")
                if json_lines and pending.strip():
                    self.parse(pending)
            except Exception as error:
                self.error = type(error).__name__
        self.thread = threading.Thread(target=drain, daemon=True)
        self.thread.start()

    def parse(self, line):
        try:
            document = json.loads(line)
            assert isinstance(document, dict)
            if document.get("collectors"):
                self.collector_states = collectors(document)
            self.valid += 1
        except (ValueError, AssertionError, KeyError, TypeError):
            self.invalid = True

    def finish(self):
        self.thread.join(timeout=10)
        assert not self.thread.is_alive(), "Product output pipe did not close"
        assert not self.error, "Product output reader failed: " + str(self.error)


def launch(binary, args, before_spawn):
    command = [str(binary), *args]
    before_spawn()
    return subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                            start_new_session=os.name != "nt")


def stop_owned(process, sampler):
    if process.poll() is None:
        if os.name == "nt":
            sampler.backend.terminate(*sampler.root)
        else:
            # poll() above returned None: this Popen child is still unreaped and
            # no other thread calls poll/wait on it. Do NOT poll/wait between this
            # authority check and the signal: its PID cannot be reused meanwhile.
            if not isinstance(sampler.backend, LinuxProcesses) or not sampler.backend.signal_owned(sampler.root, signal.SIGINT):
                os.kill(process.pid, signal.SIGINT)
        try:
            process.wait(timeout=15)
        except subprocess.TimeoutExpired:
            # Popen's retained Windows handle / unreaped Unix child owns this PID.
            process.kill()
            process.wait(timeout=15)
    sampler.cleanup_descendants()


def windows_tray_window(pid=None, close=False):
    from ctypes import wintypes as W
    user = C.WinDLL("user32", use_last_error=True)
    user.FindWindowW.argtypes = [W.LPCWSTR, W.LPCWSTR]
    user.FindWindowW.restype = W.HWND
    user.GetWindowThreadProcessId.argtypes = [W.HWND, C.POINTER(W.DWORD)]
    user.PostMessageW.argtypes = [W.HWND, W.UINT, W.WPARAM, W.LPARAM]
    hwnd = user.FindWindowW("MicCamWatchTrayClass", None)
    owner = W.DWORD()
    if hwnd:
        user.GetWindowThreadProcessId(hwnd, C.byref(owner))
    if pid is not None and owner.value != pid:
        return False
    if close and hwnd:
        if not user.PostMessageW(hwnd, 0x10, 0, 0):
            raise C.WinError(C.get_last_error())
    return bool(hwnd)


def sustain(output, sampler, alive, pump=None):
    warm = time.monotonic() + WARMUP
    while time.monotonic() < warm:
        assert alive(), "Product exited during warmup"
        if pump:
            pump(False)
        else:
            time.sleep(.02)
    output["sustained_start_seconds"] = time.monotonic() - sampler.started
    deadline = time.monotonic() + DURATION
    while time.monotonic() < deadline:
        assert alive(), "Product exited during sustained qualification"
        assert not sampler.error, "OS sampler failed: " + str(sampler.error)
        if pump:
            pump(True)
        else:
            time.sleep(.02)
    output["sustained_end_seconds"] = time.monotonic() - sampler.started
    output["sustained_duration_seconds"] = output["sustained_end_seconds"] - output["sustained_start_seconds"]


def measure(binary, backend, mode, output):
    sampler = Sampler(backend, output)
    process = terminal = reader = None
    output.update(mode=mode, functional="running", latency=dict(first_data_seconds=None, ready_seconds=None, exit_seconds=None),
                  ready_definition=None, warmup_seconds=WARMUP if mode != "status" else None,
                  requested_sustained_seconds=DURATION if mode != "status" else None)
    try:
        if mode == "top":
            terminal = NativeTerminal(binary, "en", 120, 40, launch_started=sampler.mark_launch)
            process = terminal.process
            sampler.attach(process.pid)
            deadline = time.monotonic() + 45
            while time.monotonic() < deadline:
                terminal.drain(.02)
                if terminal.raw and output["latency"]["first_data_seconds"] is None:
                    output["latency"]["first_data_seconds"] = time.monotonic() - sampler.started
                assert terminal.alive(), "Native top exited before ready"
                if "[q]" in "\n".join(terminal.display).lower():
                    break
            else:
                raise AssertionError("Native top never rendered a Quit control")
            output["latency"]["ready_seconds"] = time.monotonic() - sampler.started
            output["ready_definition"] = "native PTY/ConPTY rendered Quit control; no screen content saved"
            last_burst = 0
            def pump(active):
                nonlocal last_burst
                now = time.monotonic()
                if active and output["profile"] == "controlled_refresh_burst_slow_consumer":
                    if now - last_burst >= 1:
                        terminal.send("R" * 8)
                        terminal.resize(80 if int(now) % 2 else 120, 30 if int(now) % 2 else 40)
                        last_burst = now
                    # Delay consuming the real native terminal; OS sampler remains
                    # independent. No kill/mute/camera/notification keys are sent.
                    time.sleep(.20)
                terminal.drain(.02)
                terminal.raw.clear()  # Never retain a user's terminal/activity data.
            sustain(output, sampler, terminal.alive, pump)
            terminal.send("Q")
            deadline = time.monotonic() + 20
            while terminal.alive() and time.monotonic() < deadline:
                terminal.drain(.02)
                terminal.raw.clear()
            assert not terminal.alive(), "Native top Quit did not exit"
            code = process.exitstatus if os.name == "nt" else process.wait(timeout=2)
            assert code == 0, "Native top exited unsuccessfully"
            output["exit_code"] = code
        else:
            if mode == "tray":
                if os.name == "nt":
                    assert not windows_tray_window(), "Existing user tray detected; refusing to touch it"
                else:
                    before = subprocess.run([str(binary), "tray", "status"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=15)
                    assert before.returncode == 1, "Isolated tray namespace already occupied"
            args = {"status": ["status", "--json"], "watch": ["watch", "--json", "--no-kill", "--interval", "200"], "tray": ["tray", "run"]}[mode]
            process = launch(binary, args, sampler.mark_launch)
            reader = Reader(process.stdout, sampler.started, json_lines=mode == "watch")
            try:
                sampler.attach(process.pid, process_handle=getattr(process, "_handle", None),
                               allow_unobserved=mode == "status", name=Path(binary).name)
            except Exception as error:
                if mode != "status":
                    raise
                output["measurement_limitation"] = "Status resource observation failed: " + type(error).__name__
            if mode == "status":
                output["exit_code"] = process.wait(timeout=60)
                output["latency"]["exit_seconds"] = time.monotonic() - sampler.started
                reader.finish()
                document = json.loads(reader.data)
                output["collector_states"] = collectors(document)
                assert process.returncode in (0, 1, 2), "Status process failed"
                # A nonzero result needs explicit collector degradation, not just
                # syntactically valid JSON masking an operational failure.
                assert process.returncode == 0 or any(row["state"] != "healthy" for row in output["collector_states"]), "Status failed without explicit degradation"
                output["latency"]["ready_seconds"] = time.monotonic() - sampler.started
                output["ready_definition"] = "complete validated status JSON document; explicit degraded collectors accepted"
                reader.data.clear()
            elif mode == "watch":
                output["ready_definition"] = "no ready protocol; survival is not first scan or physical event readiness"
                sustain(output, sampler, lambda: process.poll() is None)
                stop_owned(process, sampler)
                output["latency"]["exit_seconds"] = time.monotonic() - sampler.started
                reader.finish()
                assert not reader.invalid, "Watch emitted invalid JSON document"
                output.update(valid_json_documents=reader.valid, collector_states=reader.collector_states or output.get("collector_states", []),
                              exit_code=process.returncode, stop_method="owned PID SIGINT" if os.name != "nt" else "owned retained process identity termination; no watch graceful-stop API on Windows")
                if os.name != "nt":
                    assert process.returncode == 0, "Watch failed graceful owned shutdown"
            else:
                deadline = time.monotonic() + 45
                while time.monotonic() < deadline:
                    assert process.poll() is None, "Native tray exited before ready"
                    if os.name == "nt":
                        ready = windows_tray_window(process.pid)
                    else:
                        ready = subprocess.run([str(binary), "tray", "status"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=15).returncode == 0
                    if ready:
                        break
                    time.sleep(.10)
                else:
                    raise AssertionError("Native tray failed readiness; disposable native desktop required")
                output["latency"]["ready_seconds"] = time.monotonic() - sampler.started
                output["ready_definition"] = "owned native window class/PID" if os.name == "nt" else "isolated native tray control socket status"
                sustain(output, sampler, lambda: process.poll() is None)
                if os.name == "nt":
                    assert windows_tray_window(process.pid, close=True), "Owned tray window disappeared"
                else:
                    assert subprocess.run([str(binary), "tray", "stop"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=20).returncode == 0, "Isolated tray stop failed"
                output["exit_code"] = process.wait(timeout=20)
                output["latency"]["exit_seconds"] = time.monotonic() - sampler.started
                assert process.returncode == 0, "Native tray did not exit cleanly"
                if os.name == "nt":
                    assert not windows_tray_window(process.pid), "Owned tray window remained"
                else:
                    assert subprocess.run([str(binary), "tray", "status"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=15).returncode == 1, "Isolated tray control remained"
                reader.finish()
            output["latency"]["first_data_seconds"] = reader.first
        if output["latency"]["exit_seconds"] is None:
            output["latency"]["exit_seconds"] = time.monotonic() - sampler.started
        output["functional"] = "passed"
    except Exception as error:
        output["functional"] = "failed"
        # Error types and our controlled assertions only; never product stderr or
        # JSON parsing excerpts containing private accesses/application names.
        output["error"] = str(error) if isinstance(error, (AssertionError, subprocess.TimeoutExpired)) else type(error).__name__
        raise
    finally:
        try:
            if terminal is not None:
                terminal.close()
                sampler.cleanup_descendants()
            elif process is not None:
                if sampler.root is None:
                    if process.poll() is None:
                        process.kill()
                        process.wait(timeout=15)
                else:
                    stop_owned(process, sampler)
        except Exception as error:
            output["functional"] = "failed"
            output["cleanup_error"] = type(error).__name__
            raise
        finally:
            if reader is not None:
                reader.data.clear()
            sampler.finish()
        if sampler.error and output["functional"] == "passed" and mode != "status":
            output["functional"] = "failed"
            raise RuntimeError("Sampler failed: " + sampler.error)
        if mode != "status" and output["functional"] == "passed" and (not sampler.rows or not any(r["product_rss_bytes"] > 0 for r in sampler.rows)):
            output["functional"] = "failed"
            raise RuntimeError("No real product RSS samples")


def fetch_baseline(root, key, metadata):
    extension = "zip" if key.startswith("windows") else "tar.gz"
    asset = f"miccamwatch-{key}.{extension}"
    url = f"https://github.com/Roman-Cuisset/miccamwatch/releases/download/{BASELINE_VERSION}/{asset}"
    archive = root / asset
    metadata.update(asset=asset, url=url, expected_archive_sha256=HASHES[key], source_commit=BASELINE_SOURCE)
    with urllib.request.urlopen(url, timeout=90) as response, archive.open("wb") as stream:
        while chunk := response.read(1024 * 1024):
            stream.write(chunk)
    actual = sha256(archive)
    metadata["observed_archive_sha256"] = actual
    assert actual == HASHES[key], "Public immutable baseline archive hash mismatch"
    extracted = root / "baseline"
    extracted.mkdir()
    names = {"mcw", "mcw.exe", "mcw-tray.exe", "mcw-camera-helper"}
    # Never extract paths, links or archive executables beyond this allowlist.
    if extension == "zip":
        with zipfile.ZipFile(archive) as package:
            for entry in package.infolist():
                if entry.filename in names and not entry.is_dir():
                    (extracted / entry.filename).write_bytes(package.read(entry))
    else:
        with tarfile.open(archive, "r:gz") as package:
            for entry in package.getmembers():
                name = entry.name.removeprefix("./")
                if name in names and entry.isfile():
                    stream = package.extractfile(entry)
                    with stream:
                        (extracted / name).write_bytes(stream.read())
                    (extracted / name).chmod(0o700)
    binary = extracted / ("mcw.exe" if os.name == "nt" else "mcw")
    assert binary.is_file(), "Verified baseline archive lacks native CLI"
    return binary


def isolate(root):
    home = root / "home"
    home.mkdir()
    os.environ.update(HOME=str(home), USERPROFILE=str(home), APPDATA=str(home / "appdata"),
                      LOCALAPPDATA=str(home / "localappdata"), XDG_CONFIG_HOME=str(home / "config"),
                      XDG_DATA_HOME=str(home / "data"), XDG_STATE_HOME=str(home / "state"), TERM="xterm-256color")
    # Preserve the EXISTING real isolated PipeWire runtime, bus/display and sound
    # server. Tray runtime lives under isolated XDG_DATA_HOME. Use a short
    # path for macOS native AF_UNIX; preserve shared session runtime sockets.
    settings = home / ("appdata/MicCamWatch" if os.name == "nt" else "config/MicCamWatch")
    settings.mkdir(parents=True)
    (settings / "settings.toml").write_text(
        "notifications_enabled = false\nsound_enabled = false\nshow_ready = false\nhistory_enabled = false\n"
        "mute_on_lock = false\nblock_camera_on_lock = false\nrestore_on_unlock = false\n", encoding="utf-8")
    return home


def qualify(binary, backend, metadata, report, proof, scenario, desktop):
    binary = binary.resolve()
    metadata.update(binary_sha256=sha256(binary), version=subprocess.check_output([str(binary), "--version"], timeout=30, text=True).strip())
    siblings = [binary.with_name(name) for name in ("mcw-tray.exe", "mcw-camera-helper")]
    metadata["companion_sha256"] = {path.name: sha256(path) for path in siblings if path.is_file()}
    # Query authoritative settings path only under isolated environment. Require
    # our safe preferences to be at the path actually used by this real binary.
    settings_path = Path(subprocess.check_output([str(binary), "config", "settings-path"], timeout=30, text=True).strip())
    expected = Path(os.environ["APPDATA"] if os.name == "nt" else os.environ["XDG_CONFIG_HOME"]) / "MicCamWatch/settings.toml"
    assert settings_path.resolve() == expected.resolve(), "Product settings path escaped isolated qualification home"
    assert settings_path.is_file(), "Isolated safety preferences are not at product settings path"
    metadata["safe_preferences_confirmed"] = True
    specs = [("status", "idle", "first_invocation_not_os_cache_flush"),
             ("status", "idle", "warm_process_new_spawn"), ("status", "idle", "warm_process_new_spawn"),
             ("watch", "idle", "warm_process_new_spawn"), ("top", "idle", "warm_process_new_spawn"),
             ("top", "controlled_refresh_burst_slow_consumer", "warm_process_new_spawn")]
    if desktop:
        specs.append(("tray", "idle", "warm_process_new_spawn"))
    for mode, profile, cache in specs:
        row = dict(product=metadata["label"], profile=profile, cache_state=cache, graph_scenario=scenario,
                   collector_states=next((r["collector_states"] for r in reversed(report["runs"])
                                          if r["product"] == metadata["label"] and r.get("collector_states")), []))
        report["runs"].append(row)
        try:
            measure(binary, backend, mode, row)
        finally:
            save(proof, report)


def save(proof, report):
    report["functional_success"] = bool(report["runs"]) and all(row.get("functional") == "passed" for row in report["runs"]) and not report.get("operational_error")
    report["budget_qualification"] = "unmet" if any(row.get("budget", {}).get("qualification") == "unmet" for row in report["runs"]) else "not_proven"
    temporary = proof / "resources.json.tmp"
    temporary.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    temporary.replace(proof / "resources.json")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, help="Candidate native CLI; omit to qualify public v0.16.1 alone before compilation")
    parser.add_argument("--proof", type=Path, required=True)
    parser.add_argument("--runner-desktop", action="store_true", help="Opt into tray on a disposable native runner desktop ONLY")
    parser.add_argument("--scenario", default="local_read_only_native_collectors_no_physical_transition", help="Controlled collector/graph scenario label, not user activity")
    args = parser.parse_args()
    args.proof = args.proof.resolve()
    args.proof.mkdir(parents=True, exist_ok=True)
    system = {"Linux": "linux", "Darwin": "macos", "Windows": "windows"}[platform.system()]
    arch = {"AMD64": "x86_64", "arm64": "aarch64"}.get(platform.machine(), platform.machine())
    key = f"{system}-{arch}"
    report = dict(schema_version=1, host=dict(os=system, os_version=platform.platform(), arch=arch,
                                            python_version=platform.python_version()), baseline=dict(label="baseline"), candidate=None,
                  runs=[], requested_modes=["status", "watch", "top"] + (["tray"] if args.runner_desktop else []),
                  privacy=dict(raw_status_events_terminal_clipboard_published=False, hardware_actions=False,
                               notifications_sound_history_lock_policy=False, autostart_enabled=False),
                  excluded_processes=["qualification Python sampler/PTY server", "desktop/window/session servers", "fixture PipeWire/pulse servers and virtual capture clients", "tray readiness/stop control probes"],
                  ram_policy=dict(target_bytes=BUDGET, policy="soft_optimization_target", acceptable_reference_bytes=30_000_000,
                                  reference_is_hard_cutoff=False, above_target_fails_functional_suite=False),
                  metric_limits=["All observed product descendants included; lifetime identity persists after reparenting. A helper born and exited entirely between samples can be missed.",
                                 "Concurrent peak sums RSS at each observation, never independent per-PID highwaters. OS per-process highwaters reported separately where available.",
                                 "RSS is platform-native working set/resident pages; shared mappings can be double counted across processes; no PSS equivalence asserted.",
                                 "Windows PROCESS_MEMORY_COUNTERS_EX.PrivateUsage is PRIVATE COMMIT, not private RSS; shared/private working-set split is unavailable in this sampler. Linux reports RssAnon/RssFile/RssShmem and own-process smaps_rollup Private_Clean+Private_Dirty/Pss when accessible; neither substitutes for RSS budget. macOS physical footprint is a separate ledger metric.",
                                 "Requested 25ms sampling; actual gaps and >50ms gaps reported honestly. Scheduler stalls weaken qualification and are not silently discarded.",
                                 "No OS cache purge: first measured status is after version/settings probes, cold process only, not machine-cold. New process warm runs follow in fixed baseline-then-candidate order.",
                                 "CPU excludes time before first observation; Windows retains owned handles and reads final CPU/highwater after exit. Other platforms may miss final CPU. Sampler overhead excluded but perturbs scheduling.",
                                 "Status latency validates complete JSON, accepts explicit collector degradation; watch has no ready handshake and idle may emit zero documents.",
                                 "Top first-data means terminal bytes; ready means rendered Quit control. Tray ready is native window/control protocol, not icon painting or device readiness.",
                                 "No physical-device/event latency, sound/notification delivery, hardware capture, permissions, clipboard, lock transition or autostart qualification.",
                                 "Controlled burst is 8 read-only refresh keys/second, alternating native terminal resize, and 200ms output-consumer delay; not unbounded input or fabricated data limits.",
                                 "macOS OS RSS highwater unavailable via proc_pidinfo; proc_pid_rusage v4 OS lifetime peak physical footprint reported separately, never mislabeled RSS. Intrinsic AppKit/native helper costs included without claiming 15MB passes."])
    backend = None
    original_environment = os.environ.copy()
    try:
        assert key in HASHES, "No independently pinned baseline for this native OS/architecture"
        if args.runner_desktop:
            assert os.environ.get("GITHUB_ACTIONS") == "true" and os.environ.get("RUNNER_ENVIRONMENT") == "github-hosted", "Tray mode requires disposable GitHub-hosted native runner, never a user desktop"
            if system == "linux":
                assert os.environ.get("DISPLAY") and os.environ.get("DBUS_SESSION_BUS_ADDRESS") and os.environ.get("PIPEWIRE_RUNTIME_DIR"), "Linux needs existing isolated real PipeWire/XFCE session"
            elif system == "macos":
                assert subprocess.run(["launchctl", "print", "gui/" + str(os.getuid())], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0, "macOS requires logged-in Aqua/WindowServer session"
            else:
                from ctypes import wintypes as W
                user = C.WinDLL("user32", use_last_error=True)
                user.OpenInputDesktop.argtypes = [W.DWORD, W.BOOL, W.DWORD]
                user.OpenInputDesktop.restype = W.HANDLE
                user.CloseDesktop.argtypes = [W.HANDLE]
                desktop = user.OpenInputDesktop(0, False, 1)
                assert desktop, "Windows requires accessible interactive native desktop"
                user.CloseDesktop(desktop)
                assert not windows_tray_window(), "Existing user tray detected; refusing desktop qualification"
        backend = {"linux": LinuxProcesses, "windows": WindowsProcesses, "macos": MacProcesses}[system]()
        # Short path prevents AF_UNIX path exhaustion, especially on macOS runners.
        with tempfile.TemporaryDirectory(prefix="mcwr-", dir=None if os.name == "nt" else "/tmp") as temporary:
            root = Path(temporary)
            baseline = fetch_baseline(root, key, report["baseline"])
            isolate(root)
            qualify(baseline, backend, report["baseline"], report, args.proof, args.scenario, args.runner_desktop)
            assert report["baseline"]["version"] == "mcw 0.16.1", "Pinned public baseline reports unexpected version"
            if args.binary:
                report["candidate"] = dict(label="candidate")
                qualify(args.binary, backend, report["candidate"], report, args.proof, args.scenario, args.runner_desktop)
        report["completed"] = True
    except Exception as error:
        report["completed"] = False
        report["operational_error"] = str(error) if isinstance(error, AssertionError) else type(error).__name__
    finally:
        os.environ.clear()
        os.environ.update(original_environment)
        if backend:
            backend.close()
        save(args.proof, report)
    print(json.dumps(dict(artifact=str(args.proof / "resources.json"), functional_success=report["functional_success"],
                         completed=report.get("completed", False), budget_qualification=report["budget_qualification"])))
    return 0 if report.get("completed") and report["functional_success"] else 1


if __name__ == "__main__":
    sys.exit(main())
