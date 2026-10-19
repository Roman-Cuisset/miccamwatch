#!/usr/bin/env python3
"""Measure real production ksni menus in an isolated XFCE/GTK desktop.

Run on a Linux runner with build/native dependencies already installed:
  MCW_NATIVE_LINUX_ISOLATED=1 dbus-run-session -- xvfb-run -a -s '-screen 0 1280x900x24' \
    python3 -B installer/tests/native_linux_tray_width.py \
    --proof "$RUNNER_TEMP/native-proof/linux-tray-width"

Run profiles independently with the same command plus --profiles-only (python3-gi required).
Both modes refuse non-GitHub-hosted or non-opted-in environments. The ignored Rust
fixtures use production view/menu/actions with no hardware monitor or permission grant.
"""
import argparse
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import tempfile
import time


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def until(function, message, seconds=30):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        value = function()
        if value:
            return value
        time.sleep(.15)
    raise AssertionError(message)


def run(arguments, check=True):
    result = subprocess.run([str(arg) for arg in arguments], capture_output=True, timeout=30)
    if check:
        require(result.returncode == 0, (arguments, result.stdout, result.stderr))
    return result


def native_window(kind, exclude=()):
    tree = run(['xwininfo', '-root', '-tree']).stdout.decode()
    for window in dict.fromkeys(re.findall(r'\b0x[0-9a-f]+\b', tree)):
        if window in exclude:
            continue
        properties = run(['xprop', '-id', window, '_NET_WM_WINDOW_TYPE'], check=False).stdout.decode()
        if kind not in properties:
            continue
        info = run(['xwininfo', '-id', window], check=False).stdout.decode()
        if 'Map State: IsViewable' not in info:
            continue
        values = {}
        for key, pattern in (
            ('x', r'Absolute upper-left X:\s+(-?\d+)'),
            ('y', r'Absolute upper-left Y:\s+(-?\d+)'),
            ('width', r'Width:\s+(\d+)'),
            ('height', r'Height:\s+(\d+)'),
        ):
            match = re.search(pattern, info)
            require(match, ('missing actual X11 geometry', window, info))
            values[key] = int(match.group(1))
        return window, values, info, properties
    return None


def screenshot(proof, name):
    run(['import', '-window', 'root', proof / (name + '.png')])


