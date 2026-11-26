import Cocoa
import Darwin
import Dispatch
import FlutterMacOS

@main
class AppDelegate: FlutterAppDelegate {
  private var activationSignalSource: DispatchSourceSignal?
  private let ownerQueue = DispatchQueue(label: "MicCamWatch.desktop-owner", qos: .userInitiated)
  private var ownerExitSource: DispatchSourceTimer?
  #if MCW_QUALIFICATION
  // Hosted-only instrumentation. Not compiled into the normal release runner.
  // No public IPC endpoint, input injection, filesystem or device authority.
  private var qualificationStateSource: DispatchSourceTimer?
  private var qualificationCloseSource: DispatchSourceSignal?
  #endif

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
      #if MCW_QUALIFICATION
      if ProcessInfo.processInfo.environment["MCW_DESKTOP_QUALIFICATION"] == "1" {
        fputs("MCW_QUALIFICATION_ACTIVATED pid=\(getpid()) window=\(window.windowNumber)\n", stderr)
      }
      #endif
    }
    activationSignalSource = source
    source.resume()
    #if MCW_QUALIFICATION
    if ProcessInfo.processInfo.environment["MCW_DESKTOP_QUALIFICATION"] == "1" {
      signal(SIGUSR2, SIG_IGN)
      let close = DispatchSource.makeSignalSource(signal: SIGUSR2, queue: .main)
      close.setEventHandler { [weak self] in
        // Exercise the normal close-button path, not application termination.
        self?.mainFlutterWindow?.performClose(nil)
      }
      qualificationCloseSource = close
      close.resume()
      let state = DispatchSource.makeTimerSource(queue: .main)
      state.schedule(deadline: .now(), repeating: .milliseconds(500))
      state.setEventHandler { [weak self] in
        guard let window = self?.mainFlutterWindow else { return }
        let row: [String: Any] = [
          "pid": getpid(), "window": window.windowNumber,
          "visible": window.isVisible, "miniaturized": window.isMiniaturized,
          "key": window.isKeyWindow, "title": window.title,
          "width": window.frame.width, "height": window.frame.height
        ]
        if let data = try? JSONSerialization.data(withJSONObject: row, options: [.sortedKeys]),
           let json = String(data: data, encoding: .utf8) {
          fputs("MCW_QUALIFICATION_STATE \(json)\n", stderr)
        }
      }
      qualificationStateSource = state
      state.resume()
    }
    #endif
  }

  deinit {
    activationSignalSource?.cancel()
    ownerExitSource?.cancel()
    #if MCW_QUALIFICATION
    qualificationStateSource?.cancel()
    qualificationCloseSource?.cancel()
    #endif
  }

  override func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
    return true
  }

  override func applicationSupportsSecureRestorableState(_ app: NSApplication) -> Bool {
    return true
  }
}
