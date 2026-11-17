#!/usr/bin/env python3
"""Audit all shipped Linux ELF images and the actual Ubuntu 22.04 loader."""
import argparse
import json
import os
from pathlib import Path
import platform
import re
import subprocess

from hybrid_payload import check, verify

FLOOR = {"GLIBC": (2, 35), "GLIBCXX": (3, 4, 30), "CXXABI": (1, 3, 13)}
MACHINES = {
    "x86_64": ("Advanced Micro Devices X86-64", "/lib64/ld-linux-x86-64.so.2"),
    "aarch64": ("AArch64", "/lib/ld-linux-aarch64.so.1"),
}
ELF_COMPONENTS = {"mcw", "mcw-camera-helper", "desktop/miccamwatch_desktop",
                  "desktop/lib/libapp.so", "desktop/lib/libflutter_linux_gtk.so"}


def command(*argv):
    return subprocess.check_output(argv, text=True, env={**os.environ, "LC_ALL": "C"}, timeout=30)


def audit(bundle, architecture):
    target = f"linux-{architecture}"
    manifest = verify(bundle, target)
    os_release = Path("/etc/os-release").read_text()
    glibc = command("getconf", "GNU_LIBC_VERSION").strip()
    check(re.search(r"(?m)^ID=ubuntu$", os_release)
          and re.search(r'(?m)^VERSION_ID="?22\.04"?$', os_release)
          and glibc == "glibc 2.35" and platform.machine() == architecture,
          "native Ubuntu 22.04 ABI validation host required")
    machine, loader = MACHINES[architecture]
    results = []
    for name in manifest["files"]:
        path = bundle / name
        with path.open("rb") as stream:
            is_elf = stream.read(4) == b"\x7fELF"
        check(is_elf == (name in ELF_COMPONENTS), f"ELF identity mismatch: {name}")
        if not is_elf:
            continue
        header = command("readelf", "-hW", str(path))
        check("ELF64" in header and re.search(r"Machine:\s+" + machine, header), f"wrong architecture: {name}")
        program = command("readelf", "-lW", str(path))
        interpreters = re.findall(r"Requesting program interpreter:\s*([^\]]+)", program)
        expected = [loader] if name in {"mcw", "mcw-camera-helper", "desktop/miccamwatch_desktop"} else []
        check(interpreters == expected, f"bad ELF interpreter: {name}: {interpreters}")
        version_data = command("readelf", "-VW", str(path))
        requires = version_data.partition("Version needs section")[2]
        versions = {}
        for family, version in re.findall(r"Name: (CXXABI|GLIBCXX|GLIBC)_([0-9.]+)", requires):
            numeric = tuple(map(int, version.split(".")))
            check(numeric <= FLOOR[family], f"{name} requires {family}_{version} above Ubuntu 22.04")
            versions[family] = max(versions.get(family, ()), numeric)
        check("GLIBC_PRIVATE" not in requires, f"{name} imports a private glibc symbol")
        dynamic = command("readelf", "-dW", str(path))
        needed = re.findall(r"\(NEEDED\).*\[(.*?)\]", dynamic)
        runpaths = re.findall(r"\((?:RUNPATH|RPATH)\).*\[(.*?)\]", dynamic)
        for search in runpaths:
            check(all(path in {"$ORIGIN", "$ORIGIN/lib"} for path in search.split(":")),
                  f"unapproved RUNPATH: {name}: {search}")
        linked = command("ldd", str(path)) if needed else ""
        check("not found" not in linked, f"Ubuntu 22.04 dependency missing: {name}")
        results.append({
            "path": name, "machine": machine, "interpreter": interpreters, "needed": needed,
            "required_versions": {key: ".".join(map(str, v)) for key, v in versions.items()},
            "search_paths": runpaths, "resolved_on_baseline": linked.splitlines(),
        })
    return {"status": "passed", "revision": manifest["revision"], "target": target,
            "host": "Ubuntu 22.04", "glibc": glibc, "elf": results}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("bundle", type=Path)
    parser.add_argument("--arch", choices=MACHINES, required=True)
    parser.add_argument("--receipt", type=Path, required=True)
    args = parser.parse_args()
    result = audit(args.bundle.resolve(strict=True), args.arch)
    args.receipt.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(f"{result['target']}: {len(result['elf'])} images verified with {result['glibc']}")


if __name__ == "__main__":
    main()
