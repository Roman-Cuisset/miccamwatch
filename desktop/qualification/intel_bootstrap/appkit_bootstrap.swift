// Disposable hosted-runner diagnostic only. No windows, Flutter, IPC protocol,
// activation, event injection, capture, TCC requests, or preferences API.
import AppKit
import Darwin
import Foundation

func mark(_ stage: String) {
  let row: [String: Any] = [
    "stage": stage, "pid": getpid(), "ppid": getppid(),
    "pgid": getpgrp(), "unix_sid": getsid(0),
    "unix_time": Date().timeIntervalSince1970
  ]
  if let data = try? JSONSerialization.data(withJSONObject: row, options: [.sortedKeys]) {
    FileHandle.standardOutput.write(data)
    FileHandle.standardOutput.write(Data([10]))
  }
}

mark("before_NSApplication_shared")
let application = NSApplication.shared
mark("after_NSApplication_shared")
withExtendedLifetime(application) {}
