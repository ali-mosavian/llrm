import subprocess
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import check  # noqa: E402

KERNEL = check.REPO / "crates/target/llrm-x86-m32/vsgcc/kernels/x_dct/x_dct.c"


class CheckTests(unittest.TestCase):
    def test_the_spillers_records_are_read_by_name(self):
        text = "junk\nSPILL-COUNTS _f stores=16 reloads=5 folded=18\n"
        self.assertEqual(check.spill_counts(text), {"stores": 16, "reloads": 5, "folded": 18})

    @unittest.skipUnless(check.LLC.exists() and Path(check.llrm_c()).exists(), "needs the pin's llc and a built llrm-c")
    def test_both_sides_are_split_in_the_same_three_classes(self):
        """A single 'frame refs' number for llrm against the pin's split hid which class differed (x_dct: 45 vs 27)."""
        done = subprocess.run([sys.executable, str(Path(check.__file__)), "_bench_x_dct", str(KERNEL)], capture_output=True, text=True)
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        pin, llrm = done.stdout.splitlines()
        for line in (pin, llrm):
            for word in ("stores", "reloads", "argument"):
                self.assertIn(word, line)


if __name__ == "__main__":
    unittest.main()
