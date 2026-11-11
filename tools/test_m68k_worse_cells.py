"""The worse-cell count is strict: no tolerance (a 2 B Os loss counts). A cell is judged against the best external number present (gcc, clang or both); only a cell with no ours or no external at all is skipped, and the skipped cells are listed."""
import io
import sys
import unittest

sys.path.insert(0, __file__.rsplit("/", 1)[0])
from m68k_worse_cells import count, missing, missing_levels, one_sided, report, single_input, skipped, verdict, verdicts  # noqa: E402

HEADER = "# m68k matrix; (CpuTable::byte_rate: 68000 0.5, 68020 1.33, 68030/040/060 3.2 B per clk saved)\n"
COLS = "target\tprog\tlevel\tcompiler\tcode_bytes\tclocks\tinstrs\tnote\n"


def tsv(*rows):
    return io.StringIO(HEADER + COLS + "".join("\t".join(map(str, r)) + "\n" for r in rows))


def cell(target, prog, level, ours, gcc, clang):
    return [(target, prog, level, name, b, k, 1, "") for name, (b, k) in (("ours", ours), ("gcc", gcc), ("clang", clang))]


class HeaderRates(unittest.TestCase):
    def test_a_flat_two_bytes_per_clock_header_is_refused(self):
        stale = "# m68k matrix; byte_rate: 68000/68020/68040/68060 2 B per clk saved (O2: 1 clk = 2 B)\n"
        rows = cell("m68k-68000", "partition", "O2", (98, 1264), (92, 1226), (150, 2104))
        body = stale + COLS + "".join("\t".join(map(str, r)) + "\n" for r in rows)
        with self.assertRaises(SystemExit) as why:
            count(io.StringIO(body))
        self.assertIn("timings.times", str(why.exception))

    def test_the_committed_rates_are_per_cpu(self):
        from m68k_worse_cells import committed_rates

        rates = committed_rates()
        self.assertEqual((rates["68000"], round(rates["68020"], 2), rates["68040"]), (0.5, 1.33, 3.2))


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

    def test_o1_is_strict_on_clocks(self):
        """O1 prices no byte rate: same bytes and more clocks than gcc is worse (O2 would trade them at the rate)."""
        rows = cell("m68k-68000", "fib", "O1", (20, 110), (20, 100), (24, 120))
        self.assertEqual(count(tsv(*rows)), (1, 1))

    def test_o1_one_byte_over_is_worse_even_when_faster(self):
        rows = cell("m68k-68000", "fib", "O1", (21, 90), (20, 100), (24, 120))
        self.assertEqual(count(tsv(*rows)), (1, 1))

    def test_o1_no_worse_on_either_axis_is_clean(self):
        rows = cell("m68k-68000", "fib", "O1", (20, 100), (20, 100), (24, 120))
        self.assertEqual(count(tsv(*rows)), (0, 1))

    def test_a_program_with_no_o1_cell_is_reported(self):
        """The matrix measured O2 and Os only: O1 (strict) had no judged cell, so 'N worse' said nothing about it."""
        rows = [r for level in ("O2", "O3", "Os") for r in cell("m68k-68000", "fib", level, (20, 100), (20, 100), (24, 120))]
        self.assertEqual(missing_levels(tsv(*rows)), [("m68k-68000", "fib", "O1")])

    def test_max_run_on_one_input_is_reported(self):
        """max(7, 42) alone: gcc skipped the move, ours ran it; the other order reverses it (+6.4 B-eq at 060)."""
        once = [("m68k-68060", "max", "O2", n, 16, 10, 5, "") for n in ("ours", "gcc", "clang")]
        both = [("m68k-68060", "max", "O2", n, 16, 11, 6, "inputs 2") for n in ("ours", "gcc", "clang")]
        self.assertEqual(len(single_input(tsv(*once))), 3)
        self.assertEqual(single_input(tsv(*both)), [])


    def test_a_cell_with_one_external_column_is_listed_not_judged_silently(self):
        """z80's table took the next-best column for a row missing its best variant and understated the gaps by up
        to 1.7 kB: a cell with gcc but no clang (or the reverse) is judged against what exists, and named."""
        rows = cell("m68k-68000", "both", "Os", (22, 100), (20, 100), (24, 100))
        rows += [("m68k-68000", "gcc_only", "Os", "ours", 22, 100, 1, ""), ("m68k-68000", "gcc_only", "Os", "gcc", 20, 100, 1, "")]
        rows += [("m68k-68000", "clang_only", "Os", "ours", 22, 100, 1, ""), ("m68k-68000", "clang_only", "Os", "clang", 20, 100, 1, "")]
        self.assertEqual(one_sided(tsv(*rows)), [("m68k-68000", "gcc_only", "Os", "clang"), ("m68k-68000", "clang_only", "Os", "gcc")])


