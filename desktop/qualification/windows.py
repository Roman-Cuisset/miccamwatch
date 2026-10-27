"""Exercise only the manifest-pinned read-only development tray, never installed MCW.
Requires Python3.12+, psutil and Pillow. No administrator rights or device actions.
"""
import argparse
import ctypes as C
from ctypes import wintypes as W
import hashlib
import json
import os
from pathlib import Path
import queue
import re
import statistics
import subprocess
import threading
import time

import psutil
from PIL import Image

U = C.WinDLL('user32', use_last_error=True)
G = C.WinDLL('gdi32')
CALLBACK = C.WINFUNCTYPE(W.BOOL, W.HWND, W.LPARAM)
U.EnumWindows.argtypes = [CALLBACK, W.LPARAM]
U.GetWindowThreadProcessId.argtypes = [W.HWND, C.POINTER(W.DWORD)]
U.GetWindowTextLengthW.argtypes = [W.HWND]
U.GetWindowTextW.argtypes = [W.HWND, W.LPWSTR, C.c_int]
U.IsWindowVisible.argtypes = [W.HWND]
U.PostMessageW.argtypes = [W.HWND, W.UINT, W.WPARAM, W.LPARAM]
U.GetClientRect.argtypes = [W.HWND, C.POINTER(W.RECT)]
U.GetDpiForWindow.argtypes, U.GetDpiForWindow.restype = [W.HWND], W.UINT
U.GetDC.argtypes, U.GetDC.restype = [W.HWND], W.HDC
U.ReleaseDC.argtypes = [W.HWND, W.HDC]
U.PrintWindow.argtypes = [W.HWND, W.HDC, W.UINT]
U.SetProcessDpiAwarenessContext.argtypes = [C.c_void_p]
U.SetProcessDpiAwarenessContext(C.c_void_p(-4))
G.CreateCompatibleDC.argtypes, G.CreateCompatibleDC.restype = [W.HDC], W.HDC
G.CreateCompatibleBitmap.argtypes, G.CreateCompatibleBitmap.restype = [W.HDC,C.c_int,C.c_int],W.HBITMAP
G.SelectObject.argtypes, G.SelectObject.restype = [W.HDC,W.HANDLE],W.HANDLE
G.GetDIBits.argtypes = [W.HDC,W.HBITMAP,W.UINT,W.UINT,C.c_void_p,C.c_void_p,W.UINT]
G.DeleteObject.argtypes, G.DeleteDC.argtypes = [W.HANDLE],[W.HDC]

class Header(C.Structure):
    _fields_=[('size',W.DWORD),('width',W.LONG),('height',W.LONG),('planes',W.WORD),
              ('bits',W.WORD),('compression',W.DWORD),('image_size',W.DWORD),
              ('xp',W.LONG),('yp',W.LONG),('used',W.DWORD),('important',W.DWORD)]

def sha(path):
    with path.open('rb') as f: return hashlib.file_digest(f,'sha256').hexdigest()

def until(fn, timeout=25):
    end=time.perf_counter()+timeout
    while time.perf_counter()<end:
        found=fn()
        if found: return found
        time.sleep(.01)
    raise TimeoutError('Owned development state did not arrive')

def windows(pids,title,visible=False):
    found=[]
    @CALLBACK
    def visit(hwnd,_):
        pid=W.DWORD();U.GetWindowThreadProcessId(hwnd,C.byref(pid))
        if pid.value in pids and (not visible or U.IsWindowVisible(hwnd)):
            text=C.create_unicode_buffer(U.GetWindowTextLengthW(hwnd)+1)
            U.GetWindowTextW(hwnd,text,len(text))
            if text.value==title: found.append((hwnd,pid.value))
        return True
    U.EnumWindows(visit,0)
    return found

