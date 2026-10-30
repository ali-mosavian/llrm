"""The pin's IR must be built for the cpu llrm-cc builds for.

Without `-march` clang targets pentium4 +sse2 for i486-unknown-linux-gnu: the pin compiled floats with SSE2 against
llrm-cc's x87 (nbody pin B 541 vs 749, matmul 1741 vs 2141).
"""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import cc_cost  # noqa: E402


class PinCommand(unittest.TestCase):
    def test_the_pin_front_end_targets_the_cpu_asked_for(self):
        command = cc_cost.pin_ir_command(Path("x.c"), "-O2", "i486", [])
        self.assertIn("-march=i486", command)
        self.assertNotIn("-march=pentium4", command)


if __name__ == "__main__":
    unittest.main()
