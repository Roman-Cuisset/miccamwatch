"""B5 real Windows UI proof. Local runs only use isolated development modes.

--startup-ci additionally exercises actual startup registration, but requires a
fresh interactive GitHub-hosted machine with no MCW installation/registration.
Never install/uninstall MSI, change device protection or clean by process name.
Instrumented semantic actions are not an external MSAA/UIA accessibility test.
"""
import argparse
import ctypes as C
from ctypes import wintypes as W
import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import tomllib
import subprocess
import threading
import time
import winreg

import psutil
import windows as runtime

REPO = Path(__file__).resolve().parents[2]
RUN_KEY = r'Software\Microsoft\Windows\CurrentVersion\Run'
APPROVED_KEY = r'Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run'
REQUIRED = {'real-native-history-query-and-refresh', 'development-startup-mutation-disabled',
            'notification-preference-changed-and-restored-by-Rust', 'about-rendered',
            'real-native-diagnostics-rendered'}


def require(value, message):
    if not value:
        raise RuntimeError(message)


def sha(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def save(path, value):
    Path(path).write_text(json.dumps(value, indent=2, ensure_ascii=False), encoding='utf-8')


def freeze(path, engine, ui, mode):
    path.mkdir()
    for name in ('mcw.exe', 'mcw-tray.exe'):
        shutil.copy2(engine / name, path / name)
    shutil.copytree(ui, path / 'desktop')
    files = [dict(path=str(p.relative_to(path)), bytes=p.stat().st_size, sha256=sha(p))
             for p in sorted(path.rglob('*')) if p.is_file()]
    save(path / 'qualification-manifest.json', dict(feature='flutter-desktop', mode=mode,
         engine_sha256=sha(path / 'mcw-tray.exe'), cli_sha256=sha(path / 'mcw.exe'),
         ui_sha256=sha(path / 'desktop/miccamwatch_desktop.exe'), files=files))
    return path


def environment(path):
    path.mkdir()
    roaming, local = path / 'roaming', path / 'local'
    for directory in (roaming, local):
        (directory / 'MicCamWatch').mkdir(parents=True)
    settings = roaming / 'MicCamWatch/settings.toml'
    # Explicit test preferences, not fabricated observation/history records.
    settings.write_text('notifications_enabled = false\nsound_enabled = false\nshow_ready = false\n'
                        'history_enabled = true\nprofile = "development"\nmute_on_lock = false\n'
                        'block_camera_on_lock = false\nrestore_on_unlock = true\n', encoding='utf-8')
    return dict(os.environ, APPDATA=str(roaming), LOCALAPPDATA=str(local)), settings


def proof_from(lines, child):
    for row in lines:
        line = row['line']
        if line.startswith('MCW_FLUTTER_UI_PROOF '):
            value = json.loads(line.split(' ', 1)[1])
            if value.get('pid') == child:
                return value
    return None


def capture(host, tool, target):
    if tool is None:
        return dict(status='not-executed', reason='No pinned owned-window capture tool supplied; no PrintWindow/desktop fallback')
    hwnd, pid = host.view()[0]
    process = runtime.kernel_process_state(pid)
    require(process is not None and not process['exited'], 'Capture child exited')
    result = subprocess.run([str(tool), '--hwnd', str(hwnd), '--pid', str(pid),
        '--creation-filetime', str(process['birth']), '--expected-exe', str(host.ui),
        '--content-rect', '300,300,600,320', '--timeout-ms', '5000', '--output', str(target)],
        capture_output=True, timeout=20)
    target.with_suffix('.capture.stdout').write_bytes(result.stdout)
    target.with_suffix('.capture.stderr').write_bytes(result.stderr)
    require(result.returncode == 0 and target.is_file(), 'Owned-window capture failed; no fallback')
    return dict(status='captured', sha256=sha(target), path=target.name,
                method='owned HWND/PID/creation-time/path-verified WGC', visual_review_required=True)


def development(prefix, proof, theme, instrumented, cycles, tool):
    proof.mkdir()
    env, settings = environment(proof / 'scope')
    before = tomllib.loads(settings.read_text(encoding='utf-8'))
    receipt = dict(status='running', theme=theme, instrumented=instrumented, cycles=[])
    host = None
    save(proof / 'receipt.json', receipt)
    try:
        host = runtime.Host(prefix, theme, environment=env, settings_development=instrumented)
        for index in range(cycles):
            row = dict(open=host.open(burst=3))
            child = row['open']['window_pid']
            if instrumented:
                value = runtime.until(lambda: proof_from(host.lines, child), timeout=45)
                require(value.get('status') == 'passed' and value.get('mode') == 'settings_development'
                        and value.get('platform') == 'windows' and value.get('theme') == theme
                        and value.get('page') == 'diagnostics' and not value.get('framework_errors')
                        and REQUIRED <= set(value.get('cases', [])), 'Real panel scenario failed: ' + json.dumps(value))
                row['ui_proof'] = value
            if index == 0:
                row['capture'] = capture(host, tool, proof / 'owned-window.bmp')
            tick = max(host.ticks())
            row['close'] = host.close()
            runtime.until(lambda: max(host.ticks()) > tick, timeout=25)
            row['observation_after_close'] = max(host.ticks())
            require(not any(Path(p.exe()).resolve() == host.ui.resolve() for p in host.members()),
                    'Flutter remained resident after normal close')
            receipt['cycles'].append(row)
            save(proof / 'receipt.json', receipt)
        # Exercise owner shutdown while its real release UI is still open.
        if not instrumented:
            receipt['shutdown_open'] = host.open()
        receipt['shutdown'] = host.stop()
        require(tomllib.loads(settings.read_text(encoding='utf-8')) == before,
                'Private original preference values were not restored')
        receipt['private_preferences_unchanged'] = True
        receipt['status'] = 'passed'
    except BaseException as error:
        receipt.update(status='failed', error=type(error).__name__ + ': ' + str(error))
        raise
    finally:
        if host is not None:
            receipt['events'] = host.lines
            if host.proc.poll() is None:
                try:
                    receipt['cleanup'] = host.stop()
                except BaseException as error:
                    receipt['cleanup_error'] = str(error)
        save(proof / 'receipt.json', receipt)


def registry_value(key):
    try:
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, key, 0, winreg.KEY_READ) as handle:
            return winreg.QueryValueEx(handle, 'MicCamWatch')
    except FileNotFoundError:
        return None


