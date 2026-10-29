import Cocoa
import Darwin
import Dispatch
import FlutterMacOS

@main
class AppDelegate: FlutterAppDelegate {
  private var activationSignalSource: DispatchSourceSignal?
  private let ownerQueue = DispatchQueue(label: "MicCamWatch.desktop-owner", qos: .userInitiated)
  private var ownerExitSource: DispatchSourceTimer?

  override init() {
    super.init()
    // Arm native ownership before engine setup. This queue is independent of
    // Dart and AppKit, so an unresponsive main loop cannot orphan the runner.
    // The inherited PID also detects owner death before this initializer runs.
    if let value = ProcessInfo.processInfo.environment["MCW_DESKTOP_OWNER_PID"] {
      guard let owner = Int32(value), owner > 1, getppid() == owner else {
        _exit(EXIT_FAILURE)
      }
      // Rechecking the actual parent avoids registering a recycled PID if
      // the owner exits during startup.
      let ownerSource = DispatchSource.makeTimerSource(queue: ownerQueue)
      ownerSource.schedule(deadline: .now(), repeating: .milliseconds(100), leeway: .milliseconds(10))
      ownerSource.setEventHandler {
        if getppid() != owner { _exit(EXIT_FAILURE) }
      }
      ownerExitSource = ownerSource
      ownerSource.resume()
      if getppid() != owner { _exit(EXIT_FAILURE) }
    }
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
    ownerExitSource?.cancel()
  }

  override func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
    return true
  }

  override func applicationSupportsSecureRestorableState(_ app: NSApplication) -> Bool {
    return true
  }
}
