#!/usr/bin/env python3
"""Private Linux x64 qualification; no installation, hardware or global input.

Package: python3 linux.py package --engine .../mcw --desktop .../bundle --out NEW
Run:     python3 linux.py run --prefix NEW [--tray] [--native]
Measure: python3 linux.py qualify --prefix NEW --out NEW_EVIDENCE --xwindow HELPER
All output is PRIVATE; raw status/logs/screenshots can identify applications.
"""
import argparse
import ctypes
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import stat
import subprocess
import sys
import threading
import time

sys.dont_write_bytecode = True  # never mutate a sealed bundle with __pycache__

FLOORS = {'GLIBC': (2, 35), 'GLIBCXX': (3, 4, 30), 'CXXABI': (1, 3, 13)}


def command(args, **kwargs):
    return subprocess.run([str(a) for a in args], check=True, text=True,
                          capture_output=True, timeout=60, **kwargs).stdout


def save(path, value):
    path.write_text(json.dumps(value, indent=2) + '\n', encoding='utf-8')


def sha(path):
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()


def private_new(path):
    path = path.absolute()
    # Never chmod/chown an existing directory; create only our own new tree.
    for ancestor in [path.parent, *path.parent.parents]:
        info = ancestor.lstat()
        if stat.S_ISLNK(info.st_mode) or info.st_mode & 0o022:
            raise RuntimeError(f'Unsafe output ancestor: {ancestor}')
    path.mkdir(mode=0o700)
    return path.resolve()


def payload(prefix):
    result = {}
    for path in sorted(prefix.rglob('*')):
        if path.is_symlink():
            raise RuntimeError(f'Symlink payload refused: {path}')
        if path.is_file() and path.relative_to(prefix).as_posix() not in ('qualification-manifest.json', 'SHA256SUMS'):
            result[str(path.relative_to(prefix))] = sha(path)
    return result

def audit(prefix):
    rows = []
    for name in payload(prefix):
        path = prefix / name
        with path.open('rb') as stream:
            if stream.read(4) != b'\x7fELF':
                continue
        header = command(['readelf', '-hW', path])
        if 'ELF64' not in header or 'Advanced Micro Devices X86-64' not in header:
            raise RuntimeError(f'Not Linux x64: {name}')
        versions = command(['readelf', '-VW', path])
        needs = versions.split('Version needs section', 1)[-1] if 'Version needs section' in versions else ''
        required = {}
        for family, version in re.findall(r'Name: (GLIBCXX|GLIBC|CXXABI)_([0-9.]+)', needs):
            required[family] = max(required.get(family, ()), tuple(map(int, version.split('.'))))
        dynamic = command(['readelf', '-dW', path])
        needed = re.findall(r'\(NEEDED\).*\[(.*?)\]', dynamic)
        rpaths = re.findall(r'\((?:RUNPATH|RPATH)\).*\[(.*?)\]', dynamic)
        if any(entry and not entry.startswith('$ORIGIN') for value in rpaths for entry in value.split(':')):
            raise RuntimeError(f'Non-relocatable runtime path in {name}: {rpaths}')
        rows.append({'path': name, 'required_versions': {k: '.'.join(map(str,v)) for k,v in required.items()},
                     'needed': needed, 'rpaths': rpaths,
                     'ubuntu_22_04_symbol_floor_pass': all(v <= FLOORS[k] for k,v in required.items())})
    if not rows or not (prefix/'mcw').is_file() or not (prefix/'desktop/miccamwatch_desktop').is_file():
        raise RuntimeError('Missing complete native/Flutter Linux bundle')
    return {'elf': rows, 'all_symbol_floors_pass': all(r['ubuntu_22_04_symbol_floor_pass'] for r in rows),
            'minimum_target': 'Linux Mint 21.x/22.x x64 (Ubuntu22.04 floor), not yet runtime-qualified',
            'gtk_minimum': 'Conservative GTK3.24.33 baseline (Ubuntu22.04); exact build version recorded; unversioned GTK imports alone cannot establish a minimum',
            'other_runtime': 'session D-Bus; GTK3; libstdc++; PipeWire >=0.3.45; graphics/display; real SNI host for --tray'}


