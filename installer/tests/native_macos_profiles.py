#!/usr/bin/env python3
"""Real macOS profile inspection: parser boundaries, passive CLI and native Cancel.
No profile/TCC/device changes. Interactive administrator approval is not automated.
"""
import argparse
import json
import os
from pathlib import Path
import platform
import subprocess
import tempfile
import time

SWIFT = r'''
func demand(_ condition: @autoclosure () -> Bool, _ message: String) {
    if !condition() { fatalError(message) }
}
func encoded(_ value: [String: Any]) throws -> Data {
    try PropertyListSerialization.data(fromPropertyList: value, format: .xml, options: 0)
}
let empty = try encoded([:])
let partial = try decodeCameraProfileInventory(empty, completeInventory: false)
demand(partial.state == "unknown" && !partial.completeInventory, "partial absence became allowed")
let complete = try decodeCameraProfileInventory(empty, completeInventory: true)
demand(complete.state == "allowed" && !complete.ownedInstalled, "complete absence/ownership incorrect")
let identifier = "com.roman-cuisset.miccamwatch.camera.fixture"
let uuid = "a19a13b1-fbbd-4ecd-8c05-ffb265a0e435"
let restricted = try encoded(["other-user": [["ProfileIdentifier": identifier, "ProfileUUID": uuid,
    "ProfileItems": [["PayloadType": "com.apple.applicationaccess", "PayloadContent": ["allowCamera": false]]]]]])
let global = try decodeCameraProfileInventory(restricted, completeInventory: true)
demand(global.state == "system_managed" && !global.ownedInstalled, "global profile adopted as requester-owned")
let owned = try decodeCameraProfileInventory(restricted, completeInventory: false, identifier: identifier, uuid: uuid)
demand(owned.state == "blocked" && owned.ownedInstalled, "current-user identity not recognized")
let malformed = try encoded(["user": [["ProfileIdentifier": identifier, "ProfileUUID": uuid]]])
do {
    _ = try decodeCameraProfileInventory(malformed, completeInventory: true)
    fatalError("malformed inventory became a successful global observation")
} catch {}
let rejected = microphoneControl("{\"operation\":\"camera_status_admin\",\"path\":\"/tmp/untrusted\"}")
demand(!rejected.ok && rejected.camera == nil, "privileged operation accepted request-controlled path")
let script = try administratorProfileScript()
var compileFailure: NSDictionary?
demand(script.compileAndReturnError(&compileFailure), "fixed authorization AppleScript does not compile")
let perl = Process()
perl.executableURL = URL(fileURLWithPath: "/usr/bin/perl")
perl.arguments = ["-T", "-c", "-e", profileInventoryProgram, "all"]
try perl.run()
perl.waitUntilExit()
demand(perl.terminationStatus == 0, "bounded inventory supervisor does not compile")
if CommandLine.arguments.contains("--system-scope") {
    let query = Process()
    query.executableURL = URL(fileURLWithPath: "/usr/bin/sudo")
    query.arguments = ["-n", "/usr/bin/env", "-i", "PATH=/usr/bin:/bin:/usr/sbin:/sbin",
                       "LC_ALL=C", "/usr/bin/perl", "-T", "-e", profileInventoryProgram, "all"]
    let pipe = Pipe()
    query.standardOutput = pipe
    query.standardError = FileHandle.standardError
    try query.run()
    let data = pipe.fileHandleForReading.readDataToEndOfFile()
    query.waitUntilExit()
    demand(query.terminationStatus == 0, "fixed system inventory failed (supervisor exit \(query.terminationStatus))")
    let profile = try decodeCameraProfileInventory(data, completeInventory: true)
    demand(profile.completeInventory && profile.state != "unknown" && !profile.ownedInstalled,
           "successful global query remained unknown or adopted profile ownership")
    print("runner-authorized system inventory: \(profile.state); no ownership adopted")
}
if CommandLine.arguments.contains("--cancel") {
    _ = NSApplication.shared
    NSApp.setActivationPolicy(.accessory)
    let timer = Timer(timeInterval: 0.2, repeats: false) { _ in
        guard let view = NSApp.modalWindow?.contentView,
              let bitmap = view.bitmapImageRepForCachingDisplay(in: view.bounds) else {
            fatalError("native inspection confirmation did not render")
        }
        view.cacheDisplay(in: view.bounds, to: bitmap)
        let image = bitmap.representation(using: .png, properties: [:])!
        try! image.write(to: URL(fileURLWithPath: CommandLine.arguments.last!))
        NSApp.abortModal()
    }
    RunLoop.main.add(timer, forMode: .modalPanel)
    let result = microphoneControl("{\"operation\":\"camera_status_admin\"}")
    demand(result.ok && result.camera?.authorization == "cancelled", "native pre-authorization Cancel was not preserved")
    demand(result.camera?.state == "unknown" && result.camera?.completeInventory == false, "Cancel established global camera permission")
    print("native Cancel exercised before any administrator request")
}
print("profile scope, ownership, malformed inventory and privileged input refusal passed")
'''


