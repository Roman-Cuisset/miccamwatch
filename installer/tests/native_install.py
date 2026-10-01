#!/usr/bin/env python3
"""Native, isolated checks of the public installer and real release executable.

Run on Linux x86_64 or macOS 15+ (ARM/Intel), as a normal user, with bash,
zsh and fish available. --source-url must identify a public 40-hex commit:
  python3 installer/tests/native_install.py --source-url \
    https://raw.githubusercontent.com/Roman-Cuisset/miccamwatch/COMMIT/installer/install.sh

The public smoke uses real HTTPS downloads. Failure fixtures intercept only
curl's transport, serving bounded archives containing the same verified native
executable. They execute the actual downloaded installer, not a reimplementation.
Python is a verification dependency, never an installer dependency. All HOME,
PATH and shell configuration changes are confined to a temporary child environment.
"""

import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import platform
import pty
import re
import select
import shlex
import shutil
import signal
import subprocess
import sys
import tarfile
import tempfile
import time


RELEASE_ROOT = "https://github.com/Roman-Cuisset/miccamwatch/releases/download"
SOURCE_PATTERN = re.compile(
    r"https://raw\.githubusercontent\.com/[^/]+/[^/]+/[0-9a-fA-F]{40}/installer/install\.sh"
)


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def run(argv, env=None, cwd=None, codes=(0,), timeout=120):
    result = subprocess.run(
        [str(arg) for arg in argv],
        env=env,
        cwd=cwd,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        timeout=timeout,
        start_new_session=True,
    )
    require(
        codes is None or result.returncode in codes,
        f"{argv!r}: exit {result.returncode}\nstdout:\n{result.stdout}\nstderr:\n{result.stderr}",
    )
    return result


def download(curl, url, destination):
    run([
        curl, "--fail", "--silent", "--show-error", "--location",
        "--proto", "=https", "--proto-redir", "=https", "--tlsv1.2",
        "--connect-timeout", "20", "--max-time", "180", "--output", destination, url,
    ], timeout=200)


def released_binary(curl, root, version):
    os_name = platform.system()
    arch = platform.machine().lower()
    if os_name == "Linux":
        require(arch == "x86_64", f"unsupported native test architecture: {arch}")
        asset = "miccamwatch-linux-x86_64.tar.gz"
    else:
        require(os_name == "Darwin", f"native test requires Linux or macOS, not {os_name}")
        require(arch in ("arm64", "aarch64", "x86_64"), f"unsupported native architecture: {arch}")
        # Match the installer when verification was invoked from a Rosetta shell.
        translated = run(["/usr/sbin/sysctl", "-in", "sysctl.proc_translated"], codes=None)
        arch = "aarch64" if arch in ("arm64", "aarch64") or translated.stdout.strip() == "1" else "x86_64"
        asset = f"miccamwatch-macos-{arch}.tar.gz"
    archive = root / asset
    manifest = root / "SHA256SUMS"
    download(curl, f"{RELEASE_ROOT}/{version}/{asset}", archive)
    download(curl, f"{RELEASE_ROOT}/{version}/SHA256SUMS", manifest)
    entries = []
    for line in manifest.read_text().splitlines():
        match = re.fullmatch(r"([0-9a-fA-F]{64}) [ *](.+)", line)
        if match and match.group(2) == asset:
            entries.append(match.group(1).lower())
    require(entries == [digest(archive.read_bytes())], "independent release checksum verification failed")
    with tarfile.open(archive, "r:gz") as release:
        members = [member for member in release.getmembers() if member.name.removeprefix("./") == "mcw"]
        require(len(members) == 1 and members[0].isfile(), "release lacks one regular mcw executable")
        binary = release.extractfile(members[0]).read()
    executable = root / "verified-release-mcw"
    executable.write_bytes(binary)
    executable.chmod(0o755)
    require(run([executable, "--version"]).stdout.strip() == f"mcw {version[1:]}", "release version mismatch")
    return asset, binary


