from pathlib import Path
import json
import plistlib
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
VERIFY = ROOT / 'scripts/verify-package.sh'


class PackageValidation(unittest.TestCase):
    def test_invalid_versions_do_not_touch_existing_bundle(self):
        with tempfile.TemporaryDirectory(prefix='rake-package-') as directory:
            root = Path(directory)
            (root / 'scripts').mkdir()
            shutil.copy2(ROOT / 'scripts/package.sh', root / 'scripts/package.sh')
            (root / 'Cargo.toml').write_text('[package]\nversion = "0.1.0"\n')
            (root / 'dist/Regie.app').mkdir(parents=True)
            sentinel = root / 'dist/Regie.app/keep'
            sentinel.write_text('previous preview')
            for version in ['../escape', 'v26.10.0', '26.13.0', '26.10.01', '1.2', '26.10.0\nextra']:
                result = subprocess.run(['sh', str(root / 'scripts/package.sh'), version],
                                        capture_output=True, text=True)
                self.assertNotEqual(result.returncode, 0, version)
                self.assertIn('invalid version', result.stderr, result.stderr)
                self.assertEqual(sentinel.read_text(), 'previous preview')

    @unittest.skipUnless(sys.platform == 'darwin', 'corrupt DMG inspection requires macOS')
    def test_corrupt_image_is_rejected(self):
        with tempfile.TemporaryDirectory(prefix='rake-corrupt-') as directory:
            image = Path(directory) / 'bad.dmg'
            image.write_bytes(b'not a disk image')
            result = subprocess.run(['sh', str(VERIFY), str(image), '0.1.0'],
                                    capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertNotIn("No such file", result.stderr)


@unittest.skipUnless(sys.platform == 'darwin', 'native DMG checks require macOS')
class NativePackage(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        binary = ROOT / 'target/release/regie'
        if not binary.is_file():
            raise RuntimeError('run cargo build --release --locked before native package tests')
        cls.temp = tempfile.TemporaryDirectory(prefix='rake-native-package-')
        cls.addClassCleanup(cls.temp.cleanup)
        cls.root = Path(cls.temp.name)
        for relative in ['scripts/package.sh', 'assets/icons/Regie.icns', 'Cargo.toml']:
            destination = cls.root / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT / relative, destination)
        (cls.root / 'target/release').mkdir(parents=True)
        shutil.copy2(binary, cls.root / 'target/release/regie')
        result = subprocess.run(['sh', str(cls.root / 'scripts/package.sh'), '0.1.0'],
                                cwd=cls.root, check=True, capture_output=True, text=True)
        cls.image = cls.root / result.stdout.strip()

    def verify(self, version):
        return subprocess.run(['sh', str(VERIFY), str(self.image), version],
                              capture_output=True, text=True)

    def test_real_image_is_verified_and_checksum_reported(self):
        result = self.verify('0.1.0')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertRegex(result.stdout, r'^[0-9a-f]{64}  ')

    def test_wrong_expected_version_is_rejected(self):
        result = self.verify('26.10.1')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('version', result.stderr)

    def test_cargo_product_is_regie(self):
        result = subprocess.run(['cargo', 'metadata', '--locked', '--no-deps', '--format-version', '1'],
                                cwd=ROOT, check=True, capture_output=True, text=True)
        package = json.loads(result.stdout)['packages'][0]
        self.assertEqual(package['name'], 'regie')
        self.assertEqual([t['name'] for t in package['targets'] if 'bin' in t['kind']], ['regie'])

    def test_mounted_package_has_independent_regie_identity(self):
        with tempfile.TemporaryDirectory(prefix='regie-inspect-') as directory:
            mount = Path(directory) / 'mount'
            mount.mkdir()
            subprocess.run(['hdiutil', 'attach', '-quiet', '-readonly', '-nobrowse',
                            '-mountpoint', str(mount), str(self.image)], check=True)
            try:
                app = mount / 'Regie.app'
                self.assertTrue((app / 'Contents/MacOS/regie').is_file())
                self.assertFalse((mount / 'Rake.app').exists())
                with (app / 'Contents/Info.plist').open('rb') as stream:
                    info = plistlib.load(stream)
                for key, value in {'CFBundleIdentifier': 'com.jonathantobe.regie',
                                   'CFBundleExecutable': 'regie', 'CFBundleIconFile': 'Regie',
                                   'CFBundleName': 'Regie', 'CFBundleVersion': '0.1.0',
                                   'CFBundleShortVersionString': '0.1.0'}.items():
                    self.assertEqual(info[key], value, key)
                self.assertEqual((mount / 'Applications').readlink(), Path('/Applications'))
                self.assertTrue((app / 'Contents/Resources/Regie.icns').is_file())
            finally:
                subprocess.run(['hdiutil', 'detach', '-quiet', str(mount)], check=True)


if __name__ == '__main__':
    unittest.main()
