"""Test private import with synthetic values; never read the operator's config."""
import importlib.util
import json
import os
from pathlib import Path
import stat
import sys
import tempfile
import unittest
from unittest.mock import patch

SOURCE = Path(__file__).resolve().parents[1] / "configure.py"
SPEC = importlib.util.spec_from_file_location("gateway_configure", SOURCE)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class ConfigureTests(unittest.TestCase):
    def test_import_preserves_key_and_uses_private_modes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            oauth = root / "input.json"
            oauth.write_text(json.dumps({"web": {
                "client_id": "synthetic.invalid", "client_secret": "fixture-only",
                "redirect_uris": ["http://localhost:8090/oauth/callback"],
            }}))
            arguments = [str(SOURCE), str(oauth), "--base-url", "http://localhost:8090"]
            with patch.dict(os.environ, {"XDG_CONFIG_HOME": str(root / "config")}), patch.object(sys, "argv", arguments):
                MODULE.main()
                target = root / "config/aosp-wear-gateway/config.env"
                first = target.read_text()
                MODULE.main()
                self.assertEqual(target.read_text(), first)
                self.assertIn('PORT="8090"', first)
                if os.name != "nt":
                    self.assertEqual(stat.S_IMODE(target.stat().st_mode), 0o600)
                    self.assertEqual(stat.S_IMODE(target.parent.stat().st_mode), 0o700)

    def test_rejects_import_inside_checkout(self):
        with tempfile.TemporaryDirectory() as directory:
            oauth = Path(directory) / "input.json"
            oauth.write_text(json.dumps({"web": {
                "client_id": "synthetic.invalid", "client_secret": "fixture-only",
                "redirect_uris": ["http://localhost:8080/oauth/callback"],
            }}))
            with patch.dict(os.environ, {"XDG_CONFIG_HOME": str(SOURCE.parent / "private-config")}), patch.object(sys, "argv", [str(SOURCE), str(oauth)]):
                with self.assertRaises(SystemExit):
                    MODULE.main()
            self.assertFalse((SOURCE.parent / "private-config").exists())


if __name__ == "__main__":
    unittest.main()
