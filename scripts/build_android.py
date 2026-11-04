#!/usr/bin/env python3
"""Build and sign an Android APK using the Android SDK and NDK (Linux x86-64)."""
import argparse
import hashlib
import tomllib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parent.parent
ARCHES = {
    "arm64": ("aarch64-linux-android", "arm64-v8a"),
    "x86_64": ("x86_64-linux-android", "x86_64"),
}
MIN_SDK = 26
TARGET_SDK = 35


def run(command, env=None):
    print("+ " + " ".join(str(value) for value in command), flush=True)
    subprocess.run([str(value) for value in command], cwd=ROOT, env=env, check=True)


def newest(directory, required):
    if not directory.is_dir():
        raise RuntimeError(f"Android directory not found: {directory}")
    candidates = [child for child in directory.iterdir() if child.is_dir() and (child / required).is_file()]
    candidates.sort(key=lambda path: tuple(int(part) for part in path.name.replace("android-", "").split(".") if part.isdigit()))
    if not candidates:
        raise RuntimeError(f"Android component not found in {directory}: {required}")
    return candidates[-1]


def resolve_tools(env):
    sdk_candidates = [env.get("ANDROID_HOME"), env.get("ANDROID_SDK_ROOT"), str(Path.home() / "Android/Sdk")]
    sdk = next((Path(path).expanduser().resolve() for path in sdk_candidates if path and (Path(path).expanduser() / "build-tools").is_dir()), None)
    if sdk is None:
        raise RuntimeError("Android SDK not found. Set ANDROID_HOME to an installed SDK with build-tools.")
    ndk_value = env.get("ANDROID_NDK_HOME") or env.get("ANDROID_NDK_ROOT")
    ndk = Path(ndk_value).expanduser().resolve() if ndk_value else newest(sdk / "ndk", "source.properties")
    if not (ndk / "source.properties").is_file():
        raise RuntimeError(f"Android NDK not found at {ndk}; set ANDROID_NDK_HOME to a valid NDK.")
    tools = newest(sdk / "build-tools", "aapt2")
    if not tools.name.split(".", 1)[0].isdigit() or int(tools.name.split(".", 1)[0]) < 35:
        raise RuntimeError("Android SDK Build Tools 35.0.0 or newer are required for 16 KB APK alignment")
    android_jar = sdk / f"platforms/android-{TARGET_SDK}/android.jar"
    if not android_jar.is_file():
        raise RuntimeError(f"Android API {TARGET_SDK} platform not found: {android_jar}")
    return sdk, ndk, tools, android_jar


