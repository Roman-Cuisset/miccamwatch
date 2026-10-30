#!/usr/bin/env python3
"""Hosted-only B3 bundle inspection and real read-only owned lifecycle smoke.

No device commands, preference writes, autostart, permission grants, synthetic
Flutter snapshots, global input injection, or release/deploy operations. The
normal sandboxed release and compile-time-instrumented runner are separate
prefixes. Native/menu visual fixtures are labelled simulated by their harness.
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
import subprocess
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


def smoke(prefix, proof, session, instrumented):
    proof.mkdir(parents=True)
    receipt = dict(status='running', mode='real-Rust-read-only-development', session=session,
                   capture=[], cases=[], instrumented=instrumented)
    save(proof / 'receipt.json', receipt)
    owner = None
    child = None
    log = None
    try:
        config, data = proof / 'config', proof / 'data'
        for path in (config, data):
            path.mkdir(mode=0o700)
        env = dict(os.environ, XDG_CONFIG_HOME=str(config), XDG_DATA_HOME=str(data),
                   MCW_DESKTOP_QUALIFICATION='1' if instrumented else '0')
        log_path = proof / 'owner.log'
        log = log_path.open('wb')
        argv = [str(prefix / 'mcw'), '--read-only-development', '--open', '--desktop-theme', 'light']
        owner = subprocess.Popen(argv, stdout=log, stderr=subprocess.STDOUT, env=env, cwd=prefix)
        def text():
            return log_path.read_text(errors='replace')
        def owned_child():
            require(owner.poll() is None, 'Rust owner exited before Flutter startup')
            children = [(pid, name) for pid, name in descendants(owner.pid) if name.endswith('/MicCamWatch')]
            require(len(children) <= 1, 'Duplicate owned Flutter process')
            return children[0][0] if children else None
        child = wait_for(owned_child, 'No owned Flutter child')
        wait_for(lambda: re.search(r'desktop event=ready .*pid=' + str(child) + r'\b.*sequence=[1-9]\d*', text()),
                 'No positive-sequence Ready received by real Rust owner')
        receipt['cases'].append(dict(case='sandbox-inherited-stdio-positive-snapshot', status='passed',
                                     owner=owner.pid, child=child))
        if instrumented:
            def state():
                rows = [json.loads(line.split(' ', 1)[1]) for line in text().splitlines()
                        if line.startswith('MCW_QUALIFICATION_STATE ')]
                return rows[-1] if rows else None
            visible = wait_for(lambda: (row if (row := state()) and row['visible'] else None),
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
                wait_for(lambda: (row := state()) and row['key'] and row['visible'],
                         'Owned native activation did not produce a visible key window')
                require(owned_child() == child, 'Activation replaced or duplicated owned child')
            receipt['cases'].append(dict(case='native-singleton-activation-three-times', status='passed', child=child))
            os.kill(child, signal.SIGUSR2)
            wait_for(lambda: not alive(child), 'NSWindow close left Flutter engine resident')
            time.sleep(2)
            require(owned_child() is None, 'Ready-window close automatically reopened Flutter')
            receipt['cases'].append(dict(case='normal-owned-window-close-no-resident-engine', status='passed'))
            command(argv, proof / 'reopen', env=env)
            child = wait_for(owned_child, 'Native activation failed to reopen Flutter')
            wait_for(lambda: re.search(r'desktop event=ready .*pid=' + str(child) + r'\b.*sequence=[1-9]\d*', text()),
                     'Reopened child did not establish IPC')
            receipt['cases'].append(dict(case='reopen-new-owned-child', status='passed', child=child))
        else:
            receipt['cases'].append(dict(case='window-close-activation-capture', status='not-executed',
                reason='Uninstrumented exact release: performed in separately identified instrumented prefix'))
        owner.kill()
        owner.wait(timeout=10)
        wait_for(lambda: not alive(child), 'Independent native monitor left orphan after owner death', timeout=10)
        receipt['cases'].append(dict(case='owner-death-with-open-child', status='passed'))
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
        if log:
            log.close()
        save(proof / 'receipt.json', receipt)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--proof', type=Path, required=True)
    parser.add_argument('--release-app', type=Path, required=True)
    parser.add_argument('--instrumented-app', type=Path, required=True)
    args = parser.parse_args()
    require(platform.system() == 'Darwin' and os.environ.get('RUNNER_ENVIRONMENT') == 'github-hosted'
            and os.getuid() != 0, 'Disposable non-root GitHub-hosted Mac only')
    proof = args.proof.resolve()
    require(proof.is_relative_to(Path(os.environ['RUNNER_TEMP']).resolve()), 'Evidence outside runner temp')
    proof.mkdir(parents=True, exist_ok=True)
    prefixes = []
    for name, app in (('release', args.release_app), ('instrumented', args.instrumented_app)):
        prefix = prepare_prefix(proof / (name + '-prefix'), app.resolve())
        inspect(prefix, proof / (name + '-inspection'))
        prefixes.append(prefix)
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
    for prefix, instrumented in zip(prefixes, (False, True)):
        smoke(prefix, proof / ('instrumented-runtime' if instrumented else 'release-runtime'), session, instrumented)
    # Real production AppKit declarations/NSMenu/status item; simulated data,
    # not Flutter/hardware evidence. Harness itself preserves capture provenance.
    command(['python3', '-B', REPO / 'installer/tests/native_macos_menu.py',
             '--proof', proof / 'native-menu', '--no-screen-capture'],
            proof / 'native-menu-run', timeout=1200)
    save(proof / 'graphical-receipt.json', dict(status='passed', session=session,
        flutter='real Rust owner; real snapshots; sandbox unchanged; separate build identities',
        native_menu='production AppKit with explicitly simulated input data',
        native_menu_capture='whole-display capture disabled; owned-view cache fixtures only',
        exclusions=['physical capture', 'hardware controls', 'TCC consent', 'Developer ID', 'notarization',
                    'installer/update/uninstall', 'suspended-entire-process parent death']))


if __name__ == '__main__':
    main()
