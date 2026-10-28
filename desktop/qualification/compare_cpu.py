"""Compare two manifest-pinned, read-only Windows development Hosts.

Usage (PowerShell; Python 3.12+, psutil and Pillow, as required by windows.py):
  py desktop/qualification/compare_cpu.py --baseline "$env:LOCALAPPDATA/MicCamWatchQualification/base" --candidate "$env:LOCALAPPDATA/MicCamWatchQualification/candidate" --out "$env:LOCALAPPDATA/MicCamWatchQualification/cpu-comparison" --seconds 15 --rounds 2
Add --window to measure both closed and open intervals and protocol-ready/close
latencies. Nothing is captured; protocol readiness is not a raster measurement.
Do not interact with the owned window during a run. Keep the real workload still.

The output directory must be new and under the dedicated qualification root.
It contains PRIVATE local evidence (including user/process paths and CLI output).
Do not publish it without a separate privacy review. No workload is manufactured.
Normalization removes only tool_version and ordering of accesses, collectors,
modules and evidence. Everything else, including process instance IDs, signatures,
policy decisions, timestamps and collector/protection details, remains significant.
Equal boundary observations do not prove absence of changes between observations.
No controlled capture-active, DLL-loading, permission or registry cases are run.
"""

import argparse
from datetime import datetime, timezone
import hashlib
import json
import math
import os
from pathlib import Path
import statistics
import subprocess
import sys
import threading
import time

import psutil

from windows import Host, U, sha, until, windows


STAGES = ('snapshot', 'signature', 'process_inventory', 'sensor')
MEMORY = ('private_commit', 'rss', 'uss', 'shareable_rss', 'process_count')
NORMALIZATION = {
    'ignored_fields': ['top-level tool_version'],
    'unordered_arrays': ['accesses', 'collectors', 'accesses[].modules', 'accesses[].evidence'],
    'preserved': 'All other fields, including unknown fields, PID/process instance/ancestry, '
                 'started_at, executable, signature, risk, enforcement, evidence and protection.',
    'policy': 'Fresh isolated app-data directories, no supplied policy or settings; compare '
              'effective risk/enforcement/evidence and protection, not just access counts. '
              'This does not qualify custom policies or prove the Host internal snapshot '
              'identical to CLI output.',
    'scope': 'Paired CLI boundary observations only; activity between boundaries is not proven stable.',
}


class InvalidComparison(RuntimeError):
    pass


def strict_json(text):
    def pairs(items):
        result = {}
        for key, value in items:
            if key in result:
                raise ValueError(f'Duplicate JSON key: {key}')
            result[key] = value
        return result

    def nonfinite(value):
        raise ValueError(f'Non-finite JSON value: {value}')

    return json.loads(text, object_pairs_hook=pairs, parse_constant=nonfinite)


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False, allow_nan=False)


def fingerprint(value):
    return hashlib.sha256(canonical(value).encode('utf-8')).hexdigest()


def normalize(document):
    if not isinstance(document, dict) or document.get('schema_version') != 3:
        raise InvalidComparison('Expected status schema_version 3')
    accesses, collectors = document.get('accesses'), document.get('collectors')
    if not isinstance(accesses, list) or not isinstance(collectors, list) or not collectors:
        raise InvalidComparison('Status requires accesses and nonempty collector health')
    if not isinstance(document.get('protection'), dict):
        raise InvalidComparison('Windows protection metadata is missing')
    names = []
    for collector in collectors:
        if (not isinstance(collector, dict) or not isinstance(collector.get('collector'), str)
                or collector.get('state') not in ('healthy', 'degraded', 'unavailable')):
            raise InvalidComparison('Malformed collector health')
        names.append(collector['collector'])
    if len(names) != len(set(names)):
        raise InvalidComparison('Duplicate collector identity')
    result = {key: value for key, value in document.items() if key != 'tool_version'}
    normalized_accesses = []
    for access in accesses:
        if not isinstance(access, dict):
            raise InvalidComparison('Malformed access observation')
        for key, choices in (
            ('resource', ('camera', 'microphone')), ('activity', ('ready', 'active')),
            ('risk', ('expected', 'unexplained', 'suspicious', 'blocked')),
            ('confidence', ('high', 'medium', 'low')),
            ('enforcement', ('allow', 'alert', 'deny', 'unknown')),
        ):
            if access.get(key) not in choices:
                raise InvalidComparison(f'Missing or unknown access {key}')
        for key in ('application', 'pid', 'executable', 'device', 'started_at', 'evidence'):
            if key not in access:
                raise InvalidComparison(f'Missing access field: {key}')
        signature = access.get('signature')
        if signature is not None and (not isinstance(signature, dict)
                                     or type(signature.get('verified')) is not bool):
            raise InvalidComparison('Malformed signature evidence')
        row = dict(access)
        for key in ('modules', 'evidence'):
            if key in row:
                if not isinstance(row[key], list):
                    raise InvalidComparison(f'Expected array: {key}')
                row[key] = sorted(row[key], key=canonical)
        normalized_accesses.append(row)
    result['accesses'] = sorted(normalized_accesses, key=canonical)
    result['collectors'] = sorted(collectors, key=canonical)
    return result


