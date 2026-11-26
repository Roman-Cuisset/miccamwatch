"""Collect only the qualified hybrid packages from one GitHub Actions run."""

import argparse
import json
from pathlib import Path
import re
from shutil import copyfile

from hybrid_payload import release_metadata, sha


PACKAGES = {
    "windows-x86_64": ("windows-x86_64", (".zip", ".msi")),
    "linux-x86_64": ("linux-x86_64", (".tar.gz",)),
    "linux-aarch64": ("linux-aarch64", (".tar.gz",)),
    "macos-x86_64": ("macos-x86_64", (".tar.gz", ".dmg")),
    "macos-arm64": ("macos-aarch64", (".tar.gz", ".dmg")),
}


def validate_replay(receipt, revision):
    """Accept only a build whose lone failed gate is the Windows startup replay."""
    run = json.loads(receipt.read_text(encoding="utf-8-sig"))
    if (run["headSha"], run["headBranch"], run["workflowName"], run["event"]) != (
        revision, "work/flutter-hybrid-0220", "CI", "workflow_dispatch"
    ):
        raise ValueError("Unexpected build workflow or revision")
    required_prefixes = (
        "release-candidate-audit",
        "flutter-qualification / macOS 15 arm64 owned qualification",
        "flutter-qualification / macOS 15 x86_64 owned qualification",
        "flutter-qualification / Ubuntu 22.04 ARM64",
        "flutter-qualification / Ubuntu 22.04 x64",
    )
    jobs = run["jobs"]
    for prefix in required_prefixes:
        matching = [job for job in jobs if job["name"].startswith(prefix)]
        if len(matching) != 1 or matching[0]["conclusion"] != "success":
            raise ValueError(f"Missing successful source gate: {prefix}")
    windows = [job for job in jobs if job["name"] == "flutter-windows-qualification / windows"]
    if len(windows) != 1 or windows[0]["conclusion"] != "failure":
        raise ValueError("Unexpected state of the original Windows job")
    steps = {step["name"]: step["conclusion"] for step in windows[0]["steps"]}
    if {name for name, outcome in steps.items() if outcome == "failure"} != {
        "Build distinct semantic proof UI and qualify isolated lifecycle"
    }:
        raise ValueError("Windows startup was not the sole failed step")
    for name in (
        "Check Rust and actual native release imports",
        "Build hybrid Rust and normal Flutter release",
        "Build the complete hybrid ZIP and MSI and verify both container payloads",
        "Install official v0.21.0 and exercise installed hybrid upgrade, rollback and uninstall",
        "Preserve exact hybrid Windows distribution and installation receipts",
        "Preserve only qualification evidence, never publish a release",
    ):
        if steps.get(name) != "success":
            raise ValueError(f"Missing successful Windows source gate: {name}")


def single_file(root, name):
    matches = list(root.rglob(name))
    if len(matches) != 1 or not matches[0].is_file() or matches[0].is_symlink():
        raise ValueError(f"{root.name}: expected exactly one regular {name}")
    return matches[0]


def collect(source, output, revision):
    if not re.fullmatch(r"[0-9a-f]{40}", revision):
        raise ValueError("Candidate source must be a full Git SHA")
    version, ipc = release_metadata()
    if ipc != 10:
        raise ValueError("The v0.22.0 package set requires IPC 10")
    files = {}
    for artifact, (target, suffixes) in PACKAGES.items():
        root = source / f"miccamwatch-hybrid-{artifact}-{revision}"
        manifest = json.loads(single_file(root, "miccamwatch-bundle.json").read_text(encoding="utf-8"))
        if (manifest["target"], manifest["revision"], manifest["version"], manifest["ipc"]) != (
            target, revision, version, ipc
        ):
            raise ValueError(f"Wrong bundle identity: {target}")
        checksums = single_file(root, "SHA256SUMS")
        found = {}
        for entry in checksums.read_text(encoding="ascii").splitlines():
            match = re.fullmatch(r"([0-9a-f]{64})  ([a-z0-9_.-]+)", entry)
            if not match or match[2] in found:
                raise ValueError(f"Invalid checksum manifest: {target}")
            found[match[2]] = match[1]

        package_names = {f"miccamwatch-hybrid-{target}{suffix}" for suffix in suffixes}
        if set(found) != package_names:
            raise ValueError(f"Wrong package set: {target}")
        for name in sorted(package_names):
            package = single_file(root, name)
            if not package.stat().st_size or sha(package) != found[name]:
                raise ValueError(f"Checksum mismatch: {name}")
            files[name] = package

    output.mkdir()
    for name, package in files.items():
        copyfile(package, output / name)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--validate-replay", type=Path, metavar="RUN_JSON")
    parser.add_argument("revision")
    parser.add_argument("input", type=Path, nargs="?")
    parser.add_argument("output", type=Path, nargs="?")
    args = parser.parse_args()
    if args.validate_replay:
        validate_replay(args.validate_replay, args.revision)
    elif args.input and args.output:
        collect(args.input, args.output, args.revision)
    else:
        parser.error("provide a source run receipt or both input and output paths")


if __name__ == "__main__":
    main()
