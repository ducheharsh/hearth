#!/usr/bin/env bash
# Build Hearth.app + Hearth-<version>.dmg and the Homebrew tarball into dist/.
#   packaging/macos/build.sh            # everything
#   SKIP_WEB=1 packaging/macos/build.sh # reuse an existing dashboard build
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
cd "$ROOT"
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
ARCH=$(uname -m)                       # arm64
TRIPLE="aarch64-apple-darwin"
OUT="$ROOT/dist"
APP="$OUT/Hearth.app"

echo "▸ hearth $VERSION ($ARCH)"
if [[ -z "${SKIP_WEB:-}" ]]; then
  pnpm -C web install --frozen-lockfile
  pnpm -C web --filter @hearth/dashboard build
fi
cargo build --release -p hearthd

rm -rf "$OUT" && mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

echo "▸ menu bar app"
swiftc -O -target arm64-apple-macos13 -framework AppKit -framework ServiceManagement \
  -o "$APP/Contents/MacOS/Hearth" packaging/macos/HearthMenu.swift
cp target/release/hearthd "$APP/Contents/Resources/hearthd"
cp packaging/macos/MenuIcon.png "$APP/Contents/Resources/MenuIcon.png"
sed "s/__VERSION__/$VERSION/g" packaging/macos/Info.plist > "$APP/Contents/Info.plist"

echo "▸ icon"
ICONSET="$OUT/AppIcon.iconset"; mkdir -p "$ICONSET"
for s in 16 32 128 256 512; do
  sips -z $s $s packaging/macos/AppIcon-1024.png --out "$ICONSET/icon_${s}x${s}.png" >/dev/null
  d=$((s * 2)); sips -z $d $d packaging/macos/AppIcon-1024.png --out "$ICONSET/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/AppIcon.icns"
rm -rf "$ICONSET"

echo "▸ sign (ad-hoc)"
codesign --force -s - "$APP/Contents/Resources/hearthd"
codesign --force -s - "$APP"

echo "▸ dmg"
STAGE="$OUT/dmg"; mkdir -p "$STAGE"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
hdiutil create -volname "Hearth" -srcfolder "$STAGE" -ov -format UDZO "$OUT/Hearth-$VERSION.dmg" >/dev/null
rm -rf "$STAGE"

echo "▸ homebrew tarball"
TAR="$OUT/hearthd-$VERSION-$TRIPLE.tar.gz"
tar -C target/release -czf "$TAR" hearthd

(cd "$OUT" && shasum -a 256 "Hearth-$VERSION.dmg" "$(basename "$TAR")" | tee SHA256SUMS)
echo "✓ $OUT"
