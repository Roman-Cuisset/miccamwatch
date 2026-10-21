#!/usr/bin/env python3
"""Real consumer uninstall contracts; disposable hosted runners only.

Unix uses the production installer to produce ownership/PATH receipts around
real native artifacts (the existing controlled transport, never a fake mcw).
Windows uses the exact packaging receipt producer and real CLI/tray processes.
No path, user, PID, device or application-data contents enter the proof report.
Private command/frontend output and the first failure are retained beside the
sanitized report, including failures before any native scenario completes.
Windows exercises both frontend hosts; readiness failures retain SDK diagnostics
and, after a companion exits, the same backend's actual console error output.
Unix sockets use compact data namespaces; adversarial install paths stay intact.
"""
import argparse
import ctypes
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tempfile
import time
import traceback


_evidence_directory = None
# Retain timed-out handles for diagnosis; never kill a native consumer simply
# because the fixture deadline expired. Its output continues into private files.
_timed_out_commands = []
_command_number = 0


def record_operation(argv, result):
    if _evidence_directory is not None:
        def text(value):
            return value.decode("utf-8", errors="replace") if isinstance(value, bytes) else value
        record = {"argv": [str(arg) for arg in argv], "exit_code": getattr(result, "returncode", None),
                  "timed_out": isinstance(result, subprocess.TimeoutExpired),
                  "stdout": text(getattr(result, "stdout", None)),
                  "stderr": text(getattr(result, "stderr", None)),
                  "stdout_file": getattr(result, "stdout_file", None),
                  "stderr_file": getattr(result, "stderr_file", None)}
        with (_evidence_directory / "private-commands.jsonl").open("a", encoding="utf-8") as output:
            output.write(json.dumps(record) + "\n")


def save_proof(args, proof):
    pending = args.proof.with_suffix(args.proof.suffix + ".tmp")
    pending.write_text(json.dumps(proof, indent=2) + "\n", encoding="utf-8")
    pending.replace(args.proof)


def save_failure(args, proof, error):
    if proof["first_failure"] is None:
        details = "".join(traceback.format_exception(type(error), error, error.__traceback__))
        (args.proof.parent / "private-first-failure.log").write_text(details, encoding="utf-8")
        proof["first_failure"] = {"type": type(error).__name__, "details_file": "private-first-failure.log"}
    proof["status"] = "failed"
    save_proof(args, proof)


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def capture(argv, env, timeout, *, cwd=None, start_new_session=False):
    global _command_number
    require(_evidence_directory is not None, "fixture command evidence is not initialized")
    _command_number += 1
    directory = _evidence_directory / "private-commands"
    directory.mkdir(exist_ok=True)
    stdout_path = directory / f"{_command_number:06d}.stdout"
    stderr_path = directory / f"{_command_number:06d}.stderr"
    command = [str(arg) for arg in argv]
    # Direct files retain late output without filling a pipe after a deadline.
    # Closing our handles never closes the child's inherited handles.
    with stdout_path.open("wb") as output, stderr_path.open("wb") as errors:
        process = subprocess.Popen(command, env=env, cwd=cwd, stdin=subprocess.DEVNULL,
                                   stdout=output, stderr=errors, start_new_session=start_new_session)
        try:
            process.wait(timeout=timeout)
        except subprocess.TimeoutExpired as error:
            _timed_out_commands.append(process)
            error.output = stdout_path.read_bytes()
            error.stderr = stderr_path.read_bytes()
            error.stdout_file = str(stdout_path.relative_to(_evidence_directory))
            error.stderr_file = str(stderr_path.relative_to(_evidence_directory))
            record_operation(argv, error)
            raise
    result = subprocess.CompletedProcess(command, process.returncode,
                                         stdout_path.read_text(encoding="utf-8", errors="replace"),
                                         stderr_path.read_text(encoding="utf-8", errors="replace"))
    result.stdout_file = str(stdout_path.relative_to(_evidence_directory))
    result.stderr_file = str(stderr_path.relative_to(_evidence_directory))
    record_operation(argv, result)
    return result


def run(argv, env, allowed=(0,), timeout=180):
    result = capture(argv, env, timeout)
    require(allowed is None or result.returncode in allowed,
            f"consumer operation {Path(argv[0]).name} {argv[1:2]} failed (exit {result.returncode})")
    return result


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def environment(home):
    env = os.environ.copy()
    values = {"HOME": home, "USERPROFILE": home, "APPDATA": home / "roaming",
              "LOCALAPPDATA": home / "local", "XDG_CONFIG_HOME": home / "config",
              "XDG_DATA_HOME": home / "data", "XDG_STATE_HOME": home / "state",
              "XDG_CACHE_HOME": home / "cache", "TMPDIR": home / "tmp",
              "TEMP": home / "tmp", "TMP": home / "tmp"}
    for name, value in values.items():
        value.mkdir(parents=True, exist_ok=True)
        env[name] = str(value)
    return env


