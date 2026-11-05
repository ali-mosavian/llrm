"""The worse-cell count is strict: no tolerance (a 2 B Os loss counts), cells without both gcc and clang are not comparable."""
import io
import sys
import unittest

sys.path.insert(0, __file__.rsplit("/", 1)[0])
from m68k_worse_cells import count  # noqa: E402

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

    def test_cell_missing_a_competitor_is_not_comparable(self):
        rows = [r for r in cell("m68k-68020", "x", "Os", (10, 1), (8, 1), (8, 1)) if r[3] != "clang"]
        self.assertEqual(count(tsv(*rows)), (0, 0))


if __name__ == "__main__":
    unittest.main()