def startup_preflight():
    require(os.environ.get('GITHUB_ACTIONS') == 'true'
            and os.environ.get('RUNNER_ENVIRONMENT') == 'github-hosted',
            'Startup writes are forbidden on a local/daily machine')
    require(not (Path(os.environ['LOCALAPPDATA']) / 'Programs/MicCamWatch').exists(),
            'An installed MicCamWatch exists; do not change its startup')
    require(registry_value(RUN_KEY) is None and registry_value(APPROVED_KEY) is None,
            'Existing startup registration/approval must be preserved')
    query = subprocess.run([str(runtime.SYSTEM_QUERY), '/Query', '/TN', 'MicCamWatch Tray', '/XML'],
                           capture_output=True, timeout=15)
    require(query.returncode != 0, 'An existing MicCamWatch task must be preserved')
    for process in psutil.process_iter(['name']):
        require((process.info['name'] or '').lower() not in ('mcw.exe', 'mcw-tray.exe', 'miccamwatch_desktop.exe'),
                'An existing MCW process prevents disposable startup qualification')
    u = runtime.U
    u.GetProcessWindowStation.restype = W.HANDLE
    u.GetUserObjectInformationW.argtypes = [W.HANDLE, C.c_int, C.c_void_p, W.DWORD, C.POINTER(W.DWORD)]
    name, length = C.create_unicode_buffer(256), W.DWORD()
    require(u.GetUserObjectInformationW(u.GetProcessWindowStation(), 2, name, C.sizeof(name), C.byref(length))
            and name.value == 'WinSta0', 'No genuinely interactive window station; do not count GUI as passed')
    return dict(station=name.value, admin_token=bool(C.windll.shell32.IsUserAnAdmin()),
                standard_user_claim=False, scope='disposable hosted user only')


