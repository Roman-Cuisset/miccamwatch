#!/usr/bin/env python3
"""Disposable macOS stop diagnosis and real consumer lifecycle regression.

Baseline is immutable public v0.19.1, not rebuilt source or latest. Stop clients
and candidate reopen clients receive DYLD_INSERT_LIBRARIES; GUI/status do not.
First failure ends the loop, remains failed, and is never retried as a proof.
No capture, protection, autostart, clipboard, TCC, SIP or Gatekeeper changes.
"""
import argparse
import array
import ctypes as C
import hashlib
import json
import os
from pathlib import Path
import platform
import select
import shutil
import socket
import stat
import subprocess
import sys
import tarfile
import tempfile
import time
import types
import urllib.request

BASELINE = "v0.19.1"
SOURCE = "3dd70fc6e659187f6f4b9f74c9b26773257291b9"
HASHES = {
    "x86_64": "d042af9adf8dbe7f28912277e3ddf31b799ecf16fedacb92fa7fa06712ede29d",
    "aarch64": "ad9719c28d4c576e507bcb82a62ad923e3093414b94fd1107b1683be9e9f0c01",
}
RELEASE = "https://github.com/Roman-Cuisset/miccamwatch/releases/download/" + BASELINE
SAFE = ("notifications_enabled = false\nsound_enabled = false\nshow_ready = false\n"
        "history_enabled = false\nprofile = \"development\"\nmute_on_lock = false\n"
        "block_camera_on_lock = false\nrestore_on_unlock = false\n").encode()


def require(value, message):
    if not value:
        raise AssertionError(message)


def sha(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(65536), b""):
            digest.update(chunk)
    return digest.hexdigest()


def download(url, path):
    with urllib.request.urlopen(url, timeout=90) as response, path.open("xb") as stream:
        shutil.copyfileobj(response, stream)


def baseline(work, report):
    arch = {"arm64": "aarch64"}.get(platform.machine(), platform.machine())
    require(arch in HASHES, "Unsupported macOS architecture")
    asset = "miccamwatch-macos-" + arch + ".tar.gz"
    archive = work / asset
    manifest = work / "SHA256SUMS"
    report["provenance"] = dict(tag=BASELINE, source_commit=SOURCE, asset=asset,
                                url=RELEASE + "/" + asset, expected_archive_sha256=HASHES[arch])
    download(RELEASE + "/SHA256SUMS", manifest)
    matches = [line.split()[0].lower() for line in manifest.read_text().splitlines()
               if len(line.split()) == 2 and line.split()[1].lstrip("*") == asset]
    require(matches == [HASHES[arch]], "Published manifest does not match pinned public archive")
    download(RELEASE + "/" + asset, archive)
    report["provenance"]["observed_archive_sha256"] = sha(archive)
    require(sha(archive) == HASHES[arch], "Immutable public archive SHA-256 mismatch")
    binary = work / "mcw"
    with tarfile.open(archive, "r:gz") as package:
        members = [entry for entry in package.getmembers() if entry.name.removeprefix("./") == "mcw"]
        require(len(members) == 1 and members[0].isfile() and members[0].size <= 100_000_000,
                "Public archive must contain exactly one regular bounded CLI")
        with package.extractfile(members[0]) as source, binary.open("xb") as target:
            shutil.copyfileobj(source, target)
    binary.chmod(0o700)
    return binary


