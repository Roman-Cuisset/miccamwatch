import Cocoa
import FlutterMacOS

class MainFlutterWindow: NSWindow {
  override func awakeFromNib() {
    let flutterViewController = FlutterViewController()
    self.contentViewController = flutterViewController

    RegisterGeneratedPlugins(registry: flutterViewController)

    super.awakeFromNib()
    self.title = "MicCamWatch"
    let frameName = "MicCamWatchDesktopWindow"
    self.setContentSize(NSSize(width: 1040, height: 700))
    self.center()
    _ = self.setFrameUsingName(frameName)
    if let screen = self.screen ?? NSScreen.main {
      let visible = screen.visibleFrame
      let content = self.contentRect(forFrameRect: visible)
      self.contentMinSize = NSSize(width: min(640, content.width), height: min(440, content.height))
      var fitted = self.frame
      let minimum = self.frameRect(forContentRect: NSRect(origin: .zero, size: self.contentMinSize))
      fitted.size.width = min(max(fitted.width, minimum.width), visible.width)
      fitted.size.height = min(max(fitted.height, minimum.height), visible.height)
      fitted.origin.x = max(visible.minX, min(fitted.minX, visible.maxX - fitted.width))
      fitted.origin.y = max(visible.minY, min(fitted.minY, visible.maxY - fitted.height))
      self.setFrame(fitted, display: false)
    }
    _ = self.setFrameAutosaveName(frameName)
  }
}
