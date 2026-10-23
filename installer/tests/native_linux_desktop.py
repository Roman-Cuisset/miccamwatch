#!/usr/bin/env python3
"""Qualify the production retained GTK window on an isolated native Linux desktop.

MCW_NATIVE_LINUX_ISOLATED=1 dbus-run-session -- xvfb-run -a -s '-screen 0 1280x1000x24' \
  python3 -B installer/tests/native_linux_desktop.py --proof "$RUNNER_TEMP/native-proof/linux-desktop"

Requires GTK3, python3-gi, Xvfb, openbox, xfce4-panel/xfconf, xdotool,
x11-utils, ImageMagick and fonts-noto-cjk, plus the existing Rust build dependencies.
Runs on Ubuntu 22.04 x64 and ARM64. Uses a private HOME/XDG/session bus/display;
observations are explicitly synthetic. No monitor, device control, elevation,
autostart mutation, installed application or existing PipeWire session is used.
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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--proof', required=True, type=Path)
    args = parser.parse_args()
    require(sys.platform == 'linux' and os.environ.get('GITHUB_ACTIONS') == 'true'
            and os.environ.get('RUNNER_ENVIRONMENT') == 'github-hosted'
            and os.environ.get('MCW_NATIVE_LINUX_ISOLATED') == '1',
            'Native input requires explicitly opted-in GitHub-hosted isolation')
    require(os.environ.get('DISPLAY') and os.environ.get('DBUS_SESSION_BUS_ADDRESS'),
            'Run under a private dbus-run-session + Xvfb display')
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
