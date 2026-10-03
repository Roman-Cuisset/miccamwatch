import AppKit
import Foundation
import UserNotifications
import os
import Darwin

// Desktop protocol v1: UTF-8 newline-delimited JSON, maximum frame 65536 bytes.
// Rust -> Swift: {"kind":"state","summary":String,"visual":"idle"|"ready"|"active"|"error",
// "items":[{"action":String,"label":String,"enabled":Bool}]} or {"kind":"stop"}.
// Swift -> Rust: {"kind":"ready","protocol":1}, {"kind":"action","action":String},
// {"kind":"error","error":String}, {"kind":"stopped"}. Actions: status, mic,
// camera, pause, profile, autostart, exit. This helper never collects capture data.
private struct DesktopItem: Decodable { let action: String; let label: String; let enabled: Bool }
private struct DesktopFrame: Decodable {
    let kind: String
    let summary: String?
    let visual: String?
    let items: [DesktopItem]?
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
private final class DesktopDelegate: NSObject, NSApplicationDelegate {
    private var item: NSStatusItem?
    private var menu = NSMenu()
    private var stopping = false
    private var pendingClick: DispatchWorkItem?
    func applicationDidFinishLaunching(_ notification: Notification) {
        item = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        guard let item = item, let button = item.button, item.isVisible else {
            desktopEmit(DesktopEvent(kind: "error", error: "AppKit could not register a status item"))
            NSApp.terminate(nil)
            return
        }
        button.target = self
        button.action = #selector(clicked(_:))
        button.sendAction(on: [.leftMouseUp, .rightMouseUp])
        button.toolTip = "MicCamWatch"
        button.image = icon("error")
        menu.addItem(withTitle: "MicCamWatch", action: nil, keyEquivalent: "")
        desktopEmit(DesktopEvent(kind: "ready", protocol: 1))
        DispatchQueue.global(qos: .utility).async { [weak self] in self?.readFrames() }
    }
    private func readFrames() {
        var frame = Data()
        do {
            while let chunk = try FileHandle.standardInput.read(upToCount: 4096), !chunk.isEmpty {
                for byte in chunk {
                    if byte == 10 {
                        let decoded = try JSONDecoder().decode(DesktopFrame.self, from: frame)
                        frame.removeAll(keepingCapacity: true)
                        DispatchQueue.main.async { [weak self] in self?.apply(decoded) }
                    } else {
                        guard frame.count < 65536 else { throw NSError(domain: "MicCamWatch", code: 1, userInfo: [NSLocalizedDescriptionKey: "desktop frame exceeds limit"]) }
                        frame.append(byte)
                    }
                }
            }
            if !frame.isEmpty { throw NSError(domain: "MicCamWatch", code: 2, userInfo: [NSLocalizedDescriptionKey: "truncated desktop frame"]) }
            DispatchQueue.main.async { [weak self] in self?.stop() }
        } catch {
            desktopEmit(DesktopEvent(kind: "error", error: error.localizedDescription))
            DispatchQueue.main.async { [weak self] in self?.stop() }
        }
    }
    private func apply(_ frame: DesktopFrame) {
        if frame.kind == "stop" { stop(); return }
        guard frame.kind == "state", let summary = frame.summary, let visual = frame.visual,
              ["idle", "ready", "active", "error"].contains(visual), let entries = frame.items,
              entries.count <= 16, summary.utf8.count <= 16384 else {
            desktopEmit(DesktopEvent(kind: "error", error: "invalid desktop state frame")); stop(); return
        }
        let allowed = ["status", "mic", "camera", "pause", "profile", "autostart", "exit", "header"]
        guard entries.allSatisfy({ allowed.contains($0.action) && $0.label.utf8.count <= 16384 }) else {
            desktopEmit(DesktopEvent(kind: "error", error: "invalid desktop menu item")); stop(); return
        }
        item?.button?.toolTip = summary
        item?.button?.image = icon(visual)
        menu.removeAllItems()
        for entry in entries {
            let row = NSMenuItem(title: entry.label, action: #selector(selected(_:)), keyEquivalent: "")
            row.target = self
            row.representedObject = entry.action
            row.isEnabled = entry.enabled
            menu.addItem(row)
        }
        menu.autoenablesItems = false
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
        guard !stopping, let button = item?.button else { return }
        menu.popUp(positioning: nil, at: NSPoint(x: 0, y: button.bounds.height), in: button)
    }
    @objc private func selected(_ sender: NSMenuItem) {
        if let action = sender.representedObject as? String {
            desktopEmit(DesktopEvent(kind: "action", action: action))
        }
    }
    private func stop() {
        guard !stopping else { return }
        stopping = true
        pendingClick?.cancel()
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
    let application = NSApplication.shared
    guard application.setActivationPolicy(.accessory) else {
        desktopEmit(DesktopEvent(kind: "error", error: "AppKit accessory application unavailable")); Darwin.exit(1)
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
