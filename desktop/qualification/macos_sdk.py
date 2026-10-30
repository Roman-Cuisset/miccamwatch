#!/usr/bin/env python3
"""Install only the B2 study's exact official macOS Flutter SDK in runner temp.

Pins researched from the official manifest on 2026-10-30. Never substitutes
latest, installs Rosetta, or changes host packages/settings. SDK execution is
performed by the workflow after archive and upstream metadata validation.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import urllib.request

VERSION = '3.47.8'
REVISION = '737ba8f885dc774b53b442759999a312c9461e4e'
BASE = 'https://storage.googleapis.com/flutter_infra_release/releases'
PINS = {
    'x86_64': ('x64', 'stable/macos/flutter_macos_3.47.8-stable.zip',
               'b8a3c255a51a8c0c0943a4d5bdf9b26008c34f02febf69640c8aab413afd896f'),
    'arm64': ('arm64', 'stable/macos/flutter_macos_arm64_3.47.8-stable.zip',
              '6114edc2dae3636cfefb7e4456a4510399ee856e324149b34dbacb509e329d05'),
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--proof', type=Path, required=True)
    args = parser.parse_args()
    if platform.system() != 'Darwin' or os.environ.get('RUNNER_ENVIRONMENT') != 'github-hosted':
        raise SystemExit('Only a disposable GitHub-hosted Mac is permitted')
    root = Path(os.environ['RUNNER_TEMP']).resolve()
    proof = args.proof.resolve()
    if not proof.is_relative_to(root):
        raise SystemExit('Proof must be beneath RUNNER_TEMP')
    proof.mkdir(parents=True, exist_ok=True)
    arch, archive, digest = PINS[platform.machine()]
    with urllib.request.urlopen(BASE + '/releases_macos.json', timeout=60) as response:
        metadata = response.read()
    (proof / 'flutter-official-releases.json').write_bytes(metadata)
    releases = json.loads(metadata)
    matches = [row for row in releases['releases'] if row.get('version') == VERSION
               and row.get('channel') == 'stable' and row.get('dart_sdk_arch') == arch]
    if len(matches) != 1:
        raise SystemExit('Exact official SDK release/architecture is unavailable')
    expected = dict(hash=REVISION, archive=archive, sha256=digest, dart_sdk_version='3.13.7')
    if any(matches[0].get(key) != value for key, value in expected.items()):
        raise SystemExit('Pinned SDK differs from official release metadata')
    sdk_root = root / ('mcw-flutter-' + arch)
    sdk_root.mkdir(mode=0o700, exist_ok=False)
    download = sdk_root / 'flutter.zip'
    with urllib.request.urlopen(BASE + '/' + archive, timeout=180) as response, download.open('xb') as target:
        while block := response.read(1024 * 1024):
            target.write(block)
    with download.open('rb') as source:
        actual = hashlib.file_digest(source, 'sha256').hexdigest()
    if actual != digest:
        raise SystemExit('SDK archive SHA-256 mismatch; no SDK execution')
    subprocess.run(['/usr/bin/ditto', '-x', '-k', str(download), str(sdk_root)], check=True)
    (proof / 'sdk-pin.json').write_text(json.dumps(dict(version=VERSION, architecture=arch,
        **expected, url=BASE + '/' + archive), indent=2) + '\n')
    with Path(os.environ['GITHUB_PATH']).open('a') as path:
        path.write(str(sdk_root / 'flutter/bin') + '\n')


if __name__ == '__main__':
    main()
