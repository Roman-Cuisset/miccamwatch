#!/usr/bin/env python3
"""Hosted-only real Flutter confirmation -> permanent native owner exit.

This is separate from NSWindow close and native-menu Exit qualification. No
hardware command, permission grant, global input, or PID-only orphan signal.
"""
import argparse
import json
import os
from pathlib import Path
import platform
import re
import stat
import subprocess
import tempfile

import macos as native


def qualify(prefix, proof, theme):
    proof.mkdir()
    receipt = dict(status='running', theme=theme, scenario='quit',
                   authority='settings_development; no device/process commands')
    owner = None
    try:
        base = Path.home() / 'mcw-qualification'
        base.mkdir(mode=0o700, exist_ok=True)
        info = base.lstat()
        native.require(stat.S_ISDIR(info.st_mode) and info.st_uid == os.getuid()
                       and info.st_mode & 0o077 == 0, 'Unsafe private qualification base')
        scope = Path(tempfile.mkdtemp(prefix='mac-flutter-quit-', dir=base)).resolve()
        config, data, temporary = (scope / name for name in ('config', 'data', 'tmp'))
        for path in (config, data, temporary):
            path.mkdir(mode=0o700)
        env = dict(os.environ, XDG_CONFIG_HOME=str(config), XDG_DATA_HOME=str(data),
                   TMPDIR=str(temporary) + '/', MCW_MACOS_QUALIFICATION_TRAY='1',
                   MCW_DESKTOP_QUALIFICATION='1')
        env.pop('MCW_LINUX_QUALIFICATION_TRAY', None)
        env.pop('MCW_LINUX_QUALIFICATION_NATIVE', None)
        argv = [str(prefix / 'mcw'), '--settings-development', '--open', '--desktop-theme', theme]
        receipt.update(private_tree=str(scope), engine_sha256=native.sha(prefix / 'mcw'))
        with (proof / 'owner.log').open('wb') as log:
            owner = subprocess.Popen(argv, stdout=log, stderr=subprocess.STDOUT, env=env, cwd=prefix)
            receipt.update(owner_pid=owner.pid, exit_code=owner.wait(timeout=60))
        text = (proof / 'owner.log').read_text(errors='replace')
        native.require(receipt['exit_code'] == 0, 'Owned engine did not exit normally')
        proofs = [json.loads(line.split(' ', 1)[1]) for line in text.splitlines()
                  if line.startswith('MCW_FLUTTER_UI_PROOF ')]
        native.require(len(proofs) == 1, 'Missing or concurrent Flutter quit proof')
        value = proofs[0]
        child = value.get('pid')
        native.require(isinstance(child, int) and child > 1 and child != owner.pid,
                       'Invalid Flutter identity in owned pipe log')
        native.require(value.get('scenario') == 'quit' and value.get('status') == 'quit_prepared'
                       and value.get('platform') == 'macos' and value.get('theme') == theme
                       and value.get('mode') == 'settings_development'
                       and not value.get('framework_errors')
                       and 'private-owner-quit-prepared-without-device-authority' in value.get('cases', []),
                       'Missing rendered explicit confirmation; prepared is not an exit verdict')
        native.require(re.search(r'desktop event=ready .*pid=' + str(child) + r'\b.*sequence=[1-9]\d*', text),
                       'Owned child did not establish native IPC')
        windows = [json.loads(line.split(' ', 1)[1]) for line in text.splitlines()
                   if line.startswith('MCW_QUALIFICATION_STATE ')]
        native.require(any(row.get('pid') == child and row.get('visible')
                           and row.get('title') == 'MicCamWatch' and row.get('width', 0) >= 780
                           for row in windows), 'No actual visible owned NSWindow recorded')
        ready = re.search(r'MCW_MACOS_NATIVE event=ready owner=' + str(owner.pid)
                          + r' pid=(\d+) protocol=4 mutations=disabled', text)
        native.require(ready is not None, 'No real permanent AppKit host')
        native_pid = int(ready.group(1))
        native.require(re.search(r'MCW_MACOS_NATIVE event=stopped owner=' + str(owner.pid)
                                + r' pid=' + str(native_pid) + r' protocol=4', text),
                       'Native helper did not acknowledge normal teardown')
        # Read paths, not a recycled PID. Never kill any process returned by ps.
        rows = subprocess.check_output(['/bin/ps', '-axo', 'pid=,comm='], text=True).splitlines()
        remaining = [line for line in rows if len(fields := line.strip().split(None, 1)) == 2
                     and (fields[1].startswith(str(prefix) + '/') or fields[1].startswith(str(scope) + '/'))]
        native.require(not remaining, 'Owned private-prefix process still present: ' + repr(remaining))
        receipt.update(status='passed', ui_proof=value, native_pid=native_pid,
                       no_private_prefix_process=True, rendered_window=True,
                       exclusions=['hardware controls', 'TCC consent', 'installed packages', 'daily processes'])
    except BaseException as error:
        receipt.update(status='failed', error=str(error))
        raise
    finally:
        if owner is not None and owner.poll() is None:
            owner.kill()  # Only this retained, unreaped, isolated direct child.
            owner.wait(timeout=10)
            receipt['cleanup'] = 'Failed private owner terminated; no PID-only orphan signal'
        native.save(proof / 'receipt.json', receipt)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--app', type=Path, required=True)
    parser.add_argument('--proof', type=Path, required=True)
    parser.add_argument('--session', type=Path, required=True)
    args = parser.parse_args()
    native.require(platform.system() == 'Darwin' and os.environ.get('RUNNER_ENVIRONMENT') == 'github-hosted'
                   and os.getuid() != 0, 'Disposable non-root GitHub-hosted Mac only')
    proof = args.proof.resolve()
    native.require(proof.is_relative_to(Path(os.environ['RUNNER_TEMP']).resolve()), 'Evidence outside runner temp')
    session = json.loads(args.session.read_text())
    native.require(session.get('aqua_available') is True, 'No Aqua session; real Flutter quit not executed')
    proof.mkdir()
    prefix = native.prepare_prefix(proof / 'prefix', args.app.resolve(), 'settings_development')
    native.inspect(prefix, proof / 'inspection')
    for theme in ('light', 'dark'):
        qualify(prefix, proof / theme, theme)
    print('Real Flutter quit confirmation and native owner teardown passed in both themes')


if __name__ == '__main__':
    main()
