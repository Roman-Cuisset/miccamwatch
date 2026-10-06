#!/usr/bin/env python3
"""Disposable GitHub-hosted Linux proof using the real feature archive.

Run as the runner user, never root. Explicit sudo bootstraps the reviewed root
installer; only status is authorized (no block/allow/toggle). Existing installer
fixtures supply genuine user payloads and controlled release transport. strace
sends real SIGTERM/SIGKILL at observed fsync boundaries; no
production hooks or fabricated recovery manifests are used. Proof files explain
that sudo authentication is not a test of interactive polkit authentication.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile

# Isolated root Python loads only the reviewed adjacent fixture harness.
sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
import native_install as fixtures

STATE = Path('/var/lib/miccamwatch')
HELPER = Path('/usr/local/libexec/miccamwatch/mcw-camera-helper')
POLICY = Path('/usr/share/polkit-1/actions/com.roman-cuisset.miccamwatch.camera.policy')
RECEIPT = STATE / 'camera-helper-install.json'
LOCK = STATE / 'operation.lock'
TARGETS = (HELPER, POLICY, RECEIPT, STATE / 'journal.json', STATE / 'status.json')


def require(condition, message):
    fixtures.require(condition, message)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def record(proof, name, argv, codes=(0,), env=None):
    result = subprocess.run([str(arg) for arg in argv], env=env, stdin=subprocess.DEVNULL,
                            capture_output=True, text=True, timeout=300)
    (proof / (name + '.stdout')).write_text(result.stdout)
    (proof / (name + '.stderr')).write_text(result.stderr)
    (proof / (name + '.exit')).write_text(str(result.returncode) + '\n')
    require(codes is None or result.returncode in codes,
            f'{name}: exit {result.returncode}\n{result.stdout}\n{result.stderr}')
    return result


def snapshot():
    result = {}
    for path in (*TARGETS, LOCK):
        if path.exists():
            info = path.lstat()
            require(stat.S_ISREG(info.st_mode) and info.st_uid == 0 and info.st_nlink == 1,
                    f'unsafe native root proof target: {path}')
            result[str(path)] = dict(sha256=sha(path), uid=info.st_uid, gid=info.st_gid,
                                    mode=oct(stat.S_IMODE(info.st_mode)), inode=info.st_ino,
                                    device=info.st_dev)
    return result


def usb_bindings():
    return {str(path): os.readlink(path) for path in Path('/sys/bus/usb/devices').glob('*/driver')
            if path.is_symlink()}


def verify_install(archive, version):
    receipt = json.loads(RECEIPT.read_text())
    with tarfile.open(archive, 'r:gz') as package:
        require(HELPER.read_bytes() == package.extractfile('mcw-camera-helper').read()
                and POLICY.read_bytes() == package.extractfile(POLICY.name).read(),
                'root helper/policy differ from genuine authenticated archive members')
    require(receipt == dict(format=1, version=version, archive_sha256=sha(archive),
                            helper_sha256=sha(HELPER), policy_sha256=sha(POLICY)),
            'root receipt does not authenticate the installed same-version payload')
    for path, mode in ((HELPER, 0o755), (POLICY, 0o644), (RECEIPT, 0o600), (LOCK, 0o600)):
        info = path.lstat()
        require(info.st_uid == info.st_gid == 0 and stat.S_IMODE(info.st_mode) == mode
                and stat.S_ISREG(info.st_mode) and info.st_nlink == 1,
                f'wrong root ownership/mode/type: {path}')
    require(not (STATE / '.camera-helper-transaction').exists(), 'completed setup retained transaction')


def root_phase(args):
    require(os.getuid() == os.geteuid() == 0, 'root phase needs explicit sudo')
    os.umask(0o022)  # Root proof output must remain readable to the runner/upload.
    package_version = args.version.removeprefix('v')
    proof = args.proof
    setup = ['/bin/sh', args.root_installer, '--archive', args.archive, '--sha256', sha(args.archive)]
    clean_env = dict(PATH='/usr/sbin:/usr/bin:/sbin:/bin', HOME='/root', LC_ALL='C')
    if args.root_phase == 'cleanup':
        if not (args.work / 'root-bootstrap-started').exists():
            print('No pristine-runner bootstrap started; existing root assets were not touched.')
            return
        before = snapshot()
        lock_identity = before.get(str(LOCK))
        record(proof, 'root-explicit-uninstall', ['/bin/sh', args.root_installer, '--uninstall'], env=clean_env)
        require(all(not path.exists() for path in TARGETS), 'explicit root uninstall retained owned targets')
        after = snapshot()
        require(lock_identity is not None and after.get(str(LOCK)) == lock_identity,
                'root uninstall replaced/deleted the permanent lock')
        (proof / 'root-uninstalled.json').write_text(json.dumps(after, indent=2) + '\n')
        return

    require(not any(path.exists() for path in TARGETS) and not STATE.exists(),
            'root smoke requires a pristine disposable runner; never replaces existing root assets')
    (args.work / 'root-bootstrap-started').write_text('pristine disposable runner verified\n')
    initial_usb = usb_bindings()
    (proof / 'usb-bindings-before.json').write_text(json.dumps(initial_usb, indent=2) + '\n')
    record(proof, 'root-bootstrap', setup, env=clean_env)
    verify_install(args.archive, package_version)
    before = snapshot()
    lock_identity = before[str(LOCK)]
    (proof / 'root-installed.json').write_text(json.dumps(before, indent=2) + '\n')
    protocol = record(proof, 'installed-helper-protocol', [HELPER, '--protocol-version'], env=clean_env)
    require(json.loads(protocol.stdout) == {'protocol': 1, 'version': package_version}, 'protocol probe mismatch')
    action = ['--protocol', '1', '--version', package_version, '--action', 'status']
    for name, arguments, caller in (
        ('malformed', ['--protocol'], None),
        ('wrong-protocol', ['--protocol', '2', '--version', package_version, '--action', 'status'], str(args.caller_uid)),
        ('wrong-version', ['--protocol', '1', '--version', '0.0.0', '--action', 'status'], str(args.caller_uid)),
        ('invalid-action', action[:-1] + ['not-an-action'], str(args.caller_uid)),
        ('missing-caller', action, None),
        ('invalid-caller', action, 'not-a-uid'),
    ):
        env = dict(clean_env)
        if caller is not None:
            env['PKEXEC_UID'] = caller
        refused = record(proof, 'refusal-' + name, [HELPER, *arguments], codes=(1,), env=env)
        reply = json.loads(refused.stdout)
        require(reply['ok'] is False and reply['state'] is None and reply['error'], 'refusal falsely claimed a state')
        require(snapshot() == before and usb_bindings() == initial_usb,
                'refused root request changed installation/journal/cache/USB bindings')

    # A malformed archive cannot be parsed or executed before its checksum agrees.
    # The real archive with a wrong expected digest additionally protects old bytes.
    for name, archive in (('genuine', args.archive), ('invalid-bytes', args.work / 'not-an-archive')):
        if name == 'invalid-bytes':
            archive.write_bytes(b'not an executable or an archive\n')
        trace_path = args.work / ('wrong-sha-' + name + '.strace')
        wrong = record(proof, 'wrong-sha-' + name,
                       ['strace', '-f', '-e', 'trace=execve', '-o', trace_path,
                        '/bin/sh', args.root_installer, '--archive', archive, '--sha256', '0' * 64],
                       codes=(1,), env=clean_env)
        require('SHA-256 mismatch' in wrong.stderr and snapshot() == before,
                'wrong archive hash did not preserve old root bytes/receipt')
        trace_text = trace_path.read_text()
        require(not re.search(r'execve\("[^"]*\.camera-helper-private-', trace_text),
                'wrong-checksum archive executable was invoked')
        (proof / ('wrong-sha-' + name + '-execve.txt')).write_text(trace_text)

    # Observe successful real refresh syscall boundaries, then interrupt those
    # same syscalls with kernel-delivered SIGTERM in the unchanged installer.
    trace = args.work / 'refresh.strace'
    record(proof, 'root-same-version-refresh',
           ['strace', '-e', 'trace=fsync', '-yy', '-o', trace, *setup], env=clean_env)
    verify_install(args.archive, package_version)
    require(snapshot()[str(LOCK)] == lock_identity, 'refresh replaced permanent root lock')
    fsyncs = [line for line in trace.read_text().splitlines() if line.startswith('fsync(')]
    preparation = [index + 1 for index, line in enumerate(fsyncs)
                   if re.search(r'<\/var/lib/miccamwatch/\.camera-helper-preparation-[^/<>]+>', line)]
    require(preparation, 'could not identify actual private preparation durability boundary')
    publication = preparation[-1] + 1
    require(publication <= len(fsyncs) and '</var/lib/miccamwatch>' in fsyncs[publication - 1],
            'could not identify actual transaction publication durability boundary')
    (proof / 'refresh-fsync-trace.txt').write_text(trace.read_text())
    for name, ordinal in (('preparation', preparation[0]), ('publication', publication)):
        old = snapshot()
        interrupted = record(proof, 'interrupt-' + name,
                             ['strace', '-e', 'trace=fsync', '-yy', '-o', args.work / (name + '.strace'),
                              '-e', f'inject=fsync:signal=SIGTERM:when={ordinal}', *setup],
                             codes=(1,), env=clean_env)
        require('Administrator installation interrupted' in interrupted.stderr,
                'real installer signal handler was not exercised')
        require(snapshot() == old, 'interruption changed published installation bytes or lock')
        transaction = STATE / '.camera-helper-transaction'
        require(transaction.exists() == (name == 'publication'), 'wrong recovery-publication boundary observed')
        require(not list(STATE.glob('.camera-helper-preparation-*')), 'interrupted preparation left private scaffolding')
        (proof / (name + '-signal-trace.txt')).write_text((args.work / (name + '.strace')).read_text())
        recovered = record(proof, 'recovery-' + name, setup, env=clean_env)
        if name == 'publication':
            require('Restored previous installation' in recovered.stdout, 'real published transaction was not recovered')
        verify_install(args.archive, package_version)
        require(snapshot()[str(LOCK)] == lock_identity, 'recovery replaced permanent lock')

    private = [index + 1 for index, line in enumerate(fsyncs)
               if re.search(r'<\/var/lib/miccamwatch/\.camera-helper-private-[^/<>]+/archive\.tar\.gz>', line)]
    state_syncs = [index + 1 for index, line in enumerate(fsyncs) if '</var/lib/miccamwatch>' in line]
    require(private and len(state_syncs) >= 3, 'could not identify archive/completion durability boundaries')
    for name, ordinal in (('private', private[0]), ('preparation', preparation[0]),
                          ('publication', publication), ('completed', state_syncs[-2])):
        old = snapshot()
        trace_path = args.work / ('kill-' + name + '.strace')
        # subprocess reports direct SIGKILL as -9; a shell may encode it as 137.
        record(proof, 'kill-' + name,
               ['strace', '-e', 'trace=fsync', '-yy', '-o', trace_path,
                '-e', f'inject=fsync:signal=SIGKILL:when={ordinal}', *setup], codes=(-9, 137), env=clean_env)
        trace_text = trace_path.read_text()
        require('killed by SIGKILL' in trace_text, 'uncatchable installer interruption was not exercised')
        (proof / ('kill-' + name + '-trace.txt')).write_text(trace_text)
        require(snapshot()[str(LOCK)] == lock_identity, 'uncatchable interruption changed permanent lock')
        if name != 'completed':
            require(snapshot() == old, 'pre-commit SIGKILL changed installed bytes or file identities')
        record(proof, 'kill-' + name + '-uninstall',
               ['/bin/sh', args.root_installer, '--uninstall'], env=clean_env)
        require(all(not path.exists() for path in TARGETS) and not list(STATE.glob('.camera-helper-*')),
                'explicit root uninstall did not clear genuine interrupted staging/owned targets')
        require(snapshot()[str(LOCK)] == lock_identity, 'orphan cleanup replaced permanent lock')
        record(proof, 'kill-' + name + '-reinstall', setup, env=clean_env)
        verify_install(args.archive, package_version)

    # Explicit sudo authenticates the hosted runner user. Do not claim a real
    # interactive pkexec authorization dialogue was exercised by this adapter.
    authorized_env = dict(clean_env, PKEXEC_UID=str(args.caller_uid))
    status = record(proof, 'authorized-readonly-status', [HELPER, *action], codes=(0, 1), env=authorized_env)
    reply = json.loads(status.stdout)
    require(reply['protocol'] == 1 and reply['version'] == package_version, 'status reply version mismatch')
    if status.returncode == 0:
        require(reply['ok'] is True and reply['state'] in ('allowed', 'blocked', 'system_managed'),
                'authorized status lacks genuine observed state')
    else:
        require(reply['ok'] is False and reply['state'] is None and reply['error'], 'status failure disguised as success')
    for path in (STATE / 'journal.json', STATE / 'status.json'):
        if path.exists():
            document = json.loads(path.read_text())
            require(document['entries'] == [] and document['digest'] == hashlib.sha256(b'[]').hexdigest(),
                    'read-only status created nonempty or unverifiable runtime evidence')
            (proof / ('runtime-' + path.name)).write_text(json.dumps(document, indent=2) + '\n')
    require(usb_bindings() == initial_usb, 'root read-only smoke changed actual USB bindings')
    (proof / 'usb-bindings-after.json').write_text(json.dumps(usb_bindings(), indent=2) + '\n')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--archive', type=Path, required=True)
    parser.add_argument('--source-file', type=Path, required=True)
    parser.add_argument('--root-installer', type=Path, required=True)
    parser.add_argument('--version', required=True)
    parser.add_argument('--proof', type=Path, required=True)
    parser.add_argument('--root-phase', choices=('setup', 'cleanup'))
    parser.add_argument('--caller-uid', type=int)
    parser.add_argument('--work', type=Path)
    args = parser.parse_args()
    require(platform.system() == 'Linux' and platform.machine() == 'x86_64', 'native Linux x86_64 only')
    require(os.environ.get('GITHUB_ACTIONS') == 'true' and os.environ.get('RUNNER_ENVIRONMENT') == 'github-hosted',
            'privileged smoke is restricted to disposable GitHub-hosted runners')
    require(all(path.is_absolute() for path in (args.archive, args.source_file, args.root_installer, args.proof)),
            'native archive/source/proof paths must be absolute')
    if args.root_phase:
        root_phase(args)
        return
    require(os.getuid() == os.geteuid() != 0, 'start privilege smoke as the ordinary runner user')
    require(args.version in ('v0.16.0', 'v0.16.1'), 'feature smoke requires an explicitly reviewed v0.16.0 or v0.16.1')
    args.proof.mkdir(parents=True, exist_ok=True)
    (args.proof / 'scope.txt').write_text(
        'Genuine six-member Linux archive; real ordinary-user install/update/remove/security/rollback fixtures; '
        'explicit administrator root bootstrap/refresh/uninstall. Root status uses explicit sudo authorization '
        'and the runner UID, not an interactive polkit/pkexec authentication-dialogue proof. Malformed/version/caller '
        'refusals are checked before journal/cache/USB changes. Actual strace SIGTERM/SIGKILL interrupt private archive, '
        'preparation, publication and completion; recovery uses only installer-produced backups/manifest. Only read-only status is authorized; '
        'no block/allow/toggle, media acquisition, root autostart or unknown hardware mutation. An unavailable/no-USB '
        'state remains genuine, never substituted with allowed. No physical Linux or macOS hardware parity is proved.\n')
    (args.proof / 'host.json').write_text(json.dumps(dict(os=platform.platform(), arch=platform.machine(),
                                                        uid=os.getuid(), version=args.version), indent=2) + '\n')
    shells = {shell: shutil.which(shell) for shell in ('bash', 'zsh', 'fish')}
    require(all(shells.values()), 'bash/zsh/fish prerequisites are required')
    with tempfile.TemporaryDirectory(prefix='mcw-native-camera-', dir=os.environ['RUNNER_TEMP']) as temporary:
        work = Path(temporary)
        asset, binary, camera = fixtures.local_camera_archive(args.archive, work, args.version)
        ordinary = [sys.executable, Path(fixtures.__file__).resolve(), '--source-file', args.source_file,
                    '--camera-archive', args.archive, '--version', args.version, '--mode', 'fixtures']
        record(args.proof, 'ordinary-user-fixtures', ordinary)
        sandbox = fixtures.Sandbox(work, 'managed-before-root', 'bash', shells['bash'])
        sums = work / 'SHA256SUMS'
        sums.write_text(f'{sha(args.archive)}  {asset}\n')
        env, requests = fixtures.fixture_environment(sandbox, {
            f'{fixtures.RELEASE_ROOT}/{args.version}/{asset}': str(args.archive),
            f'{fixtures.RELEASE_ROOT}/{args.version}/SHA256SUMS': str(sums),
        })
        installed = sandbox.invoke(args.source_file, '--no-modify-path', version=args.version, env=env)
        (args.proof / 'prepared-user-install.stdout').write_text(installed.stdout)
        (args.proof / 'prepared-user-install.stderr').write_text(installed.stderr)
        require(sandbox.binary.read_bytes() == binary and requests.exists(), 'managed prefix was not genuinely installed')
        payload = sandbox.prefix / 'share/miccamwatch/linux-camera'
        require({name: (payload / name).read_bytes() for name in fixtures.CAMERA_NAMES} == camera,
                'managed prefix does not contain the genuine archive payload')
        root_command = ['sudo', '--non-interactive', '--preserve-env=GITHUB_ACTIONS,RUNNER_ENVIRONMENT',
                        '/usr/bin/python3', '-I', '-B', Path(__file__).resolve(), '--archive', args.archive,
                        '--source-file', args.source_file, '--root-installer', args.root_installer,
                        '--version', args.version[1:], '--proof', args.proof,
                        '--caller-uid', str(os.getuid()), '--work', work]
        # Root-created proof/trace files stay readable by the upload and temporary
        # owner. The permanent lock is deliberately not deleted as cleanup.
        try:
            record(args.proof, 'root-phase', [*root_command, '--root-phase', 'setup'])
            action = ['--protocol', '1', '--version', args.version[1:], '--action', 'status']
            refused = record(args.proof, 'ordinary-user-helper-refusal', [HELPER, *action], codes=(1,))
            reply = json.loads(refused.stdout)
            require(reply['ok'] is False and 'administrator authorization' in reply['error'],
                    'ordinary user did not reach the real authorization refusal boundary')
            record(args.proof, 'root-presence-user-fixtures',
                   [*ordinary, '--root-presence-prefix', sandbox.prefix])
        finally:
            record(args.proof, 'root-cleanup-phase', [*root_command, '--root-phase', 'cleanup'])
            removed = sandbox.invoke(args.source_file, '--uninstall', '--no-modify-path', version=args.version, env=env)
            (args.proof / 'explicit-user-cleanup.stdout').write_text(removed.stdout)
            (args.proof / 'explicit-user-cleanup.stderr').write_text(removed.stderr)
            require(not sandbox.binary.exists() and not payload.exists(), 'explicit user cleanup retained owned payload')
    print('PASS genuine Linux privilege/user boundary smoke; inspect native proof scope and actual status')


if __name__ == '__main__':
    try:
        main()
    except (AssertionError, OSError, ValueError, subprocess.TimeoutExpired) as error:
        print(f'native camera proof failed: {error}', file=sys.stderr)
        sys.exit(1)
