import struct
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

def png_info(path):
    data = path.read_bytes()
    if data[:8] != b'\x89PNG\r\n\x1a\n':
        raise ValueError('not PNG')
    width, height, depth, color = struct.unpack('>IIBB', data[16:26])
    return width, height, depth, color

class Icons(unittest.TestCase):
    def test_masters_have_real_dimensions_and_alpha(self):
        for file, size in [('menubar-22.png',22), ('menubar-44.png',44), ('app-1024.png',1024)]:
            self.assertEqual(png_info(ROOT / 'assets/icons' / file), (size,size,8,6), file)

    @unittest.skipUnless(sys.platform == 'darwin', 'native iconutil requires macOS')
    def test_icns_contains_all_native_representations(self):
        with tempfile.TemporaryDirectory(prefix='regie-icons-') as directory:
            out = Path(directory) / 'Regie.iconset'
            subprocess.run(['iconutil','-c','iconset',str(ROOT / 'assets/icons/Regie.icns'),'-o',str(out)], check=True)
            for base in [16,32,128,256,512]:
                for scale, suffix in [(1,''),(2,'@2x')]:
                    self.assertEqual(png_info(out / f'icon_{base}x{base}{suffix}.png')[:2], (base*scale,base*scale))
