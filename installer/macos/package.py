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
import plistlib
import shutil
import subprocess
import tempfile


def run(*args, **kwargs):
    return subprocess.run([str(arg) for arg in args], check=True, **kwargs)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--arch', choices=('aarch64', 'x86_64'), required=True)
    parser.add_argument('--output', type=Path, default=Path('.'))
    args = parser.parse_args()
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
    destination = output / f'miccamwatch-macos-{args.arch}.dmg'
    if destination.exists():
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
        shutil.copyfile(work / destination.name, destination)
    print(destination)
    print('Ad-hoc sealed only; no Developer ID, notarization or Gatekeeper clearance asserted.')


if __name__ == '__main__':
    main()
