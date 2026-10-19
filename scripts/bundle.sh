#!/bin/sh
# Builds Saturn.app (release build, icon, the Claude plugin inside) and signs it ad hoc.
# `--install` also puts it in /Applications, replacing an older Saturn.app there.
# `--dmg` builds it for Apple Silicon and Intel and packs it into a disk image to share.
set -eu
cd "$(dirname "$0")/.."
mode=${1:-}

if [ "$mode" = "--dmg" ]; then
  cargo build --release -p saturn --target aarch64-apple-darwin
  cargo build --release -p saturn --target x86_64-apple-darwin
  binary=target/release/saturn-universal
  mkdir -p target/release
  lipo -create -output "$binary" \
    target/aarch64-apple-darwin/release/saturn target/x86_64-apple-darwin/release/saturn
else
  cargo build --release -p saturn
  binary=target/release/saturn
fi

# A `.noindex` folder, so Spotlight doesn't list the build next to the installed app.
out=target/bundle.noindex
app=$out/Saturn.app
rm -rf target/release/Saturn.app target/release/dmg
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
rm -rf "$app"
mkdir -p "$out" "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$binary" "$app/Contents/MacOS/saturn"
cp crates/saturn/assets/app/Saturn.icns "$app/Contents/Resources/Saturn.icns"
# Sparkle, which Saturn loads at launch to update itself from the GitHub releases.
sparkle=$(sh scripts/sparkle.sh)
mkdir -p "$app/Contents/Frameworks"
cp -R "$sparkle/Sparkle.framework" "$app/Contents/Frameworks/"
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
  <!-- Updates: the newest GitHub release's feed, signed with the key in the publisher's keychain;
       checked every hour, the shortest Sparkle allows. -->
  <key>SUFeedURL</key><string>https://github.com/itsjonasarden/saturn/releases/latest/download/appcast.xml</string>
  <key>SUPublicEDKey</key><string>L6qK9LpfH0C48mSjQNc4JAnNxO7rokEYbdIR5gemae0=</string>
  <key>SUEnableAutomaticChecks</key><true/>
  <key>SUScheduledCheckInterval</key><integer>3600</integer>
</dict>
</plist>
PLIST

# Ad-hoc signature: enough to run it and to allow notifications. Without an Apple Developer ID
# it isn't notarised, so a Mac that downloaded it asks once (System Settings → Privacy &
# Security → Open Anyway).
codesign --force --deep --sign - "$app"
echo "Built $app ($version)"

if [ "$mode" = "--dmg" ]; then
  stage=$out/dmg
  rm -rf "$stage"
  mkdir -p "$stage"
  cp -R "$app" "$stage/Saturn.app"
  ln -s /Applications "$stage/Applications"
  cp scripts/how-to-open.txt "$stage/How to open Saturn.txt"
  dmg="target/Saturn-$version.dmg"
  rm -f "$dmg"
  hdiutil create -quiet -volname "Saturn $version" -srcfolder "$stage" -format UDZO "$dmg"
  rm -rf "$stage"
  echo "Built $dmg"
fi

if [ "$mode" = "--install" ]; then
  rm -rf /Applications/Saturn.app
  cp -R "$app" /Applications/Saturn.app
  echo "Installed /Applications/Saturn.app"
fi
