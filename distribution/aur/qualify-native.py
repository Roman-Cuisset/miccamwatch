#!/usr/bin/env python3
"""Real makepkg/pacman and private PipeWire/PTY qualification; disposable Arch only.

Root runs only package-manager/build orchestration. All mcw/PipeWire probes run as
an ordinary user. No hardware session manager, capture, camera mutation, helper
bootstrap, Polkit registration, autostart enable or curl installer is executed.
See README.md and docs/next_projects.md for package ownership/publication gates.
"""
import argparse
import errno
import fcntl
import hashlib
import json
import os
from pathlib import Path
import platform
import pty
import pwd
import re
import select
import shutil
import signal
import struct
import subprocess
import sys
import termios
import time


USER = "mcw-build"
PRIVILEGED = (
    Path("/usr/local/libexec/miccamwatch"),
    Path("/usr/share/polkit-1/actions/com.roman-cuisset.miccamwatch.camera.policy"),
    Path("/var/lib/miccamwatch"),
    Path("/usr/.miccamwatch-install"),
)
PAYLOAD = Path("/usr/share/miccamwatch/linux-camera")
VERSION = "0.17.0"


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def record(proof, name, command, allowed=(0,), timeout=60, **kwargs):
    result = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                            timeout=timeout, **kwargs)
    (proof / (name + ".stdout")).write_bytes(result.stdout)
    (proof / (name + ".stderr")).write_bytes(result.stderr)
    (proof / (name + ".exit")).write_text(str(result.returncode) + "\n")
    require(result.returncode in allowed,
            f"{name}: exit {result.returncode}: {result.stderr.decode(errors='replace')}")
    return result


def protected_absent():
    for path in PRIVILEGED:
        require(not path.exists() and not path.is_symlink(), f"Unexpected privileged/receipt path: {path}")


def native_tui(proof):
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 140, 0, 0))

    def terminal_owner():
        os.setsid()
        fcntl.ioctl(0, termios.TIOCSCTTY, 0)

    process = None
    output = bytearray()

    def drain(seconds):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            if select.select([master], [], [], .1)[0]:
                try:
                    chunk = os.read(master, 65536)
                except OSError as error:
                    if error.errno == errno.EIO:
                        return
                    raise
                if not chunk:
                    return
                output.extend(chunk)

    try:
        process = subprocess.Popen(["mcw", "--lang", "en", "top"], stdin=slave,
                                   stdout=slave, stderr=slave, preexec_fn=terminal_owner)
        os.close(slave)
        slave = None
        drain(6)
        require(process.poll() is None, "Native TUI exited before keyboard input")
        require(b"\x1b[?1049h" in output, "TUI did not enter the native alternate screen")
        require(b"MicCamWatch" in output or b"miccamwatch" in output,
                "TUI did not render the application")
        # Q is the only key sent: no mute/camera/kill/autostart controls.
        os.write(master, b"Q")
        drain(2)
        require(process.wait(timeout=15) == 0, "Native TUI did not exit cleanly on Q")
        require(b"\x1b[?1049l" in output, "TUI did not restore the native screen")
    finally:
        (proof / "tui.pty").write_bytes(output)
        if process is not None and process.poll() is None:
            os.killpg(process.pid, signal.SIGTERM)
            process.wait(timeout=15)
        if slave is not None:
            os.close(slave)
        os.close(master)