def difference(left, right, path='$', limit=30):
    """Small explanatory diff; complete normalized documents are also retained."""
    found = []

    def visit(a, b, location):
        if len(found) >= limit:
            return
        if type(a) is not type(b):
            found.append({'path': location, 'expected': a, 'observed': b})
        elif isinstance(a, dict):
            for key in sorted(a.keys() | b.keys()):
                if key not in a or key not in b:
                    found.append({'path': f'{location}.{key}', 'expected_present': key in a,
                                  'observed_present': key in b})
                else:
                    visit(a[key], b[key], f'{location}.{key}')
                if len(found) >= limit:
                    break
        elif isinstance(a, list):
            if len(a) != len(b):
                found.append({'path': location + '.length', 'expected': len(a), 'observed': len(b)})
            for index, (old, new) in enumerate(zip(a, b)):
                visit(old, new, f'{location}[{index}]')
                if len(found) >= limit:
                    break
        elif a != b:
            found.append({'path': location, 'expected': a, 'observed': b})

    visit(left, right, path)
    return found


class Evidence:
    def __init__(self, out, result):
        self.out, self.result = out, result

    def save(self, phase):
        self.result['last_checkpoint'] = phase
        self.result['updated_utc'] = datetime.now(timezone.utc).isoformat()
        temporary = self.out / 'comparison.json.tmp'
        with temporary.open('w', encoding='utf-8') as stream:
            json.dump(self.result, stream, indent=2, ensure_ascii=False, allow_nan=False)
            stream.write('\n')
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, self.out / 'comparison.json')


def pin_prefix(prefix, root):
    prefix = prefix.resolve(strict=True)
    if prefix == root or not prefix.is_relative_to(root):
        raise InvalidComparison('Both prefixes must be beneath the dedicated qualification root')
    manifest_path = prefix / 'qualification-manifest.json'
    manifest = strict_json(manifest_path.read_text(encoding='utf-8-sig'))
    if (manifest.get('mode') not in ('read_only_development', 'controlled_development')
            or manifest.get('feature') != 'flutter-desktop'):
        raise InvalidComparison('A flutter-desktop development manifest is required')
    files = {'engine_sha256': 'mcw-tray.exe', 'cli_sha256': 'mcw.exe',
             'ui_sha256': 'desktop/miccamwatch_desktop.exe'}
    verified = []
    for field, relative in files.items():
        expected = manifest.get(field)
        path = (prefix / relative).resolve(strict=True)
        if (not path.is_relative_to(prefix) or not isinstance(expected, str)
                or len(expected) != 64 or sha(path) != expected.lower()):
            raise InvalidComparison(f'Manifest identity mismatch: {relative}')
        verified.append({'path': relative, 'sha256': expected.lower()})
    if manifest['engine_sha256'].lower() == '5a0253ea90a77bf7478f6a9f1826290f6a6055723cb12bdf4975b89bdfb1b6fe':
        raise InvalidComparison('Refusing stable engine without the development parser')
    for entry in manifest.get('files', []):
        path = (prefix / entry['path']).resolve(strict=True)
        if (not path.is_relative_to(prefix) or not path.is_file()
                or sha(path) != entry['sha256'].lower()
                or ('bytes' in entry and path.stat().st_size != entry['bytes'])):
            raise InvalidComparison('Manifest payload identity mismatch')
    return {'prefix': str(prefix), 'manifest': manifest, 'manifest_sha256': sha(manifest_path),
            'verified_binaries': verified}