def package(args):
    prefix = private_new(args.out)
    shutil.copy2(args.engine, prefix/'mcw')
    shutil.copytree(args.desktop, prefix/'desktop', symlinks=True)
    shutil.copy2(Path(__file__), prefix/'linux.py')
    shutil.copy2(Path(__file__).with_name('linux_accessibility.py'), prefix/'linux_accessibility.py')
    shutil.copy2(Path(__file__).with_name('linux_xwindow.c'), prefix/'linux_xwindow.c')
    report = audit(prefix)
    provenance = json.loads(args.build_receipt.read_text()) if args.build_receipt else None
    hosted = bool(provenance and provenance.get('builder') == 'github-hosted-ubuntu-22.04'
                  and provenance.get('host_os', {}).get('ID') == 'ubuntu'
                  and provenance.get('host_os', {}).get('VERSION_ID') == '22.04'
                  and provenance.get('runner_image_version'))
    baseline = bool(provenance and provenance.get('baseline') == 'ubuntu:22.04'
                    and (hosted or re.fullmatch(r'ubuntu@sha256:[0-9a-f]{64}', provenance.get('image', ''))))
    if baseline and not report['all_symbol_floors_pass']:
        raise RuntimeError('Baseline output exceeds Mint21 symbol floor; refusing compatibility label')
    label = 'baseline-built; Mint runtime qualification still required' if baseline else 'HOST-ONLY: NOT qualified or labeled Mint-compatible'
    instructions = f'''MicCamWatch isolated Linux x64 qualification\n{label}\n\nNo install/sudo/desktop entry/autostart/settings migration is needed. Do not copy\nthis over a stable installation. Keep the complete bundle together in a private\nowned directory under your HOME (not /tmp or a world-writable ancestor).\n\n1. Inspect qualification-manifest.json and verify: sha256sum -c SHA256SUMS\n2. Check runtime: python3 linux.py check --prefix .\n3. Open safely: python3 linux.py run --prefix . --tray\n   --tray requires a real Cinnamon/KDE/XFCE StatusNotifierWatcher. Absence is\n   an explicit failure, not simulated proof. Omit --tray for WSLg/window-only.\n   Add --native to inspect GTK fallback (requires --tray). Native mutation\n   controls are disabled. Flutter permits only isolated preference changes.\n4. Close the window, then Quit the qualification tray, or Ctrl+C in this\n   terminal. Only owned processes are stopped. Normal stable settings/history,\n   notifications, permissions, devices and autostart are never changed.\n\nEach run makes a NEW 0700 ~/mcw-qualification/linux-b3-* tree with separate\nconfig/data/cache/runtime. Existing trees are never reset. Evidence is private;\nreview process names/paths/screenshots before sharing. Native fallback remains\navailable; this bundle is not a signed installer or a universal GPU claim.\nPython3 and readelf/binutils are qualification tools, not application deps.\n'''
    (prefix/'README.txt').write_text(instructions)
    save(prefix/'elf-audit.json', report)
    manifest = {'schema_version': 1, 'feature': 'flutter-desktop', 'mode': 'settings_development',
                'source_revision': args.revision, 'compatibility': label, 'build_receipt': provenance,
                'audit': report, 'files': payload(prefix)}
    save(prefix/'qualification-manifest.json', manifest)
    hashes = dict(manifest['files'])
    hashes['qualification-manifest.json'] = sha(prefix/'qualification-manifest.json')
    (prefix/'SHA256SUMS').write_text(''.join(f'{digest}  {name}\n' for name,digest in sorted(hashes.items())))
    print(prefix)


def pin(prefix):
    prefix = prefix.resolve()
    for path in [prefix, *prefix.parents]:
        info = path.lstat()
        if stat.S_ISLNK(info.st_mode) or info.st_mode & 0o022:
            raise RuntimeError(f'Unsafe bundle ancestor: {path}')
    manifest = json.loads((prefix/'qualification-manifest.json').read_text())
    if manifest.get('mode') != 'settings_development' or manifest.get('feature') != 'flutter-desktop':
        raise RuntimeError('Wrong qualification manifest')
    if payload(prefix) != manifest['files']:
        raise RuntimeError('Bundle content differs from manifest')
    return manifest