def validate_tools(ndk, tools, target):
    toolchain = ndk / "toolchains/llvm/prebuilt/linux-x86_64"
    required = [tools / name for name in ("aapt2", "d8", "zipalign", "apksigner")]
    required += [toolchain / "bin" / f"{target}{MIN_SDK}-clang", toolchain / "bin/llvm-ar", toolchain / "bin/llvm-strip"]
    required += [toolchain / f"sysroot/usr/lib/{target}/libc++_shared.so"]
    missing = [str(path) for path in required if not path.is_file() or (path.parent.name == "bin" and not os.access(path, os.X_OK))]
    missing += [name for name in ("cargo", "rustc", "javac", "keytool") if shutil.which(name) is None]
    if missing:
        raise RuntimeError("Missing Android build tools: " + ", ".join(missing))
    rust_env = os.environ.copy()
    rust_env.pop("RUSTUP_FORCE_ARG0", None)
    libdir = subprocess.run(["rustc", "--print", "target-libdir", "--target", target], cwd=ROOT,
                            env=rust_env, check=True, text=True, capture_output=True).stdout.strip()
    if not any(Path(libdir).glob("libstd-*.rlib")):
        raise RuntimeError(f"Rust target {target} is not installed. Run: rustup target add {target}")
    return toolchain


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--debug", action="store_true", help="Build with the Rust dev profile")
    parser.add_argument("--production", action="store_true", help="Sign with the release keystore specified in the environment")
    parser.add_argument("--arch", choices=ARCHES, default="arm64", help="Android architecture (default: arm64)")
    parser.add_argument("--check", action="store_true", help="Check the SDK, NDK and build tools without building")
    args = parser.parse_args()
    if args.production:
        missing = [name for name in ("NM_ANDROID_KEYSTORE", "NM_ANDROID_KEY_ALIAS", "NM_ANDROID_STOREPASS", "NM_ANDROID_KEYPASS") if not os.environ.get(name)]
        if missing:
            raise RuntimeError("Production signing requires: " + ", ".join(missing))
        if not Path(os.environ["NM_ANDROID_KEYSTORE"]).expanduser().is_file():
            raise RuntimeError("NM_ANDROID_KEYSTORE must point to an existing keystore")
    sdk, ndk, tools, android_jar = resolve_tools(os.environ)
    target, abi = ARCHES[args.arch]
    toolchain = validate_tools(ndk, tools, target)
    print(f"SDK: {sdk}\nNDK: {ndk}\nBuild tools: {tools}\nPlatform: {android_jar}\nTarget: {target} ({abi})", flush=True)
    if args.check:
        return

    target_dir = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")).expanduser()
    if not target_dir.is_absolute():
        target_dir = ROOT / target_dir
    target_dir = target_dir.resolve()
    work = target_dir / "android"
    work.mkdir(parents=True, exist_ok=True)
    env = os.environ.copy()
    env.pop("RUSTUP_FORCE_ARG0", None)
    env.update({"ANDROID_HOME": str(sdk), "ANDROID_SDK_ROOT": str(sdk), "ANDROID_NDK_HOME": str(ndk), "ANDROID_NDK_ROOT": str(ndk),
                f"CARGO_TARGET_{target.upper().replace('-', '_')}_LINKER": str(toolchain / f"bin/{target}{MIN_SDK}-clang"),
                f"CC_{target.replace('-', '_')}": str(toolchain / f"bin/{target}{MIN_SDK}-clang"),
                f"CXX_{target.replace('-', '_')}": str(toolchain / f"bin/{target}{MIN_SDK}-clang++"),
                f"AR_{target.replace('-', '_')}": str(toolchain / "bin/llvm-ar"),
                f"CARGO_TARGET_{target.upper().replace('-', '_')}_RUSTFLAGS": "-C link-arg=-Wl,-z,max-page-size=16384"})
    cargo = ["cargo", "build", "--locked", "-p", "nmessenger-client", "--lib", "--target", target]
    if not args.debug:
        cargo.append("--release")
    run(cargo, env)
    native = target_dir / target / ("debug" if args.debug else "release") / "libnmessenger_client.so"
    profile = "debug" if args.debug else "release"
    signing = "production" if args.production else "development"
    output = work / f"nmessenger-{abi}-{profile}-{signing}.apk"
    keystore = Path(os.environ["NM_ANDROID_KEYSTORE"]).expanduser() if args.production else work / "debug.keystore"
    if not args.production and not keystore.exists():
        run(["keytool", "-genkeypair", "-keystore", keystore, "-storepass", "android", "-keypass", "android", "-alias", "androiddebugkey",
             "-dname", "CN=NMessenger Development,O=NMessenger,C=BR", "-keyalg", "RSA", "-keysize", "2048", "-validity", "10000"])
        keystore.chmod(0o600)

    # Fresh intermediate files avoid retaining classes or dex files from earlier builds.
    with tempfile.TemporaryDirectory(prefix="build-", dir=work) as temp:
        temp = Path(temp)
        classes, dex = temp / "classes", temp / "dex"
        classes.mkdir()
        dex.mkdir()
        sources = sorted((ROOT / "android/java").rglob("*.java"))
        if not sources:
            raise RuntimeError("Android Java sources not found in android/java")
        bootclasspath = os.pathsep.join(str(path) for path in (android_jar, tools / "core-lambda-stubs.jar") if path.is_file())
        run(["javac", "-encoding", "UTF-8", "-source", "8", "-target", "8", "-bootclasspath", bootclasspath, "-d", classes, *sources])
        jar = temp / "classes.jar"
        with zipfile.ZipFile(jar, "w") as archive:
            for file in sorted(classes.rglob("*.class")):
                archive.write(file, file.relative_to(classes))
        run([tools / "d8", "--lib", android_jar, "--min-api", str(MIN_SDK), "--output", dex, jar])
        compiled, unsigned, aligned = temp / "resources.zip", temp / "unsigned.apk", temp / "aligned.apk"
        stripped = temp / "libnmessenger_client.so"
        run([toolchain / "bin/llvm-strip", "--strip-debug", "-o", stripped, native])
        run([tools / "aapt2", "compile", "--dir", ROOT / "android/res", "-o", compiled])
        with (ROOT / "Cargo.toml").open("rb") as version_file:
            version = tomllib.load(version_file)["workspace"]["package"]["version"]
        code = os.environ.get("NM_ANDROID_VERSION_CODE", "1")
        if not code.isdecimal() or not 1 <= int(code) <= 2100000000:
            raise RuntimeError("NM_ANDROID_VERSION_CODE must be an integer between 1 and 2100000000")
        if args.production and "NM_ANDROID_VERSION_CODE" not in os.environ:
            raise RuntimeError("Production signing requires a monotonically increasing NM_ANDROID_VERSION_CODE")
        run([tools / "aapt2", "link", "-I", android_jar, "--manifest", ROOT / "android/AndroidManifest.xml", "--replace-version", "--version-code", code, "--version-name", version, "-o", unsigned, compiled])
        with zipfile.ZipFile(unsigned, "a") as archive:
            archive.write(dex / "classes.dex", "classes.dex", compress_type=zipfile.ZIP_DEFLATED)
            archive.write(stripped, f"lib/{abi}/libnmessenger_client.so", compress_type=zipfile.ZIP_STORED)
            archive.write(toolchain / f"sysroot/usr/lib/{target}/libc++_shared.so", f"lib/{abi}/libc++_shared.so", compress_type=zipfile.ZIP_STORED)
        run([tools / "zipalign", "-f", "-P", "16", "4", unsigned, aligned])
        alias = os.environ["NM_ANDROID_KEY_ALIAS"] if args.production else "androiddebugkey"
        storepass = "env:NM_ANDROID_STOREPASS" if args.production else "pass:android"
        keypass = "env:NM_ANDROID_KEYPASS" if args.production else "pass:android"
        signed = temp / "signed.apk"
        run([tools / "apksigner", "sign", "--ks", keystore, "--ks-key-alias", alias, "--ks-pass", storepass, "--key-pass", keypass, "--out", signed, aligned])
        run([tools / "apksigner", "verify", "--verbose", signed])
        run([tools / "zipalign", "-c", "-P", "16", "4", signed])
        signed.replace(output)
    run([tools / "aapt2", "dump", "badging", output])
    record = {"apk": str(output), "abi": abi, "min_sdk": MIN_SDK, "target_sdk": TARGET_SDK,
              "bytes": output.stat().st_size, "sha256": hashlib.sha256(output.read_bytes()).hexdigest(), "signing": signing, "rust_profile": profile}
    (work / f"{output.stem}.json").write_text(json.dumps(record, indent=2) + "\n")
    print(json.dumps(record, indent=2))


if __name__ == "__main__":
    try:
        main()
    except (RuntimeError, subprocess.CalledProcessError) as error:
        print(f"Android build failed: {error}", file=sys.stderr)
        sys.exit(1)