class Sandbox:
    def __init__(self, root, name, shell, shell_path):
        self.root = root / name
        self.home = self.root / "isolated home"
        # Exercise spaces, single/double quotes and literal shell metacharacters.
        self.prefix = self.home / "install space 'quote\" $literal; [brackets]"
        self.cwd = self.root / "unrelated working directory"
        self.temp = self.root / "temporary downloads"
        self.shell = shell
        self.shell_path = shell_path
        for directory in (self.home, self.cwd, self.temp):
            directory.mkdir(parents=True)
        self.env = {
            "HOME": str(self.home),
            "SHELL": shell_path,
            "PATH": os.defpath + os.pathsep + "/usr/local/bin:/opt/homebrew/bin",
            "TMPDIR": str(self.temp),
            "XDG_CONFIG_HOME": str(self.home / "config directory"),
            "XDG_DATA_HOME": str(self.home / "data directory"),
            "XDG_CACHE_HOME": str(self.home / "cache directory"),
            "ZDOTDIR": str(self.home / "zsh configuration"),
            "LANG": "C.UTF-8" if platform.system() == "Linux" else "en_US.UTF-8",
            "LC_ALL": "C.UTF-8" if platform.system() == "Linux" else "en_US.UTF-8",
            "TERM": "dumb",
        }
        # Retain the native session socket location, without writing to it or
        # starting a capture/session service. No host config paths are inherited.
        if os.environ.get("XDG_RUNTIME_DIR"):
            self.env["XDG_RUNTIME_DIR"] = os.environ["XDG_RUNTIME_DIR"]
        bash_rc = "# Existing user bash configuration\nalias mcw_user_alias='printf user'\nexport USER_INSTALL_CHECK=kept\n"
        zsh_rc = "# Existing user zsh configuration\nalias mcw_user_alias='printf user'\nexport USER_INSTALL_CHECK=kept\n"
        fish_rc = "# Existing user fish configuration\nalias mcw_user_alias 'printf user'\nset -gx USER_INSTALL_CHECK kept\n"
        self.configs = {
            self.home / ".bashrc": bash_rc.encode(),
            self.home / ".bash_profile": b'# Existing login configuration\n[ ! -f "$HOME/.bashrc" ] || . "$HOME/.bashrc"\n',
            self.home / ".profile": b"# Existing profile; not the active bash login file\n",
            self.home / ".zshrc": b"# User HOME zsh config; ZDOTDIR must take precedence\n",
            Path(self.env["ZDOTDIR"]) / ".zshrc": zsh_rc.encode(),
            Path(self.env["ZDOTDIR"]) / ".zprofile": b"# Existing zsh login configuration\n",
            Path(self.env["XDG_CONFIG_HOME"]) / "fish/config.fish": fish_rc.encode(),
        }
        for path, content in self.configs.items():
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(content)
        self.history = self.home / ".bash_history"
        self.history.write_bytes(b"# Preserve shell history\nprintf existing-history\n")
        self.binary = self.prefix / "bin/mcw"
        self.user_tool = self.prefix / "bin/user-owned-tool"
        self.user_tool.parent.mkdir(parents=True)
        self.user_tool.write_bytes(b"user-owned binary content\n")

    def shell_command(self, command, login=False):
        # Interactive startup is what a newly opened terminal uses. No rc files
        # are sourced by the harness and no parent process environment is changed.
        return [self.shell_path, "-lic" if login else "-ic", command]

    def invoke(self, script, *flags, env=None, version="v0.14.0", codes=(0,)):
        result = run(
            ["/bin/sh", script, "--version", version, "--prefix", self.prefix, *flags],
            env=env or self.env, cwd=self.cwd, codes=codes, timeout=240,
        )
        require(not list(self.temp.iterdir()), "installer leaked temporary download files")
        return result

    def unchanged_configs(self):
        for path, content in self.configs.items():
            require(path.read_bytes() == content, f"installer changed unconsented user configuration: {path}")

    def removed_path_configs(self, suffix=b""):
        for path, content in self.configs.items():
            current = path.read_bytes()
            require(current.startswith(content) and current.endswith(suffix),
                    f"uninstall removed existing shell content: {path}")
            middle_end = len(current) - len(suffix) if suffix else len(current)
            require(not current[len(content):middle_end].strip(),
                    f"uninstall left managed PATH content in {path}")
        environment = run(
            self.shell_command('printf "%s" "$PATH"'),
            env=self.env, cwd=self.cwd,
        ).stdout.split(os.pathsep)
        require(str(self.prefix / "bin") not in environment,
                "uninstall left its managed PATH entry in a new shell")

    def version(self, expected, login=False):
        result = run(self.shell_command("mcw --version", login), env=self.env, cwd=self.cwd)
        require(result.stdout.strip() == f"mcw {expected[1:]}", "new shell did not resolve installed mcw")
        alias_result = run(
            self.shell_command('mcw_user_alias; printf " %s" "$USER_INSTALL_CHECK"'),
            env=self.env, cwd=self.cwd,
        )
        require(alias_result.stdout.strip() == "user kept", "user aliases/preferences lost in new shell")

    def saved_user_data(self):
        config = Path(self.env["XDG_CONFIG_HOME"]) / "MicCamWatch"
        data = Path(self.env["XDG_DATA_HOME"]) / "MicCamWatch"
        config.mkdir(parents=True, exist_ok=True)
        data.mkdir(parents=True, exist_ok=True)
        # Written after command smoke so the checks do not depend on policy parser
        # defaults or write malformed input for the installed monitor.
        files = {
            config / "policy.toml": b"# User policy must survive uninstall\n",
            config / "settings.toml": b"# User preferences must survive uninstall\n",
            data / "history.jsonl": b'{"user":"retained history"}\n',
            data / "user.log": b"existing user log\n",
            self.history: self.history.read_bytes(),
            self.user_tool: self.user_tool.read_bytes(),
        }
        for path, content in files.items():
            path.write_bytes(content)
        return files