class ReadOnlyHost(Host):
    """Use the existing Host API, but register ownership BEFORE startup can fail.

    Host.__init__ has a forced-kill constructor fallback. This driver deliberately
    does not use that constructor: its only Host shutdown operation is WM_CLOSE.
    All snapshot/open/close/window behavior remains the existing Host behavior.
    """
    def __init__(self, pinned, environment):
        self.prefix = Path(pinned['prefix'])
        self.exe = self.prefix / 'mcw-tray.exe'
        self.ui = self.prefix / 'desktop/miccamwatch_desktop.exe'
        self.cli = self.prefix / 'mcw.exe'
        self.controlled = False
        self.environment = environment
        self.theme = 'light'
        self.lines = []
        self.known = set()
        self.identities = {}
        self.proc = self.parent = self.reader = self.hwnd = None
        self.started = time.perf_counter()

    def start(self):
        self.started = time.perf_counter()
        self.proc = subprocess.Popen(
            [str(self.exe), '--read-only-development', '--desktop-theme', self.theme],
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, encoding='utf-8', errors='replace',
            creationflags=subprocess.CREATE_NO_WINDOW, cwd=self.prefix, env=self.environment,
        )
        self.known.add(self.proc.pid)
        self.reader = threading.Thread(target=self._read, daemon=True)
        self.reader.start()
        self.parent = psutil.Process(self.proc.pid)
        self.identities[self.proc.pid] = self.parent.create_time()
        self.hwnd = until(lambda: windows({self.proc.pid}, 'MicCamWatchReadOnlyDevelopmentTray',
                                         by_class=True))[0][0]
        until(lambda: any(event.get('event') == 'observer_tick' for _, event in events(self)))
        self.members()
        return {'startup_to_first_tick_ms': (time.perf_counter() - self.started) * 1000,
                'pid': self.proc.pid, 'mode': 'read_only_development'}

    def members(self):
        result = super().members()
        for process in result:
            created = process.create_time()
            if process.pid in self.identities and self.identities[process.pid] != created:
                raise InvalidComparison('Owned process identity changed')
            self.identities[process.pid] = created
        return result

    def stop_normally(self, evidence):
        evidence['method'] = 'WM_CLOSE to exact owned Host; no terminate/kill or name cleanup'
        if self.proc is None:
            evidence.update(all_exited=True, never_launched=True)
            return
        try:
            if self.parent is not None and self.proc.poll() is None:
                self.members()
        except Exception as error:
            evidence['membership_error'] = f'{type(error).__name__}: {error}'
        if self.proc.poll() is None:
            # A partially initialized Host still owns this exact subprocess PID.
            handles = windows({self.proc.pid}, 'MicCamWatchReadOnlyDevelopmentTray', by_class=True)
            if not handles:
                try:
                    handles = until(lambda: windows({self.proc.pid},
                                    'MicCamWatchReadOnlyDevelopmentTray', by_class=True), timeout=10)
                except TimeoutError:
                    handles = []
            evidence['close_message_sent'] = bool(handles and U.PostMessageW(handles[0][0], 0x10, 0, 0))
            self.proc.wait(timeout=35)
        if self.reader is not None:
            self.reader.join(timeout=3)
        def remaining():
            live = []
            for pid, created in self.identities.items():
                try:
                    process = psutil.Process(pid)
                    if process.create_time() == created and process.is_running():
                        live.append(pid)
                except psutil.NoSuchProcess:
                    pass
            return live
        deadline = time.perf_counter() + 10
        while remaining() and time.perf_counter() < deadline:
            time.sleep(.1)
        evidence.update(code=self.proc.returncode, owned_identities=self.identities,
                        remaining_owned_pids=remaining(), all_exited=not remaining())
        if not evidence['all_exited']:
            raise InvalidComparison('Owned processes did not exit normally; no forced cleanup attempted')
        if 'membership_error' in evidence:
            raise InvalidComparison('Owned descendant identity could not be established; see shutdown evidence')
        if self.proc.returncode != 0:
            raise InvalidComparison('Owned Host exited abnormally')


