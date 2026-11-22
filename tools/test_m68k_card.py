"""The m68k card counts by the rulings: -O1/-O2/-O3 by clocks, -Os by bytes and clocks, 1% over is at the best, an unmeasured axis is named.

The old count (`m68k_worse_cells.py`, bytes-only matrix) read huffman_decode -O2, 2.7x gcc's bytes and 6% faster, as worse."""
import sys
import unittest

sys.path.insert(0, __file__.rsplit("/", 1)[0])
from m68k_card import card, judge, read  # noqa: E402


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

    def test_a_libcall_the_other_side_links_counts_as_its_bytes(self):
        """shift64_variable -Os 68020: 416 B code against gcc's 330 B, but gcc links 216 B of `__ashldi3` and friends: the card read +26% worse (ours is smaller)."""
        import os
        import tempfile

        with tempfile.NamedTemporaryFile("w", suffix=".tsv", delete=False) as out:
            out.write("68020\tshift64_variable\tOs\t416\t310265\t330\t319063\t0\t216\n")
        try:
            rows = read([out.name])
        finally:
            os.unlink(out.name)
        self.assertEqual(rows, [("68020", "shift64_variable", "Os", 416, 310265, 546, 319063)])
        self.assertEqual(judge("Os", *rows[0][3:]), "at_or_below")

    def test_a_side_whose_library_link_failed_is_unmeasured_not_small(self):
        """widening_multiply -Os 68000: ours linked 1366 B of library, gcc's link failed (empty cell) and its 339 B code alone read as the whole program: ours +368% worse."""
        import os
        import tempfile

        with tempfile.NamedTemporaryFile("w", suffix=".tsv", delete=False) as out:
            out.write("68000\twidening_multiply\tOs\t301\t\t339\t\t1366\t\n")
        try:
            rows = read([out.name])
        finally:
            os.unlink(out.name)
        self.assertEqual(rows[0][3:], (1667, None, None, None))
        self.assertEqual(judge("Os", *rows[0][3:]), "unmeasured")


if __name__ == "__main__":
    unittest.main()


def test_a_lower_bound_over_gcc_is_a_clocks_worse_cell_not_unmeasured(tmp_path):
    """pattern_fill 68000 O1 read `93048*` against gcc 86440 and the card named it unmeasured: the true count is at least 93048, a counted loss."""
    import m68k_card

    path = tmp_path / "c.tsv"
    path.write_text("68000\tpattern_fill\tO1\t264\t>=93048\t232\t86440\t0\t0\n68020\tmemmove_overlap\tO2\t794\t<=4668\t250\t8548\t0\t0\n")
    found = m68k_card.card(m68k_card.read([str(path)]))
    assert [cell[2] for cell in found["clocks_worse"]] == ["pattern_fill"], found
    assert [cell[2] for cell in found["at_or_below"]] == ["memmove_overlap"], found
