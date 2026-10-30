"""Owned-PID AT-SPI actions only. Never uses desktop/global input or settings.
Requires the desktop's existing accessibility bus/libatspi; absence fails honestly.
No gsettings accessibility toggle, package installation or keyboard injection.
"""
import ctypes as C
import fcntl
import json
import os
from pathlib import Path
import re
import time


class OwnedAccessibility:
    def __init__(self, pid):
        self.pid = pid
        self.birth = Path(f'/proc/{pid}/stat').read_text().rsplit(')',1)[1].split()[19]
        self.lib = C.CDLL('libatspi.so.0')
        self.obj = C.CDLL('libgobject-2.0.so.0')
        self.glib = C.CDLL('libglib-2.0.so.0')
        self.obj.g_object_unref.argtypes = [C.c_void_p]
        self.glib.g_free.argtypes = [C.c_void_p]
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
        if self.lib.atspi_init()!=0:
            raise RuntimeError('Existing AT-SPI bus unavailable; do not enable globally for this harness')

    def same(self):
        if Path(f'/proc/{self.pid}/stat').read_text().rsplit(')',1)[1].split()[19]!=self.birth:
            raise RuntimeError('Owned process generation changed')

    def name(self, node):
        raw = self.lib.atspi_accessible_get_name(node,None)
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
            count=self.lib.atspi_accessible_get_child_count(desktop,None)
            for index in range(min(max(count,0),1024)):
                child=self.lib.atspi_accessible_get_child_at_index(desktop,index,None)
                if child:
                    if self.lib.atspi_accessible_get_process_id(child,None)==self.pid:
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
                    if self.lib.atspi_action_get_n_actions(iface,None)>0:
                        if not self.lib.atspi_action_do_action(iface,0,None):
                            raise RuntimeError(f'Owned action refused: {action}')
                        return True
                finally:
                    self.obj.g_object_unref(iface)
        count=self.lib.atspi_accessible_get_child_count(node,None)
        for index in range(min(max(count,0),1024)):
            child=self.lib.atspi_accessible_get_child_at_index(node,index,None)
            if child:
                try:
                    if self.walk(child,output,action,depth+1):
                        return True
                finally:
                    self.obj.g_object_unref(child)
        return False

    def inspect(self, action=None):
        names=[]; performed=False
        roots=self.roots()
        try:
            for root in roots:
                if self.walk(root,names,action):
                    performed=True; break
        finally:
            for root in roots:
                self.obj.g_object_unref(root)
        return performed if action else names

    def click(self, label):
        wait(lambda:self.inspect(label),f'Owned accessible action unavailable: {label}')


def wait(fn, description):
    end=time.monotonic()+20
    while time.monotonic()<end:
        value=fn()
        if value:
            return value
        time.sleep(.15)
    raise RuntimeError(description)


def exercise(pid, root, output):
    root=root.resolve()
    if root.parent!=Path.home()/'mcw-qualification' or not root.name.startswith('linux-b3-'):
        raise RuntimeError('Preference fixture must be a newly owned qualification tree')
    ui=OwnedAccessibility(pid)
    evidence={'pid':pid,'start_ticks':ui.birth,'complete':False,'events':[]}
    def record(event):
        evidence['events'].append(event)
        output.write_text(json.dumps(evidence,indent=2))
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
