import AppKit
import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers

let size: CGFloat = 1024
let arguments = CommandLine.arguments
let cylinderOnly = arguments.contains("--cylinder-only")

func color(_ hex: String, _ alpha: CGFloat = 1) -> CGColor {
    var value = hex.trimmingCharacters(in: CharacterSet(charactersIn: "#"))
    if value.count == 3 {
        value = value.map { "\($0)\($0)" }.joined()
    }
    let number = UInt64(value, radix: 16) ?? 0
    return CGColor(
        colorSpace: CGColorSpaceCreateDeviceRGB(),
        components: [
            CGFloat((number >> 16) & 0xFF) / 255,
            CGFloat((number >> 8) & 0xFF) / 255,
            CGFloat(number & 0xFF) / 255,
            alpha,
        ]
    )!
}

guard
    let context = CGContext(
        data: nil,
        width: Int(size),
        height: Int(size),
        bitsPerComponent: 8,
        bytesPerRow: 0,
        space: CGColorSpaceCreateDeviceRGB(),
        bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
    )
else {
    fputs("cannot create bitmap context\n", stderr)
    exit(1)
}

// Flip to a top-left origin so the coordinates below read like a design tool.
context.translateBy(x: 0, y: size)
context.scaleBy(x: 1, y: -1)
context.setAllowsAntialiasing(true)
context.setShouldAntialias(true)

let bodyRect = CGRect(x: 64, y: 64, width: 896, height: 896)
let body = CGPath(
    roundedRect: bodyRect,
    cornerWidth: 218,
    cornerHeight: 218,
    transform: nil
)

// Dark navy squircle with a soft blue glow behind the glyph.
context.saveGState()
context.addPath(body)
context.clip()
let background = CGGradient(
    colorsSpace: CGColorSpaceCreateDeviceRGB(),
    colors: [color("#0A1120"), color("#14213A")] as CFArray,
    locations: [0, 1]
)!
context.drawLinearGradient(
    background,
    start: CGPoint(x: 512, y: 64),
    end: CGPoint(x: 512, y: 960),
    options: []
)
let glow = CGGradient(
    colorsSpace: CGColorSpaceCreateDeviceRGB(),
    colors: [color("#4C8DFF", 0.34), color("#4C8DFF", 0)] as CFArray,
    locations: [0, 1]
)!
context.drawRadialGradient(
    glow,
    startCenter: CGPoint(x: cylinderOnly ? 512 : 470, y: 360),
    startRadius: 0,
    endCenter: CGPoint(x: cylinderOnly ? 512 : 470, y: 360),
    endRadius: 560,
    options: []
)
context.restoreGState()

// Inner rim.
context.addPath(body)
context.setStrokeColor(color("#FFFFFF", 0.07))
context.setLineWidth(2)
context.strokePath()

let cx: CGFloat = cylinderOnly ? 512 : 420
let rx: CGFloat = cylinderOnly ? 238 : 202
let topY: CGFloat = cylinderOnly ? 286 : 300
let bottomY: CGFloat = cylinderOnly ? 738 : 724
let ry: CGFloat = cylinderOnly ? 80 : 72
let kappa: CGFloat = 1.333

let cylinder = CGMutablePath()
cylinder.move(to: CGPoint(x: cx - rx, y: topY))
cylinder.addLine(to: CGPoint(x: cx - rx, y: bottomY))
cylinder.addCurve(
    to: CGPoint(x: cx + rx, y: bottomY),
    control1: CGPoint(x: cx - rx, y: bottomY + ry * kappa),
    control2: CGPoint(x: cx + rx, y: bottomY + ry * kappa)
)
cylinder.addLine(to: CGPoint(x: cx + rx, y: topY))
cylinder.addCurve(
    to: CGPoint(x: cx - rx, y: topY),
    control1: CGPoint(x: cx + rx, y: topY - ry * kappa),
    control2: CGPoint(x: cx - rx, y: topY - ry * kappa)
)
cylinder.closeSubpath()

