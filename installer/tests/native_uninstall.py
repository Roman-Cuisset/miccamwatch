#!/usr/bin/env python3
"""Real consumer uninstall contracts; disposable hosted runners only.

Unix uses the production installer to produce ownership/PATH receipts around
real native artifacts (the existing controlled transport, never a fake mcw).
Windows uses the exact packaging receipt producer and real CLI/tray processes.
No path, user, PID, device or application-data contents enter the proof report.
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


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def run(argv, env, allowed=(0,), timeout=180):
    result = subprocess.run([str(arg) for arg in argv], env=env, stdin=subprocess.DEVNULL,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, timeout=timeout)
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
    # Same ordinary-token requirement as the real command, not a test bypass.
    from ctypes import wintypes as W
    token, needed, elevation = W.HANDLE(), W.DWORD(), W.DWORD()
    native = ctypes.WinDLL("advapi32", use_last_error=True)
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.GetCurrentProcess.restype = W.HANDLE
    require(native.OpenProcessToken(kernel.GetCurrentProcess(), 8, ctypes.byref(token)), "cannot inspect fixture token")
    try:
        require(native.GetTokenInformation(token, 20, ctypes.byref(elevation), 4, ctypes.byref(needed)), "cannot inspect elevation")
        require(elevation.value == 0, "Windows fixture requires a real ordinary token, not elevated CI")
    finally:
        kernel.CloseHandle(token)
    user = ctypes.WinDLL("user32", use_last_error=True)
    user.FindWindowW.argtypes = [W.LPCWSTR, W.LPCWSTR]
    user.FindWindowW.restype = W.HWND
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

    directory, cli, env, preferences, sentinel = install("successful removal")
    before = data_snapshot(preferences, sentinel)
    import winreg
    scheduler = Path(os.environ["SystemRoot"]) / "System32/schtasks.exe"

    def run_entry():
        try:
            with winreg.OpenKey(winreg.HKEY_CURRENT_USER, r"Software\Microsoft\Windows\CurrentVersion\Run") as key:
                return winreg.QueryValueEx(key, "MicCamWatch")[0]
        except FileNotFoundError:
            return None

    require(run([scheduler, "/Query", "/TN", "MicCamWatch Tray", "/XML"], env, allowed=None).returncode != 0
            and run_entry() is None, "pre-existing autostart must remain untouched")
    run([cli, "autostart", "enable"], env)
    require(run([cli, "autostart", "status"], env).returncode == 0, "owned native autostart was not enabled")
    tray = subprocess.Popen([str(directory / "mcw-tray.exe")], env=env,
                            stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        deadline = time.monotonic() + 20
        while not user.FindWindowW("MicCamWatchTrayClass", None):
            require(tray.poll() is None and time.monotonic() < deadline, "owned native tray did not become ready")
            time.sleep(.05)
        import re
        accepted = run([cli, "uninstall"], env)
        completion = re.search(r"Completion is recorded in (.+?result\.json)", accepted.stdout)
        require(completion is not None, "accepted deferred operation did not identify its completion record")
        result = Path(completion.group(1))
        require(result.is_relative_to(Path(env["TEMP"])) and result.parent.name.startswith(".mcw-uninstall-"),
                "worker completion record escaped the isolated temporary namespace")
        wait_result(result)
        tray.wait(timeout=20)
        require(not cli.exists() and not (directory / "mcw-tray.exe").exists()
                and not (directory / ".miccamwatch-portable.json").exists(), "verified pair/receipt survived successful worker removal")
        require(before == data_snapshot(preferences, sentinel), "uninstall changed retained settings/history/policy")
        require(run([scheduler, "/Query", "/TN", "MicCamWatch Tray", "/XML"], env, allowed=None).returncode != 0
                and run_entry() is None, "owned native autostart survived removal")
        proof["windows_actual_deferred_pair_and_owned_frontend_removal"] = True
    finally:
        if tray.poll() is None:
            # Only our exact retained process gets the ordinary production stop.
            run([cli, "tray", "stop"], env, allowed=None)
            tray.wait(timeout=20)
        if cli.exists():
            run([cli, "autostart", "disable"], env, allowed=None)

    # Stop the actual worker before its first instruction, reserve the same
    # Windows byte-range lock, then let production preflight run normally.
    from windows_uninstall_lock import busy_worker
    directory, cli, env, preferences, sentinel = install("contended worker preflight")
    run([cli, "autostart", "enable"], env)
    tray = subprocess.Popen([str(directory / "mcw-tray.exe")], env=env,
                            stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        deadline = time.monotonic() + 20
        while not user.FindWindowW("MicCamWatchTrayClass", None):
            require(tray.poll() is None and time.monotonic() < deadline, "contention fixture frontend did not become ready")
            time.sleep(.05)
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
        if tray.poll() is None:
            run([cli, "tray", "stop"], env, allowed=None)
            tray.wait(timeout=20)
        run([cli, "autostart", "disable"], env, allowed=None)

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
        protected_tray = subprocess.Popen([str(directory / "mcw-tray.exe")], env=env,
                                          stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        deadline = time.monotonic() + 20
        while not user.FindWindowW("MicCamWatchTrayClass", None):
            require(protected_tray.poll() is None and time.monotonic() < deadline,
                    "protected native frontend did not become ready")
            time.sleep(.05)
        require(run([cli, "uninstall"], env, allowed=None).returncode != 0, "active native protection did not refuse uninstall")
        after = json.loads(run([cli, "status", "--json", "--no-color"], env, allowed=(0, 1, 2)).stdout)["protection"]["microphone"]
        require(after["requested"] and after["service_active"] and cli.exists(), "uninstall silently released native protection")
        require(protected_tray.poll() is None, "uninstall shut down a protected frontend before refusing")
        proof["windows_actual_native_protection_preserved"] = True
    finally:
        run([cli, "unmute"], env, allowed=None)
        if protected_tray is not None and protected_tray.poll() is None:
            run([cli, "tray", "stop"], env, allowed=None)
            protected_tray.wait(timeout=20)


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

    def install(name):
        sandbox = producer.Sandbox(root, name, "bash", "/bin/bash")
        sandbox.env["XDG_STATE_HOME"] = str(sandbox.home / "state")
        env, _ = producer.fixture_environment(sandbox, {
            f"{producer.RELEASE_ROOT}/{tag}/{asset}": str(archive),
            f"{producer.RELEASE_ROOT}/{tag}/SHA256SUMS": str(sums)})
        sandbox.invoke(args.repository / "installer/install.sh", "--add-path", env=env, version=tag)
        preferences, sentinel = settings(sandbox.binary, sandbox.env)
        return sandbox, preferences, sentinel

    sandbox, preferences, sentinel = install("real managed removal")
    before = data_snapshot(preferences, sentinel)
    frontend = None
    if platform.system() == "Darwin":
        frontend = subprocess.Popen([str(sandbox.binary), "tray", "run"], env=sandbox.env,
                                    stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
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
    try:
        run([sandbox.binary, "uninstall"], sandbox.env, timeout=60)
        if frontend is not None:
            frontend.wait(timeout=15)
    finally:
        if frontend is not None and frontend.poll() is None and sandbox.binary.exists():
            run([sandbox.binary, "tray", "stop"], sandbox.env, allowed=None)
            frontend.wait(timeout=20)
    require(not sandbox.binary.exists() and not (sandbox.prefix / ".miccamwatch-install").exists(), "managed executable/receipt survived removal")
    require(not entry.exists() and not autostart_receipt.exists(), "owned autostart survived managed removal")
    require(sandbox.user_tool.read_bytes() == b"user-owned binary content\n", "unrelated prefix payload removed")
    sandbox.removed_path_configs()
    require(before == data_snapshot(preferences, sentinel), "retained settings/history/policy changed")
    # Already removed is representable through the same receipt-owning script.
    sandbox.invoke(args.repository / "installer/install.sh", "--uninstall", version=tag)
    proof["unix_actual_managed_payload_receipt_path_removal_and_noop"] = True

    for case in ("modified executable", "linked executable", "unknown restoration", "requested protection", "foreign autostart"):
        sandbox, preferences, sentinel = install(case)
        if case == "modified executable":
            with sandbox.binary.open("ab") as output:
                output.write(b"outside installer receipt\n")
        elif case == "linked executable":
            os.link(sandbox.binary, sandbox.binary.with_name("foreign hardlink"))
        elif case == "requested protection":
            preferences.write_text(preferences.read_text().replace("mute_on_lock = false", "mute_on_lock = true"))
        elif case == "unknown restoration":
            state = sandbox.home / ("state/miccamwatch" if platform.system() == "Linux" else "data directory/MicCamWatch/macos-controls")
            state.mkdir(parents=True, exist_ok=True, mode=0o700)
            journal = state / ("pipewire-mute.json" if platform.system() == "Linux" else "microphone.json")
            journal.write_bytes(b"corrupt restoration evidence; preserve\n")
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
        require(run([sandbox.binary, "uninstall"], sandbox.env, allowed=None).returncode != 0, f"unsafe managed removal accepted: {case}")
        require(before == (sha(sandbox.binary), {path.name: path.read_bytes() for path in receipt.iterdir()}, data_snapshot(preferences, sentinel)), f"refusal lost ownership/protection/data: {case}")
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
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--repository", type=Path, required=True)
    parser.add_argument("--cargo-install", type=Path, required=True)
    parser.add_argument("--msi", type=Path)
    parser.add_argument("--proof", type=Path, required=True)
    args = parser.parse_args()
    require(os.environ.get("GITHUB_ACTIONS") == "true" and os.environ.get("RUNNER_ENVIRONMENT") == "github-hosted",
            "this destructive fixture is restricted to disposable GitHub-hosted runners")
    args.binary = args.binary.resolve()
    args.repository = args.repository.resolve()
    args.proof.parent.mkdir(parents=True, exist_ok=True)
    proof = {"platform": platform.system(), "uses_real_native_artifacts": True,
             "data_preserved": True, "installed_user_application_untouched": True}
    with tempfile.TemporaryDirectory(prefix="mcw-uninstall-native-", ignore_cleanup_errors=True) as temporary:
        root = Path(temporary).resolve()
        if platform.system() == "Windows":
            windows_cases(args, root, proof)
            msi_case(args, root, proof)
        else:
            unix_cases(args, root, proof)
        cargo_case(args, root, proof)
    args.proof.write_text(json.dumps(proof, indent=2) + "\n", encoding="utf-8")
    print("PASS real installation-scoped removal, preservation and safety refusals")


if __name__ == "__main__":
    main()