class Report(unittest.TestCase):
    def test_a_strict_o1_cell_shows_both_axes_not_one_mixed_number(self):
        """crc16 68000 O1 printed "+52.0 B WORSE": 10 B smaller than gcc, 52 clk slower. The number was the clock excess labelled bytes."""
        rows = cell("m68k-68000", "crc16", "O1", (74, 4252), (84, 4200), (152, 8134))
        (line,) = report(tsv(*rows))
        self.assertIn("dB=-10 B", line)
        self.assertIn("dT=+52 clk", line)
        self.assertNotIn("+52.0 B", line)

    def test_an_o2_cell_shows_the_net_and_the_rate_it_is_made_at(self):
        rows = cell("m68k-68000", "partition", "O2", (98, 1264), (92, 1226), (150, 2104))
        (line,) = report(tsv(*rows))
        self.assertIn("dB=+6 B dT=+38 clk", line)
        self.assertIn("net=+25.0 B at 0.5 B/clk", line)

    def test_an_os_cell_without_clocks_has_no_dt(self):
        rows = cell("m68k-68000", "fib", "Os", (22, None), (20, None), (24, None))
        (line,) = report(tsv(*[[*r[:5], "", *r[6:]] for r in rows]))
        self.assertIn("dB=+2 B dT=n/a", line)


class LinkedSize(unittest.TestCase):
    """The matrix counted a program's object, so a compiler-rt helper that one column linked was invisible
    (bitcount_idioms 68000 O2 read ours 386 B against gcc 378 B while gcc linked 30 B more)."""

    def row(self, compiler, code, library="", note=""):
        return ("m68k-68000", "bitcount_idioms", "O2", compiler, code, "", "", note, library)

    def test_a_reference_that_links_a_helper_is_judged_on_the_linked_size(self):
        rows = [self.row("ours", 386, 0), self.row("gcc", 378, 30), self.row("clang", 400, 0)]
        self.assertEqual(count(tsv(*rows)), (0, 1))

    def test_ours_linking_a_helper_counts_it(self):
        rows = [self.row("ours", 372, 28), self.row("gcc", 378, 0), self.row("clang", 672, 0)]
        self.assertEqual(count(tsv(*rows)), (1, 1))

    def test_a_reference_with_an_unresolved_helper_is_one_sided_not_judged(self):
        rows = [self.row("ours", 386, 0), self.row("gcc", 300, "", "unresolved helpers: __paritysi2"), self.row("clang", 672, 0)]
        self.assertEqual(count(tsv(*rows)), (0, 1))
        self.assertEqual(one_sided(tsv(*rows)), [("m68k-68000", "bitcount_idioms", "O2", "gcc")])

    def test_ours_with_an_unresolved_helper_is_missing(self):
        rows = [self.row("ours", 386, "", "unresolved helpers: __x"), self.row("gcc", 378, 0), self.row("clang", 380, 0)]
        self.assertEqual(missing(tsv(*rows)), [("m68k-68000", "bitcount_idioms", "O2")])


if __name__ == "__main__":
    unittest.main()


class Verdicts(unittest.TestCase):
    """The O1 per-axis bar and its VIRTUAL-OK exception (oversight, 2026-11-09; fputable.verdict for the m32 FPU table).
    Cells are (bytes, clocks). No published m68k cell is virtual (gcc is ahead of clang on both axes, or the reverse,
    everywhere), so the VIRTUAL-OK rows are shaped after fputable's tests."""

    def judge(self, level, ours, gcc, clang, rate=0.5):
        return verdict(level, rate, ours, {"gcc": gcc, "clang": clang})

    def test_real_cell_at_the_best_on_both_axes_is_ok(self):
        """sum 68000 O1 (published 2026-11-09): equal to gcc, no clang number."""
        self.assertEqual(verdict("O1", 0.5, (18, 358), {"gcc": (18, 358)}), "ok")

    def test_real_cell_behind_the_one_compiler_that_has_both_axes_is_worse(self):
        """crc16 68020 O1 (published 2026-11-09): 78 B against gcc's 84 but 2126 clk against 1816; gcc has the best bytes and clocks."""
        self.assertEqual(self.judge("O1", (78, 2126), (84, 1816), (152, 3608), rate=1.33), "WORSE")

    def test_a_virtual_cell_passes_when_pareto_optimal_and_not_behind_the_pin(self):
        """gcc has the best bytes, clang the best clocks; ours is behind gcc on bytes, level with clang on clocks and ahead of it on bytes."""
        self.assertEqual(self.judge("O1", (100, 100), (80, 200), (120, 100)), "VIRTUAL-OK")

    def test_a_virtual_cell_the_pin_beats_on_an_axis_is_worse(self):
        self.assertEqual(self.judge("O1", (100, 150), (80, 200), (120, 100)), "WORSE")

    def test_a_dominated_cell_is_worse(self):
        self.assertEqual(self.judge("O1", (90, 210), (80, 200), (120, 100)), "WORSE")

    def test_other_levels_have_no_virtual_exception(self):
        # ahead of gcc by the allowance (20 B over, 50 clk * 0.5 = 25 B allowed) and of clang (20 B under, 10 clk slower = 5 B)
        self.assertEqual(self.judge("O2", (100, 150), (80, 200), (120, 160)), "ok")
        # the same cell against a faster clang: 20 B under but 50 clk slower is 25 B at 0.5 B per clk, 5 B past the bar
        self.assertEqual(self.judge("O2", (100, 150), (80, 200), (120, 100)), "WORSE")
        self.assertEqual(self.judge("Os", (100, 100), (80, 200), (120, 100)), "WORSE")

    def test_a_virtual_cell_is_not_counted_worse(self):
        """Counted worse before verdicts() existed: the per-axis bar alone."""
        rows = cell("m68k-68000", "virt", "O1", (100, 100), (80, 200), (120, 100))
        self.assertEqual(count(tsv(*rows)), (0, 1))
        self.assertEqual([v for *_, v, _ in verdicts(tsv(*rows))], ["VIRTUAL-OK"])
