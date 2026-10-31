#!/usr/bin/env python3
"""Parent-run official-source Flutter3.47.8 ARM64 bootstrap, never executed here.

On disposable github-hosted ubuntu-24.04-arm only, non-root:
  python3 -B desktop/qualification/linux_b4_arm_bootstrap.py --out NEW_PRIVATE
  python3 -B desktop/qualification/linux_b4_arm.py --sdk NEW_PRIVATE/flutter --out NEW_BUILD

No third-party SDK, x64 emulation, apt or product/app/test launch. Official Git
source/engine pins are checked BEFORE executing Flutter. Dart's official engine
archive is retained with upstream GCS MD5 verified and measured SHA256 recorded;
there is no invented upstream ARM64 SDK bundle/SHA256. Any upstream failure stops
this chain with the precise URL/status or native bootstrap log retained.
"""
import argparse
import base64
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import shutil
import stat
import subprocess
import sys
import urllib.error
import urllib.request
import zipfile

sys.dont_write_bytecode = True
REVISION = '737ba8f885dc774b53b442759999a312c9461e4e'
ENGINE = 'cf636b1dfb818dd6a3ff16e390e93b1a036969c8'
ORIGIN = 'https://github.com/flutter/flutter.git'
BASE = f'https://storage.googleapis.com/flutter_infra_release/flutter/{ENGINE}/'
# Official GCS x-goog-hash values actually observed by HEAD in B4. MD5 is an
# upstream transport-integrity digest, NOT a signature or a published SHA256.
ARTIFACTS = {
    'dart-sdk-linux-arm64.zip': '6rJcKM8dUtl6IcqLbUPmAA==',
    'linux-arm64/artifacts.zip': 'EuHDmvVs/NBt5em8Jd9kPw==',
    'linux-arm64-debug/linux-arm64-flutter-gtk.zip': 'LF5zGHT7H8bNI8UcbeyRCA==',
    'linux-arm64-profile/linux-arm64-flutter-gtk.zip': 'BVx2Y2gyEQtlnF4guAMiPw==',
    'linux-arm64-release/linux-arm64-flutter-gtk.zip': 'SiYuxq9gfd71QZZTCQknGw=='
}


def save(path, value):
    path.write_text(json.dumps(value, indent=2) + '\n', encoding='utf-8')


def run(argv, log, cwd=None, env=None):
    with log.open('x', encoding='utf-8') as stream:
        subprocess.run([str(arg) for arg in argv], cwd=cwd, env=env,
                       stdout=stream, stderr=subprocess.STDOUT, check=True, timeout=3600)


def capture(argv, cwd=None, env=None):
    return subprocess.run([str(arg) for arg in argv], cwd=cwd, env=env,
                          capture_output=True, text=True, check=True, timeout=60).stdout


def official_response(response):
    if not response.url.startswith(BASE):
        raise RuntimeError(f'Official engine storage redirect refused: {response.url}')
    return {'final_url': response.url, 'status': response.status,
            'headers': {key: value for key, value in response.headers.items()
                        if key.lower() in ('content-length', 'content-type', 'x-goog-hash',
                                           'x-goog-generation', 'etag', 'last-modified')}}


def check_hash_header(response, expected):
    values = response.headers.get_all('x-goog-hash') or []
    if expected not in [part.strip()[4:] for value in values for part in value.split(',')
                        if part.strip().startswith('md5=')]:
        raise RuntimeError('Official GCS MD5 differs from observed pinned engine artifact')


