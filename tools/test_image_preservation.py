"""Regression checks for retaining watch preferences during firmware rebuilds."""
import struct
import tempfile
import unittest
from pathlib import Path

import build_firmware as build


class ImagePreservation(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.image = Path(self.temp.name) / "watch.img"
        self.blobs = {name: b"old-system" for name in
                      ("boot.img", "init_boot.img", "vendor_boot.img", "vbmeta.img", "super.img")}
        build.write_sd_image(self.image, 256, self.blobs, "test")
        with self.image.open("rb") as disk:
            disk.seek(2 * build.SECTOR)
            entries = disk.read(build.GPT_ENTRIES * build.GPT_ENTRY_SIZE)
        self.parts = {}
        for offset in range(0, len(entries), build.GPT_ENTRY_SIZE):
            entry = entries[offset:offset + build.GPT_ENTRY_SIZE]
            if entry[:16] != bytes(16):
                name = entry[56:128].decode("utf-16-le").rstrip("\0")
                self.parts[name] = struct.unpack_from("<QQ", entry, 32)
        self.markers = {}
        with self.image.open("r+b") as disk:
            for name in ("metadata", "swap", "userdata"):
                first, last = self.parts[name]
                for offset in (first * build.SECTOR + 4096, (last + 1) * build.SECTOR - 64):
                    data = f"saved-{name}-{offset}".encode()
                    disk.seek(offset)
                    disk.write(data)
                    self.markers[offset] = data

    def assert_saved(self):
        with self.image.open("rb") as disk:
            for offset, data in self.markers.items():
                disk.seek(offset)
                self.assertEqual(disk.read(len(data)), data)

    def test_rebuild_updates_system_and_retains_persistent_ranges(self):
        newer = {name: b"new-system" for name in self.blobs}
        build.write_sd_image(self.image, 256, newer, "updated")
        self.assert_saved()
        with self.image.open("rb") as disk:
            disk.seek(self.parts["super"][0] * build.SECTOR)
            self.assertEqual(disk.read(10), b"new-system")

    def test_size_change_and_failed_build_leave_original_intact(self):
        with self.assertRaises(RuntimeError):
            build.write_sd_image(self.image, 512, self.blobs, "test")
        self.assert_saved()
        oversized = dict(self.blobs, **{"boot.img": bytes(9 * build.MIB)})
        with self.assertRaises(RuntimeError):
            build.write_sd_image(self.image, 256, oversized, "test")
        self.assert_saved()
        self.assertFalse(list(self.image.parent.glob(".watch.img.*")))

    @unittest.skipIf(build.fcntl is None, "POSIX image locks")
    def test_active_image_owner_prevents_rebuild(self):
        lock_path = self.image.with_suffix(".img.lock")
        with lock_path.open("a+b") as lock:
            build.fcntl.flock(lock, build.fcntl.LOCK_EX | build.fcntl.LOCK_NB)
            with self.assertRaisesRegex(RuntimeError, "image is in use"):
                build.write_sd_image(self.image, 256, self.blobs, "test")
        self.assert_saved()


if __name__ == "__main__":
    unittest.main()
