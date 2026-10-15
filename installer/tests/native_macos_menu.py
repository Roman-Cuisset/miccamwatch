#!/usr/bin/env python3
"""UI proof with explicitly simulated data: production AppKit menu and Details on macOS.

Compiles unchanged native implementation files with a test-only entrypoint in a
private temporary directory. Uses real NSMenu tracking/measurement and NSAlert,
not menu mocks, Accessibility, event posting, camera/microphone, or TCC grants.
Menu text PNGs are explicitly rendered fixtures; Details PNGs are actual native
alert content. An optional display screenshot may require preexisting permission.
"""

import argparse
import json
from pathlib import Path
import platform
import subprocess
import tempfile


HARNESS = r'''
import AppKit

func require(_ condition: @autoclosure () -> Bool, _ message: String) {
    if !condition() { fputs("FAIL: \(message)\n", stderr); exit(1) }
}
func png(_ view: NSView, _ path: URL) {
    guard let bitmap = view.bitmapImageRepForCachingDisplay(in: view.bounds) else {
        require(false, "AppKit bitmap unavailable"); return
    }
    view.cacheDisplay(in: view.bounds, to: bitmap)
    guard let data = bitmap.representation(using: .png, properties: [:]) else {
        require(false, "AppKit PNG unavailable"); return
    }
    try! data.write(to: path)
}
// A visual fixture only. All regression assertions below target the actual
// production NSMenu, its measured intrinsic size, and its native alert action.
final class MenuTextFixture: NSView {
    let presentedMenu: NSMenu
    init(_ menu: NSMenu) {
        self.presentedMenu = menu
        super.init(frame: NSRect(x: 0, y: 0, width: menu.size.width, height: CGFloat(menu.items.count * 24 + 12)))
    }
    required init?(coder: NSCoder) { fatalError("not used") }
    override var isFlipped: Bool { true }
    override func draw(_ dirtyRect: NSRect) {
        NSColor.controlBackgroundColor.setFill()
        bounds.fill()
        for (index, row) in presentedMenu.items.enumerated() {
            let point = NSPoint(x: 20, y: CGFloat(index * 24 + 6))
            if row.isSeparatorItem {
                NSColor.separatorColor.setFill()
                NSRect(x: 12, y: point.y + 8, width: bounds.width - 24, height: 1).fill()
            } else {
                (row.title as NSString).draw(at: point, withAttributes: [
                    .font: presentedMenu.font!,
                    .foregroundColor: row.isEnabled ? NSColor.controlTextColor : NSColor.disabledControlTextColor
                ])
            }
        }
    }
}
func findText(_ view: NSView) -> NSTextView? {
    if let text = view as? NSTextView { return text }
    for child in view.subviews {
        if let found = findText(child) { return found }
    }
    return nil
}
let app = NSApplication.shared
require(app.setActivationPolicy(.accessory) || app.activationPolicy() == .accessory, "accessory policy unavailable")
app.finishLaunching()
require(NSScreen.main != nil, "native Aqua desktop required")
let fixtureURL = URL(fileURLWithPath: CommandLine.arguments[1])
let proof = URL(fileURLWithPath: CommandLine.arguments[2], isDirectory: true)
let checkClipboard = CommandLine.arguments[3] == "1"
let fixtureData = try! Data(contentsOf: fixtureURL)
struct Case: Decodable { let name: String; let frame: DesktopFrame; let selected_profile: String? }
let cases = try! JSONDecoder().decode([Case].self, from: fixtureData)
// Exercise the production decoder/validator, not a parallel fixture schema.
let rawCases = try! JSONSerialization.jsonObject(with: fixtureData) as! [[String: Any]]
let valid = rawCases[0]["frame"] as! [String: Any]
func rejects(_ object: [String: Any]) -> Bool {
    let data = try! JSONSerialization.data(withJSONObject: object)
    guard let decoded = try? JSONDecoder().decode(DesktopFrame.self, from: data) else { return true }
    return !validDesktopState(decoded)
}
var oversized = valid
let validRows = valid["items"] as! [[String: Any]]
oversized["items"] = Array(repeating: validRows[0], count: 20) + [validRows.last!]
require(rejects(oversized), "unbounded menu item count accepted")
var deep = valid
var deepRows = valid["items"] as! [[String: Any]]
let profileIndex = deepRows.firstIndex { ($0["action"] as? String) == "profile" }!
var deepChoices = deepRows[profileIndex]["children"] as! [[String: Any]]
deepChoices[0]["children"] = [deepChoices[1]]
deepRows[profileIndex]["children"] = deepChoices
deep["items"] = deepRows
require(rejects(deep), "grandchild menu accepted")
var measurements = [[String: Any]]()
for test in cases {
    let frame = test.frame
    let entries = frame.items!
    require(validDesktopState(frame), "fixture rejected by production protocol consumer")
    let content = DesktopMenu(summary: frame.summary!, entries: entries, detailsLabel: frame.details_label!)
    let menu = content.menu
    func inspectRows(_ entries: [DesktopItem], _ native: NSMenu, parentEnabled: Bool = true) {
        let rows = native.items.filter { $0.representedObject != nil || $0.submenu != nil }
        require(rows.count == entries.count, "native menu lost a producer item")
        for (entry, row) in zip(entries, rows) {
            require((row.title as NSString).size(withAttributes: [.font: menu.font!]).width <= 340.5, "title exceeds native font bound")
            require(!row.title.contains("\n"), "visible title must be one line")
            require(content.diagnostic.contains(entry.detail ?? entry.label), "original diagnostic lost")
            if let children = entry.children {
                require(row.action == nil && row.representedObject == nil, "profile must not cycle")
                require(row.submenu != nil, "profile choices must be directly reachable")
                require(row.submenu!.size.width <= 400, "profile submenu exceeds native width bound")
                require(row.submenu!.items.count == 4, "all four profiles must be available")
                let selected = row.submenu!.items.filter { $0.state == .on }
                require(selected.count == (test.selected_profile == nil ? 0 : 1), "selected profile is ambiguous")
                require((selected.first?.representedObject as? String) == test.selected_profile, "native checked state does not identify selected profile")
                inspectRows(children, row.submenu!, parentEnabled: row.isEnabled)
            } else if parentEnabled && row.isEnabled {
                require(NSApp.sendAction(row.action!, to: row.target, from: row), "native action selector unavailable")
            }
        }
    }
    inspectRows(entries, menu)
    require(content.diagnostic.contains(frame.summary!), "full status summary lost")
    require(menu.size.width <= 400, "actual NSMenu intrinsic width exceeds 400 points")
    var tracked = false
    let cancel = Timer(timeInterval: 0.15, repeats: false) { _ in
        require(menu.size.width <= 400, "tracked NSMenu exceeds width bound")
        tracked = true
        if test.name == "en" {
            let screenshot = Process()
            screenshot.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
            screenshot.arguments = ["-x", proof.appendingPathComponent("en-native-menu.png").path]
            do {
                try screenshot.run()
                screenshot.waitUntilExit()
                try! String(screenshot.terminationStatus).write(
                    to: proof.appendingPathComponent("native-menu-screenshot.exit"), atomically: true, encoding: .utf8)
            } catch {
                try! error.localizedDescription.write(
                    to: proof.appendingPathComponent("native-menu-screenshot-unavailable.txt"), atomically: true, encoding: .utf8)
            }
        }
        menu.cancelTracking()
    }
    RunLoop.main.add(cancel, forMode: .eventTracking)
    RunLoop.main.add(cancel, forMode: .common)
    let screen = NSScreen.main!.visibleFrame
    menu.popUp(positioning: nil, at: NSPoint(x: screen.midX, y: screen.midY), in: nil)
    require(tracked, "actual native menu tracking not exercised")
    let fixture = MenuTextFixture(menu)
    let window = NSWindow(contentRect: fixture.bounds, styleMask: [.borderless], backing: .buffered, defer: false)
    window.contentView = fixture
    window.orderFront(nil)
    png(fixture, proof.appendingPathComponent(test.name + "-menu-text-fixture.png"))
    window.orderOut(nil)
    let details = menu.items[menu.items.count - 2]
    require((menu.items.last!.representedObject as? String) == "exit", "Exit must remain last")
    require(details.title == frame.details_label!, "Details must be immediately above Exit")
    require(details.isEnabled && details.action != nil, "Details action must be accessible, not a disabled tooltip")
    require(details.representedObject == nil, "Details must not create a Rust/device action")
    var inspected = false
    let inspect = Timer(timeInterval: 0.2, repeats: false) { _ in
        guard let window = NSApp.modalWindow, let root = window.contentView,
              let text = findText(root) else {
            require(false, "native Details alert did not render selectable diagnostic text"); return
        }
        require(text.string == content.diagnostic, "native Details text differs from full diagnostic")
        require(!text.isEditable && text.isSelectable, "Details must be read-only and copyable")
        require(!text.isHorizontallyResizable && text.textContainer!.widthTracksTextView, "Details does not wrap")
        require(window.frame.width < screen.width, "Details alert does not fit display")
        text.layoutManager!.ensureLayout(for: text.textContainer!)
        require(text.layoutManager!.usedRect(for: text.textContainer!).height > 260, "long diagnostic did not wrap/scroll")
        if checkClipboard {
            text.selectAll(nil)
            text.copy(nil)
            require(NSPasteboard.general.string(forType: .string) == content.diagnostic, "copy lost original diagnostic")
            NSPasteboard.general.clearContents()
            text.setSelectedRange(NSRange(location: 0, length: 0))
        }
        png(root, proof.appendingPathComponent(test.name + "-details-alert.png"))
        inspected = true
        NSApp.abortModal()
        window.orderOut(nil)
    }
    RunLoop.main.add(inspect, forMode: .modalPanel)
    RunLoop.main.add(inspect, forMode: .common)
    require(NSApp.sendAction(details.action!, to: details.target, from: details), "Details selector unavailable")
    require(inspected, "Details action did not open a native alert")
    try! content.diagnostic.write(to: proof.appendingPathComponent(test.name + "-full-diagnostic.txt"), atomically: true, encoding: .utf8)
    measurements.append(["case": test.name, "menu_width_points": menu.size.width,
                         "screen_width_points": screen.width, "visual": frame.visual!,
                         "titles": menu.items.map { $0.title },
                         "diagnostic_utf8_bytes": content.diagnostic.utf8.count,
                         "native_menu_tracking": tracked, "native_details_copy": inspected && checkClipboard])
}
require(verifyTrackedMenuUpdate(cases[0].frame, cases.last!.frame), "state refresh broke a tracked menu action")
for appearance in [NSAppearance.Name.aqua, .darkAqua] {
    NSApp.appearance = NSAppearance(named: appearance)
    var rendered = Set<Data>()
    for visual in ["idle", "ready", "active", "partial", "error"] {
        let image = desktopStatusImage(visual)
        require(image.isTemplate, "status mark must adapt to light/dark appearance")
        for scale in [1, 2] {
            let view = NSImageView(frame: NSRect(x: 0, y: 0, width: CGFloat(22 * scale), height: CGFloat(18 * scale)))
            view.imageScaling = .scaleAxesIndependently
            view.image = image
            view.appearance = NSAppearance(named: appearance)
            png(view, proof.appendingPathComponent("\(appearance.rawValue)-\(visual)-\(scale)x-status.png"))
        }
        rendered.insert(image.tiffRepresentation!)
    }
    require(rendered.count == 5, "status badges fail to distinguish observation states")
}
let json = try! JSONSerialization.data(withJSONObject: measurements, options: [.prettyPrinted, .sortedKeys])
try! json.write(to: proof.appendingPathComponent("native-menu-measurements.json"))
'''