def validate_dart_archive(archive):
    """Only the SDK subtree is extracted; archive-level metadata stays in ZIP."""
    ignored = []
    has_dart = False
    with zipfile.ZipFile(archive) as source:
        for info in source.infolist():
            name = info.orig_filename
            path = PurePosixPath(name)
            if (path.is_absolute() or '..' in path.parts or not path.parts
                    or '\0' in name or '\\' in name or stat.S_ISLNK(info.external_attr >> 16)):
                raise RuntimeError(f'Unsafe Dart archive member: {name}')
            if path.parts[0] != 'dart-sdk':
                ignored.append(name)
            elif name == 'dart-sdk/bin/dart' and not info.is_dir():
                has_dart = True
    if not has_dart:
        raise RuntimeError('Official archive lacks dart-sdk/bin/dart')
    return ignored


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    os.umask(0o077)
    if not (platform.system() == 'Linux' and platform.machine() == 'aarch64' and os.getuid() != 0
            and os.environ.get('GITHUB_ACTIONS') == 'true'
            and os.environ.get('RUNNER_ENVIRONMENT') == 'github-hosted'
            and os.environ.get('RUNNER_OS') == 'Linux' and os.environ.get('RUNNER_ARCH') == 'ARM64'
            and os.environ.get('ImageVersion')):
        raise RuntimeError('Disposable native GitHub-hosted ARM64 runner, non-root, required')
    release = dict(line.split('=', 1) for line in Path('/etc/os-release').read_text().splitlines() if '=' in line)
    if release.get('ID', '').strip('"') != 'ubuntu' or release.get('VERSION_ID', '').strip('"') != '24.04':
        raise RuntimeError('ubuntu-24.04-arm required')
    for key in ('FLUTTER_STORAGE_BASE_URL', 'PUB_HOSTED_URL', 'FLUTTER_ENGINE',
                'LOCAL_ENGINE', 'FLUTTER_ROOT', 'DART_SDK', 'FLUTTER_PREBUILT_ENGINE_VERSION'):
        if os.environ.get(key):
            raise RuntimeError(f'Custom SDK/engine override refused: {key}')
    spec = importlib.util.spec_from_file_location('mcw_b3_linux', Path(__file__).with_name('linux.py'))
    b3 = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(b3)
    out = b3.private_new(args.out)
    sdk = b3.private_new(out / 'flutter')
    env = {**os.environ, 'LC_ALL': 'C', 'CI': 'true', 'FLUTTER_SUPPRESS_ANALYTICS': 'true'}
    receipt = {'observed_at': datetime.now(timezone.utc).isoformat(), 'complete': False,
               'official_source': ORIGIN, 'revision': REVISION, 'engine': ENGINE,
               'runner_image_version': os.environ['ImageVersion'], 'machine': platform.machine(),
               'artifacts': [], 'scope': 'Official source bootstrap only; not product build/runtime/hardware qualification'}
    try:
        run(['git', 'init', sdk], out / 'git-init.log', env=env)
        run(['git', '-C', sdk, 'remote', 'add', 'origin', ORIGIN], out / 'git-origin.log', env=env)
        run(['git', '-C', sdk, 'fetch', '--depth=1', 'origin',
             'refs/tags/3.47.8:refs/tags/3.47.8'], out / 'git-fetch.log', env=env)
        source_sha = capture(['git', '-C', sdk, 'rev-parse', 'refs/tags/3.47.8^{commit}'], env=env).strip()
        if source_sha != REVISION:
            raise RuntimeError(f'Official3.47.8 tag commit mismatch: {source_sha}')
        run(['git', '-C', sdk, 'checkout', '--detach', REVISION], out / 'git-checkout.log', env=env)
        if capture(['git', '-C', sdk, 'remote', 'get-url', 'origin'], env=env).strip() != ORIGIN:
            raise RuntimeError('Official origin mismatch')
        if capture(['git', '-C', sdk, 'rev-parse', 'HEAD'], env=env).strip() != REVISION:
            raise RuntimeError('Source HEAD mismatch')
        if capture(['git', '-C', sdk, 'status', '--porcelain', '--untracked-files=no'], env=env).strip():
            raise RuntimeError('Modified official tracked source refused')
        if (sdk / 'bin/internal/engine.version').read_text().strip() != ENGINE:
            raise RuntimeError('Pinned engine source mismatch')
        receipt['verified_git_sha'] = source_sha
        save(out / 'bootstrap-receipt.json', receipt)
        # Fail before executing source if any required native upstream archive is
        # absent/drifted. Persist partial receipts plus exact HTTP failure below.
        for relative, digest in ARTIFACTS.items():
            url = BASE + relative
            with urllib.request.urlopen(urllib.request.Request(url, method='HEAD'), timeout=60) as response:
                record = {'url': url, **official_response(response), 'pinned_gcs_md5_base64': digest}
                receipt['artifacts'].append(record)
                save(out / 'bootstrap-receipt.json', receipt)
                check_hash_header(response, digest)
        downloads = b3.private_new(out / 'downloads')
        archive = downloads / 'dart-sdk-linux-arm64.zip'
        sha = hashlib.sha256()
        md5 = hashlib.md5(usedforsecurity=False)
        url = BASE + 'dart-sdk-linux-arm64.zip'
        with urllib.request.urlopen(url, timeout=180) as response, archive.open('xb') as destination:
            record = official_response(response)
            check_hash_header(response, ARTIFACTS['dart-sdk-linux-arm64.zip'])
            for block in iter(lambda: response.read(1024 * 1024), b''):
                destination.write(block)
                sha.update(block)
                md5.update(block)
        if base64.b64encode(md5.digest()).decode('ascii') != ARTIFACTS['dart-sdk-linux-arm64.zip']:
            raise RuntimeError('Downloaded official Dart archive MD5 mismatch')
        receipt['dart_archive'] = {'url': url, **record, 'measured_sha256': sha.hexdigest(),
                                   'verified_gcs_md5_base64': base64.b64encode(md5.digest()).decode('ascii'),
                                   'digest_limit': 'SHA256 measured here; MD5 supplied by official GCS, not a signature'}
        save(out / 'bootstrap-receipt.json', receipt)
        # Preserve executable modes, but extract only the verified SDK subtree.
        # Official archive-level metadata (including its license) remains in the
        # original retained ZIP, never unpacked into the Flutter cache.
        receipt['unextracted_archive_metadata'] = validate_dart_archive(archive)
        cache = sdk / 'bin/cache'
        cache.mkdir(mode=0o700, exist_ok=True)
        run(['unzip', '-q', archive, 'dart-sdk/*', '-d', cache], out / 'dart-extract.log', env=env)
        dart = cache / 'dart-sdk/bin/dart'
        header = capture(['readelf', '-hW', dart], env=env)
        if 'ELF64' not in header or not re.search(r'Machine:\s+AArch64', header) or not os.access(dart, os.X_OK):
            raise RuntimeError('Downloaded Dart is not executable native ARM64 ELF64')
        (cache / 'engine-dart-sdk.stamp').write_text(ENGINE + '\n')
        receipt['dart_elf'] = header
        # Source pin/origin/native-Dart checks above precede any Flutter execution.
        flutter = sdk / 'bin/flutter'
        with (out / 'flutter-version.json').open('x', encoding='utf-8') as stdout, (out / 'flutter-bootstrap.log').open('x', encoding='utf-8') as stderr:
            subprocess.run([str(flutter), '--version', '--machine'], stdout=stdout,
                           stderr=stderr, env=env, check=True, timeout=3600)
        version = json.loads((out / 'flutter-version.json').read_text())
        if not (version.get('frameworkVersion') == '3.47.8' and version.get('frameworkRevision') == REVISION
                and version.get('dartSdkVersion', '').split()[0] == '3.13.7'):
            raise RuntimeError('Actual bootstrapped Flutter/Dart version mismatch')
        receipt['flutter'] = version
        # Pinned upstream cache.dart validates official GCS MD5 for engine ZIPs.
        run([flutter, 'precache', '--linux'], out / 'flutter-precache.log', env=env)
        # Upstream flutter/flutter#156309: this exact engine archive copies the
        # x64 host Dart frontend into linux-arm64. Its own BUILD.gn copies this
        # component from a Dart SDK. Assemble the intended target SDK component,
        # not an emulated compiler or an unchecked third-party replacement.
        frontend = cache / 'artifacts/engine/linux-arm64/frontend_server_aot.dart.snapshot'
        target_frontend = cache / 'dart-sdk/bin/snapshots/frontend_server_aot.dart.snapshot'
        if (frontend.is_symlink() or target_frontend.is_symlink()
                or not frontend.is_file() or not target_frontend.is_file()):
            raise RuntimeError('Expected owned official frontend components are not regular files')
        original_sha = b3.sha(frontend)
        target_sha = b3.sha(target_frontend)
        if original_sha != '0c484223020a7aad5874e1ef3bdcc43c9d0b36a1402f223d538ee2d4cc0b847a':
            raise RuntimeError('Pinned upstream host-frontend component differs; no assembly performed')
        if target_sha != '07988b418436f0dc8e839626a27d8fd03abeb18a566ad27b332dd1ee8d4b3a75':
            raise RuntimeError('Pinned official target Dart frontend differs; no assembly performed')
        target_header = capture(['readelf', '-hW', target_frontend], env=env)
        if 'ELF64' not in target_header or not re.search(r'Machine:\s+AArch64', target_header):
            raise RuntimeError('Official target Dart frontend is not native ARM64')
        shutil.copyfile(target_frontend, frontend)
        if b3.sha(frontend) != target_sha:
            raise RuntimeError('Target frontend assembly readback mismatch')
        receipt['official_target_frontend_assembly'] = {
            'upstream_issue': 'https://github.com/flutter/flutter/issues/156309',
            'copy_rule': 'https://github.com/flutter/engine/blob/main/flutter_frontend_server/BUILD.gn',
            'cache_path': str(frontend.relative_to(sdk)),
            'original_engine_component_sha256': original_sha,
            'source': str(target_frontend.relative_to(sdk)),
            'source_archive': receipt['dart_archive'],
            'native_component_sha256': target_sha, 'native_elf_header': target_header,
            'limit': 'Explicit official-target component assembly; not an unmodified prebuilt ARM SDK, emulation, or upstream attestation'}
        save(out / 'bootstrap-receipt.json', receipt)
        native_elf = []
        for path in sorted((cache / 'artifacts/engine').rglob('*')):
            if not path.is_file() or path.is_symlink():
                continue
            with path.open('rb') as stream:
                if stream.read(4) != b'\x7fELF':
                    continue
            header = capture(['readelf', '-hW', path], env=env)
            if 'ELF64' not in header or not re.search(r'Machine:\s+AArch64', header):
                raise RuntimeError(f'Non-native cached engine ELF: {path.relative_to(sdk)}')
            native_elf.append({'path': str(path.relative_to(sdk)), 'header': header, 'sha256': b3.sha(path)})
        if not any('linux-arm64-release' in row['path'] and row['path'].endswith('libflutter_linux_gtk.so') for row in native_elf):
            raise RuntimeError('Official native ARM64 release GTK engine missing after precache')
        receipt['native_engine_elf'] = native_elf
        receipt['cache_sha256'] = {str(path.relative_to(sdk)): b3.sha(path)
                                   for path in sorted(cache.rglob('*')) if path.is_file() and not path.is_symlink()}
        receipt['complete'] = True
        save(out / 'bootstrap-receipt.json', receipt)
        print(json.dumps({'sdk': str(sdk), 'receipt': str(out / 'bootstrap-receipt.json'),
                          'product_built': False, 'runtime_launched': False}, indent=2))
    except Exception as error:
        receipt['failure'] = {'type': type(error).__name__, 'detail': str(error)}
        if isinstance(error, urllib.error.HTTPError):
            receipt['failure'].update(url=error.url, http_status=error.code)
        save(out / 'bootstrap-receipt.json', receipt)
        raise


if __name__ == '__main__':
    main()
