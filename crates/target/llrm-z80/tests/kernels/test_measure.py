import os
import shutil
import subprocess
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))


@unittest.skipUnless(shutil.which("sdasz80"), "sdasz80 not found")
class Measure(unittest.TestCase):
    def test_a_compiler_that_prints_no_assembly_is_refused_not_linked(self):
        """--z80c given llc (which writes -o, not stdout) linked an empty driver: `Undefined Global '_main'`."""
        out = subprocess.run(
            [sys.executable, "-I", f"{HERE}/measure.py", "--z80c", "/bin/true", "--ir-dir", f"{HERE}/ir",
             "--baseline", f"{HERE}/sdcc.md", "--kernels", "zero"],
            capture_output=True, text=True,
        ).stdout
        row = next(line for line in out.splitlines() if line.startswith("| zero |"))
        self.assertIn("refused: no assembly on stdout", row)


if __name__ == "__main__":
    unittest.main()
