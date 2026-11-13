"""`m68k_clk.parse_clocks` reads the clocks cell of the `cycles` table. A table with no program row (the example found nothing to price) is `None`, not 0: a zero would read as a free program in the O2 judge."""
import sys
import unittest

sys.path.insert(0, __file__.rsplit("/", 1)[0])
from m68k_clk import parse_clocks  # noqa: E402

TABLE = "| exe | insns | 68000 clk | 68000 clk/call |\n608\n| rc.exe | 5035 | 43404 | 14.5 |\n"


class ParseClocks(unittest.TestCase):
    def test_the_clocks_cell_of_the_program_row(self):
        self.assertEqual(parse_clocks(TABLE), "43404")

    def test_a_data_dependent_bound_keeps_its_star(self):
        self.assertEqual(parse_clocks("| a.exe | 9 | 24634* | 1.0 |\n"), "24634*")

    def test_no_program_row_is_none_not_zero(self):
        self.assertIsNone(parse_clocks("| exe | insns | 68000 clk |\n"))
        self.assertIsNone(parse_clocks(""))

    def test_an_empty_clocks_cell_is_none(self):
        self.assertIsNone(parse_clocks("| a.exe | 9 |  | 1.0 |\n"))


if __name__ == "__main__":
    unittest.main()
