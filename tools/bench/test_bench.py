"""`uv run --project tools python -m unittest discover -s tools/bench`"""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import bench  # noqa: E402

BASE = {"instructions": 100, "memory_operands": 40}


class GateTests(unittest.TestCase):
    def test_the_baseline_itself_passes(self):
        self.assertEqual(bench.against("x", dict(BASE), BASE), [])

    def test_a_worse_count_fails(self):
        """The reason the gate exists: +10k instructions on one benchmark showed only as a total in a PR body."""
        problems = bench.against("textfill c -O2", {"instructions": 101, "memory_operands": 40}, BASE)
        self.assertEqual(len(problems), 1)
        self.assertIn("(worse)", problems[0])

    def test_a_better_count_fails_until_it_is_blessed(self):
        problems = bench.against("x", {"instructions": 100, "memory_operands": 39}, BASE)
        self.assertIn("(better: --bless", problems[0])

    def test_no_baseline_is_a_failure_not_a_pass(self):
        self.assertEqual(len(bench.against("x", dict(BASE), None)), 1)


if __name__ == "__main__":
    unittest.main()


class KnownTests(unittest.TestCase):
    def variant(self):
        return bench.Variant("fpbench", "c", Path("x.c"), "bench_fpbench", "#358")

    def test_a_known_variant_that_fails_only_at_one_level_is_still_measured_at_the_other(self):
        """fpbench fails at -O2 (#358) and compiles at -Os: the -Os count has to gate."""
        got = {("fpbench", "c", "O2"): {"error": "stack operands"}, ("fpbench", "c", "Os"): dict(BASE)}
        marked = bench.mark_known(got, [self.variant()], ["O2", "Os"])
        self.assertIn("known", marked[("fpbench", "c", "O2")])
        self.assertEqual(marked[("fpbench", "c", "Os")], BASE)

    def test_a_known_mark_on_a_variant_that_works_everywhere_fails(self):
        got = {("fpbench", "c", "O2"): dict(BASE), ("fpbench", "c", "Os"): dict(BASE)}
        marked = bench.mark_known(got, [self.variant()], ["O2", "Os"])
        self.assertIn("xpass", marked[("fpbench", "c", "O2")])


class BlessTests(unittest.TestCase):
    def test_a_reason_with_quotes_leaves_a_readable_file(self):
        """--reason 'VAL("0")' wrote reason = "...VAL("0")...": the next gate run died on the TOML."""
        import tempfile
        import tomllib

        with tempfile.TemporaryDirectory() as where:
            bench.write_expected(Path(where), {("x", "c", "O2"): dict(BASE)}, "x", 'folded VAL("0") \\ away')
            read = tomllib.loads((Path(where) / "expected.toml").read_text())
        self.assertEqual(read["reason"], 'folded VAL("0") \\ away')
        self.assertEqual(read["c"]["O2"], BASE)


class ReferenceTests(unittest.TestCase):
    """llrm against the reference compilers, through the gate's own entry point."""

    LLRM = {"instructions": 100, "memory_operands": 40, "kernel_ms": 1.0}
    BCC = {"instructions": 120, "memory_operands": 50, "kernel_ms": 1.2}

    def run_gate(self, measured: dict, extra: list[str] = ()) -> tuple[int, str]:
        """bench.main on a one-benchmark tree whose expected.toml blesses LLRM and BCC; `measured` stands in for the emulator."""
        import io
        import tempfile
        from contextlib import redirect_stdout
        from unittest import mock

        with tempfile.TemporaryDirectory() as where:
            root = Path(where)
            (root / "x").mkdir()
            (root / "x" / "bench.toml").write_text('[region]\nc = "f"\n')
            (root / "x" / "x.c").write_text("")
            (root / "x" / "x.out").write_text("0\n")
            bench.write_expected(root / "x", {("x", "c", "O2"): self.LLRM, ("x", "bcc", "O2"): self.BCC}, "x", "test")
            out = io.StringIO()
            with mock.patch.object(bench, "BENCH", root), mock.patch.object(bench, "measure_all", return_value=measured), mock.patch("sys.argv", ["bench", "--opt", "O2", *extra]), redirect_stdout(out):
                return bench.main(), out.getvalue()

    def test_the_gate_passes_on_the_blessed_numbers(self):
        code, text = self.run_gate({("x", "c", "O2"): dict(self.LLRM)})
        self.assertEqual(code, 0, text)

    def test_a_worse_time_ratio_to_bcc_fails_with_the_counts_unchanged(self):
        """llrm's kernel time rose 20% against BCC's with every count equal to the blessed one: the old gate, which saw no time, said 0 problems."""
        code, text = self.run_gate({("x", "c", "O2"): self.LLRM | {"kernel_ms": 1.2}})
        self.assertEqual(code, 1)
        self.assertIn("kernel_ms vs bcc", text)

    def test_the_standing_is_printed_every_run(self):
        code, text = self.run_gate({("x", "c", "O2"): dict(self.LLRM)})
        self.assertIn("llrm c -O2 vs bcc -O2: instructions -16.7% (n=1)", text)

    def test_a_reference_that_measures_differently_from_the_stored_one_fails(self):
        """--references re-measured BCC at 130 instructions where 120 is stored: the instrument moved, not llrm."""
        code, text = self.run_gate({("x", "c", "O2"): dict(self.LLRM), ("x", "bcc", "O2"): self.BCC | {"instructions": 130}})
        self.assertEqual(code, 1)
        self.assertIn("reference changed", text)

    def test_blessing_one_level_keeps_the_references_and_the_other_level(self):
        """write_expected rebuilt the file from this run alone: `--bless --opt O2` erased every -Os and reference table."""
        import tempfile
        import tomllib

        with tempfile.TemporaryDirectory() as where:
            bench.write_expected(Path(where), {("x", "c", "Os"): dict(BASE), ("x", "bcc", "Os"): dict(BASE)}, "x", "first")
            bench.write_expected(Path(where), {("x", "c", "O2"): dict(BASE)}, "x", "second")
            read = tomllib.loads((Path(where) / "expected.toml").read_text())
        self.assertEqual((read["c"]["Os"], read["bcc"]["Os"], read["c"]["O2"]), (BASE, BASE, BASE))


