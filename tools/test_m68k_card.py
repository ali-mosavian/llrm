"""The m68k card counts by the rulings: -O1/-O2/-O3 by clocks, -Os by bytes and clocks, 1% over is at the best, an unmeasured axis is named.

The old count (`m68k_worse_cells.py`, bytes-only matrix) read huffman_decode -O2, 2.7x gcc's bytes and 6% faster, as worse."""
import sys
import unittest

sys.path.insert(0, __file__.rsplit("/", 1)[0])
from m68k_card import card, judge  # noqa: E402


class Rulings(unittest.TestCase):
    def test_a_speed_level_is_judged_on_clocks_not_bytes(self):
        self.assertEqual(judge("O2", 1440, 136430, 526, 145262), "at_or_below")

    def test_a_speed_level_loss_is_a_clock_loss(self):
        self.assertEqual(judge("O3", 828, 116920, 340, 113096), "clocks_worse")

    def test_one_percent_over_is_at_the_best(self):
        self.assertEqual(judge("O2", 10, 10100, 10, 10000), "at_or_below")
        self.assertEqual(judge("O2", 10, 10101, 10, 10000), "clocks_worse")

    def test_os_judges_bytes_and_clocks(self):
        self.assertEqual(judge("Os", 122, 100, 114, 100), "bytes_worse")
        self.assertEqual(judge("Os", 114, 120, 114, 100), "clocks_worse")
        self.assertEqual(judge("Os", 122, 120, 114, 100), "both_worse")
        self.assertEqual(judge("Os", 114, 100, 114, 100), "at_or_below")

    def test_an_unmeasured_axis_is_named_never_counted(self):
        self.assertEqual(judge("O2", 100, None, 100, 500), "unmeasured")
        self.assertEqual(judge("Os", 114, None, 114, 100), "unmeasured")
        self.assertEqual(judge("Os", 122, None, 114, 100), "bytes_worse")

    def test_the_card_counts_every_row_once(self):
        rows = [("68000", "a", "O2", 1, 100, 1, 100), ("68000", "b", "O2", 1, 200, 1, 100), ("68000", "c", "O1", 1, None, 1, 100)]
        found = card(rows)
        self.assertEqual({name: len(cells) for name, cells in found.items()}, {"at_or_below": 1, "bytes_worse": 0, "clocks_worse": 1, "both_worse": 0, "unmeasured": 1})


if __name__ == "__main__":
    unittest.main()
