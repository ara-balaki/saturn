#!/bin/sh
# Builds Saturn.app (release build, icon, the Claude plugin inside) and signs it for this Mac.
# `--install` also puts it in /Applications, replacing an older Saturn.app there.
set -eu
cd "$(dirname "$0")/.."

cargo build --release -p saturn

app=target/release/Saturn.app
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp target/release/saturn "$app/Contents/MacOS/saturn"
cp crates/saturn/assets/app/Saturn.icns "$app/Contents/Resources/Saturn.icns"
cp -R plugin "$app/Contents/Resources/plugin"

cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Saturn</string>
  <key>CFBundleDisplayName</key><string>Saturn</string>
  <key>CFBundleIdentifier</key><string>local.saturn.workbench</string>
  <key>CFBundleExecutable</key><string>saturn</string>
  <key>CFBundleIconFile</key><string>Saturn</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.developer-tools</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

# Ad-hoc signature: enough for this Mac to run it and to allow notifications.
codesign --force --deep --sign - "$app"
echo "Built $app ($version)"

if [ "${1:-}" = "--install" ]; then
  rm -rf /Applications/Saturn.app
  cp -R "$app" /Applications/Saturn.app
  echo "Installed /Applications/Saturn.app"
fi
