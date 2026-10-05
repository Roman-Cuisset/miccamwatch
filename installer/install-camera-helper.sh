#!/bin/sh
# Explicit administrator-only setup. Review this script before running it as root.
# Python's isolated mode ignores user Python paths; no archive script is executed.
PATH=/usr/sbin:/usr/bin:/sbin:/bin
export PATH
exec /usr/bin/python3 -I - "$@" <<'PYTHON'
import argparse
import contextlib
import fcntl
import hashlib
import json
import os
import platform
import re
import shutil
import signal
import stat
import struct
import subprocess
import sys
import tarfile
import tempfile
import xml.etree.ElementTree as ET
from pathlib import Path

STATE = Path('/var/lib/miccamwatch')
HELPER = Path('/usr/local/libexec/miccamwatch/mcw-camera-helper')
POLICY = Path('/usr/share/polkit-1/actions/com.roman-cuisset.miccamwatch.camera.policy')
RECEIPT = STATE / 'camera-helper-install.json'
TRANSACTION = STATE / '.camera-helper-transaction'
TARGETS = {'helper': (HELPER, 0o755), 'policy': (POLICY, 0o644),
           'receipt': (RECEIPT, 0o600), 'status': (STATE / 'status.json', 0o644),
           'journal': (STATE / 'journal.json', 0o600)}
ACTION = 'com.roman-cuisset.miccamwatch.camera'
MEMBERS = {'mcw', 'mcw-camera-helper', 'install-camera-helper.sh',
           ACTION + '.policy', 'README.md', 'LICENSE'}
VERSION = re.compile(r'[0-9]+\.[0-9]+\.[0-9]+(?:[-+][A-Za-z0-9][A-Za-z0-9.-]*)?\Z')
HASH = re.compile(r'[0-9a-f]{64}\Z')


def fail(message):
    raise RuntimeError(message)


def exists(path):
    return os.path.lexists(path)


def directory(path, create=False, mode=None):
    # Never resolve symlinks: inspect every component of these fixed paths.
    root = Path('/').lstat()
    if not stat.S_ISDIR(root.st_mode) or root.st_uid != 0 or root.st_mode & 0o022:
        fail('Unsafe root directory')
    current = Path('/')
    for component in path.parts[1:]:
        current /= component
        if not exists(current) and create:
            os.mkdir(current, 0o755)
            os.chown(current, 0, 0)
            os.chmod(current, 0o755)
            sync_dir(current.parent)
        info = current.lstat()
        if not stat.S_ISDIR(info.st_mode) or info.st_uid != 0 or info.st_mode & 0o022:
            fail(f'Unsafe directory ancestor: {current}; require root ownership, no links or group/other write')
    if mode is not None and stat.S_IMODE(path.lstat().st_mode) != mode:
        fail(f'{path} must have mode {mode:04o}; existing permissions were not changed')


def regular(path, mode=None):
    info = path.lstat()
    if (not stat.S_ISREG(info.st_mode) or info.st_uid != 0 or info.st_nlink != 1
            or info.st_mode & 0o7022):
        fail(f'Unsafe root-owned regular file (link, ownership or permissions): {path}')
    if mode is not None and stat.S_IMODE(info.st_mode) != mode:
        fail(f'{path} must have mode {mode:04o}')
    return info


def sync_dir(path):
    fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def digest(path):
    with path.open('rb') as source:
        result = hashlib.sha256()
        for chunk in iter(lambda: source.read(1024 * 1024), b''):
            result.update(chunk)
        return result.hexdigest()


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            fail(f'Duplicate JSON key: {key}')
        result[key] = value
    return result


def read_json(path, mode=0o600):
    regular(path, mode)
    with path.open('r', encoding='utf-8') as source:
        return json.load(source, object_pairs_hook=unique_object)


def write_file(path, data, mode):
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, mode)
    with os.fdopen(fd, 'wb') as output:
        output.write(data)
        os.fchmod(output.fileno(), mode)
        os.fchown(output.fileno(), 0, 0)
        output.flush()
        os.fsync(output.fileno())