def settings(binary, env):
    path = Path(run([binary, "config", "settings-path"], env).stdout.strip())
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("history_enabled = false\nnotifications_enabled = false\nshow_ready = false\nmute_on_lock = false\nblock_camera_on_lock = false\n", encoding="utf-8")
    policy = Path(run([binary, "config", "path"], env).stdout.strip())
    policy.parent.mkdir(parents=True, exist_ok=True)
    policy.write_text('language = "en"\n', encoding="utf-8")
    sentinel = Path(run([binary, "history", "path"], env).stdout.strip())
    sentinel.parent.mkdir(parents=True, exist_ok=True)
    sentinel.write_bytes(b'{"fixture":"existing-user-history-preserved"}\n')
    return path, sentinel


def data_snapshot(preferences, history):
    return preferences.read_bytes(), history.read_bytes(), preferences.with_name("policy.toml").read_bytes()


def wait_result(result):
    deadline = time.monotonic() + 45
    while time.monotonic() < deadline:
        if result.exists():
            try:
                document = json.loads(result.read_text())
            except (OSError, json.JSONDecodeError):
                time.sleep(.05)
                continue
            require(document["status"] == "removed", "deferred worker did not prove actual removal")
            return
        time.sleep(.05)
    raise AssertionError("deferred worker completion was not recorded")


def windows_cases(args, root, proof):
    # Preserve real worker diagnostics before the common temporary-root cleanup,
    # including failures before the caller prints its completion-record path.
    try:
        _windows_cases(args, root, proof)
    finally:
        failing = sys.exc_info()[0] is not None
        try:
            evidence = args.proof.parent / "private-windows"
            for stage in root.rglob(".mcw-uninstall-*"):
                if stage.is_dir():
                    destination = evidence / stage.relative_to(root).parent / stage.name.lstrip(".")
                    for name in ("worker.log", "result.json"):
                        source = stage / name
                        if source.is_file():
                            destination.mkdir(parents=True, exist_ok=True)
                            shutil.copy2(source, destination / name)
            for source in root.rglob("private-*-output*"):
                if source.is_file():
                    destination = evidence / source.relative_to(root)
                    destination.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copy2(source, destination)
        except Exception:
            if not failing:
                raise
            import traceback
            traceback.print_exc(file=sys.stderr)