def smoke(args):
    require(os.getuid() != 0 and os.getuid() == os.geteuid(), "Smoke must be an ordinary user")
    home = args.home.resolve(strict=True)
    runtime = home / "run"
    runtime.mkdir(mode=0o700, exist_ok=True)
    os.environ.update(HOME=str(home), XDG_CONFIG_HOME=str(home / "config"),
                      XDG_DATA_HOME=str(home / "data"), XDG_STATE_HOME=str(home / "state"),
                      XDG_CACHE_HOME=str(home / "cache"), XDG_RUNTIME_DIR=str(runtime),
                      PIPEWIRE_RUNTIME_DIR=str(runtime), PIPEWIRE_REMOTE="pipewire-0",
                      TERM="xterm-256color", LANG="C.UTF-8", PATH="/usr/bin:/bin")
    proof = args.proof.resolve()
    proof.mkdir(parents=True, exist_ok=True)
    protected_absent()
    require(shutil.which("mcw") == "/usr/bin/mcw", "Installed mcw is not available through native PATH")
    require(record(proof, "version", ["mcw", "--version"]).stdout.decode().strip() == f"mcw {VERSION}",
            "Installed CLI version differs from source release")
    helper = json.loads(record(proof, "helper-protocol", [str(PAYLOAD / "mcw-camera-helper"),
                                                        "--protocol-version"]).stdout)
    require(helper == {"protocol": 1, "version": VERSION}, "Reviewable helper protocol/version mismatch")
    for filename in ("install-camera-helper.sh", "com.roman-cuisset.miccamwatch.camera.policy"):
        require((PAYLOAD / filename).is_file(), f"Missing reviewable material: {filename}")

    log = (proof / "pipewire.log").open("wb")
    # A real daemon with a private socket, no WirePlumber/ALSA/V4L2 discovery.
    daemon = subprocess.Popen(["pipewire"], stdout=log, stderr=log, start_new_session=True)
    try:
        deadline = time.monotonic() + 20
        while not (runtime / "pipewire-0").exists():
            require(daemon.poll() is None, "Private PipeWire exited before creating its socket")
            require(time.monotonic() < deadline, "Private PipeWire socket timed out")
            time.sleep(.1)
        graph = json.loads(record(proof, "pw-dump", ["pw-dump", "--no-colors"]).stdout)
        require(isinstance(graph, list) and any("Core" in row.get("type", "") for row in graph),
                "Real private PipeWire graph has no core")
        require(not any(row.get("type", "").startswith("PipeWire:Interface:Device") for row in graph),
                "Private graph unexpectedly contains discovered hardware devices")
        status = json.loads(record(proof, "status", ["mcw", "status", "--json"], (0, 1, 2)).stdout)
        require(any(row["collector"] == "pipewire_audio" and row["state"] == "healthy"
                    for row in status["collectors"]), "Native PipeWire audio collector is not healthy")
        require(not status["accesses"], "Empty private graph unexpectedly reports capture access")
        checks = json.loads(record(proof, "doctor", ["mcw", "doctor", "--json"], (0, 2)).stdout)
        require(checks and all(row.get("detail") and row["status"] in ("ok", "warning", "error")
                               for row in checks), "Native diagnostics have invalid output")
        json.loads(record(proof, "devices", ["mcw", "devices", "--json"], (0, 2)).stdout)
        record(proof, "camera-read-only", ["mcw", "camera", "status"], (0, 2))
        record(proof, "mute-read-only", ["mcw", "mute", "--status"], (0, 2))
        record(proof, "autostart-read-only", ["mcw", "autostart", "status"], (0, 1, 2))
        history = record(proof, "history-path", ["mcw", "history", "path"]).stdout.decode().strip()
        require(history == str(home / "data/MicCamWatch/history.jsonl"), "History path ignores XDG data home")
        native_tui(proof)
        # Trace the genuine installed CLI, not a replacement curl/pw-dump command.
        refusal = record(proof, "update-refusal", ["strace", "-f", "-e", "trace=network,execve",
                         "-o", str(proof / "update.strace"), "mcw", "update"], (1, 2))
        trace = (proof / "update.strace").read_text()
        require(not re.search(r"\b(?:socket|connect)\([^\n]*AF_INET", trace),
                "Package-owned mcw update attempted internet networking before refusal")
        require(not re.search(r'execve\("[^"\n]*/curl"', trace),
                "Package-owned mcw update invoked the network updater")
    finally:
        if daemon.poll() is None:
            os.killpg(daemon.pid, signal.SIGTERM)
        daemon.wait(timeout=15)
        log.close()
    protected_absent()


