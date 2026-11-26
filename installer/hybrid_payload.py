#!/usr/bin/env python3
"""Assemble and verify the exact payload of an uninstrumented hybrid build."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import stat
import subprocess
import tarfile
import zipfile

REPO = Path(__file__).resolve().parents[1]
MANIFEST = "miccamwatch-bundle.json"
ASSETS = {
    "AssetManifest.bin", "FontManifest.json", "NativeAssetsManifest.json", "NOTICES.Z",
    "assets/miccamwatch.png", "fonts/MaterialIcons-Regular.otf",
    "shaders/ink_sparkle.frag", "shaders/stretch_effect.frag",
}
FLUTTER = {
    "windows-x86_64": {"miccamwatch_desktop.exe", "flutter_windows.dll", "data/app.so"},
    "linux-x86_64": {"miccamwatch_desktop", "lib/libapp.so", "lib/libflutter_linux_gtk.so",
                     "data/flutter_assets/version.json"},
}
FLUTTER["linux-aarch64"] = FLUTTER["linux-x86_64"]
CAMERA = {"mcw-camera-helper", "install-camera-helper.sh",
          "com.roman-cuisset.miccamwatch.camera.policy"}
DOCS = {"LICENSE", "README.md"}


def flutter_files(target):
    return FLUTTER[target] | {"data/icudtl.dat"} | {
        "data/flutter_assets/" + name for name in ASSETS
    }


def expected_files(target):
    contract = json.loads((REPO / "installer/hybrid_layout.json").read_text(encoding="utf-8"))
    return set(contract[target])


def check(condition, message):
    if not condition:
        raise ValueError(message)


def digest(stream):
    output = hashlib.sha256()
    while block := stream.read(1024 * 1024):
        output.update(block)
    return output.hexdigest()


def sha(path):
    with path.open("rb") as stream:
        return digest(stream)


def check_stat(info, path, expected, *, allow_hardlink=False):
    check(expected(info.st_mode) and not getattr(info, "st_file_attributes", 0)
          & stat.FILE_ATTRIBUTE_REPARSE_POINT, f"unexpected file type or link: {path}")
    check(allow_hardlink or not stat.S_ISREG(info.st_mode) or info.st_nlink == 1, f"hardlinked file: {path}")


def paths(root, expected):
    check_stat(root.lstat(), root, stat.S_ISDIR)
    result = {}
    allowed_dirs = {str(parent) for name in expected for parent in Path(name).parents}
    pending = [root]
    while pending:
        for path in sorted(pending.pop().iterdir()):
            name = path.relative_to(root).as_posix()
            info = path.lstat()
            if stat.S_ISDIR(info.st_mode):
                check_stat(info, path, stat.S_ISDIR)
                check(str(Path(name)) in allowed_dirs, f"unexpected directory: {name}")
                pending.append(path)
            else:
                check_stat(info, path, stat.S_ISREG)
                check(name in expected, f"unexpected component: {name}")
                result[name] = path
    check(result.keys() == expected, f"missing components: {sorted(expected - result.keys())}")
    return result


def release_metadata():
    cargo = (REPO / "Cargo.toml").read_text(encoding="utf-8")
    flutter = (REPO / "desktop/flutter/pubspec.yaml").read_text(encoding="utf-8")
    rust_protocol = (REPO / "src/frontends/desktop.rs").read_text(encoding="utf-8")
    dart_dir = REPO / "desktop/flutter/lib"
    dart_protocols = [int(number) for path in dart_dir.glob("*.dart") for number in
                      re.findall(r"\bprotocolVersion\s*=\s*(\d+);",
                                 path.read_text(encoding="utf-8"))]
    version = re.search(r'^version = "(\d+\.\d+\.\d+)"$', cargo, re.M)[1]
    dart_version = re.search(r"^version: (\d+\.\d+\.\d+)\+\d+$", flutter, re.M)[1]
    protocol = int(re.search(r"^pub const PROTOCOL_VERSION: u8 = (\d+);$", rust_protocol, re.M)[1])
    check(dart_version == version, "Cargo and Flutter versions differ")
    check(dart_protocols == [protocol], "Rust and Flutter protocol versions differ")
    return version, protocol


def check_executable_version(path, expected):
    result = subprocess.run([str(path.resolve(strict=True)), "--version"], check=True,
                            capture_output=True, text=True, timeout=10)
    check(result.stdout.strip() == expected and not result.stderr,
          f"binary version mismatch: {path.name}")


def assemble(target, native, flutter, root, revision):
    check(re.fullmatch(r"[0-9a-f]{40}", revision), "full Git SHA required")
    version, protocol = release_metadata()
    frontend = paths(flutter, flutter_files(target))
    is_windows = target.startswith("windows")
    engine = native / ("mcw.exe" if is_windows else "mcw")
    check_stat(engine.lstat(), engine, stat.S_ISREG, allow_hardlink=True)
    check_executable_version(engine, f"mcw {version}")
    source = {engine.name: engine}
    if is_windows:
        tray = native / "mcw-tray.exe"
        check_stat(tray.lstat(), tray, stat.S_ISREG, allow_hardlink=True)
        check_executable_version(tray, f"mcw-tray {version}")
        source[tray.name] = tray
    else:
        helper = native / "mcw-camera-helper"
        check_stat(helper.lstat(), helper, stat.S_ISREG, allow_hardlink=True)
        source[helper.name] = helper
        source.update({name: REPO / "installer" / name for name in CAMERA - {helper.name}})
    source.update({name: REPO / name for name in DOCS})
    source.update({"desktop/" + name: path for name, path in frontend.items()})
    check(source.keys() == expected_files(target), "builder and package contract disagree")
    root.mkdir(parents=True)
    sums = {}
    for relative, path in sorted(source.items()):
        check_stat(path.lstat(), path, stat.S_ISREG, allow_hardlink=True)
        output = root / relative
        output.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(path, output)
        executable = relative in {"mcw", "mcw-camera-helper", "install-camera-helper.sh",
                                  "desktop/miccamwatch_desktop"}
        output.chmod(0o755 if executable else 0o644)
        sums[relative] = sha(output)
    manifest = root / MANIFEST
    manifest.write_text(json.dumps({
        "schema": 1, "version": version, "ipc": protocol, "target": target,
        "revision": revision, "files": sums,
    }, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    manifest.chmod(0o644)
    return verify(root, target, revision)


def verify(root, target, revision=None, *, current=True):
    expected = expected_files(target)
    actual = paths(root, expected | {MANIFEST})
    manifest = actual.pop(MANIFEST)
    check(manifest.stat().st_size <= 16 * 1024, "oversized bundle manifest")
    data = json.loads(manifest.read_text(encoding="utf-8"), object_pairs_hook=unique_pairs)
    check(isinstance(data, dict)
          and set(data) == {"schema", "version", "ipc", "target", "revision", "files"},
          "invalid bundle manifest structure")
    version, protocol = release_metadata()
    revision_value = data["revision"]
    version_value = data["version"]
    ipc = data["ipc"]
    check(isinstance(version_value, str) and re.fullmatch(r"[0-9]+[.][0-9]+[.][0-9]+", version_value)
          and type(ipc) is int and 0 < ipc <= 255
          and (not current or (version_value, ipc) == (version, protocol))
          and type(data["schema"]) is int and data["schema"] == 1
          and data["target"] == target and isinstance(revision_value, str)
          and re.fullmatch(r"[0-9a-f]{40}", revision_value)
          and (revision is None or revision_value == revision),
          "invalid bundle identity")
    check(isinstance(data["files"], dict) and data["files"].keys() == expected,
          "manifest payload differs from platform contract")
    for relative, file in actual.items():
        check(sha(file) == data["files"][relative], f"bundle checksum mismatch: {relative}")
    return data


def unique_pairs(pairs):
    result = {}
    for key, value in pairs:
        check(key not in result, f"duplicate manifest key: {key}")
        result[key] = value
    return result


def verify_zip(archive, root, target):
    expected = paths(root, expected_files(target) | {MANIFEST})
    with zipfile.ZipFile(archive) as infile:
        actual = infile.infolist()
        check(len(actual) == len(expected) and {row.filename for row in actual} == expected.keys(),
              "ZIP contains an extra, duplicate, missing or unsafe path")
        for member in actual:
            file = expected[member.filename]
            check(member.file_size == file.stat().st_size, f"unexpected ZIP file size: {member.filename}")
            check(not stat.S_ISLNK(member.external_attr >> 16), f"ZIP link: {member.filename}")
            with infile.open(member) as stream:
                check(digest(stream) == sha(file), f"ZIP bytes differ: {member.filename}")


def verify_tar(archive, root, target):
    expected = paths(root, expected_files(target) | {MANIFEST})
    seen = set()
    with tarfile.open(archive, mode="r|gz") as stream:
        for member in stream:
            name = member.name
            check(name in expected and name not in seen and member.isreg(),
                  f"unexpected TAR member: {name}")
            file = expected[name]
            check(member.mode == stat.S_IMODE(file.stat().st_mode)
                  and member.size == file.stat().st_size, f"unexpected TAR metadata: {name}")
            check(digest(stream.extractfile(member)) == sha(file), f"TAR bytes differ: {name}")
            seen.add(name)
    check(seen == expected.keys(), f"missing TAR components: {sorted(expected.keys() - seen)}")


def archive_bundle(root, target, output):
    expected = paths(root, expected_files(target) | {MANIFEST})
    if target.startswith("windows"):
        output = output / f"miccamwatch-hybrid-{target}.zip"
        with zipfile.ZipFile(output, "x", compression=zipfile.ZIP_DEFLATED) as stream:
            for name, file in sorted(expected.items()):
                stream.write(file, name)
        verify_zip(output, root, target)
    else:
        output = output / f"miccamwatch-hybrid-{target}.tar.gz"
        with output.open("xb") as file, tarfile.open(fileobj=file, mode="w:gz") as stream:
            for name, path in sorted(expected.items()):
                info = stream.gettarinfo(str(path), arcname=name)
                info.uid = info.gid = 0
                info.uname = info.gname = "root"
                with path.open("rb") as contents:
                    stream.addfile(info, contents)
        verify_tar(output, root, target)
    return output


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=FLUTTER, required=True)
    parser.add_argument("--native", type=Path, required=True)
    parser.add_argument("--flutter", type=Path, required=True)
    parser.add_argument("--revision", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        args.output.mkdir(parents=True)
        bundle = args.output / "bundle"
        manifest = assemble(args.target, args.native, args.flutter, bundle, args.revision)
        archive = archive_bundle(bundle, args.target, args.output)
        (args.output / "SHA256SUMS").write_text(f"{sha(archive)}  {archive.name}\n", encoding="ascii")
        print(f"{archive} ({manifest['version']}, IPC {manifest['ipc']})")
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        parser.exit(1, f"{error}\n")


if __name__ == "__main__":
    main()
