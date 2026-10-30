#!/usr/bin/env python3
"""Read-only B4 Linux inventory; stdout is private, cookie/environment-free JSON.

Stream this file to an already authorized SSH alias with python3 -B - inventory
[--session c3 --display :10.0 --probe-glx]. No remote file is created. GLX is an
optional diagnostic client, never an app launch or proof of Flutter acceleration.
Observe an ALREADY RUNNING owned qualification process with observe --pid PID
--seconds 5. This reuses B3 renderer/measurement helpers and launches nothing.
"""
import argparse
from datetime import datetime, timezone
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess
import sys

sys.dont_write_bytecode = True


def command(argv, env=None):
    try:
        result = subprocess.run(argv, text=True, capture_output=True, timeout=25,
                                env=env or {**os.environ, 'LC_ALL': 'C'})
        return {'returncode': result.returncode, 'stdout': result.stdout,
                'stderr': result.stderr}
    except (OSError, subprocess.TimeoutExpired) as error:
        return {'error': str(error)}


def properties(argv):
    result = command(argv)
    values = dict(line.split('=', 1) for line in result.get('stdout', '').splitlines()
                  if '=' in line)
    return values, result


def metadata(path):
    try:
        info = path.stat()
        return {'path': str(path), 'uid': info.st_uid, 'gid': info.st_gid,
                'mode': oct(stat.S_IMODE(info.st_mode)),
                'readable': os.access(path, os.R_OK),
                'writable': os.access(path, os.W_OK)}
    except OSError as error:
        return {'path': str(path), 'error': str(error)}


