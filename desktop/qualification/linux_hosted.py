#!/usr/bin/env python3
"""Disposable Ubuntu22.04 CI desktop, real XFCE SNI, no global input/hardware.
Run under dbus-run-session -- xvfb-run. Never attach to a user's desktop bus.
Keep the hosted runner HOME; isolate desktop state through private XDG dirs.
Preference qualification owns a separate, explicitly stopped AT-SPI bus/registry;
neither desktop accessibility policy nor the SNI session bus is replaced.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import time

sys.dont_write_bytecode = True
import linux


def main():
    os.umask(0o077)
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--prefix',type=Path,required=True)
    parser.add_argument('--out',type=Path,required=True)
    parser.add_argument('--xwindow',type=Path,required=True)
    args=parser.parse_args()
    if os.environ.get('GITHUB_ACTIONS')!='true' or os.environ.get('RUNNER_OS')!='Linux':
        raise RuntimeError('Disposable hosted Linux desktop only; never attach this fixture to a user bus')
    out=linux.private_new(args.out)
    env=os.environ.copy()
    # Nesting HOME under the evidence path makes the qualification tray's Unix
    # socket exceed its native limit. This disposable runner's existing HOME
    # keeps ~/mcw-qualification short; linux.environment owns each new fixture.
    for key,name in [('XDG_CONFIG_HOME','config'),('XDG_DATA_HOME','data'),
                     ('XDG_CACHE_HOME','cache'),('XDG_STATE_HOME','state'),('XDG_RUNTIME_DIR','runtime')]:
        directory=out/name; directory.mkdir(mode=0o700); env[key]=str(directory)
    env.update(LANG='C.UTF-8',LC_ALL='C.UTF-8')
    linux.command(['dbus-update-activation-environment','DISPLAY','XAUTHORITY','HOME',
                   'XDG_CONFIG_HOME','XDG_DATA_HOME','XDG_CACHE_HOME','XDG_STATE_HOME',
                   'XDG_RUNTIME_DIR'],env=env)
    owned=[]; streams=[]
    evidence={'complete':False,'environment':'GitHub-hosted Ubuntu22.04 x64/Xvfb software-rendered desktop, not physical Mint/GPU/Wayland',
              'no_device_actions':True,'cases':[]}
    def save():
        linux.save(out/'hosted-result.json',evidence)
    def launch(name, command):
        stream=(out/f'{name}.log').open('w'); streams.append(stream)
        process=subprocess.Popen(command,env=env,stdout=stream,stderr=subprocess.STDOUT)
        owned.append((name,process)); return process
    save()
    try:
        launch('openbox',['openbox'])
        settings=[['-p','/configver','-n','-t','int','-s','2'],
                  ['-p','/panels','-n','-a','-t','int','-s','1'],
                  ['-p','/panels/panel-1/position','-n','-t','string','-s','p=6;x=640;y=20'],
                  ['-p','/panels/panel-1/length','-n','-t','uint','-s','100'],
                  ['-p','/panels/panel-1/plugin-ids','-n','-a','-t','int','-s','1'],
                  ['-p','/plugins/plugin-1','-n','-t','string','-s','systray']]
        for setting in settings:
            linux.command(['xfconf-query','-c','xfce4-panel',*setting],env=env)
        panel=launch('panel',['xfce4-panel','--disable-wm-check'])
        def watcher():
            reply=subprocess.run(['gdbus','call','--session','--dest','org.kde.StatusNotifierWatcher',
                                  '--object-path','/StatusNotifierWatcher','--method',
                                  'org.freedesktop.DBus.Properties.Get',
                                  'org.kde.StatusNotifierWatcher','IsStatusNotifierHostRegistered'],
                                 env=env,capture_output=True,text=True,timeout=10)
            return reply.stdout if reply.returncode==0 and 'true' in reply.stdout else None
        evidence['watcher']=linux.until(watcher)
        if panel.poll() is not None:
            raise RuntimeError('Real desktop SNI host exited')
        script=Path(__file__).with_name('linux.py')
        common=[sys.executable,'-B',str(script),'qualify','--prefix',str(args.prefix.resolve()),
                '--xwindow',str(args.xwindow.resolve()),'--tray','--cycles','3',
                '--seconds','15','--stabilize','15','--captures']
        # Retain complete basic lifecycle/SNI evidence even if actual AT-SPI
        # semantics are unavailable on this particular hosted Flutter backend.
        for name,extra in [('lifecycle',[]),('preferences',['--preferences'])]:
            with (out/f'{name}-harness.log').open('w') as log:
                result=subprocess.run([*common,'--out',str(out/name),*extra],env=env,
                                      stdout=log,stderr=subprocess.STDOUT,timeout=480)
            evidence['cases'].append({'name':name,'returncode':result.returncode})
            save()
            if result.returncode:
                raise RuntimeError(f'{name} qualification failed; raw failure evidence retained')
        evidence['complete']=True
    except BaseException as error:
        evidence['error']=repr(error)
        raise
    finally:
        for name,process in reversed(owned):
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    process.kill(); process.wait(timeout=5)
            evidence.setdefault('owned_desktop_cleanup',[]).append({'name':name,'pid':process.pid,'returncode':process.returncode})
        for stream in streams:
            stream.close()
        save()


if __name__=='__main__':
    main()
