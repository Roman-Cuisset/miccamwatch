import AppKit
import ApplicationServices

// Test executable only. Reads existing Accessibility authorization, never asks
// for it and never changes TCC. No device APIs or injected capture fixtures.
func stop(_ message: String, code: Int32 = 1) -> Never {
    fputs(message + "\n", stderr)
    exit(code)
}
guard CGSessionCopyCurrentDictionary() != nil else {
    stop("UNAVAILABLE: logged-in Aqua/WindowServer session required", code: 77)
}
let app = NSApplication.shared
app.setActivationPolicy(.accessory)
app.finishLaunching()
guard NSScreen.main != nil else { stop("UNAVAILABLE: logged-in Aqua/WindowServer display required", code: 77) }
if CommandLine.arguments.count == 2 && CommandLine.arguments[1] == "gui" { exit(0) }
guard AXIsProcessTrusted() else {
    stop("UNAVAILABLE: smoke driver needs preexisting Accessibility permission to click the real Exit menu; no permission was requested", code: 77)
}
if CommandLine.arguments.count == 2 && CommandLine.arguments[1] == "accessibility" { exit(0) }
guard CommandLine.arguments.count == 3, CommandLine.arguments[1] == "quit",
      let pid = Int32(CommandLine.arguments[2]) else { stop("usage: app-smoke gui | accessibility | quit HELPER_PID") }
func attribute(_ element: AXUIElement, _ name: String) -> CFTypeRef? {
    var result: CFTypeRef?
    return AXUIElementCopyAttributeValue(element, name as CFString, &result) == .success ? result : nil
}
func children(_ element: AXUIElement) -> [AXUIElement] {
    attribute(element, kAXChildrenAttribute) as? [AXUIElement] ?? []
}
func descendants(_ element: AXUIElement, depth: Int = 0) -> [AXUIElement] {
    if depth > 8 { return [] }
    return [element] + children(element).flatMap { descendants($0, depth: depth + 1) }
}
let application = AXUIElementCreateApplication(pid)
AXUIElementSetMessagingTimeout(application, 3)
let deadline = Date().addingTimeInterval(15)
var pressed = false
while Date() < deadline {
    var roots = [application]
    if let extras = attribute(application, "AXExtrasMenuBar"),
       CFGetTypeID(extras) == AXUIElementGetTypeID() {
        roots.insert(extras as! AXUIElement, at: 0)
    }
    let elements = roots.flatMap { descendants($0) }
    if let exitItem = elements.first(where: {
        (attribute($0, kAXRoleAttribute) as? String) == kAXMenuItemRole &&
        (attribute($0, kAXTitleAttribute) as? String) == "Exit"
    }) {
        guard AXUIElementPerformAction(exitItem, kAXPressAction as CFString) == .success else {
            stop("real native Exit menu press failed")
        }
        print("Pressed real native Exit menu item")
        exit(0)
    }
    if !pressed {
        // NSStatusItem is exposed by AppKit as an app-owned menu-bar item.
        if let status = elements.first(where: {
            let role = attribute($0, kAXRoleAttribute) as? String
            return role == kAXMenuBarItemRole || role == "AXMenuBarExtra"
        }) {
            guard AXUIElementPerformAction(status, kAXPressAction as CFString) == .success else {
                stop("real native status-item press failed")
            }
            pressed = true
        }
    }
    RunLoop.current.run(until: Date().addingTimeInterval(0.1))
}
stop("real app-owned status menu/Exit item was not accessible")
