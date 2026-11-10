#!/usr/bin/env python3
"""tools/6502/ab_scan.py. Two defects of the scratch script it replaced (2026-11-09): it scanned every program of tests/run/c, so the
x86 `-m32` ones read WRONG on both dists and were counted as rows; and the runtime it linked was cut short (test_ours_cr.py).
The cell test needs a dist: AB_DIST (a directory with llrm-cc and mos_rt.s) and sim65/cl65 on the path."""
import os, sys, tempfile, unittest
from pathlib import Path
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import ab_scan

DIST = os.environ.get("AB_DIST")


class Applicable(unittest.TestCase):
    def test_an_x86_program_is_not_a_row(self):
        names = ab_scan.applicable(["urem_eq", "spill_store_order", "stack_arrays_copy"])
        self.assertEqual(names, ["stack_arrays_copy"])

    def test_the_default_list_has_no_x86_only_program(self):
        names = ab_scan.applicable()
        self.assertNotIn("urem_eq", names)
        self.assertIn("stack_arrays_copy", names)


class Flags(unittest.TestCase):
    def test_the_compile_flags_are_the_ones_ours_cr_uses(self):
        """ab_scan sliced header_flags out of ours_cr.py's text; #8715 moved it to flagset and the scan died with a ValueError (2026-11-10)."""
        import flagset
        self.assertIs(ab_scan.program_flags, flagset.program_flags)
        self.assertEqual(ab_scan.CORPUS_COMMON, flagset.CORPUS_COMMON)


class Delta(unittest.TestCase):
    def cell(self, bytes_, insns, layouts):
        return {"bytes": bytes_, "insns": insns, "clks": sum(layouts) // len(layouts), "layouts": layouts}

    def test_one_unlucky_pad_is_layout_not_worse(self):
        """mul_constants O2: pads 20735549 x3 + 20971148 (base) against 20734844 x3 + 21144643 (new), 224 fewer instructions."""
        base = self.cell(14386, 4816876, [20735549, 20735548, 20735549, 20971148])
        new = self.cell(14002, 4816652, [20734844, 21144643, 20734844, 20734843])
        db, dc, dm, di, flag = ab_scan.delta(base, new)
        self.assertGreater(dc, 0)
        self.assertEqual((db, di, flag), (-384, -224, "LAYOUT"))
        self.assertLess(dm, 0)

    def test_more_executed_instructions_is_worse(self):
        base = self.cell(100, 1000, [5000] * 4)
        new = self.cell(98, 1003, [5007] * 4)
        self.assertEqual(ab_scan.delta(base, new)[4], "WORSE")


class ArgsAx(unittest.TestCase):
    def test_the_report_glue_is_chosen_per_dist(self):
        with tempfile.TemporaryDirectory() as old, tempfile.TemporaryDirectory() as new:
            (Path(new) / "ARGS_AX").write_text("")
            self.assertEqual((ab_scan.args_ax(old), ab_scan.args_ax(new)), ("0", "1"))


@unittest.skipUnless(DIST and os.path.isfile(f"{DIST}/mos_rt.s"), "AB_DIST is not a dist")
class OneCell(unittest.TestCase):
    def test_one_applicable_program_passes_under_a_dist(self):
        with tempfile.TemporaryDirectory() as tmp:
            verdict, data = ab_scan.cell(DIST, "O2", "stack_arrays_copy", f"{tmp}/c.tsv")
        self.assertEqual(verdict, "PASS", data)
        self.assertGreater(data["bytes"], 0)


if __name__ == "__main__":
    unittest.main()
