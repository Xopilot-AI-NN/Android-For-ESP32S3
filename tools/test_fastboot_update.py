"""Verify firmware update routing without opening serial or writing a board."""
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import fastboot_tool


class FastbootUpdateTests(unittest.TestCase):
    def test_native_update_checks_service_then_uses_espflash_with_explicit_layout(self):
        with tempfile.TemporaryDirectory() as temp:
            elf = Path(temp) / "firmware"
            elf.write_bytes(b"\x7fELFtest fixture")
            with patch("sys.argv", ["fastboot_tool.py", "--port", "/dev/test", "update", str(elf)]), \
                 patch.object(fastboot_tool, "transact", return_value=0) as probe, \
                 patch.object(fastboot_tool.shutil, "which", return_value="/mock/espflash"), \
                 patch.object(fastboot_tool.subprocess, "run", return_value=SimpleNamespace(returncode=0)) as flash:
                self.assertEqual(fastboot_tool.main(), 0)
                probe.assert_called_once_with("/dev/test", "getvar:version", 115200, 3.0)
                command = flash.call_args.args[0]
                self.assertEqual(command[:3], ["/mock/espflash", "flash", "--chip"])
                self.assertIn(str(elf), command)
                self.assertIn(str(Path(fastboot_tool.__file__).with_name("partitions.csv")), command)

    def test_disk_image_cannot_be_flashed_as_native_firmware(self):
        with tempfile.TemporaryDirectory() as temp:
            image = Path(temp) / "disk.img"
            image.write_bytes(b"ANDROID!not an ELF")
            with patch("sys.argv", ["fastboot_tool.py", "--port", "/dev/test", "update", str(image)]), \
                 patch.object(fastboot_tool, "transact") as probe, \
                 patch.object(fastboot_tool.subprocess, "run") as flash, \
                 patch("sys.stderr"):
                with self.assertRaises(SystemExit):
                    fastboot_tool.main()
                probe.assert_not_called()
                flash.assert_not_called()


if __name__ == "__main__":
    unittest.main()