def capture(hwnd,path,dark):
    # Reuse the study's proven HWND-only capture; never fall back to desktop pixels.
    r=W.RECT();assert U.GetClientRect(hwnd,C.byref(r))
    dc=U.GetDC(hwnd);target=G.CreateCompatibleDC(dc)
    bitmap=G.CreateCompatibleBitmap(dc,r.right,r.bottom);old=G.SelectObject(target,bitmap)
    try:
        assert U.PrintWindow(hwnd,target,3),'PrintWindow failed'
        G.SelectObject(target,old)
        header=Header(C.sizeof(Header),r.right,-r.bottom,1,32,0,0,0,0,0,0)
        pixels=C.create_string_buffer(r.right*r.bottom*4)
        assert G.GetDIBits(target,bitmap,0,r.bottom,pixels,C.byref(header),0)==r.bottom
        image=Image.frombuffer('RGB',(r.right,r.bottom),pixels.raw,'raw','BGRX',0,1)
        expected=(17,26,23) if dark else (244,246,245)
        colors={color:count for count,color in image.getcolors(r.right*r.bottom)}
        assert colors.get(expected,0)>1000,'Expected UI palette missing'
        image.save(path)
        return {'file':path.name,'sha256':sha(path),'client':[r.right,r.bottom],
                'dpi':U.GetDpiForWindow(hwnd),'api':'PrintWindow owned HWND'}
    finally:
        G.SelectObject(target,old);G.DeleteObject(bitmap);G.DeleteDC(target);U.ReleaseDC(hwnd,dc)

class Host:
    def __init__(self,prefix,theme):
        self.prefix=prefix.resolve();self.exe=self.prefix/'mcw-tray.exe'
        self.ui=self.prefix/'desktop/miccamwatch_desktop.exe'
        allowed=Path(os.environ['LOCALAPPDATA'])/'MicCamWatchQualification'
        if not self.prefix.is_relative_to(allowed.resolve()):
            raise RuntimeError('Qualification prefix must be under the dedicated local evidence root')
        manifest=json.loads((self.prefix/'qualification-manifest.json').read_text(encoding='utf-8'))
        if manifest.get('mode')!='read_only_development' or manifest.get('feature')!='flutter-desktop':
            raise RuntimeError('Read-only development manifest missing')
        if sha(self.exe)!=manifest['engine_sha256'] or sha(self.ui)!=manifest['ui_sha256']:
            raise RuntimeError('Qualified build hashes do not match; no process launched')
        if sha(self.exe)=='5a0253ea90a77bf7478f6a9f1826290f6a6055723cb12bdf4975b89bdfb1b6fe':
            raise RuntimeError('Refusing the stable binary: its parser lacks the development mode')
        self.lines=[];self.theme=theme;self.started=time.perf_counter()
        self.proc=subprocess.Popen([str(self.exe),'--read-only-development','--desktop-theme',theme],
            stdout=subprocess.PIPE,stderr=subprocess.STDOUT,encoding='utf-8',errors='replace',
            creationflags=subprocess.CREATE_NO_WINDOW,cwd=self.prefix)
        self.parent=psutil.Process(self.proc.pid);self.known={self.proc.pid}
        self.reader=threading.Thread(target=self._read,daemon=True);self.reader.start()
        self.hwnd=until(lambda:windows({self.proc.pid},'MicCamWatchReadOnlyDevelopmentTray'))[0][0]
        until(lambda:self.ticks())

    def _read(self):
        for line in self.proc.stdout:
            self.lines.append({'at':time.perf_counter(),'elapsed':time.perf_counter()-self.started,'line':line.rstrip()})

    def ticks(self):
        return [int(x['sequence']) for row in self.lines if row['line'].startswith('{')
                for x in [json.loads(row['line'])] if x.get('event')=='observer_tick']

    def members(self):
        try: candidates=[self.parent,*self.parent.children(recursive=True)]
        except psutil.NoSuchProcess:return []
        result=[]
        for p in candidates:
            try:
                if p.status()==psutil.STATUS_ZOMBIE:continue
                path=Path(p.exe()).resolve()
                if path not in [self.exe.resolve(),self.ui.resolve()]:
                    raise RuntimeError(f'Unexpected observer descendant: {path}; no name-based cleanup permitted')
                self.known.add(p.pid);result.append(p)
            except psutil.NoSuchProcess:pass
        return result

    def view(self):
        return windows({p.pid for p in self.members() if p.pid!=self.proc.pid},'MicCamWatch',True)

    def snapshot(self):
        rows=[]
        for p in self.members():
            with p.oneshot():
                m=p.memory_full_info();c=p.cpu_times()
                rows.append({'pid':p.pid,'name':p.name(),'private_commit':m.private,'rss':m.rss,
                             'uss':m.uss,'shareable_rss':m.rss-m.uss,'cpu_seconds':c.user+c.system,
                             'threads':p.num_threads(),'handles':p.num_handles()})
        return {'rows':rows,'process_count':len(rows),**{k:sum(r[k] for r in rows) for k in ['private_commit','rss','uss','shareable_rss']}}

    def idle(self,seconds):
        start=time.perf_counter();samples=[self.snapshot()]
        while time.perf_counter()-start<seconds:
            time.sleep(.5);samples.append(self.snapshot())
        elapsed=time.perf_counter()-start
        before={r['pid']:r['cpu_seconds'] for r in samples[0]['rows']}
        after={r['pid']:r['cpu_seconds'] for r in samples[-1]['rows']}
        assert before.keys()==after.keys(),'Process membership changed in idle interval'
        cpu=sum(after[pid]-before[pid] for pid in before)/elapsed*100
        return {'seconds':elapsed,'cpu_one_core_percent':cpu,'samples':samples,
                'median':{k:statistics.median(x[k] for x in samples) for k in ['private_commit','rss','uss','shareable_rss','process_count']}}

    def open(self,burst=1):
        start=time.perf_counter()
        for _ in range(burst):
            assert U.PostMessageW(self.hwnd,0x8000+42,self.proc.pid,0)
        win=until(self.view)
        visible_ms=(time.perf_counter()-start)*1000
        ready=until(lambda:[r for r in self.lines if r['at']>=start and 'desktop event=ready ' in r['line']])[-1]
        payload={k:v for k,v in re.findall(r'(\w+)=([^ ]+)',ready['line'])}
        time.sleep(.2)
        members=self.members();assert len(members)==2,'Duplicate graphical child'
        stages=[]
        for r in self.lines:
            if r['at']>=start and 'desktop event=' in r['line']:
                stages.append({'observed_ms':(r['at']-start)*1000,'line':r['line']})
        return {'window_pid':win[0][1],'visible_wall_ms':visible_ms,
                'usable_parent_ms':int(payload['parent_elapsed_ms']),
                'ready_sequence':int(payload['sequence']),'stages':stages}

    def close(self):
        window=self.view()[0];start=time.perf_counter()
        assert U.PostMessageW(window[0],0x10,0,0)
        until(lambda:not self.view())
        hidden=(time.perf_counter()-start)*1000
        until(lambda:len(self.members())==1)
        return {'hidden_ms':hidden,'process_exit_ms':(time.perf_counter()-start)*1000}

    def stop(self):
        if self.proc.poll() is None:
            U.PostMessageW(self.hwnd,0x10,0,0)
            self.proc.wait(timeout=35)
        self.reader.join(timeout=3)
        until(lambda:all(not psutil.pid_exists(pid) for pid in self.known),timeout=10)
        return {'code':self.proc.returncode,'owned_pids':sorted(self.known),'all_exited':True,'events':self.lines}


