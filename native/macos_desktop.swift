import AppKit
import CoreGraphics
import CoreFoundation
import Foundation
import UserNotifications
import os
import Darwin

// Desktop protocol v1: UTF-8 newline-delimited JSON, maximum frame 65536 bytes.
// Rust -> Swift: {"kind":"state","summary":String,"visual":"idle"|"ready"|"active"|"error",
// "details_label":String,"items":[{"action":String,"label":String,"detail":String?,"enabled":Bool}]}
// or {"kind":"stop"}. Labels are compact; optional detail retains the original.
// Swift -> Rust: {"kind":"ready","protocol":1}, {"kind":"action","action":String},
// {"kind":"error","error":String}, {"kind":"stopped"}. Actions: status, mic,
// camera, pause, profile, autostart, exit. This helper never collects capture data.
struct DesktopItem: Decodable {
    let action: String
    let detail: String?
    let label: String
    let enabled: Bool
}
struct DesktopFrame: Decodable {
    let kind: String
    let summary: String?
    let visual: String?
    let items: [DesktopItem]?
    let details_label: String?
}
private struct DesktopEvent: Encodable {
    var kind: String
    var `protocol`: Int? = nil
    var action: String? = nil
    var error: String? = nil
}
private func desktopEmit(_ event: DesktopEvent) {
    do {
        var bytes = try JSONEncoder().encode(event)
        bytes.append(10)
        try FileHandle.standardOutput.write(contentsOf: bytes)
    } catch { Darwin.exit(1) }
}
// Only documented Quartz session keys are used; console/login context is not lock evidence.
private func desktopGUIContext() -> (available: Bool, detail: String) {
    guard let session = CGSessionCopyCurrentDictionary() else {
        return (false, "quartzSession=unavailable euid=\(geteuid())")
    }
    let dictionary = session as NSDictionary
    let uid = (dictionary[kCGSessionUserIDKey] as? NSNumber)?.stringValue ?? "unknown"
    let onConsole = (dictionary[kCGSessionOnConsoleKey] as? Bool).map { String($0) } ?? "unknown"
    let loginDone = (dictionary[kCGSessionLoginDoneKey] as? Bool).map { String($0) } ?? "unknown"
    return (true, "quartzSession=available euid=\(geteuid()) sessionUID=\(uid) onConsole=\(onConsole) loginDone=\(loginDone)")
}
private func desktopPolicyName(_ policy: NSApplication.ActivationPolicy) -> String {
    switch policy {
    case .regular: return "regular"
    case .accessory: return "accessory"
    case .prohibited: return "prohibited"
    @unknown default: return "unknown(\(policy.rawValue))"
    }
}
private func desktopDiagnostic(_ context: String) {
    try? FileHandle.standardError.write(contentsOf: Data("MicCamWatch AppKit: \(context)\n".utf8))
}
// Standard AppKit menu chrome needs about 60 points in addition to the text.
// Measure the actual menu font, not bytes or character counts (localized labels
// and wide graphemes differ substantially). Never split a composed character.
final class DesktopMenu: NSObject {
    let menu = NSMenu()
    let diagnostic: String
    private let detailsLabel: String
    private let textWidth: CGFloat

