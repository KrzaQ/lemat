// Draws the Lemat app icon: the sign ∴ ("therefore") in chalk on a dark
// board, and writes it as a 1024 x 1024 PNG. `make icon` then lets the
// Tauri CLI make every size from it.
//
//   swift scripts/make-icon.swift <output.png>

import AppKit

let side: CGFloat = 1024
let board = NSColor(srgbRed: 0.137, green: 0.157, blue: 0.149, alpha: 1)  // #232826
let frame = NSColor(srgbRed: 0.545, green: 0.420, blue: 0.290, alpha: 1)  // #8B6B4A
let chalk = NSColor(srgbRed: 0.949, green: 0.941, blue: 0.910, alpha: 1)  // #F2F0E8

guard CommandLine.arguments.count == 2 else {
    FileHandle.standardError.write("usage: make-icon.swift <output.png>\n".data(using: .utf8)!)
    exit(2)
}

let image = NSImage(size: NSSize(width: side, height: side))
image.lockFocus()

// macOS icons leave a margin around the rounded square: 824 of 1024.
let inset: CGFloat = 100
let box = side - 2 * inset
let outer = NSRect(x: inset, y: inset, width: box, height: box)
frame.setFill()
NSBezierPath(roundedRect: outer, xRadius: box * 0.225, yRadius: box * 0.225).fill()
let rim: CGFloat = 34
board.setFill()
NSBezierPath(roundedRect: outer.insetBy(dx: rim, dy: rim),
             xRadius: box * 0.19, yRadius: box * 0.19).fill()

// ∴: one dot above, two below, on an equilateral triangle.
chalk.setFill()
let radius: CGFloat = 52
let spread: CGFloat = 150
let middle = side / 2
let height = spread * sqrt(3)
for (x, y) in [(middle, middle + height / 2),
               (middle - spread, middle - height / 2),
               (middle + spread, middle - height / 2)] {
    NSBezierPath(ovalIn: NSRect(x: x - radius, y: y - radius,
                                width: 2 * radius, height: 2 * radius)).fill()
}

image.unlockFocus()

let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: Int(side), pixelsHigh: Int(side),
                           bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
                           colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
image.draw(in: NSRect(x: 0, y: 0, width: side, height: side))
NSGraphicsContext.restoreGraphicsState()
try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: CommandLine.arguments[1]))
