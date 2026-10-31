from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
NEXT = ROOT / '.github/scripts/next-version.sh'
STAMP = ROOT / '.github/scripts/stamp-version.py'


class Versions(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='rake-version-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.git('init', '-q')
        self.git('-c', 'user.name=Test', '-c', 'user.email=test@example.invalid',
                 '-c', 'commit.gpgsign=false', 'commit', '--allow-empty', '-qm', 'fixture')

    def git(self, *args):
        return subprocess.run(['git', *args], cwd=self.root, check=True, capture_output=True, text=True)

    def next(self, period):
        return subprocess.run([str(NEXT), period], cwd=self.root, capture_output=True, text=True)

    def test_first_release_and_gaps(self):
        self.assertEqual(self.next('2026-10').stdout.strip(), 'v26.10.0')
        for tag in ['v26.10.0', 'v26.10.2']:
            self.git('-c', 'tag.gpgsign=false', 'tag', tag)
        self.assertEqual(self.next('2026-10').stdout.strip(), 'v26.10.3')

    def test_unrelated_and_noncanonical_tags_are_ignored(self):
        for tag in ['v25.10.99', 'v26.11.9', 'v26.10.beta', 'v26.10.01', 'v26.10.3-rc1']:
            self.git('-c', 'tag.gpgsign=false', 'tag', tag)
        self.assertEqual(self.next('2026-10').stdout.strip(), 'v26.10.0')

    def test_month_and_year_rollover(self):
        self.git('-c', 'tag.gpgsign=false', 'tag', 'v26.10.7')
        self.git('-c', 'tag.gpgsign=false', 'tag', 'v26.12.4')
        self.assertEqual(self.next('2026-11').stdout.strip(), 'v26.11.0')
        self.assertEqual(self.next('2027-01').stdout.strip(), 'v27.1.0')

    def test_invalid_periods_fail(self):
        for period in ['2026-00', '2026-13', '2026-1', 'x026-10', '2026-10-extra']:
            self.assertNotEqual(self.next(period).returncode, 0, period)

    def fixture(self):
        (self.root / 'Cargo.toml').write_text('[package]\nname = "regie"\nversion = "0.1.0"\n[dependencies.fake]\nversion = "9.9.9"\n')
        (self.root / 'Cargo.lock').write_text('version = 4\n[[package]]\nname = "regie"\nversion = "0.1.0"\n[[package]]\nname = "fake"\nversion = "9.9.9"\n')

    def stamp(self, tag):
        return subprocess.run(['python3', str(STAMP), tag], cwd=self.root, capture_output=True, text=True)

    def test_stamp_updates_only_root_package_and_lock(self):
        self.fixture()
        result = self.stamp('v26.10.2')
        self.assertEqual(result.returncode, 0, result.stderr)
        for name in ['Cargo.toml', 'Cargo.lock']:
            text = (self.root / name).read_text()
            self.assertEqual(text.count('version = "26.10.2"'), 1)
            self.assertIn('version = "9.9.9"', text)

    def test_invalid_stamp_does_not_mutate_either_file(self):
        for tag in ['26.10.0', 'v26.13.0', 'v26.10.01', 'v2026.10.0']:
            self.fixture()
            before = {name: (self.root / name).read_bytes() for name in ['Cargo.toml', 'Cargo.lock']}
            self.assertNotEqual(self.stamp(tag).returncode, 0)
            for name, data in before.items():
                self.assertEqual((self.root / name).read_bytes(), data)
        self.fixture()
        (self.root / 'Cargo.lock').write_text('invalid lock')
        before = (self.root / 'Cargo.toml').read_bytes()
        self.assertNotEqual(self.stamp('v26.10.0').returncode, 0)
        self.assertEqual((self.root / 'Cargo.toml').read_bytes(), before)


if __name__ == '__main__':
    unittest.main()
