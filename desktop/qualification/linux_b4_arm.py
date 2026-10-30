#!/usr/bin/env python3
"""Parent-run native ARM64 Flutter build/package/audit; no install or runtime launch.

Run only on disposable non-root ubuntu-24.04-arm GitHub-hosted builders:
  python3 -B desktop/qualification/linux_b4_arm.py --sdk OFFICIAL_CHECKOUT --out NEW

An existing official Flutter Git checkout at REVISION must already be bootstrapped
by parent-run linux_b4_arm_bootstrap.py on ARM64. Its receipt is checked before
build. There is NO official 3.47.8 Linux ARM64 SDK bundle or
published bundle SHA256 in releases_linux.json. Do not use the x64 SDK archive,
a third-party prebuilt SDK, emulation, or a Rust-only build as ARM qualification.
This helper pins official source/engine versions and records local cache SHA256;
those measured hashes are NOT an upstream-published ARM SDK checksum/attestation.
No claim about GPU acceleration, physical ARM devices or Mint21 symbol floors.
"""
import argparse
from datetime import datetime, timezone
import importlib.util
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import tarfile

sys.dont_write_bytecode = True
REVISION = '737ba8f885dc774b53b442759999a312c9461e4e'
ENGINE = 'cf636b1dfb818dd6a3ff16e390e93b1a036969c8'
OFFICIAL_GIT = 'https://github.com/flutter/flutter.git'


def run(argv, cwd=None, env=None):
    return subprocess.run([str(arg) for arg in argv], cwd=cwd, env=env,
                          text=True, capture_output=True, check=True, timeout=3600).stdout


def build(argv, log, cwd, env):
    # Persist both streams even on a failed compile; do not turn compiler errors
    # into an empty receipt or lose stderr (Cargo writes build output there).
    with log.open('x', encoding='utf-8') as stream:
        subprocess.run([str(arg) for arg in argv], cwd=cwd, env=env,
                       stdout=stream, stderr=subprocess.STDOUT,
                       check=True, timeout=3600)


def save(path, value):
    path.write_text(json.dumps(value, indent=2) + '\n', encoding='utf-8')


