"""The harness drivers' zero-page log pointers against the registers, the runtime scratch and the static cells ($80..$FF)."""
import re, unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent


class HarnessDriver(unittest.TestCase):
    def test_the_log_pointers_stay_below_the_registers(self):
        """lp = $A0 sat on the runtime scratch `__st`: a*b of two longs read -1 and 9 cells went WRONG in the 2026-11-18 A/B."""
        for name in ("ours.py", "ours_cr.py", "ours_keep.py", "ours.multi.py"):
            text = (HERE / name).read_text()
            for var in ("lp", "ptr"):
                at = int(re.search(rf"\\n{var} = \$([0-9A-Fa-f]+)\\n", text).group(1), 16)
                self.assertLess(at + 1, 0x80, f"{name}: {var} at ${at:X}")


if __name__ == "__main__":
    unittest.main()
