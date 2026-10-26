"""clocksweep lists a slower program with its %, never reads a program that printed another output as a result, and marks one at the start-up's cost."""
from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path

spec = importlib.util.spec_from_file_location("clocksweep", Path(__file__).resolve().parent / "clocksweep.py")
clocksweep = importlib.util.module_from_spec(spec)
spec.loader.exec_module(clocksweep)


def arm(**rows):
    return {"isel": "", "bin": "x", "rows": {name: {"language": ".bas", **row} for name, row in rows.items()}}


def row(clocks, instructions=None):
    return {"clocks": clocks, "instructions": instructions or clocks // 2}


class CompareTests(unittest.TestCase):
    def test_a_slower_program_is_listed_with_its_percent_and_clocks(self):
        report = clocksweep.compare(arm(a=row(10000), b=row(20000)), arm(a=row(11000), b=row(20000)))
        self.assertTrue(any("+10.00%" in one and "+1000 clocks" in one and one.endswith("a") for one in report), report)
        self.assertFalse(any(one.endswith(" b") for one in report[3:]), report)

    def test_a_program_that_printed_another_output_is_void_not_faster(self):
        """A run that stopped early counts few clocks: read as a result, a miscompile showed as a 90% speed-up."""
        report = clocksweep.compare(arm(a=row(10000)), arm(a={"error": "prints another output: line 1"}))
        self.assertIn("1 void", "\n".join(report))
        self.assertNotIn("faster", "\n".join(report))
        self.assertTrue(any(one.strip().startswith("void") and "prints another output" in one for one in report), report)

    def test_a_program_near_the_startup_is_marked_tiny(self):
        """A source folded to a constant runs only the start-up on both sides; its few clocks of difference is not a finding."""
        report = clocksweep.compare(arm(empty=row(1000), a=row(5000), folded=row(1500)), arm(empty=row(1000), a=row(5000), folded=row(1520)))
        folded = [one for one in report if one.endswith("folded")]
        self.assertEqual(len(folded), 1)
        self.assertIn("tiny", folded[0])

    def test_the_level_is_replaced_and_the_programs_other_flags_kept(self):
        self.assertEqual(clocksweep.with_level(["-O2", "-march=pentium"]), ["-Os", "-march=pentium"])
        self.assertEqual(clocksweep.with_level(["-fno-inline"]), ["-fno-inline", "-Os"])


if __name__ == "__main__":
    unittest.main()
