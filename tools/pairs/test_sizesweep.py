"""sizesweep's summary counts the rows of a sweep and never the TOTAL line as one."""
from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path

spec = importlib.util.spec_from_file_location("sizesweep", Path(__file__).resolve().parent / "sizesweep.py")
sizesweep = importlib.util.module_from_spec(spec)
spec.loader.exec_module(sizesweep)

SWEEP = "c/a.c [216, 115]\nc/b.c [160, 100]\nqb/c.bas [100, 110]\nTOTAL [1000, 949]\n"


class SummarizeTests(unittest.TestCase):
    def test_the_total_line_is_not_a_row(self):
        """The first summariser matched `TOTAL [..]` as a row: 32 rows shrinking 1942 B were reported as 3884 B in a PR body."""
        summary = sizesweep.summarize(SWEEP)
        self.assertIn("2 shrink by 161 B", summary)
        self.assertNotIn("212", summary)

    def test_a_grower_and_the_total_are_reported(self):
        summary = sizesweep.summarize(SWEEP)
        self.assertIn("1 variants grow by 10 B,", summary)
        self.assertIn("total 1000 -> 949 (-51 B)", summary)


if __name__ == "__main__":
    unittest.main()
