"""The worse-cell count is strict: no tolerance (a 2 B Os loss counts). A cell is judged against the best external number present (gcc, clang or both); only a cell with no ours or no external at all is skipped, and the skipped cells are listed."""
import io
import sys
import unittest

sys.path.insert(0, __file__.rsplit("/", 1)[0])
from m68k_worse_cells import count, missing, skipped  # noqa: E402

HEADER = "# m68k matrix; (CpuTable::byte_rate: 68000 0.5, 68020 1.33, 68030/040/060 3.2 B per clk saved)\n"
COLS = "target\tprog\tlevel\tcompiler\tcode_bytes\tclocks\tinstrs\tnote\n"


def tsv(*rows):
    return io.StringIO(HEADER + COLS + "".join("\t".join(map(str, r)) + "\n" for r in rows))


def cell(target, prog, level, ours, gcc, clang):
    return [(target, prog, level, name, b, k, 1, "") for name, (b, k) in (("ours", ours), ("gcc", gcc), ("clang", clang))]


class WorseCells(unittest.TestCase):
    def test_two_byte_loss_at_os_counts(self):
        rows = cell("m68k-68000", "fib", "Os", (22, 100), (20, 100), (24, 100))
        self.assertEqual(count(tsv(*rows)), (1, 1))

    def test_o2_extra_bytes_inside_the_allowance_do_not_count(self):
        rows = cell("m68k-68000", "sum", "O2", (30, 100), (24, 112), (40, 200))
        self.assertEqual(count(tsv(*rows)), (0, 1))  # 6 B over, 12 clk saved * 0.5 = 6 B allowed

    def test_o2_one_byte_past_the_allowance_counts(self):
        rows = cell("m68k-68000", "sum", "O2", (31, 100), (24, 112), (40, 200))
        self.assertEqual(count(tsv(*rows)), (1, 1))

    def test_040_uses_the_shared_030_040_060_rate(self):
        rows = cell("m68k-68040", "p", "O2", (94, 235), (94, 195), (200, 400))
        self.assertEqual(count(tsv(*rows)), (1, 1))  # same bytes, 40 clk slower

    def test_cell_with_only_gcc_is_judged_against_gcc(self):
        """sum 68000 O2 has no clang number (it calls a runtime): ours 30 B / 422 clk against gcc 18 / 358 was invisible, +44 B-eq."""
        rows = [r for r in cell("m68k-68000", "sum", "O2", (30, 422), (18, 358), (1, 1)) if r[3] != "clang"]
        self.assertEqual(count(tsv(*rows)), (1, 1))

    def test_cell_with_only_clang_is_judged_against_clang(self):
        rows = [r for r in cell("m68k-68020", "x", "Os", (10, 1), (1, 1), (8, 1)) if r[3] != "gcc"]
        self.assertEqual(count(tsv(*rows)), (1, 1))

    def test_a_one_competitor_cell_that_is_not_worse_does_not_count(self):
        rows = [r for r in cell("m68k-68020", "x", "Os", (8, 1), (8, 1), (1, 1)) if r[3] != "clang"]
        self.assertEqual(count(tsv(*rows)), (0, 1))

    def test_a_cell_with_no_external_number_is_skipped_and_listed(self):
        rows = cell("m68k-68020", "y", "Os", (10, 1), (8, 1), (8, 1))
        rows = [r for r in rows if r[3] == "ours"] + [("m68k-68020", "y", "Os", "gcc", "", "", "", "n/a")]
        self.assertEqual(count(tsv(*rows)), (0, 0))
        self.assertEqual(skipped(tsv(*rows)), [("m68k-68020", "y", "Os")])

    def test_ours_ir_is_never_judged(self):
        """`ours-ir` is the backend on the bench IR, which is not what llrm-cc ships: a cell is judged on `ours`."""
        rows = cell("m68k-68000", "z", "Os", (22, 1), (20, 1), (24, 1)) + [("m68k-68000", "z", "Os", "ours-ir", 18, 1, 1, "")]
        self.assertEqual(count(tsv(*rows)), (1, 1))  # ours 22 > gcc 20 although ours-ir 18 < 20

    def test_a_bytes_only_cell_is_judged_on_bytes(self):
        """muldiv and structcopy call the runtime: all four columns were n/a, 16 cells never judged."""
        rows = [("m68k-68000", "muldiv", "O2", n, b, "", "", "calls the runtime: bytes only") for n, b in (("ours", 80), ("gcc", 66), ("clang", 90))]
        self.assertEqual(count(tsv(*rows)), (1, 1))
        self.assertEqual(skipped(tsv(*rows)), [])

    def test_a_cell_with_no_ours_number_is_reported_missing(self):
        """A matrix with an absent column passed as 'fewer worse cells'; it must fail instead."""
        rows = [("m68k-68000", "muldiv", "Os", n, b, "", "", "") for n, b in (("ours", ""), ("gcc", 66))]
        self.assertEqual(missing(tsv(*rows)), [("m68k-68000", "muldiv", "Os")])


if __name__ == "__main__":
    unittest.main()
