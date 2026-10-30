"""Owned-PID AT-SPI actions on an explicitly owned, private accessibility bus.
The real session/SNI bus is preserved. No accessibility policy is changed.
No gsettings toggle, package installation, global input or keyboard injection.
"""
import argparse
import ctypes as C
import fcntl
import json
import os
from pathlib import Path
import re
import shutil
import signal
import stat
import subprocess
import sys
import time
from xml.sax.saxutils import escape


class PrivateAccessibility:
    """Own a bus and registry, never repair/activate the desktop's registry.

    libatspi's AT_SPI_BUS_ADDRESS override also applies to the GTK3 ATK bridge.
    Start registryd directly with the caller's DISPLAY/XAUTHORITY, not stale
    user-bus activation environment. No service dirs, X root property writes,
    org.a11y.Status setters or --use-gnome-session are involved.
    Sources: GNOME at-spi2-core 2.52 atspi/atspi-misc.c, registryd/registry-main.c;
    GTK 3.24 gtk/a11y/gtkaccessibility.c (initializes the bridge directly).
    """
    def __init__(self, root, env):
        self.root = root
        self.env = env.copy()
        self.processes = []
        self.streams = []
        self.evidence = {'complete': False, 'session_bus': env.get('DBUS_SESSION_BUS_ADDRESS'),
                         'display': env.get('DISPLAY'), 'processes': [],
                         'performance_accounting': 'Bus/registry/controller are outside the Rust owner tree and excluded from engine/Flutter resource totals',
                         'bridge': 'GTK3 ATK bridge lives in the exact PID/start_ticks/executable/ancestry-checked Flutter child; no injected bridge process'}
        self.address = f'unix:path={root}/runtime/a11y'
        self.application_environment = {'AT_SPI_BUS_ADDRESS': self.address, 'NO_AT_BRIDGE': '0'}

    def save(self):
        (self.root/'accessibility-fixture.json').write_text(json.dumps(self.evidence, indent=2))

    def call(self, destination, path, method, *arguments):
        result = subprocess.run(
            ['gdbus', 'call', '--address', self.address, '--dest', destination,
             '--object-path', path, '--method', method, '--', *arguments],
            env=self.env, capture_output=True, text=True, timeout=3)
        if result.returncode:
            raise RuntimeError(f'Private AT-SPI {method}: {result.stderr.strip()}')
        return result.stdout

    def spawn(self, name, argv):
        stream = (self.root/f'accessibility-{name}.log').open('w')
        self.streams.append(stream)
        process = subprocess.Popen(argv, env=self.env, stdout=stream,
                                   stderr=subprocess.STDOUT, start_new_session=True)
        self.processes.append(process)
        recorded = {'name': name, 'pid': process.pid, 'start_ticks': None}
        self.evidence['processes'].append(recorded)
        recorded['start_ticks'] = Path(f'/proc/{process.pid}/stat').read_text().rsplit(')', 1)[1].split()[19]
        self.save()
        return process

    def check(self):
        for process, recorded in zip(self.processes, self.evidence['processes']):
            if process.poll() is not None:
                raise RuntimeError(f"Private accessibility {recorded['name']} exited; inspect its log")
            birth = Path(f'/proc/{process.pid}/stat').read_text().rsplit(')', 1)[1].split()[19]
            if birth != recorded['start_ticks']:
                raise RuntimeError('Private accessibility process generation changed')
        owner = self.call('org.freedesktop.DBus', '/org/freedesktop/DBus',
                          'org.freedesktop.DBus.GetNameOwner', 'org.a11y.atspi.Registry')
        match = re.fullmatch(r"\('(:[0-9]+\.[0-9]+)',\)\s*", owner)
        if not match:
            raise RuntimeError(f'Unexpected private registry owner reply: {owner!r}')
        unique = match.group(1)
        pid = self.call('org.freedesktop.DBus', '/org/freedesktop/DBus',
                        'org.freedesktop.DBus.GetConnectionUnixProcessID', unique)
        if not re.fullmatch(r'\(uint32 '+str(self.processes[-1].pid)+r',\)\s*', pid):
            raise RuntimeError('Private registry bus owner is not the owned registry PID')
        if self.evidence.get('registry_owner', unique) != unique:
            raise RuntimeError('Private registry bus generation changed')
        self.evidence['registry_owner'] = unique

    def __enter__(self):
        try:
            daemon = shutil.which('dbus-daemon')
            registry = Path('/usr/libexec/at-spi2-registryd')
            if not daemon or not registry.is_file() or not shutil.which('gdbus'):
                raise RuntimeError('Existing dbus-daemon, gdbus and /usr/libexec/at-spi2-registryd required; no installation attempted')
            if not self.env.get('DISPLAY') or len(os.fsencode(self.root/'runtime/a11y')) >= 100:
                raise RuntimeError('Private accessibility needs the authorized X display and a short private socket path')
            info = self.root.lstat()
            if not stat.S_ISDIR(info.st_mode) or info.st_uid != os.getuid() or info.st_mode & 0o077:
                raise RuntimeError('Accessibility fixture root must be an owned private non-symlink directory')
            # No activation directories: these two explicit Popen children are
            # the entire fixture. Numeric uid EXTERNAL auth excludes other users.
            config = self.root/'accessibility-bus.conf'
            config.write_text(
                '<busconfig><type>accessibility</type><auth>EXTERNAL</auth>'
                f'<listen>{escape(self.address)}</listen><policy context="default">'
                f'<allow user="{os.getuid()}"/><allow own="*"/>'
                '<allow send_destination="*"/><allow receive_sender="*"/>'
                '</policy></busconfig>')
            self.env.update(self.application_environment)
            self.spawn('bus', [daemon, f'--config-file={config}', f'--address={self.address}',
                               '--nofork', '--nopidfile'])
            def bus_ready():
                if self.processes[0].poll() is not None:
                    raise RuntimeError('Private accessibility bus exited; inspect its log')
                try:
                    return self.call('org.freedesktop.DBus', '/org/freedesktop/DBus',
                                     'org.freedesktop.DBus.GetId')
                except (RuntimeError, subprocess.TimeoutExpired):
                    return None
            wait(bus_ready, 'Private accessibility bus did not start')
            self.spawn('registry', [str(registry)])
            def registry_ready():
                try:
                    self.check()
                    return True
                except (RuntimeError, subprocess.TimeoutExpired) as error:
                    self.evidence['last_startup_error'] = repr(error)
                    if any(p.poll() is not None for p in self.processes):
                        raise
                    return False
            wait(registry_ready, 'Private registry did not start; inspect accessibility logs/fixture receipt (DISPLAY/XAUTHORITY must authorize this display)')
            self.evidence['complete'] = True
            self.save()
            return self
        except BaseException as error:
            self.evidence['error'] = repr(error)
            self.__exit__(*sys.exc_info())
            raise

    def __exit__(self, kind, error, traceback):
        if error:
            self.evidence['error'] = repr(error)
        failures = []
        try:
            for process, recorded in reversed(list(zip(self.processes, self.evidence['processes']))):
                # Popen owns this unreaped child; a recycled PID cannot be
                # targeted. Each fixture process has its own process group.
                try:
                    if process.poll() is None:
                        try:
                            os.killpg(process.pid, signal.SIGTERM)
                        except ProcessLookupError:
                            pass
                        try:
                            process.wait(timeout=5)
                        except subprocess.TimeoutExpired:
                            try:
                                os.killpg(process.pid, signal.SIGKILL)
                            except ProcessLookupError:
                                pass
                            process.wait(timeout=5)
                except BaseException as cleanup_error:
                    recorded['cleanup_error'] = repr(cleanup_error)
                    failures.append(recorded['name'])
                recorded['returncode'] = process.poll()
            self.evidence['stopped'] = all(p.poll() is not None for p in self.processes)
        finally:
            for stream in self.streams:
                stream.close()
            self.save()
        if failures or not self.evidence['stopped']:
            raise RuntimeError(f'Private accessibility cleanup failed: {failures}; inspect fixture receipt')
        return False


