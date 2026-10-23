#!/usr/bin/env python3
"""UI proof with explicitly simulated data: production AppKit consultation/menu/About.

Compiles unchanged native implementation files with a test-only entrypoint in a
private temporary directory. Uses real NSWindow, NSMenu tracking and NSAlert,
not menu mocks, Accessibility, device APIs or TCC grants. Captures the actual
Overview, Settings and Diagnostics surfaces at narrow, minimum and roomy sizes
in native light/dark and high-contrast appearances. All fixture data is simulated.
Consultation/About PNGs use appearance-scoped caching of the actual AppKit views,
composited onto their resolved native window background. Capture provenance and
native pixel dimensions are recorded. Menu-text PNGs remain rendered fixtures.
"""

import argparse
import json
import os
from pathlib import Path
import platform
import subprocess
import tempfile


HARNESS = r'''
import AppKit
import CoreGraphics

func require(_ condition: @autoclosure () -> Bool, _ message: String) {
    if !condition() { fputs("FAIL: \(message)\n", stderr); exit(1) }
}
var imageCaptureRecords = [[String: Any]]()
@discardableResult
func png(_ view: NSView, _ path: URL) -> [String: Any] {
    let appearance = view.effectiveAppearance
    var record = [String: Any]()
    appearance.performAsCurrentDrawingAppearance {
        view.window?.displayIfNeeded()
        let background = view.window?.backgroundColor ?? NSColor.windowBackgroundColor
        guard let resolvedBackground = background.usingColorSpace(.sRGB)?.withAlphaComponent(1) else {
            require(false, "native window background cannot be resolved"); return
        }
        guard let bitmap = view.bitmapImageRepForCachingDisplay(in: view.bounds) else {
            require(false, "AppKit bitmap unavailable"); return
        }
        view.cacheDisplay(in: view.bounds, to: bitmap)
        guard let image = bitmap.cgImage else {
            require(false, "native capture image unavailable"); return
        }
        // Preserve source pixel dimensions, logical size and RGB color space.
        // Fill before source-over drawing: transparent view/window pixels must
        // not be left to an image viewer's arbitrary white/black background.
        let sourceSpace = image.colorSpace
        let space = sourceSpace?.model == .rgb ? sourceSpace! : CGColorSpace(name: CGColorSpace.sRGB)!
        guard let context = CGContext(data: nil, width: image.width, height: image.height,
                                      bitsPerComponent: 8, bytesPerRow: 0, space: space,
                                      bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else {
            require(false, "opaque capture context unavailable"); return
        }
        let pixels = CGRect(x: 0, y: 0, width: image.width, height: image.height)
        context.setFillColor(resolvedBackground.cgColor)
        context.fill(pixels)
        context.setBlendMode(.normal)
        context.draw(image, in: pixels)
        guard let composite = context.makeImage() else {
            require(false, "opaque capture image unavailable"); return
        }
        let opaque = NSBitmapImageRep(cgImage: composite)
        opaque.size = bitmap.size
        guard let data = opaque.representation(using: .png, properties: [:]) else {
            require(false, "AppKit PNG unavailable"); return
        }
        try! data.write(to: path)
        record = ["file": path.lastPathComponent,
                  "method": "appkit-cache-window-background-composite", "scope": "view",
                  "appearance": appearance.name.rawValue, "opaque": true,
                  "pixels_wide": image.width, "pixels_high": image.height,
                  "logical_width": bitmap.size.width, "logical_height": bitmap.size.height,
                  "window_backing_scale": view.window?.backingScaleFactor ?? 1,
                  "background_source": view.window == nil ? "NSColor.windowBackgroundColor" : "NSWindow.backgroundColor",
                  "background_srgb": [resolvedBackground.redComponent, resolvedBackground.greenComponent,
                                      resolvedBackground.blueComponent, resolvedBackground.alphaComponent]]
    }
    imageCaptureRecords.append(record)
    // Keep provenance available even if a later geometry/behavior assertion
    // stops the harness before its aggregate manifests are written.
    try! JSONSerialization.data(withJSONObject: record, options: [.prettyPrinted, .sortedKeys])
        .write(to: path.deletingPathExtension().appendingPathExtension("capture.json"))
    return record
}
// A visual fixture only. Assertions target the production native surfaces.
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
let app = NSApplication.shared
require(app.setActivationPolicy(.accessory) || app.activationPolicy() == .accessory, "accessory policy unavailable")
app.finishLaunching()
require(NSScreen.main != nil, "native Aqua desktop required")
let fixtureURL = URL(fileURLWithPath: CommandLine.arguments[1])
let proof = URL(fileURLWithPath: CommandLine.arguments[2], isDirectory: true)
let checkClipboard = CommandLine.arguments[3] == "1"
let fixtureData = try! Data(contentsOf: fixtureURL)
struct Case: Decodable {
    let name: String
    let frame: DesktopFrame
    let selected_profile: String?
    let expected_text: [String]?
    let expected_accesses: Int?
    let omission_expected: Bool?
    let expected_inspection_detail: String?
}
let cases = try! JSONDecoder().decode([Case].self, from: fixtureData)
// Exercise the production decoder/validator, not a parallel fixture schema.
let rawCases = try! JSONSerialization.jsonObject(with: fixtureData) as! [[String: Any]]
let valid = rawCases[0]["frame"] as! [String: Any]
func rejects(_ object: [String: Any]) -> Bool {
    let data = try! JSONSerialization.data(withJSONObject: object)
    return (try? decodeDesktopFrame(data)) == nil
}
// Use the production wire entrypoint, including byte limits and malformed JSON.
for data in [Data(), Data("[]".utf8), Data("{".utf8), Data([0xff]),
             Data("{\"kind\":\"refresh\"}".utf8), Data("{\"kind\":\"show\",\"unknown\":null}".utf8)] {
    require((try? decodeDesktopFrame(data)) == nil, "malformed wire frame accepted")
}
// Foundation accepts/transcodes these otherwise-valid JSON encodings by
// default; protocol v4 deliberately accepts UTF-8 only, with no frame copy.
for encoding in [String.Encoding.utf16, .utf16LittleEndian, .utf16BigEndian,
                 .utf32, .utf32LittleEndian, .utf32BigEndian] {
    let encoded = "{\"kind\":\"show\"}".data(using: encoding)!
    require((try? decodeDesktopFrame(encoded)) == nil, "non-UTF8 JSON control frame accepted")
}
let control = Data("{\"kind\":\"show\"}".utf8)
var boundary = control
boundary.append(Data(repeating: 32, count: 65536 - control.count))
require((try? decodeDesktopFrame(boundary)) != nil, "exact wire limit rejected")
boundary.append(32)
require((try? decodeDesktopFrame(boundary)) == nil, "over-limit wire frame accepted")
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
var invalidProtection = valid
var badProtection = valid["protection"] as! [String: Any]
badProtection["requested"] = [true, false, true]
invalidProtection["protection"] = badProtection
require(rejects(invalidProtection), "unbounded protection state accepted")
var invalidLanguage = valid
invalidLanguage["language"] = "unsupported"
require(rejects(invalidLanguage), "unsupported UI language accepted")
var oversizedVersion = valid
oversizedVersion["version"] = String(repeating: "V", count: 65)
require(rejects(oversizedVersion), "unbounded version accepted")
var unknownField = valid
unknownField["details_label"] = "obsolete v2 field"
require(rejects(unknownField), "obsolete/unknown protocol field accepted")
var duplicateControl = valid
duplicateControl["items"] = Array(validRows.dropLast()) + [validRows.first { ($0["action"] as? String) == "mic" }!, validRows.last!]
require(rejects(duplicateControl), "duplicate device control accepted")
var oversizedDiagnostic = valid
oversizedDiagnostic["diagnostics"] = String(repeating: "D", count: 32769)
require(rejects(oversizedDiagnostic), "unbounded expanded diagnostic accepted")
var oversizedSummary = valid
oversizedSummary["summary"] = String(repeating: "S", count: 2049)
require(rejects(oversizedSummary), "expanded diagnostics leaked into compact status")
for key in ["summary", "diagnostics", "limited", "visual", "items", "language", "version",
            "protection", "overview", "preferences", "feedback"] {
    var missing = valid
    missing.removeValue(forKey: key)
    require(rejects(missing), "required v4 field missing: \(key)")
}
for kind in ["show", "stop"] {
    require((try? JSONDecoder().decode(DesktopFrame.self, from: Data("{\"kind\":\"\(kind)\"}".utf8))) != nil,
            "kind-only control frame rejected")
    for key in ["overview", "preferences", "feedback", "summary"] {
        let object: [String: Any] = ["kind": kind, key: NSNull()]
        let data = try! JSONSerialization.data(withJSONObject: object)
        require((try? JSONDecoder().decode(DesktopFrame.self, from: data)) == nil,
                "control frame accepted state field \(key)")
    }
}
func badOverview(_ change: (inout [String: Any]) -> Void, _ message: String) {
    var object = valid
    var overview = object["overview"] as! [String: Any]
    change(&overview)
    object["overview"] = overview
    require(rejects(object), message)
}
badOverview({ $0["microphone"] = "safe" }, "invented observation accepted")
badOverview({ $0["total_accesses"] = -1 }, "negative total accepted")
badOverview({ $0["omitted_accesses"] = 999 }, "inconsistent omission count accepted")
badOverview({ $0["observed_at"] = "2026-01-01T12:00:00+01:00" }, "non-UTC timestamp accepted")
badOverview({ $0["observed_at"] = "not-a-time" }, "malformed timestamp accepted")
badOverview({ $0["observed_at"] = NSNull() }, "active observation without time accepted")
badOverview({ $0["obsolete"] = true }, "unknown overview field accepted")
badOverview({ $0["observed_at"] = "2026-02-30T12:00:00Z" }, "impossible calendar date accepted")
badOverview({ $0["observed_at"] = String(repeating: "0", count: 65) }, "unbounded timestamp accepted")
for stamp in ["2026-10-22T12:00:00.999999999Z", "2026-10-22T23:59:59.999999999+00:00",
              "2024-02-29T12:00:00Z"] {
    var object = valid
    var overview = object["overview"] as! [String: Any]
    overview["observed_at"] = stamp
    object["overview"] = overview
    require(!rejects(object), "valid UTC timestamp rejected: \(stamp)")
}
badOverview({ $0["omitted_accesses"] = -1 }, "negative omission accepted")
for pid in [-1, Int(UInt32.max) + 1] {
    badOverview({
        var rows = $0["accesses"] as! [[String: Any]]
        rows[0]["pid"] = pid
        $0["accesses"] = rows
    }, "out-of-u32 pid accepted")
}
for key in ["application", "resource", "activity", "pid"] {
    badOverview({
        var rows = $0["accesses"] as! [[String: Any]]
        rows[0].removeValue(forKey: key)
        $0["accesses"] = rows
    }, "missing access field accepted: \(key)")
}
for section in ["overview", "preferences", "protection"] {
    let nested = valid[section] as! [String: Any]
    for key in nested.keys {
        var object = valid, missing = nested
        missing.removeValue(forKey: key)
        object[section] = missing
        require(rejects(object), "missing nested field accepted: \(section).\(key)")
    }
    var object = valid, extra = nested
    extra["obsolete"] = true
    object[section] = extra
    require(rejects(object), "unknown nested field accepted: \(section)")
}
badOverview({
    var rows = $0["accesses"] as! [[String: Any]]
    rows[0]["obsolete"] = true
    $0["accesses"] = rows
}, "unknown access field accepted")
let validAccess = (valid["overview"] as! [String: Any])["accesses"] as! [[String: Any]]
for (key, value) in [("application", String(repeating: "界", count: 86)), ("resource", "screen"), ("activity", "idle")] {
    badOverview({
        var rows = validAccess
        rows[0][key] = value
        $0["accesses"] = rows
    }, "invalid access \(key) accepted")
}
badOverview({
    $0["accesses"] = Array(repeating: validAccess[0], count: 9)
    $0["total_accesses"] = 9
    $0["omitted_accesses"] = 0
}, "unbounded access list accepted")
var badFeedback = valid
badFeedback["feedback"] = String(repeating: "界", count: 683)
require(rejects(badFeedback), "unbounded UTF8 feedback accepted")
var feedbackBoundary = valid
feedbackBoundary["feedback"] = String(repeating: "界", count: 682) + "AB"
require(!rejects(feedbackBoundary), "exact UTF8 feedback limit rejected")
var accessBoundary = valid
var accessOverview = valid["overview"] as! [String: Any]
var accessBoundaryRows = validAccess
accessBoundaryRows[0]["application"] = String(repeating: "界", count: 85) + "A"
accessBoundaryRows[0]["pid"] = Int(UInt32.max)
accessOverview["accesses"] = accessBoundaryRows
accessBoundary["overview"] = accessOverview
require(!rejects(accessBoundary), "exact UTF8 application/u32 pid limit rejected")
var badPreferences = valid
var preferenceFields = valid["preferences"] as! [String: Any]
preferenceFields["sound_enabled"] = 1
badPreferences["preferences"] = preferenceFields
require(rejects(badPreferences), "nonboolean preference accepted")
preferenceFields["sound_enabled"] = true
preferenceFields["toggle"] = true
badPreferences["preferences"] = preferenceFields
require(rejects(badPreferences), "unknown preference accepted")
var projection = valid
projection["diagnostics"] = String(repeating: "D", count: 32768)
projection["items"] = validRows.map { row -> [String: Any] in
    var row = row
    row["detail"] = String(repeating: "L", count: 16000)
    return row
}
require(rejects(projection), "unbounded total text projection accepted")
projection = valid
projection["diagnostics"] = String(repeating: "\u{0000}", count: 6000)
require(rejects(projection), "escaped JSON text projection not bounded")
// Keep this boundary below the wire limit so text accounting is exercised.
var exactProjection = valid
exactProjection["summary"] = ""
exactProjection["diagnostics"] = String(repeating: "D", count: 32768)
exactProjection["feedback"] = NSNull()
var exactOverview = valid["overview"] as! [String: Any]
var exactAccesses = validAccess
exactAccesses[0]["application"] = ""
exactOverview["accesses"] = exactAccesses
exactProjection["overview"] = exactOverview
var exactRows = validRows.map { row -> [String: Any] in
    var row = row
    row["label"] = ""
    row.removeValue(forKey: "detail")
    if let children = row["children"] as? [[String: Any]] {
        row["children"] = children.map { child -> [String: Any] in
            var child = child
            child["label"] = ""
            child.removeValue(forKey: "detail")
            return child
        }
    }
    return row
}
let detailBytes = 48000 - 32768 - ("2026-10-22T12:00:00Z".utf8.count)
exactRows[0]["detail"] = String(repeating: "L", count: detailBytes)
exactProjection["items"] = exactRows
require(!rejects(exactProjection), "exact text projection limit rejected")
exactRows[0]["detail"] = String(repeating: "L", count: detailBytes + 1)
exactProjection["items"] = exactRows
require(try! JSONSerialization.data(withJSONObject: exactProjection).count < 65536,
        "projection regression fixture accidentally exceeds wire limit")
require(rejects(exactProjection), "over-limit text projection accepted")
var measurements = [[String: Any]]()
var captures = [[String: Any]]()
func descendants(_ root: NSView) -> [NSView] {
    [root] + root.subviews.flatMap { descendants($0) }
}
func captureSections(_ consultation: DesktopConsultation, _ name: String) {
    let window = consultation.window
    let simulated = NSTextField(labelWithString: "SIMULATED DATA · Native AppKit UI proof, not hardware evidence")
    simulated.font = .systemFont(ofSize: 10)
    simulated.textColor = .secondaryLabelColor
    simulated.alignment = .right
    for appearance in [NSAppearance.Name.aqua, .darkAqua,
                       .accessibilityHighContrastAqua, .accessibilityHighContrastDarkAqua] {
        guard let nativeAppearance = NSAppearance(named: appearance) else { continue }
        window.appearance = nativeAppearance
        for (sizeName, size) in [("user", NSSize(width: 645, height: 570)), ("minimum", window.minSize),
                                 ("roomy", NSSize(width: 900, height: 700))] {
            let requestedContent = window.contentRect(forFrameRect: NSRect(origin: window.frame.origin, size: size)).size
            let contentMinimum = window.contentMinSize
            require(size.width >= window.minSize.width - 1 && size.height >= window.minSize.height - 1 &&
                    requestedContent.width >= contentMinimum.width - 1 &&
                    requestedContent.height >= contentMinimum.height - 1,
                    "fixture \(sizeName) frame \(size) is below declared frame/content minimum \(window.minSize)/\(contentMinimum)")
            window.setFrame(NSRect(origin: window.frame.origin, size: size), display: true)
            for index in 0..<3 {
                consultation.tabs.selectTabViewItem(at: index)
                window.contentView!.layoutSubtreeIfNeeded()
                window.displayIfNeeded()
                let root = window.contentView!
                require(abs(window.frame.width - size.width) <= 1 && abs(window.frame.height - size.height) <= 1,
                        "native pane enlarged the requested \(sizeName) window: requested \(size), actual \(window.frame.size)")
                require(root.bounds.width > 0 && root.bounds.height > 0, "empty native surface")
                guard let scroll = consultation.tabs.selectedTabViewItem?.view as? NSScrollView,
                      let document = scroll.documentView else {
                    require(false, "native pane has no scrollable document"); return
                }
                require(document.bounds.width <= scroll.contentView.bounds.width + 1,
                        "pane content extends beyond scroll viewport")
                let controls = descendants(document).compactMap { $0 as? NSControl }
                    .filter { !($0 is NSScroller) && !$0.isHiddenOrHasHiddenAncestor }
                require(!controls.isEmpty, "pane has no native content")
                for control in controls {
                    let rectangle = control.convert(control.bounds, to: document)
                    require(rectangle.width > 0 && rectangle.height > 0 &&
                            rectangle.minX >= -1 && rectangle.maxX <= document.bounds.width + 1,
                            "native content clipped horizontally: \(control.identifier?.rawValue ?? control.description)")
                    require(rectangle.minY >= -1 && rectangle.maxY <= document.bounds.height + 1,
                            "native control extends beyond scrollable document")
                    for y in [rectangle.minY, rectangle.maxY - 1] {
                        let edge = NSRect(x: rectangle.midX, y: y, width: 1, height: 1)
                        _ = document.scrollToVisible(edge)
                        require(document.visibleRect.intersects(edge),
                                "native control edge is unreachable by scrolling")
                    }
                }
                if index == 2 {
                    let textScroll = consultation.text.enclosingScrollView!
                    require(textScroll !== scroll && textScroll.bounds.height >= 119 &&
                            textScroll.bounds.width > 0, "diagnostic text viewport is unusable")
                    let copy = controls.first { $0.identifier?.rawValue == "copy_diagnostics" }
                    require(copy != nil && copy!.isEnabled, "native Copy control unavailable")
                    _ = copy!.scrollToVisible(copy!.bounds)
                    require(copy!.visibleRect.height >= copy!.bounds.height - 1 &&
                            copy!.visibleRect.width >= copy!.bounds.width - 1,
                            "diagnostic Copy control cannot be fully reached")
                    if name == "long-inspection-label" && sizeName == "minimum" {
                        let inspection = controls.first { $0.identifier?.rawValue == "camera_inspect" } as? NSButton
                        require(inspection != nil && inspection!.title.utf8.count > 1000 &&
                                inspection!.bounds.height > scroll.contentView.bounds.height,
                                "long inspection fixture did not retain its wrapped title at the requested minimum size")
                        require(document.bounds.height > scroll.contentView.bounds.height,
                                "long inspection fixture did not exercise diagnostic overflow: document \(document.bounds.size), viewport \(scroll.contentView.bounds.size)")
                        png(root, proof.appendingPathComponent("\(name)-\(appearance.rawValue)-minimum-diagnostics-bottom-simulated.png"))
                    }
                }
                scroll.contentView.scroll(to: .zero)
                scroll.reflectScrolledClipView(scroll.contentView)
                if index == 1 {
                    require(consultation.notifications.window === window && consultation.profile.window === window,
                            "settings not attached to actual window")
                    require(consultation.notifications.enclosingScrollView?.hasVerticalScroller == true,
                            "settings cannot scroll at smaller sizes")
                }
                simulated.frame = NSRect(x: 16, y: root.bounds.height - 14, width: root.bounds.width - 32, height: 12)
                simulated.removeFromSuperview()
                root.addSubview(simulated, positioned: .above, relativeTo: nil)
                let capture = png(root, proof.appendingPathComponent("\(name)-\(appearance.rawValue)-\(sizeName)-\(index)-simulated.png"))
                captures.append(["case": name, "appearance": appearance.rawValue, "size": sizeName,
                                 "section": index, "window_width": window.frame.width,
                                 "window_height": window.frame.height, "requested_window_width": size.width,
                                 "requested_window_height": size.height,
                                 "document_height": document.bounds.height,
                                 "viewport_height": scroll.contentView.bounds.height,
                                 "capture": capture, "simulated": true])
            }
        }
    }
    window.appearance = nil
    simulated.removeFromSuperview()
}
for test in cases {
    let frame = test.frame
    let entries = frame.items!
    require(validDesktopState(frame), "fixture rejected by production protocol consumer")
    let consultation = DesktopConsultation()
    let content = DesktopMenu(frame: frame, consultation: consultation)
    if let expected = test.expected_text, let accesses = test.expected_accesses {
        require(frame.overview!.total_accesses == accesses, "producer lost total observed access count")
        for text in expected {
            require(frame.diagnostics!.contains(text) && content.diagnostic.contains(text),
                    "real Rust producer lost an access identity or unabridged normal path")
        }
        require(frame.limited == test.omission_expected, "consumer lost explicit producer omission state")
        let raw = rawCases.first { ($0["name"] as? String) == test.name }!["frame"]!
        require(try! JSONSerialization.data(withJSONObject: raw).count <= 65536, "producer frame exceeds wire limit")
        if test.name == "producer-long-accesses" {
            require(frame.diagnostics!.utf8.count > 16384, "producer did not cross old fatal field limit")
        }
        if let detail = test.expected_inspection_detail {
            require(content.diagnostic.contains(detail), "passive recovery lost historical inspection detail")
            require(frame.protection!.camera == "system_managed", "passive recovery did not publish known camera state")
            require(!entries.contains(where: { $0.action == "camera_inspect" }), "passive recovery retained stale admin action")
            require(frame.feedback == "Unrelated earlier action failed", "passive recovery reset unrelated action feedback")
        }
    }
    let menu = content.menu
    func inspectRows(_ entries: [DesktopItem], _ native: NSMenu, parentEnabled: Bool = true) {
        let rows = native.items.filter { $0.representedObject != nil || $0.submenu != nil }
        require(rows.count == entries.count, "native menu lost a producer item")
        for (entry, row) in zip(entries, rows) {
            require(row.isEnabled == entry.enabled, "native menu capability gate lost")
            require((row.title as NSString).size(withAttributes: [.font: menu.font!]).width <= 340.5, "title exceeds native font bound")
            require(!row.title.contains("\n"), "visible title must be one line")
            require(content.diagnostic.contains(entry.detail ?? entry.label), "original diagnostic lost")
            if let children = entry.children {
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
    let direct = entries.filter { !["header", "status", "camera_inspect", "autostart"].contains($0.action) }
    inspectRows(direct, menu)
    for entry in entries {
        require(content.diagnostic.contains(entry.detail ?? entry.label), "moving a row lost its full diagnostic")
    }
    require(!menu.items.contains { ["header", "status", "camera_inspect", "autostart"].contains(($0.representedObject as? String) ?? "") },
            "advanced/duplicate rows remain in tray")
    require(content.diagnostic.contains(frame.summary!), "full status summary lost")
    require(menu.size.width <= 400, "actual NSMenu intrinsic width exceeds 400 points")
    var tracked = false
    let cancel = Timer(timeInterval: 0.15, repeats: false) { _ in
        require(menu.size.width <= 400, "tracked NSMenu exceeds width bound")
        tracked = true
        if test.name == "en" && CGPreflightScreenCaptureAccess() {
            let screenshot = Process()
            screenshot.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
            screenshot.arguments = ["-x", proof.appendingPathComponent("en-native-menu-simulated.png").path]
            do {
                try screenshot.run()
                screenshot.waitUntilExit()
                try! String(screenshot.terminationStatus).write(
                    to: proof.appendingPathComponent("native-menu-screenshot.exit"), atomically: true, encoding: .utf8)
            } catch {
                try! error.localizedDescription.write(
                    to: proof.appendingPathComponent("native-menu-screenshot-unavailable.txt"), atomically: true, encoding: .utf8)
            }
        } else if test.name == "en" {
            try! "UNAVAILABLE: no preexisting Screen Recording authorization; no request was made.\n".write(
                to: proof.appendingPathComponent("native-menu-screenshot-unavailable.txt"), atomically: true, encoding: .utf8)
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
    png(fixture, proof.appendingPathComponent(test.name + "-menu-text-fixture-simulated.png"))
    window.orderOut(nil)
    let open = menu.items[0]
    let about = menu.items[menu.items.count - 2]
    let localized = DesktopWords(frame.language!)
    require((menu.items.last!.representedObject as? String) == "exit", "Exit must remain last")
    require(open.title == localized.open && about.title == localized.about, "localized consultation/About inaccessible")
    require(open.representedObject == nil && about.representedObject == nil, "local UI became a Rust/device action")
    require(NSApp.sendAction(open.action!, to: open.target, from: open), "consultation selector unavailable")
    let nativeWindow = consultation.window
    let identity = nativeWindow.windowNumber
    require(nativeWindow.isVisible && nativeWindow.styleMask.contains(.resizable), "consultation is not a real resizable window")
    require(NSApp.modalWindow == nil, "consultation must be modeless")
    let text = consultation.text
    require(text.string == content.diagnostic, "consultation lost full diagnostic")
    require(!text.isEditable && text.isSelectable, "consultation must be read-only and copyable")
    consultation.tabs.selectTabViewItem(at: 0)
    let overviewLabels = descendants(consultation.tabs.tabViewItems[0].view!).compactMap { $0 as? NSTextField }
    func overviewLabel(_ id: String) -> NSTextField {
        let label = overviewLabels.first { $0.identifier?.rawValue == id }
        require(label != nil, "native overview fact missing: \(id)")
        return label!
    }
    let observation = frame.overview!, protection = frame.protection!
    for (resource, state) in [("microphone", observation.microphone), ("camera", observation.camera)] {
        if state == "unknown" {
            require(overviewLabel("observation_" + resource).stringValue.contains(localized.unknown),
                    "unknown observation became safe or idle")
        }
    }
    let expectedMic: String
    switch protection.microphone {
    case "muted": expectedMic = localized.muted
    case "unmuted": expectedMic = localized.unmuted
    case "mixed": expectedMic = localized.mixed
    default: expectedMic = localized.unknown
    }
    let expectedCamera: String
    switch protection.camera {
    case "owned": expectedCamera = localized.restricted
    case "absent": expectedCamera = localized.unrestricted
    case "system_managed": expectedCamera = localized.systemManaged
    default: expectedCamera = localized.unknown
    }
    require(overviewLabel("effective_microphone").stringValue.contains(expectedMic) &&
            overviewLabel("effective_camera").stringValue.contains(expectedCamera),
            "native effective-control cards used requested state instead of observed facts")
    for (resource, requested) in zip(["microphone", "camera"], protection.requested) {
        let fact = requested.map { $0 ? localized.yes : localized.no } ?? localized.noRequest
        require(overviewLabel("requested_" + resource).stringValue.contains(fact),
                "requested protection was lost or conflated with effective control")
    }
    require(overviewLabel("observed_at").stringValue.contains(observation.observed_at ?? localized.unknown),
            "native observation timestamp lost")
    for (index, access) in observation.accesses.enumerated() {
        require(overviewLabel("access_\(index)").stringValue.contains(access.application),
                "current observed application identity lost")
    }
    require(overviewLabel("omitted_accesses").isHidden == (observation.omitted_accesses == 0),
            "omitted application rows were not disclosed")
    require(consultation.notifications.state == (frame.preferences!.notifications_enabled ? .on : .off) &&
            consultation.sound.state == (frame.preferences!.sound_enabled ? .on : .off) &&
            consultation.history.state == (frame.preferences!.history_enabled ? .on : .off),
            "native preferences do not reflect authoritative producer values")
    // Exercise empty-identity transitions in every UI language without pinning
    // incidental translated wording. Missing identities must never reuse the
    // observed-idle claim, even when all known rows have been omitted.
    var emptyObject = rawCases.first { ($0["name"] as? String) == test.name }!["frame"] as! [String: Any]
    var emptyOverview = emptyObject["overview"] as! [String: Any]
    emptyOverview["microphone"] = "idle"
    emptyOverview["camera"] = "idle"
    emptyOverview["observed_at"] = "2026-10-22T12:00:00Z"
    emptyOverview["total_accesses"] = 0
    emptyOverview["omitted_accesses"] = 0
    emptyOverview["accesses"] = [[String: Any]]()
    func renderEmptyObservation() {
        emptyObject["overview"] = emptyOverview
        let emptyFrame = try! decodeDesktopFrame(JSONSerialization.data(withJSONObject: emptyObject))
        consultation.update(emptyFrame, diagnostic: content.diagnostic)
        require(!overviewLabel("access_empty").isHidden, "empty identities have no visible explanation")
    }
    renderEmptyObservation()
    let idleEmptyMessage = overviewLabel("access_empty").stringValue
    require(!idleEmptyMessage.isEmpty, "idle observation has no empty-state explanation")
    for resource in ["microphone", "camera"] {
        for activity in ["active", "ready"] {
            emptyOverview[resource] = activity
            renderEmptyObservation()
            require(!overviewLabel("access_empty").stringValue.isEmpty &&
                    overviewLabel("access_empty").stringValue != idleEmptyMessage,
                    "observed activity without identities incorrectly claims no access")
        }
        emptyOverview[resource] = "idle"
    }
    emptyOverview["total_accesses"] = 3
    emptyOverview["omitted_accesses"] = 3
    renderEmptyObservation()
    require(overviewLabel("access_empty").stringValue != idleEmptyMessage &&
            !overviewLabel("omitted_accesses").isHidden &&
            overviewLabel("omitted_accesses").stringValue.contains("3"),
            "omitting every identity incorrectly claims no access or hides the omitted count")
    consultation.update(frame, diagnostic: content.diagnostic)
    captureSections(consultation, test.name)
    consultation.tabs.selectTabViewItem(at: 2)
    nativeWindow.makeFirstResponder(text)
    let selectedTab = consultation.tabs.selectedTabViewItem
    let responder = nativeWindow.firstResponder
    text.setSelectedRange(NSRange(location: 0, length: min(12, (text.string as NSString).length)))
    let selection = text.selectedRanges
    text.enclosingScrollView!.contentView.scroll(to: NSPoint(x: 0, y: 30))
    let scrollOrigin = text.enclosingScrollView!.contentView.bounds.origin
    let diagnosticPaneScroll = consultation.tabs.tabViewItems[2].view as! NSScrollView
    diagnosticPaneScroll.contentView.scroll(to: NSPoint(x: 0, y: 30))
    let diagnosticPaneOrigin = diagnosticPaneScroll.contentView.bounds.origin
    if test.name == "long-inspection-label" {
        require(diagnosticPaneOrigin.y > 0, "diagnostic refresh fixture did not scroll the overflowing pane")
    }
    consultation.update(frame, diagnostic: content.diagnostic + "\nSimulated refresh")
    require(consultation.tabs.selectedTabViewItem === selectedTab && nativeWindow.firstResponder === responder,
            "refresh stole tab or keyboard focus")
    require(text.selectedRanges == selection && text.enclosingScrollView!.contentView.bounds.origin == scrollOrigin &&
            diagnosticPaneScroll.contentView.bounds.origin == diagnosticPaneOrigin,
            "refresh lost diagnostic selection, text scroll or outer pane scroll")
    consultation.update(frame, diagnostic: content.diagnostic)
    consultation.tabs.selectTabViewItem(at: 1)
    nativeWindow.makeFirstResponder(consultation.notifications)
    let settingsResponder = nativeWindow.firstResponder
    consultation.update(frame, diagnostic: content.diagnostic)
    require(consultation.tabs.selectedTabViewItem === consultation.tabs.tabViewItems[1] &&
            nativeWindow.firstResponder === settingsResponder, "refresh stole settings focus")
    // A changed authoritative preference snapshot updates retained controls,
    // rather than leaving a locally optimistic checkbox value behind.
    var changedObject = rawCases.first { ($0["name"] as? String) == test.name }!["frame"] as! [String: Any]
    var changedPreferences = changedObject["preferences"] as! [String: Any]
    changedPreferences["notifications_enabled"] = !frame.preferences!.notifications_enabled
    changedPreferences["sound_enabled"] = !frame.preferences!.sound_enabled
    changedPreferences["history_enabled"] = !frame.preferences!.history_enabled
    changedObject["preferences"] = changedPreferences
    let changedFrame = try! decodeDesktopFrame(JSONSerialization.data(withJSONObject: changedObject))
    consultation.update(changedFrame, diagnostic: content.diagnostic)
    require(consultation.notifications.state != (frame.preferences!.notifications_enabled ? .on : .off) &&
            consultation.sound.state != (frame.preferences!.sound_enabled ? .on : .off) &&
            consultation.history.state != (frame.preferences!.history_enabled ? .on : .off),
            "authoritative preference refresh did not update retained controls")
    require(nativeWindow.firstResponder === settingsResponder, "preference refresh stole focus")
    var unavailableObject = changedObject
    var unavailableOverview = unavailableObject["overview"] as! [String: Any]
    unavailableOverview["microphone"] = "unknown"
    unavailableOverview["camera"] = "unknown"
    unavailableOverview["observed_at"] = NSNull()
    unavailableOverview["total_accesses"] = 0
    unavailableOverview["omitted_accesses"] = 0
    unavailableOverview["accesses"] = [[String: Any]]()
    unavailableObject["overview"] = unavailableOverview
    let unavailableFrame = try! decodeDesktopFrame(JSONSerialization.data(withJSONObject: unavailableObject))
    consultation.update(unavailableFrame, diagnostic: "SIMULATED unavailable observation")
    require(overviewLabel("observation_microphone").stringValue.contains(localized.unknown) &&
            overviewLabel("observation_camera").stringValue.contains(localized.unknown) &&
            !overviewLabel("access_empty").isHidden &&
            (0..<8).allSatisfy { overviewLabel("access_\($0)").isHidden },
            "failed refresh retained stale observed applications or converted unknown to idle")
    require(nativeWindow.firstResponder === settingsResponder, "observation failure stole settings focus")
    consultation.update(frame, diagnostic: content.diagnostic)
    for (name, button) in [("notifications", consultation.notifications), ("sound", consultation.sound), ("history", consultation.history)] {
        for state in [NSControl.StateValue.on, .off] {
            button.state = state == .on ? .off : .on
            button.performClick(nil)
            require(button.state == state, "native preference checkbox did not adopt desired value: \(name)")
        }
    }
    let profileEntry = entries.first { $0.action == "profile" }!
    require(consultation.profile.isEnabled == profileEntry.enabled, "explicit config profile gate lost")
    require((consultation.profile.selectedItem?.representedObject as? String) == test.selected_profile,
            "authoritative selected profile was not retained through refresh")
    for (index, child) in profileEntry.children!.enumerated() {
        consultation.profile.selectItem(at: index)
        require(NSApp.sendAction(consultation.profile.action!, to: consultation.profile.target, from: consultation.profile),
                "native profile selector unavailable")
        require(consultation.profile.item(at: index)?.isEnabled == (profileEntry.enabled && child.enabled),
                "profile child capability gate lost")
    }
    consultation.tabs.selectTabViewItem(at: 2)
    nativeWindow.makeFirstResponder(text)
    func windowCommand(_ characters: String, _ code: UInt16) -> Bool {
        let event = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [.command],
            timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: nativeWindow.windowNumber,
            context: nil, characters: characters, charactersIgnoringModifiers: characters,
            isARepeat: false, keyCode: code)!
        return nativeWindow.performKeyEquivalent(with: event)
    }
    for (key, index) in [("1", 0), ("2", 1), ("3", 2)] {
        require(windowCommand(key, 0) &&
                consultation.tabs.selectedTabViewItem === consultation.tabs.tabViewItems[index],
                "keyboard section navigation unavailable")
    }
    nativeWindow.makeFirstResponder(text)
    require(windowCommand("a", 0) && text.selectedRange().length == (content.diagnostic as NSString).length,
            "consultation keyboard Select All unavailable")
    if checkClipboard {
        require(windowCommand("c", 8), "consultation keyboard Copy unavailable")
        require(NSPasteboard.general.string(forType: .string) == content.diagnostic, "copy lost original diagnostic")
        let copy = descendants(consultation.tabs.tabViewItems[2].view!).compactMap { $0 as? NSButton }
            .first { $0.identifier?.rawValue == "copy_diagnostics" }!
        require(NSApp.sendAction(copy.action!, to: copy.target, from: copy), "diagnostic copy button unavailable")
        require(NSPasteboard.general.string(forType: .string) == content.diagnostic, "diagnostic copy button lost technical text")
        NSPasteboard.general.clearContents()
        text.setSelectedRange(NSRange(location: 0, length: 0))
    }
    text.setSelectedRange(NSRange(location: 0, length: 0))
    for entry in entries where ["mic", "camera", "pause", "camera_inspect", "autostart"].contains(entry.action) {
        func findButton(_ root: NSView) -> NSButton? {
            if let button = root as? NSButton, button.identifier?.rawValue == entry.action { return button }
            return root.subviews.compactMap { findButton($0) }.first
        }
        let button = consultation.tabs.tabViewItems.compactMap { $0.view }.compactMap { findButton($0) }.first
        require(button != nil && button!.title == entry.label && button!.isEnabled == entry.enabled,
                "advanced control was lost instead of moved to consultation")
        require(NSApp.sendAction(button!.action!, to: button!.target, from: button!), "native control selector unavailable")
    }
    require(windowCommand("w", 13), "consultation keyboard Close unavailable")
    require(!nativeWindow.isVisible && !menu.items.isEmpty, "closing consultation damaged retained menu")
    let hiddenDiagnostic = text.string
    consultation.update(frame, diagnostic: content.diagnostic + "\nSimulated hidden refresh")
    require(text.string == hiddenDiagnostic, "hidden refresh redundantly rendered the diagnostic document")
    require(!nativeWindow.isVisible, "hidden refresh unexpectedly opened consultation")
    require(NSApp.sendAction(open.action!, to: open.target, from: open), "reopen selector unavailable")
    require(consultation.window === nativeWindow && nativeWindow.windowNumber == identity && nativeWindow.isVisible,
            "reopening created a duplicate consultation")
    require(NSApp.windows.filter { $0 === nativeWindow }.count == 1, "duplicate consultation window")
    require(text.string == content.diagnostic + "\nSimulated hidden refresh",
            "reopen did not render the latest hidden snapshot")
    nativeWindow.performClose(nil)
    let alert = content.aboutAlert()
    require(alert.messageText == "MicCamWatch \(frame.version!)" && alert.informativeText == localized.description,
            "About lost version or factual localized description")
    var inspected = false
    let inspect = Timer(timeInterval: 0.2, repeats: false) { _ in
        guard let root = NSApp.modalWindow?.contentView else { require(false, "native About did not render"); return }
        png(root, proof.appendingPathComponent(test.name + "-about-simulated.png"))
        inspected = true
        NSApp.abortModal()
        NSApp.modalWindow?.orderOut(nil)
    }
    RunLoop.main.add(inspect, forMode: .modalPanel)
    RunLoop.main.add(inspect, forMode: .common)
    require(NSApp.sendAction(about.action!, to: about.target, from: about), "About selector unavailable")
    require(inspected, "About did not open native UI")
    try! content.diagnostic.write(to: proof.appendingPathComponent(test.name + "-full-diagnostic.txt"), atomically: true, encoding: .utf8)
    measurements.append(["case": test.name, "menu_width_points": menu.size.width,
                         "screen_width_points": screen.width, "visual": frame.visual!,
                         "titles": menu.items.map { $0.title },
                         "diagnostic_utf8_bytes": content.diagnostic.utf8.count,
                         "native_menu_tracking": tracked, "native_consultation_copy": checkClipboard,
                         "native_sections": ["overview", "settings", "diagnostics"],
                         "singleton_reopen_close": true, "native_about": inspected])
}
// Exercise actual AppKit keyboard tracking, not direct selector forwarding.
let keyboardContent = DesktopMenu(frame: cases[0].frame, consultation: DesktopConsultation())
let keyboardMenu = keyboardContent.menu
let profileRow = keyboardMenu.items.firstIndex { $0.submenu != nil }!
let preceding = keyboardMenu.items[..<profileRow].filter { $0.isEnabled && !$0.isSeparatorItem }.count
func postMenuKey(_ characters: String, _ code: UInt16) {
    let event = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [],
        timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: 0, context: nil,
        characters: characters, charactersIgnoringModifiers: characters, isARepeat: false, keyCode: code)!
    NSApp.postEvent(event, atStart: false)
}
let down = String(UnicodeScalar(UInt32(NSDownArrowFunctionKey))!)
let right = String(UnicodeScalar(UInt32(NSRightArrowFunctionKey))!)
let home = String(UnicodeScalar(UInt32(NSHomeFunctionKey))!)
let navigate = Timer(timeInterval: 0.2, repeats: false) { _ in
    postMenuKey(home, 115)
    for _ in 0..<preceding { postMenuKey(down, 125) }
    postMenuKey(right, 124)
    postMenuKey(home, 115)
    postMenuKey(down, 125)
    postMenuKey(down, 125)
    postMenuKey("\r", 36)
}
var keyboardTimedOut = false
let keyboardDeadline = Timer(timeInterval: 3, repeats: false) { _ in
    keyboardTimedOut = true
    keyboardMenu.cancelTracking()
}
for timer in [navigate, keyboardDeadline] {
    RunLoop.main.add(timer, forMode: .eventTracking)
    RunLoop.main.add(timer, forMode: .common)
}
let keyboardScreen = NSScreen.main!.visibleFrame
keyboardMenu.popUp(positioning: nil, at: NSPoint(x: keyboardScreen.midX, y: keyboardScreen.midY), in: nil)
keyboardDeadline.invalidate()
require(!keyboardTimedOut, "keyboard navigation did not activate a direct profile choice")
try! Data("{\"requested_action\":\"profile_meeting\",\"app_local_keyboard_tracking\":true}\n".utf8)
    .write(to: proof.appendingPathComponent("keyboard-profile.json"))
require(verifyTrackedMenuUpdate(cases[0].frame, cases.first { $0.name == "producer-passive-recovery" }!.frame), "state refresh broke a tracked menu action")
for appearance in [NSAppearance.Name.aqua, .darkAqua] {
    NSApp.appearance = NSAppearance(named: appearance)
    var rendered = Set<Data>()
    for visual in ["idle", "ready", "active", "partial", "error"] {
        let image = desktopStatusImage(visual)
        require(image.isTemplate, "status mark must adapt to light/dark appearance")
        for scale in [1, 2] {
            let view = NSImageView(frame: NSRect(x: 0, y: 0, width: image.size.width * CGFloat(scale), height: image.size.height * CGFloat(scale)))
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
try! JSONSerialization.data(withJSONObject: captures, options: [.prettyPrinted, .sortedKeys])
    .write(to: proof.appendingPathComponent("native-surface-captures.json"))
try! JSONSerialization.data(withJSONObject: imageCaptureRecords, options: [.prettyPrinted, .sortedKeys])
    .write(to: proof.appendingPathComponent("native-image-captures.json"))
'''