def orphaned_confirmation(binary, env):
    """Exit only our CLI while its real helper awaits preauthorization consent."""
    process = subprocess.Popen([str(binary), 'camera', 'inspect', '--admin', '--json'],
                               env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    helper = None
    try:
        deadline = time.monotonic() + 25
        while helper is None and process.poll() is None and time.monotonic() < deadline:
            rows = subprocess.check_output(['/bin/ps', '-axo', 'pid=,ppid='], text=True)
            for row in rows.splitlines():
                pid, parent = map(int, row.split())
                if parent != process.pid:
                    continue
                command = subprocess.run(['/bin/ps', '-p', str(pid), '-o', 'args='],
                                         capture_output=True, text=True)
                if command.returncode == 0 and 'camera_status_admin' in command.stdout:
                    helper = pid
                    break
            if helper is None:
                time.sleep(.05)
        if helper is None:
            raise RuntimeError('Real administrator-inspection confirmation helper was not reached')
        time.sleep(.25)
        process.terminate()
        process.communicate(timeout=10)
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            try:
                os.kill(helper, 0)
            except ProcessLookupError:
                return
            time.sleep(.05)
        raise RuntimeError('Native confirmation helper survived its Rust owner')
    finally:
        if process.poll() is None:
            process.terminate()
            process.communicate(timeout=10)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--proof', type=Path, required=True)
    args = parser.parse_args()
    if platform.system() != 'Darwin' or os.geteuid() == 0:
        raise SystemExit('Requires ordinary-user native macOS; do not run MCW as root')
    args.proof.mkdir(parents=True, exist_ok=True)
    root = Path(__file__).resolve().parents[2]
    results = []
    def run(name, command, *, env=None, codes=(0,), timeout=90):
        result = subprocess.run([str(arg) for arg in command], capture_output=True, env=env, timeout=timeout)
        (args.proof / (name + '.stdout')).write_bytes(result.stdout)
        (args.proof / (name + '.stderr')).write_bytes(result.stderr)
        results.append(dict(name=name, exit=result.returncode))
        if result.returncode not in codes:
            raise RuntimeError(f'{name}: exit {result.returncode}: {result.stderr.decode(errors="replace")}')
        return result
    with tempfile.TemporaryDirectory(prefix='mcw-profile-inspection-') as work:
        work = Path(work)
        source = work / 'main.swift'
        source.write_text((root / 'native/macos_brand.swift').read_text() + '\n' +
                          (root / 'native/macos_controls.swift').read_text() + '\n' + SWIFT)
        driver = work / 'profiles-driver'
        run('compile', ['swiftc', source, '-o', driver, '-framework', 'AppKit', '-framework', 'CoreAudio'])
        run('scope-boundaries', [driver])
        home = work / 'home'
        home.mkdir()
        temporary = work / 'native-tmp'
        temporary.mkdir(mode=0o700)
        env = dict(os.environ, HOME=str(home), XDG_CONFIG_HOME=str(home / 'config'),
                   XDG_DATA_HOME=str(home / 'data'), XDG_STATE_HOME=str(home / 'state'),
                   TMPDIR=str(temporary))
        baseline = sorted(str(path.relative_to(home)) for path in home.rglob('*'))
        passive = run('passive-cli', [args.binary.resolve(), 'camera', 'inspect', '--json'], env=env, codes=(0, 2))
        observation = json.loads(passive.stdout)
        if observation['authorization'] != 'not_requested':
            raise RuntimeError('Passive CLI requested or claimed administrator authorization')
        if observation['state'] == 'unknown' and not observation['administrator_required']:
            raise RuntimeError('Incomplete profile scope has no explicit administrator proposal')
        if baseline != sorted(str(path.relative_to(home)) for path in home.rglob('*')):
            raise RuntimeError('Read-only inspection created user state')
        run('native-cancel', [driver, '--cancel', args.proof.resolve() / 'inspection-confirmation.png'], env=env)
        orphaned_confirmation(args.binary.resolve(), env)
        results.append(dict(name='owner-exit-retires-native-confirmation', exit=0))
        run('runner-authorized-system-inventory', [driver, '--system-scope'], env=env)
    (args.proof / 'results.json').write_text(json.dumps(results, indent=2) + '\n')
    (args.proof / 'scope.txt').write_text(
        'Real AppKit cancellation before OS authorization, genuine passive CLI and production profile parser. '
        'No profile installation, device control or TCC change. The fixed read-only root query is exercised '
        'using the disposable runner existing sudo authorization; MCW itself stays unelevated. Interactive '
        'system administrator approval/denial dialogue requires human credentials and is not claimed here.\n')
    print('PASS passive profile inspection, scope/ownership boundaries and native pre-authorization Cancel')


if __name__ == '__main__':
    main()