class KernelTimeTests(unittest.TestCase):
    def test_a_toolchains_start_up_is_taken_out_of_its_time(self):
        """Whole-program time priced C0M's initialisation against llrm's small crt: sieve -O2 read BCC 15% slower than llrm (0.195 ms against 0.170) where the kernels differ by 2.4%: the rest was C0M's start-up."""
        measured = {("s", "c", "O2"): {"cycles": 7500}, ("startup", "c", "O2"): {"cycles": 1500}, ("s", "bcc", "O2"): {"cycles": 9000}, ("startup", "bcc", "O2"): {"cycles": 3000}}
        net = bench.net_times(measured)
        self.assertEqual((net[("s", "c", "O2")]["kernel_ms"], net[("s", "bcc", "O2")]["kernel_ms"]), (0.08, 0.08))
        self.assertFalse([key for key in net if key[0] == "startup"])


@unittest.skipUnless(bench.references_available(), "needs BCC, Open Watcom and BC")
class BccTests(unittest.TestCase):
    def test_a_bcc_floating_point_program_links_and_prints_its_output(self):
        """floats.c linked with crt.asm failed on `__version` and `_errno` (FP87, MATHM want BCC's own start-up): six programs had no BCC row."""
        import tempfile

        variant = next(one for one in bench.variants(bench.BENCH / "floats") if one.language == "c")
        with tempfile.TemporaryDirectory(dir=bench.ROOT / "target") as where:
            built = bench.build_borland("bcc", [(variant, "B000")], "O2", Path(where))["B000"]
            self.assertNotIsInstance(built, str, built)
            got = bench.measure(("B000", built[0], built[1], variant.region, bench.expected_output(bench.BENCH / "floats"), built[0].with_suffix(".OBJ")))
        self.assertGreater(got.get("instructions", 0), 0, got)  # measure() fails on any other output than floats.out


@unittest.skipUnless(bench.references_available(), "needs BCC, Open Watcom and BC")
class ReferenceFloatTests(unittest.TestCase):
    def _counts(self, build, name="floats"):
        import tempfile

        variant = next(one for one in bench.variants(bench.BENCH / name) if one.language == "c")
        with tempfile.TemporaryDirectory(dir=bench.ROOT / "target") as where:
            built = build(variant, Path(where))
            self.assertNotIsInstance(built, str, built)
            return bench.measure(("F000", built[0], built[1], variant.region, bench.expected_output(bench.BENCH / name), next(built[0].parent.glob(built[0].stem + ".[oO][bB][jJ]"))))

    def test_a_watcom_floating_point_program_links_and_prints_its_output(self):
        """floats.c linked with -zl and a stub had no Watcom row: its FP and long helpers (__CHP, __I4M) lived in a C library nobody linked."""
        self.assertGreater(self._counts(lambda variant, where: bench.build_watcom(variant, "O2", where, "F000")).get("instructions", 0), 0)

    def test_watcom_runs_a_4k_stack_array_and_keeps_the_kernel_a_call(self):
        """ring (4K of locals) died in Watcom's default stack with an invalid instruction; textfill's kernel was inlined into main at -ox, so no row ('never entered')."""
        for name in ("ring", "textfill"):
            got = self._counts(lambda variant, where: bench.build_watcom(variant, "O2", where, "F000"), name)
            self.assertGreater(got.get("instructions", 0), 0, got)

    @unittest.skipUnless((bench.BORLAND["tc"][0] / "lib" / "MATHM.LIB").exists(), "needs Turbo C 2.01")
    def test_a_turbo_c_program_with_a_define_and_a_float_links_and_prints_its_output(self):
        """TC 2.01 read `#define` after a bare LF as an illegal character (no row for the 4 programs with one), and its MATHM.LIB was empty (no floats)."""
        for name in ("nbody_single", "floats"):
            got = self._counts(lambda variant, where: bench.build_borland("tc", [(variant, "F000")], "O2", where)["F000"], name)
            self.assertGreater(got.get("instructions", 0), 0, got)