def qualify_profiles(proof, launch, stop, handles):
    # Gio decodes the real session bus GVariant layout; these are not source-text
    # or fixture-view assertions standing in for a native host/menu.
    from gi.repository import Gio, GLib

    bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)

    def call(destination, path, interface, method, signature, arguments):
        return bus.call_sync(destination, path, interface, method,
                             GLib.Variant(signature, arguments), None,
                             Gio.DBusCallFlags.NONE, 10000, None).unpack()

    def property_value(destination, path, interface, name):
        return call(destination, path, 'org.freedesktop.DBus.Properties',
                    'Get', '(ss)', (interface, name))[0]

    def service_for(pid):
        registered = property_value('org.kde.StatusNotifierWatcher', '/StatusNotifierWatcher',
                                    'org.kde.StatusNotifierWatcher', 'RegisteredStatusNotifierItems')
        for item in registered:
            destination, separator, suffix = item.partition('/')
            try:
                owner_pid = call('org.freedesktop.DBus', '/org/freedesktop/DBus',
                                 'org.freedesktop.DBus', 'GetConnectionUnixProcessID',
                                 '(s)', (destination,))[0]
                if owner_pid == pid:
                    path = '/' + suffix if separator else '/StatusNotifierItem'
                    menu_path = property_value(destination, path, 'org.kde.StatusNotifierItem', 'Menu')
                    return destination, menu_path
            except GLib.Error:
                continue
        return None

    expected_labels = {
        'en': ['Balanced', 'Private', 'Meeting', 'Development'],
        'fr': ['Équilibré', 'Privé', 'Réunion', 'Développement'],
        'de': ['Ausgewogen', 'Privat', 'Besprechung', 'Entwicklung'],
        'es': ['Equilibrado', 'Privado', 'Reunión', 'Desarrollo'],
        'ja': ['バランス', 'プライベート', '会議', '開発'],
        'zh': ['平衡', '隐私', '会议', '开发'],
        'ru': ['Сбалансированный', 'Приватный', 'Встреча', 'Разработка'],
    }
    settings_file = Path(os.environ['XDG_CONFIG_HOME']) / 'MicCamWatch' / 'settings.toml'
    policy_file = Path(os.environ['MCW_NATIVE_LINUX_ROOT']) / 'explicit-policy.toml'
    policy_file.write_text('profile = "strict"\n', encoding='utf-8')
    records = []
    for language, labels in expected_labels.items():
        for mode in ('select', 'restart', 'explicit'):
            case = language + '-' + mode
            case_proof = proof / case
            case_proof.mkdir()
            env = dict(os.environ, MCW_LINUX_MENU_PROOF=str(case_proof),
                       MCW_LINUX_PROFILE_LANGUAGE=language)
            env.pop('MCW_LINUX_EXPLICIT_CONFIG', None)
            if mode == 'explicit':
                env['MCW_LINUX_EXPLICIT_CONFIG'] = str(policy_file)
            before_settings = settings_file.read_bytes() if settings_file.exists() else None
            before_policy = policy_file.read_bytes()
            monitor = launch(case + '-menu-transport', [
                'stdbuf', '-oL', 'dbus-monitor',
                "type='method_call',interface='com.canonical.dbusmenu'"
            ])
            fixture = launch(case + '-fixture', [
                'cargo', 'test', '--lib', '--locked',
                'frontends::tray::linux_width_tests::native_linux_tray_profiles',
                '--', '--ignored', '--exact', '--nocapture'
            ], env)
            until(lambda: (case_proof / 'ready').is_file() or fixture.poll() is not None,
                  'native profile fixture did not register: ' + case, seconds=180)
            require(fixture.poll() is None, 'native profile fixture exited: ' + case)
            pid = int((case_proof / 'ready').read_text())
            # Retain the exact child process and image, not just a recyclable
            # numeric PID or an installed mcw selected by process-name search.
            pid_handle = os.fdopen(os.pidfd_open(pid), 'rb', buffering=0)
            image_handle = (Path('/proc') / str(pid) / 'exe').open('rb')
            handles.extend((pid_handle, image_handle))
            image_path = (Path('/proc') / str(pid) / 'exe').resolve(strict=True)
            image_stat = os.fstat(image_handle.fileno())
            command_line = (Path('/proc') / str(pid) / 'cmdline').read_bytes().split(b'\0')
            require(os.getpgid(pid) == fixture.pid and image_path.name.startswith('miccamwatch-')
                    and b'frontends::tray::linux_width_tests::native_linux_tray_profiles' in command_line,
                    'native tray process is not the exact spawned cargo test child')
            destination, menu_path = until(lambda: service_for(pid), 'fixture PID not registered: ' + case)
            (case_proof / 'custody.json').write_text(json.dumps(
                dict(fixture_pid=pid, cargo_pid=fixture.pid, destination=destination,
                     menu_path=menu_path, image=str(image_path), image_device=image_stat.st_dev,
                     image_inode=image_stat.st_ino), indent=2))

            def layout():
                return call(destination, menu_path, 'com.canonical.dbusmenu',
                            'GetLayout', '(iias)', (0, -1, []))[1]

            def profile_state(selected):
                tree = layout()
                candidates = [row for row in tree[2]
                              if [child[1].get('label') for child in row[2]] == labels]
                require(len(candidates) == 1, ('four localized native profile choices missing', case, tree))
                parent = candidates[0]
                require(parent[1].get('enabled', True) == (mode != 'explicit'),
                        ('incorrect native profile availability', case, parent))
                for index, child in enumerate(parent[2]):
                    properties = child[1]
                    require(properties.get('enabled', True) == (mode != 'explicit'),
                            ('incorrect native choice availability', case, properties))
                    require(properties.get('toggle-type') == 'checkmark',
                            ('profile is not a native checked menu choice', case, properties))
                    if properties.get('toggle-state', 0) != int(index == selected):
                        return None
                return tree, parent

            selected = None if mode == 'explicit' else (0 if language == 'en' and mode == 'select' else 1)
            tree, parent = until(lambda: profile_state(selected), 'native initial checks incorrect: ' + case)
            (case_proof / 'layout-0.json').write_text(json.dumps(tree, ensure_ascii=False, indent=2))
            time.sleep(.5)

            def open_menu():
                run(['xdotool', 'mousemove', '26', '12', 'click', '3'])
                time.sleep(.15)
                popup = native_window('_MENU')
                traffic = (proof / (case + '-menu-transport.stdout')).read_text()
                requests = [line for line in traffic.splitlines() if 'member=GetLayout' in line]
                if popup and any('sender=' + bus.get_unique_name() + ' ' not in line for line in requests):
                    return popup
                run(['xdotool', 'key', 'Escape'])
                return None

            def open_profiles():
                window, geometry, _, _ = until(open_menu, 'actual profile DBusMenu popup missing: ' + case)
                # Navigate only enabled root rows; disabled resource rows cannot
                # alter the index, and no control or autostart action is activated.
                enabled_after = sum(row[1].get('enabled', True) and row[1].get('visible', True)
                                    for row in tree[2][tree[2].index(parent) + 1:])
                run(['xdotool', 'key', 'End', *(['Up'] * enabled_after), 'Right'])
                return until(lambda: native_window('_MENU', exclude=(window,)),
                             'real GTK profile submenu missing: ' + case)

            if mode == 'select':
                for step, target in enumerate((2, 0, 3, 1), 1):
                    _, geometry, _, _ = open_profiles()
                    screenshot(case_proof, 'choices-before-' + str(step))
                    if step % 2:
                        # Four native single-line rows: click the measured
                        # submenu row center, never a desktop/global coordinate.
                        run(['xdotool', 'mousemove',
                             str(geometry['x'] + geometry['width'] // 2),
                             str(geometry['y'] + round(geometry['height'] * (target + .5) / 4)),
                             'click', '1'])
                    else:
                        run(['xdotool', 'key', 'Home', *(['Down'] * target), 'Return'])
                    until(lambda: (case_proof / ('view-' + str(step) + '.json')).is_file(),
                          'arbitrary native profile selection not delivered: ' + case)
                    tree, parent = until(lambda: profile_state(target),
                                         'native active check did not update: ' + case)
                    (case_proof / ('layout-' + str(step) + '.json')).write_text(
                        json.dumps(tree, ensure_ascii=False, indent=2))
                open_profiles()
                screenshot(case_proof, 'choices-final-private')
                run(['xdotool', 'key', 'Escape', 'Escape'])
            elif mode == 'restart':
                open_profiles()
                screenshot(case_proof, 'restart-persisted-private')
                run(['xdotool', 'key', 'Escape', 'Escape'])
            else:
                require((case_proof / 'backend-refused').is_file(),
                        'production backend did not refuse every explicit-config action')
                _, geometry, _, _ = until(open_menu, 'explicit-config popup missing: ' + case)
                screenshot(case_proof, 'explicit-config-disabled')
                # The disabled parent cannot be entered or select any false
                # stored preference. Inspect its real D-Bus children as well.
                run(['xdotool', 'key', 'Escape'])
                require(settings_file.read_bytes() == before_settings,
                        'explicit configuration changed stored profile')
                require(policy_file.read_bytes() == before_policy, 'explicit policy file was modified')
            until(open_menu, 'native profile menu did not reopen for Exit: ' + case)
            run(['xdotool', 'key', 'End', 'Return'])
            require(fixture.wait(timeout=30) == 0, 'native profile fixture failed: ' + case)
            stop(monitor)
            if mode == 'restart':
                require(settings_file.read_bytes() == before_settings,
                        'restarting the tray rewrote persisted settings')
            records.append(dict(language=language, mode=mode, native_labels=labels,
                                initial_selected_index=selected,
                                final_selected_index=None if mode == 'explicit' else 1,
                                selection_order=[2, 0, 3, 1] if mode == 'select' else [],
                                backend_refused=(mode == 'explicit')))
            time.sleep(.3)
    (proof / 'profiles.json').write_text(json.dumps(records, ensure_ascii=False, indent=2) + '\n')
    (proof / 'scope.txt').write_text(
        'Actual production ksni menus registered to an XFCE StatusNotifier host. Four arbitrary '
        'direct selections (Meeting, Balanced, Development, Private), alternating measured GTK mouse '
        'and keyboard input, native checked-state refresh, process restart persistence, all seven '
        'localized label sets, and explicit-policy disabled/no-check state qualified on a private '
        'X11/session bus. All four explicit-policy mutations invoke and are refused by the same '
        'production backend handler. No PlatformMonitor, hardware controls, autostart changes or '
        'notification permissions used; this test-only fixture does not qualify physical resources '
        'or claim end-to-end CLI argument parsing coverage.\n')

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--proof', required=True, type=Path)
    parser.add_argument('--profiles-only', action='store_true',
                        help='qualify direct profiles independently of resource/notification proof')
    args = parser.parse_args()
    require(sys.platform == 'linux' and os.environ.get('GITHUB_ACTIONS') == 'true'
            and os.environ.get('RUNNER_ENVIRONMENT') == 'github-hosted'
            and os.environ.get('MCW_NATIVE_LINUX_ISOLATED') == '1',
            'Native desktop input requires explicitly opted-in GitHub-hosted isolation')
    proof = args.proof.resolve()
    proof.mkdir(parents=True, exist_ok=True)
    require(os.environ.get('DISPLAY') and os.environ.get('DBUS_SESSION_BUS_ADDRESS'),
            'Run under a private dbus-run-session + Xvfb display')
    children = []
    handles = []

    def launch(name, command, environment=None):
        output = (proof / (name + '.stdout')).open('wb')
        errors = (proof / (name + '.stderr')).open('wb')
        handles.extend((output, errors))
        process = subprocess.Popen([str(arg) for arg in command], stdout=output, stderr=errors,
                                   env=environment, start_new_session=True)
        children.append(process)
        return process

    def stop(process):
        if process.poll() is None:
            os.killpg(process.pid, signal.SIGTERM)
            process.wait(timeout=20)

    # Keep the runner's actual Rust toolchain while isolating application state.
    original_home = Path.home()
    os.environ.setdefault('RUSTUP_HOME', str(original_home / '.rustup'))
    os.environ.setdefault('CARGO_HOME', str(original_home / '.cargo'))
    with tempfile.TemporaryDirectory(prefix='mcw-linux-menu-') as temporary:
        home = Path(temporary)
        runtime = home / 'run'
        runtime.mkdir(mode=0o700)
        os.environ.update(HOME=str(home), XDG_CONFIG_HOME=str(home / 'config'),
                          XDG_DATA_HOME=str(home / 'data'), XDG_STATE_HOME=str(home / 'state'),
                          XDG_CACHE_HOME=str(home / 'cache'),
                          XDG_RUNTIME_DIR=str(runtime), MCW_NATIVE_LINUX_ROOT=str(home),
                          PIPEWIRE_REMOTE='mcw-native-no-hardware', PIPEWIRE_RUNTIME_DIR=str(runtime),
                          PULSE_SERVER='unix:' + str(runtime / 'mcw-native-no-hardware'))
        try:
            run(['dbus-update-activation-environment', 'DISPLAY', 'XAUTHORITY', 'HOME',
                 'XDG_CONFIG_HOME', 'XDG_DATA_HOME', 'XDG_STATE_HOME', 'XDG_RUNTIME_DIR', 'XDG_CACHE_HOME',
                 'PIPEWIRE_REMOTE', 'PIPEWIRE_RUNTIME_DIR', 'PULSE_SERVER'])
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
            if args.profiles_only:
                qualify_profiles(proof, launch, stop, handles)
                print('PASS actual Linux direct profile menus; inspect profiles.json and screenshots')
                return
            run(['notify-send', '--expire-time=100', 'Native proof', 'Real XFCE notification daemon'])
            time.sleep(.5)
            screen_info = run(['xdpyinfo']).stdout.decode()
            (proof / 'display.txt').write_text(screen_info)
            screen_match = re.search(r'dimensions:\s+(\d+)x(\d+) pixels', screen_info)
            require(screen_match, 'cannot measure native X display')
            screen_width, screen_height = map(int, screen_match.groups())
            measurements = []
            for case in ('latin', 'cjk'):
                case_proof = proof / case
                case_proof.mkdir()
                env = dict(os.environ, MCW_LINUX_MENU_PROOF=str(case_proof), MCW_LINUX_MENU_CASE=case)
                monitor = launch(case + '-notification-transport', [
                    'stdbuf', '-oL', 'dbus-monitor',
                    "type='method_call',interface='org.freedesktop.Notifications',member='Notify'"
                ])
                menu_monitor = launch(case + '-menu-transport', [
                    'stdbuf', '-oL', 'dbus-monitor',
                    "type='method_call',interface='com.canonical.dbusmenu'"
                ])
                fixture = launch(case + '-fixture', [
                    'cargo', 'test', '--lib', '--locked',
                    'frontends::tray::linux_width_tests::native_linux_tray_width',
                    '--', '--ignored', '--exact', '--nocapture'
                ], env)
                until(lambda: (case_proof / 'ready').is_file() or fixture.poll() is not None,
                      'native fixture did not register', seconds=180)
                require(fixture.poll() is None, 'native fixture exited; inspect fixture stderr/stdout')
                view = json.loads((case_proof / 'view.json').read_text())
                diagnostic = (case_proof / 'diagnostic.txt').read_text()
                require('END-LINUX-DIAGNOSTIC' in diagnostic, 'original diagnostic tail missing')
                for row in view['items']:
                    require((row.get('detail') or row['label']).replace('\r', '\n') in diagnostic,
                            'full original menu row not retained in details')
                # The DBus service registers before the GTK tray button is
                # mapped. Let the isolated host finish its initial paint.
                time.sleep(.5)
                def open_fixture_menu():
                    run(['xdotool', 'mousemove', '26', '12', 'click', '3'])
                    time.sleep(.1)
                    popup = native_window('_MENU')
                    # ksni registration precedes GTK host/widget creation. An
                    # early click opens the panel's own context menu instead.
                    # Accept only a real host request to the fixture's DBusMenu.
                    traffic = (proof / (case + '-menu-transport.stdout')).read_text()
                    if popup and 'member=GetLayout' in traffic:
                        return popup
                    run(['xdotool', 'key', 'Escape'])
                    return None
                window, geometry, info, properties = until(
                    open_fixture_menu, 'actual fixture DBusMenu popup did not appear')
                stop(menu_monitor)
                screenshot(case_proof, 'native-popup')
                (case_proof / 'popup.xwininfo').write_text(info)
                (case_proof / 'popup.xprop').write_text(properties)
                require(0 < geometry['width'] <= screen_width // 2,
                        ('real menu exceeds half the screen', geometry, screen_width))
                require(0 < geometry['height'] < screen_height,
                        ('real menu does not fit the screen vertically', geometry))
                # Details is inserted immediately before the existing Exit command.
                run(['xdotool', 'key', 'End', 'Up', 'Return'])
                until(lambda: (case_proof / 'status-delivered').is_file(),
                      'actual native Details command did not deliver notification')
                notification = until(lambda: native_window('_NET_WM_WINDOW_TYPE_NOTIFICATION'),
                                     'real wrapping XFCE notification did not appear')
                _, notification_geometry, notification_info, _ = notification
                screenshot(case_proof, 'native-details-notification')
                (case_proof / 'notification.xwininfo').write_text(notification_info)
                require(0 < notification_geometry['width'] <= screen_width // 2,
                        ('native notification is wider than half screen', notification_geometry))
                time.sleep(.3)
                stop(monitor)
                transport = (proof / (case + '-notification-transport.stdout')).read_text()
                require(transport.count('END-LINUX-DIAGNOSTIC') == diagnostic.count('END-LINUX-DIAGNOSTIC'),
                        'actual D-Bus body did not retain every full diagnostic tail')
                require(transport.count('BEGIN-LINUX-DIAGNOSTIC') == diagnostic.count('BEGIN-LINUX-DIAGNOSTIC'),
                        'actual D-Bus body did not retain every full diagnostic prefix')
                token = '运行时摄像头错误' if case == 'cjk' else 'runtime camera error_'
                require(transport.count(token) == diagnostic.count(token),
                        'actual D-Bus body did not retain complete repeated Unicode/Latin payload')
                require('&lt;&amp;&gt;' in transport, 'diagnostic markup was not escaped by real notification route')
                run(['xdotool', 'mousemove', '26', '12', 'click', '3'])
                until(lambda: native_window('_MENU'), 'native menu did not reopen for Exit')
                run(['xdotool', 'key', 'End', 'Return'])
                require(fixture.wait(timeout=30) == 0, 'native fixture or actual Exit command failed')
                measurements.append(dict(case=case, screen=dict(width=screen_width, height=screen_height),
                                         popup=geometry, notification=notification_geometry,
                                         items=len(view['items']) + 1, full_transport_retained=True))
                time.sleep(.3)
            (proof / 'geometry.json').write_text(json.dumps(measurements, indent=2) + '\n')
            (proof / 'scope.txt').write_text(
                'Actual production ksni builders registered to a real XFCE StatusNotifier host; GTK popup '
                'and xfce4-notifyd notification windows measured on X11 and screenshotted for Latin/CJK '
                'runtime diagnostics with newlines, tabs and carriage returns. Full originals and escaped '
                'D-Bus notification bodies retained. Notification daemons may limit visible body height; '
                'transport preservation is not a claim that every line is simultaneously visible or copyable. '
                'No notification permissions granted. DBusMenu host fonts/geometry differ on other panels; '
                '48 display cells is not a cross-desktop pixel guarantee. No hardware/permission controls '
                'executed by the test-only fixture.\n')
        finally:
            for process in reversed(children):
                stop(process)
            for handle in handles:
                handle.close()
    print('PASS actual Linux XFCE menus/details; inspect geometry.json and native screenshots')


if __name__ == '__main__':
    main()
