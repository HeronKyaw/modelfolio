#!/usr/bin/env bash
# Package the release binary as Modelfolio.app (macOS).
# System notifications are most reliable from a bundled app.
set -euo pipefail
cd "$(dirname "$0")/.."

APP_NAME="Modelfolio"
BUNDLE_ID="io.modelfolio.app"
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)

cargo build --release

APP="target/Modelfolio.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

cat > "$APP/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>             <string>${APP_NAME}</string>
    <key>CFBundleDisplayName</key>      <string>Modelfolio</string>
    <key>CFBundleIdentifier</key>       <string>${BUNDLE_ID}</string>
    <key>CFBundleVersion</key>          <string>${VERSION}</string>
    <key>CFBundleShortVersionString</key> <string>${VERSION}</string>
    <key>CFBundleExecutable</key>       <string>modelfolio</string>
    <key>CFBundlePackageType</key>      <string>APPL</string>
    <key>CFBundleInfoDictionaryVersion</key> <string>6.0</string>
    <key>LSMinimumSystemVersion</key>   <string>11.0</string>
    <key>NSHighResolutionCapable</key>  <true/>
</dict>
</plist>
EOF

cp target/release/modelfolio "$APP/Contents/MacOS/"
codesign --force --sign - "$APP" 2>/dev/null || true

echo "Bundled at $APP"
