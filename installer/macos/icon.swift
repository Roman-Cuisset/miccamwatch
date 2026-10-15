import AppKit
import Foundation

// Compiled as main.swift alongside native/macos_brand.swift during packaging.
// There is no SVG renderer or icon-generation dependency in the installed app.
guard CommandLine.arguments.count == 2 else {
    fputs("usage: mcw-icon-generator OUTPUT.iconset\n", stderr)
    exit(2)
}
let directory = URL(fileURLWithPath: CommandLine.arguments[1], isDirectory: true)
try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
for points in [16, 32, 128, 256, 512] {
    for scale in [1, 2] {
        let pixels = points * scale
        guard let bitmap = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: pixels,
                pixelsHigh: pixels, bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true,
                isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0),
              let context = NSGraphicsContext(bitmapImageRep: bitmap) else {
            fatalError("AppKit icon bitmap unavailable")
        }
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = context
        mcwBrandImage(size: CGFloat(pixels)).draw(in: NSRect(x: 0, y: 0, width: CGFloat(pixels), height: CGFloat(pixels)))
        context.flushGraphics()
        NSGraphicsContext.restoreGraphicsState()
        guard let png = bitmap.representation(using: .png, properties: [:]) else {
            fatalError("AppKit PNG encoding unavailable")
        }
        let suffix = scale == 2 ? "@2x" : ""
        try png.write(to: directory.appendingPathComponent("icon_\(points)x\(points)\(suffix).png"))
    }
}
