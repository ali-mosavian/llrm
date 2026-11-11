import os
import re
import shutil
import subprocess
import tempfile
import unittest

from pcprof import by_symbol, diff, profile, symbols

SRC = """.export _main
_main:
  ldx #{outer}
outer:
  ldy #{inner}
inner:
  dey
  bne inner
tail:
  dex
  bne outer
done:
  lda #0
  rts
"""


class Parse(unittest.TestCase):
    def test_labels_and_addresses_key_the_lines(self):
        syms = symbols(['sym\tid=0,name="inner",addrsize=absolute,scope=0,def=12,ref=8,val=0x22D,seg=0,type=lab'])
        prof = profile(["022D 100 200", "022E 100 290", "0240 1 2"])
        self.assertEqual(by_symbol(syms, prof), {"inner": (201, 492)})

    def test_functions_drop_the_labels_inside_a_function(self):
        lines = [
            'sym\tid=0,name="c_long_7",addrsize=absolute,scope=0,def=1,val=0x300,seg=0,type=lab',
            'sym\tid=1,name="c_long_7_b6",addrsize=absolute,scope=0,def=1,val=0x310,seg=0,type=lab',
            'sym\tid=2,name="c_long_7__tm37",addrsize=absolute,scope=0,def=1,val=0x320,seg=0,type=lab',
        ]
        self.assertEqual([n for _, n in symbols(lines, functions=True)], ["c_long_7"])
        self.assertEqual(len(symbols(lines)), 3)

    def test_a_diff_names_the_symbol_that_moved(self):
        self.assertEqual(diff({"a": (1, 10), "b": (2, 4)}, {"a": (1, 10), "b": (2, 9), "c": (1, 3)}),
                         {"a": (0, 0), "b": (0, 5), "c": (1, 3)})


@unittest.skipUnless(shutil.which("cl65") and os.environ.get("SIM65_PCPROF_BIN"), "needs cl65 and SIM65_PCPROF_BIN (tools/sim65-pcprof.sh)")
class Sim65(unittest.TestCase):
    """Positive control: a loop of known trip counts gives exact counts, and the sum is sim65's own total."""

    def run_loop(self, outer, inner):
        work = tempfile.mkdtemp()
        self.addCleanup(shutil.rmtree, work)
        open(f"{work}/l.s", "w").write(SRC.format(outer=outer, inner=inner))
        subprocess.run(["ca65", "-g", "-t", "sim6502", "-o", f"{work}/l.o", f"{work}/l.s"], check=True)
        subprocess.run(["ld65", "-t", "sim6502", "--dbgfile", f"{work}/l.dbg", "-o", f"{work}/l.bin", f"{work}/l.o", "sim6502.lib"], check=True)
        env = dict(os.environ, SIM65_PCPROF=f"{work}/l.prof")
        out = subprocess.run([os.environ["SIM65_PCPROF_BIN"], "-c", f"{work}/l.bin"], env=env, capture_output=True, text=True, check=True)
        total = int(re.search(r"(\d+) cycles", out.stdout)[1])
        dbg, prof = open(f"{work}/l.dbg").read().splitlines(), open(f"{work}/l.prof").read().splitlines()
        return by_symbol(symbols(dbg), profile(prof)), total, profile(prof)

    def test_a_ten_by_ten_loop_counts_exactly(self):
        rows, total, prof = self.run_loop(10, 10)
        self.assertEqual(rows["inner"][0], 100 * 2)  # dey, bne: 100 trips
        self.assertEqual(rows["tail"][0], 10 * 2)
        self.assertEqual(rows["outer"][0], 10)
        self.assertEqual(sum(c for _, c in prof.values()), total)

    def test_a_longer_inner_loop_moves_inner_and_nothing_else(self):
        a, _, _ = self.run_loop(10, 10)
        b, _, _ = self.run_loop(10, 20)
        moved = {name for name in a if a[name] != b[name]}
        self.assertEqual(moved, {"inner"})
        self.assertEqual(b["inner"][0], 200 * 2)


if __name__ == "__main__":
    unittest.main()
