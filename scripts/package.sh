#!/bin/sh
# Assemble Regie.app and a DMG from an already built release binary. Prints the DMG path.
# Usage: scripts/package.sh [version]   (defaults to the version in Cargo.toml)
set -eu

cd "$(dirname "$0")/.."

version="${1:-$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml)}"
if ! printf '%s\n' "$version" | LC_ALL=C grep -Eq '^(0|[1-9][0-9]*)\.([1-9]|1[0-2])\.(0|[1-9][0-9]*)$' ||
    [ "$(printf '%s' "$version" | wc -l | tr -d ' ')" != 0 ]; then
    echo "invalid version: expected numeric major.month.build without leading zeroes" >&2
    exit 1
fi

binary="target/release/regie"
if [ ! -x "$binary" ]; then
    echo "build first: $binary is missing" >&2
    exit 1
fi

arch="$(uname -m)"
app="dist/Regie.app"
staging="dist/.dmg"
dmg="dist/Regie-v${version}-${arch}.dmg"

rm -rf "$app" "$staging"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources" "$staging"
cp "$binary" "$app/Contents/MacOS/regie"
cp assets/icons/Regie.icns "$app/Contents/Resources/Regie.icns"

cat > "$app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key><string>en</string>
  <key>CFBundleDisplayName</key><string>Regie</string>
  <key>CFBundleName</key><string>Regie</string>
  <key>CFBundleExecutable</key><string>regie</string>
  <key>CFBundleIdentifier</key><string>com.jonathantobe.regie</string>
  <key>CFBundleIconFile</key><string>Regie</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${version}</string>
  <key>CFBundleVersion</key><string>${version}</string>
  <key>LSUIElement</key><true/>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSHumanReadableCopyright</key><string>© 2026 Jonathan Tobe</string>
</dict>
</plist>
EOF

plutil -lint "$app/Contents/Info.plist" >/dev/null
# Ad-hoc signature: enough to run locally; not notarised, so downloads still need
# right-click Open until there is a Developer ID.
codesign --force --sign - "$app"

# Drag-to-install layout.
ditto "$app" "$staging/Regie.app"
ln -s /Applications "$staging/Applications"
hdiutil create -quiet -volname Regie -srcfolder "$staging" -ov -format UDZO "$dmg"
rm -rf "$staging"

echo "$dmg"
