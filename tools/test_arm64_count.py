"""tools/arm64/count.py never picks the counted range itself."""

import os
import subprocess
import sys
import unittest

COUNT = os.path.join(os.path.dirname(__file__), "arm64", "count.py")


class CountRange(unittest.TestCase):
    def test_no_range_flag_is_an_error_not_a_whole_program_count(self):
        # Without a flag the count ran over the whole program and was read as the kernel's.
        done = subprocess.run([sys.executable, COUNT, "x.elf"], capture_output=True, text=True)
        self.assertEqual(done.returncode, 2, done.stdout + done.stderr)
        self.assertIn("--kernel", done.stderr)
        self.assertIn("--whole", done.stderr)

    def test_both_flags_are_an_error(self):
        done = subprocess.run([sys.executable, COUNT, "--kernel", "--whole", "x.elf"], capture_output=True, text=True)
        self.assertEqual(done.returncode, 2, done.stderr)


if __name__ == "__main__":
    unittest.main()
