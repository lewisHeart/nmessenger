#!/usr/bin/env bash
set -euo pipefail

tag="${1:-v0.1.0-alpha}"
target="${2:-x86_64-unknown-linux-gnu}"

case "$target" in
  x86_64-unknown-linux-gnu) arch=x86_64 ;;
  aarch64-unknown-linux-gnu) arch=aarch64 ;;
  *) printf 'Unsupported Linux target: %s\n' "$target" >&2; exit 1 ;;
esac

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

binary="$root/target/$target/release/nmessenger-client"
if [[ ! -f "$binary" ]]; then
  # Fallback to target/release/nmessenger-client if non-cross compiled
  binary="$root/target/release/nmessenger-client"
fi

if [[ ! -f "$binary" ]]; then
  echo "Error: Binary not found at $binary" >&2
  exit 1
fi

name="nmessenger-$tag-$arch"
dist="$root/dist"
mkdir -p "$dist" "$work/flatpak" "$work/AppDir/usr/bin" "$work/AppDir/usr/share/applications" "$work/AppDir/usr/share/icons/hicolor/scalable/apps" "$work/AppDir/usr/share/doc/nmessenger"

# 1. Prepare files
install -m755 "$binary" "$work/flatpak/nmessenger"
cp "$root/packaging/linux/com.nmessenger.NMessenger.desktop" "$work/flatpak/"
cp "$root/packaging/linux/com.nmessenger.NMessenger.svg" "$work/flatpak/"
cp "$root/packaging/linux/com.nmessenger.NMessenger.metainfo.xml" "$work/flatpak/"
cp "$root/packaging/linux/com.nmessenger.NMessenger.yml" "$work/flatpak/"
cp "$root/packaging/linux/nmessenger-launch" "$work/flatpak/"
chmod +x "$work/flatpak/nmessenger-launch"
cp "$root/LICENSE" "$work/flatpak/LICENSE"
cp "$root/README.md" "$work/flatpak/README.md"

# 2. Package AppImage
echo "=== Packaging AppImage for $arch ==="
if command -v linuxdeploy >/dev/null 2>&1; then
  LINUXDEPLOY="linuxdeploy"
else
  LINUXDEPLOY="$work/linuxdeploy.AppImage"
  if [[ ! -f "$LINUXDEPLOY" ]]; then
    echo "Downloading linuxdeploy-$arch.AppImage..."
    curl --fail --location --retry 3 "https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-$arch.AppImage" -o "$LINUXDEPLOY" || true
    chmod +x "$LINUXDEPLOY" 2>/dev/null || true
  fi
fi

if [[ -x "$LINUXDEPLOY" ]]; then
  export APPIMAGE_EXTRACT_AND_RUN=1
  export LDAI_OUTPUT="$dist/$name.AppImage"

  cp "$work/flatpak/nmessenger" "$work/AppDir/usr/bin/nmessenger"
  cp "$work/flatpak/com.nmessenger.NMessenger.desktop" "$work/AppDir/com.nmessenger.NMessenger.desktop"
  cp "$work/flatpak/com.nmessenger.NMessenger.svg" "$work/AppDir/com.nmessenger.NMessenger.svg"

  "$LINUXDEPLOY" --appdir "$work/AppDir" \
    --executable "$work/flatpak/nmessenger" \
    --desktop-file "$work/flatpak/com.nmessenger.NMessenger.desktop" \
    --icon-file "$work/flatpak/com.nmessenger.NMessenger.svg" \
    --output appimage || echo "Warning: linuxdeploy failed; skipping AppImage generation."
else
  echo "Notice: linuxdeploy not available; AppImage skipped."
fi

# 3. Package Flatpak
if command -v flatpak-builder >/dev/null 2>&1; then
  echo "=== Packaging Flatpak for $arch ==="
  flatpak remote-add --user --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo || true
  flatpak-builder --user --install-deps-from=flathub --force-clean \
    --state-dir="$work/state" --repo="$work/repo" \
    "$work/build" "$work/flatpak/com.nmessenger.NMessenger.yml" || echo "Warning: flatpak-builder build failed."

  if [[ -d "$work/repo" ]]; then
    flatpak build-bundle --runtime-repo=https://flathub.org/repo/flathub.flatpakrepo \
      "$work/repo" "$dist/$name.flatpak" com.nmessenger.NMessenger || echo "Warning: flatpak build-bundle failed."
  fi
else
  echo "Notice: flatpak-builder not installed; Flatpak skipped."
fi

# 4. Package Portable Tarball
echo "=== Packaging Portable Tarball for $arch ==="
stage="$dist/nmessenger-$tag-$arch"
rm -rf "$stage"
mkdir -p "$stage"
cp "$work/flatpak/nmessenger" "$stage/"
cp "$work/flatpak/com.nmessenger.NMessenger.desktop" "$stage/"
cp "$work/flatpak/com.nmessenger.NMessenger.svg" "$stage/"
cp "$work/flatpak/LICENSE" "$stage/"
cp "$work/flatpak/README.md" "$stage/"

tar -czf "$dist/$name.tar.gz" -C "$dist" "nmessenger-$tag-$arch"
rm -rf "$stage"

echo "Linux packaging complete in $dist"