def interactive_consent(script, sandbox, version, answer=b"\n"):
    # Give /dev/tty a controlling terminal while stdin is the downloaded script,
    # reproducing curl | sh's distinction between script input and user consent.
    pid, master = pty.fork()
    if pid == 0:
        try:
            os.chdir(sandbox.cwd)
            fd = os.open(script, os.O_RDONLY)
            os.dup2(fd, 0)
            os.close(fd)
            os.execve("/bin/sh", [
                "sh", "-s", "--", "--version", version, "--prefix", str(sandbox.prefix),
            ], sandbox.env)
        finally:
            os._exit(127)
    output = bytearray()
    deadline = time.monotonic() + 240
    status = None
    try:
        # Queue consent on the terminal, independently of the script input.
        os.write(master, answer)
        while time.monotonic() < deadline:
            readable, _, _ = select.select([master], [], [], 0.2)
            if readable:
                try:
                    block = os.read(master, 65536)
                except OSError:
                    block = b""
                output.extend(block)
            waited, status = os.waitpid(pid, os.WNOHANG)
            if waited:
                break
        else:
            raise AssertionError("interactive consent installation blocked")
        require(os.waitstatus_to_exitcode(status) == 0, f"interactive install failed: {output.decode(errors='replace')}")
    finally:
        os.close(master)
        if status is None or waited == 0:
            os.kill(pid, signal.SIGKILL)
            os.waitpid(pid, 0)
    if answer.strip() == b"y":
        sandbox.version(version)
        sandbox.version(version, login=True)
    else:
        sandbox.unchanged_configs()
    require(not list(sandbox.temp.iterdir()), "interactive installer leaked temporary files")


def validate_status(document, version):
    require(isinstance(document, dict), "status --json must be an object")
    require(document.get("tool_version") == version[1:], "status reports a different installed release")
    require(isinstance(document.get("schema_version"), int), "status lacks a versioned JSON schema")
    require(isinstance(document.get("accesses"), list), "status accesses are not a list")
    expected = {"pipewire_audio", "pipewire_video"} if platform.system() == "Linux" else {"coreaudio_input", "avfoundation_video"}
    collectors = document.get("collectors", [])
    require({item["collector"] for item in collectors} == expected, "wrong native collectors in installed status")
    require(all(item["state"] in ("healthy", "degraded", "unavailable") for item in collectors), "invalid collector health")
    return expected