def atomic_copy(source, target, mode):
    directory(target.parent)
    fd, name = tempfile.mkstemp(prefix='.mcw-camera-stage-', dir=target.parent)
    try:
        with os.fdopen(fd, 'wb') as output, source.open('rb') as source_file:
            shutil.copyfileobj(source_file, output)
            os.fchmod(output.fileno(), mode)
            os.fchown(output.fileno(), 0, 0)
            output.flush()
            os.fsync(output.fileno())
        os.replace(name, target)
        sync_dir(target.parent)
    finally:
        if os.path.lexists(name):
            os.unlink(name)


def journal_clear(journal=STATE / 'journal.json', status=TARGETS['status'][0], status_mode=0o644):
    if exists(journal):
        data = read_json(journal)
        if (not isinstance(data, dict) or set(data) != {'format', 'digest', 'entries'}
                or type(data['format']) is not int or data['format'] != 1
                or data['entries'] != []
                or data['digest'] != hashlib.sha256(b'[]').hexdigest()):
            fail('Camera restoration journal is pending or unsupported. Run the matching old mcw camera allow first; evidence was preserved')
    # The same validation protects empty backups in known private staging.
    if exists(status):
        cache = read_json(status, status_mode)
        if (not isinstance(cache, dict) or set(cache) != {'format', 'version', 'boot_id', 'digest', 'entries'}
                or type(cache['format']) is not int or cache['format'] != 1
                or not isinstance(cache['version'], str) or not VERSION.fullmatch(cache['version'])
                or not isinstance(cache['boot_id'], str)
                or not re.fullmatch(r'[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}', cache['boot_id'])
                or cache['entries'] != []
                or cache['digest'] != hashlib.sha256(b'[]').hexdigest()):
            fail('Camera status cache is malformed or contains retained binding evidence. Use the matching old mcw camera allow; evidence was preserved')


def load_receipt():
    if not exists(RECEIPT):
        for name in ('helper', 'policy'):
            if exists(TARGETS[name][0]):
                fail(f'Unowned existing {TARGETS[name][0]}; refusing replacement/removal')
        return None
    receipt = read_json(RECEIPT)
    if (not isinstance(receipt, dict) or set(receipt) != {'format', 'version', 'archive_sha256', 'helper_sha256', 'policy_sha256'}
            or type(receipt['format']) is not int or receipt['format'] != 1
            or not isinstance(receipt['version'], str) or not VERSION.fullmatch(receipt['version'])
            or any(not isinstance(receipt[key], str) or not HASH.fullmatch(receipt[key])
                   for key in ('archive_sha256', 'helper_sha256', 'policy_sha256'))):
        fail('Invalid root installation receipt; it is data, never executable code')
    for name in ('helper', 'policy'):
        path, mode = TARGETS[name]
        if not exists(path):
            fail(f'Receipt-owned file is missing: {path}; refusing partial installation')
        regular(path, mode)
        if digest(path) != receipt[name + '_sha256']:
            fail(f'Receipt-owned file changed: {path}; refusing replacement/removal')
    return receipt


def remove_transaction():
    # Rename marks completion atomically. A crash while deleting backups must not
    # turn a committed installation into an incomplete rollback transaction.
    completed = Path(tempfile.mkdtemp(prefix='.camera-helper-completed-', dir=STATE))
    os.replace(TRANSACTION, completed)
    sync_dir(STATE)
    shutil.rmtree(completed)
    sync_dir(STATE)


def remove_orphan_staging():
    # Only unpublished preparations, authenticated-archive work, and completed
    # transactions can have these generated names. Never discard the fixed
    # published recovery transaction or a backup containing restoration evidence.
    candidates = []
    for path in STATE.iterdir():
        if path == TRANSACTION or not path.name.startswith('.camera-helper-'):
            continue
        match = re.fullmatch(r'\.camera-helper-(preparation|private|completed)-[a-z0-9_]{8}', path.name)
        if match is None:
            fail(f'Unknown administrator staging remnant preserved for inspection: {path}')
        kind = match[1]
        directory(path, mode=0o700)
        permitted = MEMBERS | {'archive.tar.gz', 'receipt.json'} if kind == 'private' else set(TARGETS) | {'transaction.json'}
        for child in path.iterdir():
            staging = kind == 'preparation' and re.fullmatch(r'\.mcw-camera-stage-[a-z0-9_]{8}', child.name)
            if child.name not in permitted and not staging:
                fail(f'Unexpected administrator staging content preserved for inspection: {child}')
            info = regular(child)
            modes = (0o600, 0o755) if kind == 'private' and child.name in ('mcw', 'mcw-camera-helper') else (0o600,)
            if stat.S_IMODE(info.st_mode) not in modes:
                fail(f'Unexpected administrator staging permissions preserved for inspection: {child}')
        if kind != 'private':
            journal_clear(path / 'journal', path / 'status', 0o600)
        candidates.append(path)
    # Preflight every candidate before deleting any. No recursive or linked
    # content is admitted; the permanent operation lock is held throughout.
    for path in candidates:
        for child in path.iterdir():
            child.unlink()
        path.rmdir()
        sync_dir(STATE)


