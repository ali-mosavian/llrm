"""tools/arm64/cellcmp.py compares only cells built the same way."""

import importlib.util
import os
import unittest

PATH = os.path.join(os.path.dirname(__file__), "arm64", "cellcmp.py")
spec = importlib.util.spec_from_file_location("cellcmp", PATH)
cellcmp = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cellcmp)


def row(comp, instrs, flags="-fPIE"):
    return ["jpeg_single", "3", comp, "6000", str(instrs), "kernel", "ok", flags]


class Columns(unittest.TestCase):
    def test_a_cc_row_against_a_ccn_row_is_refused_not_subtracted(self):
        # jpeg_single O3: `cc` 7,391,418 read against the `ccn` reference 7,132,248 sent a bisect of 8 builds
        # to two sessions for a +259k that was the flags.
        key = ("jpeg_single", "3")
        lines, refused = cellcmp.compare({key: row("ccn", 7132248)}, {key: row("cc", 7391418, "unknown")})
        self.assertTrue(refused, lines)
        self.assertIn("column ccn vs cc", lines[0])

    def test_equal_columns_give_the_delta(self):
        key = ("jpeg_single", "3")
        lines, refused = cellcmp.compare({key: row("ccn", 7132248)}, {key: row("ccn", 7117848)})
        self.assertFalse(refused)
        self.assertIn("(-14400)", lines[0])

    def test_unknown_flags_are_refused_even_in_the_same_column(self):
        key = ("jpeg_single", "3")
        _, refused = cellcmp.compare({key: row("cc", 1, "unknown")}, {key: row("cc", 2, "unknown")})
        self.assertTrue(refused)


if __name__ == "__main__":
    unittest.main()
