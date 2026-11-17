#!/usr/bin/env python3
"""Package an already-built native mcw into an ad-hoc sealed macOS 15+ app/DMG.

Requires macOS, Xcode command line tools (clang, swiftc, iconutil, codesign,
hdiutil), Python 3 and Cargo metadata. Does not build Rust, install the app,
change permissions/TCC/quarantine or claim Developer ID/notarization. Existing
CLI tar.gz packaging remains independent and unchanged.
"""
import argparse
import json
from pathlib import Path
import platform
import re
import plistlib
import shutil
import subprocess
import tempfile
import sys
import tarfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from hybrid_payload import ASSETS, check, release_metadata, sha


def run(*args, **kwargs):
    return subprocess.run([str(arg) for arg in args], check=True, **kwargs)


FLUTTER_LINKS = {
    f"Frameworks/{name}.framework/{link}": target
    for name in ("App", "FlutterMacOS")
    for link, target in (
        (name, "Versions/Current/" + name),
        ("Resources", "Versions/Current/Resources"),
        ("Versions/Current", "A"),
    )
}
FLUTTER_FILES = {
    "Info.plist", "PkgInfo", "MacOS/MicCamWatch", "_CodeSignature/CodeResources",
    "Resources/AppIcon.icns", "Resources/Assets.car", "Resources/Base.lproj/MainMenu.nib",
    "Frameworks/App.framework/Versions/A/App",
    "Frameworks/App.framework/Versions/A/Resources/Info.plist",
    "Frameworks/App.framework/Versions/A/_CodeSignature/CodeResources",
    "Frameworks/FlutterMacOS.framework/Versions/A/FlutterMacOS",
    "Frameworks/FlutterMacOS.framework/Versions/A/Resources/icudtl.dat",
    "Frameworks/FlutterMacOS.framework/Versions/A/Resources/Info.plist",
    "Frameworks/FlutterMacOS.framework/Versions/A/Resources/PrivacyInfo.xcprivacy",
    "Frameworks/FlutterMacOS.framework/Versions/A/_CodeSignature/CodeResources",
} | {"Frameworks/App.framework/Versions/A/Resources/flutter_assets/" + asset for asset in ASSETS}


def verify_flutter(app, arch, version):
    contents = (app / "Contents").resolve(strict=True)
    check(app.is_dir() and set(p.name for p in app.iterdir()) == {"Contents"}, "invalid Flutter .app root")
    regular = set()
    links = {}
    directories = set()
    for path in contents.rglob("*"):
        relative = path.relative_to(contents).as_posix()
        if path.is_symlink():
            links[relative] = str(path.readlink())
            check(path.resolve(strict=True).is_relative_to(contents), f"escaping framework link: {relative}")
        elif path.is_file():
            check(path.stat().st_nlink == 1, f"hardlinked framework file: {relative}")
            regular.add(relative)
        elif path.is_dir():
            directories.add(relative)
        else:
            raise ValueError(f"unknown macOS payload kind: {relative}")
    expected_directories = {parent.as_posix() for name in FLUTTER_FILES | FLUTTER_LINKS.keys()
                            for parent in Path(name).parents} - {"."}
    check((regular, links, directories) == (FLUTTER_FILES, FLUTTER_LINKS, expected_directories),
          f"unexpected Mac Flutter payload: missing {sorted(FLUTTER_FILES - regular)}, "
          f"extra {sorted(regular - FLUTTER_FILES)}, links {links}, directories {sorted(directories ^ expected_directories)}")
    plist = plistlib.loads((contents / "Info.plist").read_bytes())
    check(plist["CFBundleIdentifier"] == "dev.miccamwatch.desktop"
          and plist["CFBundleExecutable"] == "MicCamWatch"
          and plist["CFBundleShortVersionString"] == version
          and plist["LSMinimumSystemVersion"] == "15.0", "unexpected Flutter runner identity")
    for name in ("MacOS/MicCamWatch", "Frameworks/App.framework/Versions/A/App",
                 "Frameworks/FlutterMacOS.framework/Versions/A/FlutterMacOS"):
        architectures = subprocess.check_output(["lipo", "-archs", str(contents / name)], text=True).split()
        check(arch in architectures, f"incompatible Mach-O: {name}")
        build = subprocess.check_output(["xcrun", "vtool", "-show-build", str(contents / name)], text=True)
        minimums = re.findall(r"\bminos\s+(\d+(?:\.\d+)+)", build)
        if name == "MacOS/MicCamWatch":
            check(minimums and all(value in {"15.0", "15.0.0"} for value in minimums),
                  "unexpected Flutter deployment target")
        else:
            check(all(tuple(map(int, value.split("."))) <= (15, 0, 0) for value in minimums),
                  f"framework deployment version is too recent: {name}")
    entitlements = run("codesign", "-d", "--entitlements", ":-", app, capture_output=True)
    check(plistlib.loads(entitlements.stdout) == {"com.apple.security.app-sandbox": True},
          "unexpected Flutter sandbox entitlements")
    run("codesign", "--verify", "--deep", "--strict", app)
    return {name: sha(contents / name) for name in sorted(regular)}


