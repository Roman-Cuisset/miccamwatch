#!/usr/bin/env python3
"""Exercise a normal signed hybrid app at its installed path in a hosted Mac."""
import argparse
from contextlib import contextmanager
import ctypes
import hashlib
import json
import os
from pathlib import Path
import platform
import plistlib
import re
import shutil
import subprocess
import tempfile
import time

STABLE = {
    "aarch64": "a378bb386452112679fe4a3be896d5a1c216fe9bb7d754c4a390ca33cc39d6b3",
    "x86_64": "34c9ad2f4c113810f3ef3d695eed554b0bb220385676124427dab8dfcc548969",
}
APP = "MicCamWatch.app"


def require(test, message):
    if not test:
        raise RuntimeError(message)


def command(*args, **options):
    return subprocess.run([str(value) for value in args], check=True, timeout=120, **options)


def digest(file):
    with file.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


@contextmanager
def attached(dmg, mount):
    mount.mkdir()
    command("/usr/bin/hdiutil", "attach", dmg, "-readonly", "-nobrowse", "-mountpoint", mount)
    try:
        yield mount / APP
    finally:
        command("/usr/bin/hdiutil", "detach", mount)


def version(app):
    command("/usr/bin/codesign", "--verify", "--deep", "--strict", app)
    info = plistlib.loads((app / "Contents/Info.plist").read_bytes())
    require(info["CFBundleIdentifier"] == "com.roman-cuisset.miccamwatch", "app identity mismatch")
    executable = app / "Contents/MacOS/mcw"
    result = command(executable, "--version", capture_output=True, text=True)
    expected = info["CFBundleShortVersionString"]
    require(result.stdout.strip() == f"mcw {expected}", "app and engine version mismatch")
    return expected


def inspect_embedded_helper(app, direct):
    engine = app / "Contents/MacOS/mcw"
    parts = ("Info.plist", "MacOS/MicCamWatchHelper", "_CodeSignature/CodeResources")
    image = engine.read_bytes()
    build = direct.parent / "build"
    candidates = set(build.glob("miccamwatch-*/out/MicCamWatchHelper.app")) | set(
        build.glob("miccamwatch/*/out/MicCamWatchHelper.app"))
    matching = [helper for helper in candidates
                if all((helper / "Contents" / part).read_bytes() in image for part in parts)]
    require(len(matching) == 1, "could not identify the exact signed Swift helper embedded in the packaged engine")
    embedded, = matching
    helper = app / "Contents/Helpers/MicCamWatchHelper.app"
    require(all((helper / "Contents" / part).read_bytes() ==
                (embedded / "Contents" / part).read_bytes() for part in parts),
            "installed notification helper differs from the exact embedded signed helper")
    binary = helper / "Contents/MacOS/MicCamWatchHelper"
    plist = plistlib.loads((helper / "Contents/Info.plist").read_bytes())
    identity = "com.roman-cuisset.miccamwatch.helper"
    require(plist["CFBundleIdentifier"] == identity and plist["CFBundlePackageType"] == "APPL"
            and plist["CFBundleExecutable"] == binary.name
            and all(plist.get(key) for key in ("NSCameraUsageDescription", "NSMicrophoneUsageDescription")),
            "Swift helper bundle identity or capture usage descriptions differ")
    command("/usr/bin/codesign", "--verify", "--deep", "--strict", helper)
    signature = command("/usr/bin/codesign", "-d", "--verbose=4", helper, capture_output=True, text=True)
    require(f"Identifier={identity}" in signature.stderr.splitlines(), "Swift helper signing identifier differs")
    entitlement = command("/usr/bin/codesign", "-d", "--entitlements", "-", helper, capture_output=True)
    require(not entitlement.stdout or not plistlib.loads(entitlement.stdout),
            "Unexpected entitlement in the native notification/capture helper")
    frameworks = ("CoreAudio", "CoreMediaIO", "AVFoundation", "UserNotifications")
    linkage = command("/usr/bin/otool", "-L", binary, capture_output=True, text=True)
    require(all(f"/{name}.framework/" in linkage.stdout for name in frameworks),
            "Missing native capture or user notification framework")
    return {"bundle_id": identity, "signature_verified": True, "entitlements": {},
            "frameworks": frameworks, "engine_sha256": digest(engine), "helper_sha256": digest(binary),
            "installed_bundle": str(helper), "matches_embedded_bytes": True}


