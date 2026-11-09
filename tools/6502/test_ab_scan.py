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
