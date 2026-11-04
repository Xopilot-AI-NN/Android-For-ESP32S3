import unittest
from check_main_stack import frame_size, remaining


class StackTests(unittest.TestCase):
    def test_small_frame(self):
        self.assertEqual(frame_size('entry a1, 0x4c0\nmov.n a10, a2'), 1216)

    def test_extended_frame(self):
        self.assertEqual(frame_size('entry a1, 32\nl32r a8, 42000000 (17bb0 <literal>)\nsub a8, a1, a8\nmovsp a1, a8'), 97232)

    def test_reject_failing_release_budget(self):
        with self.assertRaisesRegex(ValueError, 'headroom'):
            remaining(123452, [97232, 22032, 4736])
        self.assertEqual(remaining(156220, [97232, 1216, 4736]), 53036)

    def test_unknown_prologue_is_rejected(self):
        with self.assertRaises(ValueError):
            frame_size('unknown')


if __name__ == '__main__':
    unittest.main()
