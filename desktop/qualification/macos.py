#!/usr/bin/env python3
"""Hosted-only bundle inspection and real read-only permanent-AppKit qualification.

No device commands, preference writes, autostart, permission grants, synthetic
Flutter snapshots, global input injection, or release/deploy operations. The
normal sandboxed release and compile-time-instrumented runner are separate
prefixes. Both run the real production AppKit helper in isolated development.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import plistlib
import re
import shutil
import signal
import stat
import subprocess
import threading
import tempfile
import time

REPO = Path(__file__).resolve().parents[2]
MAGICS = {bytes.fromhex(value) for value in ('feedface', 'cefaedfe', 'feedfacf', 'cffaedfe',
                                           'cafebabe', 'bebafeca', 'cafebabf', 'bfbafeca')}


def require(value, message):
    if not value:
        raise RuntimeError(message)


def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def save(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + '\n')


def command(argv, log, *, check=True, timeout=120, env=None):
    result = subprocess.run([str(arg) for arg in argv], capture_output=True, timeout=timeout,
                            env=env, cwd=REPO)
    log.parent.mkdir(parents=True, exist_ok=True)
    log.with_suffix('.stdout').write_bytes(result.stdout)
    log.with_suffix('.stderr').write_bytes(result.stderr)
    save(log.with_suffix('.command.json'), dict(argv=[str(arg) for arg in argv], exit=result.returncode))
    if check:
        require(result.returncode == 0, 'Command failed; inspect ' + str(log))
    return result


def prepare_prefix(root, app):
    root.mkdir(mode=0o700)
    desktop = root / 'desktop'
    desktop.mkdir(mode=0o700)
    shutil.copy2(REPO / 'target/release/mcw', root / 'mcw')
    shutil.copytree(app, desktop / 'MicCamWatch.app', symlinks=True)
    # Only our copies: remove group/other write, never alter ancestor ACLs or SDK.
    for path in [root, *root.rglob('*')]:
        if not path.is_symlink():
            path.chmod(path.stat().st_mode & ~0o022)
    files = []
    for path in sorted(root.rglob('*')):
        if path.is_symlink():
            require(path.resolve().is_relative_to(root), 'Escaping bundle symlink')
            files.append(dict(path=str(path.relative_to(root)), symlink=os.readlink(path)))
        elif path.is_file():
            files.append(dict(path=str(path.relative_to(root)), sha256=sha(path), bytes=path.stat().st_size))
    save(root / 'qualification-manifest.json', dict(feature='flutter-desktop',
        mode='read_only_development', architecture=platform.machine(), files=files))
    return root


def inspect(prefix, proof):
    app = prefix / 'desktop/MicCamWatch.app'
    executable = app / 'Contents/MacOS/MicCamWatch'
    info = plistlib.loads((app / 'Contents/Info.plist').read_bytes())
    require(info['CFBundleExecutable'] == 'MicCamWatch', 'Wrong bundle executable')
    require(info['CFBundleIdentifier'] == 'dev.miccamwatch.desktop', 'Unexpected development identity')
    require(info['LSMinimumSystemVersion'] == '15.0', 'Native/Flutter minimum must intersect at macOS15')
    require((app / 'Contents/Resources/AppIcon.icns').is_file(), 'Official compiled app icon missing')
    required = ['Contents/Frameworks/FlutterMacOS.framework/FlutterMacOS',
                'Contents/Frameworks/App.framework/App']
    for relative in required:
        require((app / relative).is_file(), 'Framework payload missing: ' + relative)
    command(['/usr/bin/codesign', '--verify', '--deep', '--strict', '--verbose=4', app], proof / 'seal')
    command(['/usr/bin/codesign', '-dvvv', app], proof / 'signing-state')
    entitlements = command(['/usr/bin/codesign', '-d', '--entitlements', ':-', app], proof / 'entitlements')
    actual_entitlements = plistlib.loads(entitlements.stdout)
    require(actual_entitlements == {'com.apple.security.app-sandbox': True},
            'Release sandbox entitlements changed; do not relax to pass IPC')
    command(['/bin/ls', '-leR', app], proof / 'bundle-acls')
    rows = []
    for index, path in enumerate(sorted(prefix.rglob('*'))):
        if not path.is_file() or path.is_symlink():
            continue
        with path.open('rb') as stream:
            magic = stream.read(4)
        if magic not in MAGICS:
            continue
        arch = command(['/usr/bin/lipo', '-archs', path], proof / ('macho-%03d-arch' % index))
        require(platform.machine() in arch.stdout.decode().split(), 'Mach-O missing native architecture')
        build = command(['/usr/bin/xcrun', 'vtool', '-show-build', path],
                        proof / ('macho-%03d-build' % index))
        if path in (executable, prefix / 'mcw'):
            minimums = re.findall(r'\bminos\s+(\d+(?:\.\d+)+)', build.stdout.decode())
            require(bool(minimums) and all(value in ('15.0', '15.0.0') for value in minimums),
                    'Native/runner deployment target is not macOS15')
        rows.append(dict(path=str(path.relative_to(prefix)), architectures=arch.stdout.decode().strip(),
                         sha256=sha(path), load_commands=build.stdout.decode()))
    require(len(rows) >= 4, 'Complete Mach-O payload not inspected')
    save(proof / 'bundle.json', dict(info=info, entitlements=actual_entitlements, macho=rows,
         signing_claim='Actual codesign output attached; no Developer ID/notarization claim'))


def wait_for(predicate, message, timeout=25):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        result = predicate()
        if result:
            return result
        time.sleep(.1)
    raise RuntimeError(message)


def descendants(pid):
    result = subprocess.run(['/bin/ps', '-axo', 'pid=,ppid=,comm='], capture_output=True, text=True, check=True)
    return [(int(row[0]), row[2]) for line in result.stdout.splitlines()
            if len(row := line.strip().split(None, 2)) == 3 and int(row[1]) == pid]


def alive(pid):
    result = subprocess.run(['/bin/ps', '-p', str(pid), '-o', 'stat='], capture_output=True, text=True)
    return result.returncode == 0 and bool(result.stdout.strip()) and not result.stdout.strip().startswith('Z')


def diagnose_reopen(prefix, owner, child, proof, timeout=8):
    """Read-only owned-child sample; no debugger entitlement or permission grant."""
    expected = str((prefix / 'desktop/MicCamWatch.app/Contents/MacOS/MicCamWatch').resolve())
    record = dict(child=child, owner=owner.pid, status='not-sampled')
    if owner.poll() is not None or (child, expected) not in descendants(owner.pid):
        record['reason'] = 'Owned live relationship no longer present'
        return record
    before = command(['/bin/ps', '-p', str(child), '-o', 'lstart=,pid=,ppid=,stat=,comm='],
                     proof / 'reopen-process-before', check=False)
    try:
        result = command(['/usr/bin/sample', str(child), '1', '1', '-file', proof / 'reopen-sample.txt'],
                         proof / 'reopen-sample', check=False, timeout=timeout)
        after = command(['/bin/ps', '-p', str(child), '-o', 'lstart=,pid=,ppid=,stat=,comm='],
                        proof / 'reopen-process-after', check=False)
        # State can change legitimately; compare creation/identity, not stat.
        def identity(value):
            rows = value.stdout.decode(errors='replace').strip().split(None, 8)
            return rows[:7], rows[8:] if len(rows) >= 9 else []
        valid = (owner.poll() is None and (child, expected) in descendants(owner.pid)
                 and before.returncode == after.returncode == 0
                 and identity(before) == identity(after))
        record.update(status='sampled' if valid and result.returncode == 0 else 'unverified',
                      exit=result.returncode, identity_consistent=valid)
        if not valid:
            (proof / 'reopen-sample.txt').unlink(missing_ok=True)
    except (subprocess.SubprocessError, OSError) as error:
        record.update(status='diagnostic-failed', error=str(error))
    return record


def process_identity(pid, parent, expected):
    """Read-only live identity; every signal is preceded by this owned relation."""
    expected = str(Path(expected).resolve())
    require((pid, expected) in descendants(parent), 'Owned process path/parent changed')
    result = subprocess.run(['/bin/ps', '-p', str(pid), '-o', 'lstart=,pid=,ppid=,stat=,comm='],
                            capture_output=True, text=True, check=True)
    fields = result.stdout.strip().split(None, 8)
    require(len(fields) == 9 and int(fields[5]) == pid and int(fields[6]) == parent
            and fields[8] == expected and not fields[7].startswith('Z'), 'Process identity unavailable')
    return dict(pid=pid, parent=parent, started=' '.join(fields[:5]), executable=expected)


def helper_identity(pid, owner, temporary, proof):
    matches = [(child, path) for child, path in descendants(owner)
               if child == pid and path.endswith('/MicCamWatchHelper.app/Contents/MacOS/MicCamWatchHelper')]
    require(len(matches) == 1, 'Ready native helper is not an actual owned production helper')
    path = Path(matches[0][1])
    require(path.is_absolute() and path.resolve() == path
            and path.is_relative_to(temporary), 'Native helper escaped the private development TMPDIR')
    root = path.parents[3]
    require(root.parent == temporary and root.name.startswith('mcw-' + str(owner) + '-'),
            'Unexpected native helper installation root')
    for item in [root, path.parents[2], path.parents[1], path.parent, path]:
        info = item.lstat()
        require(not stat.S_ISLNK(info.st_mode) and info.st_uid == os.getuid()
                and info.st_mode & 0o077 == 0, 'Native helper path is not owned/private')
    require(stat.S_ISREG(path.lstat().st_mode) and path.lstat().st_nlink == 1,
            'Native helper must be a private single-link executable')
    command(['/usr/bin/codesign', '--verify', '--deep', '--strict', path.parents[2]], proof / 'native-helper-seal')
    return dict(**process_identity(pid, owner, path), sha256=sha(path),
                private_installation_root=str(root), protocol=4, registration='real-AppKit-ready')


def smoke(prefix, proof, session, instrumented, bootstrap, cycles):
    proof.mkdir(parents=True)
    receipt = dict(status='running', mode='real-Rust-read-only-permanent-AppKit-development',
                   architecture=platform.machine(), session=session, capture=[], cases=[],
                   cycles=[], instrumented=instrumented, requested_cycles=cycles if instrumented else 0,
                   ui_transport='Flutter supervisor', native_protocol=4)
    receipt['bootstrap_controls'] = bool(bootstrap)
    if bootstrap:
        receipt['scope'] = 'Controlled diagnostic reproduction, not unconditioned Intel qualification'
    save(proof / 'receipt.json', receipt)
    owner = None
    child = None
    native = None
    native_identity = None
    log = None
    reader = None
    timeline = []
    control_jobs = []
    try:
        private_base = Path.home() / 'mcw-qualification'
        private_base.mkdir(mode=0o700, exist_ok=True)
        metadata = private_base.lstat()
        require(stat.S_ISDIR(metadata.st_mode) and metadata.st_uid == os.getuid()
                and metadata.st_mode & 0o077 == 0, 'Unsafe private qualification base')
        private_tree = Path(tempfile.mkdtemp(prefix='mac-b4-', dir=private_base)).resolve()
        receipt['private_tree'] = str(private_tree)
        config, data, temporary = (private_tree / name for name in ('config', 'data', 'tmp'))
        for path in (config, data, temporary):
            path.mkdir(mode=0o700)
        env = dict(os.environ, XDG_CONFIG_HOME=str(config), XDG_DATA_HOME=str(data),
                   TMPDIR=str(temporary) + '/', MCW_MACOS_QUALIFICATION_TRAY='1',
                   MCW_DESKTOP_QUALIFICATION='1' if instrumented else '0')
        env.pop('MCW_LINUX_QUALIFICATION_TRAY', None)
        env.pop('MCW_LINUX_QUALIFICATION_NATIVE', None)
        receipt['development_paths'] = dict(config=str(config), data=str(data),
                                            temporary=str(temporary), engine=str(prefix / 'mcw'))
        def controls(phase, synchronous=False):
            output = proof / ('bootstrap-' + phase)
            argv = ['python3', '-B', REPO / 'desktop/qualification/intel_bootstrap/controls.py',
                    '--hosted-disposable', '--phase', phase, '--session-probe', bootstrap[0],
                    '--appkit-probe', bootstrap[1], '--working-directory',
                    prefix / 'desktop/MicCamWatch.app/Contents/MacOS', '--output', output]
            if owner is not None:
                argv += ['--owner-pid', owner.pid]
            if child is not None:
                argv += ['--child-pid', child]
            argv = [str(value) for value in argv]
            if synchronous:
                try:
                    result = command(argv, proof / ('control-' + phase), check=False, timeout=20, env=env)
                    receipt.setdefault('control_commands', []).append(dict(phase=phase, exit=result.returncode))
                except (subprocess.SubprocessError, OSError) as error:
                    receipt.setdefault('control_commands', []).append(dict(phase=phase, error=str(error)))
            else:
                stream = (proof / ('control-' + phase + '.log')).open('wb')
                try:
                    job = subprocess.Popen(argv, stdout=stream, stderr=subprocess.STDOUT, env=env, cwd=prefix)
                except BaseException:
                    stream.close()
                    raise
                control_jobs.append((phase, job, stream))
        if bootstrap:
            controls('before-open', synchronous=True)
        log_path = proof / 'owner.log'
        log = log_path.open('wb')
        argv = [str(prefix / 'mcw'), '--read-only-development', '--open', '--desktop-theme', 'light']
        owner = subprocess.Popen(argv, stdout=subprocess.PIPE if bootstrap else log,
                                 stderr=subprocess.STDOUT, env=env, cwd=prefix)
        if bootstrap:
            def drain():
                for line in iter(owner.stdout.readline, b''):
                    timeline.append(dict(monotonic=time.monotonic(), unix_time=time.time(),
                                         line=line.decode(errors='replace').rstrip()))
                    log.write(line)
                    log.flush()
            reader = threading.Thread(target=drain, daemon=True)
            reader.start()
        def text():
            return log_path.read_text(errors='replace')
        native_ready = wait_for(lambda: re.search(
            r'MCW_MACOS_NATIVE event=ready owner=' + str(owner.pid) + r' pid=(\d+) protocol=4 mutations=disabled',
            text()), 'Actual AppKit native host never became ready', timeout=20)
        native = int(native_ready.group(1))
        native_identity = helper_identity(native, owner.pid, temporary, proof)
        receipt['native_host'] = native_identity
        receipt['owner'] = dict(pid=owner.pid, executable=str(prefix / 'mcw'),
                                sha256=sha(prefix / 'mcw'))
        save(proof / 'receipt.json', receipt)
        def native_still_ready():
            require(owner.poll() is None, 'Rust owner exited while native host was required')
            current = process_identity(native, owner.pid, native_identity['executable'])
            require(all(current[key] == native_identity[key] for key in current),
                    'Permanent AppKit helper identity changed')
            require(sha(Path(current['executable'])) == native_identity['sha256'],
                    'Live native helper executable changed')
            return current
        def owned_child():
            require(owner.poll() is None, 'Rust owner exited before Flutter startup')
            children = [(pid, name) for pid, name in descendants(owner.pid) if name.endswith('/MicCamWatch')]
            require(len(children) <= 1, 'Duplicate owned Flutter process')
            return children[0][0] if children else None
        child = wait_for(owned_child, 'No owned Flutter child')
        wait_for(lambda: re.search(r'desktop event=ready .*pid=' + str(child) + r'\b.*sequence=[1-9]\d*', text()),
                 'No positive-sequence Ready received by real Rust owner')
        expected_flutter = prefix / 'desktop/MicCamWatch.app/Contents/MacOS/MicCamWatch'
        receipt['initial_ui_child'] = process_identity(child, owner.pid, expected_flutter)
        native_still_ready()
        receipt['cases'].append(dict(case='sandbox-inherited-stdio-positive-snapshot', status='passed',
                                     owner=owner.pid, child=child))
        if instrumented:
            def probe_registration(phase):
                native_still_ready()
                marker = 'qualification registration ready pid=' + str(native) + ' protocol=4 visible=true'
                before = text().count(marker)
                os.kill(native, signal.SIGUSR1)
                wait_for(lambda: text().count(marker) > before,
                         'Actual AppKit menu registration unavailable during ' + phase, timeout=10)
                return dict(phase=phase, native_pid=native, protocol=4, visible=True,
                            method='owned native signal; live NSStatusItem.isVisible/button check; no UI open')
            receipt['native_registration_initial'] = probe_registration('initial')
            def state():
                rows = [json.loads(line.split(' ', 1)[1]) for line in text().splitlines()
                        if line.startswith('MCW_QUALIFICATION_STATE ')]
                return rows[-1] if rows else None
            visible = wait_for(lambda: (row if (row := state()) and row['pid'] == child and row['visible'] else None),
                               'Owned NSWindow never became visible')
            require(visible['title'] == 'MicCamWatch' and visible['width'] >= 780, 'Wrong native window')
            if session['screen_capture_previously_authorized']:
                capture = proof / 'owned-window.png'
                result = command(['/usr/sbin/screencapture', '-x', '-o', '-l', str(visible['window']), capture],
                                 proof / 'capture', check=False)
                captured = result.returncode == 0 and capture.is_file()
                receipt['capture'].append(dict(status='captured' if captured else 'failed',
                    method='screencapture-owned-window-id', window=visible['window']))
                require(captured, 'Authorized owned-window capture failed; see capture logs')
            else:
                receipt['capture'].append(dict(status='not-executed', reason='No preexisting Screen Recording authorization; no prompt/grant attempted'))
            for index in range(3):
                before = text().count('MCW_QUALIFICATION_ACTIVATED pid=' + str(child) + ' ')
                peer = command(argv, proof / ('activation-%d' % index), env=env)
                require(peer.returncode == 0, 'Native singleton activation failed')
                wait_for(lambda: text().count('MCW_QUALIFICATION_ACTIVATED pid=' + str(child) + ' ') > before,
                         'Owned native SIGUSR1 activation was not handled')
                wait_for(lambda: (row := state()) and row['pid'] == child and row['key'] and row['visible'],
                         'Owned native activation did not produce a visible key window')
                require(owned_child() == child, 'Activation replaced or duplicated owned child')
            receipt['cases'].append(dict(case='native-singleton-activation-three-times', status='passed', child=child))
            for cycle in range(cycles):
                before_close = process_identity(child, owner.pid, expected_flutter)
                native_before = native_still_ready()
                require(owned_child() == child, 'Owned Flutter child changed before normal close')
                os.kill(child, signal.SIGUSR2)
                wait_for(lambda: not alive(child), 'NSWindow close left Flutter engine resident')
                if bootstrap:
                    controls('after-close-' + str(cycle))
                time.sleep(2)
                require(owned_child() is None, 'Ready-window close automatically reopened Flutter')
                native_after_close = native_still_ready()
                dispatch_before = text().count('action=Status protocol=4')
                registration_after_close = probe_registration('after-close-' + str(cycle + 1))
                require(owned_child() is None, 'Native registration probe opened a Flutter engine')
                registration_before = text().count('qualification registration ready pid=' + str(native))
                command(argv, proof / ('reopen-' + str(cycle)), env=env)
                child = wait_for(owned_child, 'Native activation failed to reopen Flutter')
                receipt['reopened_child_pid'] = child
                reopen_started = time.monotonic()
                diagnosed = False
                def reopened_ready():
                    nonlocal diagnosed
                    ready = re.search(r'desktop event=ready .*pid=' + str(child)
                                      + r'\b.*sequence=[1-9]\d*', text())
                    if not ready and not diagnosed and time.monotonic() - reopen_started >= 1.5:
                        diagnosed = True
                        if bootstrap:
                            controls('reopen-stalled-' + str(cycle))
                        diagnostic = proof / ('reopen-diagnostic-' + str(cycle))
                        diagnostic.mkdir()
                        receipt['reopen_diagnostic'] = diagnose_reopen(prefix, owner, child, diagnostic)
                        save(proof / 'receipt.json', receipt)
                    return ready
                # Product deadline is still15s; the harness allowance observes
                # timeout/teardown and never promotes a late/no-Ready child.
                wait_for(reopened_ready, 'Reopened child did not establish IPC')
                require(text().count('action=Status protocol=4') > dispatch_before,
                        'Reopen bypassed native Status dispatch to the same supervisor')
                require(text().count('qualification registration ready pid=' + str(native)) > registration_before,
                        'Actual native menu registration was not ready on reopen')
                after_open = process_identity(child, owner.pid, expected_flutter)
                require(after_open != before_close, 'Reopen did not create a disposable new Flutter child')
                reopened_window = wait_for(lambda: (row if (row := state()) and row['pid'] == child
                                                    and row['visible'] and not row['miniaturized'] else None),
                                           'New owned Flutter window never became visible')
                require(reopened_window['title'] == 'MicCamWatch' and reopened_window['width'] >= 780,
                        'Reopened owned window has the wrong native identity/geometry')
                receipt['cycles'].append(dict(cycle=cycle + 1, status='passed',
                    closed_ui=before_close, reopened_ui=after_open, native_before=native_before,
                    native_after_close=native_after_close, native_after_reopen=native_still_ready(),
                    reopened_window=reopened_window,
                    registration_after_close=registration_after_close,
                    close='owned NSWindow.performClose; Flutter engine exited',
                    reopen='authenticated singleton -> actual AppKit show -> native Status -> same supervisor'))
                save(proof / 'receipt.json', receipt)
            receipt['cases'].append(dict(case='repeated-disposable-Flutter-permanent-native-host',
                                         status='passed', count=cycles))
            native_still_ready()
            os.kill(native, signal.SIGUSR2)
            require(owner.wait(timeout=15) == 0, 'Native menu Exit did not stop the owned Rust host cleanly')
            wait_for(lambda: not alive(child) and not alive(native),
                     'Native Exit left an owned native/Flutter helper running', timeout=10)
            require(re.search(r'MCW_MACOS_NATIVE event=dispatch .*pid=' + str(native)
                              + r' action=Exit protocol=4', text()), 'Native menu Exit was not dispatched')
            require(re.search(r'MCW_MACOS_NATIVE event=stopped .*pid=' + str(native)
                              + r' protocol=4', text()), 'Native helper did not acknowledge clean shutdown')
            receipt['cases'].append(dict(case='native-menu-Exit-owned-clean-teardown', status='passed',
                stimulus='owned native SIGUSR2 probe dispatches the real enabled menu target/action; no physical click claim'))
        else:
            receipt['cases'].append(dict(case='window-close-activation-capture', status='not-executed',
                reason='Uninstrumented exact release: performed in separately identified instrumented prefix'))
        if not instrumented:
            native_still_ready()
            owner.kill()
            owner.wait(timeout=10)
            wait_for(lambda: not alive(child) and not alive(native),
                     'Owned native/Flutter helper remained after owner death', timeout=10)
            receipt['cases'].append(dict(case='owner-death-with-open-native-and-Flutter-children', status='passed'))
        receipt['status'] = 'passed'
    except Exception as error:
        receipt.update(status='failed', error=str(error))
        raise
    finally:
        if owner is not None and owner.poll() is None:
            owner.kill()
            owner.wait(timeout=10)
        if child is not None and alive(child):
            # Never signal a possibly recycled orphan PID. The independently
            # armed native monitor owns shutdown; disposal owns failed-run residue.
            receipt['cleanup'] = 'Child still observed after owner cleanup; no PID-only orphan kill attempted'
        if native is not None and alive(native):
            receipt['native_cleanup'] = 'Native PID still observed after owner cleanup; no recycled-orphan PID kill attempted'
        for phase, job, stream in control_jobs:
            try:
                code = job.wait(timeout=15)
                receipt.setdefault('control_commands', []).append(dict(phase=phase, exit=code))
            except subprocess.TimeoutExpired:
                job.kill()  # only this retained, unreaped direct diagnostic child
                job.wait(timeout=3)
                receipt.setdefault('control_commands', []).append(dict(phase=phase, forced_controller_stop=True))
            finally:
                stream.close()
        if reader:
            reader.join(timeout=3)
            receipt['timeline_reader_stopped'] = not reader.is_alive()
            save(proof / 'owner-timeline.json', timeline)
        if log:
            log.close()
        save(proof / 'receipt.json', receipt)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--proof', type=Path, required=True)
    parser.add_argument('--release-app', type=Path, required=True)
    parser.add_argument('--instrumented-app', type=Path, required=True)
    parser.add_argument('--cycles', type=int, default=3, help='Normal close/reopen cycles with one permanent native host')
    parser.add_argument('--intel-bootstrap-controls', action='store_true',
                        help='Observational diagnostic reproduction only; not an unconditioned qualification pass')
    args = parser.parse_args()
    require(platform.system() == 'Darwin' and os.environ.get('RUNNER_ENVIRONMENT') == 'github-hosted'
            and os.getuid() != 0, 'Disposable non-root GitHub-hosted Mac only')
    proof = args.proof.resolve()
    require(proof.is_relative_to(Path(os.environ['RUNNER_TEMP']).resolve()), 'Evidence outside runner temp')
    proof.mkdir(parents=True, exist_ok=True)
    require(not args.intel_bootstrap_controls or platform.machine() == 'x86_64',
            'Intel bootstrap controls require a hosted Intel runner')
    require(2 <= args.cycles <= 10, 'Native host qualification requires2..10 close/reopen cycles')
    prefixes = []
    for name, app in (('release', args.release_app), ('instrumented', args.instrumented_app)):
        prefixes.append(prepare_prefix(proof / (name + '-prefix'), app.resolve()))
    for name, prefix in zip(('release', 'instrumented'), prefixes):
        inspect(prefix, proof / (name + '-inspection'))
    helper = proof / 'session-probe'
    command(['xcrun', 'swiftc', '-target', platform.machine() + '-apple-macosx15.0',
             REPO / 'desktop/qualification/macos_session.swift', '-o', helper], proof / 'session-build')
    result = command([helper], proof / 'session')
    session = json.loads(result.stdout)
    save(proof / 'session.json', session)
    if not session['aqua_available']:
        save(proof / 'graphical-receipt.json', dict(status='not-executed', session=session,
            reason='Runner has no on-console Aqua screen; no runtime/menu/window/IPC success claimed'))
        print('::warning::macOS graphical qualification NOT EXECUTED: no Aqua session')
        return
    bootstrap = None
    if args.intel_bootstrap_controls:
        appkit = proof / 'appkit-probe'
        command(['xcrun', 'swiftc', '-target', 'x86_64-apple-macosx15.0',
                 REPO / 'desktop/qualification/intel_bootstrap/appkit_bootstrap.swift', '-o', appkit],
                proof / 'appkit-probe-build')
        bootstrap = (helper, appkit)
    for prefix, instrumented in zip(prefixes, (False, True)):
        smoke(prefix, proof / ('instrumented-runtime' if instrumented else 'release-runtime'),
              session, instrumented, bootstrap if instrumented else None, args.cycles)
    # Real production AppKit declarations/NSMenu/status item; simulated data,
    # not Flutter/hardware evidence. Harness itself preserves capture provenance.
    command(['python3', '-B', REPO / 'installer/tests/native_macos_menu.py',
             '--proof', proof / 'native-menu', '--no-screen-capture'],
            proof / 'native-menu-run', timeout=1200)
    save(proof / 'graphical-receipt.json', dict(
        status='diagnostic-condition-passed' if bootstrap else 'passed', session=session,
        flutter='real Rust owner; real snapshots; sandbox unchanged; separate build identities',
        native_menu='actual permanent production AppKit helper in both development runtimes; separate simulated-input visual fixture explicitly labelled',
        native_menu_capture='whole-display capture disabled; owned-view cache fixtures only',
        exclusions=['physical capture', 'hardware controls', 'TCC consent', 'Developer ID', 'notarization',
                    'installer/update/uninstall', 'suspended-entire-process parent death']))


if __name__ == '__main__':
    main()
