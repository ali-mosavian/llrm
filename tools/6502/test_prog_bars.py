#!/usr/bin/env python3
"""tools/6502/prog_bars.py: the external program bars. Needs cc65, sdcc, sim65 and the llvm-mos SDK (a missing tool skips its test)."""
import os, shutil, sys, tempfile, unittest
from pathlib import Path
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import prog_bars as pb

HAVE = {"cc65": shutil.which("cc65") and shutil.which("sim65"), "sdcc": shutil.which("sdcc") and shutil.which("sim65"),
        "llvm-mos": (pb.MOS / "bin/mos-sim-clang").exists()}
HAVE[pb.LTO] = HAVE["llvm-mos"]


def measure(compiler, source, want, level="O2"):
    """(bytes, clocks, insns, note) of `source` reporting `want`, on a temporary corpus."""
    with tempfile.TemporaryDirectory() as tmp:
        Path(tmp, "t.c").write_text(source)
        Path(tmp, "t.out").write_text("".join(f"{v}\n" for v in want))
        old, pb.CORPUS = pb.CORPUS, Path(tmp)
        try:
            return pb.measure(compiler, level, "t")
        finally:
            pb.CORPUS = old


class Counted(unittest.TestCase):
    def test_a_tie_counts_and_a_program_without_a_bar_is_not_counted(self):
        self.assertTrue(pb.counted(100, [100, 120]))
        self.assertFalse(pb.counted(101, [100, None]))
        self.assertIsNone(pb.counted(100, [None, None]))


class NoBarLayout(unittest.TestCase):
    """volatile_bitfield: `struct S` is 4 B here (clang's msp430 layout packs f at bit 3) and 5 B for llvm-mos and sdcc (f at bit 8), so
    their 502 B and 426 B were bars of another program: the cell counted +147 B / +223 B that no code change could close."""
    DUMPS = ("*** Dumping AST Record Layout\n         0 | struct S\n     0:0-2 |   unsigned int a\n    0:3-17 |   unsigned int f\n"
             "           | [sizeof=4, align=1]\n", "*** Dumping AST Record Layout\n         0 | struct S\n     0:0-2 |   unsigned int a\n"
             "    1:0-14 |   unsigned int f\n           | [sizeof=5, align=1]\n")

    def test_a_record_that_lays_out_differently_is_named_with_both_sizes(self):
        ours, theirs = (pb.record_layouts(d) for d in self.DUMPS)
        self.assertEqual(pb.layout_reasons(ours, theirs), ["struct S 4 B here against 5 B"])
        self.assertEqual(pb.layout_reasons(ours, ours), [])

    def test_a_bar_of_another_struct_layout_is_not_a_bar(self):
        with tempfile.TemporaryDirectory() as tmp:
            bars = Path(tmp, "bars.tsv")
            bars.write_text("# header\n" + "".join(f"volatile_bitfield\t{c}\tO2\t-O2\t502\t946\t-\t\n" for c in ("llvm-mos", pb.LTO)) +
                            "other\tllvm-mos\tO2\t-O2\t90\t800\t-\t\nvolatile_bitfield\tsdcc\tO2\t-\tn/a\tn/a\t-\tn/a: i64: x\n")
            rows = pb.load_bars(bars, no_bar={("volatile_bitfield", "llvm-mos"): "struct S 4 B here against 5 B",
                                              ("volatile_bitfield", "sdcc"): "struct S is not 4 B there"})
        self.assertEqual(rows[("volatile_bitfield", "llvm-mos", "O2")][1], "n/a")
        self.assertEqual(rows[("volatile_bitfield", pb.LTO, "O2")][1], "n/a", "the LTO bar is llvm-mos's program too")
        self.assertIn("layout", rows[("volatile_bitfield", "llvm-mos", "O2")][4])
        self.assertEqual(rows[("other", "llvm-mos", "O2")][1], "90")
        self.assertIn("i64", rows[("volatile_bitfield", "sdcc", "O2")][4], "a cell that did not build keeps its own reason")

    def test_the_committed_list_takes_the_bars_of_volatile_bitfield(self):
        rows = pb.load_bars(pb.BARS)
        for level in pb.LEVELS:
            for compiler in pb.ALL:
                row = rows.get(("volatile_bitfield", compiler, level))
                self.assertTrue(row is None or row[1] == "n/a", f"{compiler} {level}: {row}")


class Images(unittest.TestCase):
    def test_mos_image_leaves_the_vectors_out(self):
        seg = lambda at, n: bytes([at & 255, at >> 8, n, 0]) + bytes(n)
        self.assertEqual(pb.image_bytes_mos(seg(0x200, 7) + seg(0xFFFA, 6)), 7)

    def test_ihx_counts_data_records_only(self):
        self.assertEqual(pb.image_bytes_ihx(":03010000AABBCC00\n:020000040000FA\n:00000001FF\n"), 3)


