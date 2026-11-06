#!/usr/bin/env python3
"""Unified build and packaging system for NMessenger across Linux, Windows, macOS, and Android.

Supported Matrix:
- Windows (amd64 / x86_64): Portable .zip, NSIS setup installer (.exe)
- Windows (arm64 / aarch64): Portable .zip, NSIS setup installer (.exe)
- Linux (amd64 / x86_64): Portable tar.gz, AppImage (.AppImage), Flatpak (.flatpak)
- Linux (arm64 / aarch64): Portable tar.gz, AppImage (.AppImage), Flatpak (.flatpak)
- macOS (Intel / x86_64): Portable tar.gz, .app bundle, Disk Image (.dmg)
- macOS (Apple Silicon / arm64): Portable tar.gz, .app bundle, Disk Image (.dmg)
- Android: APK (arm64-v8a, x86_64)
"""

import argparse
import hashlib
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tarfile
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parent.parent

TARGETS = {
    # Linux
    "linux-x86_64": {
        "platform": "linux",
        "arch": "x86_64",
        "rust_target": "x86_64-unknown-linux-gnu",
        "binary_name": "nmessenger-client",
        "output_name": "nmessenger-{version}-linux-x86_64",
    },
    "linux-arm64": {
        "platform": "linux",
        "arch": "arm64",
        "rust_target": "aarch64-unknown-linux-gnu",
        "binary_name": "nmessenger-client",
        "output_name": "nmessenger-{version}-linux-arm64",
    },
    # Windows
    "windows-x86_64": {
        "platform": "windows",
        "arch": "x86_64",
        "rust_target": "x86_64-pc-windows-gnu" if sys.platform != "win32" else "x86_64-pc-windows-msvc",
        "fallback_target": "x86_64-pc-windows-msvc" if sys.platform != "win32" else "x86_64-pc-windows-gnu",
        "binary_name": "nmessenger-client.exe",
        "output_name": "nmessenger-{version}-windows-x86_64",
    },
    "windows-arm64": {
        "platform": "windows",
        "arch": "arm64",
        "rust_target": "aarch64-pc-windows-msvc",
        "fallback_target": "aarch64-pc-windows-gnullvm",
        "binary_name": "nmessenger-client.exe",
        "output_name": "nmessenger-{version}-windows-arm64",
    },
    # macOS
    "macos-x86_64": {
        "platform": "macos",
        "arch": "x86_64",
        "rust_target": "x86_64-apple-darwin",
        "binary_name": "nmessenger-client",
        "output_name": "nmessenger-{version}-macos-x86_64",
    },
    "macos-arm64": {
        "platform": "macos",
        "arch": "arm64",
        "rust_target": "aarch64-apple-darwin",
        "binary_name": "nmessenger-client",
        "output_name": "nmessenger-{version}-macos-arm64",
    },
    # Android
    "android-arm64": {
        "platform": "android",
        "arch": "arm64",
        "rust_target": "aarch64-linux-android",
        "abi": "arm64-v8a",
    },
    "android-x86_64": {
        "platform": "android",
        "arch": "x86_64",
        "rust_target": "x86_64-linux-android",
        "abi": "x86_64",
    },
}


def get_version():
    with (ROOT / "Cargo.toml").open("rb") as f:
        data = tomllib.load(f)
    return data.get("workspace", {}).get("package", {}).get("version", "0.1.0-alpha")


def check_tool(name):
    return shutil.which(name) is not None


def get_installed_rust_targets():
    try:
        res = subprocess.run(["rustup", "target", "list", "--installed"], cwd=ROOT, capture_output=True, text=True, check=True)
        return set(res.stdout.splitlines())
    except Exception:
        return set()


def run_cmd(cmd, env=None, cwd=ROOT, allow_failure=False):
    print(f"+ {' '.join(str(c) for c in cmd)}", flush=True)
    res = subprocess.run(cmd, env=env, cwd=cwd)
    if res.returncode != 0 and not allow_failure:
        raise RuntimeError(f"Command failed with exit code {res.returncode}: {' '.join(str(c) for c in cmd)}")
    return res.returncode == 0