def events(host, start=None, end=None):
    for row in list(host.lines):
        if start is not None and row['at'] < start:
            continue
        if end is not None and row['at'] > end:
            continue
        try:
            event = strict_json(row['line'])
        except (ValueError, TypeError):
            continue
        if isinstance(event, dict):
            yield row, event


def activity(host, start, end):
    duration = end - start
    ticks = []
    stages = {name: {'count': 0, 'cpu_100ns': 0, 'wall_ns': 0} for name in STAGES}
    malformed = []
    for row, event in events(host, start, end):
        if event.get('event') == 'observer_tick':
            sequence = event.get('sequence')
            if type(sequence) is int and sequence > 0:
                ticks.append({'at': row['at'], 'sequence': sequence})
        elif event.get('event') == 'collector_profile':
            stage = event.get('stage')
            if (stage not in STAGES or any(type(event.get(key)) is not int or event[key] < 0
                                          for key in ('cpu_100ns', 'wall_ns'))):
                malformed.append({'at': row['at'], 'event': event})
                continue
            stages[stage]['count'] += 1
            for key in ('cpu_100ns', 'wall_ns'):
                stages[stage][key] += event[key]
    for stage in stages.values():
        stage['completion_events_per_second'] = stage['count'] / duration
        stage['cpu_seconds'] = stage['cpu_100ns'] / 1e7 if stage['count'] else None
        stage['one_core_percent'] = stage['cpu_seconds'] / duration * 100 if stage['count'] else None
        stage['mean_cpu_ms'] = stage['cpu_100ns'] / stage['count'] / 1e4 if stage['count'] else None
        stage['mean_wall_ms'] = stage['wall_ns'] / stage['count'] / 1e6 if stage['count'] else None
        if not stage['count']:
            stage['cpu_100ns'] = stage['wall_ns'] = None
    tick_frequency = None
    if len(ticks) > 1:
        if any(b['sequence'] <= a['sequence'] for a, b in zip(ticks, ticks[1:])):
            raise InvalidComparison('Observer sequence did not increase')
        elapsed = ticks[-1]['at'] - ticks[0]['at']
        if elapsed > 0:
            tick_frequency = (ticks[-1]['sequence'] - ticks[0]['sequence']) / elapsed
    return {
        'observer_tick_events': ticks, 'logged_tick_events_per_second': len(ticks) / duration,
        'collector_cycles_per_second_from_sparse_sequence': tick_frequency,
        'profile_available': any(stage['count'] for stage in stages.values()),
        'stages': stages, 'malformed_profile_events': malformed,
        'accounting': 'Stages are inclusive and may nest. Never add stages together. snapshot CPU '
                      'is the instrumented outer scope, NOT necessarily whole-process CPU; '
                      'snapshot completion frequency is a separate cycle proxy. Receipt times '
                      'bound these events, not exact execution times. No events means unavailable, not zero CPU.',
    }


def isolated_environment(out, label):
    base = out / 'isolated' / label
    roaming, local = base / 'roaming', base / 'local'
    roaming.mkdir(parents=True, exist_ok=False)
    local.mkdir(parents=True, exist_ok=False)
    environment = os.environ.copy()
    environment['APPDATA'], environment['LOCALAPPDATA'] = str(roaming), str(local)
    return environment


