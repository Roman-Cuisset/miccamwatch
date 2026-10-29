import Cocoa
import Darwin
import Dispatch
import FlutterMacOS

@main
class AppDelegate: FlutterAppDelegate {
  private var activationSignalSource: DispatchSourceSignal?

  override init() {
    super.init()
    // Install before the Flutter view can announce readiness to its native owner.
    // SIGUSR1 is activation only; inherited private pipes remain the UI transport.
    signal(SIGUSR1, SIG_IGN)
    let source = DispatchSource.makeSignalSource(signal: SIGUSR1, queue: .main)
    source.setEventHandler { [weak self] in
      guard let window = self?.mainFlutterWindow else { return }
      window.deminiaturize(nil)
      window.makeKeyAndOrderFront(nil)
      NSApp.activate(ignoringOtherApps: true)
    }
    activationSignalSource = source
    source.resume()
  }

  deinit {
    activationSignalSource?.cancel()
  }

  override func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
    return true
  }

  override func applicationSupportsSecureRestorableState(_ app: NSApplication) -> Bool {
    return true
  }
}
