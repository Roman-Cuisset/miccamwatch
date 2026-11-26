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
    parser.add_argument("input", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("revision")
    args = parser.parse_args()
    collect(args.input, args.output, args.revision)


if __name__ == "__main__":
    main()