def inspect_collectors(engine, env):
    probe = subprocess.run([engine, "status", "--json"], env=env, capture_output=True, timeout=60)
    require(probe.returncode in (0, 1, 2), "native status process failed")
    status = json.loads(probe.stdout)
    collectors = {item["collector"]: item for item in status["collectors"]}
    for name in ("coreaudio_input", "coremediaio_video"):
        item = collectors.get(name, {})
        require(item.get("state") in {"healthy", "degraded", "unavailable"}
                and not item.get("detail", "").startswith("macOS capture helper unavailable"),
                f"{name} did not produce a native response")
        require(item["state"] == "healthy" or item.get("detail"), f"{name} lacks a native diagnostic")
    for access in status["accesses"]:
        if access["resource"] == "camera":
            require(access["application"] == "Unknown application" and access.get("pid") is None,
                    "reported camera application without native attribution")
    return {name: item["state"] for name, item in collectors.items()}


def inspect_notification_settings(engine, env):
    output = subprocess.check_output(
        [engine, "notifications", "native-status"], env=env, text=True, timeout=45
    )
    rows = dict(line.split("=", 1) for line in output.splitlines())
    require(rows.get("macos_authorization") in {
        "not_determined", "denied", "authorized", "authorized_without_alerts", "provisional", "unknown"
    }, "Missing native notification authorization state")
    require(rows.get("banner_delivery") == "unverified", "Banner delivery was not observed")
    return rows


def exchange(left, right):
    # macOS renamex_np(RENAME_SWAP); refuse rather than copying part of an app.
    api = ctypes.CDLL(None, use_errno=True).renamex_np
    api.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_uint]
    api.restype = ctypes.c_int
    if api(os.fsencode(left), os.fsencode(right), 2):
        error = ctypes.get_errno()
        raise OSError(error, os.strerror(error))


def processes():
    libproc = ctypes.CDLL("/usr/lib/libproc.dylib")
    libproc.proc_pidpath.argtypes = [ctypes.c_int, ctypes.c_void_p, ctypes.c_uint32]
    libproc.proc_pidpath.restype = ctypes.c_int
    listing = command("/bin/ps", "-axo", "pid=,ppid=", capture_output=True, text=True)
    for line in listing.stdout.splitlines():
        pid, parent = map(int, line.split())
        path = ctypes.create_string_buffer(4096)
        if libproc.proc_pidpath(pid, path, len(path)) > 0:
            yield pid, parent, Path(os.fsdecode(path.value))


def until(probe, explanation):
    end = time.monotonic() + 35
    while time.monotonic() < end:
        result = probe()
        if result:
            return result
        time.sleep(.2)
    raise RuntimeError(explanation)