def build_rust_binary(target, release=True, package="nmessenger-client"):
    installed_targets = get_installed_rust_targets()
    if target not in installed_targets:
        print(f"Adding rust target {target} via rustup...")
        subprocess.run(["rustup", "target", "add", target], check=False)

    args = ["cargo", "build", "-p", package]
    if release:
        args.append("--release")
    if target:
        args.extend(["--target", target])

    run_cmd(args)


def find_binary(rust_target, binary_name, release=True):
    profile = "release" if release else "debug"
    candidates = [ROOT / "target" / rust_target / profile / binary_name]
    for candidate in candidates:
        if candidate.is_file():
            return candidate
    return None


def package_linux(tag, target_info, dist_dir, binary, formats):
    arch = target_info["arch"]
    rust_target = target_info["rust_target"]
    name = f"nmessenger-{tag}-linux-{arch}"

    # 1. Portable Tarball
    if "all" in formats or "portable" in formats:
        stage = dist_dir / name
        if stage.exists():
            shutil.rmtree(stage)
        stage.mkdir(parents=True)
        shutil.copy2(binary, stage / "nmessenger")
        shutil.copy2(ROOT / "packaging/linux/com.nmessenger.NMessenger.desktop", stage / "nmessenger.desktop")
        shutil.copy2(ROOT / "packaging/linux/com.nmessenger.NMessenger.svg", stage / "nmessenger.svg")
        shutil.copy2(ROOT / "LICENSE", stage / "LICENSE")
        shutil.copy2(ROOT / "README.md", stage / "README.md")

        tar_path = dist_dir / f"{name}.tar.gz"
        print(f"Creating Linux tarball: {tar_path}")
        with tarfile.open(tar_path, "w:gz") as tar:
            tar.add(stage, arcname=name)
        shutil.rmtree(stage)

    # 2. AppImage and Flatpak via package_linux.sh
    if any(f in formats for f in ("all", "appimage", "flatpak")):
        pkg_script = ROOT / "packaging/linux/package_linux.sh"
        if pkg_script.is_file():
            print(f"Running Linux packaging script: {pkg_script}")
            run_cmd(["bash", str(pkg_script), tag, rust_target])


def package_windows(tag, target_info, dist_dir, binary, formats):
    arch = target_info["arch"]
    name = f"nmessenger-{tag}-windows-{arch}"

    # 1. Portable Zip
    if "all" in formats or "portable" in formats:
        zip_path = dist_dir / f"{name}.zip"
        print(f"Creating Windows portable zip: {zip_path}")
        with zipfile.ZipFile(zip_path, "w", zipfile.ZIP_DEFLATED) as z:
            z.write(binary, "nmessenger.exe")
            z.write(ROOT / "LICENSE", "LICENSE.txt")
            z.write(ROOT / "README.md", "README.txt")

    # 2. NSIS Installer
    if "all" in formats or "installer" in formats:
        installer_out = dist_dir / f"{name}-setup.exe"
        nsis_script = ROOT / "packaging/windows/installer.nsi"
        
        makensis = shutil.which("makensis")
        if makensis:
            print(f"Building Windows NSIS Installer: {installer_out}")
            cmd = [
                makensis,
                f"-DVERSION={tag.lstrip('v')}",
                f"-DPROJECT_ROOT={ROOT}",
                f"-DBINARY={binary}",
                f"-DOUTPUT={installer_out}",
                str(nsis_script),
            ]
            run_cmd(cmd)
        elif check_tool("docker"):
            inspect = subprocess.run(["docker", "image", "inspect", "nmessenger-nsis"], capture_output=True)
            if inspect.returncode != 0:
                print("Building local nmessenger-nsis docker image...")
                subprocess.run([
                    "docker", "build", "-t", "nmessenger-nsis", "-"
                ], input=b"FROM debian:bookworm-slim\nRUN apt-get update -qq && apt-get install -qq -y --no-install-recommends nsis ca-certificates && rm -rf /var/lib/apt/lists/*\nWORKDIR /work\nENTRYPOINT [\"makensis\"]\n", check=True)
            print(f"Building Windows NSIS Installer via Docker (nmessenger-nsis): {installer_out}")
            cmd = [
                "docker", "run", "--rm",
                "-v", f"{ROOT}:/work",
                "-w", "/work",
                "nmessenger-nsis",
                f"-DVERSION={tag.lstrip('v')}",
                f"-DPROJECT_ROOT=/work",
                f"-DBINARY=/work/{binary.relative_to(ROOT)}",
                f"-DOUTPUT=/work/dist/{name}-setup.exe",
                "packaging/windows/installer.nsi",
            ]
            run_cmd(cmd)
        else:
            raise RuntimeError("makensis not found and docker unavailable")


