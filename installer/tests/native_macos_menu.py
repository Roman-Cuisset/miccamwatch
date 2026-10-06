#!/usr/bin/env python3
"""Exercise the production AppKit menu and copyable Details alert on macOS.

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
struct Case: Decodable { let name: String; let frame: DesktopFrame }
let cases = try! JSONDecoder().decode([Case].self, from: fixtureData)
var measurements = [[String: Any]]()
for test in cases {
    let frame = test.frame
    require(frame.visual == "error", "fixture must preserve unknown/degraded state")
    let entries = frame.items!
    let content = DesktopMenu(summary: frame.summary!, entries: entries, detailsLabel: frame.details_label!)
    let menu = content.menu
    require(menu.items.count == entries.count + 2, "only separator and local Details action may be added")
    for (index, entry) in entries.enumerated() {
        let row = menu.items[index]
        require(row.representedObject as? String == entry.action, "action ID changed")
        require(row.isEnabled == entry.enabled, "action enabled state changed")
        require((row.title as NSString).size(withAttributes: [.font: menu.font!]).width <= 340.5, "title exceeds native font bound")
        require(!row.title.contains("\n"), "visible title must be one line")
        require(content.diagnostic.contains(entry.detail ?? entry.label), "original diagnostic lost")
        if row.isEnabled {
            require(NSApp.sendAction(row.action!, to: row.target, from: row), "native action selector unavailable")
        }
    }
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
    let details = menu.items.last!
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
require(verifyTrackedMenuUpdate(cases[0].frame), "state refresh broke a tracked menu action")
let json = try! JSONSerialization.data(withJSONObject: measurements, options: [.prettyPrinted, .sortedKeys])
try! json.write(to: proof.appendingPathComponent("native-menu-measurements.json"))
'''

TRACKING_REGRESSION = r'''
// Appended only to a private copy of the production Swift file: same-file
// extension can exercise the actual delegate's private frame/update/tracking.
private extension DesktopDelegate {
    func verifyTrackedUpdate(_ frame: DesktopFrame) -> Bool {
        item = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        defer { if let item = item { NSStatusBar.system.removeStatusItem(item) } }
        apply(frame)
        weak var trackedContent = menuContent
        let trackedMenu = menuContent!.menu
        let row = trackedMenu.items.first { ($0.representedObject as? String) == "status" }!
        var delivered = false
        let update = Timer(timeInterval: 0.15, repeats: false) { _ in
            self.apply(frame)
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
func verifyTrackedMenuUpdate(_ frame: DesktopFrame) -> Bool {
    DesktopDelegate().verifyTrackedUpdate(frame)
}
'''


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def fixture_cases():
    locales = (
        ('en', 'Action failed', 'Telemetry degraded', 'Session lock unknown', 'Details…'),
        ('fr', 'Échec de l’action', 'Télémétrie dégradée', 'Verrouillage inconnu', 'Détails…'),
        ('de', 'Aktion fehlgeschlagen', 'Telemetrie eingeschränkt', 'Sitzungssperre unbekannt', 'Details…'),
        ('es', 'Acción fallida', 'Telemetría degradada', 'Bloqueo de sesión desconocido', 'Detalles…'),
        ('ja', '操作に失敗しました', 'テレメトリーが制限されています', 'セッションロック不明', '詳細…'),
        ('zh', '操作失败', '遥测受限', '会话锁定状态未知', '详细信息…'),
        ('ru', 'Ошибка действия', 'Телеметрия ограничена', 'Блокировка сеанса неизвестна', 'Подробности…'),
    )
    cases = []
    observed = ('Camera restriction status unknown: Current-user profiles contain no camera restriction; '
                'device/other-user scope is not readable, so effective status is unknown. ')
    for lang, failed, degraded, lock, details in locales:
        prefix = failed + ': ' + observed
        diagnostic = prefix + 'W' * (16384 - len(prefix.encode()))
        items = [dict(action='header', label='MicCamWatch native surface proof', enabled=False),
                 dict(action='status', label=degraded, detail=degraded + ': collector scope unknown', enabled=True),
                 dict(action='header', label=failed, detail=diagnostic, enabled=False),
                 dict(action='header', label=lock, detail=lock + '; automatic controls unavailable', enabled=False)]
        items.extend(dict(action=action, label=label, enabled=enabled) for action, label, enabled in (
            ('mic', 'Microphone control unavailable', False),
            ('camera', 'Approve camera restriction profile', False),
            ('pause', 'Pause alerts (30 min)', True), ('profile', 'Profile: Balanced', True),
            ('autostart', 'Enable autostart', True), ('exit', 'Exit', True)))
        cases.append(dict(name=lang, frame=dict(kind='state', summary=degraded + ': collector scope unknown',
                                              visual='error', items=items, details_label=details)))
    for name, label in (('unbroken', 'W' * 16384),
                        ('wide-graphemes', ('界👩🏽‍💻e\u0301' * 600)[:6000])):
        case = json.loads(json.dumps(cases[0]))
        case['name'] = name
        case['frame']['items'][2] = dict(action='header', label=label, enabled=False)
        require(len(label.encode()) <= 16384, 'stress fixture exceeds protocol bound')
        cases.append(case)
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
                                                     'macos_trust.swift')), str(desktop),
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
        expected = [dict(kind='action', action=row['action']) for case in cases
                    for row in case['frame']['items'] if row['enabled']]
        expected += [dict(kind='ready', protocol=1), dict(kind='action', action='status')]
        require(events == expected, 'native dispatch changed existing action IDs or emitted a Details/device action')
    (proof / 'scope.txt').write_text(
        'Real production AppKit NSMenu intrinsic width and menu tracking; real enabled action dispatch; '
        'real local Details NSAlert rendering, wrapping, scrolling and full-text copy. '
        'Menu-text PNGs are rendered visual fixtures, not screenshots of the tracked menu. '
        'Details-alert PNGs are actual NSAlert content snapshots. '
        'No Accessibility/event-posting/TCC grants or capture/device actions. '
        'Native tracked-menu screen capture is optional and uses only existing authorization.\n')
    print('PASS native AppKit width, full Details, live menu updates; clipboard exercised=' + str(args.isolated_clipboard))


if __name__ == '__main__':
    main()