def runtime_check(prefix):
    pin(prefix)
    if platform.machine() not in ('x86_64', 'AMD64') or platform.system() != 'Linux':
        raise RuntimeError('Linux x64 runtime required')
    report = audit(prefix)
    # ldd is used only after integrity-pinning our own built payload.
    report['ldd'] = {r['path']: command(['ldd', prefix/r['path']]) for r in report['elf']}
    if any('not found' in value for value in report['ldd'].values()):
        raise RuntimeError('Missing runtime dependency: ' + json.dumps(report['ldd']))
    gtk = ctypes.CDLL('libgtk-3.so.0')
    report['runtime_gtk'] = [getattr(gtk, f'gtk_get_{part}_version')() for part in ('major','minor','micro')]
    if tuple(report['runtime_gtk']) < (3,24,33):
        raise RuntimeError(f"GTK below conservative Ubuntu22.04 minimum: {report['runtime_gtk']}")
    pipewire = ctypes.CDLL('libpipewire-0.3.so.0')
    pipewire.pw_get_library_version.restype = ctypes.c_char_p
    report['runtime_pipewire'] = pipewire.pw_get_library_version().decode('ascii')
    numeric = tuple(int(v) for v in re.match(r'(\d+)\.(\d+)\.(\d+)', report['runtime_pipewire']).groups())
    if numeric < (0,3,45):
        raise RuntimeError(f"PipeWire below required0.3.45: {report['runtime_pipewire']}")
    report['runtime_glibc'] = platform.libc_ver()
    report['os_release'] = Path('/etc/os-release').read_text()
    return report


def environment():
    base = Path.home()/'mcw-qualification'
    if not base.exists():
        base.mkdir(mode=0o700)
    info = base.lstat()
    if not stat.S_ISDIR(info.st_mode) or info.st_uid != os.getuid() or info.st_mode & 0o077:
        raise RuntimeError('~/mcw-qualification must be owned private non-symlink directory')
    root = private_new(base/f'linux-b3-{time.time_ns()}')
    env = os.environ.copy()
    original_runtime = env.get('XDG_RUNTIME_DIR')
    if original_runtime:
        if env.get('WAYLAND_DISPLAY') and not os.path.isabs(env['WAYLAND_DISPLAY']):
            env['WAYLAND_DISPLAY'] = str(Path(original_runtime)/env['WAYLAND_DISPLAY'])
        if not env.get('DBUS_SESSION_BUS_ADDRESS') and (Path(original_runtime)/'bus').exists():
            env['DBUS_SESSION_BUS_ADDRESS'] = f'unix:path={original_runtime}/bus'
        # Read the existing media-server namespace, not a fictitious empty one.
        env.setdefault('PIPEWIRE_RUNTIME_DIR', original_runtime)
        if not env.get('PULSE_SERVER') and (Path(original_runtime)/'pulse/native').exists():
            env['PULSE_SERVER'] = f'unix:{original_runtime}/pulse/native'
    # Keep the real display/session bus; never replace user bus or alter GPU config.
    for variable, name in [('XDG_CONFIG_HOME','config'),('XDG_DATA_HOME','data'),
                           ('XDG_CACHE_HOME','cache'),('XDG_RUNTIME_DIR','runtime')]:
        path = root/name
        path.mkdir(mode=0o700)
        env[variable] = str(path)
    for key in ('LD_PRELOAD','MCW_LINUX_QUALIFICATION_TRAY','MCW_LINUX_QUALIFICATION_NATIVE'):
        env.pop(key, None)
    env.update(LANG='C.UTF-8', LC_ALL='C.UTF-8')
    return root, env


def launch(prefix, env, tray=False, native=False, theme='system'):
    pin(prefix)  # revalidate immediately before each owner creation
    if native and not tray:
        raise RuntimeError('--native requires --tray')
    env = env.copy()
    if tray:
        env['MCW_LINUX_QUALIFICATION_TRAY'] = '1'
    if native:
        env['MCW_LINUX_QUALIFICATION_NATIVE'] = '1'
    return subprocess.Popen([str(prefix/'mcw'), '--settings-development', '--desktop-theme', theme],
                            cwd=prefix, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                            text=True), env


