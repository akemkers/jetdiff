#!/bin/sh
# Wraps the jetdiff binary in jetdiff.app, so macOS shows the icon and the name
# in the Dock and the app switcher.
#
# usage: scripts/release/bundle.sh <binary> <version> <out-dir>
set -eu

binary=$1
version=$2
out=$3
root=$(cd "$(dirname "$0")/../.." && pwd)
app=$out/jetdiff.app

rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$binary" "$app/Contents/MacOS/jetdiff"

iconset=$(mktemp -d)/jetdiff.iconset
mkdir -p "$iconset"
for size in 16 32 128 256 512; do
  sips -z $size $size "$root/assets/icon.png" --out "$iconset/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  sips -z $double $double "$root/assets/icon.png" --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/jetdiff.icns"

cat > "$app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>jetdiff</string>
  <key>CFBundleIdentifier</key><string>io.github.akemkers.jetdiff</string>
  <key>CFBundleExecutable</key><string>jetdiff</string>
  <key>CFBundleIconFile</key><string>jetdiff</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
EOF

# Ad hoc, so the bundle's signature matches what is in it. Apple Silicon won't run an invalid one.
codesign --force --deep --sign - "$app"