def qualify(args):
    require(os.getuid() == 0, "Package qualification requires container root for pacman only")
    require(args.disposable and Path("/.dockerenv").is_file(),
            "Refusing non-container environment; use the documented disposable Docker invocation")
    require(platform.machine() == "x86_64" and Path("/etc/arch-release").is_file(),
            "Native Arch x86_64 is required")
    require(not list(Path("/dev").glob("video*")) and not Path("/dev/snd").exists(),
            "Hardware devices must not be exposed to this qualification container")
    protected_absent()
    require(shutil.which("mcw") is None, "Fresh Arch container already contains mcw")
    proof = args.proof.resolve()
    proof.mkdir(parents=True, exist_ok=True)
    report = {"schema": 1, "status": "failed", "source_version": VERSION,
              "scope": "native_arch_x86_64_source_package_private_pipewire_read_only_cli_pty",
              "completed": [], "hardware_capture": False, "privileged_bootstrap": False,
              "publication": "not_submitted"}
    try:
        record(proof, "arch-dependencies", ["pacman", "-Syu", "--noconfirm", "--needed",
               "base-devel", "rust", "clang", "pkgconf", "libpipewire", "pipewire",
               "python", "dbus", "strace"], timeout=1800)
        record(proof, "native-environment", ["pacman", "-Q"])
        record(proof, "rust-version", ["rustc", "--version"])
        record(proof, "spa-version", ["pkg-config", "--modversion", "libspa-0.2", "libpipewire-0.3"])
        record(proof, "pw-dump-owner", ["pacman", "-Qo", "/usr/bin/pw-dump"])
        record(proof, "create-build-user", ["useradd", "--create-home", USER])
        account = pwd.getpwnam(USER)
        build = Path(account.pw_dir) / "package"
        build.mkdir()
        for filename in ("PKGBUILD", ".SRCINFO"):
            shutil.copy2(Path(__file__).resolve().parent / filename, build / filename)
            os.chown(build / filename, account.pw_uid, account.pw_gid)
        os.chown(build, account.pw_uid, account.pw_gid)

        def as_user(command):
            return ["runuser", "-u", USER, "--", "env", "HOME=" + account.pw_dir,
                    "PATH=/usr/bin:/bin", "LANG=C.UTF-8", *command]

        metadata = record(proof, "generated-srcinfo", as_user(["makepkg", "--printsrcinfo"]), cwd=build).stdout
        require(metadata == (build / ".SRCINFO").read_bytes(),
                ".SRCINFO is stale; regenerate with native makepkg --printsrcinfo")
        record(proof, "clean-native-build", as_user(["makepkg", "--cleanbuild", "--noconfirm"]),
               cwd=build, timeout=3600)
        report["completed"].append("clean_native_locked_source_build_and_srcinfo")

        def package_path(label):
            paths = record(proof, label, as_user(["makepkg", "--packagelist"]), cwd=build).stdout.decode().splitlines()
            require(len(paths) == 1, "Expected one source-package output, no binary/debug variant")
            path = Path(paths[0])
            require(path.is_file(), f"Native package artifact missing: {path}")
            artifacts = proof / "packages"
            artifacts.mkdir(exist_ok=True)
            shutil.copy2(path, artifacts / path.name)
            return path

        original = package_path("original-package")
        contents = record(proof, "archive-contents", ["bsdtar", "-tf", str(original)]).stdout.decode().splitlines()
        allowed = {"usr/bin/mcw", "usr/share/licenses/miccamwatch/LICENSE",
                   "usr/share/doc/miccamwatch/README.md",
                   "usr/share/miccamwatch/linux-camera/mcw-camera-helper",
                   "usr/share/miccamwatch/linux-camera/install-camera-helper.sh",
                   "usr/share/miccamwatch/linux-camera/com.roman-cuisset.miccamwatch.camera.policy",
                   ".PKGINFO", ".BUILDINFO", ".MTREE"}
        members = {name.removeprefix("./") for name in contents if not name.endswith("/")}
        require(members == allowed, f"Unexpected package payload/hooks: {members.symmetric_difference(allowed)}")
        session = Path(account.pw_dir) / "session-home"
        session.mkdir(mode=0o700)
        settings = session / "config/MicCamWatch/settings.toml"
        settings.parent.mkdir(parents=True)
        settings.write_text("notifications_enabled = false\nsound_enabled = false\nhistory_enabled = false\n"
                            "show_ready = true\nprofile = 'balanced'\nmute_on_lock = false\n"
                            "block_camera_on_lock = false\nrestore_on_unlock = false\n")
        history = session / "data/MicCamWatch/history.jsonl"
        history.parent.mkdir(parents=True)
        # Preservation fixture only: not a claim that native capture was observed.
        history.write_text('{"qualification_fixture":"preexisting_user_history_bytes"}\n')
        registration = session / "config/autostart/com.roman-cuisset.miccamwatch.desktop"
        registration.parent.mkdir(parents=True)
        registration.write_text("[Desktop Entry]\nType=Application\nName=External disabled registration\n"
                                "Exec=/usr/bin/mcw tray run\nHidden=true\n")
        preserved = {path: path.read_bytes() for path in (settings, history, registration)}
        for path in (session, *session.rglob("*")):
            os.chown(path, account.pw_uid, account.pw_gid)
        os.chown(proof, account.pw_uid, account.pw_gid)

        def unchanged():
            protected_absent()
            for path, data in preserved.items():
                require(path.read_bytes() == data, f"User settings/history/autostart changed: {path}")
            require(not (session / "config/MicCamWatch/autostart/autostart.json").exists(),
                    "Package claimed ownership of external autostart intent")

        def native(label):
            record(proof, label, as_user(["dbus-run-session", "--", "python", str(Path(__file__).resolve()),
                   "--smoke", "--home", str(session), "--proof", str(proof / label)]), timeout=180)
            unchanged()

        record(proof, "install", ["pacman", "-U", "--noconfirm", str(original)], timeout=180)
        record(proof, "binary-owner", ["pacman", "-Qo", "/usr/bin/mcw"])
        record(proof, "installed-files", ["pacman", "-Ql", "miccamwatch"])
        record(proof, "installed-integrity", ["pacman", "-Qkk", "miccamwatch"])
        native("installed-native")
        report["completed"].append("pacman_install_private_pipewire_cli_pty_update_refusal")
        record(proof, "idempotent-reinstall", ["pacman", "-U", "--noconfirm", str(original)], timeout=180)
        native("reinstalled-native")
        report["completed"].append("idempotent_native_reinstall_preserves_user_files")
        # A genuine rebuild of the same immutable source with pkgrel incremented
        # exercises pacman's upgrade transaction, not fake binaries/version output.
        recipe = build / "PKGBUILD"
        text = recipe.read_text()
        require(text.count("pkgrel=1\n") == 1, "Qualification expects published pkgrel=1")
        recipe.write_text(text.replace("pkgrel=1\n", "pkgrel=2\n", 1))
        record(proof, "native-upgrade-build", as_user(["makepkg", "--force", "--noextract", "--noconfirm"]),
               cwd=build, timeout=3600)
        upgrade = package_path("upgrade-package")
        record(proof, "upgrade", ["pacman", "-U", "--noconfirm", str(upgrade)], timeout=180)
        require(record(proof, "upgraded-version", ["pacman", "-Q", "miccamwatch"]).stdout.decode().strip()
                == f"miccamwatch {VERSION}-2", "Pacman did not execute the real pkgrel upgrade")
        record(proof, "upgraded-integrity", ["pacman", "-Qkk", "miccamwatch"])
        native("upgraded-native")
        report["completed"].append("real_pkgrel_upgrade_preserves_user_files_and_disabled_external_autostart")
        record(proof, "remove", ["pacman", "-R", "--noconfirm", "miccamwatch"], timeout=180)
        unchanged()
        require(shutil.which("mcw") is None and not PAYLOAD.exists(), "Package-owned payload survived removal")
        record(proof, "removed-package-query", ["pacman", "-Q", "miccamwatch"], (1,))
        report["completed"].append("pacman_remove_preserves_user_files_no_root_helper_policy_or_receipt")
        report["packages"] = {path.name: hashlib.sha256(path.read_bytes()).hexdigest()
                              for path in (proof / "packages").iterdir()}
        report["status"] = "passed"
    except BaseException as error:
        report["error"] = str(error)
        raise
    finally:
        (proof / "qualification.json").write_text(json.dumps(report, indent=2) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--disposable", action="store_true", help="Authorize package-manager mutations in a fresh isolated Arch container")
    parser.add_argument("--proof", type=Path, required=True)
    parser.add_argument("--smoke", action="store_true", help=argparse.SUPPRESS)
    parser.add_argument("--home", type=Path, help=argparse.SUPPRESS)
    args = parser.parse_args()
    if args.smoke:
        require(args.home is not None, "Internal smoke requires an isolated home")
        smoke(args)
    else:
        qualify(args)


if __name__ == "__main__":
    main()