def status_pair(pins, environment, boundary, store, evidence, reference):
    # Alternate CLI query order as well; both binaries observe the SAME live workload.
    failures = []
    for name in boundary['order']:
        pin = pins[name]
        cli = Path(pin['prefix']) / 'mcw.exe'
        row = {'variant': name, 'command': [str(cli), '--lang', 'en', 'status', '--json', '--include-ready'],
               'started_at': time.perf_counter(), 'json_valid': False, 'schema_valid': False}
        boundary['statuses'].append(row)
        try:
            if sha(cli) != pin['manifest']['cli_sha256'].lower():
                raise InvalidComparison('Paired CLI changed after manifest validation')
            completed = subprocess.run(
                row['command'], cwd=pin['prefix'], env=environment,
                capture_output=True, encoding='utf-8', errors='replace',
                creationflags=subprocess.CREATE_NO_WINDOW, timeout=90, check=False,
            )
            row.update(returncode=completed.returncode, stdout=completed.stdout, stderr=completed.stderr)
            # Codes 1 (observed access/risk) and 2 (unavailable collector) can carry valid JSON.
            document = strict_json(completed.stdout.lstrip('\ufeff'))
            row['document'] = document
            row['json_valid'] = True
            normalized = normalize(document)
            row.update(normalized=normalized, semantic_sha256=fingerprint(normalized), schema_valid=True)
            if completed.returncode not in (0, 1, 2):
                raise InvalidComparison('Unexpected status exit code, despite parseable JSON')
            if any(c['state'] != 'healthy' for c in normalized['collectors']):
                raise InvalidComparison('Incomplete collector health cannot establish equal workloads')
            if reference['value'] is None:
                reference.update(value=normalized, source={'boundary': boundary['name'], 'variant': name})
                store['workload_reference'] = reference.copy()
            row['equal_to_reference'] = row['semantic_sha256'] == fingerprint(reference['value'])
            if not row['equal_to_reference']:
                row['semantic_difference'] = difference(reference['value'], normalized)
                raise InvalidComparison('Workload/signature/policy/collector/protection changed')
        except subprocess.TimeoutExpired as error:
            # subprocess.run reaps only this owned, read-only CLI on timeout, never a Host.
            row.update(timeout=True, stdout=error.stdout.decode('utf-8', 'replace')
                       if isinstance(error.stdout, bytes) else error.stdout,
                       stderr=error.stderr.decode('utf-8', 'replace')
                       if isinstance(error.stderr, bytes) else error.stderr)
            row['error'] = 'Read-only status CLI timed out'
            failures.append(row['error'])
        except Exception as error:
            row['error'] = f'{type(error).__name__}: {error}'
            failures.append(row['error'])
        finally:
            row['finished_at'] = time.perf_counter()
            evidence.save(boundary['name'] + ':' + name)
    if failures:
        boundary['valid'] = False
        boundary['failures'] = failures
        raise InvalidComparison('; '.join(failures))
    boundary['valid'] = True


def measure_interval(host, seconds, row):
    row.update(started_at=time.perf_counter(), samples=[])
    try:
        deadline = row['started_at'] + seconds
        identities = None
        while True:
            sample = host.snapshot()
            sample['at'] = time.perf_counter()
            for process in sample['rows']:
                process['created'] = host.identities[process['pid']]
            row['samples'].append(sample)
            current = {(p['pid'], p['created']) for p in sample['rows']}
            if not current or (identities is not None and current != identities):
                raise InvalidComparison('Owned process membership changed during measurement')
            identities = current
            if host.proc.poll() is not None:
                raise InvalidComparison('Owned Host exited during measurement')
            remaining = deadline - time.perf_counter()
            if remaining <= 0:
                break
            time.sleep(min(.5, remaining))
        first, last = row['samples'][0], row['samples'][-1]
        elapsed = last['at'] - first['at']
        if elapsed <= 0:
            raise InvalidComparison('No measurable interval')
        before = {p['pid']: p['cpu_seconds'] for p in first['rows']}
        cpu = sum(p['cpu_seconds'] - before[p['pid']] for p in last['rows'])
        if cpu < 0:
            raise InvalidComparison('CPU accounting moved backwards')
        row.update(seconds=elapsed, cpu_seconds=cpu, cpu_one_core_percent=cpu / elapsed * 100,
                   median={key: statistics.median(s[key] for s in row['samples']) for key in MEMORY})
    finally:
        row['finished_at'] = time.perf_counter()
        row['activity'] = activity(host, row['started_at'], row['finished_at'])
    if not row['activity']['observer_tick_events']:
        raise InvalidComparison('No collector progress evidence during measurement; increase --seconds')
    if row['activity']['malformed_profile_events']:
        raise InvalidComparison('Malformed optional profiling evidence')