def native_cli(script, sandbox, version):
    status = run(sandbox.shell_command("mcw --lang en status --json"), env=sandbox.env, cwd=sandbox.cwd, codes=(0, 1, 2))
    expected = validate_status(json.loads(status.stdout), version)
    doctor = run(sandbox.shell_command("mcw --lang en doctor --json"), env=sandbox.env, cwd=sandbox.cwd, codes=(0, 2))
    checks = json.loads(doctor.stdout)
    require(isinstance(checks, list) and {item["name"] for item in checks} == expected, "wrong native doctor checks")
    require(all(item["status"] in ("ok", "warning", "error") for item in checks), "invalid doctor result")
    devices = run(sandbox.shell_command("mcw --lang en devices --json"), env=sandbox.env, cwd=sandbox.cwd, codes=(0, 2))
    if devices.returncode == 0:
        inventory = json.loads(devices.stdout)
        require(isinstance(inventory, list), "devices --json must be an array")
        for item in inventory:
            require(isinstance(item["id"], str) and isinstance(item["name"], str), "invalid device identity JSON")
            require(item["resource"] in ("camera", "microphone"), "invalid device resource JSON")
    else:
        # Headless Linux runners may have no accessible PipeWire session. The
        # released CLI returns an explicit error, not a fabricated empty inventory.
        require(platform.system() == "Linux" and not devices.stdout.strip() and "mcw:" in devices.stderr,
                "devices failed without the supported headless Linux diagnostic")
        print(f"  {sandbox.shell}: devices unavailable on this native session: {devices.stderr.strip()}")
    stdout = sandbox.root / "watch.jsonl"
    stderr = sandbox.root / "watch.stderr"
    before = sandbox.binary.stat().st_ino, digest(sandbox.binary.read_bytes())
    with stdout.open("w") as out, stderr.open("w") as err:
        watcher = subprocess.Popen(
            sandbox.shell_command("exec mcw --lang en watch --json --interval 250 --no-kill"),
            env=sandbox.env, cwd=sandbox.cwd, stdout=out, stderr=err,
            stdin=subprocess.DEVNULL, start_new_session=True,
        )
        try:
            time.sleep(8)
            require(watcher.poll() is None, f"installed watch exited before SIGINT: {stderr.read_text()}")
            # Never kill/stop the running executable to force an installer success.
            replacement = sandbox.invoke(script, "--no-modify-path", version=version, codes=None)
            require(replacement.returncode != 0, "installer replaced a running executable without refusing")
            require((sandbox.binary.stat().st_ino, digest(sandbox.binary.read_bytes())) == before,
                    "running executable refusal altered installed binary")
            removal = sandbox.invoke(script, "--uninstall", "--no-modify-path", version=version, codes=None)
            require(removal.returncode != 0, "installer uninstalled a running executable without refusing")
            require((sandbox.binary.stat().st_ino, digest(sandbox.binary.read_bytes())) == before,
                    "running uninstall refusal altered installed binary")
            require(watcher.poll() is None, "installer stopped the user's running monitor")
            os.killpg(watcher.pid, signal.SIGINT)
            require(watcher.wait(timeout=20) == 0, f"installed watch failed graceful SIGINT: {stderr.read_text()}")
        finally:
            if watcher.poll() is None:
                os.killpg(watcher.pid, signal.SIGKILL)
                watcher.wait()
    for line in stdout.read_text().splitlines():
        event = json.loads(line)
        if isinstance(event, dict) and "collectors" in event:
            validate_status(event, version)
            continue
        require(isinstance(event, dict) and event.get("action") in ("start", "update", "stop"), "invalid watch JSON event")
        require(event.get("resource") in ("microphone", "camera") and isinstance(event.get("evidence"), list),
                "watch event lacks flattened access evidence")
        require(event.get("tool_version") == version[1:], "watch reports a different installed release")
    print(f"  {sandbox.shell}: native status/doctor/devices and watch SIGINT exercised")


def smoke(script, root, shells, version, binary):
    for shell, shell_path in shells.items():
        sandbox = Sandbox(root, f"public-{shell}", shell, shell_path)
        sandbox.invoke(script, "--no-modify-path", version=version)
        sandbox.unchanged_configs()
        require(sandbox.binary.read_bytes() == binary, "fresh public install differs from verified released executable")
        require(run([sandbox.binary, "--version"], env=sandbox.env, cwd=sandbox.cwd).stdout.strip() == f"mcw {version[1:]}", "fresh install version mismatch")
        before = sandbox.binary.stat().st_ino
        sandbox.invoke(script, version=version)
        sandbox.unchanged_configs()
        require(sandbox.binary.read_bytes() == binary, "noninteractive reinstall corrupted working executable")
        require(sandbox.binary.stat().st_ino != before, "managed reinstall did not atomically replace existing executable")
        sandbox.invoke(script, "--add-path", version=version)
        configured = {path: path.read_bytes() for path in sandbox.configs}
        sandbox.version(version)
        sandbox.version(version, login=True)
        sandbox.invoke(script, "--add-path", version=version)
        require({path: path.read_bytes() for path in sandbox.configs} == configured, "reinstall duplicated or rewrote PATH configuration")
        sandbox.invoke(script, "--no-modify-path", version=version)
        require({path: path.read_bytes() for path in sandbox.configs} == configured, "--no-modify-path changed existing managed PATH")
        before_failure = sandbox.binary.stat().st_ino, digest(sandbox.binary.read_bytes())
        failed = sandbox.invoke(script, "--no-modify-path", version="v9999.9999.9999", codes=None)
        require(failed.returncode != 0, "nonexistent public release was reported installed")
        require((sandbox.binary.stat().st_ino, digest(sandbox.binary.read_bytes())) == before_failure,
                "failed HTTP upgrade replaced working executable")
        sandbox.version(version)
        native_cli(script, sandbox, version)
        saved = sandbox.saved_user_data()
        # Changes made by the user after installation must survive block removal.
        suffix = b"# User edit after installation\n"
        for path in sandbox.configs:
            with path.open("ab") as handle:
                handle.write(suffix)
        sandbox.invoke(script, "--uninstall", "--no-modify-path", version=version)
        require(not sandbox.binary.exists(), "uninstall retained installer-owned executable")
        sandbox.removed_path_configs(suffix)
        for path, content in saved.items():
            require(path.read_bytes() == content, f"uninstall removed or modified user data: {path}")
        remaining = {path.relative_to(sandbox.prefix) for path in sandbox.prefix.rglob("*") if path.is_file() or path.is_symlink()}
        require(remaining == {Path("bin/user-owned-tool")}, f"uninstall retained owned files or removed user files: {remaining}")
        sandbox.invoke(script, "--uninstall", "--no-modify-path", version=version)
        sandbox.removed_path_configs(suffix)
        print(f"PASS public {shell}: fresh/reinstall/atomic replacement/PATH/uninstall preservation")

    interactive = Sandbox(root, "public-interactive-default-no", "bash", shells["bash"])
    interactive_consent(script, interactive, version)
    interactive.invoke(script, "--uninstall", "--no-modify-path", version=version)
    print("PASS public interactive curl-style stdin: default answer does not modify PATH")

    consent = Sandbox(root, "public-interactive-yes", "bash", shells["bash"])
    interactive_consent(script, consent, version, answer=b"y\n")
    consent.invoke(script, "--uninstall", "--no-modify-path", version=version)
    consent.removed_path_configs()
    print("PASS public interactive curl-style stdin: explicit terminal consent adds usable PATH")

    already = Sandbox(root, "public-already-on-path", "bash", shells["bash"])
    already.env["PATH"] = str(already.prefix / "bin") + os.pathsep + already.env["PATH"]
    already.invoke(script, "--add-path", version=version)
    already.unchanged_configs()
    already.version(version)
    already.invoke(script, "--uninstall", "--no-modify-path", version=version)
    print("PASS public already-on-PATH: no redundant shell configuration")


