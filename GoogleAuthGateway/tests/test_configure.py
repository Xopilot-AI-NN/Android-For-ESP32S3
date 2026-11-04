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

    def test_desktop_local_import_creates_ip_certificate_and_preserves_ca(self):
        import subprocess
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            oauth = root / "desktop.json"
            oauth.write_text(json.dumps({"installed": {
                "client_id": "synthetic.invalid", "client_secret": "fixture-only"}}))
            arguments = [str(SOURCE), str(oauth), "--local-ip", "192.168.1.22"]
            with patch.dict(os.environ, {"XDG_CONFIG_HOME": str(root / "config")}), patch.object(sys, "argv", arguments):
                MODULE.main()
                private = root / "config/aosp-wear-gateway"
                ca = (private / "local-ca.pem").read_bytes()
                subprocess.run(["openssl", "verify", "-x509_strict", "-CAfile", str(private / "local-ca.pem"),
                                "-verify_ip", "192.168.1.22", str(private / "local-server.pem")], check=True, capture_output=True)
                self.assertIn('OAUTH_REDIRECT_URL="http://localhost:8080/oauth/callback"', (private / "config.env").read_text())
                MODULE.main()
                self.assertEqual((private / "local-ca.pem").read_bytes(), ca)
                self.assertEqual(stat.S_IMODE((private / "local-server.key").stat().st_mode), 0o600)

    def test_legacy_ca_repair_preserves_key_and_passes_strict_validation(self):
        import subprocess
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            ca, key = root / "local-ca.pem", root / "local-ca.key"
            subprocess.run(["openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1",
                            "-subj", "/CN=AOSP Wear Local CA", "-addext", "basicConstraints=critical,CA:TRUE",
                            "-keyout", str(key), "-out", str(ca)], check=True, capture_output=True)
            before = key.read_bytes()
            MODULE.create_local_certificates(root, "192.168.1.22")
            self.assertEqual(key.read_bytes(), before)
            subprocess.run(["openssl", "verify", "-x509_strict", "-CAfile", str(ca),
                            "-verify_ip", "192.168.1.22", str(root / "local-server.pem")], check=True, capture_output=True)

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
