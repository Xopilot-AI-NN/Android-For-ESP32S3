"""Synthetic secrets verify the guard without embedding real credentials in tests."""
import unittest
from check_public_files import inspect


class CredentialGuardTests(unittest.TestCase):
    def test_private_names_are_blocked_even_when_git_was_forced(self):
        for name in (".env", "client_secret_fixture.json", "private/credentials.json", "device.key"):
            self.assertIn("private runtime filename", inspect(name, b""))

    def test_example_filename_does_not_exempt_real_secret_content(self):
        synthetic = ("GO" + "CSPX-" + "x" * 32).encode()
        self.assertIn("Google OAuth secret", inspect(".env.example", synthetic))
        self.assertNotIn(synthetic.decode(), " ".join(inspect("source.rs", synthetic)))

    def test_clean_example_is_publishable(self):
        self.assertEqual(inspect(".env.example", b"GOOGLE_CLIENT_SECRET=\nSTORE_KEY=\n"), [])
    def test_private_json_is_blocked_even_after_renaming(self):
        self.assertIn('private device/provider credentials',inspect('renamed.json',b'{"device_credential":"synthetic-device-only"}'))
        self.assertEqual(inspect('example.json',b'{"brave_search_api_key":""}'),[])

    def test_large_debug_files_are_not_exempt(self):
        synthetic = ("GO" + "CSPX-" + "x" * 32).encode()
        self.assertIn("Google OAuth secret", inspect("large.bin", b"x" * (2*1024*1024) + synthetic))
        self.assertIn("private disk/debug output", inspect("userdata.img", b""))


if __name__ == "__main__":
    unittest.main()