def recover_transaction():
    if not exists(TRANSACTION):
        return
    directory(TRANSACTION, mode=0o700)
    manifest = read_json(TRANSACTION / 'transaction.json')
    if not isinstance(manifest, dict) or set(manifest) != {'format', 'old', 'new'} or type(manifest['format']) is not int or manifest['format'] != 1:
        fail('Invalid installation recovery manifest; preserved for administrator inspection')
    for group in ('old', 'new'):
        values = manifest[group]
        if not isinstance(values, dict) or set(values) != set(TARGETS):
            fail('Invalid installation recovery file set')
        for value in values.values():
            if value is not None and (not isinstance(value, str) or not HASH.fullmatch(value)):
                fail('Invalid installation recovery checksum')
    if set(os.listdir(TRANSACTION)) != {'transaction.json'} | {name for name, value in manifest['old'].items() if value is not None}:
        fail('Unexpected installation recovery contents; preserved')
    # Validate the entire rollback before changing any destination.
    for name, (path, mode) in TARGETS.items():
        directory(path.parent)
        if exists(path):
            regular(path, mode)
            # Runtime may have refreshed empty evidence after an interrupted
            # install. journal_clear already validates it under the shared lock;
            # neither journal nor cache is an installation ownership receipt.
            if name not in ('status', 'journal') and digest(path) not in (manifest['old'][name], manifest['new'][name]):
                fail(f'Changed file prevents safe rollback: {path}')
        if manifest['old'][name] is not None:
            backup = TRANSACTION / name
            regular(backup, 0o600)
            if digest(backup) != manifest['old'][name]:
                fail('Changed recovery backup; old installation preserved for inspection')
    for name, (path, mode) in TARGETS.items():
        if manifest['old'][name] is None:
            if exists(path):
                path.unlink()
                sync_dir(path.parent)
        else:
            atomic_copy(TRANSACTION / name, path, mode)
    remove_transaction()
    print('Restored previous installation from interrupted administrator transaction.')


def commit(staged):
    # Incomplete private preparation cannot occupy the fixed recovery path.
    # Publish only complete, durable backups + manifest before any target change.
    preparation = Path(tempfile.mkdtemp(prefix='.camera-helper-preparation-', dir=STATE))
    old = {}
    new = {}
    try:
        for name, (path, mode) in TARGETS.items():
            old[name] = digest(path) if exists(path) else None
            new[name] = digest(staged[name]) if name in staged else None
            if old[name] is not None:
                atomic_copy(path, preparation / name, 0o600)
        write_file(preparation / 'transaction.json',
                   (json.dumps({'format': 1, 'old': old, 'new': new}, sort_keys=True) + '\n').encode(), 0o600)
        sync_dir(preparation)
        if exists(TRANSACTION):
            fail('An installation recovery transaction already exists; refusing publication')
        os.rename(preparation, TRANSACTION)
        sync_dir(STATE)
    except BaseException:
        # No installed file was touched. If publication occurred, leave its
        # complete manifest for normal recovery; otherwise discard preparation.
        if exists(preparation):
            shutil.rmtree(preparation)
        raise
    try:
        for name, (path, mode) in TARGETS.items():
            if name in staged:
                atomic_copy(staged[name], path, mode)
            elif exists(path):
                path.unlink()
                sync_dir(path.parent)
    except BaseException as error:
        try:
            recover_transaction()
        except BaseException as rollback:
            fail(f'Installation failed ({error}); rollback could not complete ({rollback}). Recovery backups retained at {TRANSACTION}')
        raise
    remove_transaction()


