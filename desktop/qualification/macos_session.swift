// Passive Aqua capability probe: no TCC request, Accessibility or screen capture.
import AppKit
import CoreGraphics
import Foundation

let session = CGSessionCopyCurrentDictionary() as? [String: Any] ?? [:]
let onConsole = session[kCGSessionOnConsoleKey as String] as? Bool ?? false
let row: [String: Any] = [
  "on_console": onConsole,
  "screens": NSScreen.screens.count,
  "aqua_available": onConsole && !NSScreen.screens.isEmpty,
  "screen_capture_previously_authorized": CGPreflightScreenCaptureAccess()
]
let data = try JSONSerialization.data(withJSONObject: row, options: [.sortedKeys])
print(String(decoding: data, as: UTF8.self))
