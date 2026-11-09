"""The worse-cell count is strict: no tolerance (a 2 B Os loss counts). A cell is judged against the best external number present (gcc, clang or both); only a cell with no ours or no external at all is skipped, and the skipped cells are listed."""
import io
import sys
import unittest

sys.path.insert(0, __file__.rsplit("/", 1)[0])
from m68k_worse_cells import (  # noqa: E402
    count,
    missing,
    missing_levels,
    one_sided,
    ours_failed,
    single_input,
    skipped,
    verdict,
    verdicts,
    with_library,
)

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

    def test_the_rate_text_follows_the_table_it_is_read_from(self):
        """A header writer that types its rates cannot follow a changed timings.times: this one reads them."""
        import tempfile
        from m68k_worse_cells import rate_text
        with tempfile.NamedTemporaryFile("w", suffix=".times") as table:
            table.write("march m68000 m68020 m68040\ncache_line 2 4 16\nline_fill_clocks 4 3 4\n")
            table.flush()
            self.assertEqual(rate_text(table.name), "68000 0.5, 68020 1.33, 68040 4")

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

    def test_ours_failing_to_build_where_gcc_builds_is_worse(self):
        """The corpus table: a program ours cannot compile is a loss, not a hole and not a skipped cell."""
        rows = [("m68k-68000", "p", "O2", "ours", "", "", "", "n/a: compile-fail: ours: error"), ("m68k-68000", "p", "O2", "gcc", 40, "", "", "")]
        self.assertEqual(count(tsv(*rows)), (1, 1))
        self.assertEqual(missing(tsv(*rows)), [])

    def test_ours_failing_with_no_external_number_is_skipped_not_judged(self):
        rows = [("m68k-68000", "p", "O2", n, "", "", "", "n/a: compile-fail: x") for n in ("ours", "gcc", "clang")]
        self.assertEqual(count(tsv(*rows)), (0, 0))
        self.assertEqual(missing(tsv(*rows)), [])

    def test_ours_with_no_number_and_no_reason_is_still_a_hole(self):
        rows = [("m68k-68000", "p", "O2", "ours", "", "", "", ""), ("m68k-68000", "p", "O2", "gcc", 40, "", "", "")]
        self.assertEqual(missing(tsv(*rows)), [("m68k-68000", "p", "O2")])

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

    def test_whole_program_adds_the_library_members(self):
        """ours built fewer bytes than gcc and called more runtime: object alone 40 < 44, whole program 40+30 > 44+0."""
        rows = [("m68k-68000", "p", "Os", "ours", 40, "", "", "", 30), ("m68k-68000", "p", "Os", "gcc", 44, "", "", "", 0)]
        self.assertEqual(count(tsv(*rows)), (0, 1))
        self.assertEqual(count(with_library(tsv(*rows))), (1, 1))

    def test_a_cell_worse_only_with_the_library_estimate_is_listed_not_counted(self):
        """--whole made `40 B + 30 B of our members` against gcc's `44 B + 0` a verdict: gcc's library bytes are our archive's for the helpers
        it calls, an estimate. The count stays the measured one; the cell is named under ESTIMATE."""
        import subprocess
        import tempfile
        rows = [("m68k-68000", "p", lv, name, bytes_, "", "", "", lib) for lv in ("O1", "O2", "O3", "Os")
                for name, bytes_, lib in (("ours", 40, 30 if lv == "Os" else 0), ("gcc", 44, 0))]
        with tempfile.NamedTemporaryFile("w", suffix=".tsv") as table:
            table.write(tsv(*rows).read())
            table.flush()
            out = subprocess.run([sys.executable, __file__.rsplit("/", 1)[0] + "/m68k_worse_cells.py", table.name, "--whole", "--allow-one-sided"],
                                 capture_output=True, text=True).stdout
        self.assertIn("0 worse of 4 judged cells", out)
        self.assertIn("ESTIMATE (never in the count above): 1 more cells", out)
        self.assertIn("m68k-68000/p/Os", out.split("ESTIMATE", 1)[1])

    def test_a_cell_whose_every_column_failed_still_counts_as_measured_at_that_level(self):
        """sum_three (x86 far pointers) fails in all three columns: its O1 row exists, it was reported 'level not measured'."""
        rows = [("m68k-68000", "p", lv, n, "", "", "", "n/a: compile-fail: x") for lv in ("O1", "O2", "O3", "Os") for n in ("ours", "gcc", "clang")]
        self.assertEqual(missing_levels(tsv(*rows)), [])

    def test_a_helper_the_archive_lacks_makes_the_whole_program_size_unknown(self):
        """gcc's __addxf3 is not in our compiler-rt: it linked as 0 library bytes, so a 96-bit long double program read 43 KB worse for ours."""
        rows = [("m68k-68000", "p", "O2", "ours", 100, "", "", "", 300), ("m68k-68000", "p", "O2", "gcc", 120, "", "", "unresolved helpers: __addxf3", ""), ("m68k-68000", "p", "O2", "clang", 150, "", "", "", 0)]
        whole = list(with_library(tsv(*rows)))
        self.assertEqual(count(iter(whole)), (0, 0))
        self.assertEqual(missing(iter(whole)), [])

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
        self.assertEqual(self.judge("O2", (100, 150), (80, 200), (120, 100)), "ok")  # 20 B over, 50 clk * 0.5 saved
        self.assertEqual(self.judge("Os", (100, 100), (80, 200), (120, 100)), "WORSE")

    def test_a_virtual_cell_is_not_counted_worse(self):
        """Counted worse before verdicts() existed: the per-axis bar alone."""
        rows = cell("m68k-68000", "virt", "O1", (100, 100), (80, 200), (120, 100))
        self.assertEqual(count(tsv(*rows)), (0, 1))
        self.assertEqual([v for *_, v, _ in verdicts(tsv(*rows))], ["VIRTUAL-OK"])
