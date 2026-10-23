import AppKit
import ApplicationServices
import Darwin

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
guard CommandLine.arguments.count == 3,
      ["quit", "window", "close-window"].contains(CommandLine.arguments[1]),
      let pid = Int32(CommandLine.arguments[2]) else {
    stop("usage: app-smoke gui | accessibility | quit|window|close-window HELPER_PID")
}
let operation = CommandLine.arguments[1]
func attribute(_ element: AXUIElement, _ name: String) -> CFTypeRef? {
    var result: CFTypeRef?
    return AXUIElementCopyAttributeValue(element, name as CFString, &result) == .success ? result : nil
}
func children(_ element: AXUIElement) -> [AXUIElement] {
    attribute(element, kAXChildrenAttribute) as? [AXUIElement] ?? []
}
func descendants(_ element: AXUIElement, depth: Int = 0) -> [AXUIElement] {
    if depth > 16 { return [] }
    return [element] + children(element).flatMap { descendants($0, depth: depth + 1) }
}
let application = AXUIElementCreateApplication(pid)
AXUIElementSetMessagingTimeout(application, 3)
let deadline = Date().addingTimeInterval(15)
if operation != "quit" {
    while Date() < deadline {
        let windows = attribute(application, kAXWindowsAttribute) as? [AXUIElement] ?? []
        let consultation = windows.filter { (attribute($0, kAXTitleAttribute) as? String) == "MicCamWatch" }
        if consultation.count == 1 {
            let window = consultation[0]
            guard (attribute(window, kAXRoleAttribute) as? String) == kAXWindowRole else {
                stop("consultation is not a native window")
            }
            guard let sections = descendants(window).first(where: {
                (attribute($0, kAXRoleAttribute) as? String) == kAXTabGroupRole
            }), let tabs = attribute(sections, kAXTabsAttribute) as? [AXUIElement], tabs.count == 3 else {
                stop("consultation has no accessible native Overview/Settings/Diagnostics tabs")
            }
            let originalTab = tabs.first {
                (attribute($0, kAXValueAttribute) as? NSNumber)?.boolValue == true
            }
            let originalFocus = attribute(application, kAXFocusedUIElementAttribute)
            // Only local navigation: never press settings/device/admin controls.
            guard AXUIElementPerformAction(tabs[1], kAXPressAction as CFString) == .success else {
                stop("native Settings tab is not accessible")
            }
            let settings = descendants(window)
            guard settings.filter({ (attribute($0, kAXRoleAttribute) as? String) == kAXCheckBoxRole }).count >= 3,
                  settings.contains(where: { (attribute($0, kAXRoleAttribute) as? String) == kAXPopUpButtonRole }) else {
                stop("native Settings lacks preference checkboxes or profile chooser")
            }
            guard AXUIElementPerformAction(tabs[2], kAXPressAction as CFString) == .success,
                  descendants(window).contains(where: {
                      (attribute($0, kAXRoleAttribute) as? String) == kAXTextAreaRole &&
                      (attribute($0, kAXValueAttribute) as? String)?.isEmpty == false
                  }) else { stop("native Diagnostics document is not accessible/readable") }
            guard AXUIElementPerformAction(tabs[0], kAXPressAction as CFString) == .success else {
                stop("native Overview tab is not accessible")
            }
            if let originalTab = originalTab {
                guard AXUIElementPerformAction(originalTab, kAXPressAction as CFString) == .success else {
                    stop("native section navigation could not restore the selected tab")
                }
            }
            if let originalFocus = originalFocus, CFGetTypeID(originalFocus) == AXUIElementGetTypeID() {
                // Focus restoration is local navigation, never a control press.
                _ = AXUIElementSetAttributeValue(originalFocus as! AXUIElement,
                                                kAXFocusedAttribute as CFString, kCFBooleanTrue)
            }
            if operation == "close-window" {
                guard let close = attribute(window, kAXCloseButtonAttribute),
                      CFGetTypeID(close) == AXUIElementGetTypeID(),
                      AXUIElementPerformAction(close as! AXUIElement, kAXPressAction as CFString) == .success else {
                    stop("native consultation close button unavailable")
                }
                let closeDeadline = Date().addingTimeInterval(3)
                var closed = false
                while Date() < closeDeadline {
                    let remaining = attribute(application, kAXWindowsAttribute) as? [AXUIElement] ?? []
                    if !remaining.contains(where: { (attribute($0, kAXTitleAttribute) as? String) == "MicCamWatch" }) {
                        closed = true
                        break
                    }
                    RunLoop.current.run(until: Date().addingTimeInterval(0.05))
                }
                guard closed else { stop("native consultation close did not hide the window") }
                guard Darwin.kill(pid, 0) == 0 || errno == EPERM else {
                    stop("closing consultation terminated the desktop helper instead of retaining tray service")
                }
            }
            let visibleCount = operation == "close-window" ? 0 : 1
            print("{\"native_consultation\":\"\(operation)\",\"window_count\":\(visibleCount),\"observed_window_count_before\":1,\"native_sections\":3,\"helper_pid\":\(pid)}")
            exit(0)
        }
        if consultation.count > 1 { stop("duplicate native consultation windows") }
        RunLoop.current.run(until: Date().addingTimeInterval(0.1))
    }
    stop("native consultation window never became accessible")
}
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
        // The orchestrator separately proves readiness and both owned processes
        // retire. A successful AX press alone is not process-exit evidence.
        print("{\"action\":\"AXPress\",\"title\":\"Exit\",\"helper_pid\":\(pid),\"native_menu_press\":\"passed\"}")
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
