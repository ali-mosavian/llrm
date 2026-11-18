"""`python3 -m unittest discover -s crates/target/llrm-z80/tests/programs`"""

import importlib.util
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("dominated", HERE / "dominated.py")
dom = importlib.util.module_from_spec(spec)
spec.loader.exec_module(dom)


class Dominated(unittest.TestCase):
    def test_a_cell_bigger_and_slower_than_the_level_below_is_found(self):
        """idioms -O2: 4555 B / 25,582,412 T against -O1's 1869 B / 24,469,785 T; a cell bigger and no faster (signbit_idioms
        +162 B, +0 T) counts, one that buys clocks with bytes (loop_fusion) does not."""
        low = {"idioms": (1869, 24469785), "signbit_idioms": (2700, 410715), "loop_fusion": (844, 245592), "same": (10, 10)}
        high = {"idioms": (4555, 25582412), "signbit_idioms": (2862, 410715), "loop_fusion": (1623, 208734), "same": (10, 10)}
        self.assertEqual(dom.dominated(low, high), [("idioms", 2686, 1112627), ("signbit_idioms", 162, 0)])

    def test_every_known_cell_has_a_row_with_an_owner_and_a_date(self):
        rows = (HERE.parents[4] / "docs" / "working" / "above-main-rows.md").read_text()
        for cell, row in dom.known().items():
            line = next((line for line in rows.splitlines() if line.startswith(f"| {row} |")), None)
            self.assertIsNotNone(line, f"{cell}: no row {row}")
            self.assertRegex(line, r"20\d\d-\d\d-\d\d", f"{cell}: row {row} has no exit date")


if __name__ == "__main__":
    unittest.main()