def package_macos(tag, target_info, dist_dir, binary, formats):
    arch = target_info["arch"]
    name = f"nmessenger-{tag}-macos-{arch}"
    rust_target = target_info["rust_target"]

    # Run packaging script
    pkg_script = ROOT / "packaging/macos/package_macos.sh"
    if pkg_script.is_file():
        print(f"Running macOS packaging script: {pkg_script}")
        run_cmd(["bash", str(pkg_script), tag, rust_target])


def build_android(tag, target_info, dist_dir, is_production=False):
    arch = target_info["arch"]
    script = ROOT / "scripts/build_android.py"
    if not script.is_file():
        print("Android build script not found.")
        return

    cmd = ["python3", str(script), "--arch", arch]
    if is_production:
        cmd.append("--production")
    else:
        cmd.append("--debug")

    print(f"=== Building Android APK for {arch} ===")
    run_cmd(cmd)
    abi = target_info["abi"]
    profile, signing = ("release", "production") if is_production else ("debug", "development")
    apk = ROOT / "target/android" / f"nmessenger-{abi}-{profile}-{signing}.apk"
    if not apk.is_file():
        raise RuntimeError(f"Expected APK not found: {apk}")
    out_apk = dist_dir / f"nmessenger-{tag}-android-{abi}.apk"
    shutil.copy2(apk, out_apk)
    print(f"Generated Android APK: {out_apk}")


def generate_checksums(dist_dir):
    artifacts = sorted(
        p for p in dist_dir.glob("*")
        if p.is_file() and p.name != "SHA256SUMS.txt" and not p.name.endswith(".json")
    )
    if not artifacts:
        print("No artifacts found to checksum.")
        return

    lines = []
    for art in artifacts:
        digest = hashlib.sha256(art.read_bytes()).hexdigest()
        lines.append(f"{digest}  {art.name}\n")

    sums_file = dist_dir / "SHA256SUMS.txt"
    sums_file.write_text("".join(lines), encoding="utf-8")
    print(f"\nSHA-256 Checksums written to: {sums_file}")
    for line in lines:
        print(f"  {line.strip()}")