def fixture_curl(arguments):
    """Controlled transport for failure fixtures; unexpected URLs fail closed."""
    plan = json.loads(Path(os.environ["MCW_TEST_CURL_PLAN"]).read_text())
    urls = [arg for arg in arguments if arg.startswith("https://")]
    destination = None
    for index, arg in enumerate(arguments):
        if arg in ("-o", "--output"):
            destination = arguments[index + 1]
        elif arg.startswith("--output="):
            destination = arg.split("=", 1)[1]
    if len(urls) != 1 or urls[0] not in plan["responses"]:
        print(f"fixture curl: unexpected request: {arguments!r}", file=sys.stderr)
        return 97
    response = plan["responses"][urls[0]]
    with Path(plan["log"]).open("a") as log:
        log.write(json.dumps({"url": urls[0], "response": response}) + "\n")
    if isinstance(response, int):
        print("curl: (22) controlled HTTP failure", file=sys.stderr)
        return response
    if isinstance(response, dict):
        if "--head" not in arguments or "--write-out" not in arguments:
            print("fixture curl: redirect response requires HEAD and write-out", file=sys.stderr)
            return 97
        sys.stdout.write(response["effective_url"])
        return 0
    content = Path(response).read_bytes()
    if destination:
        Path(destination).write_bytes(content)
    else:
        sys.stdout.buffer.write(content)
    return 0


def archive_bytes(binary, extra=None, replacement=None):
    output = io.BytesIO()
    with tarfile.open(fileobj=output, mode="w:gz", format=tarfile.USTAR_FORMAT) as archive:
        for name, content, mode in (("mcw", binary, 0o755), ("README.md", b"fixture readme\n", 0o644), ("LICENSE", b"fixture license\n", 0o644)):
            member = tarfile.TarInfo(name)
            member.mode = mode
            if name == "mcw" and replacement is not None:
                member = replacement
                content = b""
            member.size = len(content) if member.isfile() else 0
            archive.addfile(member, io.BytesIO(content) if member.isfile() else None)
        if extra is not None:
            content = b"archive must not escape\n" if extra.isfile() else b""
            extra.size = len(content)
            archive.addfile(extra, io.BytesIO(content) if extra.isfile() else None)
    return output.getvalue()


def fixture_environment(sandbox, responses):
    wrapper_dir = sandbox.root / "controlled transport"
    wrapper_dir.mkdir(exist_ok=True)
    wrapper = wrapper_dir / "curl"
    wrapper.write_text(
        "#!/bin/sh\nexec " + shlex.quote(sys.executable) + " " +
        shlex.quote(str(Path(__file__).resolve())) + ' --fixture-curl "$@"\n'
    )
    wrapper.chmod(0o755)
    log = wrapper_dir / "requests.jsonl"
    plan = wrapper_dir / "plan.json"
    plan.write_text(json.dumps({"responses": responses, "log": str(log)}))
    env = dict(sandbox.env)
    env["PATH"] = str(wrapper_dir) + os.pathsep + env["PATH"]
    env["MCW_TEST_CURL_PLAN"] = str(plan)
    return env, log


