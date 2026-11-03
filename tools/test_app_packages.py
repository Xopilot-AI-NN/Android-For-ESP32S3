"""Check package ownership, complete catalogs and bootloader dependency boundaries."""
import json
from pathlib import Path
import shutil
import tempfile
import unittest
from unittest.mock import patch

import build_firmware as builder


class AppPackagesTests(unittest.TestCase):
    """Exercise the same package validation used for building disk images."""

    def test_all_launcher_apps_have_real_sources(self):
        catalog = builder.validate_app_packages()
        self.assertEqual(sum(entry.get("launcher", False) for entry in catalog), 19)
        self.assertEqual(len(catalog), 22)
        self.assertFalse((builder.ROOT / "system/app").exists())
        self.assertFalse((builder.ROOT / "system/priv-app").exists())

    def test_missing_controller_prevents_image_build(self):
        with tempfile.TemporaryDirectory() as temp:
            app = Path(temp) / "App"
            shutil.copytree(builder.APP_ROOT, app, ignore=shutil.ignore_patterns(".git"))
            (app / "system/app/Files/main.rs").unlink()
            with patch.object(builder, "APP_ROOT", app):
                with self.assertRaisesRegex(ValueError, "Unlisted application sources"):
                    builder.validate_app_packages()

    def test_new_app_cannot_silently_disappear_from_native_menu(self):
        with tempfile.TemporaryDirectory() as temp:
            app = Path(temp) / "App"
            shutil.copytree(builder.APP_ROOT, app, ignore=shutil.ignore_patterns(".git"))
            source = app / "src/catalog.rs"
            source.write_text(source.read_text().replace("0, 1, 2, 6", "1, 0, 2, 6"))
            with patch.object(builder, "APP_ROOT", app):
                with self.assertRaisesRegex(ValueError, "Native launcher differs"):
                    builder.validate_app_packages()

    def test_bootloader_has_no_application_or_firmware_dependency(self):
        loader = builder.ROOT.parent / "Bootloader"
        for source in (loader / "src").rglob("*.rs"):
            text = source.read_text()
            self.assertNotIn("App/", text, source)
            self.assertNotIn("Firmware/", text, source)
            self.assertNotIn("rhai::", text, source)
            self.assertNotIn("crate::material", text, source)
        manifest = (loader / "Cargo.toml").read_text()
        self.assertNotIn('path = "../Firmware', manifest)
        self.assertNotIn('path = "../App', manifest)


if __name__ == "__main__":
    unittest.main()
