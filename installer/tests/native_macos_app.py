#!/usr/bin/env python3
"""Real DMG mount/copy/update-refusal and Finder launch/native Exit smoke.

Run only on a disposable macOS 15+ runner, never a user's installation. Uses
open --env to isolate the launched app without modifying launchd's environment.
Aqua and preexisting Accessibility for the compiled driver are prerequisites;
absence is recorded as unavailable with exit 77 (not PASS). Does not grant TCC,
remove quarantine, bypass Gatekeeper, enable autostart or control capture devices.
"""
import argparse
import ctypes
import hashlib
import json
import os
from pathlib import Path
import platform
import plistlib
import stat
import subprocess
import tempfile
import time


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def snapshot(root):
    rows = {}
    for path in sorted(root.rglob('*')):
        info = path.lstat()
        attrs = subprocess.check_output(['/usr/bin/xattr', str(path)], text=True).splitlines()
        row = dict(mode=stat.S_IMODE(info.st_mode), uid=info.st_uid, gid=info.st_gid,
                   inode=info.st_ino, xattrs={name: subprocess.check_output(
                       ['/usr/bin/xattr', '-px', name, str(path)], text=True) for name in attrs})
        if path.is_file():
            row['sha256'] = hashlib.sha256(path.read_bytes()).hexdigest()
        rows[str(path.relative_to(root))] = row
    return rows


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--dmg', type=Path, required=True)
    parser.add_argument('--proof', type=Path, required=True)
    parser.add_argument('--disposable-runner', action='store_true', required=True)
    args = parser.parse_args()
    require(platform.system() == 'Darwin', 'native DMG smoke requires macOS')
    require(int(platform.mac_ver()[0].split('.')[0]) >= 15, 'macOS 15+ required')
    proof = args.proof.resolve()
    proof.mkdir(parents=True, exist_ok=True)
    report = dict(status='failed', device_control_tested=False, physical_capture_tested=False,
                  developer_id_or_notarization_asserted=False)

    def record(name, argv, env=None, codes=(0,), timeout=60):
        result = subprocess.run([str(arg) for arg in argv], capture_output=True,
                                env=env, timeout=timeout)
        (proof / (name + '.stdout')).write_bytes(result.stdout)
        (proof / (name + '.stderr')).write_bytes(result.stderr)
        (proof / (name + '.exit')).write_text(str(result.returncode) + '\n')
        require(codes is None or result.returncode in codes,
                f'{name}: exit {result.returncode}: {result.stderr.decode(errors="replace")}')
        return result

    def save():
        (proof / 'app-smoke.json').write_text(json.dumps(report, indent=2) + '\n')

    repo = Path(__file__).resolve().parents[2]
    try:
        with tempfile.TemporaryDirectory(prefix='mcw-app-', dir='/tmp') as directory:
            work = Path(directory).resolve()
            mount = work / 'mount'
            mount.mkdir()
            attached = False
            cli = None
            env = None
            try:
                record('mount', ['hdiutil', 'attach', '-readonly', '-nobrowse', '-noautoopen',
                                 '-mountpoint', mount, args.dmg.resolve()])
                attached = True
                require((mount / 'Applications').is_symlink() and
                        os.readlink(mount / 'Applications') == '/Applications',
                        'DMG must include the Applications link')
                app = work / 'Copied Applications' / 'MicCamWatch.app'
                app.parent.mkdir()
                record('copy', ['/usr/bin/ditto', mount / 'MicCamWatch.app', app])
                record('app-seal', ['codesign', '--verify', '--deep', '--strict', app])
                plist = plistlib.loads((app / 'Contents/Info.plist').read_bytes())
                require(plist['LSUIElement'] is True and plist['LSMinimumSystemVersion'] == '15.0',
                        'app accessory/minimum OS metadata missing')
                require(plist['CFBundleIdentifier'] == 'com.roman-cuisset.miccamwatch',
                        'unexpected product identity')
                require((app / 'Contents/Resources/MicCamWatch.icns').stat().st_size > 0,
                        'Finder icon missing')
                cli = app / 'Contents/MacOS/mcw'
                version = record('version', [cli, '--version']).stdout.decode().strip()
                require(version == 'mcw ' + plist['CFBundleShortVersionString'] and
                        plist['CFBundleVersion'] == plist['CFBundleShortVersionString'],
                        'bundle/Rust version mismatch')
                home = work / 'home'
                home.mkdir(mode=0o700)
                runtime = home / 'run'
                runtime.mkdir(mode=0o700)
                env = dict(os.environ, HOME=str(home), XDG_CONFIG_HOME=str(home / 'config'),
                           XDG_DATA_HOME=str(home / 'data'), XDG_STATE_HOME=str(home / 'state'),
                           XDG_RUNTIME_DIR=str(runtime), LANG='en_US.UTF-8', LC_ALL='en_US.UTF-8')
                sentinel = home / 'unrelated-user-file'
                sentinel.write_bytes(b'preserve user files and permissions\n')
                before_app, before_home = snapshot(app), snapshot(home)
                refusal = record('bundle-update-refusal', [cli, '--lang', 'en', 'update'], env=env, codes=None)
                require(refusal.returncode != 0 and b'replace the whole MicCamWatch.app' in refusal.stderr,
                        'binary-only bundle update did not give whole-app replacement guidance')
                require(snapshot(app) == before_app and snapshot(home) == before_home,
                        'refused bundle update changed app/state/permissions/xattrs')
                report.update(bundle_update_refusal='passed', mount_copy_seal_version_icon='passed',
                              version=plist['CFBundleShortVersionString'])
                driver = work / 'app-smoke'
                record('compile-native-driver', ['xcrun', 'swiftc', '-O',
                       repo / 'installer/macos/app_smoke.swift', '-framework', 'AppKit',
                       '-framework', 'ApplicationServices', '-o', driver])
                preflight = record('gui-prerequisites', [driver, 'gui'], codes=(0, 77))
                if preflight.returncode == 77:
                    report.update(status='unavailable', gui=preflight.stderr.decode().strip())
                    save()
                    print('UNAVAILABLE: DMG/update checks passed; real GUI lifecycle not exercised. See app-smoke.json.')
                    return 77

                def running():
                    return subprocess.run([str(cli), 'tray', 'status'], env=env,
                                          capture_output=True, timeout=10).returncode == 0

                def until(predicate, message):
                    deadline = time.monotonic() + 40
                    while time.monotonic() < deadline:
                        if predicate():
                            return
                        time.sleep(.25)
                    raise AssertionError(message)

                libproc = ctypes.CDLL('/usr/lib/libproc.dylib')
                libproc.proc_pidpath.argtypes = [ctypes.c_int, ctypes.c_void_p, ctypes.c_uint32]
                libproc.proc_pidpath.restype = ctypes.c_int

                def processes():
                    rows = []
                    for line in subprocess.check_output(['/bin/ps', '-axo', 'pid=,ppid='], text=True).splitlines():
                        pid, parent = map(int, line.split())
                        buffer = ctypes.create_string_buffer(4096)
                        if libproc.proc_pidpath(pid, buffer, len(buffer)) > 0:
                            rows.append((pid, parent, os.fsdecode(buffer.value)))
                    return rows

                launch = ['/usr/bin/open', '-n', '-a', app]
                for name in ('HOME', 'XDG_CONFIG_HOME', 'XDG_DATA_HOME', 'XDG_STATE_HOME',
                             'XDG_RUNTIME_DIR', 'LANG', 'LC_ALL'):
                    launch.extend(['--env', name + '=' + env[name]])
                record('finder-open', launch, env=env)
                until(running, 'copied .app never reached native tray readiness')
                record('menu-readiness', [cli, 'tray', 'status'], env=env)
                owners = [pid for pid, _, path in processes() if path == str(cli)]
                require(len(owners) == 1, f'exec launcher must leave exactly one Rust tray owner: {owners}')
                owner = owners[0]
                record('duplicate-finder-open', launch, env=env)
                time.sleep(3)
                require(running(), 'duplicate start stopped original tray')
                require([pid for pid, _, path in processes() if path == str(cli)] == [owner],
                        'duplicate start replaced original owner or left another resident wrapper')
                helpers = []
                for pid, parent, path in processes():
                    if parent != owner or Path(path).name != 'MicCamWatchHelper':
                        continue
                    mode = subprocess.run(['/bin/ps', '-p', str(pid), '-o', 'args='],
                                          capture_output=True, text=True)
                    if mode.returncode == 0 and mode.stdout.rstrip().endswith(' desktop'):
                        helpers.append(pid)
                require(len(helpers) == 1, 'persistent AppKit desktop helper child not found')
                report.update(finder_open_native_readiness='passed', duplicate_preserved_owner='passed')
                registration = home / 'Library/LaunchAgents/com.roman-cuisset.miccamwatch.tray.plist'
                require(not registration.exists(), 'app launch installed autostart registration')
                record('autostart-not-enabled', [cli, 'autostart', 'status'], env=env, codes=(1,))
                report['autostart_not_enabled'] = True
                accessibility = record('accessibility-prerequisite', [driver, 'accessibility'], codes=(0, 77))
                if accessibility.returncode == 77:
                    record('authenticated-ipc-quit', [cli, 'tray', 'stop'], env=env)
                    until(lambda: not running(), 'authenticated Quit did not retire app readiness')
                    until(lambda: not any(pid in (owner, helpers[0]) for pid, _, _ in processes()),
                          'authenticated Quit left product processes resident')
                    report.update(status='partial', native_menu_exit='unavailable', authenticated_ipc_quit='passed',
                                  accessibility=accessibility.stderr.decode().strip())
                    print('PARTIAL: Finder/readiness/duplicate/authenticated Quit passed; native Exit click needs preexisting Accessibility.')
                    return 77
                record('native-exit-menu', [driver, 'quit', helpers[0]])
                until(lambda: not running(), 'real native Exit did not retire tray readiness')
                until(lambda: not any(pid in (owner, helpers[0]) for pid, _, _ in processes()),
                      'real native Exit left product processes resident')
                require(not registration.exists(), 'app launch enabled autostart')
                require(sentinel.read_bytes() == b'preserve user files and permissions\n',
                        'app changed unrelated user file')
                require(snapshot(app) == before_app, 'app launch/quit changed bundle bytes or metadata')
                report.update(status='passed', finder_open_native_readiness='passed',
                              duplicate_preserved_owner='passed', native_menu_exit='passed',
                              autostart_not_enabled=True)
            finally:
                # Stop only this sandbox's authenticated tray socket, never the
                # user's actual tray or another process selected by name.
                if cli is not None and env is not None:
                    record('sandbox-cleanup-stop', [cli, 'tray', 'stop'], env=env, codes=(0, 1))
                if attached:
                    record('detach', ['hdiutil', 'detach', mount])
    except BaseException as error:
        report['error'] = str(error)
        raise
    finally:
        save()
    print('PASS real DMG mount/copy/.app launch/native readiness/duplicate/Exit/update refusal; no capture assertions.')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