def audit(prefix, b3, dependencies=False):
    rows = []
    for name in b3.payload(prefix):
        path = prefix / name
        with path.open('rb') as stream:
            if stream.read(4) != b'\x7fELF':
                continue
        header = run(['readelf', '-hW', path])
        if 'ELF64' not in header or not re.search(r'Machine:\s+AArch64', header):
            raise RuntimeError(f'Non-native ARM64 ELF: {name}')
        program = run(['readelf', '-lW', path])
        interpreters = re.findall(r'Requesting program interpreter:\s*([^\]]+)', program)
        if any(value != '/lib/ld-linux-aarch64.so.1' for value in interpreters):
            raise RuntimeError(f'Unexpected ARM interpreter in {name}: {interpreters}')
        dynamic = run(['readelf', '-dW', path])
        needed = re.findall(r'\(NEEDED\).*\[(.*?)\]', dynamic)
        rpaths = re.findall(r'\((?:RUNPATH|RPATH)\).*\[(.*?)\]', dynamic)
        if any(value and value != '$ORIGIN' and not value.startswith('$ORIGIN/')
               for group in rpaths for value in group.split(':')):
            raise RuntimeError(f'Non-relocatable RPATH: {name}')
        versions = run(['readelf', '-VW', path])
        needs = versions.split('Version needs section', 1)[-1] if 'Version needs section' in versions else ''
        requirements = {}
        for family, version in re.findall(r'Name: (GLIBCXX|GLIBC|CXXABI)_([0-9.]+)', needs):
            numeric = tuple(int(part) for part in version.split('.'))
            requirements[family] = max(requirements.get(family, ()), numeric)
        row = {'path': name, 'header': header, 'interpreter': interpreters,
               'needed': needed, 'rpaths': rpaths,
               'required_versions': {key: '.'.join(map(str, value)) for key, value in requirements.items()}}
        if dependencies:
            row['ldd'] = run(['ldd', path])
            if 'not found' in row['ldd']:
                raise RuntimeError(f'Missing native builder runtime dependency: {name}')
        rows.append(row)
    paths = {row['path'] for row in rows}
    required = {'mcw', 'desktop/miccamwatch_desktop', 'desktop/lib/libflutter_linux_gtk.so', 'desktop/lib/libapp.so'}
    if not required <= paths:
        raise RuntimeError(f'Incomplete actual native Flutter/engine bundle: {required - paths}')
    return {'elf': rows, 'architecture': 'aarch64', 'baseline': 'Ubuntu24.04 ARM64 host only',
            'compatibility': 'No Ubuntu22.04/Mint21 floor claim; measured imported symbol versions are recorded',
            'runtime_limitations': ['No app was launched by this helper', 'No GPU, physical ARM/SBC or capture-control qualification',
                                    'Runtime needs GTK3, GNU libstdc++, session D-Bus, display/graphics, PipeWire >=0.3.45; tray additionally needs an actual SNI host']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--sdk', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--bootstrap-receipt', type=Path,
                        help='Defaults to SDK parent/bootstrap-receipt.json')
    args = parser.parse_args()
    os.umask(0o077)
    if not (platform.system() == 'Linux' and platform.machine() == 'aarch64' and os.getuid() != 0
            and os.environ.get('GITHUB_ACTIONS') == 'true'
            and os.environ.get('RUNNER_ENVIRONMENT') == 'github-hosted'
            and os.environ.get('RUNNER_OS') == 'Linux' and os.environ.get('RUNNER_ARCH') == 'ARM64'
            and os.environ.get('ImageVersion')):
        raise RuntimeError('Disposable non-root native GitHub-hosted Linux ARM64 builder required')
    release = dict(line.split('=', 1) for line in Path('/etc/os-release').read_text().splitlines() if '=' in line)
    if release.get('ID', '').strip('"') != 'ubuntu' or release.get('VERSION_ID', '').strip('"') != '24.04':
        raise RuntimeError('Pinned ubuntu-24.04-arm baseline required')
    for key in ('FLUTTER_STORAGE_BASE_URL', 'PUB_HOSTED_URL', 'FLUTTER_ENGINE',
                'LOCAL_ENGINE', 'CARGO_BUILD_TARGET', 'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER'):
        if os.environ.get(key):
            raise RuntimeError(f'Override forbidden for native official-source qualification: {key}')
    if run(['dpkg', '--print-architecture']).strip() != 'arm64' or run(['cc', '-dumpmachine']).strip() != 'aarch64-linux-gnu':
        raise RuntimeError('Native Debian architecture/compiler mismatch')
    rust = run(['rustc', '-Vv'])
    if not re.search(r'^release: 1\.98\.1$', rust, re.M) or not re.search(r'^host: aarch64-unknown-linux-gnu$', rust, re.M):
        raise RuntimeError('Exact native Rust1.98.1 required')
    sdk = args.sdk.resolve(strict=True)
    if run(['git', '-C', sdk, 'remote', 'get-url', 'origin']).strip() != OFFICIAL_GIT:
        raise RuntimeError('Official Flutter repository origin required')
    if run(['git', '-C', sdk, 'rev-parse', 'HEAD']).strip() != REVISION:
        raise RuntimeError('Wrong official Flutter source revision')
    if run(['git', '-C', sdk, 'status', '--porcelain', '--untracked-files=no']).strip():
        raise RuntimeError('Modified Flutter tracked sources refused')
    if (sdk / 'bin/internal/engine.version').read_text().strip() != ENGINE:
        raise RuntimeError('Wrong pinned engine version')
    dart = sdk / 'bin/cache/dart-sdk/bin/dart'
    if not dart.is_file():
        raise RuntimeError('Parent must first bootstrap the official checkout natively; no SDK installation performed here')
    if 'AArch64' not in run(['readelf', '-hW', dart]):
        raise RuntimeError('Native ARM64 Dart SDK required, not x64/emulation')
    if (sdk / 'bin/cache/engine-dart-sdk.stamp').read_text().strip() != ENGINE:
        raise RuntimeError('Dart cache does not match the pinned official engine')
    spec = importlib.util.spec_from_file_location('mcw_b3_linux', Path(__file__).with_name('linux.py'))
    b3 = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(b3)
    bootstrap_path = args.bootstrap_receipt or sdk.parent / 'bootstrap-receipt.json'
    bootstrap = json.loads(bootstrap_path.read_text())
    if not (bootstrap.get('complete') is True and bootstrap.get('official_source') == OFFICIAL_GIT
            and bootstrap.get('verified_git_sha') == REVISION and bootstrap.get('engine') == ENGINE
            and bootstrap.get('machine') == 'aarch64'):
        raise RuntimeError('Successful exact official-source native bootstrap receipt required')
    dart_key = 'bin/cache/dart-sdk/bin/dart'
    if b3.sha(dart) != bootstrap.get('cache_sha256', {}).get(dart_key):
        raise RuntimeError('Native Dart differs from the official bootstrap receipt')
    native_engine = bootstrap.get('native_engine_elf', [])
    if not native_engine:
        raise RuntimeError('Bootstrap receipt lacks actual native engine ELF evidence')
    for row in native_engine:
        relative = Path(row['path'])
        if relative.is_absolute() or '..' in relative.parts or not relative.as_posix().startswith('bin/cache/artifacts/engine/'):
            raise RuntimeError('Invalid engine path in private bootstrap receipt')
        if b3.sha(sdk / relative) != row['sha256']:
            raise RuntimeError(f'Engine cache differs from official bootstrap: {relative}')
    out = b3.private_new(args.out)
    repo = Path(__file__).resolve().parents[2]
    env = {**os.environ, 'LC_ALL': 'C', 'CARGO_TARGET_DIR': str(out / 'cargo-target'),
           'FLUTTER_ROOT': str(sdk), 'PATH': str(sdk / 'bin') + os.pathsep + os.environ['PATH']}
    flutter = sdk / 'bin/flutter'
    version = json.loads(run([flutter, '--version', '--machine'], env=env))
    if not (version.get('frameworkVersion') == '3.47.8' and version.get('frameworkRevision') == REVISION
            and version.get('dartSdkVersion', '').split()[0] == '3.13.7'):
        raise RuntimeError('Flutter/Dart exact pin mismatch')
    revision = run(['git', 'rev-parse', 'HEAD'], cwd=repo).strip()
    status = run(['git', 'status', '--porcelain'], cwd=repo)
    save(out / 'source-receipt.json', {'source_revision': revision, 'source_status': status})
    build(['cargo', 'clippy', '--all-targets', '--locked', '--features', 'flutter-desktop', '--', '-D', 'warnings'], out / 'cargo-clippy.log', repo, env)
    for name in ('frontends::desktop', 'history::', 'autostart::tests', 'settings::tests'):
        build(['cargo', 'test', '--lib', '--locked', '--features', 'flutter-desktop', name],
              out / ('rust-tests-' + name.replace('::', '-') + '.log'), repo, env)
    build(['cargo', 'build', '--locked', '--release', '--features', 'flutter-desktop', '--bin', 'mcw'], out / 'cargo-build.log', repo, env)
    desktop = repo / 'desktop/flutter'
    build([flutter, 'pub', 'get', '--enforce-lockfile'], out / 'flutter-pub.log', desktop, env)
    build([flutter, 'analyze', '--no-pub'], out / 'flutter-analyze.log', desktop, env)
    build([flutter, 'test', '--no-pub'], out / 'flutter-tests.log', desktop, env)
    build([flutter, 'build', 'linux', '--release', '--no-pub', '--target-platform=linux-arm64'], out / 'flutter-build.log', desktop, env)
    prefix = b3.private_new(out / 'bundle')
    shutil.copy2(out / 'cargo-target/release/mcw', prefix / 'mcw')
    shutil.copytree(desktop / 'build/linux/arm64/release/bundle', prefix / 'desktop', symlinks=True)
    # payload() rejects all payload symlinks and computes complete SHA256 inventory.
    b3.payload(prefix)
    report = audit(prefix, b3, dependencies=True)
    sdk_cache = {str(path.relative_to(sdk)): b3.sha(path)
                 for path in sorted((sdk / 'bin/cache').rglob('*'))
                 if path.is_file() and not path.is_symlink()}
    receipt = {'observed_at': datetime.now(timezone.utc).isoformat(), 'builder': 'github-hosted-ubuntu-24.04-arm',
               'runner_image_version': os.environ['ImageVersion'], 'machine': platform.machine(),
               'source_revision': revision, 'source_status': status, 'flutter': version, 'rust': rust,
               'sdk_origin': OFFICIAL_GIT, 'sdk_revision': REVISION, 'engine_revision': ENGINE,
               'sdk_bootstrap_receipt': bootstrap,
               'sdk_provenance_limit': 'Official source checkout/native bootstrap; no upstream ARM64 bundle SHA256 exists. Cache hashes below are locally measured, not upstream attestations.',
               'sdk_cache_sha256': sdk_cache, 'gtk_build': run(['pkg-config', '--modversion', 'gtk+-3.0']).strip(),
               'pipewire_build': run(['pkg-config', '--modversion', 'libpipewire-0.3']).strip(),
               'packages': run(['dpkg-query', '-W'])}
    save(out / 'build-receipt.json', receipt)
    save(prefix / 'elf-audit.json', report)
    save(prefix / 'qualification-manifest.json', {'schema_version': 1, 'feature': 'flutter-desktop',
         'mode': 'settings_development', 'architecture': 'aarch64', 'source_revision': revision,
         'compatibility': 'Native Ubuntu24.04 ARM64 build only; NOT runtime/hardware qualified',
         'build_receipt': receipt, 'audit': report, 'files': b3.payload(prefix)})
    hashes = b3.payload(prefix)
    hashes['qualification-manifest.json'] = b3.sha(prefix / 'qualification-manifest.json')
    (prefix / 'SHA256SUMS').write_text(''.join(f'{digest}  {name}\n' for name, digest in sorted(hashes.items())))
    archive = out / 'miccamwatch-b4-linux-arm64-qualification.tar.gz'
    with tarfile.open(archive, 'w:gz') as stream:
        stream.add(prefix, arcname='bundle')
    (out / 'archive.SHA256SUMS').write_text(f'{b3.sha(archive)}  {archive.name}\n')
    print(json.dumps({'bundle': str(prefix), 'archive': str(archive), 'runtime_executed': False}, indent=2))


if __name__ == '__main__':
    main()
