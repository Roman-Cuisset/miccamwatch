#!/usr/bin/env python3
"""Real DMG/Finder/Exit, relocation, whole-app replacement and removal smoke.

Run only on a disposable macOS 15+ ordinary-user runner. open --env isolates
Finder launches. LaunchAgent scenarios additionally require a GitHub-hosted
runner and absent login-account product state, because launchd does not inherit
the invoking CLI's private HOME. Never overwrite existing login-account state.
Aqua and preexisting Accessibility are prerequisites for native Exit and hidden
background-window proof; absence is unavailable/partial (exit 77), never PASS.
Does not grant TCC, remove quarantine, bypass Gatekeeper or control capture devices.
"""
import argparse
import ctypes
import fcntl
import hashlib
import json
import os
from pathlib import Path
import platform
import plistlib
import pwd
import select
import stat
import subprocess
import shutil
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
    require(os.getuid() != 0 and os.geteuid() == os.getuid(),
            'run smoke as an ordinary user, without root/sudo/setuid')
    proof = args.proof.resolve()
    proof.mkdir(parents=True, exist_ok=True)
    report = dict(status='failed', device_control_tested=False, physical_capture_tested=False,
                  developer_id_or_notarization_asserted=False,
                  cross_version_app_upgrade_tested=False,
                  replacement_scope='same-build whole-bundle replacement',
                  os_authorization_granted=False)

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
        directory = tempfile.mkdtemp(prefix='mcw-app-', dir='/tmp')
        work = Path(directory).resolve()
        report['sandbox'] = str(work)
        cleanup_safe = True
        try:
            mount = work / 'mount'
            mount.mkdir()
            attached = False
            cli = None
            env = None
            login_env = None
            login_roots = {}
            login_ancestors = {}
            login_preserved = {}
            owned_autostart = False
            service = 'gui/' + str(os.getuid()) + '/com.roman-cuisset.miccamwatch.tray'
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
                sentinel_before = (sentinel.read_bytes(), stat.S_IMODE(sentinel.stat().st_mode),
                                   sentinel.stat().st_ino)
                def seed_state(prefix, state_env):
                    paths = {}
                    for name, command in (('settings', ['config', 'settings-path']),
                                          ('history', ['history', 'path'])):
                        path = Path(record(prefix + '-' + name + '-path',
                                    [cli, *command], env=state_env).stdout.decode().strip())
                        require(path.is_absolute() and path.name ==
                                ('settings.toml' if name == 'settings' else 'history.jsonl'),
                                'CLI returned an unexpected persistent state path')
                        paths[name] = path
                    return paths

                def write_state(paths):
                    settings = (b'notifications_enabled = false\nsound_enabled = false\n'
                                b'show_ready = false\nhistory_enabled = false\nprofile = "development"\n'
                                b'mute_on_lock = false\nblock_camera_on_lock = false\n'
                                b'restore_on_unlock = false\n')
                    # Genuine history schema, seeded rather than an observation.
                    history = (json.dumps(dict(
                        schema_version=3, event_code=1001,
                        tool_version=plist['CFBundleShortVersionString'],
                        action='start', observed_at='2026-01-01T00:00:00Z',
                        resource='microphone', activity='ready', risk='expected',
                        confidence='low', enforcement='alert',
                        application='retained lifecycle history fixture (not live capture)',
                        pid=None, executable=None, device=None, started_at=None, evidence=[]
                    ), separators=(',', ':')) + '\n').encode()
                    for name, contents in (('settings', settings), ('history', history)):
                        path = paths[name]
                        path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
                        with path.open('xb') as handle:
                            handle.write(contents)
                        path.chmod(0o600)
                        report.setdefault('persistent_fixtures', []).append(dict(
                            path=str(path), sha256=hashlib.sha256(contents).hexdigest(),
                            mode=stat.S_IMODE(path.stat().st_mode), inode=path.stat().st_ino,
                            seeded_history_not_capture_evidence=name == 'history'))
                    return {path: (path.read_bytes(), stat.S_IMODE(path.stat().st_mode),
                                   path.stat().st_ino) for path in paths.values()}

                preserved = write_state(seed_state('private', env))

                def unchanged_state(stage):
                    for path, expected in {**preserved, **login_preserved}.items():
                        require((path.read_bytes(), stat.S_IMODE(path.stat().st_mode),
                                 path.stat().st_ino) == expected,
                                f'{stage}: preferences/history changed: {path}')
                    require((sentinel.read_bytes(), stat.S_IMODE(sentinel.stat().st_mode),
                             sentinel.stat().st_ino) == sentinel_before,
                            f'{stage}: unrelated sentinel bytes/permissions/inode changed')
                    report.setdefault('state_preservation', []).append(stage)

                require(record('settings-loaded-notifications',
                               [cli, 'notifications', 'status'], env=env).stdout.strip() == b'disabled',
                        'product did not load safe notification preferences')
                before_app, before_home = snapshot(app), snapshot(home)
                refusal = record('bundle-update-refusal', [cli, '--lang', 'en', 'update'], env=env, codes=None)
                require(refusal.returncode != 0, 'binary-only bundle update was not refused')
                require(snapshot(app) == before_app and snapshot(home) == before_home,
                        'refused bundle update changed app/state/permissions/xattrs')
                report.update(bundle_update_refusal='passed', mount_copy_seal_version_icon='passed',
                              version=plist['CFBundleShortVersionString'])
                driver = work / 'app-smoke'
                record('compile-native-driver', ['xcrun', 'swiftc', '-O',
                       repo / 'installer/macos/app_smoke.swift', '-framework', 'AppKit',
                       '-framework', 'ApplicationServices', '-o', driver])
                barrier = work / 'launch-race.dylib'
                record('compile-launch-barrier', ['xcrun', 'clang', '-dynamiclib', '-O2',
                       '-Wall', '-Wextra', '-Werror', repo / 'installer/tests/macos_launch_race.c',
                       '-o', barrier])
                preflight = record('gui-prerequisites', [driver, 'gui'], codes=(0, 77))
                if preflight.returncode == 77:
                    report.update(status='unavailable', gui=preflight.stderr.decode().strip())
                    save()
                    print('UNAVAILABLE: DMG/update checks passed; real GUI lifecycle not exercised. See app-smoke.json.')
                    return 77

                def running(state_env=env):
                    return subprocess.run([str(cli), 'tray', 'status'], env=state_env,
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

                accessibility = record('accessibility-prerequisite', [driver, 'accessibility'], codes=(0, 77))
                native_exit = accessibility.returncode == 0
                report['native_menu_exit'] = 'pending' if native_exit else 'unavailable'
                if not native_exit:
                    report['accessibility'] = accessibility.stderr.decode().strip()

                def tray_processes():
                    launcher = str(app / 'Contents/MacOS/MicCamWatch')
                    require(not any(path == launcher for _, _, path in processes()),
                            'LaunchServices wrapper remained resident instead of retiring for ordinary Finder reopen')
                    owners = [pid for pid, _, path in processes() if path == str(cli)]
                    require(len(owners) == 1, f'exactly one sandbox Rust tray owner required: {owners}')
                    owner = owners[0]
                    helpers = []
                    for pid, parent, path in processes():
                        if parent != owner or Path(path).name != 'MicCamWatchHelper':
                            continue
                        mode = subprocess.run(['/bin/ps', '-p', str(pid), '-o', 'args='],
                                              capture_output=True, text=True)
                        if mode.returncode == 0 and mode.stdout.rstrip().endswith(' desktop'):
                            helpers.append(pid)
                    require(len(helpers) == 1, 'persistent AppKit desktop helper child not found')
                    return owner, helpers[0]

                def finder_command(name):
                    launch = ['/usr/bin/open', '-a', app,
                              '--stdout', proof / (name + '-app.stdout'),
                              '--stderr', proof / (name + '-app.stderr')]
                    for key in ('HOME', 'XDG_CONFIG_HOME', 'XDG_DATA_HOME', 'XDG_STATE_HOME',
                                'XDG_RUNTIME_DIR', 'LANG', 'LC_ALL'):
                        launch.extend(['--env', key + '=' + env[key]])
                    return launch

                def finder_open(name, duplicate=False):
                    launch = finder_command(name)
                    record(name, launch, env=env)
                    until(running, name + ': app never reached native readiness')
                    until(lambda: not any(path == str(app / 'Contents/MacOS/MicCamWatch')
                                          for _, _, path in processes()),
                          name + ': native launcher did not retire')
                    record(name + '-readiness', [cli, 'tray', 'status'], env=env)
                    owner, helper = tray_processes()
                    record(name + '-native-identity', [driver, 'identity', helper])
                    if native_exit:
                        record(name + '-native-consultation', [driver, 'window', helper])
                    if duplicate:
                        if native_exit:
                            record(name + '-native-window-close', [driver, 'close-window', helper])
                            require(running() and tray_processes() == (owner, helper),
                                    'closing consultation changed tray/control lifetime')
                        record('duplicate-finder-open', launch, env=env)
                        time.sleep(3)
                        require(running() and tray_processes() == (owner, helper),
                                'duplicate start replaced original owner/helper or left a resident wrapper')
                        report['duplicate_preserved_owner'] = 'passed'
                        if native_exit:
                            record(name + '-ordinary-window-reopen', [driver, 'window', helper])
                            record(name + '-ordinary-window-close', [driver, 'close-window', helper])
                        forced = [launch[0], '-n', *launch[1:]]
                        record('forced-duplicate-finder-open', forced, env=env)
                        time.sleep(3)
                        require(running() and tray_processes() == (owner, helper),
                                'forced duplicate launch replaced original frontend or left a wrapper')
                        report['ordinary_finder_reopen_preserved_owner'] = 'passed'
                        if native_exit:
                            record(name + '-native-window-reopen', [driver, 'window', helper])
                            report['native_consultation_close_singleton_reopen'] = 'passed'
                        else:
                            report['native_consultation_close_singleton_reopen'] = 'unavailable: existing Accessibility authorization required'
                    return owner, helper

                def quit_app(name, state_env=env):
                    owner, helper = tray_processes()
                    result_key = 'native_menu_exit' if native_exit else 'authenticated_ipc_quit'
                    report[result_key] = 'pending'
                    if native_exit:
                        record(name + '-native-exit', [driver, 'quit', helper])
                    else:
                        record(name + '-authenticated-ipc-quit', [cli, 'tray', 'stop'], env=state_env)
                    until(lambda: not running(state_env), name + ': Quit left readiness active')
                    until(lambda: not any(pid in (owner, helper) for pid, _, _ in processes()),
                          name + ': Quit left product processes resident')
                    report[result_key] = 'passed'
                    unchanged_state(name)

                def inspect_background(name):
                    owner, helper = tray_processes()
                    record(name + '-native-identity', [driver, 'identity', helper])
                    if native_exit:
                        record(name + '-hidden-window', [driver, 'hidden-window', helper])
                        report.setdefault('background_service_hidden_window', []).append(name)
                    else:
                        report['background_service_hidden_window_unavailable'] = (
                            'existing Accessibility authorization required')
                    return owner, helper

                registration = home / 'Library/LaunchAgents/com.roman-cuisset.miccamwatch.tray.plist'
                receipt = home / 'config/MicCamWatch/autostart/autostart.json'

                # Hold a genuine background owner after flock but before bind,
                # then deliver the real LaunchServices activation in that gap.
                # Only the owner is interposed; AppKit and Finder stay native.
                cold_socket = next(path for path in preserved if path.name == 'history.jsonl').parent / 'tray-runtime/control.sock'
                ready_read, ready_write = os.pipe()
                release_read, release_write = os.pipe()
                cold_env = dict(env, DYLD_INSERT_LIBRARIES=str(barrier),
                                MCW_COLD_SOCKET=str(cold_socket),
                                MCW_COLD_READY_FD=str(ready_write),
                                MCW_COLD_RELEASE_FD=str(release_read))
                with (proof / 'cold-background.stdout').open('wb') as output, (
                        proof / 'cold-background.stderr').open('wb') as error:
                    active = subprocess.Popen([str(cli), 'tray', 'run'], env=cold_env,
                                              stdout=output, stderr=error,
                                              pass_fds=(ready_write, release_read))
                    os.close(ready_write)
                    os.close(release_read)
                    released = False
                    try:
                        require(select.select([ready_read], [], [], 30)[0] and
                                os.read(ready_read, 1) == b'L',
                                'background owner never reached the real lease-before-bind barrier')
                        require(not cold_socket.exists() and active.poll() is None,
                                'cold barrier did not retain a live owner before socket creation')
                        with cold_socket.with_suffix('.lock').open('r+b') as lease:
                            try:
                                fcntl.flock(lease, fcntl.LOCK_EX | fcntl.LOCK_NB)
                            except BlockingIOError:
                                pass
                            else:
                                raise AssertionError('background owner did not actually hold the native lease')
                        record('cold-finder-open', finder_command('cold-finder-open'), env=env)
                        time.sleep(.5)
                        require(active.poll() is None and not cold_socket.exists(),
                                'background owner escaped its pre-bind barrier')
                        require(len([pid for pid, _, path in processes() if path == str(cli)]) == 2,
                                'explicit Finder open was lost instead of waiting for the cold owner')
                        os.write(release_write, b'R')
                        released = True
                        until(lambda: len([pid for pid, _, path in processes()
                                           if path == str(cli)]) == 1 and
                              not any(path == str(app / 'Contents/MacOS/MicCamWatch')
                                      for _, _, path in processes()),
                              'cold open did not retire its forwarding client and launcher')
                        until(running, 'cold background owner never reached native readiness')
                        owner, helper = tray_processes()
                        require(owner == active.pid, 'cold explicit launch replaced the background owner')
                        record('cold-native-identity', [driver, 'identity', helper])
                        if native_exit:
                            record('cold-native-window', [driver, 'window', helper])
                            report['cold_background_finder_open'] = 'passed'
                        else:
                            report['cold_background_finder_open'] = (
                                'owner retained; native window unavailable without existing Accessibility')
                        quit_app('cold')
                    finally:
                        if not released:
                            try:
                                os.write(release_write, b'R')
                            except BrokenPipeError:
                                pass
                        os.close(release_write)
                        os.close(ready_read)
                        until(lambda: active.poll() is not None or running(),
                              'cold owner neither exited nor exposed authenticated cleanup')
                        if active.poll() is None:
                            record('cold-cleanup-stop', [cli, 'tray', 'stop'], env=env)
                        active.wait(timeout=45)

                finder_open('finder-open', duplicate=True)
                require(not registration.exists(), 'app launch installed autostart registration')
                record('autostart-not-enabled', [cli, 'autostart', 'status'], env=env, codes=(1,))
                quit_app('initial')
                require(not registration.exists(), 'app launch enabled autostart')
                require(snapshot(app) == before_app, 'app launch/quit changed bundle bytes or metadata')
                report.update(finder_open_native_readiness='passed', autostart_not_enabled=True)

                # Ordinary POSIX refusal, not TCC/Gatekeeper or an administrator prompt.
                blocked = work / 'Permission Refusal'
                blocked.mkdir(mode=0o700)
                blocked.chmod(0o500)
                try:
                    denied = record('permission-refusal', ['/usr/bin/ditto',
                                    mount / 'MicCamWatch.app', blocked / 'MicCamWatch.app'], codes=None)
                    require(denied.returncode != 0 and not (blocked / 'MicCamWatch.app').exists(),
                            'ordinary-user unwritable destination did not refuse installation')
                finally:
                    blocked.chmod(0o700)
                record('permission-recovery-copy', ['/usr/bin/ditto', mount / 'MicCamWatch.app',
                                                   blocked / 'MicCamWatch.app'])
                record('permission-recovery-seal', ['codesign', '--verify', '--deep', '--strict',
                                                   blocked / 'MicCamWatch.app'])
                shutil.rmtree(blocked / 'MicCamWatch.app')
                unchanged_state('permission-refusal-and-recovery')
                report['ordinary_user_filesystem_permission_refusal_recovery'] = 'passed'

                def replace_bundle(name):
                    require(not any(path == str(cli) for _, _, path in processes()),
                            'whole-bundle replacement requires Quit first')
                    staged = work / (name + '.app')
                    record(name + '-fresh-dmg-copy', ['/usr/bin/ditto', mount / 'MicCamWatch.app', staged])
                    record(name + '-fresh-seal', ['codesign', '--verify', '--deep', '--strict', staged])
                    require((staged / 'Contents/MacOS/mcw').read_bytes() == cli.read_bytes(),
                            'same-build replacement payload unexpectedly differs')
                    retired = work / (name + '-retired.app')
                    app.rename(retired)
                    staged.rename(app)
                    record(name + '-installed-seal', ['codesign', '--verify', '--deep', '--strict', app])
                    shutil.rmtree(retired)
                    unchanged_state(name)
                    report.setdefault('whole_bundle_replacements', []).append(name)

                replace_bundle('manual-replacement-no-opt-in')
                finder_open('replacement-finder-reopen')
                quit_app('replacement-reopen')
                require(not registration.exists(), 'manual replacement/reopen implicitly opted into autostart')

                # The native service ignores the invoking CLI's HOME. The sole
                # permitted real-account fixture is an empty GitHub-hosted login.
                launchagent_ready = (os.environ.get('GITHUB_ACTIONS') == 'true' and
                                     os.environ.get('RUNNER_ENVIRONMENT') == 'github-hosted')
                unavailable = 'requires explicitly disposable GitHub-hosted login account'
                if launchagent_ready:
                    existing = record('launchagent-before', ['launchctl', 'print', service], codes=(0, 3, 113))
                    disabled = record('launchagent-disabled-before',
                                      ['launchctl', 'print-disabled', 'gui/' + str(os.getuid())])
                    disabled_lines = disabled.stdout.decode().splitlines()
                    override = [line.strip() for line in disabled_lines
                                if line.strip().startswith('"com.roman-cuisset.miccamwatch.tray" => ')]
                    launchagent_ready = existing.returncode != 0 and all(
                        line.rstrip(',;').endswith('false') for line in override)
                    unavailable = 'preexisting fixed-label service or disabled/unknown intent; preserved untouched'
                if launchagent_ready:
                    login_env = dict(os.environ, HOME=pwd.getpwuid(os.getuid()).pw_dir)
                    for key in ('HOME', 'XDG_CONFIG_HOME', 'XDG_DATA_HOME',
                                'XDG_STATE_HOME', 'XDG_RUNTIME_DIR'):
                        value = record('login-launchd-env-' + key, ['launchctl', 'getenv', key],
                                       codes=(0, 1)).stdout.decode().strip()
                        if value:
                            login_env[key] = value
                        elif key != 'HOME':
                            login_env.pop(key, None)
                    login_paths = seed_state('login', login_env)
                    roots = set(path.parent for path in login_paths.values())
                    require(not any(a != b and a in b.parents for a in roots for b in roots),
                            'overlapping login state roots are unsupported; preserving them')
                    launchagent_ready = all(not path.exists() and not path.is_symlink() for path in roots)
                    unavailable = 'preexisting login settings/history parent; preserved untouched'
                if launchagent_ready:
                    for root in roots:
                        missing = []
                        ancestor = root
                        while not ancestor.exists():
                            require(not ancestor.is_symlink(), 'login state ancestor is a symlink')
                            missing.append(ancestor)
                            ancestor = ancestor.parent
                        require(ancestor.resolve() == ancestor,
                                'login state has a redirected ancestor; preserved')
                        for created in reversed(missing):
                            created.mkdir(mode=0o700)
                            identity = created.stat().st_dev, created.stat().st_ino
                            if created == root:
                                login_roots[root] = identity
                            else:
                                login_ancestors[created] = identity
                    login_preserved = write_state(login_paths)
                    # Record intent before invoking: a failing enable may retain
                    # an ownership journal; cleanup must use the genuine CLI.
                    owned_autostart = True
                    record('explicit-autostart-enable', [cli, 'autostart', 'enable'], env=env)
                    record('explicit-autostart-enabled', [cli, 'autostart', 'status'], env=env)
                    until(lambda: running(login_env), 'owned LaunchAgent never reached login readiness')
                    inspect_background('enabled-login-background')
                    quit_app('enabled-login', login_env)
                    enabled_registration = registration.read_bytes(), receipt.read_bytes()
                    replace_bundle('manual-replacement-enabled')
                    record('enabled-refresh', [cli, 'autostart', 'refresh'], env=env)
                    record('enabled-after-replacement', [cli, 'autostart', 'status'], env=env)
                    require((registration.read_bytes(), receipt.read_bytes()) == enabled_registration,
                            'same-path replacement changed explicitly enabled registration/receipt')
                    record('enabled-service-preserved', ['launchctl', 'print', service])

                    # Reopen moved app first: Finder must not silently repair or
                    # enable autostart. Only an explicit CLI refresh may repoint.
                    old_cli = cli
                    moved = work / 'Moved Applications' / 'MicCamWatch.app'
                    moved.parent.mkdir()
                    app.rename(moved)
                    app, cli = moved, moved / 'Contents/MacOS/mcw'
                    finder_open('moved-enabled-finder-reopen')
                    quit_app('moved-enabled-reopen')
                    require((registration.read_bytes(), receipt.read_bytes()) == enabled_registration,
                            'moved Finder reopen silently changed autostart')
                    require(plistlib.loads(registration.read_bytes())['ProgramArguments'][0] == str(old_cli),
                            'registration did not retain the old path before explicit refresh')
                    record('moved-enabled-explicit-refresh', [cli, 'autostart', 'refresh'], env=env)
                    record('moved-enabled-status', [cli, 'autostart', 'status'], env=env)
                    require(plistlib.loads(registration.read_bytes())['ProgramArguments'] ==
                            [str(cli), 'tray', 'run'], 'explicit refresh did not repoint enabled intent')
                    until(lambda: running(login_env), 'repointed enabled service did not reach readiness')
                    inspect_background('moved-enabled-login-background')
                    quit_app('moved-enabled-login', login_env)
                    report['enabled_owned_intent_replacement_refresh'] = 'passed'

                    # Dormant means explicitly owned but unloaded, not a forged
                    # receipt and not launchctl disable's persistent user override.
                    record('make-owned-registration-dormant', ['launchctl', 'bootout', service])
                    record('dormant-service-absent', ['launchctl', 'print', service], codes=(3, 113))
                    dormant_registration = registration.read_bytes(), receipt.read_bytes()
                    replace_bundle('manual-replacement-dormant')
                    record('dormant-same-path-refresh', [cli, 'autostart', 'refresh'], env=env)
                    require((registration.read_bytes(), receipt.read_bytes()) == dormant_registration,
                            'replacement changed dormant owned intent')
                    moved = work / 'Moved Again' / 'MicCamWatch.app'
                    moved.parent.mkdir()
                    app.rename(moved)
                    app, cli = moved, moved / 'Contents/MacOS/mcw'
                    finder_open('moved-dormant-finder-reopen')
                    quit_app('moved-dormant-reopen')
                    require((registration.read_bytes(), receipt.read_bytes()) == dormant_registration,
                            'moved reopen implicitly refreshed dormant registration')
                    dormant = record('dormant-status-guidance', [cli, 'autostart', 'status'],
                                     env=env, codes=None)
                    require(dormant.returncode != 0,
                            'dormant stale-path intent did not refuse enabled status')
                    require((registration.read_bytes(), receipt.read_bytes()) == dormant_registration,
                            'dormant status inspection changed the registration/receipt')
                    record('dormant-status-still-unloaded', ['launchctl', 'print', service], codes=(3, 113))
                    require(not running(login_env), 'dormant status inspection implicitly started login tray')
                    record('dormant-explicit-refresh', [cli, 'autostart', 'refresh'], env=env)
                    require(plistlib.loads(registration.read_bytes())['ProgramArguments'] ==
                            [str(cli), 'tray', 'run'], 'refresh did not repoint dormant registration')
                    record('dormant-refresh-still-unloaded', ['launchctl', 'print', service], codes=(3, 113))
                    require(not running(login_env), 'dormant refresh implicitly started login tray')
                    report.update(dormant_owned_intent_replacement_refresh='passed',
                                  relocation_autostart_guidance='Finder reopen preserves old registration; '
                                  'explicit bundled mcw autostart refresh repoints owned intent without '
                                  'enabling a dormant service; enable is a separate explicit repair/opt-in')
                    unchanged_state('owned-enabled-and-dormant-lifecycle')
                    record('remove-owned-autostart', [cli, 'autostart', 'disable'], env=env)
                    require(not registration.exists() and not receipt.exists(),
                            'owned registration/receipt remained after removal')
                    record('removed-service-absent', ['launchctl', 'print', service], codes=(3, 113))
                    owned_autostart = False
                    report['owned_registration_removal'] = 'passed'
                    report['owned_launchagent_lifecycle'] = 'passed'
                else:
                    report['owned_launchagent_lifecycle'] = 'unavailable'
                    report['owned_launchagent_unavailable_reason'] = unavailable
                    moved = work / 'Moved Applications' / 'MicCamWatch.app'
                    moved.parent.mkdir()
                    app.rename(moved)
                    app, cli = moved, moved / 'Contents/MacOS/mcw'
                    finder_open('moved-no-opt-in-finder-reopen')
                    quit_app('moved-no-opt-in-reopen')
                    require(not registration.exists(), 'relocation implicitly enabled autostart')

                report['moved_bundle_reopen'] = 'passed'
                unchanged_state('before-owned-app-removal')
                before_uninstall_app, before_uninstall_home = snapshot(app), snapshot(home)
                uninstall = record('bundle-uninstall-manual-procedure', [cli, 'uninstall'], env=env, codes=(2,))
                require(str(app).encode() in uninstall.stdout + uninstall.stderr,
                        'manual uninstall did not identify this exact app bundle')
                require(app.is_dir() and snapshot(app) == before_uninstall_app and
                        snapshot(home) == before_uninstall_home,
                        'manual uninstall modified the sealed bundle or user state')
                record('bundle-uninstall-preserved-seal', ['codesign', '--verify', '--deep', '--strict', app])
                unchanged_state('manual-bundle-uninstall-refusal')
                report['bundle_uninstall_exact_manual_procedure_and_preservation'] = 'passed'
                require(app.is_relative_to(work) and not app.is_symlink(),
                        'refusing removal outside the authenticated sandbox')
                record('remove-owned-app-seal', ['codesign', '--verify', '--deep', '--strict', app])
                shutil.rmtree(app)
                require(not app.exists(), 'sandbox app removal failed')
                unchanged_state('after-owned-app-and-registration-removal')
                report.update(owned_app_removal='passed',
                              preferences_history_and_unrelated_sentinel_preserved='passed',
                              status='passed' if native_exit and launchagent_ready else 'partial')
            finally:
                # CLI disable reauthenticates the exact owned plist and job.
                cleanup_cli = cli if cli is not None and cli.exists() else mount / 'MicCamWatch.app/Contents/MacOS/mcw'
                try:
                    if owned_autostart:
                        record('cleanup-owned-autostart', [cleanup_cli, 'autostart', 'disable'], env=env)
                        owned_autostart = False
                    if login_roots:
                        record('cleanup-service-absent', ['launchctl', 'print', service], codes=(3, 113))
                        record('cleanup-login-authenticated-stop', [cleanup_cli, 'tray', 'stop'],
                               env=login_env, codes=(0, 1))
                    if env is not None and cleanup_cli.exists():
                        record('sandbox-cleanup-stop', [cleanup_cli, 'tray', 'stop'], env=env, codes=(0, 1))
                    if 'processes' in locals():
                        until(lambda: not any(Path(path).is_relative_to(work) and
                                              Path(path).name in ('mcw', 'MicCamWatchHelper')
                                              for _, _, path in processes()),
                              'owned sandbox processes remain; preserving their files')
                    for root, identity in login_roots.items():
                        require(not root.is_symlink() and
                                (root.stat().st_dev, root.stat().st_ino) == identity,
                                'login fixture root changed; preserving state')
                    for path, expected in login_preserved.items():
                        require((path.read_bytes(), stat.S_IMODE(path.stat().st_mode),
                                 path.stat().st_ino) == expected,
                                'login fixture state changed; preserving it for review')
                    allowed_directories, allowed_locks = set(), set()
                    if login_roots:
                        config = login_paths['settings'].parent
                        data = login_paths['history'].parent
                        allowed_directories.update((config / 'autostart', data / 'tray-runtime',
                                                    data / 'macos-controls'))
                        allowed_locks.update((config / 'autostart/autostart.lock',
                                              data / 'tray-runtime/control.lock',
                                              data / 'macos-controls/camera.lock'))
                    allowed = set(login_preserved) | allowed_directories | allowed_locks
                    for root in login_roots:
                        for path in root.rglob('*'):
                            info = path.lstat()
                            require(path in allowed and not path.is_symlink() and
                                    info.st_uid == os.getuid(),
                                    f'unexpected login fixture entry; preserving state: {path}')
                            if path in allowed_directories:
                                require(stat.S_ISDIR(info.st_mode) and stat.S_IMODE(info.st_mode) == 0o700,
                                        f'owned fixture directory changed; preserving state: {path}')
                            elif path in allowed_locks:
                                require(stat.S_ISREG(info.st_mode) and info.st_nlink == 1 and
                                        info.st_size == 0 and stat.S_IMODE(info.st_mode) == 0o600,
                                        f'owned fixture lock changed; preserving state: {path}')
                    for root in login_roots:
                        shutil.rmtree(root)
                    for ancestor, identity in sorted(login_ancestors.items(),
                                                     key=lambda row: len(row[0].parts), reverse=True):
                        require(not ancestor.is_symlink() and
                                (ancestor.stat().st_dev, ancestor.stat().st_ino) == identity,
                                'created login ancestor changed; preserving it')
                        if not any(ancestor.iterdir()):
                            ancestor.rmdir()
                    report['owned_login_fixture_cleanup'] = 'passed' if login_roots else 'not-created'
                except BaseException as cleanup_error:
                    cleanup_safe = False
                    report['cleanup_error'] = str(cleanup_error)
                    raise
                finally:
                    if attached:
                        record('detach', ['hdiutil', 'detach', mount])
        finally:
            if cleanup_safe and report['status'] in ('passed', 'partial', 'unavailable'):
                shutil.rmtree(work)
                report['sandbox_cleanup'] = 'passed'
            else:
                report['sandbox_cleanup'] = 'retained for conflict/failure inspection'
    except BaseException as error:
        report['status'] = 'failed'
        report['error'] = str(error)
        raise
    finally:
        save()
    print(('PASS' if report['status'] == 'passed' else 'PARTIAL') +
          ': real DMG app lifecycle; see app-smoke.json for exact scope and unavailable proof.')
    return 0 if report['status'] == 'passed' else 77


if __name__ == '__main__':
    raise SystemExit(main())