def operational_phase(prefix, proof, env, phase):
    proof.mkdir()
    receipt = dict(status='running', phase=phase)
    lines = []
    owned = None
    reader = None
    try:
        child_env = dict(env, MCW_UI_QUALIFICATION_STARTUP='allow', MCW_UI_QUALIFICATION_PHASE=phase)
        owned = subprocess.Popen([str(prefix / 'mcw-tray.exe'), '--open'], cwd=prefix, env=child_env,
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, encoding='utf-8', errors='replace',
            creationflags=subprocess.CREATE_NO_WINDOW)
        owner = psutil.Process(owned.pid)
        def drain():
            for line in owned.stdout:
                lines.append(dict(line=line.rstrip()))
        reader = threading.Thread(target=drain, daemon=True)
        reader.start()
        def report():
            require(owned.poll() is None, 'Operational tray exited before native startup confirmation')
            matches = [p for p in owner.children() if Path(p.exe()).resolve() ==
                       (prefix / 'desktop/miccamwatch_desktop.exe').resolve()]
            require(len(matches) <= 1, 'Concurrent Flutter instances')
            return proof_from(lines, matches[0].pid) if matches else None
        value = runtime.until(report, timeout=60)
        require(value.get('status') == 'passed' and value.get('mode') == 'operational'
                and value.get('scenario') == 'startup', 'Native startup phase failed: ' + json.dumps(value))
        receipt['ui_proof'] = value
        ui_pid = value['pid']
        generation = runtime.kernel_process_state(ui_pid)
        require(generation is not None and not generation['exited'], 'Proof UI already exited')
        receipt['ui_generation'] = dict(pid=ui_pid, **generation)
        # The paired, pinned CLI stops only the authenticated owned operational scope.
        require(sha(prefix / 'mcw.exe') == json.loads((prefix / 'qualification-manifest.json').read_text())['cli_sha256'],
                'Paired CLI changed')
        stop = subprocess.run([str(prefix / 'mcw.exe'), 'tray', 'stop'], cwd=prefix, env=child_env,
                              capture_output=True, timeout=25)
        receipt['stop'] = dict(code=stop.returncode, stdout=stop.stdout.decode(errors='replace'),
                               stderr=stop.stderr.decode(errors='replace'))
        require(stop.returncode == 0 and owned.wait(timeout=25) == 0, 'Owned operational shutdown failed')
        runtime.until(lambda: not runtime.same_process_alive(ui_pid, generation['birth']), timeout=20)
        receipt['owned_flutter_exited'] = True
        reader.join(timeout=3)
        receipt['status'] = 'passed'
    except BaseException as error:
        receipt.update(status='failed', error=type(error).__name__ + ': ' + str(error))
        raise
    finally:
        if owned is not None and owned.poll() is None:
            # Only this retained Popen process handle, never a PID/name lookup.
            owned.kill()
            owned.wait(timeout=10)
            receipt['cleanup'] = 'failed owned CI parent terminated by retained process handle; not a clean-shutdown pass'
        if reader is not None:
            reader.join(timeout=3)
        receipt['events'] = lines
        save(proof / 'receipt.json', receipt)


def seeded_registry_phase(prefix, proof, env, phase, key, value, kind):
    require(registry_value(key) is None, 'Never overwrite an existing registration or approval')
    with winreg.CreateKeyEx(winreg.HKEY_CURRENT_USER, key, 0, winreg.KEY_SET_VALUE) as handle:
        winreg.SetValueEx(handle, 'MicCamWatch', 0, kind, value)
    expected = (value, kind)
    try:
        operational_phase(prefix, proof, env, phase)
        require(registry_value(key) == expected, 'Native command changed a foreign or user-disabled seed')
    finally:
        # Remove only the exact disposable fixture, never a different/new value.
        require(registry_value(key) == expected, 'Seed changed; preserve evidence instead of overwriting it')
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, key, 0, winreg.KEY_SET_VALUE) as handle:
            winreg.DeleteValue(handle, 'MicCamWatch')