    init(summary: String, entries: [DesktopItem], detailsLabel: String) {
        self.detailsLabel = detailsLabel
        textWidth = min(340, max(80, (NSScreen.main?.visibleFrame.width ?? 400) - 60))
        var paragraphs = [summary]
        for entry in entries {
            let full = entry.detail ?? entry.label
            if !paragraphs.contains(full) { paragraphs.append(full) }
        }
        diagnostic = paragraphs.joined(separator: "\n\n")
        super.init()
        menu.font = NSFont.menuFont(ofSize: 0)
        menu.autoenablesItems = false
        for entry in entries {
            let row = NSMenuItem(title: boundedTitle(entry.label), action: #selector(selected(_:)), keyEquivalent: "")
            row.target = self
            row.representedObject = entry.action
            row.isEnabled = entry.enabled
            menu.addItem(row)
        }
        menu.addItem(.separator())
        let details = NSMenuItem(title: boundedTitle(detailsLabel), action: #selector(showDetails(_:)), keyEquivalent: "")
        details.target = self
        menu.addItem(details)
    }

    private func boundedTitle(_ original: String) -> String {
        var title = original
        if original.contains(where: { $0.isNewline || $0 == "\t" }) {
            title = ""
            title.reserveCapacity(original.utf8.count)
            for character in original {
                if character.isNewline || character == "\t" {
                    title.append(" ")
                } else {
                    title.append(character)
                }
            }
        }
        let font = menu.font ?? NSFont.menuFont(ofSize: 0)
        func fits(_ value: String) -> Bool {
            (value as NSString).size(withAttributes: [.font: font]).width <= textWidth
        }
        if fits(title) { return title }
        let source = title as NSString
        var low = 0, high = source.length
        var result = "…"
        while low <= high {
            let middle = low + (high - low) / 2
            let range = source.rangeOfComposedCharacterSequences(for: NSRange(location: 0, length: middle))
            let candidate = source.substring(with: range) + "…"
            if fits(candidate) {
                result = candidate
                low = middle + 1
            } else {
                high = middle - 1
            }
        }
        return result
    }

    func detailsAlert() -> NSAlert {
        let alert = NSAlert()
        alert.messageText = "MicCamWatch — \(detailsLabel)"
        // The document is selectable/copyable, scrollable, and wraps even a
        // single unbroken 16 KB diagnostic. It never sets a menu's intrinsic width.
        let scroll = NSScrollView(frame: NSRect(x: 0, y: 0, width: 440, height: 260))
        scroll.hasVerticalScroller = true
        scroll.hasHorizontalScroller = false
        scroll.borderType = .bezelBorder
        let view = NSTextView(frame: scroll.contentView.bounds)
        view.isEditable = false
        view.isSelectable = true
        view.isRichText = false
        view.font = NSFont.systemFont(ofSize: NSFont.systemFontSize)
        view.isVerticallyResizable = true
        view.isHorizontallyResizable = false
        view.autoresizingMask = [.width]
        view.textContainer?.widthTracksTextView = true
        view.textContainer?.lineBreakMode = .byCharWrapping
        view.textContainer?.containerSize = NSSize(width: scroll.contentSize.width, height: .greatestFiniteMagnitude)
        view.string = diagnostic
        scroll.documentView = view
        alert.accessoryView = scroll
        return alert
    }

    @objc private func showDetails(_ sender: NSMenuItem) {
        NSApp.activate(ignoringOtherApps: true)
        detailsAlert().runModal()
    }
    @objc private func selected(_ sender: NSMenuItem) {
        if let action = sender.representedObject as? String {
            desktopEmit(DesktopEvent(kind: "action", action: action))
        }
    }
}
private final class DesktopDelegate: NSObject, NSApplicationDelegate {
    private var item: NSStatusItem?
    private var menuContent: DesktopMenu?
    private weak var trackingMenu: NSMenu?
    private var stopping = false
    private var initialized = false
    private var pendingClick: DispatchWorkItem?
    func applicationDidFinishLaunching(_ notification: Notification) {
        desktopDiagnostic("didFinishLaunching policy=\(desktopPolicyName(NSApp.activationPolicy()))")
        item = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        guard let item = item, let button = item.button, item.isVisible else {
            let context = desktopGUIContext().detail
            desktopEmit(DesktopEvent(kind: "error", error: "AppKit could not register a visible status item; policy=\(desktopPolicyName(NSApp.activationPolicy())) \(context)"))
            NSApp.terminate(nil)
            return
        }
        button.target = self
        button.action = #selector(clicked(_:))
        button.sendAction(on: [.leftMouseUp, .rightMouseUp])
        button.toolTip = "MicCamWatch"
        button.image = icon("error")
        desktopDiagnostic("statusItem registered visible=\(item.isVisible) buttonWindow=\(button.window != nil)")
        DispatchQueue.global(qos: .utility).async { [weak self] in self?.readFrames() }
    }
    private func readFrames() {
        var frame = Data()
        var buffer = [UInt8](repeating: 0, count: 4096)
        do {
            while true {
                let count = buffer.withUnsafeMutableBytes { bytes in
                    Darwin.read(STDIN_FILENO, bytes.baseAddress!, bytes.count)
                }
                if count == 0 { break }
                if count < 0 {
                    if errno == EINTR { continue }
                    throw NSError(domain: NSPOSIXErrorDomain, code: Int(errno))
                }
                for byte in buffer.prefix(count) {
                    if byte == 10 {
                        let decoded = try JSONDecoder().decode(DesktopFrame.self, from: frame)
                        frame.removeAll(keepingCapacity: true)
                        if decoded.kind == "stop" {
                            scheduleStop()
                        } else {
                            DispatchQueue.main.async { [weak self] in self?.apply(decoded) }
                        }
                    } else {
                        guard frame.count < 65536 else { throw NSError(domain: "MicCamWatch", code: 1, userInfo: [NSLocalizedDescriptionKey: "desktop frame exceeds limit"]) }
                        frame.append(byte)
                    }
                }
            }
            if !frame.isEmpty { throw NSError(domain: "MicCamWatch", code: 2, userInfo: [NSLocalizedDescriptionKey: "truncated desktop frame"]) }
            scheduleStop()
        } catch {
            desktopEmit(DesktopEvent(kind: "error", error: error.localizedDescription))
            scheduleStop()
        }
    }
    private func scheduleStop() {
        // NSMenu tracking does not service the main dispatch queue.
        RunLoop.main.perform(inModes: [.common]) { [weak self] in self?.stop() }
        CFRunLoopWakeUp(CFRunLoopGetMain())
    }
    private func apply(_ frame: DesktopFrame) {
        guard frame.kind == "state", let summary = frame.summary, let visual = frame.visual,
              ["idle", "ready", "active", "error"].contains(visual), let entries = frame.items,
              let detailsLabel = frame.details_label, detailsLabel.utf8.count <= 128,
              entries.count <= 16, summary.utf8.count <= 16384 else {
            desktopEmit(DesktopEvent(kind: "error", error: "invalid desktop state frame")); stop(); return
        }
        let allowed = ["status", "mic", "camera", "pause", "profile", "autostart", "exit", "header"]
        guard entries.allSatisfy({ allowed.contains($0.action) && $0.label.utf8.count <= 16384 && ($0.detail?.utf8.count ?? 0) <= 16384 }) else {
            desktopEmit(DesktopEvent(kind: "error", error: "invalid desktop menu item")); stop(); return
        }
        item?.button?.toolTip = summary
        item?.button?.image = icon(visual)
        let content = DesktopMenu(summary: summary, entries: entries, detailsLabel: detailsLabel)
        menuContent = content
        if !initialized {
            initialized = true
            desktopEmit(DesktopEvent(kind: "ready", protocol: 1))
        }
    }
    private func icon(_ visual: String) -> NSImage {
        let image = NSImage(size: NSSize(width: 18, height: 18), flipped: false) { rect in
            let color: NSColor
            switch visual { case "idle": color = .systemGreen; case "ready": color = .systemYellow;
            case "active": color = .systemRed; default: color = .systemGray }
            color.setFill()
            NSBezierPath(ovalIn: rect.insetBy(dx: 2, dy: 2)).fill()
            NSColor.white.setStroke()
            let path = NSBezierPath(); path.lineWidth = 2
            if visual == "error" {
                path.move(to: NSPoint(x: 9, y: 6)); path.line(to: NSPoint(x: 9, y: 13)); path.stroke()
                NSColor.white.setFill(); NSBezierPath(ovalIn: NSRect(x: 8, y: 3, width: 2, height: 2)).fill()
            } else if visual == "active" {
                NSColor.white.setFill(); NSBezierPath(ovalIn: NSRect(x: 6, y: 6, width: 6, height: 6)).fill()
            } else if visual == "ready" {
                path.move(to: NSPoint(x: 7, y: 5)); path.line(to: NSPoint(x: 7, y: 13))
                path.move(to: NSPoint(x: 11, y: 5)); path.line(to: NSPoint(x: 11, y: 13)); path.stroke()
            } else {
                path.move(to: NSPoint(x: 5, y: 9)); path.line(to: NSPoint(x: 8, y: 6)); path.line(to: NSPoint(x: 13, y: 12)); path.stroke()
            }
            return true
        }
        image.isTemplate = false
        return image
    }
    @objc private func clicked(_ sender: Any?) {
        guard initialized, !stopping else { return }
        pendingClick?.cancel()
        pendingClick = nil
        if NSApp.currentEvent?.clickCount == 2 {
            desktopEmit(DesktopEvent(kind: "action", action: "status"))
        } else if NSApp.currentEvent?.type == .rightMouseUp {
            showMenu()
        } else {
            // Do not enter menu tracking on click one before AppKit can deliver click two.
            let work = DispatchWorkItem { [weak self] in self?.showMenu() }
            pendingClick = work
            DispatchQueue.main.asyncAfter(deadline: .now() + NSEvent.doubleClickInterval, execute: work)
        }
    }
    private func showMenu() {
        guard !stopping, trackingMenu == nil, let button = item?.button, let content = menuContent else { return }
        // A state frame can replace menuContent during native menu tracking.
        // NSMenuItem targets are weak; retain this snapshot until selection ends.
        trackingMenu = content.menu
        defer { trackingMenu = nil }
        withExtendedLifetime(content) {
            _ = content.menu.popUp(positioning: nil, at: NSPoint(x: 0, y: button.bounds.height), in: button)
        }
    }
    private func stop() {
        guard !stopping else { return }
        stopping = true
        pendingClick?.cancel()
        trackingMenu?.cancelTracking()
        if let item = item { NSStatusBar.system.removeStatusItem(item) }
        item = nil
        desktopEmit(DesktopEvent(kind: "stopped"))
        NSApp.terminate(nil)
    }
    func applicationWillTerminate(_ notification: Notification) {
        if let item = item { NSStatusBar.system.removeStatusItem(item) }
    }
}
func desktopMain(args: [String]) -> Never {
    guard args.isEmpty, Thread.isMainThread else {
        desktopEmit(DesktopEvent(kind: "error", error: "desktop requires the main thread and no arguments")); Darwin.exit(1)
    }
    let gui = desktopGUIContext()
    guard gui.available else {
        desktopEmit(DesktopEvent(kind: "error", error: "AppKit menu bar requires a Quartz GUI/WindowServer session for this process; \(gui.detail)")); Darwin.exit(1)
    }
    let application = NSApplication.shared
    let initialPolicy = application.activationPolicy()
    let agentBundle = Bundle.main.bundleURL.pathExtension == "app"
    let uiElement = (Bundle.main.object(forInfoDictionaryKey: "LSUIElement") as? NSNumber)?.boolValue
    let context = "\(gui.detail) appBundle=\(agentBundle) LSUIElement=\(uiElement.map { String($0) } ?? "unknown") initialPolicy=\(desktopPolicyName(initialPolicy))"
    // LSUIElement=true already requests accessory policy. The setter reports a
    // policy switch, not status-item registration; do not require a redundant switch.
    if initialPolicy != .accessory {
        let switched = application.setActivationPolicy(.accessory)
        let resultingPolicy = application.activationPolicy()
        desktopDiagnostic("startup \(context) switchAccepted=\(switched) resultingPolicy=\(desktopPolicyName(resultingPolicy))")
        guard switched, resultingPolicy == .accessory else {
            desktopEmit(DesktopEvent(kind: "error", error: "AppKit rejected the accessory activation policy transition; \(context) switchAccepted=\(switched) resultingPolicy=\(desktopPolicyName(resultingPolicy))")); Darwin.exit(1)
        }
    } else {
        desktopDiagnostic("startup \(context) switch=unnecessary")
    }
    let delegate = DesktopDelegate()
    application.delegate = delegate
    withExtendedLifetime(delegate) { application.run() }
    Darwin.exit(0)
}

private func desktopFailure(_ error: String) -> NativeEffectReply {
    NativeEffectReply(ok: false, error: error, state: nil)
}
// Native effect requests are explicit side effects. Passive observation never reaches here.
func desktopEffect(mode: String, args: [String]) -> NativeEffectReply {
    if mode == "log" {
        guard args.count == 1 else { return desktopFailure("log requires a message") }
        let logger = Logger(subsystem: "com.roman-cuisset.miccamwatch.helper", category: "capture")
        logger.notice("\(args[0], privacy: .public)")
        return NativeEffectReply(ok: true, error: nil, state: "accepted")
    }
    if mode == "sound" {
        guard args.isEmpty else { return desktopFailure("sound takes no arguments") }
        guard let sound = NSSound(named: NSSound.Name("Glass")), sound.play() else {
            return desktopFailure("system sound could not be played")
        }
        // Keep this one-shot helper alive while AppKit supplies the audio.
        let deadline = Date().addingTimeInterval(10)
        while sound.isPlaying && Date() < deadline { RunLoop.current.run(until: Date().addingTimeInterval(0.02)) }
        if sound.isPlaying { sound.stop(); return desktopFailure("system sound playback timed out") }
        return NativeEffectReply(ok: true, error: nil, state: "played")
    }
    guard mode == "notify" || mode == "notification-identity" else { return desktopFailure("unknown desktop effect") }
    guard Bundle.main.bundleIdentifier == "com.roman-cuisset.miccamwatch.helper",
          Bundle.main.bundleURL.pathExtension == "app" else {
        return desktopFailure("notification helper must run with its installed stable application identity")
    }
    guard (mode == "notify" && args.count == 2) || (mode == "notification-identity" && args.isEmpty) else {
        return desktopFailure("invalid notification arguments")
    }
    let center = UNUserNotificationCenter.current()
    // Callbacks mutate their result only on the main queue; the main run loop waits boundedly.
    var authorization: Bool? = nil
    var failure: String? = nil
    center.requestAuthorization(options: [.alert, .sound]) { accepted, error in
        DispatchQueue.main.async { authorization = accepted; failure = error?.localizedDescription }
    }
    let deadline = Date().addingTimeInterval(45)
    while authorization == nil && Date() < deadline { RunLoop.current.run(until: Date().addingTimeInterval(0.02)) }
    guard authorization == true else { return desktopFailure(failure ?? (authorization == nil ? "notification authorization timed out" : "notification authorization denied")) }
    if mode == "notification-identity" { return NativeEffectReply(ok: true, error: nil, state: "authorized") }
    let content = UNMutableNotificationContent()
    content.title = args[0]; content.body = args[1]
    let request = UNNotificationRequest(identifier: UUID().uuidString, content: content, trigger: nil)
    var completed = false
    center.add(request) { error in
        DispatchQueue.main.async { failure = error?.localizedDescription; completed = true }
    }
    let deliveryDeadline = Date().addingTimeInterval(10)
    while !completed && Date() < deliveryDeadline { RunLoop.current.run(until: Date().addingTimeInterval(0.02)) }
    guard completed else { return desktopFailure("notification submission timed out") }
    if let error = failure { return desktopFailure(error) }
    return NativeEffectReply(ok: true, error: nil, state: "submitted")
}
