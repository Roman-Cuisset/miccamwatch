#!/usr/bin/env python3
"""Qualify the production retained GTK window on an isolated native Linux desktop.

MCW_NATIVE_LINUX_ISOLATED=1 dbus-run-session -- xvfb-run -a -s '-screen 0 1280x1000x24' \
  python3 -B installer/tests/native_linux_desktop.py --proof "$RUNNER_TEMP/native-proof/linux-desktop"

Requires GTK3, python3-gi, Xvfb, openbox, xfce4-panel/xfconf, xdotool,
x11-utils, ImageMagick and fonts-noto-cjk, plus the existing Rust build dependencies.
Runs on Ubuntu 22.04 x64 and ARM64. Uses a private HOME/XDG/session bus/display;
observations are explicitly synthetic. No monitor, device control, elevation,
autostart mutation, installed application or existing PipeWire session is used
unless --launcher-archive explicitly opts into the isolated installed-launcher proof.
"""
import argparse
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile

from native_linux_tray_width import require, run, until


def qualify_installed_launcher(root, proof, archive, version):
    """Launch actual installed desktop files through Gio, never parse Exec as shell."""
    from gi.repository import Gio
    import shutil
    import time
    import platform
    import re
    from native_install import (Sandbox, archive_bytes, digest, download, fixture_environment,
                                local_camera_archive, RELEASE_ROOT)

    proof.mkdir()
    require(tuple(map(int, version.removeprefix('v').split('.'))) >= (0, 21, 0),
            'the native Linux app launcher requires a genuine 0.21.0+ candidate')
    asset, binary, camera = local_camera_archive(archive, root, version)
    shell = shutil.which('bash')
    require(shell, 'bash is required for isolated installer fixture')
    sandbox = Sandbox(root, 'installed-launcher 日本語', 'bash', shell)
    sandbox.prefix = sandbox.home / 'native 日本語 %f \\slash "quote" $literal'
    sandbox.binary = sandbox.prefix / 'bin/mcw'
    for key in ('DISPLAY', 'XAUTHORITY', 'DBUS_SESSION_BUS_ADDRESS', 'XDG_RUNTIME_DIR',
                'PIPEWIRE_REMOTE', 'PIPEWIRE_RUNTIME_DIR', 'PULSE_SERVER', 'GDK_BACKEND'):
        if key in os.environ:
            sandbox.env[key] = os.environ[key]
    package = sandbox.root / asset
    package.write_bytes(archive_bytes(binary, camera=camera))
    sums = sandbox.root / 'SHA256SUMS'
    sums.write_text(f'{digest(package.read_bytes())}  {asset}\n')
    environment, requests = fixture_environment(sandbox, {
        f'{RELEASE_ROOT}/{version}/{asset}': str(package),
        f'{RELEASE_ROOT}/{version}/SHA256SUMS': str(sums),
    })
    installer = Path(__file__).resolve().parents[1] / 'install.sh'
    legacy_source = None
    if platform.machine() == 'x86_64':
        # Genuine immutable 0.20 producer: install the real new candidate just
        # as the old updater does before invoking the new autostart refresh.
        legacy_source = ('https://raw.githubusercontent.com/Roman-Cuisset/miccamwatch/'
                         'a26522028eafe02591f58c03ceb5c367f2d5d60b/installer/install.sh')
        legacy_installer = proof / 'legacy-0.20-install.sh'
        download(shutil.which('curl'), legacy_source, legacy_installer)
        sandbox.invoke(legacy_installer, '--no-modify-path', env=environment, version=version)
        require(not (sandbox.prefix / '.miccamwatch-install/desktop.path').exists(),
                'legacy producer unexpectedly created desktop integration')
    else:
        # No 0.20 Linux ARM installer existed; do not invent a historical upgrade.
        sandbox.invoke(installer, '--no-modify-path', env=environment, version=version)
    data = Path(sandbox.env['XDG_DATA_HOME'])
    desktop = data / 'applications/com.roman-cuisset.miccamwatch.desktop'
    icon = data / 'icons/hicolor/96x96/apps/com.roman-cuisset.miccamwatch.png'

    def command(*arguments, check=True, env=None, trace=None):
        argv = [str(sandbox.binary), *arguments]
        if trace is not None:
            argv = ['strace', '-f', '-e', 'trace=network,execve', '-o', str(trace), *argv]
        result = subprocess.run(argv, env=env or sandbox.env, capture_output=True, timeout=60)
        if check:
            require(result.returncode == 0, (arguments, result.stdout, result.stderr))
        return result

    def windows():
        result = run(['xdotool', 'search', '--onlyvisible', '--name', '^MicCamWatch$'], check=False)
        return result.stdout.split()

    def owners():
        found = []
        for process in Path('/proc').iterdir():
            if process.name.isdigit():
                try:
                    if (process / 'exe').readlink() == sandbox.binary:
                        found.append(int(process.name))
                except (FileNotFoundError, PermissionError, ProcessLookupError):
                    pass
        return found

    context = Gio.AppLaunchContext()
    for key, value in sandbox.env.items():
        context.setenv(key, value)

    def activate(path):
        app = Gio.DesktopAppInfo.new_from_filename(str(path))
        require(app is not None, 'Gio rejected the installed desktop entry')
        require(app.launch([], context), 'Gio failed to activate the installed desktop entry')
        return app
    binary_before = sandbox.binary.stat().st_ino, digest(sandbox.binary.read_bytes())
    camera_before = {name: digest((sandbox.prefix / 'share/miccamwatch/linux-camera' / name).read_bytes())
                     for name in camera}
    requests_before = requests.read_bytes()
    refresh_trace = proof / 'offline-refresh.strace'
    command('autostart', 'refresh', env=environment, trace=refresh_trace)
    trace = refresh_trace.read_bytes()
    require(b'AF_INET' not in trace and not re.search(rb'execve\("[^"\n]*/curl"', trace),
            'local desktop refresh attempted internet networking or a curl subprocess')
    require((sandbox.binary.stat().st_ino, digest(sandbox.binary.read_bytes())) == binary_before
            and all(digest((sandbox.prefix / 'share/miccamwatch/linux-camera' / name).read_bytes()) == value
                    for name, value in camera_before.items())
            and requests.read_bytes() == requests_before and desktop.is_file() and icon.is_file()
            and not (Path(sandbox.env['XDG_CONFIG_HOME']) / 'autostart'
                     / 'com.roman-cuisset.miccamwatch.desktop').exists(),
            'post-upgrade offline refresh downloaded, replaced CLI/payload or enabled startup')
    integration_before = {path: (path.stat().st_ino, path.read_bytes()) for path in
                          (desktop, icon, sandbox.prefix / '.miccamwatch-install/desktop.path',
                           sandbox.prefix / '.miccamwatch-install/desktop.sha256',
                           sandbox.prefix / '.miccamwatch-install/icon.path',
                           sandbox.prefix / '.miccamwatch-install/icon.sha256')}
    command('autostart', 'refresh', env=environment)
    require(all((path.stat().st_ino, path.read_bytes()) == value
                for path, value in integration_before.items())
            and requests.read_bytes() == requests_before,
            'already-current desktop integration was rewritten or used the network')
    unmanaged = Sandbox(root, 'unmanaged-local-copy', 'bash', shell)
    shutil.copy2(sandbox.binary, unmanaged.binary)
    unmanaged_env, unmanaged_requests = fixture_environment(unmanaged, {})
    unmanaged_before = unmanaged.binary.stat().st_ino, digest(unmanaged.binary.read_bytes())
    unmanaged_refresh = subprocess.run([str(unmanaged.binary), 'autostart', 'refresh'],
                                       env=unmanaged_env, capture_output=True, timeout=60)
    require(unmanaged_refresh.returncode == 0
            and not (unmanaged.prefix / '.miccamwatch-install').exists()
            and not (Path(unmanaged.env['XDG_DATA_HOME']) / 'applications'
                     / 'com.roman-cuisset.miccamwatch.desktop').exists()
            and not (Path(unmanaged.env['XDG_CONFIG_HOME']) / 'autostart'
                     / 'com.roman-cuisset.miccamwatch.desktop').exists()
            and not unmanaged_requests.exists()
            and (unmanaged.binary.stat().st_ino, digest(unmanaged.binary.read_bytes())) == unmanaged_before,
            'non-managed refresh adopted ownership, replaced executable or contacted transport')

    entry = Gio.DesktopAppInfo.new_from_filename(str(desktop))
    require(entry is not None and entry.get_display_name() == 'MicCamWatch'
            and entry.should_show() and entry.get_icon().to_string() == str(icon)
            and entry.get_startup_wm_class() == 'MicCamWatch',
            'the installed launcher is hidden, misnamed or lacks the official icon')
    try:
        command('autostart', 'enable')
        startup = Path(sandbox.env['XDG_CONFIG_HOME']) / 'autostart/com.roman-cuisset.miccamwatch.desktop'
        # Both Gio launches are asynchronous; do not wait for owner readiness
        # between cold background startup and explicit application activation.
        activate(startup)
        activate(desktop)
        race_window = until(windows, 'cold background/explicit race lost the window request')
        require(len(race_window) == 1, 'cold background/explicit race duplicated the native window')
        until(lambda: len(owners()) == 1, 'cold background/explicit race duplicated the owner')
        run(['import', '-window', 'root', proof / 'cold-launch-race.png'])
        command('tray', 'stop')
        until(lambda: not owners() and not windows(), 'cold-race owner failed to exit cleanly')
        activate(startup)
        until(lambda: command('tray', 'status', check=False).returncode == 0,
              'the real session desktop file did not start its background owner')
        time.sleep(1)
        require(not windows(), 'background session autostart unexpectedly raised the main window')
        command('autostart', 'refresh')
        require(not windows(), 'refreshing background autostart raised the main window')
        owner = owners()
        require(len(owner) == 1, 'background launch did not establish exactly one installed owner')
        activate(desktop)
        window = until(windows, 'the normal app launcher did not open the retained GTK window')
        require(len(window) == 1, 'explicit activation duplicated the native window')
        until(lambda: owners() == owner, 'explicit activation duplicated the native owner')
        run(['import', '-window', 'root', proof / 'explicit-launch.png'])
        wm_class = run(['xprop', '-id', window[0].decode(), 'WM_CLASS']).stdout
        (proof / 'wm-class.txt').write_bytes(wm_class)
        window_identity = re.findall(rb'"([^"]*)"', wm_class)
        require(window_identity == [b'com.roman-cuisset.miccamwatch', b'MicCamWatch'],
                'real native window identity does not match its conventional launcher')
        activate(desktop)
        time.sleep(.5)
        require(windows() == window and owners() == owner, 'repeat app-menu launch duplicated owner/window')
        run(['xdotool', 'windowclose', window[0].decode()])
        until(lambda: not windows(), 'native close did not hide the retained main window')
        activate(desktop)
        until(lambda: windows() == window, 'app-menu reopen did not reuse the same native window')
        until(lambda: owners() == owner, 'app-menu reopen replaced the retained owner')
        original_entry = desktop.read_bytes()
        prior_requests = requests.read_bytes()
        try:
            desktop.write_bytes(original_entry + b'\n# user customization must be preserved\n')
            for action in ('update', 'uninstall'):
                refused = command(action, check=False, env=environment)
                require(refused.returncode != 0 and sandbox.binary.exists()
                        and windows() == window and owners() == owner
                        and desktop.read_bytes().endswith(b'# user customization must be preserved\n')
                        and requests.read_bytes() == prior_requests,
                        action + ' failed to refuse modified launcher before network/frontend changes')
                (proof / (action + '-modified-launcher.stderr')).write_bytes(refused.stderr)
        finally:
            desktop.write_bytes(original_entry)
        foreign = sandbox.root / 'foreign mcw'
        shutil.copy2(sandbox.binary, foreign)
        refused = subprocess.run([str(foreign), 'tray', 'open'], env=sandbox.env,
                                 capture_output=True, timeout=30)
        require(refused.returncode != 0 and windows() == window and owners() == owner,
                'a foreign executable adopted or disrupted the existing owner')
        (proof / 'foreign-open.stderr').write_bytes(refused.stderr)
        foreign_namespace = dict(sandbox.env, XDG_DATA_HOME=str(sandbox.home / 'foreign data namespace'))
        refused = command('tray', 'open', env=foreign_namespace, check=False)
        require(refused.returncode != 0 and windows() == window and owners() == owner,
                'a different data namespace adopted or disrupted the existing owner')
        (proof / 'foreign-namespace.stderr').write_bytes(refused.stderr)
        result = {'activation': 'real Gio desktop-file launch',
                  'background_owner_pid': owner[0], 'retained_window_id': window[0].decode(),
                  'same_owner_and_window_after_repeat_close_reopen': True,
                  'foreign_image_refused': True, 'modified_launcher_lifecycle_refused_before_shutdown': True,
                  'physical_hardware_validated': False}
        result['legacy_installer_source'] = legacy_source
        result['offline_post_upgrade_refresh'] = 'existing autostart refresh; no CLI/payload replacement, transport or startup enable'
        result['cold_background_plus_explicit_single_owner_window'] = True
        result['wm_class'] = [value.decode() for value in window_identity]
        result['foreign_namespace_refused'] = True
        command('tray', 'stop')
        until(lambda: not owners(), 'owned Exit did not stop the installed native owner')
        command('autostart', 'disable')
        saved = sandbox.saved_user_data()
        # Verify the Rust preflight plus installer handoff, not just shell removal.
        command('uninstall')
        require(not sandbox.binary.exists() and not desktop.exists() and not icon.exists()
                and not startup.exists(), 'native uninstall retained an owned integration asset')
        require(all(path.read_bytes() == content for path, content in saved.items()),
                'native uninstall removed unrelated user data')
        result['uninstall'] = 'real CLI preflight and installer handoff; unrelated data retained'
        (proof / 'results.json').write_text(json.dumps(result, indent=2) + '\n')
    finally:
        if sandbox.binary.exists():
            command('tray', 'stop', check=False)
            command('autostart', 'disable', check=False)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--proof', required=True, type=Path)
    parser.add_argument('--launcher-archive', type=Path,
                        help='genuine native Linux candidate archive; opts into installed Gio launch/removal proof')
    parser.add_argument('--launcher-version', help='matching concrete candidate version, e.g. v0.21.0')
    args = parser.parse_args()
    require(sys.platform == 'linux' and os.environ.get('GITHUB_ACTIONS') == 'true'
            and os.environ.get('RUNNER_ENVIRONMENT') == 'github-hosted'
            and os.environ.get('MCW_NATIVE_LINUX_ISOLATED') == '1',
            'Native input requires explicitly opted-in GitHub-hosted isolation')
    require(os.environ.get('DISPLAY') and os.environ.get('DBUS_SESSION_BUS_ADDRESS'),
            'Run under a private dbus-run-session + Xvfb display')
    require(bool(args.launcher_archive) == bool(args.launcher_version),
            '--launcher-archive and --launcher-version must be supplied together')
    if args.launcher_archive:
        args.launcher_archive = args.launcher_archive.resolve(strict=True)
    proof = args.proof.resolve()
    proof.mkdir(parents=True, exist_ok=True)
    original_home = Path.home()
    os.environ.setdefault('RUSTUP_HOME', str(original_home / '.rustup'))
    os.environ.setdefault('CARGO_HOME', str(original_home / '.cargo'))
    children, handles, results = [], [], []

    def launch(name, command, environment=None):
        output = (proof / (name + '.stdout')).open('wb')
        errors = (proof / (name + '.stderr')).open('wb')
        handles.extend((output, errors))
        process = subprocess.Popen([str(arg) for arg in command], stdout=output, stderr=errors,
                                   env=environment, start_new_session=True)
        children.append(process)
        return process

    with tempfile.TemporaryDirectory(prefix='mcw-linux-desktop-') as temporary:
        root = Path(temporary)
        runtime = root / 'run'
        runtime.mkdir(mode=0o700)
        os.environ.update(HOME=str(root), XDG_CONFIG_HOME=str(root / 'config'),
                          XDG_DATA_HOME=str(root / 'data'), XDG_STATE_HOME=str(root / 'state'),
                          XDG_CACHE_HOME=str(root / 'cache'), XDG_RUNTIME_DIR=str(runtime),
                          MCW_NATIVE_LINUX_ROOT=str(root), PIPEWIRE_REMOTE='mcw-native-no-hardware',
                          PIPEWIRE_RUNTIME_DIR=str(runtime),
                          PULSE_SERVER='unix:' + str(runtime / 'mcw-native-no-hardware'),
                          GDK_BACKEND='x11')
        try:
            run(['dbus-update-activation-environment', 'DISPLAY', 'XAUTHORITY', 'HOME',
                 'XDG_CONFIG_HOME', 'XDG_DATA_HOME', 'XDG_STATE_HOME', 'XDG_RUNTIME_DIR',
                 'XDG_CACHE_HOME', 'PIPEWIRE_REMOTE', 'PIPEWIRE_RUNTIME_DIR', 'PULSE_SERVER'])
            launch('openbox', ['openbox'])
            for arguments in (
                ['-p', '/configver', '-n', '-t', 'int', '-s', '2'],
                ['-p', '/panels', '-n', '-a', '-t', 'int', '-s', '1'],
                ['-p', '/panels/panel-1/position', '-n', '-t', 'string', '-s', 'p=6;x=640;y=20'],
                ['-p', '/panels/panel-1/length', '-n', '-t', 'uint', '-s', '100'],
                ['-p', '/panels/panel-1/plugin-ids', '-n', '-a', '-t', 'int', '-s', '1'],
                ['-p', '/plugins/plugin-1', '-n', '-t', 'string', '-s', 'systray'],
            ):
                run(['xfconf-query', '-c', 'xfce4-panel', *arguments])
            launch('xfce-panel', ['xfce4-panel', '--disable-wm-check'])
            host = ['gdbus', 'call', '--session', '--dest', 'org.kde.StatusNotifierWatcher',
                    '--object-path', '/StatusNotifierWatcher', '--method',
                    'org.freedesktop.DBus.Properties.Get', 'org.kde.StatusNotifierWatcher',
                    'IsStatusNotifierHostRegistered']
            until(lambda: b'true' in run(host, check=False).stdout,
                  'real XFCE StatusNotifier host did not register')
            (proof / 'host.txt').write_bytes(run(host).stdout)
            if args.launcher_archive:
                qualify_installed_launcher(root, proof / 'installed-launcher',
                                           args.launcher_archive, args.launcher_version)
            headless = proof / 'headless'
            headless.mkdir()
            environment = dict(os.environ, DISPLAY='', WAYLAND_DISPLAY='',
                               MCW_LINUX_DESKTOP_HEADLESS='1',
                               MCW_LINUX_DESKTOP_PROOF=str(headless),
                               MCW_LINUX_DESKTOP_LANGUAGE='en')
            fixture = launch('headless', ['cargo', 'test', '--locked', '--lib',
                                         'native_linux_desktop', '--', '--ignored',
                                         '--test-threads=1', '--nocapture'], environment)
            require(fixture.wait(timeout=300) == 0 and (headless / 'passed').is_file(),
                    'headless GTK failure/retry disrupted the real SNI tray')
            for theme, gtk_theme in [('light', 'Adwaita'), ('dark', 'Adwaita:dark'),
                                     ('high-contrast', 'HighContrast')]:
                for language in ('en', 'fr', 'de', 'es', 'ja', 'zh', 'ru'):
                    name = theme + '-' + language
                    case = proof / name
                    case.mkdir()
                    environment = dict(os.environ, GTK_THEME=gtk_theme,
                                       MCW_LINUX_DESKTOP_PROOF=str(case),
                                       MCW_LINUX_DESKTOP_LANGUAGE=language)
                    fixture = launch(name, ['cargo', 'test', '--locked', '--lib',
                                           'native_linux_desktop', '--', '--ignored',
                                           '--test-threads=1', '--nocapture'], environment)
                    require(fixture.wait(timeout=300) == 0, 'native GTK fixture failed: ' + name)
                    require((case / 'passed').is_file(), 'native GTK proof did not finish: ' + name)
                    images = list(case.glob('*.png'))
                    require(len(images) >= 8 and all(image.stat().st_size > 1000 for image in images),
                            'missing or empty native screenshots: ' + name)
                    results.append({'language': language, 'theme': gtk_theme,
                                    'screenshots': [str(image.relative_to(proof)) for image in images],
                                    'status': 'passed', 'physical_hardware_validated': False})
            (proof / 'results.json').write_text(json.dumps(results, indent=2) + '\n')
            (proof / 'scope.txt').write_text(
                'Actual production GTK owner thread, loaded C ABI, retained native widgets and real '
                'XFCE StatusNotifier host. Seven languages in Adwaita light/dark and HighContrast; '
                'native tabs, diagnostics refresh/focus/selection/scroll, desired-boolean preference '
                'persistence/concurrent-field preservation, exact disabled controls, close/hide/reopen '
                'and ordered teardown/recreate. All observation rows are SIMULATED. No hardware '
                'monitor, protection mutation, elevation or autostart mutation is called. Native '
                'x64/ARM execution is software/runtime proof, not physical device certification.\n')
        finally:
            for process in reversed(children):
                if process.poll() is None:
                    os.killpg(process.pid, signal.SIGTERM)
                    process.wait(timeout=20)
            for handle in handles:
                handle.close()
    print('PASS native GTK desktop; inspect results.json, native state JSON and screenshots')


if __name__ == '__main__':
    main()
