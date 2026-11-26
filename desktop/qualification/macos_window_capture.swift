import AppKit
import CoreGraphics
import Darwin
import Foundation
import ScreenCaptureKit

// Qualification only. Never request permission or capture a display/other window.
@main
struct OwnedWindowCapture {
    @MainActor
    static func main() async {
        do {
            let args = CommandLine.arguments
            guard args.count == 5,
                  let windowID = UInt32(args[1]),
                  let pid = Int32(args[2]), pid > 0 else {
                throw Failure("Expected WINDOW_ID PID EXPECTED_EXECUTABLE NEW_PNG")
            }
            // Reuse the passive Aqua probe's working initialization order.
            // Enumerating screens initializes CGS before the permission query;
            // this creates no window and changes no application activation policy.
            guard !NSScreen.screens.isEmpty else {
                throw Failure("No Aqua screen available for the owned-window capture")
            }
            guard CGPreflightScreenCaptureAccess() else {
                throw Failure("No preexisting Screen Recording authorization; no prompt attempted")
            }
            let expected = URL(fileURLWithPath: args[3]).resolvingSymlinksInPath().standardizedFileURL
            guard let app = NSRunningApplication(processIdentifier: pid), !app.isTerminated,
                  app.executableURL?.resolvingSymlinksInPath().standardizedFileURL == expected else {
                throw Failure("Owned process executable identity unavailable or changed")
            }
            let content = try await SCShareableContent.excludingDesktopWindows(true, onScreenWindowsOnly: true)
            let matches = content.windows.filter {
                $0.windowID == windowID && $0.owningApplication?.processID == pid
            }
            guard matches.count == 1 else {
                throw Failure("Exact owned on-screen window unavailable")
            }
            let filter = SCContentFilter(desktopIndependentWindow: matches[0])
            let configuration = SCStreamConfiguration()
            configuration.width = max(1, Int(filter.contentRect.width * CGFloat(filter.pointPixelScale)))
            configuration.height = max(1, Int(filter.contentRect.height * CGFloat(filter.pointPixelScale)))
            configuration.showsCursor = false
            let image = try await SCScreenshotManager.captureImage(contentFilter: filter, configuration: configuration)
            guard !app.isTerminated,
                  let png = NSBitmapImageRep(cgImage: image).representation(using: .png, properties: [:]) else {
                throw Failure("Owned process exited or PNG encoding failed")
            }
            try png.write(to: URL(fileURLWithPath: args[4]), options: .withoutOverwriting)
            let proof: [String: Any] = ["pid": pid, "window": windowID,
                "width": image.width, "height": image.height,
                "method": "ScreenCaptureKit-desktop-independent-owned-window",
                "preexisting_screen_recording": true]
            let data = try JSONSerialization.data(withJSONObject: proof, options: [.sortedKeys])
            print(String(decoding: data, as: UTF8.self))
        } catch {
            fputs("Owned-window capture failed: \(error)\n", stderr)
            exit(1)
        }
    }

    private struct Failure: Error, CustomStringConvertible {
        let description: String
        init(_ description: String) { self.description = description }
    }
}
