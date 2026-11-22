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

    def test_the_cpu_names_the_targets(self):
        """`OURS_CPU=65c02` had no effect: every 65C02 A/B ran the 6502 target (none could be run, 2026-11-09)."""
        with tempfile.NamedTemporaryFile("w", suffix=".s", delete=False) as f:
            f.write(RUNTIME)
        code = "import ours_cr; print(ours_cr.CPU, ours_cr.SIM)"
        here = os.path.dirname(os.path.abspath(__file__))
        got = {}
        for cpu in ("6502", "65c02"):
            env = dict(os.environ, OURS_RT=f.name, OURS_CPU=cpu)
            got[cpu] = subprocess.run([sys.executable, "-c", code], cwd=here, env=env, capture_output=True, text=True, check=True).stdout.split()
        os.unlink(f.name)
        self.assertEqual(got, {"6502": ["6502", "sim6502"], "65c02": ["65c02", "sim65c02"]})


if __name__ == "__main__":
    unittest.main()


class ZeroBss(unittest.TestCase):
    """sim65 starts with unzeroed memory, so cc65's crt0 zeroes the BSS (12-15 clk a byte), and llvm-mos's mos-sim starts zeroed
    and charges nothing (400 B of BSS: 61 cycles, as 1 B): a program with static storage read worse than the bar by the zeroing
    alone, long_double_constant_store O3 x4.01 with ~1,450 of 3,930 net clocks (2026-11-22). A program's clocks are net of it."""
    def net(self, res):
        code = ("import ours_cr, subprocess, tempfile, pathlib\n"
                "w = pathlib.Path(tempfile.mkdtemp())\n"
                "(w / 'd.s').write_text(ours_cr.driver())\n"
                f"(w / 'f.s').write_text('.export main\\nmain: rts\\nrc0=$80\\nrc1=$81\\nrc2=$82\\nrc3=$83\\nrc4=$84\\nrc5=$85\\n.segment \"BSS\"\\nb: .res {res}\\n')\n"
                "k = subprocess.run(['cl65', '-t', ours_cr.SIM, '-m', 'x.map', '-o', 'x.bin', 'd.s', 'f.s'], cwd=w, capture_output=True, text=True)\n"
                "assert k.returncode == 0, k.stderr\n"
                "print(ours_cr.net_cycles(w))\n")
        with tempfile.NamedTemporaryFile("w", suffix=".s", delete=False) as f:
            f.write("")
        env = dict(os.environ, OURS_RT=f.name)
        here = os.path.dirname(os.path.abspath(__file__))
        out = subprocess.run([sys.executable, "-c", code], cwd=here, env=env, capture_output=True, text=True)
        os.unlink(f.name)
        self.assertEqual(out.returncode, 0, out.stderr)
        return int(out.stdout.split()[-1])

    def test_the_clocks_do_not_grow_with_the_bss(self):
        small, large = self.net(1), self.net(400)
        self.assertLess(abs(large - small), 12, f"1 B of BSS: {small} clocks, 400 B: {large}")