def startup(prefix, proof):
    proof.mkdir()
    baseline = startup_preflight()
    save(proof / 'preflight.json', baseline)
    env, settings = environment(proof / 'scope')
    before = settings.read_bytes()
    operational_phase(prefix, proof / 'enable', env, 'enable')
    # A completely new native owner and UI, same installation/data/registration.
    operational_phase(prefix, proof / 'disable-after-owner-restart', env, 'disable')
    foreign = '"' + str(prefix / 'mcw-tray.exe') + '" --version'
    seeded_registry_phase(prefix, proof / 'foreign-registration-refused', env,
                          'foreign_refusal', RUN_KEY, foreign, winreg.REG_SZ)
    lock = prefix / '.mcw-install.lock'
    require(lock.is_file(), 'Native installation lock was not created')
    original_stat, lock_hash = lock.stat(), sha(lock)
    lock.chmod(stat.S_IREAD)
    try:
        operational_phase(prefix, proof / 'write-permission-refused', env, 'write_failure')
        after = lock.stat()
        require((after.st_dev, after.st_ino) == (original_stat.st_dev, original_stat.st_ino)
                and sha(lock) == lock_hash, 'Failed write changed the permanent lock identity/content')
    finally:
        lock.chmod(original_stat.st_mode)
    # This actual Windows opt-out must not be bypassed by creating a logon task.
    disabled = bytes([3]) + bytes(11)
    seeded_registry_phase(prefix, proof / 'windows-user-optout-preserved', env,
                          'user_disabled', APPROVED_KEY, disabled, winreg.REG_BINARY)
    require(registry_value(RUN_KEY) is None, 'Run registration left after native disable')
    query = subprocess.run([str(runtime.SYSTEM_QUERY), '/Query', '/TN', 'MicCamWatch Tray', '/XML'],
                           capture_output=True, timeout=15)
    require(query.returncode != 0, 'Task left after native disable')
    require(settings.read_bytes() == before, 'Startup lifecycle changed preferences')
    save(proof / 'summary.json', dict(status='passed', activation=True, deactivation=True,
         owner_restart_persistence=True, foreign_registration_preserved=True,
         write_access_denial_confirmed=True, windows_user_optout_preserved=True,
         preferences_preserved=True, preflight=baseline,
         limitations=['Hosted token is reported; not automatically a standard-user test',
                      'No device protection or installed-package lifecycle exercised']))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--engine-directory', type=Path, required=True)
    parser.add_argument('--release-bundle', type=Path, required=True)
    parser.add_argument('--instrumented-bundle', type=Path, required=True)
    parser.add_argument('--startup-bundle', type=Path)
    parser.add_argument('--proof', type=Path, required=True)
    parser.add_argument('--cycles', type=int, default=3)
    parser.add_argument('--capture-tool', type=Path)
    parser.add_argument('--startup-ci', action='store_true')
    args = parser.parse_args()
    allowed = (Path(os.environ['LOCALAPPDATA']) / 'MicCamWatchQualification').resolve()
    proof = args.proof.resolve()
    require(proof != allowed and proof.is_relative_to(allowed), 'Evidence must be in the private qualification tree')
    require(2 <= args.cycles <= 10, 'Two to ten real cycles required')
    proof.mkdir(parents=True)
    ordinary = freeze(proof / 'normal-prefix', args.engine_directory.resolve(), args.release_bundle.resolve(), 'read_only_development')
    panels = freeze(proof / 'panel-prefix', args.engine_directory.resolve(), args.instrumented_bundle.resolve(), 'settings_development')
    for theme in ('light', 'dark'):
        development(ordinary, proof / ('normal-' + theme), theme, False, args.cycles, args.capture_tool)
        development(panels, proof / ('panels-' + theme), theme, True, args.cycles, args.capture_tool)
    if args.startup_ci:
        require(args.startup_bundle is not None, 'Separate startup proof build required')
        operational = freeze(proof / 'startup-prefix', args.engine_directory.resolve(), args.startup_bundle.resolve(), 'disposable_operational_ci')
        startup(operational, proof / 'startup')
    save(proof / 'summary.json', dict(status='passed', cycles_per_theme=args.cycles,
         real_panels=True, themes=['light', 'dark'], startup_executed=args.startup_ci,
         device_mutations=False, stable_replaced=False, external_accessibility_qualified=False))


if __name__ == '__main__':
    main()