def native_command(prefix, env, *args):
    return command([prefix/'mcw', *args], cwd=prefix, env=env)


def until(fn, seconds=25):
    end = time.monotonic()+seconds
    while time.monotonic() < end:
        value = fn()
        if value:
            return value
        time.sleep(.1)
    raise TimeoutError('Owned state did not arrive')


def sample(pid):
    root = Path('/proc')/str(pid)
    fields = (root/'stat').read_text().rsplit(')',1)[1].split()
    values = {'pid': pid, 'start_ticks': int(fields[19]), 'cpu_seconds': (int(fields[11])+int(fields[12]))/os.sysconf('SC_CLK_TCK')}
    for filename in ('status','smaps_rollup'):
        try:
            text = (root/filename).read_text()
            values[filename] = {k:int(v)*1024 for k,v in re.findall(r'^(VmRSS|RssAnon|RssFile|RssShmem|Pss|Private_Clean|Private_Dirty):\s+(\d+) kB', text, re.M)}
        except (FileNotFoundError, PermissionError) as error:
            values[filename] = {'unavailable': str(error)}
    return values


def tree(parent):
    result = [parent]
    for pid in result:
        try:
            result.extend(int(v) for v in Path(f'/proc/{pid}/task/{pid}/children').read_text().split())
        except FileNotFoundError:
            pass
    return result


def sample_tree(parent):
    values = {}
    for pid in tree(parent):
        try:
            values[pid] = sample(pid)
        except FileNotFoundError:
            pass  # short-lived collector recorded as a CPU lower-bound limitation
    if parent not in values:
        raise RuntimeError('Owned parent exited during resource sample')
    return values


def interval(parent, seconds):
    started = time.monotonic()
    before = sample_tree(parent)
    time.sleep(seconds)
    after = sample_tree(parent)
    elapsed = time.monotonic()-started
    common = [p for p in before.keys() & after.keys() if before[p]['start_ticks']==after[p]['start_ticks']]
    return {'elapsed': elapsed, 'before': before, 'after': after,
            'cpu_one_core_percent': sum(after[p]['cpu_seconds']-before[p]['cpu_seconds'] for p in common)/elapsed*100,
            'limitation': 'CPU lower bound: short-lived children omitted; RSS sums double-count shared pages; PSS raw when readable'}


def sni_owned(pid, env):
    def call(dest, path, method, *arguments):
        return command(['gdbus','call','--session','--dest',dest,'--object-path',path,
                        '--method',method,*arguments], env=env)
    raw = call('org.kde.StatusNotifierWatcher','/StatusNotifierWatcher',
               'org.freedesktop.DBus.Properties.Get',
               'org.kde.StatusNotifierWatcher','RegisteredStatusNotifierItems')
    for entry in re.findall(r"'([^']+)'", raw)[:128]:
        service, slash, tail = entry.partition('/')
        path = '/'+tail if slash else '/StatusNotifierItem'
        owner = call('org.freedesktop.DBus','/org/freedesktop/DBus',
                     'org.freedesktop.DBus.GetConnectionUnixProcessID',service)
        match = re.search(r'uint32 (\d+)',owner)
        if match and int(match.group(1)) == pid:
            # Pin the unique bus owner before any action, not a reusable name.
            unique = call('org.freedesktop.DBus','/org/freedesktop/DBus',
                          'org.freedesktop.DBus.GetNameOwner', service)
            name = re.search(r"'([^']+)'",unique).group(1)
            confirmed = call('org.freedesktop.DBus','/org/freedesktop/DBus',
                             'org.freedesktop.DBus.GetConnectionUnixProcessID',name)
            if not re.search(r'uint32 '+str(pid)+r'\b',confirmed):
                raise RuntimeError('SNI bus owner changed')
            menu = call(name,path,'org.freedesktop.DBus.Properties.Get',
                        'org.kde.StatusNotifierItem','Menu')
            menu_path = re.search(r"'(/[^']*)'",menu).group(1)
            layout = call(name,menu_path,'com.canonical.dbusmenu.GetLayout','0','-1','[]')
            return {'service':name,'path':path,'menu_path':menu_path,'raw_menu':layout}
    return None