def inventory(args):
    if sys.platform != 'linux':
        raise RuntimeError('Run inventory on the authorized Linux host')
    uid = os.getuid()
    result = {'observed_at': datetime.now(timezone.utc).isoformat(),
              'uid': uid, 'gid': os.getgid(), 'groups': os.getgroups(),
              'machine': os.uname().machine,
              'os_release': Path('/etc/os-release').read_text(),
              'sessions': [], 'gpus': [], 'drm_nodes': [],
              'scope': 'Read-only; no cookies/process environments, permission changes, login or app launches'}
    listing = command(['loginctl', 'list-sessions', '--no-legend', '--no-pager'])
    result['session_listing'] = listing
    for line in listing.get('stdout', '').splitlines():
        session = line.split()[0]
        if not re.fullmatch(r'[A-Za-z0-9_-]{1,64}', session):
            continue
        values, receipt = properties(['loginctl', 'show-session', session, '--no-pager',
                                     '-p', 'Id', '-p', 'User', '-p', 'Name', '-p', 'Seat',
                                     '-p', 'Display', '-p', 'Remote', '-p', 'Service',
                                     '-p', 'Leader', '-p', 'Type', '-p', 'Class',
                                     '-p', 'Active', '-p', 'State'])
        result['sessions'].append({'properties': values, 'receipt': receipt})
    for card in sorted(Path('/sys/class/drm').glob('card[0-9]*')):
        if not re.fullmatch(r'card[0-9]+', card.name):
            continue
        device = card / 'device'
        row = {'card': card.name, 'device_path': str(device.resolve())}
        for key in ('vendor', 'device', 'subsystem_vendor', 'subsystem_device', 'boot_vga'):
            try:
                row[key] = (device / key).read_text().strip()
            except OSError:
                pass
        row['driver'] = (device / 'driver').resolve().name if (device / 'driver').exists() else None
        result['gpus'].append(row)
    for path in sorted(Path('/dev/dri').glob('*')):
        if path.is_char_device():
            result['drm_nodes'].append(metadata(path))
    runtime = Path('/run/user') / str(uid)
    result['runtime'] = [metadata(runtime), metadata(runtime / 'bus')]
    result['graphical_processes'] = []
    allowed = {'Xorg', 'Xwayland', 'xfce4-session', 'cinnamon', 'gnome-shell',
               'lightdm', 'xrdp', 'xrdp-sesman', 'kwin_wayland'}
    for proc in Path('/proc').iterdir():
        if not proc.name.isdecimal():
            continue
        try:
            name = (proc / 'comm').read_text().strip()
            if name in allowed:
                result['graphical_processes'].append({'pid': int(proc.name),
                                                     'uid': proc.stat().st_uid, 'name': name})
        except OSError:
            pass
    consoles = [item['properties'] for item in result['sessions']
                if item['properties'].get('User') == str(uid)
                and item['properties'].get('Class') == 'user'
                and item['properties'].get('Active') == 'yes'
                and item['properties'].get('State') == 'active'
                and item['properties'].get('Seat')
                and item['properties'].get('Type') in ('x11', 'wayland')
                and item['properties'].get('Service') != 'xrdp-sesman']
    result['owned_active_local_graphical_sessions'] = consoles
    result['authorized_accelerated_flutter_session'] = 'not established; requires actual owned Flutter child renderer evidence'
    if args.probe_glx:
        selected = [item['properties'] for item in result['sessions']
                    if item['properties'].get('Id') == args.session]
        if len(selected) != 1:
            raise RuntimeError('Select one existing logind session')
        session = selected[0]
        if not (session.get('User') == str(uid) and session.get('Class') == 'user'
                and session.get('Type') == 'x11' and session.get('Active') == 'yes'
                and session.get('State') == 'active'):
            raise RuntimeError('Refusing GLX probe outside current UID active X11 user session')
        if not re.fullmatch(r':[0-9]+(?:\.[0-9]+)?', args.display or ''):
            raise RuntimeError('Existing local display required')
        if args.display.split('.')[0] != session.get('Display', '').split('.')[0]:
            raise RuntimeError('Display differs from existing logind session')
        if not shutil.which('glxinfo'):
            result['glx'] = {'error': 'glxinfo unavailable; not installed by this helper'}
        else:
            # Keep only ordinary runtime keys. Never import a foreign process's
            # XAUTHORITY or read/copy/log authorization material. The ordinary
            # GLX client may use the current user's existing default authority.
            env = {key: os.environ[key] for key in ('HOME', 'PATH') if key in os.environ}
            env.update(LC_ALL='C', DISPLAY=args.display, XDG_RUNTIME_DIR=str(runtime))
            result['glx'] = command(['glxinfo', '-B'], env)
            result['glx']['scope'] = 'Existing-user GLX capability only; not actual Flutter renderer evidence'
    return result


def observe(args):
    if not 0 < args.seconds <= 300:
        raise RuntimeError('Observation must be 0 < seconds <= 300')
    root = Path('/proc') / str(args.pid)
    if root.stat().st_uid != os.getuid():
        raise RuntimeError('Only an already running current-user process may be observed')
    helper = Path(__file__).with_name('linux.py')
    spec = importlib.util.spec_from_file_location('mcw_b3_linux', helper)
    b3 = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(b3)
    before = b3.sample(args.pid)
    renderer = b3.renderer(args.pid)
    measured = b3.interval(args.pid, args.seconds)
    after = b3.sample(args.pid)
    if before['start_ticks'] != after['start_ticks']:
        raise RuntimeError('Observed PID changed identity')
    return {'pid': args.pid, 'renderer': renderer, 'interval': measured,
            'scope': 'Read-only existing-process observation; driver maps/DRI fds alone do not prove acceleration'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='operation', required=True)
    inv = sub.add_parser('inventory')
    inv.add_argument('--session')
    inv.add_argument('--display')
    inv.add_argument('--probe-glx', action='store_true')
    obs = sub.add_parser('observe')
    obs.add_argument('--pid', type=int, required=True)
    obs.add_argument('--seconds', type=float, default=5)
    args = parser.parse_args()
    value = inventory(args) if args.operation == 'inventory' else observe(args)
    print(json.dumps(value, indent=2))


if __name__ == '__main__':
    main()