def fixtures(script, root, shells, version, asset, binary):
    good_archive = archive_bytes(binary)
    scenarios = {
        "checksum-tamper": (good_archive, "0" * 64, asset, version),
        "checksum-wrong-asset": (good_archive, None, asset + ".decoy", version),
        "checksum-duplicate": (good_archive, "duplicate", asset, version),
        "checksum-malformed": (good_archive, "not-a-checksum", asset, version),
        "truncated-archive": (good_archive[:100], None, asset, version),
        "version-mismatch": (good_archive, None, asset, "v0.14.999"),
    }
    for name, member_name in (("parent-traversal", "../escaped-sentinel"),
                              ("absolute-path", str(root / "escaped-absolute-sentinel")),
                              ("unexpected-member", "nested/unexpected-file"),
                              ("duplicate-executable", "mcw")):
        member = tarfile.TarInfo(member_name)
        scenarios[name] = (archive_bytes(binary, extra=member), None, asset, version)
    for name, kind in (("symlink-executable", tarfile.SYMTYPE),
                       ("hardlink-executable", tarfile.LNKTYPE),
                       ("directory-executable", tarfile.DIRTYPE),
                       ("fifo-executable", tarfile.FIFOTYPE)):
        member = tarfile.TarInfo("mcw")
        member.type = kind
        member.linkname = str(root / "escaped-link-sentinel") if kind in (tarfile.SYMTYPE, tarfile.LNKTYPE) else ""
        scenarios[name] = (archive_bytes(binary, replacement=member), None, asset, version)

    # A successful fixture with real mcw bytes is the positive control: failed
    # cases cannot pass merely because the wrapper/installer integration is broken.
    control = Sandbox(root, "fixture-positive-control", "bash", shells["bash"])
    archive_path = control.root / asset
    archive_path.write_bytes(good_archive)
    sums_path = control.root / "SHA256SUMS"
    sums_path.write_text(f"{digest(good_archive)}  {asset}\n")
    env, _ = fixture_environment(control, {
        f"{RELEASE_ROOT}/{version}/{asset}": str(archive_path),
        f"{RELEASE_ROOT}/{version}/SHA256SUMS": str(sums_path),
    })
    control.invoke(script, "--add-path", env=env, version=version)
    control.version(version)
    control.invoke(script, "--uninstall", "--no-modify-path", version=version)
    print("PASS fixture positive control: actual release executable installed and runnable")

    latest = Sandbox(root, "fixture-latest-default-prefix", "bash", shells["bash"])
    latest_url = "https://github.com/Roman-Cuisset/miccamwatch/releases/latest"
    env, latest_log = fixture_environment(latest, {
        latest_url: {"effective_url": f"https://github.com/Roman-Cuisset/miccamwatch/releases/tag/{version}"},
        f"{RELEASE_ROOT}/{version}/{asset}": str(archive_path),
        f"{RELEASE_ROOT}/{version}/SHA256SUMS": str(sums_path),
    })
    run(["/bin/sh", script, "--no-modify-path"], env=env, cwd=latest.cwd)
    default_binary = latest.home / ".local/bin/mcw"
    require(default_binary.read_bytes() == binary, "default prefix/latest did not install the verified executable")
    require(run([default_binary, "--version"], env=latest.env, cwd=latest.cwd).stdout.strip() == f"mcw {version[1:]}",
            "resolved latest release version mismatch")
    requests = {json.loads(line)["url"] for line in latest_log.read_text().splitlines()}
    require(requests == {latest_url, f"{RELEASE_ROOT}/{version}/{asset}", f"{RELEASE_ROOT}/{version}/SHA256SUMS"},
            "latest resolution did not pin both downloads to one concrete release")
    latest.unchanged_configs()
    run(["/bin/sh", script, "--uninstall", "--no-modify-path"], env=latest.env, cwd=latest.cwd)
    require(not default_binary.exists(), "default-prefix uninstall retained its executable")
    print("PASS fixture latest/default prefix: concrete release resolved before both downloads")

    for name, (payload, expected, manifest_asset, target_version) in scenarios.items():
        sandbox = Sandbox(root, "fixture-" + name, "bash", shells["bash"])
        # Seed the existing executable through the real installer, using the same
        # positive-control transport, rather than fabricating receipt state.
        seed_env, _ = fixture_environment(sandbox, {
            f"{RELEASE_ROOT}/{version}/{asset}": str(archive_path),
            f"{RELEASE_ROOT}/{version}/SHA256SUMS": str(sums_path),
        })
        sandbox.invoke(script, "--add-path", env=seed_env, version=version)
        prior_binary = sandbox.binary.stat().st_ino, sandbox.binary.read_bytes()
        receipt = sandbox.prefix / ".miccamwatch-install"
        prior_receipt = {path.relative_to(receipt): path.read_bytes() for path in receipt.rglob("*") if path.is_file()}
        prior_paths = {path.relative_to(sandbox.prefix) for path in sandbox.prefix.rglob("*")}
        prior_configs = {path: path.read_bytes() for path in sandbox.configs}
        bad_archive = sandbox.root / "candidate.tar.gz"
        bad_archive.write_bytes(payload)
        bad_sums = sandbox.root / "candidate-SHA256SUMS"
        checksum = digest(payload) if expected is None or expected == "duplicate" else expected
        manifest = f"{checksum}  {manifest_asset}\n"
        if expected == "duplicate":
            manifest += manifest
        bad_sums.write_text(manifest)
        env, log = fixture_environment(sandbox, {
            f"{RELEASE_ROOT}/{target_version}/{asset}": str(bad_archive),
            f"{RELEASE_ROOT}/{target_version}/SHA256SUMS": str(bad_sums),
        })
        log.unlink(missing_ok=True)
        failed = sandbox.invoke(script, "--add-path", env=env, version=target_version, codes=None)
        require(failed.returncode != 0, f"installer accepted dangerous fixture: {name}")
        require((sandbox.binary.stat().st_ino, sandbox.binary.read_bytes()) == prior_binary,
                f"failed {name} replaced the working executable")
        require({path.relative_to(receipt): path.read_bytes() for path in receipt.rglob("*") if path.is_file()} == prior_receipt,
                f"failed {name} changed the ownership receipt")
        require({path.relative_to(sandbox.prefix) for path in sandbox.prefix.rglob("*")} == prior_paths,
                f"failed {name} leaked staging files or changed installed paths")
        require({path: path.read_bytes() for path in sandbox.configs} == prior_configs, f"failed {name} changed user PATH")
        sandbox.version(version)
        require(log.exists(), f"fixture {name} was rejected before consuming controlled release data")
        requested = {json.loads(line)["url"] for line in log.read_text().splitlines()}
        require(f"{RELEASE_ROOT}/{target_version}/SHA256SUMS" in requested,
                f"fixture {name} did not consume its controlled checksum manifest")
        if not name.startswith("checksum-"):
            require(f"{RELEASE_ROOT}/{target_version}/{asset}" in requested,
                    f"fixture {name} did not consume its controlled archive")
        # Traversal/link targets and extraction leftovers are confined and checked.
        require(not list(sandbox.root.rglob("escaped-sentinel")), "archive traversal created an escaped file")
        require(not (root / "escaped-absolute-sentinel").exists(), "absolute archive member escaped extraction")
        require(not (root / "escaped-link-sentinel").exists(), "archive link escaped extraction")
        sandbox.invoke(script, "--uninstall", "--no-modify-path", version=version)
        print(f"PASS fixture {name}: working executable/receipt/PATH preserved")

    unmanaged = Sandbox(root, "fixture-unmanaged-binary", "bash", shells["bash"])
    unmanaged.binary.write_bytes(binary)
    unmanaged.binary.chmod(0o755)
    before = unmanaged.binary.stat().st_ino, unmanaged.binary.read_bytes()
    refused = unmanaged.invoke(script, "--no-modify-path", version=version, codes=None)
    require(refused.returncode != 0 and (unmanaged.binary.stat().st_ino, unmanaged.binary.read_bytes()) == before,
            "installer overwrote an unmanaged executable")
    unmanaged.invoke(script, "--uninstall", "--no-modify-path", version=version)
    require((unmanaged.binary.stat().st_ino, unmanaged.binary.read_bytes()) == before,
            "uninstall removed or changed unmanaged executable")
    unmanaged.unchanged_configs()
    print("PASS unmanaged executable: install refuses and uninstall preserves user binary")

    changed = Sandbox(root, "fixture-user-modified-binary", "bash", shells["bash"])
    env, _ = fixture_environment(changed, {
        f"{RELEASE_ROOT}/{version}/{asset}": str(archive_path),
        f"{RELEASE_ROOT}/{version}/SHA256SUMS": str(sums_path),
    })
    changed.invoke(script, "--add-path", env=env, version=version)
    with changed.binary.open("ab") as handle:
        handle.write(b"user modification outside installer ownership\n")
    user_binary = changed.binary.read_bytes()
    receipt = changed.prefix / ".miccamwatch-install"
    user_receipt = {path.relative_to(receipt): path.read_bytes() for path in receipt.rglob("*") if path.is_file()}
    user_configs = {path: path.read_bytes() for path in changed.configs}
    for flags in (("--no-modify-path",), ("--uninstall", "--no-modify-path")):
        refused = changed.invoke(script, *flags, env=env, version=version, codes=None)
        require(refused.returncode != 0 and changed.binary.read_bytes() == user_binary,
                "installer overwrote or removed a user-modified managed binary")
        require({path.relative_to(receipt): path.read_bytes() for path in receipt.rglob("*") if path.is_file()} == user_receipt,
                "modified binary refusal destroyed recovery ownership state")
        require({path: path.read_bytes() for path in changed.configs} == user_configs,
                "modified binary refusal changed shell PATH")
    print("PASS user-modified executable: refused install/uninstall preserve file, receipt and PATH")

    modified_path = Sandbox(root, "fixture-user-modified-path", "bash", shells["bash"])
    env, _ = fixture_environment(modified_path, {
        f"{RELEASE_ROOT}/{version}/{asset}": str(archive_path),
        f"{RELEASE_ROOT}/{version}/SHA256SUMS": str(sums_path),
    })
    modified_path.invoke(script, "--add-path", env=env, version=version)
    receipt = modified_path.prefix / ".miccamwatch-install"
    record = sorted(receipt.glob("path-*.path"))[0]
    snapshot = record.with_suffix(".block")
    config = Path(record.read_text().strip())
    owned_block = snapshot.read_bytes()
    # Insert a comment within the exact owned block, before its final marker.
    lines = owned_block.splitlines(keepends=True)
    customized_block = b"".join(lines[:-1]) + b"# user customization\n" + lines[-1]
    original_config = config.read_bytes()
    require(original_config.count(owned_block) == 1, "installed PATH block is not unambiguous")
    customized_config = original_config.replace(owned_block, customized_block, 1)
    config.write_bytes(customized_config)
    partial = modified_path.invoke(script, "--uninstall", "--no-modify-path", version=version, codes=None)
    require(partial.returncode != 0 and not modified_path.binary.exists(),
            "modified PATH uninstall failed to report its partial, executable-only removal")
    require(config.read_bytes() == customized_config and record.is_file() and snapshot.read_bytes() == owned_block,
            "uninstall removed user-customized PATH content or its recovery receipt")
    # An explicit user repair makes the preserved receipt usable for safe retry.
    config.write_bytes(customized_config.replace(customized_block, owned_block, 1))
    modified_path.invoke(script, "--uninstall", "--no-modify-path", version=version)
    modified_path.removed_path_configs()
    require(not receipt.exists(), "successful uninstall retry retained installer ownership state")
    print("PASS modified PATH: preserve customization/receipt, then safely complete after explicit repair")


