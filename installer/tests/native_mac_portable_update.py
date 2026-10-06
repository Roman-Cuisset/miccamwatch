#!/usr/bin/env python3
"""Native macOS ARM/Intel portable update proof, with genuine Mach-O executables.

python3 -B installer/tests/native_mac_portable_update.py \
  --source-file "$PWD/installer/install.sh" \
  --archive "$PWD/miccamwatch-macos-aarch64.tar.gz" --version v0.16.1 \
  --proof "$RUNNER_TEMP/native-proof/macos-portable-update"

The immutable public v0.16.0 baseline needs explicit installer bootstrap: its
already-published updater cannot acquire corrected code before it updates itself.
A separate private copy of corrected source is built as v0.16.0 to prove the real
exec/installer/atomic-replacement path. Only that copy's latest-release curl
executable selection is redirected to the existing controlled transport helper;
no ownership, hash, version, archive or in-use check is bypassed. The candidate
archive and bootstrapped candidate use their unmodified production executable.
Python and Cargo are test dependencies, not installation/runtime dependencies.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shlex
import shutil
import signal
import stat
import subprocess
import sys
import tarfile
import tempfile
import time

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
import native_install as fixtures

LATEST = "https://github.com/Roman-Cuisset/miccamwatch/releases/latest"
TAG_BASE = "https://github.com/Roman-Cuisset/miccamwatch/releases/tag/"
BASELINE = "v0.16.0"
QUARANTINE = b"0000;67000000;MCW native portable proof;"


def require(condition, message):
    fixtures.require(condition, message)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def record(proof, name, argv, env=None, cwd=None, codes=(0,), timeout=300):
    process = subprocess.Popen([str(arg) for arg in argv], env=env, cwd=cwd,
                               stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                               stderr=subprocess.PIPE, text=True, start_new_session=True)
    try:
        stdout, stderr = process.communicate(timeout=timeout)
    except BaseException:
        kill_group(process.pid)
        process.communicate(timeout=30)
        raise
    result = subprocess.CompletedProcess(argv, process.returncode, stdout, stderr)
    (proof / (name + ".stdout")).write_text(result.stdout)
    (proof / (name + ".stderr")).write_text(result.stderr)
    (proof / (name + ".exit")).write_text(str(result.returncode) + "\n")
    require(codes is None or result.returncode in codes,
            f"{name}: exit {result.returncode}\n{result.stdout}\n{result.stderr}")
    return result


def kill_group(pid):
    try:
        os.killpg(pid, signal.SIGKILL)
    except ProcessLookupError:
        pass


def get_xattr(path, name):
    # macOS Python does not expose the Linux os.*xattr APIs.
    result = subprocess.run(["/usr/bin/xattr", "-px", name, str(path)],
                            check=True, capture_output=True, text=True)
    return bytes.fromhex("".join(result.stdout.split()))


def set_xattr(path, name, value):
    subprocess.run(["/usr/bin/xattr", "-wx", name, value.hex(), str(path)],
                   check=True, capture_output=True)


def identity(path):
    info = path.lstat()
    names = subprocess.run(["/usr/bin/xattr", str(path)], check=True,
                           capture_output=True, text=True).stdout.splitlines()
    attributes = {name: get_xattr(path, name).hex() for name in names}
    return dict(sha256=sha(path), inode=info.st_ino, device=info.st_dev,
                uid=info.st_uid, gid=info.st_gid, mode=stat.S_IMODE(info.st_mode),
                nlink=info.st_nlink, xattrs=attributes)


def candidate(path, version, root, env):
    arch = platform.machine().lower()
    require(arch in ("arm64", "aarch64", "x86_64"), f"unsupported architecture: {arch}")
    arch = "aarch64" if arch in ("arm64", "aarch64") else "x86_64"
    asset = f"miccamwatch-macos-{arch}.tar.gz"
    with tarfile.open(path, "r:gz") as archive:
        members = archive.getmembers()
        require(len(members) == 3 and {member.name for member in members} == {"mcw", "README.md", "LICENSE"}
                and all(member.isfile() for member in members),
                "candidate must contain exactly three flat regular macOS release members")
        require(next(member for member in members if member.name == "mcw").mode & 0o111,
                "genuine candidate archive does not mark mcw executable")
        binary = archive.extractfile("mcw").read()
    require(binary[:4] in (b"\xcf\xfa\xed\xfe", b"\xfe\xed\xfa\xcf", b"\xca\xfe\xba\xbe", b"\xbe\xba\xfe\xca"),
            "candidate is not a genuine Mach-O executable")
    executable = root / "verified candidate mcw"
    executable.write_bytes(binary)
    executable.chmod(0o755)
    require(fixtures.run([executable, "--version"], env=env).stdout.strip() == "mcw " + version[1:],
            "candidate archive executable has a different version")
    manifest = root / "candidate SHA256SUMS"
    manifest.write_text(f"{sha(path)}  {asset}\n")
    return asset, binary, manifest


class Portable(fixtures.Sandbox):
    def __init__(self, root, name, binary, mode=0o751, quarantine=QUARANTINE):
        super().__init__(root, name, "bash", "/bin/bash")
        self.binary = self.home / "standalone space 'quote\" $literal; [brackets]" / "mcw"
        self.binary.parent.mkdir()
        self.binary.write_bytes(binary)
        self.binary.chmod(mode)
        # A non-enforcing quarantine value tests exact xattr retention without
        # authorizing a Gatekeeper UI prompt or changing host security settings.
        if quarantine is not None:
            set_xattr(self.binary, "com.apple.quarantine", quarantine)
        self.preferences = self.home / "existing user preferences.json"
        self.preferences.write_bytes(b'{"language":"fr","notification_sound":false}\n')
        self.unrelated = self.binary.parent / "unrelated user file"
        self.unrelated.write_bytes(b"preserve unrelated standalone directory content\n")
        self.protected = {path: path.read_bytes() for path in
                          (*self.configs, self.history, self.preferences, self.unrelated, self.user_tool)}
        self.protected_identities = {path: identity(path) for path in self.protected}
        self.original_paths = {path.relative_to(self.home) for path in self.home.rglob("*")}

    def preserved(self):
        for path, content in self.protected.items():
            require(path.read_bytes() == content, f"portable update changed unrelated file/config: {path}")
            require(identity(path) == self.protected_identities[path],
                    f"portable update changed unrelated file permissions/ownership/identity: {path}")
        require({path.relative_to(self.home) for path in self.home.rglob("*")} == self.original_paths,
                "portable update created/deleted HOME files (receipt, autostart or preferences)")
        require(not list(self.root.rglob(".miccamwatch-install")), "portable update created a managed receipt")
        self.check_installer_cleanup()
        require(not list(self.root.rglob(".mcw-update-*")), "updater staging remained after exit")

    def installed(self, binary, old):
        current = identity(self.binary)
        require(self.binary.read_bytes() == binary, "installed bytes differ from the genuine candidate archive")
        for key in ("uid", "gid", "mode", "xattrs", "nlink"):
            require(current[key] == old[key], f"portable replacement changed target {key}")
        require(current["inode"] != old["inode"], "portable replacement was not an atomic new-file commit")
        self.preserved()
        return current


def transport(sandbox, version, asset, archive, manifest):
    return fixtures.fixture_environment(sandbox, {
        LATEST: {"effective_url": TAG_BASE + version},
        f"{fixtures.RELEASE_ROOT}/{version}/{asset}": str(archive),
        f"{fixtures.RELEASE_ROOT}/{version}/SHA256SUMS": str(manifest),
    })


def installer(proof, name, script, sandbox, env, version, expected=None, codes=(0,), target=None):
    target = target or sandbox.binary
    return record(proof, name, ["/bin/sh", script, "--update-portable", target,
                               "--current-sha256", expected or sha(target),
                               "--version", version, "--no-modify-path"],
                  env=env, cwd=sandbox.cwd, codes=codes)


def updater(proof, name, sandbox, env, codes=(0,)):
    # This is the consumer's actual invocation, including the space/quote path.
    return record(proof, name, ["./mcw", "--lang", "en", "update"],
                  env=env, cwd=sandbox.binary.parent, codes=codes)


def refused(result, message):
    require(result.returncode != 0, message)


def older_corrected_source(workspace, root, proof, cargo):
    copied = root / "private corrected older source"
    shutil.copytree(workspace, copied, ignore=shutil.ignore_patterns(".git", "target", "__pycache__"))
    manifest = copied / "Cargo.toml"
    text, count = re.subn(r'(\[package\]\s*\nname = "miccamwatch"\s*\nversion = ")[^"]+',
                          lambda match: match.group(1) + BASELINE[1:],
                          manifest.read_text(), count=1)
    require(count == 1, "cannot identify fixture workspace package version")
    manifest.write_text(text)
    lock = copied / "Cargo.lock"
    text, count = re.subn(r'(name = "miccamwatch"\nversion = ")[^"]+',
                          lambda match: match.group(1) + BASELINE[1:], lock.read_text())
    require(count == 1, "cannot identify fixture lockfile root package version")
    lock.write_text(text)
    source = copied / "src/updater/unix.rs"
    text = source.read_text()
    needle = 'Command::new("/usr/bin/curl")'
    require(text.count(needle) == 1, "latest-release transport convention changed; review native fixture")
    source.write_text(text.replace(needle, 'Command::new("curl")'))
    target = root / "private older build target"
    build_env = dict(os.environ, CARGO_TARGET_DIR=str(target))
    record(proof, "build-corrected-older-client", [cargo, "build", "--release", "--locked", "--bin", "mcw"],
           env=build_env, cwd=copied, timeout=1800)
    binary = target / "release/mcw"
    require(fixtures.run([binary, "--version"]).stdout.strip() == "mcw " + BASELINE[1:],
            "genuine source-built client does not report its fixture version")
    (proof / "older-client-provenance.json").write_text(json.dumps({
        "version": BASELINE, "sha256": sha(binary),
        "source": "private copy of current corrected workspace, excluding .git/target",
        "metadata_changes": ["Cargo.toml root version", "Cargo.lock root version"],
        "transport_change": 'latest_tag Command::new("/usr/bin/curl") -> Command::new("curl")',
        "production_checks_changed": False,
    }, indent=2) + "\n")
    print("Transport fixture: corrected older client selects PATH curl only for latest lookup; "
          "production candidate retains absolute /usr/bin/curl and genuine HTTPS latest lookup.")
    return binary.read_bytes()


def fixture_curl(arguments):
    # An atomic user replacement after Rust inspection, before installer
    # inspection. Both old and replacement payloads are real native executables.
    mutation = os.environ.get("MCW_TEST_NATIVE_MUTATION")
    if mutation:
        plan = json.loads(Path(mutation).read_text())
        marker = Path(plan["marker"])
        if plan["url"] in arguments and not marker.exists():
            os.replace(plan["replacement"], plan["target"])
            marker.write_text("genuine candidate atomically replaced by fixture user\n")
    return fixtures.fixture_curl(arguments)

def fixture_mv(arguments):
    result = subprocess.run(["/bin/mv", *arguments])
    plan = json.loads(Path(os.environ["MCW_TEST_NATIVE_MOVE"]).read_text())
    marker = Path(plan["marker"])
    if (result.returncode == 0 and len(arguments) >= 2
            and Path(arguments[-2]).name.startswith(".mcw-stage.")
            and arguments[-1] == plan["target"] and not marker.exists()):
        marker.write_text("real atomic replacement completed before injected SIGTERM\n")
        if plan["mode"] == "replacement":
            os.replace(plan["replacement"], plan["target"])
        elif plan["mode"] == "quarantine":
            set_xattr(plan["target"], "com.apple.quarantine", b"0000;fixture-user-change;MCW;")
        os.kill(os.getppid(), signal.SIGTERM)
    return result.returncode


def scenarios(args, root, proof, curl, cargo):
    workspace = Path(__file__).resolve().parents[2]
    asset, binary, sums = candidate(args.archive, args.version, root, os.environ)
    baseline_root = root / "immutable baseline download"
    baseline_root.mkdir()
    baseline_asset, old_binary, _ = fixtures.released_binary(curl, baseline_root, BASELINE)
    with tarfile.open(baseline_root / asset, "r:gz") as archive:
        members = [member for member in archive.getmembers() if member.name.removeprefix("./") == "mcw"]
        require(len(members) == 1 and members[0].isfile() and members[0].mode & 0o111,
                "public baseline archive must already mark its native mcw executable")
    require(asset == baseline_asset, "baseline and candidate architecture disagree")
    (proof / "baseline-provenance.json").write_text(json.dumps({
        "tag": BASELINE, "url": f"{fixtures.RELEASE_ROOT}/{BASELINE}/{asset}",
        "archive_sha256": sha(baseline_root / asset), "binary_sha256": hashlib.sha256(old_binary).hexdigest(),
        "manifest_verified": True,
    }, indent=2) + "\n")

    if args.public_release:
        public = Portable(root, "genuine public standalone migration", old_binary)
        before = identity(public.binary)
        installer(proof, "public-portable-bootstrap", args.source_file, public, public.env, args.version)
        after = public.installed(binary, before)
        updater(proof, "public-standalone-update-noop", public, public.env)
        require(identity(public.binary) == after, "public up-to-date call changed the installation")
        public.preserved()
        print(f"PASS genuine public standalone {BASELINE} -> {args.version}; production HTTPS bootstrap/update.")

    bootstrap = Portable(root, "published old client bootstrap", old_binary)
    env, _ = transport(bootstrap, args.version, asset, args.archive, sums)
    before = identity(bootstrap.binary)
    installer(proof, "explicit-portable-bootstrap", args.source_file, bootstrap, env, args.version)
    after = bootstrap.installed(binary, before)
    # This is the unmodified candidate, including absolute /usr/bin/curl. Its
    # latest-release lookup is genuine HTTPS, not a fixture reimplementation.
    updater(proof, "bootstrapped-production-update-noop", bootstrap, env)
    require(identity(bootstrap.binary) == after, "no-op replaced the candidate")
    bootstrap.preserved()

    unmarked = Portable(root, "standalone without quarantine", old_binary, quarantine=None)
    env, _ = transport(unmarked, args.version, asset, args.archive, sums)
    before = identity(unmarked.binary)
    installer(proof, "bootstrap-without-quarantine", args.source_file, unmarked, env, args.version)
    unmarked.installed(binary, before)

    corrected = older_corrected_source(workspace, root, proof, cargo)
    upgrade = Portable(root, "corrected client upgrade", corrected)
    for name, latest in (("same", BASELINE), ("newer", "v0.15.0")):
        env, _ = transport(upgrade, latest, asset, args.archive, sums)
        before = identity(upgrade.binary)
        result = updater(proof, "corrected-noop-" + name, upgrade, env)
        require("already up to date" in result.stdout, f"corrected {name} release was not a no-op")
        require(identity(upgrade.binary) == before, "corrected no-op changed executable")
        upgrade.preserved()
    env, _ = transport(upgrade, args.version, asset, args.archive, sums)
    before = identity(upgrade.binary)
    updater(proof, "corrected-true-in-place-upgrade", upgrade, env)
    upgrade.installed(binary, before)
    require(record(proof, "upgraded-version", ["./mcw", "--version"], env=env,
                   cwd=upgrade.binary.parent).stdout.strip() == "mcw " + args.version[1:], "upgraded CLI version differs")

    for mode in ("rollback", "replacement", "quarantine"):
        interrupted = Portable(root, "interrupted transaction " + mode, old_binary)
        env, _ = transport(interrupted, args.version, asset, args.archive, sums)
        wrapper = Path(env["PATH"].split(os.pathsep)[0]) / "mv"
        wrapper.write_text("#!/bin/sh\nexec " + shlex.quote(sys.executable) + " " +
                           shlex.quote(str(Path(__file__).resolve())) + ' --fixture-mv "$@"\n')
        wrapper.chmod(0o755)
        replacement = interrupted.root / "identical genuine candidate replacement"
        shutil.copy2(bootstrap.binary, replacement)
        expected_replacement = identity(replacement)
        marker = interrupted.root / "real-move-observed"
        plan = interrupted.root / "move-plan.json"
        plan.write_text(json.dumps(dict(mode=mode, target=str(interrupted.binary),
                                       replacement=str(replacement), marker=str(marker))))
        env["MCW_TEST_NATIVE_MOVE"] = str(plan)
        before = identity(interrupted.binary)
        installer(proof, "sigterm-" + mode, args.source_file, interrupted, env, args.version, codes=(143,))
        require(marker.is_file(), "SIGTERM scenario did not reach a real atomic replacement")
        backups = list(interrupted.binary.parent.glob(".mcw-backup.*"))
        after = identity(interrupted.binary)
        (proof / ("sigterm-" + mode + "-identity.json")).write_text(
            json.dumps(dict(before=before, after=after), indent=2) + "\n")
        if mode == "rollback":
            require(interrupted.binary.read_bytes() == old_binary and not backups,
                    "interrupted update did not restore the old genuine executable")
            for key in ("uid", "gid", "mode", "nlink", "xattrs"):
                require(after[key] == before[key],
                        f"rollback changed old target {key}: {before[key]!r} -> {after[key]!r}")
            record(proof, "rollback-usable-version", [interrupted.binary, "--version"], env=env)
        else:
            require(interrupted.binary.read_bytes() == binary and len(backups) == 1,
                    "rollback overwrote a concurrent replacement/quarantine change or lost recovery backup")
            require(backups[0].read_bytes() == old_binary, "recovery backup was not the previous genuine executable")
            if mode == "replacement":
                require(identity(interrupted.binary) == expected_replacement,
                        "rollback changed the identical-byte user replacement")
            else:
                require(get_xattr(interrupted.binary, "com.apple.quarantine") ==
                        b"0000;fixture-user-change;MCW;", "rollback removed a changed quarantine attribute")
            (proof / ("retained-backup-" + mode + ".json")).write_text(json.dumps({
                "path": str(backups[0]), "sha256": sha(backups[0]),
                "target": identity(interrupted.binary), "automatic_restore": False}, indent=2) + "\n")
            # The observer owns this entire fixture. Move only its intentional
            # recovery backup outside HOME before checking every other path.
            shutil.move(str(backups[0]), str(interrupted.root / "retained recovery executable"))
        interrupted.preserved()

    stale = Portable(root, "changed current hash", old_binary)
    env, _ = transport(stale, args.version, asset, args.archive, sums)
    old_hash = sha(stale.binary)
    stale.binary.write_bytes(binary)
    before = identity(stale.binary)
    result = installer(proof, "changed-current-hash-refusal", args.source_file, stale, env,
                       args.version, expected=old_hash, codes=None)
    refused(result, "installer replaced a target whose hash changed")
    require(identity(stale.binary) == before, "stale hash refusal changed user replacement")
    stale.preserved()

    race = Portable(root, "changed after updater inspection", corrected)
    env, _ = transport(race, args.version, asset, args.archive, sums)
    wrapper = Path(env["PATH"].split(os.pathsep)[0]) / "curl"
    wrapper.write_text("#!/bin/sh\nexec " + shlex.quote(sys.executable) + " " +
                       shlex.quote(str(Path(__file__).resolve())) + ' --fixture-curl "$@"\n')
    replacement = race.root / "genuine user replacement"
    shutil.copy2(bootstrap.binary, replacement)
    expected = identity(replacement)
    marker = race.root / "user replacement occurred"
    mutation = race.root / "mutation plan.json"
    mutation.write_text(json.dumps(dict(url=LATEST, replacement=str(replacement), target=str(race.binary), marker=str(marker))))
    env["MCW_TEST_NATIVE_MUTATION"] = str(mutation)
    result = updater(proof, "hash-race-refusal", race, env, codes=None)
    refused(result, "updater overwrote a target changed since inspection")
    require(marker.exists() and identity(race.binary) == expected, "hash-race refusal did not preserve genuine user replacement")
    race.preserved()

    running = Portable(root, "another real watcher running", corrected)
    env, _ = transport(running, args.version, asset, args.archive, sums)
    before = identity(running.binary)
    with (proof / "watcher.stdout").open("w") as stdout, (proof / "watcher.stderr").open("w") as stderr:
        watcher = subprocess.Popen(["./mcw", "--lang", "en", "watch", "--json", "--interval", "250", "--no-kill"],
                                   env=env, cwd=running.binary.parent, stdin=subprocess.DEVNULL,
                                   stdout=stdout, stderr=stderr, start_new_session=True)
        try:
            time.sleep(8)
            require(watcher.poll() is None, "native watcher exited before in-use test")
            for name, invoke in (("installer-running-refusal", lambda: installer(proof, "installer-running-refusal", args.source_file,
                                                                                running, env, args.version, codes=None)),
                                 ("updater-running-refusal", lambda: updater(proof, "updater-running-refusal", running, env, codes=None))):
                result = invoke()
                refused(result, name + " replaced a running target")
                require(identity(running.binary) == before and watcher.poll() is None,
                        "in-use refusal changed executable or stopped user's watcher")
                running.preserved()
            os.killpg(watcher.pid, signal.SIGINT)
            require(watcher.wait(timeout=30) == 0, "native watcher failed graceful SIGINT")
        finally:
            # Also reap helpers remaining in our owned session after the
            # watcher exits; no host/user monitor is ever targeted.
            kill_group(watcher.pid)
            watcher.wait(timeout=30)
    running.preserved()

    linked = Portable(root, "hardlinked executable", corrected)
    alias = linked.root / "other hardlink"
    os.link(linked.binary, alias)
    env, _ = transport(linked, args.version, asset, args.archive, sums)
    before = identity(linked.binary)
    for name, invoke in (("hardlink-installer", lambda: installer(proof, "hardlink-installer", args.source_file, linked, env, args.version, codes=None)),
                         ("hardlink-updater", lambda: updater(proof, "hardlink-updater", linked, env, codes=None))):
        refused(invoke(), name + " accepted multiply-linked target")
        require(identity(linked.binary) == before and identity(alias) == before, "hardlink refusal changed either link")
        linked.preserved()

    linked = Portable(root, "symlink executable", corrected)
    external = linked.root / "real user executable"
    linked.binary.rename(external)
    linked.binary.symlink_to(external)
    env, _ = transport(linked, args.version, asset, args.archive, sums)
    before = identity(external)
    result = installer(proof, "symlink-installer", args.source_file, linked, env, args.version,
                       expected=sha(external), codes=None)
    refused(result, "installer followed a symlink portable target")
    require(linked.binary.is_symlink() and linked.binary.readlink() == external and identity(external) == before,
            "symlink refusal changed link or destination")
    linked.preserved()

    for name, malformed in (("bin-no-receipt", False), ("bin-malformed-receipt", True)):
        layout = Portable(root, name, corrected)
        layout.binary.unlink()
        layout.binary = layout.prefix / "bin/mcw"
        layout.binary.write_bytes(corrected)
        layout.binary.chmod(0o755)
        receipt = layout.prefix / ".miccamwatch-install"
        if malformed:
            receipt.mkdir()
            (receipt / "format").write_text("not an installer receipt\n")
        layout.original_paths = {path.relative_to(layout.home) for path in layout.home.rglob("*")}
        env, log = transport(layout, args.version, asset, args.archive, sums)
        before = identity(layout.binary)
        result = updater(proof, name + "-updater", layout, env, codes=None)
        refused(result, name + " silently fell back to portable update")
        result = installer(proof, name + "-installer", args.source_file, layout, env, args.version, codes=None)
        refused(result, name + " installer accepted bin portable target")
        require(identity(layout.binary) == before and not log.exists(), name + " changed target/contacted releases")
        for path, content in layout.protected.items():
            require(path.read_bytes() == content, "bin refusal changed unrelated user file")
            require(identity(path) == layout.protected_identities[path], "bin refusal changed unrelated file metadata")
        require({path.relative_to(layout.home) for path in layout.home.rglob("*")} == layout.original_paths,
                "bin refusal changed installation contents")
        if malformed:
            require((receipt / "format").read_bytes() == b"not an installer receipt\n", "refusal rewrote invalid receipt")
        layout.check_installer_cleanup()


def main():
    if len(sys.argv) > 1 and sys.argv[1] == "--fixture-curl":
        return fixture_curl(sys.argv[2:])
    if len(sys.argv) > 1 and sys.argv[1] == "--fixture-mv":
        return fixture_mv(sys.argv[2:])
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--source-file", type=Path, required=True, help="absolute reviewed current installer")
    parser.add_argument("--archive", type=Path, required=True, help="absolute genuine current macOS release archive")
    parser.add_argument("--version", required=True, help="candidate tag, v0.16.1 or newer")
    parser.add_argument("--public-release", action="store_true",
                        help="also bootstrap through genuine production HTTPS release downloads; candidate must be public latest")
    parser.add_argument("--proof", type=Path, required=True, help="persistent logs outside temporary fixture HOME")
    args = parser.parse_args()
    require(platform.system() == "Darwin" and os.getuid() == os.geteuid() != 0, "run natively on macOS as an ordinary user")
    require(re.fullmatch(r"v[0-9]+\.[0-9]+\.[0-9]+", args.version)
            and tuple(map(int, args.version[1:].split("."))) > (0, 16, 0), "candidate must be newer than v0.16.0")
    require(args.source_file.is_absolute() and args.source_file.is_file() and args.archive.is_absolute() and args.archive.is_file(),
            "installer/archive must be explicit existing absolute paths")
    args.source_file = args.source_file.resolve()
    args.archive = args.archive.resolve()
    args.proof = args.proof.resolve()
    args.proof.mkdir(parents=True, exist_ok=True)
    curl, cargo = shutil.which("curl"), shutil.which("cargo")
    require(curl and cargo, "native proof requires existing curl and Cargo")
    (args.proof / "scope.json").write_text(json.dumps({
        "host": platform.platform(), "architecture": platform.machine(), "candidate": args.version,
        "production_candidate_sha256": sha(args.archive),
        "baseline": "immutable genuine public v0.16.0, authenticated by published SHA256SUMS",
        "older_client": "genuine current corrected source built with private v0.16.0 metadata and transport-only curl selection",
        "limitations": ["not historical v0.16.0 portable support", "not active Gatekeeper/quarantine UI approval",
                        "candidate no-op uses genuine public latest; candidate must be same/newer than public latest",
                        "no physical capture or device access transition test", "no enabled autostart registration in this proof"],
    }, indent=2) + "\n")
    def interrupted(signum, _frame):
        raise SystemExit(128 + signum)

    for signum in (signal.SIGTERM, signal.SIGHUP):
        signal.signal(signum, interrupted)
    # Use private owned ancestry, not macOS world-writable /private/tmp.
    with tempfile.TemporaryDirectory(prefix="mcw native portable update ", dir=Path.home().resolve()) as temporary:
        root = Path(temporary).resolve()
        scenarios(args, root, args.proof, curl, cargo)
    print(f"PASS native macOS portable update: {platform.machine()}, immutable {BASELINE} bootstrap and corrected-client upgrade to {args.version}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