def _windows_cases(args, root, proof):
    # Same ordinary-token requirement as the real command, not a test bypass.
    from ctypes import wintypes as W
    token, needed, elevation = W.HANDLE(), W.DWORD(), W.DWORD()
    native = ctypes.WinDLL("advapi32", use_last_error=True)
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    native.OpenProcessToken.argtypes = [W.HANDLE, W.DWORD, ctypes.POINTER(W.HANDLE)]
    native.OpenProcessToken.restype = W.BOOL
    native.GetTokenInformation.argtypes = [W.HANDLE, ctypes.c_int, ctypes.c_void_p, W.DWORD,
                                          ctypes.POINTER(W.DWORD)]
    native.GetTokenInformation.restype = W.BOOL
    kernel.CloseHandle.argtypes = [W.HANDLE]
    kernel.CloseHandle.restype = W.BOOL
    kernel.GetCurrentProcess.restype = W.HANDLE
    require(native.OpenProcessToken(kernel.GetCurrentProcess(), 8, ctypes.byref(token)), "cannot inspect fixture token")
    try:
        require(native.GetTokenInformation(token, 20, ctypes.byref(elevation), 4, ctypes.byref(needed)), "cannot inspect elevation")
        require(elevation.value == 0, "Windows fixture requires a real ordinary token, not elevated CI")
    finally:
        kernel.CloseHandle(token)
    proof["windows_fixture_real_ordinary_token"] = True
    user = ctypes.WinDLL("user32", use_last_error=True)
    user.FindWindowW.argtypes = [W.LPCWSTR, W.LPCWSTR]
    user.FindWindowW.restype = W.HWND
    user.GetWindowThreadProcessId.argtypes = [W.HWND, ctypes.POINTER(W.DWORD)]
    user.GetWindowThreadProcessId.restype = W.DWORD
    require(not user.FindWindowW("MicCamWatchTrayClass", None), "foreign tray present on disposable runner")

    def install(name):
        home = root / name
        env = environment(home)
        directory = home / "portable installation"
        directory.mkdir()
        cli = directory / "mcw.exe"
        shutil.copy2(args.binary, cli)
        shutil.copy2(args.binary.with_name("mcw-tray.exe"), directory / "mcw-tray.exe")
        receipt = run([cli, "__portable-receipt"], env).stdout
        (directory / ".miccamwatch-portable.json").write_text(receipt, encoding="utf-8")
        preferences, sentinel = settings(cli, env)
        return directory, cli, env, preferences, sentinel

    def cleanup(cli, env, tray, autostart=False):
        # A cooperative stop failure must not replace the scenario's first
        # exception. Never terminate a process or bypass native ownership.
        failing = sys.exc_info()[0] is not None
        try:
            if tray is not None and tray.poll() is None:
                run([cli, "tray", "stop"], env)
                tray.wait(timeout=20)
            if autostart and cli.exists():
                run([cli, "autostart", "disable"], env)
        except Exception:
            if not failing:
                raise
            import traceback
            traceback.print_exc(file=sys.stderr)

    def launch_frontend(cli, env, host, log="private-tray-output.log"):
        command = [str(cli), "tray", "run"] if host == "console" else [str(cli.with_name("mcw-tray.exe"))]
        with (cli.parent / log).open("wb") as output:
            return subprocess.Popen(command, env=env, stdin=subprocess.DEVNULL,
                                    stdout=output, stderr=output)

    def frontend_ready(tray):
        window = user.FindWindowW("MicCamWatchTrayClass", None)
        owner = W.DWORD()
        return (tray.poll() is None and window
                and user.GetWindowThreadProcessId(window, ctypes.byref(owner))
                and owner.value == tray.pid)

    def wait_frontend(cli, env, tray, host):
        deadline = time.monotonic() + 20
        while not frontend_ready(tray):
            if tray.poll() is not None or time.monotonic() >= deadline:
                details = cli.parent / "private-tray-output.readiness.json"
                proof["windows_owned_tray_readiness_failure"] = {
                    "host": host, "exit_code": tray.poll(),
                    "details_file": str(Path("private-windows") / details.relative_to(root))}
                save_proof(args, proof)
                context = {"argv": tray.args, "pid": tray.pid, "exit_code": tray.poll(),
                           "environment": {key: env.get(key) for key in
                                           ("USERPROFILE", "APPDATA", "LOCALAPPDATA", "TEMP")},
                           "settings": str(Path(env["APPDATA"]) / "MicCamWatch/settings.toml"),
                           "data": str(Path(env["LOCALAPPDATA"]) / "MicCamWatch"),
                           "tray_mutex": r"Local\MicCamWatch.Tray"}
                try:
                    if host == "companion" and tray.poll() is not None:
                        # This is diagnosis, not replacement companion coverage.
                        # Wait for exit to avoid contaminating the mutex result.
                        probe = launch_frontend(cli, env, "console", "private-console-tray-output.log")
                        try:
                            probe_deadline = time.monotonic() + 20
                            while not frontend_ready(probe) and probe.poll() is None and time.monotonic() < probe_deadline:
                                time.sleep(.05)
                            context["console_backend"] = {
                                "pid": probe.pid, "exit_code": probe.poll(),
                                "ready": bool(frontend_ready(probe)),
                                "log": "private-console-tray-output.log"}
                        finally:
                            if probe.poll() is None:
                                run([cli, "tray", "stop"], env, allowed=None, timeout=20)
                                try:
                                    probe.wait(timeout=20)
                                except subprocess.TimeoutExpired:
                                    _timed_out_commands.append(probe)
                                    raise
                except Exception:
                    context["diagnostic_failure"] = traceback.format_exc()
                finally:
                    try:
                        # Doctor initializes the same COM/Media Foundation/audio
                        # SDK. Its failure must not prevent the console probe.
                        sdk = run([cli, "doctor", "--json", "--no-color"], env, allowed=None, timeout=20)
                        context["sdk_diagnostic"] = {"exit_code": sdk.returncode,
                                                     "stdout_file": sdk.stdout_file,
                                                     "stderr_file": sdk.stderr_file}
                    except Exception:
                        context["sdk_diagnostic_failure"] = traceback.format_exc()
                    details.write_text(json.dumps(context, indent=2) + "\n", encoding="utf-8")
                raise AssertionError(f"owned native {host} tray did not become ready; see private readiness diagnostics")
            time.sleep(.05)

    import re

    def remove_portable(cli, env):
        accepted = run([cli, "uninstall"], env)
        completion = re.search(r"Completion is recorded in (.+?result\.json)", accepted.stdout)
        require(completion is not None, "accepted deferred operation did not identify its completion record")
        result = Path(completion.group(1))
        require(result.is_relative_to(Path(env["TEMP"])) and result.parent.name.startswith(".mcw-uninstall-"),
                "worker completion record escaped the isolated temporary namespace")
        wait_result(result)

    directory, cli, env, preferences, sentinel = install("successful removal without frontend")
    before = data_snapshot(preferences, sentinel)
    remove_portable(cli, env)
    require(not cli.exists() and not (directory / "mcw-tray.exe").exists()
            and not (directory / ".miccamwatch-portable.json").exists(),
            "deferred removal without a frontend left verified payloads behind")
    require(before == data_snapshot(preferences, sentinel), "removal without a frontend changed retained user data")
    proof["windows_actual_deferred_pair_removal_without_frontend"] = True
    save_proof(args, proof)

    import winreg
    scheduler = Path(os.environ["SystemRoot"]) / "System32/schtasks.exe"

    def run_entry():
        try:
            with winreg.OpenKey(winreg.HKEY_CURRENT_USER, r"Software\Microsoft\Windows\CurrentVersion\Run") as key:
                return winreg.QueryValueEx(key, "MicCamWatch")[0]
        except FileNotFoundError:
            return None

    for host in ("companion", "console"):
        directory, cli, env, preferences, sentinel = install("successful removal " + host)
        before = data_snapshot(preferences, sentinel)
        require(run([scheduler, "/Query", "/TN", "MicCamWatch Tray", "/XML"], env, allowed=None).returncode != 0
                and run_entry() is None, "pre-existing autostart must remain untouched")
        run([cli, "autostart", "enable"], env)
        require(run([cli, "autostart", "status"], env).returncode == 0, "owned native autostart was not enabled")
        tray = launch_frontend(cli, env, host)
        try:
            wait_frontend(cli, env, tray, host)
            remove_portable(cli, env)
            tray.wait(timeout=20)
            require(tray.returncode == 0, "cooperatively removed native frontend exited unsuccessfully")
            require(not cli.exists() and not (directory / "mcw-tray.exe").exists()
                    and not (directory / ".miccamwatch-portable.json").exists(), "verified pair/receipt survived successful worker removal")
            require(before == data_snapshot(preferences, sentinel), "uninstall changed retained settings/history/policy")
            require(run([scheduler, "/Query", "/TN", "MicCamWatch Tray", "/XML"], env, allowed=None).returncode != 0
                    and run_entry() is None, "owned native autostart survived removal")
            proof[f"windows_actual_deferred_pair_and_owned_{host}_frontend_removal"] = True
            save_proof(args, proof)
        finally:
            cleanup(cli, env, tray, autostart=True)
    proof["windows_actual_deferred_pair_and_owned_frontend_removal"] = True

    # Stop the actual worker before its first instruction, reserve the same
    # Windows byte-range lock, then let production preflight run normally.
    from windows_uninstall_lock import busy_worker
    directory, cli, env, preferences, sentinel = install("contended worker preflight")
    run([cli, "autostart", "enable"], env)
    tray = launch_frontend(cli, env, "companion")
    try:
        wait_frontend(cli, env, tray, "companion")
        task_before = run([scheduler, "/Query", "/TN", "MicCamWatch Tray", "/XML"], env, allowed=None)
        registration_before = task_before.returncode, task_before.stdout, run_entry()
        before = sha(cli), sha(directory / "mcw-tray.exe"), data_snapshot(preferences, sentinel)
        busy_worker(cli, env, directory / "private-busy-worker-output")
        task_after = run([scheduler, "/Query", "/TN", "MicCamWatch Tray", "/XML"], env, allowed=None)
        require(tray.poll() is None and registration_before == (task_after.returncode, task_after.stdout, run_entry()),
                "refused worker changed the actual frontend or autostart")
        require(before == (sha(cli), sha(directory / "mcw-tray.exe"), data_snapshot(preferences, sentinel)),
                "refused worker changed payloads or retained data")
        proof["windows_busy_worker_truthful_refusal_preserves_live_frontend_and_autostart"] = True
    finally:
        cleanup(cli, env, tray, autostart=True)

    for case in ("modified companion", "unknown receipt", "linked target", "requested protection"):
        directory, cli, env, preferences, sentinel = install(case)
        receipt = directory / ".miccamwatch-portable.json"
        if case == "modified companion":
            with (directory / "mcw-tray.exe").open("ab") as output:
                output.write(b"outside producer ownership\n")
        elif case == "unknown receipt":
            receipt.write_text('{"format":999}', encoding="utf-8")
        elif case == "linked target":
            os.link(directory / "mcw-tray.exe", directory / "foreign hardlink")
        else:
            preferences.write_text(preferences.read_text().replace("mute_on_lock = false", "mute_on_lock = true"))
        before = {path.name: sha(path) for path in (cli, directory / "mcw-tray.exe", receipt, preferences, sentinel, preferences.with_name("policy.toml"))}
        result = run([cli, "uninstall"], env, allowed=None)
        require(result.returncode != 0, f"unsafe native removal accepted: {case}")
        require(before == {path.name: sha(path) for path in (cli, directory / "mcw-tray.exe", receipt, preferences, sentinel, preferences.with_name("policy.toml"))}, f"refusal changed installation or protection: {case}")
        proof["windows_refuses_" + case.replace(" ", "_")] = True

    # Actual native zero-input protection ownership, not a mocked flag. Never
    # activate controls on a runner with physical/native capture devices.
    directory, cli, env, preferences, sentinel = install("native protection owner")
    observation = json.loads(run([cli, "status", "--json", "--no-color"], env, allowed=(0, 1, 2)).stdout)["protection"]
    require(observation["microphone_error"] is None and observation["microphone"]["endpoint_count"] == 0,
            "native ownership fixture requires independently observed zero input endpoints")
    protected_tray = None
    try:
        run([cli, "mute"], env, allowed=None)
        before = json.loads(run([cli, "status", "--json", "--no-color"], env, allowed=(0, 1, 2)).stdout)["protection"]["microphone"]
        require(before["requested"] and before["service_active"], "real microphone protection owner was not established")
        protected_tray = launch_frontend(cli, env, "companion")
        wait_frontend(cli, env, protected_tray, "companion")
        require(run([cli, "uninstall"], env, allowed=None).returncode != 0, "active native protection did not refuse uninstall")
        after = json.loads(run([cli, "status", "--json", "--no-color"], env, allowed=(0, 1, 2)).stdout)["protection"]["microphone"]
        require(after["requested"] and after["service_active"] and cli.exists(), "uninstall silently released native protection")
        require(protected_tray.poll() is None, "uninstall shut down a protected frontend before refusing")
        proof["windows_actual_native_protection_preserved"] = True
    finally:
        failing = sys.exc_info()[0] is not None
        try:
            run([cli, "unmute"], env)
        except Exception:
            if not failing:
                raise
            import traceback
            traceback.print_exc(file=sys.stderr)
        finally:
            cleanup(cli, env, protected_tray)


