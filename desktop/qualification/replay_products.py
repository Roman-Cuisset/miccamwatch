"""Hosted-only runtime replay of immutable, individually pinned B5 products.
Product and harness provenance stay separate, including the failed Intel runtime.
No product build, installation, re-signing, permission grant or fallback download.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import posixpath
import shutil
import subprocess
import tarfile
import zipfile

REPOSITORY = 'Roman-Cuisset/miccamwatch'
PINS = {
    ('macos', 'arm64'): dict(run=41422733643,
        source='90d69b4738f76485e40f1002125de1d46c6254c3', artifact=13130913060,
        zip_sha256='e61a484236911212ecf49d5b850b37814f423d790ff1021336fb53b70de22de7',
        native_sha256='3fc50d9b24edef1464293114a32c9217c5dbeaff126f0b66a25e099d8e27e5ca'),
    ('macos', 'x86_64'): dict(run=41454270155,
        source='a7f562d5c0c1bdc491668336ac45dedbb2c38bcd', artifact=13148645788,
        zip_sha256='193baf06b757277f0cc6553404152d92ea1491b2a97ff63ca9d04864b55231b2',
        native_sha256='60facbfa9ee5454811c3040d04ce63fa6d4a856f74cd2585e8145ffcd150f13c'),
    ('linux', 'x86_64'): dict(run=41422733643,
        source='90d69b4738f76485e40f1002125de1d46c6254c3', artifact=13130543571,
        zip_sha256='eadf89691b1c4a6992014d8e383b59026bcd9ec3626a708e46e01201237edffc',
        native_sha256='67a6f101708bbe49e5ece263bb02ed97b83ce6b9ad31e1a288c3516f4f5e1444'),
}


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def sha(path):
    result = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024*1024), b''):
            result.update(block)
    return result.hexdigest()


def api(route):
    result = subprocess.run(['gh', 'api', 'repos/'+REPOSITORY+'/'+route],
                            capture_output=True, text=True, check=True, timeout=60)
    return json.loads(result.stdout)


def validate_prefix(prefix, target, mode, source):
    manifest = json.loads((prefix/'qualification-manifest.json').read_text())
    require(manifest['feature'] == 'flutter-desktop', 'Wrong frozen feature')
    actual = {path.relative_to(prefix).as_posix(): path for path in prefix.rglob('*')
              if (path.is_file() or path.is_symlink()) and path != prefix/'qualification-manifest.json'}
    if target == 'macos':
        require(manifest['architecture'] == platform.machine()
                and manifest['mode'] == mode, 'Wrong frozen Mac mode/architecture')
        expected = {row['path']: row for row in manifest['files']}
        require(len(expected) == len(manifest['files']), 'Duplicate frozen Mac paths')
        require(set(actual) == set(expected), 'Missing/extra frozen Mac payload')
        for name, row in expected.items():
            path = actual[name]
            if 'symlink' in row:
                require(path.is_symlink() and os.readlink(path) == row['symlink']
                        and path.resolve().is_relative_to(prefix), 'Changed/escaping frozen symlink')
            else:
                require(not path.is_symlink() and path.stat().st_size == row['bytes']
                        and sha(path) == row['sha256'], 'Changed frozen Mac bytes: '+name)
    else:
        require(manifest['source_revision'] == source and manifest['mode'] == mode,
                'Wrong frozen Linux source/mode')
        actual.pop('SHA256SUMS', None)
        require(set(actual) == set(manifest['files']), 'Missing/extra frozen Linux payload')
        for name, digest in manifest['files'].items():
            require(not actual[name].is_symlink() and sha(actual[name]) == digest,
                    'Changed frozen Linux bytes: '+name)
    return manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--platform', choices=['macos', 'linux'], required=True)
    parser.add_argument('--root', type=Path, required=True)
    args = parser.parse_args()
    require(os.environ.get('RUNNER_ENVIRONMENT') == 'github-hosted' and os.geteuid() != 0,
            'Disposable non-root GitHub-hosted replay only')
    require(platform.system() == ('Darwin' if args.platform == 'macos' else 'Linux'), 'Wrong native host')
    require(os.environ['GITHUB_REPOSITORY'] == REPOSITORY, 'Wrong repository')
    root = args.root.resolve()
    allowed = Path(os.environ['RUNNER_TEMP']).resolve() if args.platform == 'macos' else Path.home()/'mcw-qualification'
    require(root.is_relative_to(allowed), 'Replay output outside qualification scope')
    if not root.exists():
        root.mkdir(mode=0o700)
    replay = root/'replay'
    replay.mkdir(mode=0o700)
    architecture = platform.machine()
    require((args.platform, architecture) in PINS, 'No audited B5 replay artifact for this platform/architecture')
    pin = PINS[(args.platform, architecture)]
    product_source, run, artifact_id = pin['source'], pin['run'], pin['artifact']
    zip_digest, native_digest = pin['zip_sha256'], pin['native_sha256']
    expected_name = ('flutter-macos-'+architecture if args.platform == 'macos'
                     else 'flutter-linux-x64-ubuntu2204-baseline')+'-'+product_source
    metadata = api('actions/artifacts/'+str(artifact_id))
    require(metadata['name'] == expected_name and not metadata['expired']
            and metadata['workflow_run']['id'] == run
            and metadata['workflow_run']['head_sha'] == product_source
            and metadata['digest'] == 'sha256:'+zip_digest, 'Artifact provenance mismatch')
    comparison = api('compare/'+product_source+'...'+os.environ['GITHUB_SHA'])
    require(comparison['merge_base_commit']['sha'] == product_source and len(comparison['files']) < 300,
            'Replay source ancestry/diff is incomplete')
    allowed_exact = {'.github/workflows/ci.yml', '.github/workflows/flutter-qualification.yml',
                     '.github/workflows/flutter-windows-qualification.yml',
                     'installer/tests/native_macos_menu.py'}
    changed = [row['filename'] for row in comparison['files']]
    require(all(name in allowed_exact or name.startswith(('desktop/qualification/', 'docs/'))
                for name in changed), 'Product source changed since frozen build; rebuild required')
    (replay/'artifact.json').write_text(json.dumps(metadata, indent=2))
    archive = replay/'original-artifact.zip'
    with archive.open('xb') as output:
        subprocess.run(['gh', 'api', 'repos/'+REPOSITORY+'/actions/artifacts/'+str(artifact_id)+'/zip'],
                       stdout=output, check=True, timeout=180)
    require(sha(archive) == zip_digest, 'Artifact ZIP SHA256 mismatch')
    native = replay/'original-native.tar.gz'
    with zipfile.ZipFile(archive) as package:
        basename = 'evidence-native.tar.gz' if args.platform == 'macos' else 'miccamwatch-b3-linux-x64-qualification.tar.gz'
        matches = [item for item in package.infolist() if PurePosixPath(item.filename).name == basename]
        require(len(matches) == 1, 'Missing/duplicate native archive')
        with package.open(matches[0]) as source, native.open('xb') as output:
            shutil.copyfileobj(source, output, 1024*1024)
        if args.platform == 'linux':
            helpers = [item for item in package.infolist() if PurePosixPath(item.filename).name == 'linux_xwindow']
            require(len(helpers) == 1, 'Missing/duplicate owned-window helper')
            helper = root/'linux_xwindow'
            with package.open(helpers[0]) as source, helper.open('xb') as output:
                shutil.copyfileobj(source, output)
            require(sha(helper) == '55ce40fa80ade032c9b6557fe9bf47597d93c73c957f76e305183ffdf8a590ac',
                    'Owned-window helper SHA256 mismatch')
            helper.chmod(0o700)  # only this verified new fixture; ZIP loses executable modes
    require(sha(native) == native_digest, 'Native TAR SHA256 mismatch')
    top = 'evidence' if args.platform == 'macos' else 'bundle'
    with tarfile.open(native) as package:
        for member in package.getmembers():
            parts = PurePosixPath(member.name)
            require(not parts.is_absolute() and '..' not in parts.parts
                    and parts.parts and parts.parts[0] == top, 'Escaping TAR member')
            require(member.isfile() or member.isdir() or member.issym() or member.islnk(),
                    'Unexpected native TAR object')
            if member.issym() or member.islnk():
                target = posixpath.normpath(posixpath.join(str(parts.parent), member.linkname)
                                            if member.issym() else member.linkname)
                require(target.startswith(top+'/'), 'Escaping TAR link')
    destination = replay/'frozen' if args.platform == 'macos' else root
    if args.platform == 'macos':
        destination.mkdir(mode=0o700)
    subprocess.run(['/usr/bin/tar', '-xzf', str(native), '-C', str(destination)], check=True, timeout=120)
    environment = {}
    if args.platform == 'macos':
        evidence = destination/'evidence'
        for variant in ('release', 'instrumented'):
            mode = 'settings_development' if variant == 'instrumented' else 'read_only_development'
            validate_prefix(evidence/(variant+'-prefix'), 'macos', mode, product_source)
        target = Path(__file__).resolve().parents[2]/'target/release/mcw'
        target.parent.mkdir(parents=True, exist_ok=True)
        require(not target.exists(), 'Unexpected release binary: no overwrite permitted')
        shutil.copy2(evidence/'release-prefix/mcw', target)
        environment = {'MCW_FROZEN_RELEASE_APP': str(evidence/'release-prefix/desktop/MicCamWatch.app'),
                       'MCW_FROZEN_INSTRUMENTED_APP': str(evidence/'instrumented-prefix/desktop/MicCamWatch.app')}
    else:
        validate_prefix(root/'bundle', 'linux', 'settings_development', product_source)
    with open(os.environ['GITHUB_ENV'], 'a') as output:
        for key, value in environment.items():
            require('\n' not in value and '\r' not in value, 'Invalid environment path')
            output.write(key+'='+value+'\n')
    receipt = dict(status='restored-and-pinned', product_source=product_source,
                   harness_source=os.environ['GITHUB_SHA'], original_run=run,
                   artifact_id=artifact_id, zip_sha256=zip_digest, native_tar_sha256=native_digest,
                   changed_controller_paths=changed,
                   builds_and_unit_suites='NOT rerun in replay; source run retains original results',
                   runtime='Not established by restoration; subsequent owned tests must pass')
    (root/'replay-products.json').write_text(json.dumps(receipt, indent=2))
    print(json.dumps(receipt))


if __name__ == '__main__':
    main()
