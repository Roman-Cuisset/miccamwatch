import AppKit
import Darwin

final class Delegate: NSObject, NSApplicationDelegate {
    private var refused = false
    func applicationDidFinishLaunching(_ notification: Notification) {
        try! String(getpid()).write(toFile: CommandLine.arguments[1], atomically: true, encoding: .utf8)
    }
    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        if CommandLine.arguments.contains("refuse-once") && !refused {
            refused = true
            return .terminateCancel
        }
        return .terminateNow
    }
}
let application = NSApplication.shared
let delegate = Delegate()
application.delegate = delegate
application.run()