class GError(C.Structure):
    _fields_ = [('domain', C.c_uint), ('code', C.c_int), ('message', C.c_char_p)]


class OwnedAccessibility:
    def __init__(self, pid, start_ticks, owner_pid, owner_start_ticks, executable):
        self.pid = pid
        self.birth = Path(f'/proc/{pid}/stat').read_text().rsplit(')',1)[1].split()[19]
        self.owner_pid = owner_pid
        self.owner_birth = owner_start_ticks
        self.executable = executable.resolve()
        if self.birth != start_ticks:
            raise RuntimeError('Owned UI generation changed before accessibility attachment')
        self.same()
        self.last_names = []
        self.last_root_count = 0
        self.lib = C.CDLL('libatspi.so.0')
        self.obj = C.CDLL('libgobject-2.0.so.0')
        self.glib = C.CDLL('libglib-2.0.so.0')
        self.obj.g_object_unref.argtypes = [C.c_void_p]
        self.obj.g_object_unref.restype = None
        self.glib.g_free.argtypes = [C.c_void_p]
        self.glib.g_free.restype = None
        self.glib.g_error_free.argtypes = [C.POINTER(GError)]
        self.glib.g_error_free.restype = None
        self.glib.g_main_context_iteration.argtypes = [C.c_void_p, C.c_int]
        self.glib.g_main_context_iteration.restype = C.c_int
        signatures = {
            'atspi_init': (C.c_int, []),
            'atspi_get_desktop': (C.c_void_p,[C.c_int]),
            'atspi_accessible_get_process_id': (C.c_uint,[C.c_void_p,C.c_void_p]),
            'atspi_accessible_get_child_count': (C.c_int,[C.c_void_p,C.c_void_p]),
            'atspi_accessible_get_child_at_index': (C.c_void_p,[C.c_void_p,C.c_int,C.c_void_p]),
            'atspi_accessible_get_name': (C.c_void_p,[C.c_void_p,C.c_void_p]),
            'atspi_accessible_get_action_iface': (C.c_void_p,[C.c_void_p]),
            'atspi_action_get_n_actions': (C.c_int,[C.c_void_p,C.c_void_p]),
            'atspi_action_do_action': (C.c_int,[C.c_void_p,C.c_int,C.c_void_p]),
        }
        for name,(result,args) in signatures.items():
            fn = getattr(self.lib,name); fn.restype=result; fn.argtypes=args
        if self.lib.atspi_init() not in (0, 1):
            raise RuntimeError('Private AT-SPI bus unavailable; inspect owned registry log, never enable globally')

    def call(self, name, *arguments):
        error = C.POINTER(GError)()
        value = getattr(self.lib, name)(*arguments, C.byref(error))
        if error:
            try:
                detail = error.contents.message.decode('utf-8', 'replace')
                raise RuntimeError(f'{name}: {detail} (domain={error.contents.domain}, code={error.contents.code})')
            finally:
                self.glib.g_error_free(error)
        return value

    def same(self):
        if Path(f'/proc/{self.pid}/stat').read_text().rsplit(')',1)[1].split()[19]!=self.birth:
            raise RuntimeError('Owned process generation changed')
        if Path(f'/proc/{self.pid}/exe').resolve() != self.executable:
            raise RuntimeError('Owned accessible executable changed')
        owner = Path(f'/proc/{self.owner_pid}/stat').read_text().rsplit(')', 1)[1].split()
        if owner[19] != self.owner_birth or owner[0] == 'Z':
            raise RuntimeError('Owned Rust process generation changed')
        ancestor = self.pid
        for _ in range(64):
            if ancestor == self.owner_pid:
                return
            fields = Path(f'/proc/{ancestor}/stat').read_text().rsplit(')', 1)[1].split()
            ancestor = int(fields[1])
            if ancestor <= 1:
                break
        raise RuntimeError('Accessible UI is no longer a descendant of the owned Rust process')

    def name(self, node):
        raw = self.call('atspi_accessible_get_name', node)
        if not raw:
            return ''
        try:
            return C.string_at(raw).decode('utf-8','replace')
        finally:
            self.glib.g_free(raw)

    def roots(self):
        self.same()
        desktop = self.lib.atspi_get_desktop(0)
        if not desktop:
            return []
        result=[]
        try:
            count=self.call('atspi_accessible_get_child_count', desktop)
            for index in range(min(max(count,0),1024)):
                child=self.call('atspi_accessible_get_child_at_index', desktop, index)
                if child:
                    if self.call('atspi_accessible_get_process_id', child)==self.pid:
                        result.append(child)
                    else:
                        self.obj.g_object_unref(child)
        finally:
            self.obj.g_object_unref(desktop)
        return result

    def walk(self, node, output, action=None, depth=0):
        if depth>32 or len(output)>4096:
            raise RuntimeError('Owned accessibility tree exceeds bounds')
        name=self.name(node)
        output.append(name)
        # Actions occur only inside an application whose root PID was matched.
        if action and name==action:
            self.same()
            iface=self.lib.atspi_accessible_get_action_iface(node)
            if iface:
                try:
                    if self.call('atspi_action_get_n_actions', iface)>0:
                        if not self.call('atspi_action_do_action', iface, 0):
                            raise RuntimeError(f'Owned action refused: {action}')
                        return True
                finally:
                    self.obj.g_object_unref(iface)
        count=self.call('atspi_accessible_get_child_count', node)
        for index in range(min(max(count,0),1024)):
            child=self.call('atspi_accessible_get_child_at_index', node, index)
            if child:
                try:
                    if self.walk(child,output,action,depth+1):
                        return True
                finally:
                    self.obj.g_object_unref(child)
        return False

    def inspect(self, action=None):
        # Dispatch bounded pending registry/cache events; never a blocking loop.
        for _ in range(256):
            if not self.glib.g_main_context_iteration(None, False):
                break
        names=[]; performed=False
        roots=self.roots()
        self.last_root_count = len(roots)
        try:
            for root in roots:
                if self.walk(root,names,action):
                    performed=True; break
        finally:
            for root in roots:
                self.obj.g_object_unref(root)
        self.last_names = names
        return performed if action else names

    def click(self, label):
        try:
            wait(lambda:self.inspect(label), f'Owned accessible action unavailable: {label}')
        except BaseException as error:
            raise RuntimeError(f'{error}; owned_pid={self.pid}, start_ticks={self.birth}, '
                               f'owned_roots={self.last_root_count}, names={self.last_names!r}. '
                               'Inspect private registry/owner logs; no global enable or input fallback attempted') from error