class Processes:
    """Darwin birth identity + exact executable + retained kqueue exit filters.

    No process is signalled, by PID or otherwise. Owned root is a direct Popen
    child; peers are discovered only through its live descendant relationship.
    """
    class Bsd(C.Structure):
        _fields_ = [(name, C.c_uint32) for name in (
            "flags", "status", "xstatus", "pid", "parent", "uid", "gid", "ruid", "rgid", "svuid", "svgid", "reserved")]
        _fields_ += [("comm", C.c_char * 16), ("name", C.c_char * 32)]
        _fields_ += [(name, C.c_uint32) for name in ("nfiles", "pgid", "jobc", "tdev", "tpgid", "nice")]
        _fields_ += [("start_sec", C.c_uint64), ("start_usec", C.c_uint64)]

    def __init__(self):
        self.lib = C.CDLL("/usr/lib/libproc.dylib", use_errno=True)
        self.lib.proc_pidinfo.argtypes = [C.c_int, C.c_int, C.c_uint64, C.c_void_p, C.c_int]
        self.lib.proc_pidpath.argtypes = [C.c_int, C.c_void_p, C.c_uint32]
        self.queue = select.kqueue()
        self.owned = {}

    def get(self, pid):
        info, check = self.Bsd(), self.Bsd()
        path = C.create_string_buffer(4096)
        if self.lib.proc_pidinfo(pid, 3, 0, C.byref(info), C.sizeof(info)) != C.sizeof(info):
            return None
        if self.lib.proc_pidpath(pid, path, len(path)) <= 0:
            return None
        if self.lib.proc_pidinfo(pid, 3, 0, C.byref(check), C.sizeof(check)) != C.sizeof(check):
            return None
        if (info.start_sec, info.start_usec) != (check.start_sec, check.start_usec):
            return None
        return dict(pid=pid, parent=info.parent, uid=info.uid,
                    birth=[info.start_sec, info.start_usec], executable=os.fsdecode(path.value))

    def retain(self, row):
        require(row["uid"] == os.getuid(), "Owned process has unexpected UID")
        if row["pid"] in self.owned:
            require(self.owned[row["pid"]]["birth"] == row["birth"], "Owned PID was reused")
            return
        self.queue.control([select.kevent(row["pid"], filter=select.KQ_FILTER_PROC,
                            flags=select.KQ_EV_ADD | select.KQ_EV_ENABLE | select.KQ_EV_ONESHOT,
                            fflags=select.KQ_NOTE_EXIT)], 0, 0)
        require(self.get(row["pid"]) == row, "Process changed during exit-filter registration")
        self.owned[row["pid"]] = dict(row, exit_observed=False)

    def discover(self, root):
        listing = subprocess.check_output(["/bin/ps", "-axo", "pid=,ppid="], text=True)
        rows = [tuple(map(int, line.split())) for line in listing.splitlines()]
        pending = {root}
        while pending:
            parents, pending = pending, set()
            for pid, parent in rows:
                if parent in parents and pid not in self.owned:
                    row = self.get(pid)
                    if row is not None and row["parent"] == parent:
                        self.retain(row)
                        pending.add(pid)

    def drain(self):
        for event in self.queue.control(None, max(1, len(self.owned)), 0):
            require(not event.flags & select.KQ_EV_ERROR, "Owned process exit filter failed")
            if event.fflags & select.KQ_NOTE_EXIT and event.ident in self.owned:
                self.owned[event.ident]["exit_observed"] = True

    def exited(self):
        self.drain()
        return all(row["exit_observed"] for row in self.owned.values())

    def close(self):
        self.queue.close()


def metadata(path):
    info = path.lstat()
    return dict(device=info.st_dev, inode=info.st_ino, uid=info.st_uid,
                mode=oct(stat.S_IMODE(info.st_mode)), socket=stat.S_ISSOCK(info.st_mode))