def run_variant(name, round_index, order, args, pins, store, evidence, reference):
    row = {'variant': name, 'round': round_index, 'intervals': [], 'boundaries': []}
    store['runs'].append(row)
    environment = isolated_environment(args.out, f'round-{round_index:03d}-{name}')
    row['isolated_environment'] = {key: environment[key] for key in ('APPDATA', 'LOCALAPPDATA')}
    # Revalidate every payload immediately before starting each owned Host.
    refreshed = pin_prefix(Path(pins[name]['prefix']), Path(store['qualification_root']))
    if refreshed['manifest_sha256'] != pins[name]['manifest_sha256']:
        raise InvalidComparison('Manifest changed after initial pinning')
    host = ReadOnlyHost(pins[name], environment)
    row['host_events'] = host.lines
    try:
        row['startup'] = host.start()
        evidence.save(f'round-{round_index}:{name}:started')
        for state in (('closed', 'open') if args.window else ('closed',)):
            if state == 'open':
                opened = host.open()
                row['window_open'] = {**opened, 'protocol_ready_parent_ms': opened['usable_parent_ms'],
                                      'claim': 'Protocol ready and visible HWND only; no raster timing claim'}
                evidence.save(f'round-{round_index}:{name}:window-open')
            for edge in ('before', 'after'):
                boundary = {'name': f'round-{round_index}:{name}:{state}:{edge}',
                            'order': order if edge == 'before' else list(reversed(order)), 'statuses': []}
                row['boundaries'].append(boundary)
                status_pair(pins, environment, boundary, store, evidence, reference)
                if edge == 'before':
                    interval = {'state': state, 'valid': False}
                    row['intervals'].append(interval)
                    try:
                        measure_interval(host, args.seconds, interval)
                    finally:
                        evidence.save(f'round-{round_index}:{name}:{state}:measured')
            interval['valid'] = True
            evidence.save(f'round-{round_index}:{name}:{state}:equal-workload')
        if args.window:
            row['window_close'] = host.close()
            evidence.save(f'round-{round_index}:{name}:window-closed')
        row['complete'] = True
    except BaseException as error:
        row['failure'] = {'type': type(error).__name__, 'detail': str(error)}
        raise
    finally:
        row['shutdown'] = {}
        try:
            host.stop_normally(row['shutdown'])
        except BaseException as error:
            row['shutdown'].update(all_exited=False, error=f'{type(error).__name__}: {error}')
            raise
        finally:
            row['finished_at'] = time.perf_counter()
            evidence.save(f'round-{round_index}:{name}:finally')


