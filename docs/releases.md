# Building & Releases

NMessenger provides a unified multi-platform build and packaging system supporting Linux, Windows, macOS, and Android.

---

## 1. Supported Distribution Matrix

| Platform | Architecture | Formats Generated |
| :--- | :--- | :--- |
| **Linux** | `amd64` (x86_64) | `.AppImage`, `.flatpak`, `.tar.gz` (portable) |
| **Linux** | `arm64` (aarch64) | `.AppImage`, `.flatpak`, `.tar.gz` (portable) |
| **Windows** | `amd64` (x86_64) | `-setup.exe` (NSIS installer), `.zip` (portable) |
| **Windows** | `arm64` (aarch64) | `-setup.exe` (NSIS installer), `.zip` (portable) |
| **macOS** | `Intel` (x86_64) | `NMessenger.app`, `.dmg`, `.tar.gz` |
| **macOS** | `Apple Silicon` (arm64) | `NMessenger.app`, `.dmg`, `.tar.gz` |
| **Android** | `arm64-v8a` & `x86_64` | `.apk` (signed, 16 KB page-aligned) |

---

## 2. Using `scripts/build_all.py`

The unified orchestrator manages compiling, asset gathering, packaging, and checksum generation:

```sh
# Check available compilers, tools, and Android SDK
python3 scripts/build_all.py --check
# or: make dist-check

# Compile and package everything available on this workstation
python3 scripts/build_all.py
# or: make dist

# Build only for a specific platform:
python3 scripts/build_all.py --platform linux
python3 scripts/build_all.py --platform windows
python3 scripts/build_all.py --platform macos
python3 scripts/build_all.py --platform android

# Package existing compiled binaries without recompiling
python3 scripts/build_all.py --package-only
```

All distribution artifacts are written to `dist/` alongside a cryptographically verified `dist/SHA256SUMS.txt`.

---

## 3. Platform Packaging Details

### Linux AppImage
Packaged using `linuxdeploy`. The script automatically bundles dependencies, generates the desktop file symlinks, and sets up high-resolution application icons.

### Linux Flatpak
Defined in `packaging/linux/com.nmessenger.NMessenger.yml`.
Compiled using `flatpak-builder` and bundled via `flatpak build-bundle` against the standard Freedesktop 24.08 runtime.

### Windows Installer (NSIS)
Defined in `packaging/windows/installer.nsi`.
Generates a modern user-level installer:
- Installs to `%LOCALAPPDATA%\Programs\NMessenger` (no admin rights required).
- Creates Start Menu and Desktop shortcuts.
- Registers an uninstaller in Windows *Add or Remove Programs*.
- Can be compiled natively on Windows with NSIS (`makensis`) or from Linux via Docker.

### macOS App Bundle & DMG
Defined in `packaging/macos/Info.plist` and `packaging/macos/package_macos.sh`.
Assembles `NMessenger.app` with Retina display support and camera/microphone privacy entitlements, generating a compressed `.dmg` and portable `.tar.gz`.

### Android APK
Built directly on Linux x86-64 without Gradle via `scripts/build_android.py`:
- Target SDK 35 with mandatory 16 KB memory page alignment (`zipalign -P 16 4`).
- Minimum SDK 26 (Android 8.0+).
- Signed using `apksigner` with APK Signature Scheme v2 and v3.
- Production signing credentials configured via `NM_ANDROID_KEYSTORE`, `NM_ANDROID_KEY_ALIAS`, `NM_ANDROID_STOREPASS`, and `NM_ANDROID_KEYPASS`.

---

## 4. GitHub Actions Automated Releases

The repository includes a multi-platform release workflow at `.github/workflows/release.yml`.

Set the repository secrets `NM_ANDROID_KEYSTORE_BASE64`, `NM_ANDROID_KEY_ALIAS`,
`NM_ANDROID_STOREPASS`, and `NM_ANDROID_KEYPASS` to the existing production
Android signing identity. Preserve and back up this keystore: changing it breaks
updates of already-installed APKs. The workflow sets `NM_ANDROID_VERSION_CODE`
from the monotonically increasing workflow run number. If releases were signed
elsewhere, ensure the next run number is greater than their version code first.

When a git tag matching the workspace version (e.g. `v0.1.0-alpha`) is pushed or manually triggered via GitHub Actions:
1. Native matrix runners (Ubuntu x86_64, Ubuntu ARM64, Windows x86_64, Windows ARM64, macOS Intel, macOS Apple Silicon) build and package their respective native artifacts.
2. The Android job compiles and signs ARM64 and x86_64 APKs.
3. The coordinator verifies that every required artifact exists, computes `SHA256SUMS.txt`, creates a draft, uploads the assets, and then publishes it. Tags containing a hyphen are marked as prereleases.

Windows and macOS packages require separate platform signing and notarization
credentials for a fully trusted distribution; the current workflow does not sign
or notarize those packages. The client verifies GitHub's SHA-256 asset digest and
saves the selected package; installation still requires opening it manually.
