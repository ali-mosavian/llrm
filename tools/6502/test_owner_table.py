import os
import tempfile
import unittest

import owner_table as t


def row(prog, lv, comp, b, c):
    return ["6502", prog, lv, comp, str(b), str(c)]


FULL = [
    row("k", "Os", "ours", 10, 100),
    row("k", "Os", "llvm-mos", 12, 90),
    row("k", "Os", "cc65", 14, 90),
    row("k", "Os", "sdcc-size", 9, 95),
    row("k", "Os", "sdcc-size-ma", 11, 95),
]


class OwnerTable(unittest.TestCase):
    def test_a_cell_with_all_three_references_is_graded_on_the_best_bytes(self):
        rows, refused = t.grade(t.cells_of(FULL), set(), False)
        self.assertEqual(refused, [])
        self.assertEqual(rows[0][-1], "WORSE")  # 10 B against SDCC's 9 B at Os

    def test_a_cell_missing_a_reference_is_refused_not_graded_on_the_rest(self):
        """z80's table took the next-best column and understated gaps by up to 1.7 kB."""
        short = [r for r in FULL if r[3] != "sdcc-size-ma"]
        rows, refused = t.grade(t.cells_of(short), set(), False)
        self.assertEqual(rows, [])
        self.assertEqual(refused, [("k", "Os", "6502", ["sdcc-size-ma"])])

    def test_program_cells_with_no_reference_are_refused_unless_declared_no_bar(self):
        """172 program cells had `ours` and no external column and were skipped without a line."""
        cells = t.cells_of([row("prog", "O2", "ours", 5, 50)])
        _, refused = t.grade(cells, {"prog"}, False)
        self.assertEqual(len(refused), 1)
        rows, refused = t.grade(cells, {"prog"}, True)
        self.assertEqual((rows[0][-1], refused), ("NO_BAR", []))

    def test_sdcc_at_its_default_optimisation_is_not_a_reference(self):
        """The SDCC columns passed no optimisation flag: the Os bar was looser than SDCC's best."""
        old = [r for r in FULL if not r[3].startswith("sdcc")] + [row("k", "Os", "sdcc", 9, 95)]
        rows, refused = t.grade(t.cells_of(old), set(), False)
        self.assertEqual(rows, [])
        self.assertEqual(refused[0][3], ["sdcc-size", "sdcc-size-ma"])

    def test_the_os_bar_comes_from_the_size_variants_and_o2_from_all_four(self):
        speed = row("k", "Os", "sdcc-speed", 5, 50)  # fewer bytes, but not a size variant
        rows, _ = t.grade(t.cells_of(FULL + [speed]), set(), False)
        self.assertEqual(rows[0][5], 9)  # the Os bar ignores the speed variant
        o2 = [row("k", "O2", c, b, k) for c, b, k in (("ours", 10, 100), ("llvm-mos", 12, 90), ("cc65", 14, 90), ("sdcc-size", 9, 95), ("sdcc-speed", 12, 60), ("sdcc-size-ma", 9, 94), ("sdcc-speed-ma", 12, 59))]
        rows, refused = t.grade(t.cells_of(o2), set(), False)
        self.assertEqual((refused, rows[0][-1]), ([], "WORSE"))  # 100 + 30 against SDCC's 59 + 36

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
            names, _ = t.corpus_programs(d)
        cells = t.cells_of([row("a", "O2", "ours", 5, 50)])
        self.assertEqual(t.coverage(cells, names), (1, ["b"]))


def cell(level, **points):
    return [row("k", level, name.replace("_", "-"), b, c) for name, (b, c) in points.items()]


class Applies(unittest.TestCase):
    """A program `corpus_targets.applies` drops for 6502 (a `-m32`-forced one) stayed in the table's denominator and,
    as a program with no bar, in its cells (2026-11-09: 14 programs)."""

    def corpus(self, files):
        d = tempfile.mkdtemp()
        for name, text in files.items():
            open(os.path.join(d, name), "w").write(text)
        return d

    def test_a_program_the_target_does_not_take_is_dropped_from_the_denominator(self):
        d = self.corpus({"a.c": "int main(){}", "a.out": "", "b.c": "// flags: -m32\nint main(){}", "b.out": ""})
        names, dropped = t.corpus_programs(d)
        self.assertEqual((names, dropped), ({"a"}, {"b"}))


class VirtualOk(unittest.TestCase):
    """2026-11-09 ruling: a cell where no single reference attains both axes passes when ours is Pareto-optimal
    and at least as good as the pin-equivalent (llvm-mos); it was WORSE against the per-axis minima."""

    def grade(self, rows):
        graded, refused = t.grade(t.cells_of(rows), set(), False)
        self.assertEqual(refused, [])
        return graded[0][-1]

    def refs(self, level, ours, llvm_mos, cc65, size, size_ma, speed=(99, 999), speed_ma=(99, 999)):
        points = dict(ours=ours, llvm_mos=llvm_mos, cc65=cc65, sdcc_size=size, sdcc_size_ma=size_ma)
        if level == "O2":
            points.update(sdcc_speed=speed, sdcc_speed_ma=speed_ma)
        return cell(level, **points)

    def test_a_virtual_cell_with_ours_pareto_optimal_and_ahead_of_llvm_mos_passes(self):
        # bytes bar 8 (sdcc-size), clocks bar 70 (cc65): nobody has both; ours (9, 80) is dominated by none, llvm-mos is (12, 90)
        rows = self.refs("Os", (9, 80), (12, 90), (20, 70), (8, 100), (11, 95))
        self.assertEqual(self.grade(rows), "VIRTUAL_OK")

    def test_a_virtual_cell_dominated_by_a_reference_is_worse(self):
        rows = self.refs("Os", (9, 100), (12, 90), (20, 70), (8, 95), (11, 95))  # sdcc-size (8, 95) beats ours on both
        self.assertEqual(self.grade(rows), "WORSE")

    def test_a_virtual_cell_behind_llvm_mos_is_worse(self):
        rows = self.refs("Os", (13, 60), (12, 90), (20, 70), (8, 100), (11, 95))  # Pareto-optimal, but more bytes than llvm-mos
        self.assertEqual(self.grade(rows), "WORSE")

    def test_a_virtual_cell_ahead_of_llvm_mos_on_the_level_metric_but_behind_on_clocks_is_worse(self):
        rows = self.refs("Os", (9, 100), (12, 90), (20, 70), (8, 110), (11, 95))  # fewer bytes than llvm-mos, more clocks
        self.assertEqual(self.grade(rows), "WORSE")

    def test_a_cell_where_one_reference_attains_both_bars_stays_worse(self):
        rows = self.refs("Os", (10, 100), (12, 90), (14, 95), (9, 85), (11, 95))  # sdcc-size has the best bytes and clocks
        self.assertEqual(self.grade(rows), "WORSE")

    def test_the_virtual_cells_are_listed_with_every_columns_numbers(self):
        rows = self.refs("Os", (9, 80), (12, 90), (20, 70), (8, 100), (11, 95))
        cells = t.cells_of(rows)
        graded, _ = t.grade(cells, set(), False)
        text = "".join(t.virtual_notes(cells, graded))
        self.assertIn("k\tOs\t6502\t9/80\t8\t70", text)
        self.assertIn("llvm-mos=12/90", text)


if __name__ == "__main__":
    unittest.main()
