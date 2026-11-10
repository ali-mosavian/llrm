"""Two-sided drivers refuse two sides that run one llrm-llc (z80 identity #8527 compared one llc with itself)."""

import os
import subprocess
import tempfile
import unittest

TOOLS = os.path.dirname(os.path.abspath(__file__))


class SideIdentity(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.d = self.dir.name
        self.src = os.path.join(self.d, "a.c")
        open(self.src, "w").write("int main(void){return 0;}\n")
        for side in ("base", "new"):
            os.makedirs(os.path.join(self.d, side))
            self.tool(side, "llrm-cc", f"echo {side} > \"$3\"\n")
            self.tool(side, "llrm-llc", "# one llc\n")

    def tool(self, side, name, body):
        path = os.path.join(self.d, side, name)
        open(path, "w").write("#!/bin/bash\n" + body)
        os.chmod(path, 0o755)

    def run_driver(self, *extra, env=None):
        return subprocess.run(
            [os.path.join(TOOLS, "identity_targets.sh"), *extra, os.path.join(self.d, "base", "llrm-cc"), os.path.join(self.d, "new", "llrm-cc"), "z80-sdcc", "O2", self.src],
            capture_output=True, text=True, env={**os.environ, **(env or {})}, cwd=self.d,
        )

    def test_one_llc_under_two_ccs_is_refused_before_any_compile(self):
        """Cost: every row read SAME because both ccs ran one backend; now exit 2, nothing compiled."""
        done = self.run_driver()
        self.assertEqual(done.returncode, 2, done.stdout + done.stderr)
        self.assertIn("same llrm-llc", done.stderr)
        self.assertNotIn("same=", done.stdout)

    def test_each_side_prints_its_cc_and_llc_md5(self):
        done = self.run_driver("--same-ok")
        self.assertIn("side base:", done.stderr)
        self.assertIn("side head:", done.stderr)
        self.assertEqual(done.stderr.count("md5"), 4, done.stderr)

    def test_distinct_llcs_pass(self):
        self.tool("new", "llrm-llc", "# another llc\n")
        done = self.run_driver()
        self.assertNotIn("same llrm-llc", done.stderr)
        self.assertIn("same=0 diff=1", done.stdout)  # reached the compile step


if __name__ == "__main__":
    unittest.main()