context.saveGState()
context.addPath(cylinder)
context.clip()
let blue = CGGradient(
    colorsSpace: CGColorSpaceCreateDeviceRGB(),
    colors: [color("#3D7DFF"), color("#7FAEFF"), color("#4C8DFF")] as CFArray,
    locations: [0, 0.52, 1]
)!
context.drawLinearGradient(
    blue,
    start: CGPoint(x: cx - rx, y: topY),
    end: CGPoint(x: cx + rx, y: bottomY),
    options: []
)
// Left-side sheen.
let sheen = CGGradient(
    colorsSpace: CGColorSpaceCreateDeviceRGB(),
    colors: [color("#FFFFFF", 0.20), color("#FFFFFF", 0)] as CFArray,
    locations: [0, 1]
)!
context.drawLinearGradient(
    sheen,
    start: CGPoint(x: cx - rx + 26, y: topY + 20),
    end: CGPoint(x: cx - rx + 150, y: topY + 20),
    options: []
)
// Slight darkening on the right edge.
let shade = CGGradient(
    colorsSpace: CGColorSpaceCreateDeviceRGB(),
    colors: [color("#0B1830", 0), color("#0B1830", 0.24)] as CFArray,
    locations: [0, 1]
)!
context.drawLinearGradient(
    shade,
    start: CGPoint(x: cx + rx - 150, y: 0),
    end: CGPoint(x: cx + rx, y: 0),
    options: []
)
context.restoreGState()

// Top disk.
let topDisk = CGRect(x: cx - rx, y: topY - ry, width: rx * 2, height: ry * 2)
context.setFillColor(color("#A6C8FF"))
context.fillEllipse(in: topDisk)
context.setStrokeColor(color("#D9E6FF", 0.55))
context.setLineWidth(3)
context.strokeEllipse(in: topDisk.insetBy(dx: 1.5, dy: 1.5))

// Disk separator arcs.
for separator in [topY + 138, topY + 278] {
    let arc = CGMutablePath()
    arc.move(to: CGPoint(x: cx - rx + 7, y: separator))
    arc.addCurve(
        to: CGPoint(x: cx + rx - 7, y: separator),
        control1: CGPoint(x: cx - rx + 7, y: separator - ry * 1.18),
        control2: CGPoint(x: cx + rx - 7, y: separator - ry * 1.18)
    )
    context.addPath(arc)
    context.setStrokeColor(color("#122647", 0.55))
    context.setLineWidth(13)
    context.setLineCap(.round)
    context.strokePath()
}

// Bottom lip highlight.
let lip = CGMutablePath()
lip.move(to: CGPoint(x: cx - rx + 28, y: bottomY + 4))
lip.addCurve(
    to: CGPoint(x: cx + rx - 28, y: bottomY + 4),
    control1: CGPoint(x: cx - rx + 28, y: bottomY + ry * 1.06),
    control2: CGPoint(x: cx + rx - 28, y: bottomY + ry * 1.06)
)
context.addPath(lip)
context.setStrokeColor(color("#D9E6FF", 0.20))
context.setLineWidth(6)
context.setLineCap(.round)
context.strokePath()

// SQL cursor: a green I-beam to the right of the database.
if !cylinderOnly {
    let cursorX: CGFloat = 728
    let cursorTop: CGFloat = 338
    let cursorBottom: CGFloat = 686
    let barWidth: CGFloat = 38
    let capWidth: CGFloat = 118
    let capHeight: CGFloat = 26

    context.saveGState()
    context.setShadow(
        offset: CGSize(width: 0, height: 0),
        blur: 34,
        color: color("#3FB950", 0.45)
    )
    context.setFillColor(color("#3FB950"))
    let bar = CGPath(
        roundedRect: CGRect(
            x: cursorX - barWidth / 2,
            y: cursorTop,
            width: barWidth,
            height: cursorBottom - cursorTop
        ),
        cornerWidth: barWidth / 2,
        cornerHeight: barWidth / 2,
        transform: nil
    )
    context.addPath(bar)
    context.fillPath()
    for capY in [cursorTop - 14, cursorBottom - capHeight + 14] {
        let cap = CGPath(
            roundedRect: CGRect(
                x: cursorX - capWidth / 2,
                y: capY,
                width: capWidth,
                height: capHeight
            ),
            cornerWidth: capHeight / 2,
            cornerHeight: capHeight / 2,
            transform: nil
        )
        context.addPath(cap)
        context.fillPath()
    }
    context.restoreGState()
}

guard let image = context.makeImage() else {
    fputs("cannot render image\n", stderr)
    exit(1)
}

let path = arguments.dropFirst().first(where: { !$0.hasPrefix("--") })
    ?? "/tmp/mydb-icon.png"
let url = URL(fileURLWithPath: path)
guard
    let destination = CGImageDestinationCreateWithURL(
        url as CFURL,
        UTType.png.identifier as CFString,
        1,
        nil
    )
else {
    fputs("cannot create image destination\n", stderr)
    exit(1)
}
CGImageDestinationAddImage(destination, image, nil)
guard CGImageDestinationFinalize(destination) else {
    fputs("cannot write \(path)\n", stderr)
    exit(1)
}
print(path)