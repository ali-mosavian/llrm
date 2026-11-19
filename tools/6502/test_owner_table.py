import os
import tempfile
import unittest

import owner_table as t


def row(prog, lv, comp, b, c):
    return ["6502", prog, lv, comp, str(b), str(c)]


FULL = [row("k", "Os", "ours", 10, 100), row("k", "Os", "llvm-mos", 12, 90), row("k", "Os", "cc65", 14, 90), row("k", "Os", "sdcc", 9, 95)]


class OwnerTable(unittest.TestCase):
    def test_a_cell_with_all_three_references_is_graded_on_the_best_bytes(self):
        rows, refused = t.grade(t.cells_of(FULL), set(), False)
        self.assertEqual(refused, [])
        self.assertEqual(rows[0][-1], "WORSE")  # 10 B against SDCC's 9 B at Os

    def test_a_cell_missing_a_reference_is_refused_not_graded_on_the_rest(self):
        """z80's table took the next-best column and understated gaps by up to 1.7 kB."""
        short = [r for r in FULL if r[3] != "sdcc"]
        rows, refused = t.grade(t.cells_of(short), set(), False)
        self.assertEqual(rows, [])
        self.assertEqual(refused, [("k", "Os", "6502", ["sdcc"])])

    def test_program_cells_with_no_reference_are_refused_unless_declared_no_bar(self):
        """172 program cells had `ours` and no external column and were skipped without a line."""
        cells = t.cells_of([row("prog", "O2", "ours", 5, 50)])
        _, refused = t.grade(cells, {"prog"}, False)
        self.assertEqual(len(refused), 1)
        rows, refused = t.grade(cells, {"prog"}, True)
        self.assertEqual((rows[0][-1], refused), ("NO_BAR", []))

    def test_a_cpu_declared_ungraded_is_listed_not_graded_and_not_refused(self):
        """The matrix has no cc65 65C02 column: 116 kernel cells would otherwise be graded on two columns."""
        cells = t.cells_of([row("k", "Os", "ours-65c02", 5, 50), row("k", "Os", "llvm-mos-65c02", 4, 40)])
        rows, refused = t.grade(cells, set(), False, ("65c02",))
        self.assertEqual((rows[0][-1], refused), ("UNGRADED", []))
        self.assertEqual(len(t.grade(cells, set(), False)[1]), 1)

    def test_a_kernel_without_references_is_refused_even_with_programs_no_bar(self):
        cells = t.cells_of([row("kern", "O2", "ours", 5, 50)])
        _, refused = t.grade(cells, {"prog"}, True)
        self.assertEqual(len(refused), 1)

    def test_a_corpus_program_absent_from_the_table_is_listed(self):
        """The table had 86 of 170 executed corpus programs and said nothing."""
        with tempfile.TemporaryDirectory() as d:
            for n in ("a", "b"):
                open(os.path.join(d, n + ".c"), "w").close()
                open(os.path.join(d, n + ".out"), "w").close()
            names = t.corpus_programs(d)
        cells = t.cells_of([row("a", "O2", "ours", 5, 50)])
        self.assertEqual(t.coverage(cells, names), (1, ["b"]))


if __name__ == "__main__":
    unittest.main()