TRACKING_REGRESSION = r'''
// Appended only to a private copy of the production Swift file: same-file
// extension can exercise the actual delegate's private frame/update/tracking.
private extension DesktopDelegate {
    func verifyTrackedUpdate(_ frame: DesktopFrame, _ recovery: DesktopFrame) -> Bool {
        item = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        defer { if let item = item { NSStatusBar.system.removeStatusItem(item) } }
        apply(frame)
        weak var trackedContent = menuContent
        let trackedMenu = menuContent!.menu
        let row = trackedMenu.items.first { ($0.representedObject as? String) == "status" }!
        var delivered = false
        let update = Timer(timeInterval: 0.15, repeats: false) { _ in
            self.apply(recovery)
            require(self.item!.button!.toolTip == recovery.summary!, "status tooltip failed to recover")
            require(!self.menuContent!.diagnostic.contains("device/other-user scope"), "recovery retained stale error")
            require(!self.menuContent!.menu.items.contains { ($0.representedObject as? String) == "camera_inspect" },
                    "recovered inventory still requests administrator inspection")
            require(trackedContent != nil && row.target != nil,
                    "replacing a frame deallocated the tracked menu action target")
            delivered = NSApp.sendAction(row.action!, to: row.target, from: row)
            trackedMenu.cancelTracking()
        }
        RunLoop.main.add(update, forMode: .common)
        showMenu()
        return delivered
    }
}
func verifyTrackedMenuUpdate(_ frame: DesktopFrame, _ recovery: DesktopFrame) -> Bool {
    DesktopDelegate().verifyTrackedUpdate(frame, recovery)
}
'''


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def fixture_cases():
    locales = (
        ('en', 'State unknown', 'Observation incomplete', 'Session lock unknown', 'Details…'),
        ('fr', 'État inconnu', 'Observation incomplète', 'Verrouillage inconnu', 'Détails…'),
        ('de', 'Zustand unbekannt', 'Beobachtung unvollständig', 'Sitzungssperre unbekannt', 'Details…'),
        ('es', 'Estado desconocido', 'Observación incompleta', 'Bloqueo de sesión desconocido', 'Detalles…'),
        ('ja', '状態不明', '観測範囲が不完全です', 'セッションロック不明', '詳細…'),
        ('zh', '状态未知', '观测范围不完整', '会话锁定状态未知', '详细信息…'),
        ('ru', 'Состояние неизвестно', 'Неполное наблюдение', 'Блокировка сеанса неизвестна', 'Подробности…'),
    )
    cases = []
    observed = ('Camera restriction status unknown: Current-user profiles contain no camera restriction; '
                'device/other-user scope is not readable, so effective status is unknown. ')
    for lang, unknown, degraded, lock, details in locales:
        prefix = unknown + ': ' + observed
        diagnostic = prefix + 'W' * (16384 - len(prefix.encode()))
        activity = 'corespeechd: MIC ACTIVE — ' + degraded
        summary = activity + '\n\ncoremediaio_video: Degraded\nRunning-state has no client identity'
        items = [dict(action='header', label='MicCamWatch simulated-data UI proof', enabled=False),
                 dict(action='status', label=activity, detail=summary, enabled=True),
                 dict(action='header', label=unknown, detail=diagnostic, enabled=False),
                 dict(action='header', label=lock, detail=lock + '; automatic controls unavailable', enabled=False)]
        items.extend(dict(action=action, label=label, enabled=enabled) for action, label, enabled in (
            ('mic', 'Mute — writable inputs', True),
            ('camera', 'Approve camera restriction profile', True),
            ('camera_inspect', 'Inspect camera restrictions (administrator approval; read-only)', True),
            ('pause', 'Pause alerts (30 min)', True)))
        choices = [dict(action='profile_' + name.lower(), label=name,
                        enabled=True, checked=name == 'Meeting')
                   for name in ('Balanced', 'Private', 'Meeting', 'Development')]
        items.append(dict(action='profile', label='Profile: Meeting', enabled=True, children=choices))
        items.extend(dict(action=action, label=label, enabled=True)
                     for action, label in (('autostart', 'Enable autostart'), ('exit', 'Exit')))
        cases.append(dict(name=lang, selected_profile='profile_meeting',
                          frame=dict(kind='state', summary=summary,
                                     visual='partial', items=items, details_label=details)))
    for name, label in (('unbroken', 'W' * 16384),
                        ('wide-graphemes', ('界👩🏽‍💻e\u0301' * 600)[:6000])):
        case = json.loads(json.dumps(cases[0]))
        case['name'] = name
        case['frame']['items'][2] = dict(action='header', label=label, enabled=False)
        require(len(label.encode()) <= 16384, 'stress fixture exceeds protocol bound')
        cases.append(case)
    for selected in ('Private', 'Development'):
        case = json.loads(json.dumps(cases[0]))
        case['name'] = 'selected-' + selected.lower()
        case['selected_profile'] = 'profile_' + selected.lower()
        profile = next(row for row in case['frame']['items'] if row['action'] == 'profile')
        profile['label'] = 'Profile: ' + selected
        for choice in profile['children']:
            choice['checked'] = choice['action'] == case['selected_profile']
        cases.append(case)
    override = json.loads(json.dumps(cases[0]))
    override['name'] = 'explicit-config'
    override['selected_profile'] = None
    profile = next(row for row in override['frame']['items'] if row['action'] == 'profile')
    profile['enabled'] = False
    profile['label'] = 'Profile controlled by explicit --config; menu changes disabled'
    for choice in profile['children']:
        choice.update(enabled=False, checked=False)
    cases.append(override)
    recovery = json.loads(json.dumps(cases[0]))
    recovery['name'] = 'recovery'
    recovery['selected_profile'] = 'profile_balanced'
    recovery['frame']['summary'] = 'corespeechd: MIC ACTIVE'
    recovery['frame']['visual'] = 'active'
    recovery['frame']['items'] = [
        row for index, row in enumerate(recovery['frame']['items'])
        if index not in (2, 3) and row['action'] != 'camera_inspect']
    status = next(row for row in recovery['frame']['items'] if row['action'] == 'status')
    status.update(label='corespeechd: MIC ACTIVE',
                  detail='corespeechd: MIC ACTIVE\n' + 'Recovered collector detail. ' * 180)
    profile = next(row for row in recovery['frame']['items'] if row['action'] == 'profile')
    profile['label'] = 'Profile: Balanced'
    for choice in profile['children']:
        choice['checked'] = choice['action'] == 'profile_balanced'
    cases.append(recovery)
    return cases


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--proof', type=Path, required=True)
    parser.add_argument('--isolated-clipboard', action='store_true',
                        help='exercise native Copy only on a disposable runner; default leaves user clipboard untouched')
    args = parser.parse_args()
    require(platform.system() == 'Darwin', 'native AppKit proof requires macOS')
    proof = args.proof.resolve()
    proof.mkdir(parents=True, exist_ok=True)
    repo = Path(__file__).resolve().parents[2]
    native = repo / 'native'
    cases = fixture_cases()
    fixtures = proof / 'diagnostic-frames.json'
    fixtures.write_text(json.dumps(cases, ensure_ascii=False, indent=2) + '\n')
    with tempfile.TemporaryDirectory(prefix='mcw-native-menu-') as directory:
        work = Path(directory)
        # Keep production declarations unchanged; replace only its CLI entrypoint.
        capture = (native / 'macos_capture.swift').read_text()
        declarations, separator, _ = capture.partition('let arguments = Array(CommandLine.arguments.dropFirst())')
        require(bool(separator), 'cannot locate production Swift CLI entrypoint')
        entrypoint = work / 'main.swift'
        entrypoint.write_text(declarations + HARNESS)
        bridge = work / 'native.h'
        bridge.write_text('#include <libproc.h>\n#include <bsm/libbsm.h>\n')
        executable = work / 'native-menu'
        arch = 'arm64' if platform.machine() == 'arm64' else 'x86_64'
        desktop = work / 'macos_desktop.swift'
        desktop.write_text((native / 'macos_desktop.swift').read_text() + TRACKING_REGRESSION)
        command = ['swiftc', '-target', arch + '-apple-macosx15.0', '-O', str(entrypoint),
                   *(str(native / name) for name in ('macos_controls.swift', 'macos_process.swift',
                                                     'macos_trust.swift', 'macos_brand.swift')), str(desktop),
                   '-import-objc-header', str(bridge), '-lproc', '-lbsm', '-o', str(executable)]
        for framework in ('AppKit', 'AVFoundation', 'CoreAudio', 'CoreGraphics', 'CoreMediaIO',
                          'Foundation', 'Security', 'UserNotifications'):
            command += ['-framework', framework]
        for name, argv in (('build', command), ('surface', [str(executable), str(fixtures), str(proof),
                                                         '1' if args.isolated_clipboard else '0'])):
            result = subprocess.run(argv, capture_output=True, text=True, timeout=180)
            (proof / (name + '.stdout')).write_text(result.stdout)
            (proof / (name + '.stderr')).write_text(result.stderr)
            (proof / (name + '.exit')).write_text(str(result.returncode) + '\n')
            require(result.returncode == 0, f'{name} failed: {result.stdout}\n{result.stderr}')
        events = [json.loads(line) for line in (proof / 'surface.stdout').read_text().splitlines()]
        def dispatched(rows, parent_enabled=True):
            for row in rows:
                enabled = parent_enabled and row['enabled']
                if 'children' in row:
                    yield from dispatched(row['children'], enabled)
                elif enabled:
                    yield dict(kind='action', action=row['action'])
        expected = [event for case in cases for event in dispatched(case['frame']['items'])]
        expected += [dict(kind='ready', protocol=2), dict(kind='action', action='status')]
        require(events == expected, 'native dispatch changed direct action IDs or emitted a Details/device action')
    (proof / 'scope.txt').write_text(
        'Real production AppKit NSMenu intrinsic width and menu tracking; real enabled action dispatch; '
        'Simulated input data only; no claim of hardware capture coverage. '
        'real local Details NSAlert rendering, wrapping, scrolling and full-text copy. '
        'Menu-text PNGs are rendered visual fixtures, not screenshots of the tracked menu. '
        'Details-alert PNGs are actual NSAlert content snapshots. '
        'No Accessibility/event-posting/TCC grants or capture/device actions. '
        'Native tracked-menu screen capture is optional and uses only existing authorization.\n')
    print('PASS native AppKit width, full Details, live menu updates; clipboard exercised=' + str(args.isolated_clipboard))


if __name__ == '__main__':
    main()