SIZED = {"instructions": 100, "memory_operands": 40, "code_bytes": 200, "data_bytes": 10, "bss_bytes": 4}


class SizeTests(unittest.TestCase):
    """Code and data bytes of the program's own object, gated beside the counts."""

    def test_a_size_growth_fails(self):
        """A second copy of a helper grew code 200 -> 260 bytes with every count equal: the gate had no size to see."""
        problems = bench.against("x c -O2", SIZED | {"code_bytes": 260}, SIZED)
        self.assertEqual(len(problems), 1)
        self.assertIn("code_bytes 260 > 200 (worse)", problems[0])

    def test_a_data_or_bss_growth_fails(self):
        for counter in ("data_bytes", "bss_bytes"):
            self.assertIn("(worse)", bench.against("x", SIZED | {counter: SIZED[counter] + 1}, SIZED)[0])

    def test_a_baseline_without_sizes_fails_until_blessed(self):
        problems = bench.against("x", dict(SIZED), {"instructions": 100, "memory_operands": 40})
        self.assertEqual(len(problems), 3)
        self.assertIn("no baseline for code_bytes", problems[0])

    def test_the_size_ratio_to_a_reference_may_not_worsen(self):
        """llrm's code grew 200 -> 260 against BCC's 300: the ratio went 0.67 -> 0.87."""
        problems = bench.ratio_problems("x", "bcc", SIZED | {"code_bytes": 260}, SIZED, SIZED | {"code_bytes": 300})
        self.assertTrue(any("code_bytes vs bcc" in one for one in problems), problems)

    def test_a_reference_whose_size_moved_fails(self):
        problems = bench.drift_problems("x bcc -O2", SIZED | {"code_bytes": 210}, SIZED)
        self.assertIn("code_bytes 210 != stored 200", problems[0])

    def test_sizes_are_written_and_read_back(self):
        import tempfile
        import tomllib

        with tempfile.TemporaryDirectory() as where:
            bench.write_expected(Path(where), {("x", "c", "O2"): dict(SIZED), ("x", "bcc", "O2"): dict(SIZED)}, "x", "sizes")
            read = tomllib.loads((Path(where) / "expected.toml").read_text())
        self.assertEqual((read["c"]["O2"], read["bcc"]["O2"]), (SIZED, SIZED))

    def test_the_standing_prints_the_size_geomeans(self):
        import tempfile
        from unittest import mock

        with tempfile.TemporaryDirectory() as where:
            root = Path(where)
            (root / "x").mkdir()
            (root / "x" / "bench.toml").write_text('[region]\nc = "f"\n')
            bench.write_expected(root / "x", {("x", "bcc", "O2"): SIZED | {"code_bytes": 400}}, "x", "t")
            with mock.patch.object(bench, "BENCH", root):
                text = "\n".join(bench.standing({("x", "c", "O2"): dict(SIZED)}, [], ["O2"]))
        self.assertIn("code bytes -50.0% (n=1)", text)

    def test_a_program_is_sized_by_its_object_not_its_image(self):
        """The image holds the start-up and the libraries: 3000 bytes here for a 12-byte program. A size taken from it
        (or from a map that lists the libraries) reads the toolchain's bytes, which differ per compiler."""
        import tempfile
        from unittest import mock

        sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
        import test_sizes

        with tempfile.TemporaryDirectory() as where:
            obj, exe = Path(where) / "p.obj", Path(where) / "p.exe"
            obj.write_bytes(test_sizes.object_with(12))
            exe.write_bytes(b"MZ" + bytes(3000))
            run = mock.Mock(error=None, output=b"1\n", region=mock.Mock(instructions=5, memory_operands=2))
            with mock.patch.object(bench.maps, "locate", return_value=0), mock.patch.object(bench.icount, "run", return_value=run):
                got = bench.measure(("p", exe, exe, "f", ["1"], obj))
        self.assertEqual((got["code_bytes"], got["data_bytes"], got["bss_bytes"]), (12, 0, 40))


class OwnFramesTests(unittest.TestCase):
    def test_a_basic_source_is_measured_with_and_without_own_frames(self):
        """recursive BASIC pays the runtime's B$ENRA/B$EXSA per call (fib 14.9 ms against 5.0 with --own-frames): the bench only built the default."""
        languages = [one.language for one in bench.variants(bench.BENCH / "fib")]
        self.assertEqual(languages.count("bas"), 1)
        self.assertEqual(languages.count("basown"), 1)

    def test_only_the_own_frames_variant_passes_the_flag(self):
        import tempfile
        from unittest import mock

        seen = {}
        for variant in bench.variants(bench.BENCH / "fib"):
            if variant.language in bench.BASIC_LANGUAGES:
                with tempfile.TemporaryDirectory() as where, mock.patch.object(bench.subprocess, "run") as run:
                    run.return_value = mock.Mock(returncode=1, stderr="", stdout="")
                    bench.build(variant, "O2", Path(where), "V000")
                    seen[variant.language] = run.call_args[0][0]
        self.assertNotIn("--own-frames", seen["bas"])
        self.assertIn("--own-frames", seen["basown"])