def resource_context(binary, env, proof, report, run, client_handles):
    """Reuse the production native resource measurement, never synthesized rows.

    The sole measurement adapter is a traced stop client; safety adapters remove
    forced-kill fallback from failure cleanup. Every returned result is from the
    real consumer process. Original 25ms/2s/20s sampling constants are untouched.
    """
    import native_resources as resources

    resource_proof = proof / "resource-context"
    resource_proof.mkdir(mode=0o700)
    resource_report = dict(
        schema_version=1, host=dict(os="macos", os_version=platform.platform(),
                                   arch=platform.machine(), python_version=platform.python_version()),
        baseline=dict(label="baseline", **report["provenance"]), candidate=None, runs=[],
        requested_modes=["status", "watch", "top", "tray"],
        functional_gate="all_measured_products",
        privacy=dict(raw_status_events_terminal_clipboard_published=False, hardware_actions=False,
                     notifications_sound_history_lock_policy=False, autostart_enabled=False),
        excluded_processes=["qualification Python sampler/PTY server", "desktop/window/session servers",
                            "tray readiness/stop control probes"],
        ram_policy=dict(target_bytes=resources.BUDGET, policy="soft_optimization_target",
                        acceptable_reference_bytes=30_000_000, reference_is_hard_cutoff=False,
                        above_target_fails_functional_suite=False),
        metric_limits=[
            "Uses native_resources.MacProcesses, Sampler, measure, sustain and qualify unchanged for observations.",
            "Requested 25ms sampling; actual gaps and >50ms gaps remain in every measured row.",
            "All observed product descendants included; short-lived descendants between samples can be missed.",
            "Concurrent peak sums RSS at each observation, not independent per-PID highwaters; shared mappings may be double counted.",
            "macOS OS RSS highwater unavailable; proc_pid_rusage v4 lifetime physical footprint is a separate metric, never RSS.",
            "CPU excludes time before first observation and may miss final CPU; sampler overhead excluded but perturbs scheduling.",
            "No cache purge; version/settings probes precede fixed status/status/status/watch/top/top-burst/tray order.",
            "Status accepts explicit collector degradation; watch has no ready handshake and idle may emit zero documents.",
            "Top readiness is rendered Quit; tray readiness is control protocol, not icon painting or physical device readiness.",
            "Burst sends 8 read-only refresh keys/second, alternating terminal resize, with 200ms output-consumer delay.",
            "Safety-only deviations: forced-kill fallback disabled; original stop-client 20s deadline retained.",
            "The earlier public0161 seven-profile history is not replayed; no0161 archive is downloaded.",
            "No physical-device/event latency, capture, permission, clipboard, lock transition, sound or autostart qualification.",
        ])
    report["resource_context"] = dict(
        status="failed", measured_binary="same immutable public0191 as provenance",
        suite="real status x3, watch, top, top burst, tray",
        sampling_interval_seconds=resources.INTERVAL, warmup_seconds=resources.WARMUP,
        sustained_seconds=resources.DURATION, first_failure=None)
    # One context suite, not eight repetitions of the already-passing plain loop.
    report["cycles"].append(dict(index=0, status="failed", profile="resource-context"))
    children = []
    terminals = []
    authority = Processes()
    original_environment = os.environ.copy()
    original_subprocess = resources.subprocess
    original_terminal = resources.NativeTerminal
    original_sampler = resources.Sampler
    original_stop_owned = resources.stop_owned
    backend = resources.MacProcesses()

    def owned_popen(*args, **kwargs):
        process = subprocess.Popen(*args, **kwargs)
        children.append(process)
        def refuse_forced_kill():
            raise RuntimeError("Resource failure cleanup requested forced kill; refused")
        process.kill = refuse_forced_kill
        return process

    def observed_run(argv, *positional, **kwargs):
        arguments = [str(value) for value in argv]
        if arguments == [str(binary), "tray", "stop"]:
            require(not positional, "Unexpected positional native stop subprocess arguments")
            require(kwargs.get("stdout") == subprocess.PIPE and kwargs.get("stderr") == subprocess.PIPE,
                    "Original stop diagnostic transport changed")
            # The original consumer's 20-second process deadline is not enlarged.
            # run() preserves original bytes/code and does not kill on timeout.
            code, stdout, stderr = run("resource-context-stop", arguments, traced=True,
                                       timeout=kwargs["timeout"], preserve_timeout=True)
            return subprocess.CompletedProcess(argv, code, stdout, stderr)
        # Same actual process transport and outcomes, but a timeout is preserved
        # without subprocess.run's implicit kill. Current non-stop calls are
        # only real status/version/settings probes.
        require(not positional, "Unexpected positional resource subprocess arguments")
        timeout = kwargs.pop("timeout", None)
        check = kwargs.pop("check", False)
        process = owned_popen(argv, **kwargs)
        stdout, stderr = process.communicate(timeout=timeout)
        result = subprocess.CompletedProcess(argv, process.returncode, stdout, stderr)
        if check:
            result.check_returncode()
        return result

    def observed_check_output(argv, **kwargs):
        require("stdout" not in kwargs, "Unexpected check_output stdout override")
        return observed_run(argv, stdout=subprocess.PIPE, check=True, **kwargs).stdout

    class ObservedSampler(original_sampler):
        def track(self, row, process_handle=None):
            key = (row["pid"], row["identity"])
            fresh = key not in self.tracked
            super().track(row, process_handle)
            if fresh:
                owned = authority.get(row["pid"])
                if owned is not None:
                    birth = ":".join(str(value) for value in owned["birth"])
                    if birth != row["identity"]:
                        report["resource_context"].setdefault("exit_filter_registration_gaps", []).append(
                            dict(pid=row["pid"], sampled_birth=row["identity"], reason="birth changed"))
                    else:
                        try:
                            authority.retain(owned)
                        except (ProcessLookupError, AssertionError):
                            if authority.get(row["pid"]) == owned:
                                raise
                            # Short status/SDK children can exit between the real
                            # sampler observation and added diagnostic retention.
                            # Keep the measurement row unchanged and expose gap.
                            report["resource_context"].setdefault("exit_filter_registration_gaps", []).append(
                                dict(pid=row["pid"], sampled_birth=row["identity"], reason="exited during registration"))

    class CooperativeTerminal(original_terminal):
        def __init__(self, *args, **kwargs):
            super().__init__(*args, **kwargs)
            children.append(self.process)
            terminals.append(self)

        def close(self):
            # Successful Q already exited. A failed Q remains failed; never
            # turn it into a pass through another input, TERM or KILL.
            if self.process.poll() is None:
                raise RuntimeError("Owned terminal remains after failed Quit; no signal sent")
            os.close(self.master)
            terminals.remove(self)

    def cooperative_stop_owned(process, sampler):
        if process.poll() is None:
            if sampler.output["mode"] != "watch":
                # A failing tray control is not retried or bypassed. Keep exact
                # handles and leave teardown to the disposable hosted runner.
                raise RuntimeError("Owned non-watch process remains after failure; no signal sent")
            # Existing real watch shutdown is Ctrl-C, bounded to its unreaped
            # direct Popen child. There is no forced-kill fallback.
            process.send_signal(__import__("signal").SIGINT)
            process.wait(timeout=15)
        sampler.cleanup_descendants()

    try:
        os.environ.clear()
        os.environ.update(env)
        resources.subprocess = types.SimpleNamespace(
            **{name: getattr(subprocess, name) for name in dir(subprocess)
               if name not in ("run", "Popen", "check_output")},
            run=observed_run, Popen=owned_popen, check_output=observed_check_output)
        resources.NativeTerminal = CooperativeTerminal
        resources.Sampler = ObservedSampler
        resources.stop_owned = cooperative_stop_owned
        resources.qualify(binary, backend, resource_report["baseline"], resource_report,
                          resource_proof, "immutable_public0191_original_resource_order_with_stop_syscalls",
                          True, reference_only=False)
        require(resource_report["baseline"]["version"] == "mcw 0.19.1",
                "Resource context substituted a different product version")
        resource_report["completed"] = True
        report["resource_context"]["status"] = "passed"
        report["cycles"][-1]["status"] = "passed"
    except Exception as error:
        resource_report["completed"] = False
        failed = next((row for row in resource_report["runs"] if row.get("functional") == "failed"), None)
        primary = failed.get("error") if failed else str(error)
        resource_report["operational_error"] = primary
        report["resource_context"]["first_failure"] = dict(
            type="measured-functional-failure" if failed else type(error).__name__, message=primary,
            mode=failed.get("mode") if failed else None,
            profile=failed.get("profile") if failed else None,
            stop_control_exit_code=failed.get("stop_control_exit_code") if failed else None,
            stop_control_error=failed.get("stop_control_error") if failed else None)
        if failed and failed.get("cleanup_error"):
            report["resource_context"]["cleanup_error"] = dict(type=type(error).__name__, message=str(error))
        raise
    finally:
        resources.subprocess = original_subprocess
        resources.NativeTerminal = original_terminal
        resources.Sampler = original_sampler
        resources.stop_owned = original_stop_owned
        os.environ.clear()
        os.environ.update(original_environment)
        resources.save(resource_proof, resource_report)
        backend.close()
        deadline = time.monotonic() + 8
        while not authority.exited() and time.monotonic() < deadline:
            time.sleep(.05)
        report["resource_context"]["owned_process_handles"] = list(authority.owned.values())
        report["resource_context"]["all_exit_events_observed"] = authority.exited()
        report["resource_context_live_processes"] = [process.pid for process in children + client_handles
                                                    if process.poll() is None]
        report["resource_context_live_processes"] += [
            row["pid"] for row in authority.owned.values() if not row["exit_observed"]
            and row["pid"] not in report["resource_context_live_processes"]]
        authority.close()
        report["resource_context"]["terminal_descriptors_retained"] = [terminal.master for terminal in terminals]
    require(not report["resource_context_live_processes"],
            "Resource context retains live owned peers; no forced kill sent")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--proof", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, help="Source-built real candidate; omit for pinned public0191")
    parser.add_argument("--cycles", type=int, default=8)
    parser.add_argument("--resource-context", action="store_true",
                        help="Measure public0191 through real status/watch/top/burst/tray history and 25ms sampler once")
    parser.add_argument("--disposable-runner", action="store_true", required=True)
    args = parser.parse_args()
    require(not (args.resource_context and args.candidate), "Resource diagnosis requires immutable public0191, not source candidate")
    require(platform.system() == "Darwin", "macOS native runner required")
    require(os.environ.get("GITHUB_ACTIONS") == "true" and os.environ.get("RUNNER_ENVIRONMENT") == "github-hosted",
            "Only disposable GitHub-hosted runners are supported")
    require(os.getuid() == os.geteuid() != 0, "Ordinary unprivileged runner required")
    require(1 <= args.cycles <= 40, "Cycles must be bounded between 1 and 40")
    proof = args.proof.resolve()
    proof.mkdir(parents=True, exist_ok=False)
    os.chmod(proof, 0o700)
    report = dict(schema=1, status="failed", mode="source-candidate" if args.candidate else "immutable-public0191",
                  host=platform.platform(), architecture=platform.machine(), cycles=[],
                  limitations=["No physical capture or protection mutation", "No SIP/TCC/Gatekeeper changes",
                               "No forced kill or arbitrary process stop", "No UI visibility assertion"],
                  first_failure=None, cleanup=[])
    report["fixture_commit"] = os.environ.get("GITHUB_SHA")
    report["root_cause"] = "not established; passing cycles do not explain the historical EINVAL"
    if args.candidate:
        report["provenance"] = dict(source_commit=os.environ.get("GITHUB_SHA"), origin="source candidate, not public baseline")
    work = Path(tempfile.mkdtemp(prefix="ms-", dir="/tmp")).resolve()
    report["sandbox"] = str(work)
    active = None
    peers = None
    env = None
    sockpath = None
    client_handles = []

    def save():
        (proof / "stop.json").write_text(json.dumps(report, indent=2) + "\n")

    def run(name, argv, *, traced=False, timeout=60, preserve_timeout=False):
        child_env = env.copy()
        fd = None
        descriptors = ()
        if traced:
            trace_path = proof / (name + ".syscalls.jsonl")
            fd = os.open(trace_path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
            descriptors = (fd,)
            child_env.update(DYLD_INSERT_LIBRARIES=str(work / "stop-trace.dylib"), MCW_STOP_TRACE_FD=str(fd))
        process = None
        try:
            # communicate(timeout) does not kill the child on timeout. Preserve
            # the exact handle instead of subprocess.run's forced-kill behavior.
            process = subprocess.Popen([str(arg) for arg in argv], env=child_env,
                                       stdout=subprocess.PIPE, stderr=subprocess.PIPE, pass_fds=descriptors)
            client_handles.append(process)
            row = dict(name=name, pid=process.pid, timed_out=False)
            try:
                stdout, stderr = process.communicate(timeout=timeout)
            except subprocess.TimeoutExpired as error:
                stdout, stderr = error.output or b"", error.stderr or b""
                row["timed_out"] = True
            row.update(exit_code=process.poll(), stdout_file=name + ".stdout", stderr_file=name + ".stderr")
            (proof / (name + ".stdout")).write_bytes(stdout)
            (proof / (name + ".stderr")).write_bytes(stderr)
            (proof / (name + ".command.json")).write_text(json.dumps(row, indent=2) + "\n")
            if traced:
                report["cycles"][-1]["stop_client"] = row
            if row["timed_out"]:
                if preserve_timeout:
                    raise subprocess.TimeoutExpired([str(arg) for arg in argv], timeout, output=stdout, stderr=stderr)
                raise AssertionError(name + ": bounded client deadline exceeded; no kill sent")
            if traced:
                operations = [json.loads(line) for line in trace_path.read_text().splitlines()]
                report["cycles"][-1]["syscall_errors"] = [op for op in operations if op["errno"]]
                require(any(op["operation"] == "interposer_loaded" for op in operations),
                        "Stop interposer was not loaded; native diagnosis unavailable")
                require(len(operations) > 1, "Stop interposer observed no native operations")
                report["cycles"][-1]["errno22_stages"] = [op for op in operations if op["errno"] == 22]
            return process.returncode, stdout, stderr
        finally:
            if fd is not None:
                os.close(fd)

    try:
        home = work / "h"
        home.mkdir(mode=0o700)
        runtime = home / "r"
        runtime.mkdir(mode=0o700)
        env = {key: value for key, value in os.environ.items() if not key.startswith("DYLD_") and key != "MCW_STOP_TRACE_FD"}
        env.update(HOME=str(home), XDG_CONFIG_HOME=str(home / "c"), XDG_DATA_HOME=str(home / "d"),
                   XDG_STATE_HOME=str(home / "s"), XDG_RUNTIME_DIR=str(runtime),
                   LANG="en_US.UTF-8", LC_ALL="en_US.UTF-8")
        settings = home / "c/MicCamWatch/settings.toml"
        settings.parent.mkdir(parents=True, mode=0o700)
        settings.write_bytes(SAFE)
        settings.chmod(0o600)
        safe_identity = settings.stat().st_ino
        sockpath = home / "d/MicCamWatch/tray-runtime/control.sock"
        require(len(os.fsencode(sockpath)) < 104, "Private socket path is too long")
        binary = args.candidate.resolve() if args.candidate else baseline(work, report)
        report["binary_sha256"] = sha(binary)
        code, output, _ = run("version", [binary, "--version"])
        require(code == 0, "Version command failed")
        report["version"] = output.decode().strip()
        require(args.candidate or report["version"] == "mcw 0.19.1", "Public baseline version mismatch")
        code, output, _ = run("settings-path", [binary, "config", "settings-path"])
        require(code == 0 and Path(os.fsdecode(output).strip()).resolve() == settings, "Settings escaped private HOME")
        code, output, _ = run("notifications-disabled", [binary, "notifications", "status"])
        require(code == 0 and output.strip() == b"disabled", "Safe preferences not loaded")
        code, _, _ = run("compile-stop-interposer", ["/usr/bin/xcrun", "clang", "-dynamiclib", "-O2",
                         Path(__file__).with_name("macos_stop_trace.c"), "-o", work / "stop-trace.dylib"])
        require(code == 0, "Native stop interposer compilation failed")
        code, _, _ = run("aqua-session", ["/bin/launchctl", "print", "gui/" + str(os.getuid())])
        require(code == 0, "Logged-in native Aqua session unavailable")
        if args.resource_context:
            report["mode"] = "immutable-public0191-resource-context"
            # Match native_resources.isolate() safe settings exactly, including
            # its default profile; source candidate defaults are not substituted.
            context_settings = SAFE.replace(b'profile = "development"\n', b"")
            settings.write_bytes(context_settings)
            resource_context(binary, env, proof, report, run, client_handles)
            require(settings.read_bytes() == context_settings and settings.stat().st_ino == safe_identity,
                    "Resource measurement changed private safety preferences")
        for cycle in range(0 if args.resource_context else args.cycles):
            label = f"cycle-{cycle:02d}"
            row = dict(index=cycle, status="failed", dwell_seconds=20 if cycle == 0 else 1)
            report["cycles"].append(row)
            require(not sockpath.exists(), "A previous owned control socket remained")
            code, _, _ = run(label + "-before", [binary, "tray", "status"])
            require(code == 1, "Private namespace was not initially stopped")
            peers = Processes()
            with (proof / (label + "-frontend.stdout")).open("xb") as out, (proof / (label + "-frontend.stderr")).open("xb") as err:
                active = subprocess.Popen([str(binary), "tray", "run"], env=env, stdout=out, stderr=err)
            root = peers.get(active.pid)
            require(root and Path(root["executable"]).resolve() == binary and root["parent"] == os.getpid(),
                    "Native frontend ownership could not be established")
            peers.retain(root)
            ready = False
            deadline = time.monotonic() + 45
            attempt = 0
            while time.monotonic() < deadline:
                require(active.poll() is None, "Frontend exited before readiness")
                code, _, _ = run(label + f"-status-{attempt:03d}", [binary, "tray", "status"], timeout=8)
                attempt += 1
                if code == 0:
                    ready = True
                    break
                time.sleep(.2)
            require(ready, "Native frontend did not become ready")
            peers.discover(active.pid)
            require(len(peers.owned) >= 2, "Real GUI helper peer not observed")
            row["socket_before_stop"] = metadata(sockpath)
            require(row["socket_before_stop"]["socket"] and row["socket_before_stop"]["uid"] == os.getuid()
                    and row["socket_before_stop"]["mode"] == "0o600", "Unsafe control socket")
            if args.candidate:
                original = set(peers.owned)
                for reopen in range(3):
                    code, _, _ = run(label + f"-reopen-{reopen}", [binary, "tray", "open"], traced=True, timeout=10)
                    require(code == 0 and active.poll() is None, "Real consumer reopen failed")
                    peers.discover(active.pid)
                    require(set(peers.owned) == original, "Reopen created another persistent frontend peer")
                row["consumer_reopen"] = "passed"
            time.sleep(row["dwell_seconds"])
            code, _, _ = run(label + "-ready-before-stop", [binary, "tray", "status"])
            require(code == 0 and active.poll() is None, "Frontend lost readiness before stop")
            peers.discover(active.pid)
            row["owned_process_handles"] = list(peers.owned.values())
            save()
            # Instrument from the first stop, not a re-run of an already-known
            # failing plain stop. A failure aborts the loop permanently.
            code, _, _ = run(label + "-stop", [binary, "tray", "stop"], traced=True)
            require(code == 0, "Real tray stop failed; see preserved first client exit/error/syscalls")
            require(active.wait(timeout=15) == 0, "Frontend exit was not clean")
            deadline = time.monotonic() + 8
            while not peers.exited() and time.monotonic() < deadline:
                time.sleep(.05)
            row["owned_process_handles"] = list(peers.owned.values())
            require(peers.exited(), "Retained frontend/helper peers did not exit within bounded shutdown")
            require(not sockpath.exists(), "Stop left its control socket")
            code, _, _ = run(label + "-after", [binary, "tray", "status"])
            require(code == 1, "Stopped consumer status was not stopped")
            require(settings.read_bytes() == SAFE and settings.stat().st_ino == safe_identity,
                    "Lifecycle changed private safety preferences")
            row["status"] = "passed"
            active = None
            peers.close()
            peers = None
            save()
        report["status"] = "passed"
    except Exception as error:
        report["first_failure"] = report.get("resource_context", {}).get("first_failure") or dict(
            type=type(error).__name__, message=str(error))
        save()
    finally:
        # Not a proof retry: a separate direct protocol request, authenticated
        # to our exact still-live direct child, is ownership-bounded cleanup.
        # It cannot change first_failure/status or make a failed cycle pass.
        try:
            if active is not None and active.poll() is None and peers is not None and sockpath is not None:
                saved_root = peers.owned.get(active.pid)
                current_root = peers.get(active.pid)
                require(saved_root and current_root and current_root["birth"] == saved_root["birth"],
                        "Cleanup root identity changed; refusing control")
                with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as control:
                    control.settimeout(45)
                    control.connect(str(sockpath))
                    user, group = C.c_uint(), C.c_uint()
                    libc = C.CDLL("/usr/lib/libSystem.B.dylib", use_errno=True)
                    libc.getpeereid.argtypes = [C.c_int, C.POINTER(C.c_uint), C.POINTER(C.c_uint)]
                    require(libc.getpeereid(control.fileno(), C.byref(user), C.byref(group)) == 0,
                            "Cleanup peer UID authentication failed")
                    peer_uid = user.value
                    peer_pid = array.array("i", control.getsockopt(0, 2, 4))[0]  # SOL_LOCAL / LOCAL_PEERPID
                    require(peer_uid == os.getuid() and peer_pid == active.pid,
                            "Cleanup socket peer is not our exact direct child")
                    control.sendall(b"stop\n")
                    reply = control.recv(64)
                    report["cleanup"].append(dict(method="owned-peer direct stop (not qualification)",
                                                  peer_pid=peer_pid, reply=reply.decode("ascii", "replace")))
                active.wait(timeout=15)
            if peers is not None:
                deadline = time.monotonic() + 8
                while not peers.exited() and time.monotonic() < deadline:
                    time.sleep(.05)
                report["cleanup"].append(dict(owned_process_handles=list(peers.owned.values()),
                                              all_exit_events_observed=peers.exited()))
                require(peers.exited(), "Owned peers remain; no kill sent, disposable runner teardown required")
            if active is not None:
                require(active.poll() is not None, "Owned frontend remains; sandbox retained without forced kill")
            for client in client_handles:
                if client.poll() is None:
                    try:
                        client.communicate(timeout=5)
                    except subprocess.TimeoutExpired:
                        report.setdefault("outstanding_owned_clients", []).append(dict(pid=client.pid))
            require(not report.get("outstanding_owned_clients"),
                    "Owned diagnostic client remains; sandbox retained without forced kill")
            require(not report.get("resource_context_live_processes"),
                    "Resource context retains live owned processes; sandbox retained without forced kill")
            shutil.rmtree(work)
            report["sandbox_cleanup"] = "removed owned private tree after bounded cooperative exit"
        except Exception as error:
            report["cleanup_error"] = dict(type=type(error).__name__, message=str(error))
            report["status"] = "failed"
            report["sandbox_cleanup"] = "retained; no forced kill or broad process cleanup"
        finally:
            if peers is not None:
                peers.close()
            save()
    print(report["status"].upper() + ": " + report["mode"] + "; inspect stop.json and first stop syscall trace")
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    sys.exit(main())