def summarize(store, rounds, window):
    states = ('closed', 'open') if window else ('closed',)
    if len(store['runs']) != rounds * 2 or not all(
            run.get('complete') and run.get('shutdown', {}).get('all_exited') for run in store['runs']):
        raise InvalidComparison('Not all requested runs completed and shut down')
    summary = {'claim': 'Descriptive paired measurements only, not a speedup or significance claim.',
               'states': {}, 'paired_round_deltas': []}
    for state in states:
        metrics = {}
        for name in ('baseline', 'candidate'):
            rows = [interval for run in store['runs'] if run['variant'] == name
                    for interval in run['intervals'] if interval['state'] == state and interval['valid']]
            if len(rows) != rounds:
                raise InvalidComparison('Missing validated interval')
            metrics[name] = {'cpu_one_core_percent': statistics.median(r['cpu_one_core_percent'] for r in rows),
                             **{key: statistics.median(r['median'][key] for r in rows) for key in MEMORY}}
        metrics['candidate_minus_baseline'] = {key: metrics['candidate'][key] - metrics['baseline'][key]
                                               for key in metrics['baseline']}
        summary['states'][state] = metrics
        for index in range(1, rounds + 1):
            pair = {run['variant']: next(i for i in run['intervals'] if i['state'] == state)
                    for run in store['runs'] if run['round'] == index}
            summary['paired_round_deltas'].append({
                'round': index, 'state': state,
                'cpu_one_core_percentage_points': pair['candidate']['cpu_one_core_percent']
                                                 - pair['baseline']['cpu_one_core_percent'],
                'memory_and_process_count': {key: pair['candidate']['median'][key]
                                            - pair['baseline']['median'][key] for key in MEMORY},
            })
    return summary


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--baseline', type=Path, required=True)
    parser.add_argument('--candidate', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--seconds', type=float, default=15)
    parser.add_argument('--rounds', type=int, default=2)
    parser.add_argument('--window', action='store_true', help='Also measure open UI; no screenshots or raster claims')
    args = parser.parse_args()
    if not math.isfinite(args.seconds) or args.seconds <= 0 or args.rounds < 1:
        parser.error('--seconds must be finite and positive; --rounds must be positive')
    root = (Path(os.environ['LOCALAPPDATA']) / 'MicCamWatchQualification').resolve()
    args.out = args.out.resolve()
    if args.out == root or not args.out.is_relative_to(root):
        parser.error('--out must be beneath the local qualification root (private evidence only)')
    for prefix in (args.baseline.resolve(), args.candidate.resolve()):
        if args.out.is_relative_to(prefix) or prefix.is_relative_to(args.out):
            parser.error('--out and binary prefixes must not overlap')
    args.out.mkdir(parents=True, exist_ok=False)
    store = {
        'schema_version': 1, 'status': 'running', 'comparison_valid': False,
        'started_utc': datetime.now(timezone.utc).isoformat(), 'qualification_root': str(root),
        'parameters': {'baseline': str(args.baseline.resolve()), 'candidate': str(args.candidate.resolve()),
                       'seconds': args.seconds, 'rounds': args.rounds, 'window': args.window},
        'host': {'python': sys.version, 'psutil': psutil.__version__, 'logical_cpu_count': psutil.cpu_count(),
                 'platform': sys.platform},
        'normalization': NORMALIZATION, 'runs': [], 'rounds_completed': [],
        'measurement': 'CPU100%=one logical core; process CPU includes user+kernel of owned Host/UI only, '
                       'not status CLIs. Private commit is not RSS; RSS sums may double-count shared pages. '
                       'USS is unique resident memory. No GPU/DWM or hardware energy accounting. '
                       'Closed then optionally open intervals; CLI boundary checks are outside CPU intervals. '
                       'Round order AB/BA alternates, fresh isolated environment per Host run, no cache flush.',
        'not_run': [
            {'case': 'multiple controlled app/capture-active cases', 'status': 'not_run',
             'reason': 'Requires suitable separate authorization/context; existing real activity is only observed.'},
            {'case': 'DLL-loading probes or registry/permission/consent seams', 'status': 'not_run',
             'reason': 'Excluded from this tranche; no synthetic Active streams or hardware actions.'},
        ],
        'limitations': ['Boundary equality is necessary, not proof of uninterrupted workload equality.',
                        'No screenshots. Window metrics are protocol-ready/visibility/close only.',
                        'Optional profile scopes nest; per-stage totals must never be summed.',
                        'Host startup/close errors invalidate the entire comparison; no partial speedup claim.'],
    }
    evidence = Evidence(args.out, store)
    reference = {'value': None, 'source': None}
    code = 2
    try:
        evidence.save('initialized')
        pins = {name: pin_prefix(getattr(args, name), root) for name in ('baseline', 'candidate')}
        if pins['baseline']['prefix'] == pins['candidate']['prefix']:
            raise InvalidComparison('Baseline and candidate must be distinct pinned prefixes')
        store['pins'] = pins
        evidence.save('manifests-validated')
        for index in range(1, args.rounds + 1):
            order = ['baseline', 'candidate'] if index % 2 else ['candidate', 'baseline']
            for name in order:
                run_variant(name, index, order, args, pins, store, evidence, reference)
            store['rounds_completed'].append({'round': index, 'order': order})
            evidence.save(f'round-{index}:complete')
        store['summary'] = summarize(store, args.rounds, args.window)
        store.update(status='complete', comparison_valid=True)
        code = 0
    except BaseException as error:
        store.update(status='invalid', comparison_valid=False,
                     failure={'type': type(error).__name__, 'detail': str(error)}, summary=None)
        code = 130 if isinstance(error, KeyboardInterrupt) else 2
    finally:
        evidence.save('finally')
    print(f"Comparison {store['status']}; private evidence: {args.out / 'comparison.json'}")
    return code


if __name__ == '__main__':
    raise SystemExit(main())