def wait(fn, description):
    end=time.monotonic()+20
    while time.monotonic()<end:
        value=fn()
        if value:
            return value
        time.sleep(.15)
    raise RuntimeError(description)


def exercise(pid, root, output, start_ticks, owner_pid, owner_start_ticks, executable):
    root=root.resolve()
    if root.parent!=Path.home()/'mcw-qualification' or not root.name.startswith('linux-b3-'):
        raise RuntimeError('Preference fixture must be a newly owned qualification tree')
    ui=OwnedAccessibility(pid, start_ticks, owner_pid, owner_start_ticks, executable)
    evidence={'pid':pid,'start_ticks':ui.birth,'executable':str(ui.executable),
              'owner_pid':owner_pid,'owner_start_ticks':owner_start_ticks,
              'controller_pid':os.getpid(),
              'controller_start_ticks':Path('/proc/self/stat').read_text().rsplit(')',1)[1].split()[19],
              'complete':False,'events':[]}
    def record(event):
        evidence['events'].append(event)
        output.write_text(json.dumps(evidence,indent=2))
    record({'case':'owned_ui_generation_checked','accessibility_bus':os.environ.get('AT_SPI_BUS_ADDRESS')})
    ui.click('Settings')
    path=root/'config/MicCamWatch/settings.toml'
    def content():
        return path.read_text() if path.is_file() else ''
    def sound():
        match=re.search(r'^sound_enabled\s*=\s*(true|false)',content(),re.M)
        return match.group(1) if match else None
    before=sound()
    ui.click('Capture sounds')
    wait(lambda:sound() is not None and sound()!=before,'No native saved sound change')
    wait(lambda:any('Saved by native.' in n for n in ui.inspect()),'No native UI success')
    changed=sound()
    record({'case':'sound_write','before':before,'after':changed,'settings':content(),'ui':ui.inspect()})
    ui.click('Capture sounds')
    wait(lambda:sound()!=changed,'No native restore readback')
    wait(lambda:any('Saved by native.' in n for n in ui.inspect()),'No restore success')
    record({'case':'sound_restore','settings':content()})
    ui.click('Pause 30 min')
    wait(lambda:re.search(r'^pause_notifications_until\s*=\s*"',content(),re.M),
         'No server-owned pause deadline saved')
    wait(lambda:any('Paused by native.' in n for n in ui.inspect()),'No native pause state')
    record({'case':'pause_native_deadline','settings':content(),'ui':ui.inspect()})
    ui.click('Resume')
    wait(lambda:not re.search(r'^pause_notifications_until\s*=',content(),re.M),
         'Native pause deadline not cleared')
    wait(lambda:any('Not paused according to native.' in n for n in ui.inspect()),
         'No native resume state')
    record({'case':'resume_native_readback','settings':content(),'ui':ui.inspect()})
    # Real persistence failure using only a disposable tree: hold the same flock
    # inode, admit UI work, temporarily replace the owned settings file with a
    # directory, then release. No chmod/permission or stable mutation occurs.
    backup=path.with_name('settings.qualification-backup')
    lock=path.with_suffix('.lock')
    with lock.open('a+b') as stream:
        fcntl.flock(stream,fcntl.LOCK_EX)
        moved=False
        try:
            try:
                ui.click('Capture sounds')
                wait(lambda:any('Accepted by native' in n for n in ui.inspect()),'No native admission before failure fixture')
                path.rename(backup); moved=True; path.mkdir(mode=0o700)
            finally:
                fcntl.flock(stream,fcntl.LOCK_UN)
            wait(lambda:any('Saving failed.' in n for n in ui.inspect()),'No native persistence failure outcome')
            record({'case':'owned_settings_load_failure','ui':ui.inspect()})
        finally:
            if moved:
                if path.is_dir():
                    path.rmdir()
                backup.rename(path)
    evidence['complete']=True
    record({'case':'fixture_restored','settings':content()})
    return evidence


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--pid', type=int, required=True)
    parser.add_argument('--start-ticks', required=True)
    parser.add_argument('--owner-pid', type=int, required=True)
    parser.add_argument('--owner-start-ticks', required=True)
    parser.add_argument('--executable', type=Path, required=True)
    parser.add_argument('--root', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    try:
        if not os.environ.get('AT_SPI_BUS_ADDRESS'):
            raise RuntimeError('Explicit private AT_SPI_BUS_ADDRESS required')
        exercise(args.pid, args.root, args.output, args.start_ticks, args.owner_pid,
                 args.owner_start_ticks, args.executable)
    except BaseException as error:
        evidence = json.loads(args.output.read_text()) if args.output.is_file() else {
            'pid': args.pid, 'complete': False, 'events': []}
        evidence.update(complete=False, error=repr(error),
                        accessibility_bus=os.environ.get('AT_SPI_BUS_ADDRESS'),
                        session_bus=os.environ.get('DBUS_SESSION_BUS_ADDRESS'))
        args.output.write_text(json.dumps(evidence, indent=2))
        raise


if __name__ == '__main__':
    main()
