import AppKit
import ApplicationServices
import CoreGraphics
import Foundation

func fail(_ reason: String) -> Never {
    fputs("MicCamWatch entrypoint: \(reason)\n", stderr)
    exit(1)
}

func attribute(_ element: AXUIElement, _ name: String) -> CFTypeRef? {
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, name as CFString, &value) == .success else {
        return nil
    }
    return value
}

func children(_ element: AXUIElement) -> [AXUIElement] {
    attribute(element, kAXChildrenAttribute) as? [AXUIElement] ?? []
}

let args = CommandLine.arguments
guard args.count == 4, let pid = Int32(args[2]), pid > 1,
      let app = NSRunningApplication(processIdentifier: pid), !app.isTerminated,
      app.executableURL?.resolvingSymlinksInPath().standardizedFileURL ==
          URL(fileURLWithPath: args[3]).resolvingSymlinksInPath().standardizedFileURL else {
    fail("Exact owned process was not found")
}

switch args[1] {
case "window":
    let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements],
                                         kCGNullWindowID) as? [[String: Any]] ?? []
    let matches = list.filter { window in
        guard window[kCGWindowOwnerPID as String] as? Int == Int(pid),
              window[kCGWindowLayer as String] as? Int == 0,
              let bounds = window[kCGWindowBounds as String] as? [String: Any],
              let width = bounds["Width"] as? NSNumber, let height = bounds["Height"] as? NSNumber else {
            return false
        }
        return width.doubleValue >= 240 && height.doubleValue >= 160
    }
    guard matches.count == 1, let window = matches[0][kCGWindowNumber as String] as? Int else {
        fail("No uniquely owned on-screen Flutter window")
    }
    print("{\"pid\":\(pid),\"window\":\(window),\"on_screen\":true}")
case "menu":
    // This test accepts only an existing authorization. It never requests one.
    guard AXIsProcessTrusted() else { fail("No preexisting Accessibility authorization for native menu selection") }
    let target = AXUIElementCreateApplication(pid)
    guard let extras = attribute(target, "AXExtrasMenuBar"), CFGetTypeID(extras) == AXUIElementGetTypeID() else {
        fail("The exact helper exposes no status menu bar")
    }
    let menuBar = unsafeDowncast(extras, to: AXUIElement.self)
    let items = children(menuBar)
    guard items.count == 1, AXUIElementPerformAction(items[0], kAXPressAction as CFString) == .success else {
        fail("Couldn't press the single status item of the exact helper")
    }
    for _ in 0..<20 {
        let targets = children(items[0]).flatMap(children).filter {
            attribute($0, kAXTitleAttribute) as? String == "Open MicCamWatch…"
        }
        if targets.count == 1,
           AXUIElementPerformAction(targets[0], kAXPressAction as CFString) == .success {
            exit(0)
        }
        Thread.sleep(forTimeInterval: 0.1)
    }
    fail("The actual native menu did not expose a pressable Open MicCamWatch item")
default:
    fail("Expected window or menu")
}
