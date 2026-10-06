#!/usr/bin/env python3
"""Measure real production ksni menus in an isolated XFCE/GTK desktop.

Run on a Linux runner with build/native dependencies already installed:
  dbus-run-session -- xvfb-run -a -s '-screen 0 1280x900x24' \
    python3 -B installer/tests/native_linux_tray_width.py \
    --proof "$RUNNER_TEMP/native-proof/linux-tray-width"

The ignored Rust fixture uses the production view/menu/status-notification code.
No production GUI fault-injection switch, fake D-Bus host, or permission grant.
"""
import argparse
import json
import os
from pathlib import Path
import re
import signal
import subprocess
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


def native_window(kind):
    tree = run(['xwininfo', '-root', '-tree']).stdout.decode()
    for window in dict.fromkeys(re.findall(r'\b0x[0-9a-f]+\b', tree)):
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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--proof', required=True, type=Path)
    args = parser.parse_args()
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
                          XDG_RUNTIME_DIR=str(runtime))
        try:
            run(['dbus-update-activation-environment', 'DISPLAY', 'XAUTHORITY', 'HOME',
                 'XDG_CONFIG_HOME', 'XDG_DATA_HOME', 'XDG_STATE_HOME', 'XDG_RUNTIME_DIR'])
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
                require(view['visual'] == 'error', 'degraded fixture was downgraded')
                require(len(view['items']) + 1 <= 16, 'native menu exceeded item limit')
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