def copy_archive(source, destination):
    # User-owned source bytes are never executed. Only this fresh private root copy
    # is authenticated and opened by tarfile; source races cannot bypass SHA-256.
    fd = os.open(source, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, 'rb') as incoming:
        info = os.fstat(incoming.fileno())
        if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1:
            fail('Archive must be a regular file, not a symlink or hardlink')
        out_fd = os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
        with os.fdopen(out_fd, 'wb') as output:
            shutil.copyfileobj(incoming, output)
            output.flush()
            os.fsync(output.fileno())


def extract_archive(archive, work):
    with tarfile.open(archive, 'r:gz') as package:
        members = package.getmembers()
        if len(members) != len(MEMBERS) or {member.name for member in members} != MEMBERS:
            fail('Archive must contain exactly the six flat Linux release members, once each')
        for member in members:
            if member.type not in (tarfile.REGTYPE, tarfile.AREGTYPE) or member.sparse or member.linkname:
                fail(f'Archive links/special files are forbidden: {member.name}')
            # Stream regular payloads to fixed names, ignoring archive uid/mode/path.
            with contextlib.closing(package.extractfile(member)) as source:
                fd = os.open(work / member.name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
                with os.fdopen(fd, 'wb') as output:
                    shutil.copyfileobj(source, output)
                    output.flush()
                    os.fsync(output.fileno())


def validate_policy(path):
    root = ET.parse(path).getroot()
    if root.tag != 'policyconfig' or root.attrib or [child.tag for child in root] != ['vendor', 'vendor_url', 'action']:
        fail('Unexpected polkit policy structure')
    for child in list(root)[:2]:
        if child.attrib or len(child):
            fail('Unexpected polkit vendor configuration')
    action = root.find('action')
    if action.attrib != {'id': ACTION} or [child.tag for child in action] != ['description', 'message', 'defaults', 'annotate']:
        fail('Policy must define only the fixed MicCamWatch camera action')
    for child in list(action)[:2]:
        if child.attrib or len(child):
            fail('Unexpected polkit description configuration')
    defaults = action.find('defaults')
    if defaults.attrib or [child.tag for child in defaults] != ['allow_any', 'allow_inactive', 'allow_active']:
        fail('Policy defaults must cover exactly all three authentication contexts')
    for child in defaults:
        if child.attrib or len(child) or (child.text or '').strip() != 'auth_admin':
            fail('Policy must require auth_admin, without implicit or cached grants')
    annotation = action.find('annotate')
    if (annotation.attrib != {'key': 'org.freedesktop.policykit.exec.path'}
            or len(annotation) or (annotation.text or '').strip() != str(HELPER)):
        fail('Policy must authorize only the fixed root camera helper path')


def probe(path, option):
    with path.open('rb') as source:
        header = source.read(20)
    if (len(header) != 20 or header[:6] != b'\x7fELF\x02\x01'
            or header[7] not in (0, 3) or struct.unpack('<HH', header[16:20]) not in ((2, 62), (3, 62))):
        fail(f'Not a Linux x86_64 ELF executable: {path.name}')
    os.chmod(path, 0o755)
    # This is the first execution of archive bytes, strictly after checksum match.
    result = subprocess.run([str(path), option], check=True, stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE, text=True,
                            env={'PATH': '/usr/sbin:/usr/bin:/sbin:/bin', 'LC_ALL': 'C', 'HOME': '/root'},
                            cwd=path.parent)
    return result.stdout.rstrip('\n')


def interrupted(signum, frame):
    raise RuntimeError('Administrator installation interrupted')


def main():
    parser = argparse.ArgumentParser(
        description='Reviewed explicit root setup/removal of the fixed Linux USB camera helper. No network, elevation, autostart or device actions.',
        epilog='Run only after an administrator reviews this installer and policy. --sha256 must come from a trusted source: SHA-256 proves archive integrity, not independent publisher authenticity. Requires Linux x86_64 and /usr/bin/python3. Updates/removal refuse pending restoration evidence; first use the matching old mcw camera allow. Explicit uninstall removes only integrity-valid empty journal/cache and preserves the operation lock; no user-prefix executable is run.')
    parser.add_argument('--archive', help='Absolute path to the Linux release .tar.gz; copied freshly to private root storage before verification')
    parser.add_argument('--sha256', help='Expected trusted SHA-256 of the archive (64 hexadecimal characters)')
    parser.add_argument('--uninstall', action='store_true', help='Explicitly remove receipt-owned unchanged helper/policy and integrity-valid empty runtime journal/cache')
    args = parser.parse_args()
    if args.uninstall:
        if args.archive is not None or args.sha256 is not None:
            parser.error('--uninstall cannot be combined with --archive/--sha256')
    elif args.archive is None or args.sha256 is None:
        parser.error('setup/update requires both --archive ABSOLUTE_PATH and --sha256 TRUSTED_HASH')
    if os.geteuid() != 0 or os.getuid() != 0:
        fail('Run this reviewed installer explicitly as root; it never elevates itself')
    if platform.system() != 'Linux' or platform.machine() != 'x86_64':
        fail('Only Linux x86_64 release archives are supported')
    os.umask(0o077)
    if not args.uninstall:
        if not os.path.isabs(args.archive):
            fail('--archive must be an absolute path')
        args.sha256 = args.sha256.lower()
        if not HASH.fullmatch(args.sha256):
            fail('--sha256 must contain exactly 64 hexadecimal characters')
    for path, mode in TARGETS.values():
        directory(path.parent, create=True)
        if exists(path):
            regular(path, mode)
    directory(STATE, mode=0o755)
    lock = STATE / 'operation.lock'
    if exists(lock):
        regular(lock, 0o600)
    fd = os.open(lock, os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, 'r+b') as operation_lock:
        regular(lock, 0o600)
        fcntl.flock(operation_lock, fcntl.LOCK_EX)
        journal_clear()
        recover_transaction()
        remove_orphan_staging()
        old = load_receipt()
        if args.uninstall:
            if old is None and not any(exists(TARGETS[name][0]) for name in ('journal', 'status')):
                print('No administrator-owned camera helper installation or empty runtime state to remove.')
                return
            # journal_clear validated both files as integrity-valid and empty.
            # Remove stale versioned observations with the root installation,
            # but never replace or unlink the permanent operation.lock inode.
            commit({})
            print('Removed unchanged administrator-owned helper, policy and receipt plus validated empty journal/cache. The permanent operation lock was preserved.')
            return
        with tempfile.TemporaryDirectory(prefix='.camera-helper-private-', dir=STATE) as temporary:
            work = Path(temporary)
            directory(work, mode=0o700)
            archive = work / 'archive.tar.gz'
            copy_archive(args.archive, archive)
            if digest(archive) != args.sha256:
                fail('Archive SHA-256 mismatch; no archive executable was run and old installation was preserved')
            extract_archive(archive, work)
            policy = work / (ACTION + '.policy')
            validate_policy(policy)
            reported = probe(work / 'mcw', '--version')
            if not reported.startswith('mcw ') or not VERSION.fullmatch(reported[4:]):
                fail('Authenticated mcw returned an invalid exact version')
            version = reported[4:]
            helper_info = json.loads(probe(work / 'mcw-camera-helper', '--protocol-version'), object_pairs_hook=unique_object)
            if (not isinstance(helper_info, dict) or set(helper_info) != {'protocol', 'version'}
                    or type(helper_info['protocol']) is not int or helper_info['protocol'] != 1
                    or helper_info['version'] != version):
                fail('Authenticated mcw and helper must report the same exact version and helper protocol 1')
            receipt = {'format': 1, 'version': version, 'archive_sha256': args.sha256,
                       'helper_sha256': digest(work / 'mcw-camera-helper'), 'policy_sha256': digest(policy)}
            staged_receipt = work / 'receipt.json'
            write_file(staged_receipt, (json.dumps(receipt, sort_keys=True) + '\n').encode(), 0o600)
            # The shared operation lock remains held from journal check through commit.
            staged = {'helper': work / 'mcw-camera-helper', 'policy': policy, 'receipt': staged_receipt}
            journal = TARGETS['journal'][0]
            if exists(journal):
                staged['journal'] = journal
            commit(staged)
            print(f'Installed root camera helper {version} (protocol 1) at {HELPER}; policy {POLICY}. No camera action or autostart was performed.')


for caught_signal in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
    signal.signal(caught_signal, interrupted)
try:
    main()
except (Exception, KeyboardInterrupt) as error:
    print(f'mcw camera administrator installer: {error}', file=sys.stderr)
    sys.exit(1)
PYTHON
