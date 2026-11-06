"""mir-oracle.sh with no .ll files checked nothing and exited 0; it must refuse (and before the cargo build)."""

import os
import subprocess
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SCRIPT = os.path.join(ROOT, "tools", "mir-oracle.sh")


class MirOracleArgs(unittest.TestCase):
    def test_no_files_is_refused(self):
        env = dict(os.environ, LLVM20="/nonexistent")  # a missing LLVM must not be what makes it fail
        r = subprocess.run(["bash", SCRIPT], cwd=ROOT, env=env, capture_output=True, text=True)
        self.assertEqual(r.returncode, 2, r.stdout + r.stderr)
        self.assertIn("usage", r.stderr)


if __name__ == "__main__":
    unittest.main()
