#!/usr/bin/env python3
"""Observational bootstrap controls; never starts/signals the product.
Requires prebuilt existing session-probe and the sibling AppKit-only probe.
Run only in a disposable GitHub-hosted Intel macOS job. No success exit/receipt
is a product qualification result. See experiment.txt for exact hook points.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import time


def save(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + '\n')


def snapshot(pids):
    result = subprocess.run(['/bin/ps', '-p', ','.join(map(str, pids)),
                             '-o', 'lstart=,pid=,ppid=,pgid=,stat=,comm='],
                            capture_output=True, timeout=2)
    return dict(exit=result.returncode, stdout=result.stdout.decode(errors='replace'),
                stderr=result.stderr.decode(errors='replace'))


def control(name, executable, directory, output):
    started = time.monotonic()
    row = dict(name=name, executable=str(executable),
               sha256=hashlib.sha256(executable.read_bytes()).hexdigest(),
               started_utc=datetime.now(timezone.utc).isoformat(),
               process_group=0, status='not-started')
    process = None
    try:
        # Match product's new BSD process group and private stdio, not setsid.
        # No environment additions: inherit the caller's existing environment.
        process = subprocess.Popen([str(executable)], cwd=directory,
                                   stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                   stderr=subprocess.PIPE, process_group=0)
        row.update(pid=process.pid, status='running')
        try:
            process.wait(timeout=1.5)
        except subprocess.TimeoutExpired:
            row['alive_at_1_5_seconds'] = True
            row['identity_before_sample'] = snapshot([process.pid])
            if process.poll() is None:
                try:
                    sampled = subprocess.run(['/usr/bin/sample', str(process.pid), '1', '1',
                                              '-file', str(output / (name + '-sample.txt'))],
                                             capture_output=True, timeout=2)
                    row['sample'] = dict(exit=sampled.returncode,
                        stdout=sampled.stdout.decode(errors='replace'),
                        stderr=sampled.stderr.decode(errors='replace'))
                except subprocess.TimeoutExpired:
                    row['sample'] = dict(status='timed-out')
            row['identity_after_sample'] = snapshot([process.pid])
        if process.poll() is None:
            # This is our unreaped direct diagnostic child, never a product PID.
            row['bounded_control_kill'] = True
            process.kill()
        stdout, stderr = process.communicate(timeout=2)
        (output / (name + '.stdout')).write_bytes(stdout)
        (output / (name + '.stderr')).write_bytes(stderr)
        row.update(exit=process.returncode,
                   status='bounded-timeout' if row.get('bounded_control_kill') else
                          ('completed' if process.returncode == 0 else 'failed'))
        if name == 'session' and process.returncode == 0:
            row['session'] = json.loads(stdout)
        if name == 'appkit' and process.returncode == 0:
            row['stages'] = [json.loads(line) for line in stdout.splitlines()]
    except Exception as error:
        row.update(status='diagnostic-error', error=str(error))
    finally:
        if process is not None and process.poll() is None:
            process.kill()
            try:
                process.communicate(timeout=2)
            except subprocess.TimeoutExpired:
                row['cleanup'] = 'Direct diagnostic child did not reap within 2s'
        row['elapsed_seconds'] = time.monotonic() - started
        save(output / (name + '.json'), row)
    return row


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--hosted-disposable', action='store_true', required=True)
    parser.add_argument('--phase', choices=['before-open', 'after-close', 'reopen-stalled'], required=True)
    parser.add_argument('--session-probe', type=Path, required=True)
    parser.add_argument('--appkit-probe', type=Path, required=True)
    parser.add_argument('--working-directory', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--owner-pid', type=int)
    parser.add_argument('--child-pid', type=int)
    args = parser.parse_args()
    if not (platform.system() == 'Darwin' and platform.machine() == 'x86_64'
            and os.environ.get('GITHUB_ACTIONS') == 'true'
            and os.environ.get('RUNNER_ENVIRONMENT') == 'github-hosted' and os.geteuid() != 0
            and os.environ.get('RUNNER_OS') == 'macOS' and sys.version_info >= (3, 11)):
        raise SystemExit('Refusing: requires disposable GitHub-hosted Intel macOS, Python >=3.11')
    root = Path(os.environ['RUNNER_TEMP']).resolve(strict=True)
    output = args.output.resolve()
    probes = [args.session_probe.resolve(strict=True), args.appkit_probe.resolve(strict=True)]
    if output == root or not output.is_relative_to(root):
        raise SystemExit('Diagnostic output must be a fresh child directory under RUNNER_TEMP')
    if not all(path.is_file() and path.is_relative_to(root) for path in probes):
        raise SystemExit('Both prebuilt diagnostic probes must be files under RUNNER_TEMP')
    directory = args.working_directory.resolve(strict=True)
    output.mkdir(parents=True, exist_ok=False)
    record = dict(status='observational-only', phase=args.phase,
                  started_utc=datetime.now(timezone.utc).isoformat(),
                  controller_pid=os.getpid(), controller_ppid=os.getppid(),
                  controller_pgid=os.getpgrp(), controller_unix_sid=os.getsid(0),
                  caveat='Unix SID/PGID are not Mach bootstrap or security session IDs')
    pids = [os.getpid(), os.getppid(), *[pid for pid in
            (args.owner_pid, args.child_pid) if pid is not None and pid > 1]]
    save(output / 'phase.json', record)
    try:
        record['processes_before'] = snapshot(pids)
        with ThreadPoolExecutor(max_workers=2) as pool:
            jobs = [pool.submit(control, name, probe, directory, output)
                    for name, probe in zip(('session', 'appkit'), probes)]
            record['controls'] = [job.result() for job in jobs]
        record['processes_after'] = snapshot(pids)
    finally:
        record['finished_utc'] = datetime.now(timezone.utc).isoformat()
        save(output / 'phase.json', record)


if __name__ == '__main__':
    main()