def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--prefix',required=True,type=Path)
    ap.add_argument('--out',required=True,type=Path)
    ap.add_argument('--trials',type=int,default=3)
    ap.add_argument('--cycles',type=int,default=3)
    ap.add_argument('--idle-seconds',type=float,default=5)
    ap.add_argument('--theme',choices=['light','dark'],default='light')
    ap.add_argument('--captures',action='store_true')
    args=ap.parse_args();args.out.mkdir(parents=True,exist_ok=True)
    result={'platform':'Windows11 x64 local; no hardware mutations',
            'measurement':'Private commit != resident. RSS includes shareable pages; sums may double-count. No GPU/DWM accounting. CPU100%=one logical core.',
            'trials':[]}
    for index in range(args.trials):
        print('Read-only actual-engine trial',index+1,flush=True)
        host=Host(args.prefix,args.theme);row={'trial':index+1}
        try:
            row['closed_before']=host.idle(args.idle_seconds)
            row['open']=host.open(burst=10)
            row['opened']=host.idle(args.idle_seconds)
            if args.captures and index==0:
                row['capture']=capture(host.view()[0][0],args.out/f'live-{args.theme}.png',args.theme=='dark')
            row['close']=host.close()
            tick=max(host.ticks());time.sleep(1)
            row['closed_after']=host.idle(args.idle_seconds)
            until(lambda:max(host.ticks())>tick)
            row['monitoring_continued_closed']={'before_sequence':tick,'after_sequence':max(host.ticks())}
            row['cycles']=[]
            for cycle in range(args.cycles):
                opened=host.open();time.sleep(.2)
                closed=host.close()
                row['cycles'].append({'cycle':cycle+1,'open':opened,'close':closed,'closed_process_count':len(host.members())})
        finally:
            row['shutdown']=host.stop();result['trials'].append(row)
            (args.out/'qualification.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
    print('Completed actual-engine read-only qualification',args.out,flush=True)

if __name__=='__main__':main()