def run_app(app, env, proof):
    engine = app / "Contents/MacOS/mcw"
    launcher = app / "Contents/MacOS/MicCamWatch"
    flutter = app / "Contents/Helpers/MicCamWatch.app/Contents/MacOS/MicCamWatch"
    with proof.open("xb") as log:
        command(launcher, env=env, stdout=log, stderr=subprocess.STDOUT)

        def ready():
            current = list(processes())
            owners = [pid for pid, _, path in current if path == engine]
            if len(owners) != 1:
                return False
            children = [pid for pid, parent, path in current if parent == owners[0] and path == flutter]
            if len(children) != 1:
                return False
            if not re.search(rf"desktop event=ready endpoint=parent_received pid={children[0]} ",
                             proof.read_text(errors="replace")):
                return False
            return owners[0], children[0]

        try:
            owner, child = until(ready, "installed app did not establish authenticated Flutter readiness")
            command(engine, "tray", "status", env=env)
        finally:
            if any(path == engine for _, _, path in processes()):
                command(engine, "tray", "stop", env=env)
                until(lambda: not any(path.is_relative_to(app) for _, _, path in processes()),
                      "installed app left owned bundle processes")
        return {"owner": owner, "flutter": child, "rust_acknowledged_ready": True,
                "owner_quit": "cooperative CLI", "processes_released": True}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("dmg", type=Path)
    parser.add_argument("--direct-engine", type=Path, required=True)
    parser.add_argument("--proof", type=Path, required=True)
    args = parser.parse_args()
    require(os.environ.get("RUNNER_ENVIRONMENT") == "github-hosted"
            and platform.system() == "Darwin" and os.getuid() != 0, "disposable Mac required")
    arch = "aarch64" if platform.machine() == "arm64" else platform.machine()
    proof = args.proof.resolve()
    proof.mkdir()
    work = Path(tempfile.mkdtemp(prefix="mcw-hybrid-", dir=Path.home())).resolve()
    stable = work / "stable.dmg"
    candidate = args.dmg.resolve(strict=True)
    source = os.environ["GITHUB_SHA"]
    report = {"status": "failed", "source": source, "target": arch}
    try:
        applications = Path("/Applications")
        metadata = applications.lstat()
        report["system_applications"] = {"uid": metadata.st_uid, "gid": metadata.st_gid,
                                         "mode": f"{metadata.st_mode & 0o7777:04o}",
                                         "symlink": applications.is_symlink()}
        with (proof / "system-applications-acl.log").open("xb") as log:
            command("/bin/ls", "-ldeO", applications, stdout=log)
        command("/usr/bin/curl", "--fail", "--location", "--silent", "--show-error", "--output", stable,
                f"https://github.com/Roman-Cuisset/miccamwatch/releases/download/v0.21.0/miccamwatch-macos-{arch}.dmg")
        require(digest(stable) == STABLE[arch], "public stable disk image checksum mismatch")
        report.update(baseline_sha256=STABLE[arch], candidate_sha256=digest(candidate))
        installed = work / "Applications" / APP
        staged = work / "staged" / APP
        for path in (installed, staged):
            path.parent.mkdir()
        with attached(stable, work / "stable-mount") as app:
            command("/usr/bin/ditto", app, installed)
        with attached(candidate, work / "candidate-mount") as app:
            command("/usr/bin/ditto", app, staged)
        require(version(installed) == "0.21.0" and version(staged) == "0.22.0", "wrong upgrade fixtures")
        altered = work / "altered" / APP
        altered.parent.mkdir()
        command("/usr/bin/ditto", staged, altered)
        (altered / "Contents/Resources/LICENSE").write_text("deliberately altered in fixture")
        bad_seal = subprocess.run(["/usr/bin/codesign", "--verify", "--deep", "--strict", str(altered)],
                                  capture_output=True, timeout=30)
        require(bad_seal.returncode != 0 and version(installed) == "0.21.0",
                "altered app was admitted before the update")
        report["altered_seal_refused"] = True
        metadata = json.loads((staged / "Contents/Resources/miccamwatch-bundle.json").read_bytes())
        require(metadata["revision"] == source and metadata["ipc"] == 10, "unexpected package revision or IPC")
        profile = work / "home"
        profile.mkdir(mode=0o700)
        env = dict(os.environ, HOME=str(profile), LANG="en_US.UTF-8", LC_ALL="en_US.UTF-8")
        for kind in ("CONFIG", "DATA", "STATE", "CACHE"):
            env[f"XDG_{kind}_HOME"] = str(profile / kind.lower())
        env["XDG_RUNTIME_DIR"] = str(profile / "run")
        Path(env["XDG_RUNTIME_DIR"]).mkdir(mode=0o700)
        config = profile / "config/MicCamWatch/settings.toml"
        history = profile / "data/MicCamWatch/history.jsonl"
        for file in (config, history):
            file.parent.mkdir(parents=True, mode=0o700)
        config.write_text("notifications_enabled = false\nhistory_enabled = false\nmute_on_lock = false\n")
        history.write_text('{"fixture":"persistent history"}\n')
        data = {file: digest(file) for file in (config, history)}
        exchange(installed, staged)
        require(version(installed) == "0.22.0" and version(staged) == "0.21.0", "whole-app update failed")
        report["notification_capture_helper"] = inspect_embedded_helper(installed, args.direct_engine)
        report["direct_collectors"] = inspect_collectors(args.direct_engine.resolve(strict=True), env)
        report["installed_collectors"] = inspect_collectors(installed / "Contents/MacOS/mcw", env)
        report["installed_notification_settings"] = inspect_notification_settings(
            installed / "Contents/MacOS/mcw", env
        )
        report["native_notification_delivery"] = "not exercised"
        first = run_app(installed, env, proof / "installed-first.log")
        exchange(installed, staged)
        require(version(installed) == "0.21.0", "whole-app rollback lost the stable app")
        exchange(installed, staged)
        require(version(installed) == "0.22.0", "whole-app opt-in reinstall failed")
        second = run_app(installed, env, proof / "installed-reopen.log")
        require(first["owner"] != second["owner"] and first["flutter"] != second["flutter"],
                "new launch did not establish a new owned engine and Flutter child")
        shutil.rmtree(installed)
        require(not installed.exists() and not any(path.is_relative_to(installed) for _, _, path in processes()),
                "app removal retained an installed payload or process")
        require(all(digest(file) == expected for file, expected in data.items()), "user state changed")
        report.update(status="passed", installed=[first, second], whole_app_upgrade=True,
                      whole_app_rollback=True, whole_app_removal=True, user_data_preserved=True,
                      ad_hoc=True, developer_id=False, notarized=False, tcc_changes=False)
    except Exception as error:
        report["error"] = str(error)
        raise
    finally:
        (proof / "result.json").write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