def unix_cases(args, root, proof):
    import native_install as producer
    require(os.geteuid() != 0, "Unix fixture requires an ordinary user")
    version = run([args.binary, "--version"], os.environ).stdout.strip().split()[1]
    tag = "v" + version
    machine = platform.machine().lower()
    architecture = "aarch64" if machine in ("aarch64", "arm64") else "x86_64"
    asset = f"miccamwatch-{'linux' if platform.system() == 'Linux' else 'macos'}-{architecture}.tar.gz"
    camera = {}
    if platform.system() == "Linux":
        camera = {"mcw-camera-helper": args.binary.with_name("mcw-camera-helper").read_bytes(),
                  "install-camera-helper.sh": (args.repository / "installer/install-camera-helper.sh").read_bytes(),
                  "com.roman-cuisset.miccamwatch.camera.policy": (args.repository / "installer/com.roman-cuisset.miccamwatch.camera.policy").read_bytes()}
    archive = root / asset
    archive.write_bytes(producer.archive_bytes(args.binary.read_bytes(), camera=camera))
    sums = root / "SHA256SUMS"
    sums.write_text(f"{sha(archive)}  {asset}\n")

    def invoke_installer(sandbox, *flags, env=None):
        argv = ["/bin/sh", args.repository / "installer/install.sh", "--version", tag,
                "--prefix", sandbox.prefix, *flags]
        result = capture(argv, env or sandbox.env, 240, cwd=sandbox.cwd, start_new_session=True)
        # Retain output before cleanup assertions can raise, and do not replace
        # the first installer failure with a secondary staging assertion.
        require(result.returncode == 0, "production installer failed; see private command evidence")
        sandbox.check_installer_cleanup()

    install_count = 0
    def install(name):
        nonlocal install_count
        sandbox = producer.Sandbox(root, name, "bash", "/bin/bash")
        sandbox.env["XDG_STATE_HOME"] = str(sandbox.home / "state")
        # Both Unix tray implementations enforce a 104-byte socket-path limit,
        # including uninstall preflight when no frontend is running. Keep this
        # namespace compact without weakening the adversarial installation path.
        data = root / "d" / str(install_count)
        install_count += 1
        data.mkdir(parents=True, mode=0o700)
        sandbox.env["XDG_DATA_HOME"] = str(data)
        require(len(os.fsencode(data / "MicCamWatch/tray-runtime/control.sock")) < 104,
                "managed Unix fixture socket path is too long")
        env, _ = producer.fixture_environment(sandbox, {
            f"{producer.RELEASE_ROOT}/{tag}/{asset}": str(archive),
            f"{producer.RELEASE_ROOT}/{tag}/SHA256SUMS": str(sums)})
        invoke_installer(sandbox, "--add-path", env=env)
        preferences, sentinel = settings(sandbox.binary, sandbox.env)
        return sandbox, preferences, sentinel

    sandbox, preferences, sentinel = install("real managed removal")
    before = data_snapshot(preferences, sentinel)
    frontend = None
    try:
        if platform.system() == "Darwin":
            with (args.proof.parent / "private-macos-frontend.stdout").open("wb") as output, \
                    (args.proof.parent / "private-macos-frontend.stderr").open("wb") as errors:
                frontend = subprocess.Popen([str(sandbox.binary), "tray", "run"], env=sandbox.env,
                                            stdin=subprocess.DEVNULL, stdout=output, stderr=errors)
            deadline = time.monotonic() + 30
            while run([sandbox.binary, "tray", "status"], sandbox.env, allowed=None, timeout=10).returncode != 0:
                require(frontend.poll() is None and time.monotonic() < deadline, "actual managed Mac frontend did not become ready")
                time.sleep(.05)
        run([sandbox.binary, "autostart", "enable"], sandbox.env)
        entry = (Path(sandbox.env["XDG_CONFIG_HOME"]) / "autostart/com.roman-cuisset.miccamwatch.desktop"
                 if platform.system() == "Linux" else sandbox.home / "Library/LaunchAgents/com.roman-cuisset.miccamwatch.tray.plist")
        autostart_receipt = preferences.parent / "autostart/autostart.json"
        require(entry.is_file() and autostart_receipt.is_file(), "production autostart did not create owned registration evidence")
        if frontend is not None:
            import fcntl
            camera_lock = Path(sandbox.env["XDG_DATA_HOME"]) / "MicCamWatch/macos-controls/camera.lock"
            # IPC readiness precedes the first passive camera observation.
            # Wait for the real frontend to create its ownership lock; never
            # fabricate one and thereby hide missing production polling.
            deadline = time.monotonic() + 20
            while not camera_lock.exists():
                require(frontend.poll() is None and time.monotonic() < deadline,
                        "managed Mac frontend did not initialize passive camera observation")
                time.sleep(.05)
            with camera_lock.open("r+b") as reservation:
                deadline = time.monotonic() + 20
                while True:
                    try:
                        fcntl.flock(reservation, fcntl.LOCK_EX | fcntl.LOCK_NB)
                        break
                    except BlockingIOError:
                        require(frontend.poll() is None and time.monotonic() < deadline, "camera polling did not release its observation lock")
                        time.sleep(.02)
                started = time.monotonic()
                observation = run([sandbox.binary, "camera", "status"], sandbox.env, allowed=None, timeout=5)
                require(observation.returncode != 0 and "native ownership is busy" in observation.stderr
                        and time.monotonic() - started < 5,
                        "passive camera observation did not promptly report unavailable/busy ownership")
                # Hold ownership across multiple actual tray polling intervals.
                time.sleep(5)
                require(frontend.poll() is None and run([sandbox.binary, "tray", "status"], sandbox.env, timeout=10).returncode == 0,
                        "managed Mac frontend stopped servicing IPC while passive camera polling overlapped a reservation")
                fcntl.flock(reservation, fcntl.LOCK_UN)
            proof["macos_actual_managed_frontend_reservation_overlap_remains_responsive"] = True
        run([sandbox.binary, "uninstall"], sandbox.env, timeout=60)
        if frontend is not None:
            frontend.wait(timeout=15)
    except BaseException as error:
        # Persist before cooperative cleanup can fail or replace the exception.
        save_failure(args, proof, error)
        raise
    finally:
        if frontend is not None:
            try:
                if frontend.poll() is None and sandbox.binary.exists():
                    run([sandbox.binary, "tray", "stop"], sandbox.env, allowed=None)
                    frontend.wait(timeout=20)
            except BaseException:
                with (args.proof.parent / "private-macos-cleanup.log").open("a", encoding="utf-8") as output:
                    traceback.print_exc(file=output)
                if proof["first_failure"] is None:
                    raise
            finally:
                (args.proof.parent / "private-macos-frontend.exit").write_text(str(frontend.poll()) + "\n", encoding="utf-8")
    require(not sandbox.binary.exists() and not (sandbox.prefix / ".miccamwatch-install").exists(), "managed executable/receipt survived removal")
    require(not entry.exists() and not autostart_receipt.exists(), "owned autostart survived managed removal")
    require(sandbox.user_tool.read_bytes() == b"user-owned binary content\n", "unrelated prefix payload removed")
    sandbox.removed_path_configs()
    require(before == data_snapshot(preferences, sentinel), "retained settings/history/policy changed")
    # Already removed is representable through the same receipt-owning script.
    invoke_installer(sandbox, "--uninstall")
    proof["unix_actual_managed_payload_receipt_path_removal_and_noop"] = True

    cases = ["modified executable", "linked executable", "unknown restoration", "requested protection", "foreign autostart"]
    if platform.system() == "Darwin":
        cases += ["unknown camera restoration", "requested camera protection"]
    for case in cases:
        sandbox, preferences, sentinel = install(case)
        journal = None
        if case == "modified executable":
            with sandbox.binary.open("ab") as output:
                output.write(b"outside installer receipt\n")
        elif case == "linked executable":
            os.link(sandbox.binary, sandbox.binary.with_name("foreign hardlink"))
        elif case == "requested protection":
            preferences.write_text(preferences.read_text().replace("mute_on_lock = false", "mute_on_lock = true"))
        elif case == "unknown restoration":
            state = (Path(sandbox.env["XDG_STATE_HOME"]) / "miccamwatch" if platform.system() == "Linux"
                     else Path(sandbox.env["XDG_DATA_HOME"]) / "MicCamWatch/macos-controls")
            state.mkdir(parents=True, exist_ok=True, mode=0o700)
            journal = state / ("pipewire-mute.json" if platform.system() == "Linux" else "microphone.json")
            journal.write_bytes(b"corrupt restoration evidence; preserve\n")
            journal.chmod(0o600)
        elif case in ("unknown camera restoration", "requested camera protection"):
            state = Path(sandbox.env["XDG_DATA_HOME"]) / "MicCamWatch/macos-controls"
            state.mkdir(parents=True, exist_ok=True, mode=0o700)
            journal = state / "camera.json"
            if case == "unknown camera restoration":
                journal.write_bytes(b"corrupt camera restoration evidence; preserve\n")
            else:
                identity = "a19a13b1-fbbd-4ecd-8c05-ffb265a0e435"
                journal.write_text(json.dumps({
                    "version": 1, "identifier": "com.roman-cuisset.miccamwatch.camera." + identity,
                    "uuid": identity, "payload_uuid": "900c15c3-5b51-4aaa-b914-0f9f1529510f",
                    "intent": "install"
                }) + "\n", encoding="utf-8")
            journal.chmod(0o600)
        else:
            if platform.system() == "Linux":
                entry = Path(sandbox.env["XDG_CONFIG_HOME"]) / "autostart/com.roman-cuisset.miccamwatch.desktop"
            else:
                entry = sandbox.home / "Library/LaunchAgents/com.roman-cuisset.miccamwatch.tray.plist"
            entry.parent.mkdir(parents=True, exist_ok=True)
            entry.write_bytes(b"external registration; do not adopt or remove\n")
        receipt = sandbox.prefix / ".miccamwatch-install"
        before = sha(sandbox.binary), {path.name: path.read_bytes() for path in receipt.iterdir()}, data_snapshot(preferences, sentinel)
        restoration = journal.read_bytes() if journal is not None else None
        require(run([sandbox.binary, "uninstall"], sandbox.env, allowed=None).returncode != 0, f"unsafe managed removal accepted: {case}")
        require(before == (sha(sandbox.binary), {path.name: path.read_bytes() for path in receipt.iterdir()}, data_snapshot(preferences, sentinel)), f"refusal lost ownership/protection/data: {case}")
        require(journal is None or journal.read_bytes() == restoration, f"refusal changed restoration evidence: {case}")
        proof["unix_refuses_" + case.replace(" ", "_")] = True


