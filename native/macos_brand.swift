import AppKit

// Canonical native rendering of site/assets/mark.svg (48 × 48 coordinates).
// Shared by the embedded desktop helper and the packaging-only icon generator.
func mcwBrandImage(size: CGFloat, menuBar: Bool = false) -> NSImage {
    let image = NSImage(size: NSSize(width: size, height: size), flipped: true) { rect in
        guard let context = NSGraphicsContext.current?.cgContext else { return false }
        context.saveGState()
        defer { context.restoreGState() }
        context.scaleBy(x: rect.width / 48, y: rect.height / 48)
        if !menuBar {
            context.setFillColor(NSColor(srgbRed: 204 / 255, green: 237 / 255, blue: 206 / 255, alpha: 1).cgColor)
            context.addPath(CGPath(roundedRect: CGRect(x: 0, y: 0, width: 48, height: 48),
                                   cornerWidth: 13, cornerHeight: 13, transform: nil))
            context.fillPath()
        }
        context.setStrokeColor((menuBar ? NSColor.black :
            NSColor(srgbRed: 18 / 255, green: 107 / 255, blue: 80 / 255, alpha: 1)).cgColor)
        context.setLineWidth(2.3)
        context.setLineCap(.round)
        context.setLineJoin(.round)
        // SVG smooth cubics expanded to explicit control points.
        context.move(to: CGPoint(x: 6.5, y: 24))
        context.addCurve(to: CGPoint(x: 24, y: 13), control1: CGPoint(x: 6.5, y: 24), control2: CGPoint(x: 12.5, y: 13))
        context.addCurve(to: CGPoint(x: 41.5, y: 24), control1: CGPoint(x: 35.5, y: 13), control2: CGPoint(x: 41.5, y: 24))
        context.addCurve(to: CGPoint(x: 24, y: 35), control1: CGPoint(x: 41.5, y: 24), control2: CGPoint(x: 35.5, y: 35))
        context.addCurve(to: CGPoint(x: 6.5, y: 24), control1: CGPoint(x: 12.5, y: 35), control2: CGPoint(x: 6.5, y: 24))
        context.closePath()
        context.strokePath()
        context.addPath(CGPath(roundedRect: CGRect(x: 21, y: 17.5, width: 6, height: 10),
                               cornerWidth: 3, cornerHeight: 3, transform: nil))
        context.strokePath()
        context.move(to: CGPoint(x: 17.5, y: 23.5))
        context.addLine(to: CGPoint(x: 17.5, y: 24.5))
        context.addArc(center: CGPoint(x: 24, y: 24.5), radius: 6.5,
                       startAngle: .pi, endAngle: 0, clockwise: true)
        context.addLine(to: CGPoint(x: 30.5, y: 23.5))
        context.move(to: CGPoint(x: 24, y: 31))
        context.addLine(to: CGPoint(x: 24, y: 34))
        context.strokePath()
        return true
    }
    image.isTemplate = menuBar
    return image
}
