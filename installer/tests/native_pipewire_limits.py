#!/usr/bin/env python3
"""Exercise native Linux graph limits against an already-owned virtual capture.

--binary CANDIDATE --capture-pid OWNED_PW_CAT_PID --proof DIRECTORY
Only disposable GitHub-hosted Linux runners are accepted. The private pw-dump
proxy defaults to the real executable; fault modes are controlled subprocess
fixtures, not claims about physical capture, event latency, or product RSS.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import select
import shlex
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import uuid

FAULTS = ("malformed", "retained_limit", "stdout_flood", "stderr_flood", "blocked", "orphan")
ROOT_TIMEOUT = 8.0
MAX_REGISTRY = 256 * 1024
MAX_OUTPUT = 4 * 1024 * 1024


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def process_identity(pid):
    try:
        text = Path(f"/proc/{pid}/stat").read_text()
    except (FileNotFoundError, ProcessLookupError):
        return None
    fields = text[text.rfind(")") + 2:].split()
    return int(fields[19]), fields[0]


def register_fixture(pid, role, mode):
    identity = process_identity(pid)
    if identity is None:
        return
    entry = dict(event="spawn", pid=pid, start=identity[0], role=role,
                 mode=mode, run=os.environ["MCW_FIXTURE_RUN"])
    append_registry(entry)


def append_registry(entry):
    path = os.environ["MCW_FIXTURE_REGISTRY"]
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o600)
    try:
        require(os.fstat(fd).st_size < MAX_REGISTRY, "fixture registry exceeded its bound")
        payload = (json.dumps(entry, separators=(",", ":")) + "\n").encode()
        require(len(payload) < 4096, "fixture registry record too large")
        require(os.write(fd, payload) == len(payload), "incomplete fixture registry write")
    finally:
        os.close(fd)


def write_all(fd, data):
    view = memoryview(data)
    while view:
        count = os.write(fd, view)
        require(count > 0, "fixture output write made no progress")
        view = view[count:]


def proxy(arguments):
    mode = Path(os.environ["MCW_FIXTURE_MODE"]).read_text().strip()
    register_fixture(os.getpid(), "proxy", mode)
    genuine = os.environ["MCW_REAL_PW_DUMP"]
    if mode == "normal":
        os.execv(genuine, [genuine, *arguments])
    if mode == "augmented":
        # Stream ignored objects plus the genuine top-level array. Never build
        # a DOM or retain the real graph in this qualification helper either.
        child = subprocess.Popen([genuine, *arguments], stdin=subprocess.DEVNULL,
                                 stdout=subprocess.PIPE)
        register_fixture(child.pid, "graph_dump", mode)
        output_bytes = 0

        def emit(data):
            nonlocal output_bytes
            output_bytes += len(data)
            write_all(1, data)

        def next_nonspace():
            while True:
                byte = child.stdout.read(1)
                require(byte, "real pw-dump ended before its array")
                if not byte.isspace():
                    return byte

        require(next_nonspace() == b"[", "real pw-dump did not produce an array")
        emit(b"[")
        port = b'{"type":"PipeWire:Interface:Port/3","info":{"params":[' + b"0," * 63 + b"0]}}"
        for index in range(16384):
            emit((b"," if index else b"") + port)
        first = next_nonspace()
        emit(first if first == b"]" else b"," + first)
        while chunk := child.stdout.read(65536):
            emit(chunk)
        require(child.wait(timeout=2) == 0, "real pw-dump failed inside graph overlay")
        append_registry(dict(event="output", mode=mode, run=os.environ["MCW_FIXTURE_RUN"],
                             bytes=output_bytes, ignored_ports=16384))
        return 0
    if mode == "malformed":
        write_all(1, b'[{"type":')
        return 0
    if mode == "retained_limit":
        write_all(1, b"[")
        for index in range(4097):
            write_all(1, (b"," if index else b"") + b'{"type":"PipeWire:Interface:Client/3"}')
        write_all(1, b"]")
        return 0
    if mode == "blocked":
        time.sleep(120)
        return 0
    if mode == "orphan":
        child = subprocess.Popen([sys.executable, str(Path(__file__).resolve()), "--fixture-orphan"],
                                 stdin=subprocess.DEVNULL)
        register_fixture(child.pid, "orphan", mode)
        write_all(1, b"[]\n")
        # Deliberately exit without waiting: the child retains both descriptors.
        os._exit(0)
    if mode in ("stdout_flood", "stderr_flood"):
        write_all(1, b"[]\n")
        fd, chunk = (1, b" " * 65536) if mode == "stdout_flood" else (2, b"x" * 65536)
        while True:
            write_all(fd, chunk)
    raise AssertionError(f"unknown proxy mode: {mode}")


class ProcessToken:
    """A pidfd, never a numeric PID, authorizes signaling after observation."""

    def __init__(self, pid, expected_start, capture_pid):
        require(pid != capture_pid, "attempt to acquire signal authority for the capture client")
        before = process_identity(pid)
        if before is None or before[0] != expected_start:
            raise ProcessLookupError(pid)
        self.fd = os.pidfd_open(pid)
        after = process_identity(pid)
        if after is None or after[0] != expected_start:
            os.close(self.fd)
            raise ProcessLookupError(pid)

    def exited(self):
        poller = select.poll()
        poller.register(self.fd, select.POLLIN)
        return bool(poller.poll(0))

    def send(self, sig):
        try:
            signal.pidfd_send_signal(self.fd, sig)
        except ProcessLookupError:
            pass

    def close(self):
        os.close(self.fd)


def registry_entries(path):
    if not path.exists():
        return []
    require(path.stat().st_size <= MAX_REGISTRY + 4096, "fixture registry is oversized")
    entries = [json.loads(line) for line in path.read_text().splitlines()]
    require(len(entries) <= 2048, "fixture registry accumulated too many records")
    return entries


def cleanup_fixtures(path, run, capture_pid):
    tokens = []
    seen = set()
    signaled = 0
    try:
        for entry in registry_entries(path):
            if entry.get("event") != "spawn" or entry.get("run") != run:
                continue
            require(entry["role"] in ("proxy", "orphan", "graph_dump"), "unexpected fixture owner")
            identity = entry["pid"], entry["start"]
            if identity in seen:
                continue
            seen.add(identity)
            try:
                token = ProcessToken(*identity, capture_pid)
            except (ProcessLookupError, FileNotFoundError):
                continue
            tokens.append(token)
            if not token.exited():
                token.send(signal.SIGKILL)
                signaled += 1
        deadline = time.monotonic() + 3
        while any(not token.exited() for token in tokens):
            require(time.monotonic() < deadline, "owned fixture failed to terminate")
            time.sleep(.02)
    finally:
        for token in tokens:
            token.close()
    return dict(registered_identities=len(seen), identity_pinned_terminations=signaled)


def diagnostic_classes(detail):
    if not detail:
        return []
    choices = ("timed out", "stderr exceeds", "graph exceeds", "relevant object count",
               "retained graph text", "invalid", "did not end cleanly")
    found = [choice for choice in choices if choice in detail]
    return found or ["other_explicit_diagnostic"]


def summarize(document, capture_pid):
    require(isinstance(document, dict), "product emitted non-object JSON")
    if "collectors" in document:
        collectors = {item["collector"]: item["state"] for item in document["collectors"]}
        details = {item["collector"]: diagnostic_classes(item.get("detail"))
                   for item in document["collectors"]}
        accesses = document.get("accesses", [])
        active = any(item.get("pid") == capture_pid and item.get("resource") == "microphone"
                     and item.get("activity") == "active" for item in accesses)
        return dict(kind="status", collectors=collectors, diagnostic_classes=details,
                    owned_capture_active=active, access_count=len(accesses))
    require(document.get("action") in ("start", "update", "stop"), "invalid watch action")
    return dict(kind="event", action=document["action"], resource=document.get("resource"),
                activity=document.get("activity"), owned_capture=document.get("pid") == capture_pid)


class Product:
    def __init__(self, binary, arguments, environment, capture_pid):
        self.run = uuid.uuid4().hex
        environment = dict(environment, MCW_FIXTURE_RUN=self.run)
        self.process = subprocess.Popen([str(binary), "--lang", "en", *arguments],
                                        env=environment, stdin=subprocess.DEVNULL,
                                        stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                        start_new_session=True)
        try:
            identity = process_identity(self.process.pid)
            require(identity is not None, "newly launched product identity unavailable")
            self.token = ProcessToken(self.process.pid, identity[0], capture_pid)
        except Exception:
            # Popen still owns its unreaped direct child here; its kill/wait
            # protocol cannot signal a numeric PID after releasing ownership.
            try:
                self.process.kill()
                self.process.wait(timeout=3)
            finally:
                try:
                    cleanup_fixtures(Path(environment["MCW_FIXTURE_REGISTRY"]), self.run, capture_pid)
                finally:
                    self.process.stdout.close()
                    self.process.stderr.close()
            raise
        self.capture_pid = capture_pid
        self.records = []
        self.pending = bytearray()
        self.bytes = {"stdout": 0, "stderr": 0}
        self.eof = {"stdout": False, "stderr": False}
        self.started = time.monotonic()
        for stream in (self.process.stdout, self.process.stderr):
            os.set_blocking(stream.fileno(), False)

    def pump(self):
        for name in ("stdout", "stderr"):
            if self.eof[name]:
                continue
            stream = getattr(self.process, name)
            for _ in range(8):
                try:
                    chunk = os.read(stream.fileno(), 65536)
                except BlockingIOError:
                    break
                if not chunk:
                    self.eof[name] = True
                    if name == "stdout":
                        require(not self.pending, "product JSON output ended mid-line")
                    break
                self.bytes[name] += len(chunk)
                require(self.bytes[name] <= MAX_OUTPUT, f"qualification {name} exceeded its bound")
                if name == "stdout":
                    self.pending.extend(chunk)
                    require(len(self.pending) <= 1024 * 1024, "product JSON line exceeded qualification bound")
                    while b"\n" in self.pending:
                        line, _, rest = self.pending.partition(b"\n")
                        self.pending = bytearray(rest)
                        require(line.strip(), "empty product JSON line")
                        self.records.append(summarize(json.loads(line), self.capture_pid))
                        require(len(self.records) <= 2048, "qualification record count exceeded its bound")

    def send(self, sig):
        self.token.send(sig)

    def close(self):
        self.token.close()
        self.process.stdout.close()
        self.process.stderr.close()


class Session:
    def __init__(self, root, genuine, capture_pid):
        self.capture_pid = capture_pid
        self.capture_identity = process_identity(capture_pid)
        require(self.capture_identity is not None and self.capture_identity[1] not in ("Z", "X"),
                "provided capture client is not live")
        expected = Path(shutil.which("pw-cat") or "").resolve()
        require(Path(os.readlink(f"/proc/{capture_pid}/exe")).resolve() == expected,
                "capture PID is not the existing native pw-cat executable")
        require(Path(f"/proc/{capture_pid}").stat().st_uid == os.getuid(), "capture has a different owner")
        self.mode = root / "mode"
        self.registry = root / "fixtures.jsonl"
        self.active = []
        self.environment = os.environ.copy()
        home = root / "home"
        settings = home / "config" / "MicCamWatch"
        settings.mkdir(parents=True)
        (settings / "settings.toml").write_text(
            "notifications_enabled = false\nsound_enabled = false\nshow_ready = false\nhistory_enabled = false\n"
            "mute_on_lock = false\nblock_camera_on_lock = false\nrestore_on_unlock = false\n")
        self.environment.update(HOME=str(home), XDG_CONFIG_HOME=str(home / "config"),
                                XDG_DATA_HOME=str(home / "data"), XDG_STATE_HOME=str(home / "state"),
                                XDG_CACHE_HOME=str(home / "cache"), MCW_REAL_PW_DUMP=str(genuine),
                                MCW_FIXTURE_MODE=str(self.mode), MCW_FIXTURE_REGISTRY=str(self.registry))
        proxy_dir = root / "bin"
        proxy_dir.mkdir()
        wrapper = proxy_dir / "pw-dump"
        wrapper.write_text("#!/bin/sh\nexec " + shlex.quote(sys.executable) + " "
                           + shlex.quote(str(Path(__file__).resolve())) + ' --proxy "$@"\n')
        wrapper.chmod(0o700)
        self.environment["PATH"] = str(proxy_dir) + os.pathsep + os.environ["PATH"]
        self.set_mode("normal")

    def capture_alive(self):
        identity = process_identity(self.capture_pid)
        require(identity is not None and identity[0] == self.capture_identity[0]
                and identity[1] not in ("Z", "X"), "owned virtual capture stopped or changed identity")

    def set_mode(self, mode):
        require(mode in ("normal", "augmented", *FAULTS), "invalid qualification mode")
        temporary = self.mode.with_suffix(".next")
        temporary.write_text(mode)
        os.replace(temporary, self.mode)

    def launch(self, binary, arguments):
        product = Product(binary, arguments, self.environment, self.capture_pid)
        self.active.append(product)
        return product

    def pump(self):
        self.capture_alive()
        for product in self.active:
            product.pump()
        time.sleep(.02)

    def wait_for(self, product, start, predicate, timeout=ROOT_TIMEOUT):
        deadline = time.monotonic() + timeout
        while True:
            self.pump()
            if any(predicate(record) for record in product.records[start:]):
                return
            require(product.process.poll() is None, "watch exited during graph scenario")
            require(time.monotonic() < deadline, "watch did not observe required graph state before deadline")

    def finish(self, product, graceful=False):
        fixtures_cleaned = False
        try:
            if product.process.poll() is None:
                product.send(signal.SIGINT if graceful else signal.SIGKILL)
            deadline = time.monotonic() + ROOT_TIMEOUT
            while product.process.poll() is None:
                self.pump()
                if time.monotonic() >= deadline:
                    product.send(signal.SIGKILL)
                    product.process.wait(timeout=3)
                    break
            cleanup = cleanup_fixtures(self.registry, product.run, self.capture_pid)
            fixtures_cleaned = True
            drain_deadline = time.monotonic() + 3
            while not all(product.eof.values()):
                product.pump()
                require(time.monotonic() < drain_deadline, "product descendants retained output handles")
                time.sleep(.02)
            return cleanup
        finally:
            # Parser/identity assertions must not skip owned process cleanup.
            # Retained pidfds remain safe even if poll() already reaped a root.
            try:
                if product.process.poll() is None:
                    product.send(signal.SIGKILL)
                product.process.wait(timeout=3)
            finally:
                try:
                    if not fixtures_cleaned:
                        cleanup_fixtures(self.registry, product.run, self.capture_pid)
                finally:
                    self.active.remove(product)
                    product.close()

    def status(self, binary):
        product = self.launch(binary, ["status", "--json"])
        timeout = False
        try:
            deadline = product.started + ROOT_TIMEOUT
            while product.process.poll() is None:
                self.pump()
                if time.monotonic() >= deadline:
                    timeout = True
                    break
            cleanup = self.finish(product)
            return dict(timed_out=timeout, elapsed_seconds=round(time.monotonic() - product.started, 3),
                        exit_code=product.process.returncode, records=product.records,
                        cleanup=cleanup, output_bytes=product.bytes)
        finally:
            if product in self.active:
                self.finish(product)

    def close(self):
        errors = []
        for product in list(self.active):
            try:
                self.finish(product)
            except Exception as error:
                errors.append(str(error))
        require(not errors, "owned process cleanup failed: " + "; ".join(errors))
        self.capture_alive()


def capture_status(record):
    return record.get("kind") == "status" and record.get("owned_capture_active") \
        and record["collectors"].get("pipewire_audio") == "healthy"


def capture_event(record):
    return record.get("kind") == "event" and record.get("owned_capture") \
        and record.get("resource") == "microphone" and record.get("activity") == "active" \
        and record.get("action") in ("start", "update")


def unavailable(record):
    return record.get("kind") == "status" and all(
        record["collectors"].get(name) == "unavailable"
        and record["diagnostic_classes"].get(name)
        for name in ("pipewire_audio", "pipewire_video"))


def checked_status(session, binary, healthy):
    result = session.status(binary)
    require(not result["timed_out"], "candidate status exceeded external finite deadline")
    require(len(result["records"]) == 1, "status did not emit exactly one complete JSON document")
    require((capture_status if healthy else unavailable)(result["records"][0]),
            "status did not preserve capture/health semantics for the selected fixture")
    return result


def candidate_proof(session, binary, report):
    session.set_mode("normal")
    report["initial_real_graph"] = checked_status(session, binary, True)
    watcher = session.launch(binary, ["watch", "--json", "--no-kill", "--interval", "200"])
    try:
        session.wait_for(watcher, 0, lambda record: capture_status(record) or capture_event(record))
        session.set_mode("augmented")
        report["augmented_real_graph"] = checked_status(session, binary, True)
        output = [entry for entry in registry_entries(session.registry)
                  if entry.get("event") == "output" and entry.get("mode") == "augmented"]
        require(output and all(entry["bytes"] <= 16 * 1024 * 1024 for entry in output),
                "real graph overlay was not completely streamed within the product byte bound")
        report["augmented_output"] = [dict(bytes=entry["bytes"], ignored_ports=entry["ignored_ports"])
                                      for entry in output]
        session.set_mode("normal")
        report["scenarios"] = []
        for mode in FAULTS:
            session.capture_alive()
            start = len(watcher.records)
            session.set_mode(mode)
            fault = checked_status(session, binary, False)
            expected = {
                "malformed": ("invalid",),
                "retained_limit": ("relevant object count",),
                "stdout_flood": ("graph exceeds", "timed out"),
                "stderr_flood": ("stderr exceeds",),
                "blocked": ("timed out",),
                "orphan": ("timed out",),
            }[mode]
            require(all(any(reason in fault["records"][0]["diagnostic_classes"][name]
                            for reason in expected)
                        for name in ("pipewire_audio", "pipewire_video")),
                    "selected fixture did not exercise its intended failure path")
            session.wait_for(watcher, start, unavailable)
            recovery_start = len(watcher.records)
            session.set_mode("normal")
            session.wait_for(watcher, recovery_start, capture_status)
            recovery = checked_status(session, binary, True)
            require(watcher.process.poll() is None, "watch failed across a collector observation gap")
            require(not any(record.get("action") == "stop" and record.get("owned_capture")
                            and record.get("resource") == "microphone" for record in watcher.records),
                    "watch emitted false STOP for the still-live virtual capture")
            report["scenarios"].append(dict(mode=mode, fault=fault, recovery=recovery,
                                             explicit_watch_gap=True, owned_capture_false_stop=False))
        session.set_mode("normal")
        report["final_real_graph"] = checked_status(session, binary, True)
        report["watch_cleanup"] = session.finish(watcher, graceful=True)
        require(watcher.process.returncode == 0, "watch did not exit cleanly on owned SIGINT")
        report["watch"] = dict(records=watcher.records, output_bytes=watcher.bytes,
                                exit_code=watcher.process.returncode,
                                owned_capture_false_stop=False)
    finally:
        session.set_mode("normal")
        if watcher in session.active:
            session.finish(watcher)


def baseline_proof(session, root, report):
    try:
        import native_resources
        require(native_resources.BASELINE_VERSION == "v0.16.1", "baseline is not immutable public v0.16.1")
        key = "linux-" + platform.machine().lower()
        require(key in native_resources.HASHES, "no independently pinned native baseline asset")
        binary = native_resources.fetch_baseline(root, key, report)
    except (ImportError, OSError) as error:
        report.update(status="unavailable", error=f"{type(error).__name__}: {error}"[:512])
        return
    session.set_mode("orphan")
    try:
        result = session.status(binary)
        require(result["timed_out"], "immutable baseline did not reproduce the controlled inherited-pipe hang")
        report.update(status="reproduced", scenario="controlled_orphan_inherited_pipes", result=result)
    finally:
        session.set_mode("normal")


def binary_digest(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        while chunk := stream.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--capture-pid", type=int, required=True)
    parser.add_argument("--proof", type=Path, required=True)
    args = parser.parse_args()
    require(sys.platform == "linux" and os.geteuid() != 0, "proof requires non-root native Linux")
    require(os.environ.get("GITHUB_ACTIONS") == "true"
            and os.environ.get("RUNNER_ENVIRONMENT") == "github-hosted",
            "proof requires a disposable GitHub-hosted runner")
    require(hasattr(os, "pidfd_open") and hasattr(signal, "pidfd_send_signal"),
            "identity-safe fixture cleanup requires native Python pidfd support")
    binary = args.binary.resolve(strict=True)
    genuine = shutil.which("pw-dump")
    require(genuine, "real PipeWire pw-dump is unavailable")
    args.proof.mkdir(parents=True, exist_ok=True)
    report = dict(schema=1, status="failed", candidate_sha256=binary_digest(binary),
                  scope="controlled_native_graph_and_subprocess_fixtures_with_existing_owned_virtual_pw_cat",
                  exclusions=["physical hardware capture or writes", "permissions/root actions", "RSS qualification",
                              "user desktop/UI reproduction", "mute/restore/autostart changes"],
                  baseline={}, candidate={})
    try:
        with tempfile.TemporaryDirectory(prefix="pipewire-limits-", dir=args.proof) as temporary:
            root = Path(temporary)
            session = Session(root, Path(genuine).resolve(), args.capture_pid)
            try:
                baseline_dir = root / "public-baseline"
                baseline_dir.mkdir()
                baseline_proof(session, baseline_dir, report["baseline"])
                candidate_proof(session, binary, report["candidate"])
                session.capture_alive()
                report["status"] = "passed"
                report["owned_capture_identity_unchanged"] = True
            finally:
                session.close()
    except Exception as error:
        report["status"] = "failed"
        report["error"] = f"{type(error).__name__}: {error}"[:1024]
        raise
    finally:
        (args.proof / "pipewire-limits.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(dict(proof=str(args.proof / "pipewire-limits.json"), status=report["status"])))


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "--proxy":
        sys.exit(proxy(sys.argv[2:]))
    if len(sys.argv) > 1 and sys.argv[1] == "--fixture-orphan":
        time.sleep(120)
        sys.exit(0)
    main()