def cargo_case(args, root, proof):
    install = args.cargo_install.resolve(strict=True)
    require(install != args.repository and install.name == "mcw-cargo-uninstall",
            "Cargo removal is restricted to the workflow's dedicated installation root")
    binary = install / "bin" / ("mcw.exe" if platform.system() == "Windows" else "mcw")
    require(binary.is_file() and (install / ".crates2.json").is_file(),
            "Workflow did not produce a genuine Cargo installation")
    env = environment(root / "cargo data")
    preferences, history = settings(binary, env)
    if platform.system() == "Windows":
        # Cargo ownership takes precedence even after a verified portable
        # updater produced a pair receipt in this manager-owned directory.
        receipt = run([binary, "__portable-receipt"], env).stdout
        (binary.parent / ".miccamwatch-portable.json").write_text(receipt, encoding="utf-8")
    before = sha(binary), (install / ".crates2.json").read_bytes(), data_snapshot(preferences, history)
    delegated = run([binary, "uninstall"], env, allowed=(2,))
    require("cargo uninstall --root" in delegated.stderr and str(install) in delegated.stderr,
            "Cargo delegation did not identify the exact root")
    require(before == (sha(binary), (install / ".crates2.json").read_bytes(), data_snapshot(preferences, history)),
            "MCW mutated manager-owned payload, metadata or retained data")
    run(["cargo", "uninstall", "--root", install, "miccamwatch"], os.environ)
    require(not binary.exists() and before[2] == data_snapshot(preferences, history),
            "Actual Cargo procedure did not remove its binary while retaining data")
    proof["genuine_cargo_manager_delegation_and_manager_removal"] = True


