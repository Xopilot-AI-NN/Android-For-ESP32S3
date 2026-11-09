"""Exercise updates on a real GPT without writing a card or running watch."""
import importlib.util
import tempfile
from pathlib import Path
import unittest
import uuid

from partition_tool import flash_partition, partitions

spec = importlib.util.spec_from_file_location(
    "builder", Path(__file__).resolve().parents[2] / "Firmware/tools/build_firmware.py")
builder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(builder)


class PartitionUpdateTests(unittest.TestCase):
    """Firmware writes must preserve GPT, neighbors and protected userdata."""
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.disk = tempfile.TemporaryFile()
        self.addCleanup(self.disk.close)
        sectors = 8192
        self.disk.truncate(sectors * 512)
        primary, entries, backup, _ = builder.gpt_headers(
            sectors, [("boot_a", 2048, 3071), ("userdata", 4096, sectors - 34)], uuid.uuid4())
        for offset, data in [(0, builder.protective_mbr(sectors)), (512, primary),
                             (1024, entries), ((sectors - 33) * 512, entries),
                             ((sectors - 1) * 512, backup)]:
            self.disk.seek(offset)
            self.disk.write(data)
        self.payload = Path(self.temp.name) / "boot.img"
        self.payload.write_bytes(b"ANDROID!" + b"firmware" * 200)

    def test_write_and_verify_leave_gpt_tail_and_userdata_intact(self):
        before = partitions(self.disk)
        self.disk.seek(0)
        headers = self.disk.read(34 * 512)
        self.disk.seek(4096 * 512)
        self.disk.write(b"user documents")
        self.disk.seek(2048 * 512 + self.payload.stat().st_size)
        self.disk.write(b"tail")
        digest = flash_partition(self.disk, "boot_a", self.payload)
        self.assertEqual(len(digest), 64)
        self.assertEqual(partitions(self.disk), before)
        self.disk.seek(0)
        self.assertEqual(self.disk.read(34 * 512), headers)
        self.disk.seek(2048 * 512)
        self.assertEqual(self.disk.read(self.payload.stat().st_size), self.payload.read_bytes())
        self.assertEqual(self.disk.read(4), b"tail")
        self.disk.seek(4096 * 512)
        self.assertEqual(self.disk.read(14), b"user documents")

    def test_userdata_and_oversized_payload_are_rejected_before_writing(self):
        with self.assertRaisesRegex(ValueError, "protected"):
            flash_partition(self.disk, "userdata", self.payload)
        self.payload.write_bytes(b"X" * (1024 * 512 + 1))
        with self.assertRaisesRegex(ValueError, "exceeds"):
            flash_partition(self.disk, "boot_a", self.payload)
        self.disk.seek(2048 * 512)
        self.assertEqual(self.disk.read(16), bytes(16))

    def test_bad_gpt_blocks_partition_write(self):
        self.disk.seek(512)
        self.disk.write(b"BROKEN!!")
        with self.assertRaises(AssertionError):
            flash_partition(self.disk, "boot_a", self.payload)
        self.disk.seek(2048 * 512)
        self.assertEqual(self.disk.read(16), bytes(16))


if __name__ == "__main__":
    unittest.main()
