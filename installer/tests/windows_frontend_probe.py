"""Temporary hosted diagnostic; real binaries and ordinary-user namespace only."""
import argparse
import ctypes
from ctypes import wintypes as W
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time

parser = argparse.ArgumentParser()
parser.add_argument('--payload', type=Path, required=True)
parser.add_argument('--proof', type=Path, required=True)
args = parser.parse_args()
if os.name != 'nt' or os.environ.get('GITHUB_ACTIONS') != 'true' or os.environ.get('RUNNER_ENVIRONMENT') != 'github-hosted':
    raise SystemExit('Disposable hosted Windows fixture required')
import native_uninstall as fixture
args.proof.mkdir(exist_ok=True)
fixture._evidence_directory = args.proof
home = Path(tempfile.mkdtemp(prefix='mcw-front-probe-'))
env = fixture.environment(home / 'data')
install = home / 'bin'
install.mkdir()
for name in ('mcw.exe', 'mcw-tray.exe'):
    shutil.copy2(args.payload / name, install / name)
cli = install / 'mcw.exe'
fixture.settings(cli, env)
user = ctypes.WinDLL('user32', use_last_error=True)
user.FindWindowW.argtypes = [W.LPCWSTR, W.LPCWSTR]
user.FindWindowW.restype = W.HWND
report = {'status':'observing', 'home':str(home), 'hosts':[]}
try:
    for host, command in [('cli',[str(cli),'tray','run']), ('companion',[str(install/'mcw-tray.exe')])]:
        with (args.proof/(host+'.stdout.log')).open('wb') as output, (args.proof/(host+'.stderr.log')).open('wb') as errors:
            process = subprocess.Popen(command,env=env,stdin=subprocess.DEVNULL,stdout=output,stderr=errors)
            deadline = time.monotonic()+20
            while not user.FindWindowW('MicCamWatchTrayClass',None) and process.poll() is None and time.monotonic()<deadline:
                time.sleep(.05)
            ready = bool(user.FindWindowW('MicCamWatchTrayClass',None))
            item={'host':host,'pid':process.pid,'window_ready':ready,'exit_before_stop':process.poll()}
            report['hosts'].append(item)
            if ready:
                stopped=fixture.run([cli,'tray','stop'],env,allowed=None,timeout=30)
                item['cooperative_stop_exit']=stopped.returncode
                try:
                    process.wait(timeout=20)
                except subprocess.TimeoutExpired:
                    item['retained_alive']=True
            item['exit_after_observation']=process.poll()
            if process.poll() is None:
                item['retained_alive']=True
                report['status']='incomplete-live-frontend-retained'
                break
    else:
        report['status']='observations_complete'
finally:
    (args.proof/'frontend-observation.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
