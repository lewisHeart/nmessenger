#!/usr/bin/env bash
set -euo pipefail

tag="${1:-v0.1.0-alpha}"
target="${2:-x86_64-apple-darwin}"

case "$target" in
  x86_64-apple-darwin) arch=x86_64 ;;
  aarch64-apple-darwin) arch=arm64 ;;
  *) printf 'Unsupported macOS target: %s\n' "$target" >&2; exit 1 ;;
esac

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

binary="$root/target/$target/release/nmessenger-client"
if [[ ! -f "$binary" ]]; then
  binary="$root/target/release/nmessenger-client"
fi

if [[ ! -f "$binary" ]]; then
  echo "Error: Binary not found at $binary" >&2
  exit 1
fi

dist="$root/dist"
mkdir -p "$dist"

app_dir="$work/NMessenger.app"
mkdir -p "$app_dir/Contents/MacOS" "$app_dir/Contents/Resources"

install -m755 "$binary" "$app_dir/Contents/MacOS/nmessenger"
cp "$root/packaging/macos/Info.plist" "$app_dir/Contents/Info.plist"
sed -i "s/0.1.0-alpha/${tag#v}/g" "$app_dir/Contents/Info.plist" 2>/dev/null || true

# Copy logo icon
cp "$root/assets/logo.svg" "$app_dir/Contents/Resources/AppIcon.svg" 2>/dev/null || true

name="nmessenger-$tag-macos-$arch"

# 1. Package Tarball containing .app
tar -czf "$dist/$name.tar.gz" -C "$work" "NMessenger.app"

# 2. Package DMG if hdiutil is available (macOS)
if command -v hdiutil >/dev/null 2>&1; then
  echo "=== Creating DMG for $arch ==="
  hdiutil create -volname "NMessenger" -srcfolder "$work" -ov -format UDZO "$dist/$name.dmg"
else
  echo "Notice: hdiutil not found (non-macOS host); .dmg skipped (tar.gz generated)."
fi

echo "macOS packaging complete in $dist"