def msi_case(args, root, proof):
    import re
    require(args.msi is not None and args.msi.is_file(), "Workflow did not build the genuine MSI")
    home = root / "MSI manager"
    env = environment(home)
    install = home / "MSI owned exact directory"
    run(["msiexec.exe", "/i", args.msi.resolve(), f"INSTALLFOLDER={install}", "/qn", "/norestart"], os.environ)
    binary = install / "mcw.exe"
    require(binary.is_file(), "MSI did not install its actual payload")
    preferences, history = settings(binary, env)
    before = sha(binary), data_snapshot(preferences, history)
    delegated = run([binary, "uninstall"], env, allowed=(2,))
    product = re.search(r"msiexec\.exe /x (\{[0-9A-Fa-f-]{36}\})", delegated.stderr)
    require(product is not None, "MCW did not identify the exact registered MSI product")
    require(before == (sha(binary), data_snapshot(preferences, history)),
            "Manager delegation changed MSI payload or user data")
    run(["msiexec.exe", "/x", product.group(1), "/qn", "/norestart"], os.environ)
    require(not binary.exists() and before[1] == data_snapshot(preferences, history),
            "Actual MSI procedure did not remove its payload while retaining user data")
    proof["genuine_msi_exact_product_delegation_and_manager_removal"] = True