def check_prerequisites():
    print("=== Checking Toolchains and Prerequisites ===")
    installed_targets = get_installed_rust_targets()
    print("Installed Rust targets:")
    for t in sorted(installed_targets):
        print(f"  - {t}")

    print("\nTools status:")
    tools = {
        "cargo": check_tool("cargo"),
        "rustup": check_tool("rustup"),
        "gcc / clang": check_tool("gcc") or check_tool("clang"),
        "flatpak-builder": check_tool("flatpak-builder"),
        "appimagetool": check_tool("appimagetool"),
        "makensis (Windows NSIS)": check_tool("makensis"),
        "hdiutil (macOS DMG)": check_tool("hdiutil"),
        "docker": check_tool("docker"),
    }
    for tool, present in tools.items():
        status = "OK" if present else "MISSING"
        print(f"  [{status:^7}] {tool}")

    print("\nAndroid environment:")
    try:
        res = subprocess.run(["python3", "scripts/build_android.py", "--check"], cwd=ROOT, capture_output=True, text=True)
        if res.returncode == 0:
            print("  [  OK   ] Android SDK & NDK available")
            for line in res.stdout.strip().splitlines():
                print(f"    {line}")
        else:
            print("  [MISSING] Android SDK or NDK: " + res.stderr.strip())
    except Exception as e:
        print(f"  [MISSING] Android SDK check error: {e}")


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--platform", choices=["all", "linux", "windows", "macos", "android"], default="all",
                        help="Filter target platform (default: all)")
    parser.add_argument("--arch", choices=["all", "x86_64", "arm64"], default="all",
                        help="Filter target architecture (default: all)")
    parser.add_argument("--format", choices=["all", "portable", "installer", "appimage", "flatpak", "apk", "dmg"],
                        default="all", help="Filter output package formats (default: all)")
    parser.add_argument("--target", help="Specific target key (e.g. linux-x86_64, windows-x86_64, etc.)")
    parser.add_argument("--check", action="store_true", help="Check installed tools and targets without building")
    parser.add_argument("--package-only", action="store_true", help="Package existing compiled binaries without compiling")
    parser.add_argument("--production", action="store_true", help="Production release build and signing")
    parser.add_argument("--output-dir", default=str(ROOT / "dist"), help="Output directory (default: dist/)")
    parser.add_argument("--skip-missing", action="store_true",
                        help="Skip targets where cross-compilers or tools are missing instead of aborting")
    args = parser.parse_args()

    if args.check:
        check_prerequisites()
        return

    version = get_version()
    tag = f"v{version}"
    dist_dir = Path(args.output_dir).resolve()
    dist_dir.mkdir(parents=True, exist_ok=True)

    print(f"=== NMessenger Release Builder {tag} ===")
    print(f"Output directory: {dist_dir}\n")

    selected_targets = {}
    if args.target:
        if args.target not in TARGETS:
            sys.exit(f"Unknown target: {args.target}. Available: {list(TARGETS.keys())}")
        selected_targets[args.target] = TARGETS[args.target]
    else:
        for key, info in TARGETS.items():
            if args.platform != "all" and info["platform"] != args.platform:
                continue
            if args.arch != "all" and info["arch"] != args.arch:
                continue
            selected_targets[key] = info

    for key, info in selected_targets.items():
        platform = info["platform"]
        arch = info["arch"]
        rust_target = info.get("rust_target")
        print(f"\n>>> Processing {key} ({platform} / {arch} / {rust_target}) <<<")

        if platform == "android":
            build_android(tag, info, dist_dir, is_production=args.production)
            continue

        binary_name = info["binary_name"]
        binary = None

        # Build binary if not package-only
        if not args.package_only:
            try:
                print(f"Compiling {rust_target}...")
                build_rust_binary(rust_target, release=True)
            except Exception as e:
                print(f"Warning: Build failed for {rust_target}: {e}")
                if "fallback_target" in info:
                    fallback = info["fallback_target"]
                    print(f"Trying fallback target {fallback}...")
                    try:
                        build_rust_binary(fallback, release=True)
                        rust_target = fallback
                    except Exception as e2:
                        print(f"Fallback build also failed: {e2}")
                        if not args.skip_missing:
                            raise
                        continue
                elif not args.skip_missing:
                    raise
                else:
                    continue

        binary = find_binary(rust_target, binary_name, release=True)
        if not binary or not binary.is_file():
            if args.skip_missing:
                print(f"Binary not found for {key} ({rust_target}); skipping packaging.")
                continue
            raise RuntimeError(f"Binary not found for {key} ({rust_target})")

        print(f"Found binary: {binary}")

        if platform == "linux":
            package_linux(tag, info, dist_dir, binary, [args.format])
        elif platform == "windows":
            package_windows(tag, info, dist_dir, binary, [args.format])
        elif platform == "macos":
            package_macos(tag, info, dist_dir, binary, [args.format])

    generate_checksums(dist_dir)
    print("\nAll builds and packaging complete!")


if __name__ == "__main__":
    main()
