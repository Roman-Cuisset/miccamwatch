#!/usr/bin/env python3
"""Real brew qualification on a disposable macOS 15+ Aqua host; no devices changed."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import plistlib
import re
import shutil
import signal
import subprocess
import tempfile
import time
import uuid

LABEL = "com.roman-cuisset.miccamwatch.tray"
RELEASE_COMMIT = "a58c8b9f3a967589a576879d6d27bfdbc40f9dff"


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def candidate_formula(path, args):
    text = path.read_text()
    overrides = (args.candidate_source_url, args.candidate_source_sha256,
                 args.candidate_ref, args.candidate_version)
    require(not any(overrides) or all(overrides), "Supply all four candidate source overrides, or none")
    if all(overrides):
        require(re.fullmatch(r"[0-9a-f]{40}", args.candidate_ref), "Candidate ref must be a full immutable commit SHA")
        expected = "https://codeload.github.com/Roman-Cuisset/miccamwatch/tar.gz/" + args.candidate_ref
        require(args.candidate_source_url == expected, "Candidate URL must identify exactly the supplied upstream commit")
        require(re.fullmatch(r"[0-9a-f]{64}", args.candidate_source_sha256), "Candidate SHA256 must be an exact lowercase digest")
        require(re.fullmatch(r"\d+\.\d+\.\d+", args.candidate_version), "Candidate version must match the source package version")
        for key, value in (("url", args.candidate_source_url), ("sha256", args.candidate_source_sha256),
                           ("version", args.candidate_version)):
            text, count = re.subn(r'^  ' + key + r' "[^"]+"$', '  ' + key + ' "' + value + '"', text, flags=re.M)
            require(count == 1, "Formula must declare exactly one " + key)
    version = re.search(r'^  version "([^"]+)"$', text, re.M).group(1)
    return text, version


class Qualification:
    def __init__(self, args):
        self.args = args
        self.proof = args.proof.resolve()
        self.proof.mkdir(parents=True, exist_ok=False)
        self.login_env = os.environ.copy()
        self.root = Path(tempfile.mkdtemp(prefix="mcw-brew-", dir="/tmp")).resolve()
        self.home = self.root / "home"
        self.home.mkdir(mode=0o700)
        self.env = self.environment(self.home)
        self.service = f"gui/{os.getuid()}/{LABEL}"
        self.domain = f"gui/{os.getuid()}"
        self.entry = self.home / "Library/LaunchAgents" / (LABEL + ".plist")
        self.receipt = self.home / "config/MicCamWatch/autostart/autostart.json"
        self.tap = "mcw/qualification-" + uuid.uuid4().hex[:12]
        self.formula = self.tap + "/miccamwatch"
        self.tap_path = None
        self.installed = False
        self.package_reserved = False
        self.login_state = []
        self.children = []
        self.handles = []
        self.sequence = 0
        self.disabled_before = None

    def environment(self, home):
        return dict(os.environ, HOME=str(home), XDG_CONFIG_HOME=str(home / "config"),
                    XDG_DATA_HOME=str(home / "data"), XDG_STATE_HOME=str(home / "state"),
                    HOMEBREW_NO_AUTO_UPDATE="1", HOMEBREW_NO_INSTALL_CLEANUP="1",
                    HOMEBREW_NO_ANALYTICS="1", HOMEBREW_NO_ENV_HINTS="1")

    def run(self, name, command, codes=(0,), env=None, timeout=90):
        self.sequence += 1
        stem = self.proof / f"{self.sequence:03d}-{name}"
        result = subprocess.run([str(item) for item in command], env=env or self.env,
                                capture_output=True, timeout=timeout)
        stem.with_suffix(".stdout").write_bytes(result.stdout)
        stem.with_suffix(".stderr").write_bytes(result.stderr)
        stem.with_suffix(".json").write_text(json.dumps({"argv": [str(item) for item in command],
                                                        "exit": result.returncode}, indent=2) + "\n")
        require(result.returncode in codes, f"{name}: exit {result.returncode}: {result.stderr.decode(errors='replace')}")
        return result

    def brew(self, name, *command, env=None):
        return self.run(name, ["brew", *command], env=env, timeout=3600)

    def mcw(self, name, *command, codes=(0,), env=None):
        return self.run(name, [self.binary, *command], codes=codes, env=env)

    def absent_service(self, name):
        self.run(name, ["launchctl", "print", self.service], codes=(3, 113))

    def disabled(self, name):
        text = self.run(name, ["launchctl", "print-disabled", self.domain]).stdout.decode()
        match = re.search(r'"' + re.escape(LABEL) + r'"\s*=>\s*(true|false)', text)
        return bool(match and match.group(1) == "true")

    def until(self, predicate, description):
        deadline = time.monotonic() + 35
        while time.monotonic() < deadline:
            if predicate():
                return
            time.sleep(0.4)
        raise AssertionError(description)

    def stop_menu(self, env):
        self.mcw("stop-owned-menu", "tray", "stop", codes=(0, 1), env=env)

    def retained(self, snapshot):
        for path, data in snapshot.items():
            require(path.is_file(), f"User state disappeared: {path}")
            current = path.read_bytes()
            retained = current == data if path.name == "settings.toml" else current.startswith(data)
            require(retained, f"User state lost or replaced: {path}")

    def registration(self):
        return {path: path.read_bytes() for path in (self.entry, self.receipt)}

    def ownership(self, name):
        executable = self.binary.resolve(strict=True)
        before = digest(executable)
        refusal_env = dict(self.env, HTTPS_PROXY="http://127.0.0.1:9", HTTP_PROXY="http://127.0.0.1:9",
                           ALL_PROXY="http://127.0.0.1:9", NO_PROXY="")
        result = self.mcw(name, "update", codes=(2,), env=refusal_env)
        require(digest(executable) == before, "Updater altered Homebrew's executable")
        require(not (executable.parent.parent / ".miccamwatch-install").exists(), "Curl ownership receipt appeared in brew keg")
        require(not (self.prefix / ".miccamwatch-install").exists(), "Curl ownership receipt appeared in brew prefix")

    def native_menu(self):
        out = (self.proof / "native-menu.stdout").open("wb")
        err = (self.proof / "native-menu.stderr").open("wb")
        self.handles.extend((out, err))
        process = subprocess.Popen([str(self.binary), "--lang", "en", "tray", "run"], env=self.env,
                                   stdout=out, stderr=err, start_new_session=True)
        self.children.append(process)
        self.until(lambda: process.poll() is None and subprocess.run(
            [str(self.binary), "tray", "status"], env=self.env, capture_output=True, timeout=15).returncode == 0,
            "Packaged embedded AppKit helper did not create its native menu bar")
        self.mcw("native-menu-ready", "tray", "status")
        shot = self.run("native-menu-screenshot", ["screencapture", "-x", self.proof / "native-menu.png"], codes=(0, 1))
        if shot.returncode:
            (self.proof / "screenshot-prerequisite.txt").write_text(
                "Native menu lifecycle passed; screencapture lacks authorization. No TCC permissions were changed.\n")
        self.stop_menu(self.env)
        require(process.wait(timeout=25) == 0, "Owned native menu did not stop cleanly")
        self.mcw("native-menu-stopped", "tray", "status", codes=(1,))

    def execute(self):
        require(platform.system() == "Darwin" and os.getuid() != 0, "Run as a normal user on macOS")
        require(self.args.disposable_machine, "Explicit --disposable-machine is required; this uses a real login LaunchAgent")
        actual = platform.machine()
        require(actual == self.args.architecture, f"Native architecture mismatch: {actual}")
        runtime = self.run("macos-runtime", ["sw_vers", "-productVersion"]).stdout.decode().strip()
        require(int(runtime.split(".")[0]) >= 15, "macOS 15+ is required")
        self.run("macos-sdk", ["xcrun", "--show-sdk-version"])
        self.run("swift-compiler", ["swiftc", "--version"])
        self.run("aqua-session", ["launchctl", "print", self.domain])
        self.absent_service("no-existing-launchagent")
        real_entry = Path(self.login_env["HOME"]) / "Library/LaunchAgents" / (LABEL + ".plist")
        require(not real_entry.exists() and not real_entry.is_symlink(), "Existing login-user LaunchAgent must remain untouched")
        self.disabled_before = self.disabled("disabled-state-before")
        require(not self.disabled_before, "A pre-existing launchctl disabled override must remain untouched")
        self.prefix = Path(self.brew("brew-prefix", "--prefix").stdout.decode().strip())
        self.binary = self.prefix / "bin/mcw"
        require(not self.binary.exists() and not self.binary.is_symlink(), "An existing mcw must remain untouched")
        require(not (self.prefix / "Cellar/miccamwatch").exists(), "An existing Homebrew rack must remain untouched")
        source_dir = Path(__file__).resolve().parent
        if actual == "x86_64":
            # New Intel bottles are unavailable on this native Homebrew tier.
            # Provision genuine package-owned prerequisites from source rather
            # than substituting rustup or inventing Homebrew ownership metadata.
            missing = self.brew("intel-missing-rust-dependencies", "deps", "--include-build",
                                "--missing", "--topological", "rust").stdout.decode().splitlines()
            self.run("intel-native-rust-source-install",
                     ["brew", "install", "--build-from-source", "--formula", *missing, "rust"],
                     timeout=18000)
        formula_text, version = candidate_formula(source_dir / "Formula/miccamwatch.rb", self.args)
        self.brew("create-native-tap", "tap-new", "--no-git", self.tap)
        repository = Path(self.brew("brew-repository", "--repository").stdout.decode().strip())
        self.tap_path = repository / "Library/Taps/mcw" / ("homebrew-" + self.tap.split("/")[1])
        recipe = self.tap_path / "Formula/miccamwatch.rb"
        recipe.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source_dir / "fixtures/miccamwatch-0.16.1.rb", recipe)
        self.package_reserved = True
        self.brew("baseline-install", "install", "--build-from-source", self.formula)
        self.installed = True
        require(self.mcw("baseline-version", "--version").stdout.strip() == b"mcw 0.16.1", "Historical version mismatch")
        self.absent_service("install-did-not-activate")
        require(not self.entry.exists(), "Installation silently enabled autostart")
        self.ownership("baseline-updater-ownership")
        self.mcw("preferences-opt-in", "notifications", "pause", "10080")
        settings = Path(self.mcw("settings-path", "config", "settings-path").stdout.decode().strip())
        history = Path(self.mcw("history-path", "history", "path").stdout.decode().strip())
        history.parent.mkdir(parents=True, exist_ok=True)
        # A retained user-data fixture, not a fabricated hardware/capture event.
        history.write_text('{"qualification_fixture":"preserve-user-history-without-capture"}\n')
        rotated = history.with_suffix(".jsonl.1")
        rotated.write_text('{"qualification_fixture":"preserve-rotated-history"}\n')
        snapshot = {path: path.read_bytes() for path in (settings, history, rotated)}
        for command in (("config", "settings-path"), ("history", "path")):
            directory = Path(self.mcw("login-state-prerequisite", *command, env=self.login_env).stdout.decode().strip()).parent
            require(not directory.exists(), "Existing login-session app state must remain untouched")
            self.login_state.append(directory)
        baseline = self.binary.resolve(strict=True)
        self.run("baseline-physical-executable-ancestry", ["stat", "-f", "%N uid=%u gid=%g mode=%Lp",
                 *reversed(baseline.parent.parents), baseline.parent, baseline])
        # Historical Darwin current_exe retains invocation aliases. Invoke the
        # authentic physical keg, not writable prefix/bin; never patch its source
        # or loosen filesystem permissions to satisfy this prerequisite.
        self.run("baseline-explicit-autostart-enable", [baseline, "autostart", "enable"])
        self.run("baseline-launchagent-loaded", ["launchctl", "print", self.service])
        self.until(lambda: subprocess.run([str(self.binary), "tray", "status"], env=self.login_env,
                                         capture_output=True, timeout=15).returncode == 0,
                   "Historical LaunchAgent failed to start the embedded native menu in the login session")
        original = plistlib.loads(self.entry.read_bytes())
        require("/Cellar/miccamwatch/0.16.1/" in original["ProgramArguments"][0], "Historical fixture did not exercise the versioned Cellar target")
        if self.args.autostart_intent != "loaded":
            self.run("baseline-explicit-bootout", ["launchctl", "bootout", self.domain, self.entry])
            self.stop_menu(self.login_env)
            self.absent_service("baseline-dormant")
        if self.args.autostart_intent == "disabled":
            self.run("baseline-explicit-disabled-intent", ["launchctl", "disable", self.service])
        recipe.write_text(formula_text)
        self.brew("native-upgrade", "upgrade", "--build-from-source", self.formula)
        require(self.mcw("candidate-version", "--version").stdout.decode().strip() == "mcw " + version,
                "Candidate archive's Cargo version does not match the supplied formula version")
        mach_o = self.run("installed-mach-o", ["file", self.binary.resolve()]).stdout.decode()
        require("Mach-O" in mach_o and actual in mach_o, "Brew installed the wrong native executable architecture")
        for name, command in (("cli-help", ("--lang", "en", "--help")), ("tray-help", ("tray", "--help")),
                              ("autostart-help", ("autostart", "--help"))):
            self.mcw(name, *command)
        selected = self.binary.resolve(strict=True)
        updated = plistlib.loads(self.entry.read_bytes())
        require(updated["ProgramArguments"] == [str(selected), "tray", "run"],
                "Upgrade did not refresh owned autostart to the validated physical keg")
        require(updated["MicCamWatchVersion"] == version, "Registration version was not refreshed")
        require(selected != Path(original["ProgramArguments"][0]), "Upgrade kept the historical executable target")
        require(self.disabled("disabled-state-after-upgrade") == (self.args.autostart_intent == "disabled"),
                "Upgrade changed the user's launchctl disabled intent")
        if self.args.autostart_intent == "loaded":
            loaded = self.run("upgraded-loaded-service", ["launchctl", "print", self.service]).stdout
            require(str(selected).encode() in loaded, "Loaded LaunchAgent did not select the refreshed physical target")
        else:
            self.absent_service("upgrade-did-not-activate")
        self.retained(snapshot)
        self.ownership("candidate-updater-ownership")
        registration = self.registration()
        if self.args.autostart_intent == "loaded":
            before_repeat = self.run("loaded-before-no-op", ["launchctl", "print", self.service]).stdout
            repeated_pid = re.search(rb"(?m)^\s*pid = (\d+)$", before_repeat)
            require(repeated_pid is not None, "Loaded native service did not expose its process")
        self.brew("idempotent-upgrade", "upgrade", "--build-from-source", self.formula)
        self.brew("idempotent-postinstall", "postinstall", self.formula)
        require(self.registration() == registration, "Repeated upgrade/postinstall changed owned registration bytes")
        if self.args.autostart_intent == "loaded":
            after_repeat = self.run("loaded-after-no-op", ["launchctl", "print", self.service]).stdout
            repeated_after = re.search(rb"(?m)^\s*pid = (\d+)$", after_repeat)
            require(repeated_after is not None and repeated_after.group(1) == repeated_pid.group(1),
                    "Idempotent postinstall restarted the user's loaded native menu")
        if self.args.autostart_intent != "loaded":
            self.absent_service("repeat-did-not-activate")
        self.brew("remove-old-keg", "cleanup", self.formula)
        require(not Path(original["ProgramArguments"][0]).exists(), "Old Cellar target remained; cleanup scenario was not exercised")
        require(selected.is_file(), "Refreshed LaunchAgent target broke after old-keg cleanup")
        self.brew("formula-native-test", "test", self.formula)
        if self.args.autostart_intent == "loaded":
            self.run("explicit-bootout-before-menu-test", ["launchctl", "bootout", self.domain, self.entry])
            self.stop_menu(self.login_env)
        self.native_menu()
        self.retained(snapshot)
        preserved = self.registration()
        if tuple(map(int, version.split("."))) >= (0, 20, 0):
            before_uninstall_request = digest(self.binary.resolve(strict=True))
            delegation = self.mcw("consumer-uninstall-manager-delegation", "uninstall", codes=(2,))
            require(b"brew uninstall --formula miccamwatch" in delegation.stderr,
                    "Consumer uninstall did not report the exact Homebrew procedure")
            require(self.binary.is_file() and digest(self.binary.resolve(strict=True)) == before_uninstall_request,
                    "Consumer uninstall mutated the manager-owned executable")
            require(self.registration() == preserved, "Manager delegation changed autostart")
            self.retained(snapshot)
        self.brew("native-uninstall", "uninstall", self.formula)
        self.installed = False
        require(not self.binary.exists() and not selected.exists(), "Package-owned executable/link remained after uninstall")
        require(self.registration() == preserved, "Uninstall deleted/changed user's autostart intent")
        require(self.disabled("disabled-state-after-uninstall") == (self.args.autostart_intent == "disabled"),
                "Uninstall changed retained launchctl disabled intent")
        self.retained(snapshot)
        self.brew("native-reinstall", "install", "--build-from-source", self.formula)
        self.installed = True
        self.absent_service("reinstall-did-not-activate")
        self.mcw("explicit-remove-login-intent", "autostart", "disable")
        self.stop_menu(self.login_env)
        require(not self.entry.exists(), "Explicit autostart disable did not remove owned registration")
        fresh = self.root / "fresh-home"
        fresh.mkdir(mode=0o700)
        self.brew("absent-registration-postinstall", "postinstall", self.formula, env=self.environment(fresh))
        require(not (fresh / "Library/LaunchAgents" / (LABEL + ".plist")).exists(), "Postinstall enabled absent registration")
        require(not (fresh / "config/MicCamWatch/autostart").exists(), "Postinstall manufactured absent autostart ownership state")
        self.absent_service("fresh-postinstall-did-not-activate")
        external = b'<?xml version="1.0"?><plist version="1.0"><dict><key>Label</key><string>' + LABEL.encode() + b'</string><key>ProgramArguments</key><array><string>/usr/bin/true</string></array></dict></plist>\n'
        self.entry.parent.mkdir(parents=True, exist_ok=True)
        self.entry.write_bytes(external)
        for action in ("refresh", "enable", "disable"):
            result = self.mcw("external-ownership-" + action, "autostart", action, codes=(2,))
            require(self.entry.read_bytes() == external, "External registration was overwritten")
            self.absent_service("external-refusal-did-not-activate")
        self.brew("external-postinstall-refusal", "postinstall", self.formula)
        require(self.entry.read_bytes() == external, "Formula hook overwrote external registration")
        self.brew("external-native-uninstall", "uninstall", self.formula)
        self.installed = False
        require(self.entry.read_bytes() == external, "Uninstall deleted an external registration")
        self.retained(snapshot)
        self.entry.unlink()  # Only the fixture written above, never user-owned data.
        (self.proof / "result.json").write_text(json.dumps({"qualified": True, "architecture": actual,
            "macos": runtime, "baseline": "0.16.1", "candidate": version,
            "candidate_ref": self.args.candidate_ref or RELEASE_COMMIT,
            "autostart_intent": self.args.autostart_intent,
            "scope": "Genuine brew install/upgrade/cleanup/test/uninstall/reinstall, CLI, embedded AppKit menu lifecycle, ownership refusals and byte-preserved user fixtures. No physical capture/device mutation, Developer ID/notarization or external tap/core acceptance."}, indent=2) + "\n")

    def cleanup(self):
        for process in self.children:
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGTERM)
                process.wait(timeout=25)
        if self.entry.exists():
            subprocess.run(["launchctl", "bootout", self.domain, str(self.entry)], capture_output=True, timeout=30)
        if self.package_reserved and self.binary.exists():
            for environment in (self.env, self.login_env):
                subprocess.run([str(self.binary), "tray", "stop"], env=environment, capture_output=True, timeout=30)
        if self.disabled_before is not None:
            subprocess.run(["launchctl", "disable" if self.disabled_before else "enable", self.service], capture_output=True, timeout=30)
        if self.installed:
            self.brew("cleanup-owned-package", "uninstall", self.formula)
        if self.tap_path is not None and self.tap_path.exists():
            self.brew("remove-native-tap", "untap", self.tap)
        for handle in self.handles:
            handle.close()
        for directory in set(self.login_state):
            if directory.exists():
                shutil.rmtree(directory)
        shutil.rmtree(self.root)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--proof", type=Path, required=True)
    parser.add_argument("--architecture", choices=("arm64", "x86_64"), required=True)
    parser.add_argument("--autostart-intent", choices=("loaded", "dormant", "disabled"), required=True)
    parser.add_argument("--disposable-machine", action="store_true")
    parser.add_argument("--candidate-source-url", default="")
    parser.add_argument("--candidate-source-sha256", default="")
    parser.add_argument("--candidate-ref", default="")
    parser.add_argument("--candidate-version", default="")
    args = parser.parse_args()
    qualification = Qualification(args)
    try:
        qualification.execute()
    except Exception as error:
        (qualification.proof / "failure.txt").write_text(str(error) + "\n")
        raise
    finally:
        qualification.cleanup()


if __name__ == "__main__":
    main()