def verify_dmg(dmg, temporary, expected, arch, version):
    mount = temporary / "mount"
    mount.mkdir()
    run("hdiutil", "attach", dmg, "-readonly", "-nobrowse", "-mountpoint", mount)
    try:
        app = mount / "MicCamWatch.app"
        run("codesign", "--verify", "--deep", "--strict", app)
        check(verify_flutter(app / "Contents/Helpers/MicCamWatch.app", arch, version) == expected,
              "attached disk image differs from signed Flutter bytes")
    finally:
        run("hdiutil", "detach", mount)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--arch', choices=('aarch64', 'x86_64'), required=True)
    parser.add_argument('--output', type=Path, default=Path('.'))
    parser.add_argument("--flutter", type=Path, help="Uninstrumented, sealed Flutter Release .app")
    parser.add_argument("--revision", help="Exact Git revision for hybrid packaging")
    args = parser.parse_args()
    if bool(args.flutter) != bool(args.revision):
        parser.error("--flutter and --revision must be specified together")
    if platform.system() != 'Darwin':
        parser.error('real macOS packaging requires a macOS host')
    repo = Path(__file__).resolve().parents[2]
    metadata = json.loads(subprocess.check_output(
        ['cargo', 'metadata', '--no-deps', '--format-version', '1'], cwd=repo))
    package = next(package for package in metadata['packages'] if package['name'] == 'miccamwatch')
    version = package['version']
    binary = args.binary.resolve(strict=True)
    reported = subprocess.check_output([str(binary), '--version'], text=True).strip()
    if reported != 'mcw ' + version:
        parser.error(f'Rust binary version {reported!r} does not match Cargo {version}')
    arch = 'arm64' if args.arch == 'aarch64' else 'x86_64'
    architectures = subprocess.check_output(['lipo', '-archs', str(binary)], text=True).split()
    if architectures != [arch]:
        parser.error(f'expected thin {arch} Mach-O; got {architectures}')
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    base = f"miccamwatch-{'hybrid-' if args.flutter else ''}macos-{args.arch}"
    destination = output / (base + ".dmg")
    if destination.exists() or (args.flutter and (output / (base + '.tar.gz')).exists()):
        parser.error(f'refusing to overwrite existing artifact: {destination}')
    with tempfile.TemporaryDirectory(prefix='mcw-app-package-') as directory:
        work = Path(directory)
        volume = work / 'volume'
        app = volume / 'MicCamWatch.app'
        contents = app / 'Contents'
        executables = contents / 'MacOS'
        resources = contents / 'Resources'
        executables.mkdir(parents=True)
        resources.mkdir()
        shutil.copy2(binary, executables / 'mcw')
        run('xcrun', 'clang', '-arch', arch, '-mmacosx-version-min=15.0', '-O2',
            '-Wall', '-Wextra', '-Werror', repo / 'installer/macos/launcher.c',
            '-o', executables / 'MicCamWatch')
        shutil.copyfile(repo / 'installer/macos/icon.swift', work / 'main.swift')
        run('xcrun', 'swiftc', '-target', f'{arch}-apple-macosx15.0', '-O',
            repo / 'native/macos_brand.swift', work / 'main.swift', '-framework', 'AppKit',
            '-o', work / 'mcw-icon-generator')
        run(work / 'mcw-icon-generator', work / 'MicCamWatch.iconset')
        run('iconutil', '-c', 'icns', work / 'MicCamWatch.iconset',
            '-o', resources / 'MicCamWatch.icns')
        for name in ('README.md', 'LICENSE'):
            shutil.copyfile(repo / name, resources / name)
        plist = {
            'CFBundleIdentifier': 'com.roman-cuisset.miccamwatch',
            'CFBundleExecutable': 'MicCamWatch',
            'CFBundleName': 'MicCamWatch',
            'CFBundleDisplayName': 'MicCamWatch',
            'CFBundlePackageType': 'APPL',
            'CFBundleVersion': version,
            'CFBundleShortVersionString': version,
            'CFBundleIconFile': 'MicCamWatch.icns',
            'LSMinimumSystemVersion': '15.0',
            'LSUIElement': True,
            'NSHighResolutionCapable': True,
            'NSPrincipalClass': 'NSApplication',
            'LSApplicationCategoryType': 'public.app-category.utilities',
            'NSCameraUsageDescription': 'MicCamWatch passively inventories cameras; it does not request capture access or record video.',
            'NSMicrophoneUsageDescription': 'MicCamWatch passively observes input clients; it does not request capture access or record audio.',
        }
        with (contents / 'Info.plist').open('wb') as stream:
            plistlib.dump(plist, stream)
        if args.flutter:
            check(re.fullmatch(r"[a-f0-9]{40}", args.revision), "full Git SHA required")
            actual_version, ipc = release_metadata()
            check(actual_version == version, "packaging version mismatch")
            source = args.flutter.resolve(strict=True)
            expected = verify_flutter(source, arch, version)
            nested = contents / "Helpers/MicCamWatch.app"
            nested.parent.mkdir()
            run("ditto", source, nested)
            check(verify_flutter(nested, arch, version) == expected, "nested Flutter app differs")
            (resources / "miccamwatch-bundle.json").write_text(json.dumps({
                "schema": 1, "target": f"macos-{args.arch}", "revision": args.revision,
                "version": version, "ipc": ipc,
                "frontend_contents": expected, "framework_links": FLUTTER_LINKS,
            }, sort_keys=True, indent=2) + "\n", encoding="utf-8")
        # Seal nested executable first, then app. This is NOT Developer ID signing.
        run('codesign', '--force', '--sign', '-', '--timestamp=none',
            '--identifier', 'com.roman-cuisset.miccamwatch.cli', executables / 'mcw')
        run('codesign', '--force', '--sign', '-', '--timestamp=none',
            '--identifier', plist['CFBundleIdentifier'], app)
        run('codesign', '--verify', '--deep', '--strict', app)
        (volume / 'Applications').symlink_to('/Applications', target_is_directory=True)
        run('hdiutil', 'create', '-volname', 'MicCamWatch', '-srcfolder', volume,
            '-format', 'UDZO', '-ov', work / destination.name)
        run('hdiutil', 'verify', work / destination.name)
        if args.flutter:
            verify_dmg(work / destination.name, work, expected, arch, version)
            archive = output / (base + ".tar.gz")
            with tarfile.open(archive, "x:gz", dereference=False) as stream:
                stream.add(app, arcname=app.name)
            tar_destination = work / "tar-image"
            tar_destination.mkdir()
            run("tar", "-xzf", archive, "-C", tar_destination)
            extracted = tar_destination / app.name
            run("codesign", "--verify", "--deep", "--strict", extracted)
            check(verify_flutter(extracted / "Contents/Helpers/MicCamWatch.app", arch, version) == expected,
                  "tar archive differs from the signed Flutter bytes")
            shutil.copy2(resources / "miccamwatch-bundle.json", output)
        shutil.copyfile(work / destination.name, destination)
        if args.flutter:
            (output / "SHA256SUMS").write_text("".join(
                f"{sha(file)}  {file.name}\n" for file in (destination, archive)), encoding="ascii")
    print(destination)
    print('Ad-hoc sealed only; no Developer ID, notarization or Gatekeeper clearance asserted.')


if __name__ == '__main__':
    main()
