"""Exercise asset writes against a real CRC-protected GPT, without hardware."""
import importlib.util
from pathlib import Path
import tempfile
import unittest
import uuid

from watch_assets import import_wallpaper, userdata_range, LENGTH

spec = importlib.util.spec_from_file_location(
    'builder', Path(__file__).resolve().parents[2] / 'Firmware/tools/build_firmware.py')
builder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(builder)


class AssetTests(unittest.TestCase):
    def test_write_stays_inside_reserved_range_and_preserves_other_data(self):
        with tempfile.TemporaryFile() as disk:
            sectors = 8192
            disk.truncate(sectors * 512)
            primary, entries, backup, backup_entries = builder.gpt_headers(
                sectors, [('userdata', 2048, sectors - 34)], uuid.uuid4())
            for offset, data in [(0, builder.protective_mbr(sectors)),
                                 (512, primary), (1024, entries),
                                 ((sectors - 33) * 512, entries),
                                 ((sectors - 1) * 512, backup)]:
                disk.seek(offset)
                disk.write(data)
            base, _ = userdata_range(disk)
            disk.seek(base + 200000)
            disk.write(b'keep')
            import_wallpaper(disk, bytes([0x12, 0x34]) * (LENGTH // 2))
            disk.seek(base)
            self.assertEqual(disk.read(8), b'WALLv1\0\0')
            disk.seek(base + 200000)
            self.assertEqual(disk.read(4), b'keep')
            disk.seek(base)
            disk.write(b'FOREIGN!')
            with self.assertRaisesRegex(ValueError, 'refusing'):
                import_wallpaper(disk, bytes(LENGTH))


if __name__ == '__main__':
    unittest.main()