class Instrument(unittest.TestCase):
    """The harness' own faults, found building it (2026-11-15): the sdcc `report` assembled `(lp),y` as `sta abs,y`, so every sdcc cell
    read `wrong`; sdcc and cc65 refused every program naming a clang predefine (`__INT32_TYPE__`, 16 cells n/a); the empty-main floor
    must cancel exactly (a program that is the floor measures its own `rts`, 1 byte)."""
    SRC = "extern void report(long v);\nint main(void)\n{\n    report(0x01020304L);\n    report(-2L);\n    return 0;\n}\n"

    def each(self, f):
        for compiler in pb.ALL:
            if HAVE[compiler]:
                with self.subTest(compiler=compiler):
                    f(compiler)

    def test_reported_longs_are_read_back(self):
        def check(c):
            self.assertIsNotNone(measure(c, self.SRC, [0x01020304, -2])[0], c)
        self.each(check)

    def test_a_wrong_or_missing_report_is_not_a_bar(self):
        def check(c):
            bytes_, _, _, note = measure(c, self.SRC, [0x01020304, -3])
            self.assertIsNone(bytes_)
            self.assertTrue(note.startswith("wrong"), note)
            bytes_, _, _, note = measure(c, self.SRC, [0x01020304, -2, 5])
            self.assertTrue(note.startswith("wrong"), note)
        self.each(check)

    def test_a_program_with_over_255_reports_is_checked_not_looped_on(self):
        """The check loop indexed with `unsigned char`: overflow_builtins (324 reports) never ended on a right answer, read `no halt`
        and dropped out of the bars (llvm-mos, 2026-11-15)."""
        src = "extern void report(long v);\nint main(void)\n{\n    int i;\n    for (i = 0; i < 300; i++) report(i * 3L);\n    return 0;\n}\n"
        def check(c):
            self.assertIsNotNone(measure(c, src, [i * 3 for i in range(300)])[0], c)
        self.each(check)

    def test_a_difference_at_report_123_is_wrong_not_a_timeout(self):
        """The driver exits 1 + index (124 for report #123) and a timeout was also 124: the cell read `no halt`, not `wrong`."""
        src = "extern void report(long v);\nint main(void)\n{\n    int i;\n    for (i = 0; i < 130; i++) report(i);\n    return 0;\n}\n"
        want = list(range(130))
        want[123] = -1
        def check(c):
            note = measure(c, src, want)[3]
            self.assertEqual(note, "wrong: report #123 differs", c)
        self.each(check)

    def test_a_program_that_never_halts_is_n_a(self):
        def check(c):
            old, pb.RUN_TIMEOUT = pb.RUN_TIMEOUT, 3
            try:
                note = measure(c, "volatile int v;\nint main(void)\n{\n    for (;;) v = 1;\n}\n", [])[3]
            finally:
                pb.RUN_TIMEOUT = old
            self.assertTrue(note.startswith("n/a: no halt"), note)
        self.each(check)

    def test_the_floor_cancels_exactly(self):
        self.each(lambda c: self.assertEqual(measure(c, "void main(void)\n{\n}\n", [])[0], 1, c))

    def test_clang_predefines_exist_for_every_compiler(self):
        src = ("extern void report(long v);\nint main(void)\n{\n    __INT32_TYPE__ x = 70000L;\n"
               "    report(__BYTE_ORDER__ == __ORDER_LITTLE_ENDIAN__ ? x : 0);\n    return 0;\n}\n")
        self.each(lambda c: self.assertIsNotNone(measure(c, src, [70000])[0], c))


class OursNeedsTheProductRuntime(unittest.TestCase):
    """`ours` without `--rtlib` linked the runtime with BSS scratch cells: a 200-trip 32-bit multiply loop read 361k clocks against
    llvm-mos's 342k where the product's zero-page runtime reads 339k, and a ranking of cells over the bar was made on it."""

    def test_ours_refuses_to_run_without_an_rtlib(self):
        import subprocess
        run = subprocess.run([sys.executable, os.path.join(HERE, "prog_bars.py"), "ours", "--cc", "/nonexistent/llrm-cc", "--rt", "/nonexistent.s",
                              "--out", tempfile.mkdtemp(), "--progs", "x"], capture_output=True, text=True)
        self.assertNotEqual(run.returncode, 0)
        self.assertIn("--rtlib is required", run.stderr)


if __name__ == "__main__":
    unittest.main()
