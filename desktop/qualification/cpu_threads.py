"""Read-only CPU attribution for one exact development process; never starts/stops it."""
import argparse
import ctypes as C
from ctypes import wintypes as W
import json
import os
from pathlib import Path
import time
import psutil

K=C.WinDLL('kernel32',use_last_error=True)
K.OpenThread.argtypes=[W.DWORD,W.BOOL,W.DWORD];K.OpenThread.restype=W.HANDLE
K.CloseHandle.argtypes=[W.HANDLE]
K.GetProcessIdOfThread.argtypes=[W.HANDLE];K.GetProcessIdOfThread.restype=W.DWORD
K.GetThreadTimes.argtypes=[W.HANDLE,*([C.POINTER(W.FILETIME)]*4)]
K.GetThreadDescription.argtypes=[W.HANDLE,C.POINTER(C.c_void_p)];K.GetThreadDescription.restype=C.c_long
K.LocalFree.argtypes=[C.c_void_p];K.LocalFree.restype=C.c_void_p

def ticks(value):return (value.dwHighDateTime<<32)|value.dwLowDateTime

def threads(process):
    result={}
    for row in process.threads():
        handle=K.OpenThread(0x0800,False,row.id) # THREAD_QUERY_LIMITED_INFORMATION
        if not handle:continue
        try:
            if K.GetProcessIdOfThread(handle)!=process.pid:continue
            created,exited,kernel,user=[W.FILETIME() for _ in range(4)]
            if not K.GetThreadTimes(handle,C.byref(created),C.byref(exited),C.byref(kernel),C.byref(user)):continue
            text=C.c_void_p();name=''
            if K.GetThreadDescription(handle,C.byref(text))>=0 and text.value:
                try:name=C.wstring_at(text)
                finally:K.LocalFree(text)
            key=f'{row.id}:{ticks(created)}'
            result[key]={'tid':row.id,'created_100ns':ticks(created),'name':name,
                         'kernel_seconds':ticks(kernel)/1e7,'user_seconds':ticks(user)/1e7,
                         'at':time.perf_counter()}
        finally:K.CloseHandle(handle)
    return result

def sample(process,exe,created):
    if Path(process.exe()).resolve()!=exe or process.create_time()!=created:
        raise RuntimeError('Process identity changed; sampling stopped without controlling it')
    before=time.perf_counter();cpu=process.cpu_times();after=time.perf_counter()
    return {'at':(before+after)/2,'kernel_seconds':cpu.system,'user_seconds':cpu.user,
            'threads':threads(process)}

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--pid',type=int,required=True)
    parser.add_argument('--exe',type=Path,required=True)
    parser.add_argument('--seconds',type=float,default=60)
    parser.add_argument('--out',type=Path,required=True)
    args=parser.parse_args();exe=args.exe.resolve()
    allowed=Path(os.environ['LOCALAPPDATA'])/'MicCamWatchQualification'
    if not exe.is_relative_to(allowed.resolve()):raise RuntimeError('Only dedicated qualification binaries are allowed')
    process=psutil.Process(args.pid);created=process.create_time()
    if process.name().lower()=='mcw-tray.exe' and not any(a in ['--read-only-development','--controlled-development'] for a in process.cmdline()):
        raise RuntimeError('Refusing operational tray sampling through this development helper')
    readings=[];end=time.perf_counter()+args.seconds
    while True:
        readings.append(sample(process,exe,created))
        if time.perf_counter()>=end:break
        time.sleep(min(1,max(0,end-time.perf_counter())))
    first,last=readings[0],readings[-1];elapsed=last['at']-first['at']
    total=sum(last[k]-first[k] for k in ['kernel_seconds','user_seconds'])
    tracked={};seen={}
    for reading in readings:
        for key,row in reading['threads'].items():
            if key in seen:
                old=seen[key];entry=tracked.setdefault(key,{'tid':row['tid'],'name':row['name'],'cpu_seconds':0.0})
                entry['cpu_seconds']+=sum(max(0,row[k]-old[k]) for k in ['kernel_seconds','user_seconds'])
            seen[key]=row
    ranking=sorted(tracked.values(),key=lambda x:x['cpu_seconds'],reverse=True)
    for row in ranking:row['one_core_percent']=100*row['cpu_seconds']/elapsed
    result={'pid':args.pid,'exe':str(exe),'created':created,'seconds':elapsed,'cpu_seconds':total,
            'one_core_percent':100*total/elapsed,'threads':ranking,
            'unattributed_cpu_seconds':total-sum(x['cpu_seconds'] for x in ranking),
            'measurement':'CPU100%=one logical core; no memory scans or process control; short-lived/unobserved threads remain unattributed',
            'samples':readings}
    args.out.parent.mkdir(parents=True,exist_ok=True)
    args.out.write_text(json.dumps(result,indent=2),encoding='utf-8')
    print(json.dumps({k:v for k,v in result.items() if k!='samples'},indent=2))

if __name__=='__main__':main()
