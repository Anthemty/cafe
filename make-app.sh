#!/usr/bin/env bash
# Build cafe into a proper macOS .app bundle with an app icon.
#
# Produces: dist/cafe.app
#
# The bundle lets the app show a real icon, a proper name in the menu bar
# "Force Quit" list, and avoids the Gatekeeper "unidentified developer"
# friction for unsigned-but-locally-built apps (the user can still right-click
# → Open the first time).
set -euo pipefail

cd "$(dirname "$0")"
ROOT="$(pwd)"
DIST="$ROOT/dist"
APP="$DIST/cafe.app"
RES="$ROOT/resources"

echo "==> Building release binary (universal: aarch64 + x86_64)"
if [[ "${CAFE_SKIP_UNIVERSAL:-0}" == "1" ]]; then
    # Fallback: native arch only (e.g. CI runners without cross targets).
    cargo build --release
    cp target/release/cafe "$DIST/cafe-bin"
else
    rustup target list --installed | grep -q aarch64-apple-darwin || rustup target add aarch64-apple-darwin
    rustup target list --installed | grep -q x86_64-apple-darwin || rustup target add x86_64-apple-darwin
    mkdir -p "$DIST"
    cargo build --release --target aarch64-apple-darwin
    cargo build --release --target x86_64-apple-darwin
    lipo -create \
        target/aarch64-apple-darwin/release/cafe \
        target/x86_64-apple-darwin/release/cafe \
        -output "$DIST/cafe-bin"
fi

echo "==> Rendering app icon"
mkdir -p "$RES"
TMP_LOGO="$RES/logo_1024.png"
SWIFT_OUT="$TMP_LOGO" swift - <<'SWIFT'
import AppKit

func render(size: CGFloat) -> NSImage {
    let img = NSImage(size: NSSize(width: size, height: size))
    img.lockFocusFlipped(true)
    let ctx = NSGraphicsContext.current!.cgContext
    let rect = CGRect(x: 0, y: 0, width: size, height: size)

    // Background gradient: warm brown to dark espresso.
    let colors = [NSColor(srgbRed: 0.30, green: 0.17, blue: 0.10, alpha: 1.0).cgColor,
                  NSColor(srgbRed: 0.16, green: 0.09, blue: 0.06, alpha: 1.0).cgColor] as CFArray
    let grad = CGGradient(colorsSpace: CGColorSpaceCreateDeviceRGB(),
                          colors: colors, locations: [0.0, 1.0])!
    ctx.saveGState()
    let path = CGPath(roundedRect: rect.insetBy(dx: size*0.04, dy: size*0.04),
                      cornerWidth: size*0.22, cornerHeight: size*0.22, transform: nil)
    ctx.addPath(path)
    ctx.clip()
    ctx.drawLinearGradient(grad, start: CGPoint(x: 0, y: size),
                           end: CGPoint(x: size, y: 0), options: [])
    ctx.restoreGState()

    // Coffee cup symbol, cream-colored, centered, scaled to ~62%.
    let cfg = NSImage.SymbolConfiguration(pointSize: size*0.52, weight: .semibold)
    if let cup = NSImage(systemSymbolName: "cup.and.saucer.fill", accessibilityDescription: nil)?
        .withSymbolConfiguration(cfg) {
        let cupColor = NSColor(srgbRed: 0.98, green: 0.92, blue: 0.80, alpha: 1.0)
        let cupCfg = NSImage.SymbolConfiguration(hierarchicalColor: cupColor)
        if let tinted = cup.withSymbolConfiguration(cupCfg) {
            let s = size * 0.62
            tinted.draw(in: NSRect(x: (size-s)/2, y: (size-s)/2, width: s, height: s))
        }
    }
    img.unlockFocus()
    return img
}

func savePNG(_ img: NSImage, to path: String) {
    let rep = NSBitmapImageRep(data: img.tiffRepresentation!)!
    let png = rep.representation(using: .png, properties: [:])!
    try! png.write(to: URL(fileURLWithPath: path))
}

let out = ProcessInfo.processInfo.environment["SWIFT_OUT"]!
savePNG(render(size: 1024), to: out)
SWIFT

echo "==> Building .icns"
ICONSET="$(mktemp -d -t cafe_iconset)/AppIcon.iconset"
mkdir -p "$ICONSET"
for spec in "16 16" "32 16@2x" "32 32" "64 32@2x" "128 128" "256 128@2x" "256 256" "512 256@2x" "512 512" "1024 512@2x"; do
    set -- $spec; px="$1"; name="$2"
    sips -s format png -z "$px" "$px" "$TMP_LOGO" --out "$ICONSET/icon_${name}.png" >/dev/null
done
ICNS="$RES/AppIcon.icns"
iconutil -c icns "$ICONSET" -o "$ICNS"

echo "==> Assembling bundle at $APP"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$DIST/cafe-bin" "$APP/Contents/MacOS/cafe"
cp "$ICNS" "$APP/Contents/Resources/AppIcon.icns"

cat > "$APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>cafe</string>
    <key>CFBundleDisplayName</key>
    <string>Cafe</string>
    <key>CFBundleIdentifier</key>
    <string>dev.cafe.app</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>CFBundleShortVersionString</key>
    <string>0.1.0</string>
    <key>CFBundleExecutable</key>
    <string>cafe</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>LSMinimumSystemVersion</key>
    <string>11.0</string>
    <!-- Run as a menu bar accessory: no Dock icon, no main window. -->
    <key>LSUIElement</key>
    <true/>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSHumanReadableCopyright</key>
    <string>MIT</string>
</dict>
</plist>
PLIST

# Refresh icon cache so Finder picks up the icon immediately.
touch "$APP"

echo "==> Done: $APP"
echo "    Run with: open $APP"
