"""ours_cr.py's runtime reachability reads code, not comments, as the Rust port does: a comment in
`__mulqi3` that named `__mulhi3` linked its 39 bytes into every program that multiplies bytes
(`huffman_decode` +39 B in an A/B of the runtime, read as the cost of the change; 2026-11-09)."""
import os
import subprocess
import sys
import tempfile
import unittest

RUNTIME = """.export __one
__one:                       ; as __two does
  rts
.export __two
__two:
  rts
"""


class Reach(unittest.TestCase):
    def test_a_name_in_a_comment_links_nothing(self):
        with tempfile.NamedTemporaryFile("w", suffix=".s", delete=False) as f:
            f.write(RUNTIME)
        code = "import ours_cr; print(ours_cr.reachable_runtime('  jsr __one\\n'))"
        env = dict(os.environ, OURS_RT=f.name)
        out = subprocess.run([sys.executable, "-c", code], cwd=os.path.dirname(os.path.abspath(__file__)), env=env,
                             capture_output=True, text=True, check=True).stdout
        os.unlink(f.name)
        self.assertIn("__one:", out)
        self.assertNotIn("__two:", out)


if __name__ == "__main__":
    unittest.main()