def main():
    if len(sys.argv) > 1 and sys.argv[1] == "--fixture-curl":
        return fixture_curl(sys.argv[2:])
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--source-url", required=True, help="public raw.githubusercontent.com commit-pinned installer URL")
    parser.add_argument("--version", default="v0.14.0", help="real public release to install (default: v0.14.0)")
    parser.add_argument("--mode", choices=("all", "smoke", "fixtures"), default="all")
    args = parser.parse_args()
    require(SOURCE_PATTERN.fullmatch(args.source_url), "source URL must pin a public 40-hex commit, not main/a local file")
    require(re.fullmatch(r"v[0-9]+\.[0-9]+\.[0-9]+", args.version), "expected release must be a concrete version")
    require(os.geteuid() != 0, "run native installation verification as a normal user, not root")
    require(platform.system() in ("Linux", "Darwin"), "run verification on a supported native Unix host")
    shells = {shell: shutil.which(shell) for shell in ("bash", "zsh", "fish")}
    require(all(shells.values()), f"install the runner's missing shells first: {shells}")
    curl = shutil.which("curl")
    require(curl is not None, "curl is required for public HTTPS verification")
    with tempfile.TemporaryDirectory(prefix="mcw native installation ") as temporary:
        root = Path(temporary)
        script = root / "public-install.sh"
        print(f"Public source: {args.source_url}")
        download(curl, args.source_url, script)
        asset, binary = released_binary(curl, root, args.version)
        if args.mode in ("all", "smoke"):
            smoke(script, root, shells, args.version, binary)
        if args.mode in ("all", "fixtures"):
            fixtures(script, root, shells, args.version, asset, binary)
    print(f"PASS native installation verification: {platform.system()} {platform.machine()}, {args.version}")
    print("Evidence covers managed same-release atomic replacement and failed upgrades, not an unavailable older native release.")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (AssertionError, OSError, subprocess.TimeoutExpired, ValueError, tarfile.TarError) as error:
        print(f"native installation verification failed: {error}", file=sys.stderr)
        sys.exit(1)