def main():
    global _evidence_directory
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--repository", type=Path, required=True)
    parser.add_argument("--cargo-install", type=Path, required=True)
    parser.add_argument("--msi", type=Path)
    parser.add_argument("--proof", type=Path, required=True)
    args = parser.parse_args()
    require(os.environ.get("GITHUB_ACTIONS") == "true" and os.environ.get("RUNNER_ENVIRONMENT") == "github-hosted",
            "this destructive fixture is restricted to disposable GitHub-hosted runners")
    args.proof = args.proof.resolve()
    args.proof.parent.mkdir(parents=True, exist_ok=True)
    _evidence_directory = args.proof.parent
    proof = {"platform": platform.system(), "uses_real_native_artifacts": True,
             "data_preserved": True, "installed_user_application_untouched": True,
             "status": "running", "first_failure": None}
    save_proof(args, proof)
    root = None
    try:
        args.binary = args.binary.resolve()
        args.repository = args.repository.resolve()
        # Keep macOS sockets independent of the hosted runner's long TMPDIR.
        root = Path(tempfile.mkdtemp(prefix="mu-" if platform.system() == "Darwin" else "mcw-uninstall-native-",
                                     dir="/tmp" if platform.system() == "Darwin" else None)).resolve()
        if platform.system() == "Windows":
            windows_cases(args, root, proof)
            msi_case(args, root, proof)
        else:
            unix_cases(args, root, proof)
        cargo_case(args, root, proof)
        # Only complete success proves that native consumers/workers and all
        # cooperative cleanup finished. Never unlink live sockets/state on error.
        shutil.rmtree(root)
        proof["status"] = "passed"
        save_proof(args, proof)
    except BaseException as error:
        save_failure(args, proof, error)
        if root is not None:
            preserved = {"path": str(root), "preserved": root.exists(),
                         "reason": "fixture failed; no automatic sandbox deletion",
                         "timed_out_commands": [{"argv": process.args, "pid": process.pid,
                                                  "exit_code": process.poll()}
                                                 for process in _timed_out_commands]}
            (args.proof.parent / "private-sandbox.json").write_text(json.dumps(preserved, indent=2) + "\n", encoding="utf-8")
        raise
    print("PASS real installation-scoped removal, preservation and safety refusals")


if __name__ == "__main__":
    main()