TRACKING_REGRESSION = r'''
// Appended only to a private copy of the production Swift file: same-file
// extension can exercise the actual delegate's private frame/update/tracking.
private extension DesktopDelegate {
    func verifyTrackedUpdate(_ frame: DesktopFrame, _ recovery: DesktopFrame) -> Bool {
        item = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        defer { if let item = item { NSStatusBar.system.removeStatusItem(item) } }
        apply(frame)
        let previousAppearance = NSApp.appearance
        var statusGeometry = [[String: Any]]()
        for appearance in [NSAppearance.Name.aqua, .darkAqua] {
            NSApp.appearance = NSAppearance(named: appearance)
            for visual in ["idle", "ready", "active", "partial", "error"] {
                guard let button = item?.button else { require(false, "native status button unavailable"); return false }
                button.appearance = NSAppearance(named: appearance)
                button.image = desktopStatusImage(visual)
                button.needsDisplay = true
                RunLoop.main.run(until: Date().addingTimeInterval(0.03))
                guard let image = button.image else { require(false, "native status image unavailable"); return false }
                require(button.bounds.width >= image.size.width && button.bounds.height >= image.size.height,
                        "real menu-bar button clips the mark or its badge")
                png(button, proof.appendingPathComponent("\(appearance.rawValue)-\(visual)-native-status-button.png"))
                statusGeometry.append(["appearance": appearance.rawValue, "visual": visual,
                                       "buttonWidth": button.bounds.width, "buttonHeight": button.bounds.height,
                                       "imageWidth": image.size.width, "imageHeight": image.size.height])
            }
        }
        try! JSONSerialization.data(withJSONObject: statusGeometry, options: [.prettyPrinted, .sortedKeys])
            .write(to: proof.appendingPathComponent("native-status-button-geometry.json"))
        item?.button?.appearance = nil
        NSApp.appearance = previousAppearance
        apply(frame)
        weak var trackedContent = menuContent
        let trackedMenu = menuContent!.menu
        let row = trackedMenu.items.first { ($0.representedObject as? String) == "mic" }!
        let originalWindow = consultation.window
        showConsultation()
        var delivered = false
        let update = Timer(timeInterval: 0.15, repeats: false) { _ in
            self.apply(recovery)
            require(self.item!.button!.toolTip == recovery.summary!.components(separatedBy: .newlines).first, "status tooltip failed to recover")
            require(self.consultation.window === originalWindow && originalWindow.isVisible,
                    "state refresh replaced or closed the consultation window")
            require(self.consultation.text.string == self.menuContent!.diagnostic, "visible consultation did not refresh")
            require(!self.menuContent!.menu.items.contains { ($0.representedObject as? String) == "camera_inspect" },
                    "recovered inventory still requests administrator inspection")
            require(trackedContent != nil && row.target != nil,
                    "replacing a frame deallocated the tracked menu action target")
            delivered = NSApp.sendAction(row.action!, to: row.target, from: row)
            trackedMenu.cancelTracking()
        }
        RunLoop.main.add(update, forMode: .common)
        showMenu()
        consultation.window.performClose(nil)
        require(item != nil && initialized && !stopping, "window close retired tray/control readiness")
        _ = applicationShouldHandleReopen(NSApp, hasVisibleWindows: false)
        require(consultation.window === originalWindow && originalWindow.isVisible,
                "application reopen did not raise the existing consultation")
        consultation.window.performClose(nil)
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
        ('en', 'State unknown', 'Observation incomplete', 'Session lock unknown'),
        ('fr', 'État inconnu', 'Observation incomplète', 'Verrouillage inconnu'),
        ('de', 'Zustand unbekannt', 'Beobachtung unvollständig', 'Sitzungssperre unbekannt'),
        ('es', 'Estado desconocido', 'Observación incompleta', 'Bloqueo de sesión desconocido'),
        ('ja', '状態不明', '観測範囲が不完全です', 'セッションロック不明'),
        ('zh', '状态未知', '观测范围不完整', '会话锁定状态未知'),
        ('ru', 'Состояние неизвестно', 'Неполное наблюдение', 'Блокировка сеанса неизвестна'),
    )
    controls = {
        'en': ('Mute — writable inputs', 'Approve camera restriction profile',
               'Inspect camera profiles (admin, read-only)', 'Pause alerts (30 min)',
               'Profile', 'Enable autostart', 'Exit', ('Balanced', 'Private', 'Meeting', 'Development')),
        'fr': ('Couper — entrées modifiables', 'Approuver le profil de restriction caméra',
               'Inspecter profils caméra (admin, lecture seule)', 'Suspendre les alertes (30 min)',
               'Profil', 'Activer le démarrage automatique', 'Quitter', ('Équilibré', 'Privé', 'Réunion', 'Développement')),
        'de': ('Stummschalten — nur schreibbare Eingänge', 'Kamerasperrprofil genehmigen',
               'Kameraprofile prüfen (Admin, nur lesen)', 'Hinweise pausieren (30 Min.)',
               'Profil', 'Autostart aktivieren', 'Beenden', ('Ausgewogen', 'Privat', 'Besprechung', 'Entwicklung')),
        'es': ('Silenciar — solo entradas modificables', 'Aprobar perfil de restricción de cámara',
               'Perfiles de cámara (admin, solo lectura)', 'Pausar alertas (30 min)',
               'Perfil', 'Activar inicio automático', 'Salir', ('Equilibrado', 'Privado', 'Reunión', 'Desarrollo')),
        'ja': ('ミュート — 書き込み可能な入力', 'カメラ制限プロファイルを承認',
               'カメラプロファイル（管理者・読み取り専用）', '通知を一時停止 (30 分)',
               'プロファイル', '自動起動を有効化', '終了', ('バランス', 'プライベート', '会議', '開発')),
        'zh': ('静音 — 仅可写输入', '批准摄像头限制描述文件',
               '检查摄像头配置（管理员；只读）', '暂停提醒 (30 分钟)',
               '配置', '启用自动启动', '退出', ('平衡', '隐私', '会议', '开发')),
        'ru': ('Выключить звук — только доступные для записи входы', 'Одобрить профиль ограничения камеры',
               'Профили камер (админ, только чтение)', 'Приостановить уведомления (30 мин)',
               'Профиль', 'Включить автозапуск', 'Выход', ('Сбалансированный', 'Приватный', 'Встреча', 'Разработка')),
    }
    cases = []
    observed = ('Camera restriction status unknown: Current-user profiles contain no camera restriction; '
                'device/other-user scope is not readable, so effective status is unknown. ')
    for lang, unknown, degraded, lock in locales:
        prefix = unknown + ': ' + observed
        diagnostic = prefix + 'W' * (16384 - len(prefix.encode()))
        activity = 'corespeechd: MIC ACTIVE — ' + degraded
        summary = activity + '\n\ncoremediaio_video: Degraded\nRunning-state has no client identity'
        items = [dict(action='header', label='MicCamWatch simulated-data UI proof', enabled=False),
                 dict(action='status', label=activity, detail=summary, enabled=True),
                 dict(action='header', label=unknown, detail=diagnostic, enabled=False),
                 dict(action='header', label=lock, detail=lock + '; automatic controls unavailable', enabled=False)]
        mic, camera, inspect, pause, profile, autostart, exit_label, profiles = controls[lang]
        items.extend(dict(action=action, label=label, enabled=True) for action, label in (
            ('mic', mic), ('camera', camera), ('camera_inspect', inspect), ('pause', pause)))
        choices = [dict(action='profile_' + name, label=label,
                        enabled=True, checked=name == 'meeting')
                   for name, label in zip(('balanced', 'private', 'meeting', 'development'), profiles)]
        items.append(dict(action='profile', label=profile + ': ' + profiles[2], enabled=True, children=choices))
        items.extend(dict(action=action, label=label, enabled=True)
                     for action, label in (('autostart', autostart), ('exit', exit_label)))
        cases.append(dict(name=lang, selected_profile='profile_meeting',
                          frame=dict(kind='state', summary=summary,
                                     visual='partial', items=items, language=lang, version='0.21.0-simulated',
                                     protection=dict(microphone='mixed', camera='unknown', requested=[True, True],
                                                     microphone_on_lock=False, camera_on_lock=False))))
    for name, label in (('unbroken', 'W' * 16384),
                        ('wide-graphemes', ('界👩🏽‍💻e\u0301' * 600)[:6000])):
        case = json.loads(json.dumps(cases[0]))
        case['name'] = name
        case['frame']['items'][2] = dict(action='header', label=label, enabled=False)
        require(len(label.encode()) <= 16384, 'stress fixture exceeds protocol bound')
        cases.append(case)
    for name, visual, mic, camera, requested in (
            ('idle-unprotected', 'idle', 'unmuted', 'absent', [False, False]),
            ('active-protected', 'active', 'muted', 'owned', [True, True]),
            ('system-managed', 'active', 'unmuted', 'system_managed', [None, None]),
            ('unknown-protection', 'error', 'unknown', 'unknown', [None, None])):
        case = json.loads(json.dumps(cases[0]))
        case['name'] = name
        case['frame']['visual'] = visual
        case['frame']['protection'].update(microphone=mic, camera=camera, requested=requested)
        if visual == 'idle':
            case['frame']['summary'] = 'MIC: No observed capture · CAM: No observed capture'
        elif camera != 'unknown':
            case['frame']['summary'] = 'MIC: ACTIVE · CAM: No observed capture\n\ncorespeechd: MIC ACTIVE'
        if camera != 'unknown':
            case['frame']['items'] = [row for index, row in enumerate(case['frame']['items'])
                                     if index not in (2, 3) and row['action'] != 'camera_inspect']
            status = next(row for row in case['frame']['items'] if row['action'] == 'status')
            status.update(label=case['frame']['summary'],
                          detail=case['frame']['summary'] + '\n' + 'Point-in-time device observation. ' * 180)
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
    disabled = json.loads(json.dumps(cases[0]))
    disabled['name'] = 'disabled-capabilities'
    for row in disabled['frame']['items']:
        if row['action'] in ('mic', 'camera', 'camera_inspect', 'pause', 'autostart'):
            row['enabled'] = False
    cases.append(disabled)
    long_settings = json.loads(json.dumps(cases[0]))
    long_settings['name'] = 'long-settings-labels'
    autostart = next(row for row in long_settings['frame']['items'] if row['action'] == 'autostart')
    autostart['label'] += '\n/private/var/' + '界👩🏽‍💻e\u0301-component-' * 80
    cases.append(long_settings)
    for case in cases:
        frame = case['frame']
        frame['diagnostics'] = frame['summary']
        frame['summary'] = frame['summary'].splitlines()[0]
        frame['limited'] = False
        frame['overview'] = dict(microphone='active', camera='unknown',
                                 observed_at='2026-10-22T12:00:00Z',
                                 total_accesses=1, omitted_accesses=0,
                                 accesses=[dict(application='corespeechd (simulated)', resource='microphone',
                                                activity='active', pid=123)])
        if case['name'] == 'idle-unprotected':
            frame['overview'].update(microphone='idle', camera='idle', total_accesses=0, accesses=[])
        if case['name'] == 'unknown-protection':
            frame['overview'].update(microphone='unknown', camera='unknown', observed_at=None, total_accesses=0, accesses=[])
        frame['preferences'] = dict(notifications_enabled=True, sound_enabled=False,
                                    history_enabled=True, notifications_paused=False)
        frame['feedback'] = None
    long_inspection = json.loads(json.dumps(cases[0]))
    long_inspection['name'] = 'long-inspection-label'
    inspection = next(row for row in long_inspection['frame']['items'] if row['action'] == 'camera_inspect')
    inspection['label'] += '\n/private/var/' + '界👩🏽‍💻e\u0301-component-' * 80
    require(len(inspection['label'].encode()) <= 16384, 'inspection fixture exceeds protocol label bound')
    cases.append(long_inspection)
    for name, overrides in (
            ('active-without-identities', dict(microphone='active', camera='idle',
                                            total_accesses=0, omitted_accesses=0, accesses=[])),
            ('ready-without-identities', dict(microphone='idle', camera='ready',
                                           total_accesses=0, omitted_accesses=0, accesses=[])),
            ('all-identities-omitted', dict(microphone='active', camera='ready',
                                         total_accesses=3, omitted_accesses=3, accesses=[])),
            ('ready-camera', dict(microphone='idle', camera='ready', total_accesses=1,
                                 accesses=[dict(application='Camera preview (simulated)',
                                                resource='camera', activity='ready', pid=None)])),
            ('bounded-applications', dict(microphone='active', camera='ready',
                                         total_accesses=12, omitted_accesses=4,
                                         accesses=[dict(application=f'Simulated app {index} · ' + '界' * 40,
                                                        resource='microphone' if index % 2 == 0 else 'camera',
                                                        activity='active' if index % 2 == 0 else 'ready',
                                                        pid=index + 100) for index in range(8)]))):
        case = json.loads(json.dumps(cases[0]))
        case['name'] = name
        case['frame']['overview'].update(overrides)
        if name == 'all-identities-omitted':
            case['frame']['limited'] = True
        if name == 'bounded-applications':
            case['frame']['limited'] = True
            case['frame']['feedback'] = 'SIMULATED failed action: permission was not granted. ' * 8
            case['frame']['preferences'].update(notifications_enabled=False, sound_enabled=True,
                                                history_enabled=False, notifications_paused=True)
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
    with tempfile.TemporaryDirectory(prefix='mcw-native-menu-') as directory:
        work = Path(directory)
        # Build/run only the ignored pure producer fixture: no monitor or
        # microphone/camera/autostart mutations. Its settings roots are private.
        home = work / 'home'
        home.mkdir()
        producer_frames = work / 'rust-producer-frames.json'
        producer_env = dict(os.environ, HOME=str(home),
                            CARGO_HOME=os.environ.get('CARGO_HOME', str(Path.home() / '.cargo')),
                            RUSTUP_HOME=os.environ.get('RUSTUP_HOME', str(Path.home() / '.rustup')),
                            XDG_CONFIG_HOME=str(home / 'config'), XDG_DATA_HOME=str(home / 'data'),
                            XDG_STATE_HOME=str(home / 'state'), XDG_RUNTIME_DIR=str(home / 'runtime'),
                            MCW_NATIVE_MENU_FRAMES=str(producer_frames))
        producer = subprocess.run(
            ['cargo', 'test', '--lib',
             'frontends::tray::menu_tests::native_consultation_producer_frames',
             '--', '--ignored', '--exact'], cwd=repo, env=producer_env,
            capture_output=True, text=True, timeout=900)
        (proof / 'rust-producer.stdout').write_text(producer.stdout)
        (proof / 'rust-producer.stderr').write_text(producer.stderr)
        (proof / 'rust-producer.exit').write_text(str(producer.returncode) + '\n')
        require(producer.returncode == 0 and producer_frames.is_file(),
                'real Rust producer fixture failed: ' + producer.stdout + producer.stderr)
        produced = json.loads(producer_frames.read_text())
        require(len(produced) >= 3, 'real Rust producer cases missing')
        cases.extend(produced)
        fixtures.write_text(json.dumps(cases, ensure_ascii=False, indent=2) + '\n')
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
            result = subprocess.run(argv, capture_output=True, text=True, timeout=300)
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
                elif enabled and row['action'] not in ('header', 'status', 'camera_inspect', 'autostart'):
                    yield dict(kind='action', action=row['action'])
        expected = []
        for case in cases:
            expected.extend(dispatched(case['frame']['items']))
            for preference in ('notifications', 'sound', 'history'):
                expected.extend(dict(kind='action', action=preference + '_' + value) for value in ('enable', 'disable'))
            profile = next(row for row in case['frame']['items'] if row['action'] == 'profile')
            expected.extend(dict(kind='action', action=child['action']) for child in profile['children']
                            if profile['enabled'] and child['enabled'])
            expected.extend(dict(kind='action', action=row['action']) for row in case['frame']['items']
                            if row['action'] in ('mic', 'camera', 'pause', 'camera_inspect', 'autostart') and row['enabled'])
        expected += [dict(kind='action', action='profile_meeting'),
                     dict(kind='ready', protocol=4), dict(kind='action', action='mic')]
        require(events == expected, 'native dispatch lost retained controls or consultation/About emitted a device action')
    (proof / 'scope.txt').write_text(
        'Real production AppKit NSMenu intrinsic width and menu tracking; real enabled action dispatch; '
        'Simulated input data only; no claim of hardware capture coverage. '
        'Real modeless consultation window: Overview/Settings/Diagnostics light/dark/high-contrast at 645x570, minimum and roomy sizes, '
        'read-only full diagnostic copy, refresh tab/focus/selection/scroll retention, close and singleton reopen; '
        'localized native About with factual description and version in seven languages. '
        'Consultation/About PNGs use appearance-scoped caching of the actual AppKit views, '
        'composited opaquely over the resolved native window background; method, appearance and pixel/logical dimensions are recorded in per-image capture.json sidecars and native-image-captures.json. '
        'Menu-text PNGs are rendered fixtures, not native menu screen captures. '
        'App-local keyboard navigation selects a direct profile through real NSMenu tracking. '
        'No Accessibility/global-event-posting/TCC grants or capture/device actions. '
        'Native tracked-menu screen capture is optional and uses only existing authorization.\n')
    print('PASS native consultation, singleton reopen/close, localized About, simplified menu and retained controls; clipboard exercised=' + str(args.isolated_clipboard))


if __name__ == '__main__':
    main()