def sni_activate(owned, env):
    return command(['gdbus','call','--session','--dest',owned['service'],
                    '--object-path',owned['path'],'--method',
                    'org.kde.StatusNotifierItem.Activate','0','0'], env=env)


def renderer(pid):
    root = Path('/proc')/str(pid)
    threads = []
    for task in (root/'task').iterdir():
        try:
            threads.append((task/'comm').read_text().strip())
        except FileNotFoundError:
            pass
    maps = (root/'maps').read_text()
    fds = []
    for fd in (root/'fd').iterdir():
        try:
            name = os.readlink(fd)
            if '/dev/dri/' in name:
                fds.append(name)
        except FileNotFoundError:
            pass
    software = any(re.search(r'llvmpipe|lavapipe|swrast', name, re.I) for name in threads)
    return {'classification': 'software renderer observed' if software else 'unknown; driver mapping/fd is not proof of acceleration',
            'threads': threads, 'dri_fds': fds,
            'graphics_maps': [line for line in maps.splitlines() if re.search(r'mesa|dri/|vulkan|libEGL|libGL|flutter',line,re.I)]}


def run(args):
    prefix = args.prefix.resolve()
    check = runtime_check(prefix)
    root, env = environment()
    save(root/'runtime-check.json', check)
    process, env = launch(prefix, env, args.tray, args.native, args.theme)
    print(f'Private qualification tree: {root}; Ctrl+C stops only this owner', flush=True)
    log = root/'owner.log'
    try:
        native_command(prefix, env, '--settings-development', '--open')
        with log.open('w') as stream:
            for line in process.stdout:
                stream.write(line); stream.flush()
                print(line, end='')
        if process.wait() != 0:
            raise RuntimeError(f'Owner failed; inspect {log}')
    except KeyboardInterrupt:
        pass
    finally:
        if process.poll() is None:
            try:
                native_command(prefix, env, 'tray', 'stop')
                process.wait(timeout=30)
            finally:
                if process.poll() is None:
                    process.kill(); process.wait(timeout=10)


