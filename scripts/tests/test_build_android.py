import importlib.util
from pathlib import Path
import tempfile
import unittest


spec = importlib.util.spec_from_file_location("build_android", Path(__file__).resolve().parents[1] / "build_android.py")
build_android = importlib.util.module_from_spec(spec)
spec.loader.exec_module(build_android)


class AndroidToolsTest(unittest.TestCase):
    def test_explicit_ndk_does_not_require_sdk_ndk_directory(self):
        with tempfile.TemporaryDirectory() as root:
            root = Path(root)
            sdk = root / "sdk"
            ndk = root / "external-ndk"
            (sdk / "build-tools/35.0.0").mkdir(parents=True)
            (sdk / "build-tools/35.0.0/aapt2").touch()
            (sdk / "platforms/android-35").mkdir(parents=True)
            (sdk / "platforms/android-35/android.jar").touch()
            ndk.mkdir()
            (ndk / "source.properties").touch()
            found_sdk, found_ndk, tools, platform = build_android.resolve_tools({
                "ANDROID_HOME": str(sdk), "ANDROID_NDK_HOME": str(ndk),
            })
            self.assertEqual(found_sdk, sdk)
            self.assertEqual(found_ndk, ndk)
            self.assertEqual(tools.name, "35.0.0")
            self.assertEqual(platform.name, "android.jar")

    def test_android_35_platform_is_required(self):
        with tempfile.TemporaryDirectory() as root:
            sdk = Path(root)
            (sdk / "build-tools/35.0.0").mkdir(parents=True)
            (sdk / "build-tools/35.0.0/aapt2").touch()
            (sdk / "ndk/29.0").mkdir(parents=True)
            (sdk / "ndk/29.0/source.properties").touch()
            with self.assertRaisesRegex(RuntimeError, "API 35"):
                build_android.resolve_tools({"ANDROID_HOME": str(sdk)})


if __name__ == "__main__":
    unittest.main()