def qualify(args):
    prefix = args.prefix.resolve()
    out = private_new(args.out)
    result = {'complete': False, 'runtime': runtime_check(prefix), 'trials': [],
              'preference_ui': 'not exercised unless --preferences is supplied',
              'sni': 'requested real registration' if args.tray else 'not requested; window-only is not SNI evidence'}
    save(out/'result.json', result)
    for theme in ('light','dark'):
        root, env = environment()
        process, env = launch(prefix, env, args.tray, False, theme)
        row = {'theme': theme, 'private_tree': str(root), 'cycles': []}
        result['trials'].append(row)
        events = []
        def drain():
            for line in process.stdout:
                events.append({'monotonic': time.monotonic(), 'line': line.rstrip()})
        reader = threading.Thread(target=drain, daemon=True); reader.start()
        generations = {}
        def ui():
            found = [p for p in tree(process.pid)[1:] if Path(f'/proc/{p}/exe').resolve()==prefix/'desktop/miccamwatch_desktop']
            if len(found)>1:
                raise RuntimeError('Duplicate owned UI')
            for child in found:
                fields = Path(f'/proc/{child}/stat').read_text().rsplit(')',1)[1].split()
                generations.setdefault(child, fields[19])
            return found[0] if found else None
        def window(pid, action='info', path=None):
            fields = Path(f'/proc/{pid}/stat').read_text().rsplit(')',1)[1].split()
            if fields[19] != generations.get(pid) or pid not in tree(process.pid)[1:]:
                raise RuntimeError('Window target is no longer the owned child generation')
            values = [args.xwindow, str(pid), action]
            if path:
                values.append(path)
            try:
                return json.loads(command(values, env=env))
            except subprocess.CalledProcessError:
                return None
        try:
            time.sleep(1)
            if process.poll() is not None:
                raise RuntimeError('Owner exited; inspect raw owner log (SNI availability may be absent)')
            row['closed'] = interval(process.pid,args.seconds)
            owned_sni = until(lambda:sni_owned(process.pid,env)) if args.tray else None
            if owned_sni:
                row['real_sni'] = owned_sni
            for cycle in range(args.cycles):
                started = time.monotonic()
                if owned_sni:
                    sni_activate(owned_sni,env)
                else:
                    native_command(prefix,env,'--settings-development','--open','--desktop-theme',theme)
                pid = until(ui)
                state = until(lambda:window(pid))
                item = {'pid':pid, 'window':state,'mapped_ms':(time.monotonic()-started)*1000}
                row['cycles'].append(item)
                if cycle == 0:
                    for _ in range(3):
                        native_command(prefix,env,'--settings-development','--open')
                    if ui()!=pid:
                        raise RuntimeError('Activation replaced owned child')
                    time.sleep(args.stabilize)
                    item['renderer'] = renderer(pid)
                    item['open'] = interval(process.pid,args.seconds)
                    if args.preferences:
                        import linux_accessibility
                        item['preferences'] = linux_accessibility.exercise(pid, root, out/f'preferences-{theme}.json')
                    if args.captures:
                        if not window(pid,'capture',out/f'{theme}.ppm'):
                            raise RuntimeError('Owned capture failed; no desktop fallback permitted')
                if not window(pid,'close'):
                    raise RuntimeError('Owned close failed')
                until(lambda:ui() is None)
                if process.poll() is not None:
                    raise RuntimeError('Rust owner exited with disposable UI')
            row['closed_final'] = interval(process.pid,args.seconds)
            row['success'] = True
        except BaseException as error:
            row['error'] = repr(error)
            raise
        finally:
            try:
                if process.poll() is None:
                    try:
                        row['stop_stdout'] = native_command(prefix,env,'tray','stop')
                        process.wait(timeout=30)
                    finally:
                        if process.poll() is None:
                            process.kill(); process.wait(timeout=10)
            except BaseException as error:
                row['cleanup_error'] = repr(error)
            reader.join(timeout=3)
            row['owner_returncode'] = process.poll()
            remaining = []
            for child, birth in generations.items():
                try:
                    fields = Path(f'/proc/{child}/stat').read_text().rsplit(')',1)[1].split()
                    if fields[19] == birth and fields[0] != 'Z':
                        remaining.append(child)
                except FileNotFoundError:
                    pass
            row['remaining_owned_ui'] = remaining
            if row['owner_returncode'] != 0 or remaining or 'cleanup_error' in row:
                row['success'] = False
            save(out/f'owner-{theme}.json',events)
            save(out/'result.json',result)
        if not row.get('success'):
            raise RuntimeError('Qualification teardown failed; inspect raw evidence')
    result['complete'] = True
    save(out/'result.json',result)
    print(out)


def main():
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='action',required=True)
    p = commands.add_parser('package')
    p.add_argument('--engine',type=Path,required=True); p.add_argument('--desktop',type=Path,required=True)
    p.add_argument('--out',type=Path,required=True); p.add_argument('--revision',required=True)
    p.add_argument('--build-receipt',type=Path)
    for name in ('check','run','qualify'):
        p = commands.add_parser(name); p.add_argument('--prefix',type=Path,required=True)
        if name!='check':
            p.add_argument('--tray',action='store_true')
        if name=='run':
            p.add_argument('--native',action='store_true'); p.add_argument('--theme',choices=['system','light','dark'],default='system')
        if name=='qualify':
            p.add_argument('--out',type=Path,required=True); p.add_argument('--xwindow',type=Path,required=True)
            p.add_argument('--captures',action='store_true'); p.add_argument('--preferences',action='store_true')
            p.add_argument('--cycles',type=int,default=3); p.add_argument('--seconds',type=float,default=15)
            p.add_argument('--stabilize',type=float,default=15)
    args = parser.parse_args()
    if args.action=='qualify' and (args.cycles<1 or not 0<args.seconds<3600 or not 0<=args.stabilize<3600):
        parser.error('Bounded positive cycles/intervals required')
    if args.action=='check':
        print(json.dumps(runtime_check(args.prefix.resolve()),indent=2))
    else:
        globals()[args.action](args)


if __name__=='__main__':
    main()
