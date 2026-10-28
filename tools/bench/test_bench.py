"""`uv run --project tools python -m unittest discover -s tools/bench`"""

import re
import sys
import json
import pytest
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

    def test_a_better_count_passes_and_is_the_next_base(self):
        """A better count failed until someone blessed it into expected.toml: every PR that moved a count edited the file and conflicted with the next."""
        self.assertEqual(bench.against("x", {"instructions": 100, "memory_operands": 39}, BASE), [])

    def test_a_row_with_no_base_is_not_judged(self):
        """A new program has no base to be worse than."""
        self.assertEqual(bench.against("x", dict(BASE), None), [])


class LinkTests(unittest.TestCase):
    def test_a_nib_benchmark_links_the_files_its_link_header_names_resolved(self):
        """`link: @c-runtime` went to nib-build.sh as a file name (llrm-c: wccq failed on .../@c-runtime) for parity and scalar."""
        extras = bench.nib_extras(bench.ROOT / "bench/parity/parity/parity.nib")
        self.assertEqual(extras, [str(bench.ROOT / "runtime/c/x86-m16/ext.asm")])


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
    def test_blessing_changes_the_files_whose_numbers_moved_and_no_others(self):
        """A bless rewrote the reason and date of every benchmark: a pull request of 43 files where 6 had moved (#1024, #1085)."""
        import tempfile
        import tomllib

        with tempfile.TemporaryDirectory() as where:
            one, other = Path(where) / "one", Path(where) / "other"
            for directory in (one, other):
                directory.mkdir()
                bench.write_expected(directory, {("x", "c", "O2"): dict(BASE)}, "x", "first")
            before = {d: (d / "expected.toml").read_bytes() for d in (one, other)}
            moved = dict(BASE, instructions=BASE["instructions"] - 7)
            self.assertFalse(bench.write_expected(other, {("x", "c", "O2"): dict(BASE)}, "x", "second"))
            self.assertTrue(bench.write_expected(one, {("x", "c", "O2"): moved}, "x", "second"))
            self.assertEqual((other / "expected.toml").read_bytes(), before[other])
            changed = tomllib.loads((one / "expected.toml").read_text())
        self.assertEqual(changed["reason"], "second")
        self.assertEqual(changed["c"]["O2"]["instructions"], BASE["instructions"] - 7)

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
            base = {("x", "c", "O2"): self.LLRM}
            with mock.patch.object(bench, "BENCH", root), mock.patch.object(bench, "base_rows", return_value=(base, {"x"}, "0" * 40)), mock.patch.object(bench, "measure_all", return_value=measured), mock.patch("sys.argv", ["bench", "--opt", "O2", *extra]), redirect_stdout(out):
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
        with tempfile.TemporaryDirectory(dir=bench.llrmbin.target_dir()) as where:
            built = bench.build_borland("bcc", [(variant, "B000")], "O2", Path(where))["B000"]
            self.assertNotIsInstance(built, str, built)
            got = bench.measure(("B000", built[0], built[1], variant.region, bench.expected_output(bench.BENCH / "floats"), built[0].with_suffix(".OBJ")))
        self.assertGreater(got.get("instructions", 0), 0, got)  # measure() fails on any other output than floats.out


@unittest.skipUnless(bench.references_available(), "needs BCC, Open Watcom and BC")
class ReferenceFloatTests(unittest.TestCase):
    def _counts(self, build, name="floats"):
        import tempfile

        variant = next(one for one in bench.variants(bench.BENCH / name) if one.language == "c")
        with tempfile.TemporaryDirectory(dir=bench.llrmbin.target_dir()) as where:
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

    def test_a_counter_the_base_lacks_is_not_judged(self):
        self.assertEqual(bench.against("x", dict(SIZED), {"instructions": 100, "memory_operands": 40}), [])

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


    def test_a_kernel_main_never_calls_because_its_value_folded_is_a_folded_row_not_an_error(self):
        """parity/branch, loop and qlight (bas, bq16) printed the right value with BenchX never called (the call folded to a constant): 6 rows of the m16 -Os
        table read "the region was never entered", an instrument fault listed beside real failures. The output decides: right is folded, wrong is an error."""
        import tempfile
        from unittest import mock

        sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
        import test_sizes

        with tempfile.TemporaryDirectory() as where:
            obj, exe = Path(where) / "p.obj", Path(where) / "p.exe"
            obj.write_bytes(test_sizes.object_with(12))
            exe.write_bytes(b"MZ" + bytes(3000))
            for printed, folded in ((b"1\n", True), (b"2\n", False)):
                run = mock.Mock(error=bench.icount.NEVER_ENTERED, output=printed, region=None)
                with mock.patch.object(bench.maps, "locate", return_value=0), mock.patch.object(bench.icount, "run", return_value=run):
                    got = bench.measure(("p", exe, exe, "f", ["1"], obj))
                self.assertEqual(bench.folded(got), folded)
                self.assertEqual("error" in got, not folded)


class OwnFramesTests(unittest.TestCase):
    def test_a_basic_source_is_measured_once_on_own_frames(self):
        """own frames are the default (fib 14.9 ms on the runtime's B$ENRA/B$EXSA against 5.0): `bas` is that build, with no second language for it."""
        languages = [one.language for one in bench.variants(bench.BENCH / "fib")]
        self.assertEqual(languages.count("bas"), 1)
        self.assertNotIn("basown", languages)

    def test_the_basic_build_asks_for_neither_frame_switch(self):
        import tempfile
        from unittest import mock

        variant = next(one for one in bench.variants(bench.BENCH / "fib") if one.language == "bas")
        with tempfile.TemporaryDirectory() as where, mock.patch.object(bench.subprocess, "run") as run:
            run.return_value = mock.Mock(returncode=1, stderr="", stdout="")
            bench.build(variant, "O2", Path(where), "V000")
        command = run.call_args[0][0]
        self.assertNotIn("--runtime-frames", command)
        self.assertNotIn("--own-frames", command)

    def test_the_basic_build_passes_the_selector_the_run_asks_for(self):
        """`LLRM_ISEL=old` left llrm-qb's command line alone, so an A/B of the selectors on QB compared the default with itself (123 identical rows)."""
        import os
        import tempfile
        from unittest import mock

        variant = next(one for one in bench.variants(bench.BENCH / "fib") if one.language == "bq16")
        with tempfile.TemporaryDirectory() as where, mock.patch.object(bench.subprocess, "run") as run, mock.patch.dict(os.environ, {"LLRM_ISEL": "old"}):
            run.return_value = mock.Mock(returncode=1, stderr="", stdout="")
            bench.build(variant, "O2", Path(where), "V000")
        self.assertIn("--isel=old", run.call_args[0][0])


class StartupTests(unittest.TestCase):
    def test_every_language_has_a_startup_program(self):
        """Nib had no startup/startup.nib: its whole-program time was never netted, so it had no kernel_ms."""
        have = {one.language for one in bench.variants(bench.STARTUP)}
        self.assertEqual(have, {"bas", "bq16", "bq32", "c", "nib"})


def test_every_basic_benchmark_has_a_variant_on_our_runtime_with_the_same_kernel():
    """BASIC on our runtime (bq16: llrm runtime, real mode, jwlink) runs beside the Microsoft-runtime `bas` and needs no QB45."""
    have = {one.language: one for one in bench.variants(bench.BENCH / "fib")}
    assert have["bq16"].region == have["bas"].region
    assert have["bq16"].source == have["bas"].source



class TimedOnlyTests(unittest.TestCase):
    """grep scans 10 MB: the emulator would step ~10^8 instructions, so it is sized and timed, not counted."""

    def test_a_timed_only_program_is_sized_and_never_run_in_the_emulator(self):
        import tempfile
        from unittest import mock

        sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
        import test_sizes

        with tempfile.TemporaryDirectory() as where:
            obj = Path(where) / "p.obj"
            obj.write_bytes(test_sizes.object_with(12))
            with mock.patch.object(bench.icount, "run", side_effect=AssertionError("stepped it")):
                got = bench.measure(("p", obj, obj, None, ["1"], obj))
        self.assertEqual(got, {"code_bytes": 12, "data_bytes": 0, "bss_bytes": 40})

    def test_a_baseline_without_counts_does_not_fail_a_run_without_them(self):
        """against() read got['instructions'] for a measurement that has none: a KeyError, and the gate died."""
        sized = {"code_bytes": 12, "data_bytes": 0, "bss_bytes": 40}
        self.assertEqual(bench.against("grep c -O2", sized, sized | {"kernel_ms": 3.4}), [])

    def test_the_time_of_a_timed_only_program_may_not_rise(self):
        want = {"kernel_ms": 100.0}
        self.assertEqual(bench.time_against("g", {"kernel_ms": 100.5}, want), [])
        self.assertIn("(worse)", bench.time_against("g", {"kernel_ms": 103.0}, want)[0])
        self.assertEqual(bench.time_against("g", {"kernel_ms": 97.0}, want), [])
        self.assertEqual(bench.time_against("g", {}, want), [])  # not run with --time

    def test_sizes_and_time_are_blessed_without_an_instruction_count(self):
        """write_expected read new['instructions'] to decide whether to keep the old time: a KeyError for a timed-only program."""
        import tempfile
        import tomllib

        counts = {"code_bytes": 12, "data_bytes": 0, "bss_bytes": 40, "kernel_ms": 3.4}
        with tempfile.TemporaryDirectory() as where:
            bench.write_expected(Path(where), {("g", "c", "O2"): counts}, "g", "t")
            self.assertEqual(tomllib.loads((Path(where) / "expected.toml").read_text())["c"]["O2"], counts)

    def run_timed(self, text: str, inputs: dict):
        import tempfile
        from unittest import mock

        with tempfile.TemporaryDirectory() as where:
            exe = Path(where) / "p.exe"
            exe.write_bytes(b"MZ")
            with mock.patch.object(bench.dosbatch, "run", return_value={"W000": bench.dosbatch.Result("ok", text)}), mock.patch.object(bench.subprocess, "run", return_value=mock.Mock(returncode=0)):
                return bench.time_programs({("g", "c", "O2"): exe}, Path(where) / "t", inputs)[("g", "c", "O2")]

    def test_a_timed_only_program_that_prints_other_numbers_is_an_error_not_a_time(self):
        """Nothing else checks its output: a scan over a short or wrong file would have been timed and blessed."""
        got = self.run_timed("4745\r\n999\r\nTSC 75000\r\n", {("g", "c", "O2"): {"files": (), "want": ["4745", "200504993"]}})
        self.assertIn("want", got["error"])
        good = self.run_timed("4745\r\n200504993\r\nTSC 75000\r\n", {("g", "c", "O2"): {"files": (), "want": ["4745", "200504993"]}})
        self.assertEqual(good["ms"], 1.0)

    def test_an_unavailable_corpus_skips_the_timing_with_the_reason(self):
        import io
        from contextlib import redirect_stdout

        out = io.StringIO()
        with redirect_stdout(out):
            got = self.run_timed("", {("g", "c", "O2"): {"skip": "dickens is not cached and could not be fetched (offline?)"}})
        self.assertIn("skipped", got["error"])
        self.assertIn("offline", out.getvalue())


class CSourceTests(unittest.TestCase):
    def test_header_lines_are_not_sent_to_the_reference_compilers(self):
        """grep.c opens with `// data: @dickens`: Turbo C 2.01 stopped on it, so TC had no grep row."""
        import tempfile

        with tempfile.TemporaryDirectory() as where:
            source = Path(where) / "g.c"
            source.write_bytes(b"// data: @dickens\n// flags: -O2\n// a comment\nint main(void) { return 0; }\n")
            self.assertEqual(bench.c_source(source), b"// a comment\nint main(void) { return 0; }\n")


def test_reference_builds_find_bench_io_in_the_volume_root(tmp_path):
    """jpeg.c includes "../bench_io.h", which a DOS volume's root cannot name: the Borland builds' copy includes it beside the source, and BC and TC had no row."""
    text = bench.stage_shared_headers(tmp_path, b'#include "../bench_io.h"\nint x;\n')
    assert text == b'#include "bench_io.h"\nint x;\n'
    assert (tmp_path / "bench_io.h").read_bytes() == (bench.BENCH / "bench_io.h").read_bytes()


def test_llrm_c_benches_find_the_os_layer_header(tmp_path):
    """bench_io.h includes llrm_os.h: bench.py built C without -I for it, so jpeg's C variants did not compile."""
    flag, directory = bench.c_include_arguments(tmp_path)
    assert flag == "-I"
    assert (Path(directory) / "llrm_os.h").exists()


def test_without_the_microsoft_install_its_basic_is_skipped_by_name_and_ours_is_not(monkeypatch, tmp_path):
    """The reason names the dialect, the directory and the variable, as the other unavailable tools do; bq16 and bq32 need none."""
    monkeypatch.setitem(bench.run_tests.TOOLS, "qb45", bench.run_tests.TOOLS["qb45"]._replace(mount=tmp_path / "none") if hasattr(bench.run_tests.TOOLS["qb45"], "_replace") else __import__("dataclasses").replace(bench.run_tests.TOOLS["qb45"], mount=tmp_path / "none"))
    have = {one.language: one for one in bench.variants(bench.BENCH / "fib")}
    reason = bench.tools_missing(have["bas"])
    assert "QB45_DIR" in reason and str(tmp_path / "none") in reason
    assert bench.tools_missing(have["bq16"]) is None and bench.tools_missing(have["bq32"]) is None


def test_a_flat_programs_count_is_net_of_the_empty_startup_program(tmp_path):
    """The start-up program prints ` 0` and was compared with `0`: it errored, nothing was subtracted, and every bq32 row held
    DOS/32A's load and the runtime's start-up too (41,766 of sum_three's 43,290 instructions), so a change to the start-up code moved
    every row (+149 on #1591's)."""
    counts = bench.measure_all(["parity/sum_three"], ["O2"], tmp_path)[("parity/sum_three", "bq32", "O2")]
    assert "error" not in counts, counts
    assert counts["instructions"] < 5000, counts


def test_a_real_mode_basic_program_runs_in_the_emulator_with_its_output_checked(tmp_path):
    """Every bq16 program died in the emulator ("int 0x5", KeyError 426 from an INT 21h write) while test_bench stayed green: the neighbouring test runs bq32 only.
    measure() checks the output against the program's expected lines, so a count is a program that printed them."""
    counts = bench.measure_all(["parity/sum_three"], ["O2"], tmp_path)[("parity/sum_three", "bq16", "O2")]
    assert "error" not in counts, counts
    assert 0 < counts["instructions"] < 1000, counts


def test_a_flat_kernels_count_does_not_move_with_work_outside_it(tmp_path):
    """bq32 counted the whole run: DOS/32A's load and the runtime's start-up were in every row (about 41.8k instructions), so work
    outside the kernel moved every row (a function pointer added to the runtime: +165).  What does not move a run: one more
    referenced byte array (its statics add no relocations; that was the first guess and it failed).  What does: work the program
    does outside the kernel, here 200 loop iterations before it.  The kernel's count is the emulator's own for its function."""
    kernel = "FUNCTION Kern%\n  s% = 0\n  FOR i% = 1 TO 100: s% = s% + i%: NEXT\n  Kern% = s%\nEND FUNCTION\n"
    plain = "DECLARE FUNCTION Kern% ()\nPRINT Kern%\nEND\n" + kernel
    more = "DECLARE FUNCTION Kern% ()\nFOR j% = 1 TO 200: t% = t% + j%: NEXT\nPRINT Kern%\nEND\n" + kernel
    objects = bench.qbbuild.runtimes(tmp_path / "runtime")
    flat, whole = {}, {}
    for name, text in (("PLAIN", plain), ("MORE", more)):
        source = tmp_path / f"{name}.BAS"
        source.write_text(text)
        (tmp_path / name).mkdir()
        built = bench.qbbuild.one_target(
            source, tmp_path / name, objects, ("-O2", "-fno-inline-functions", "-fno-inline-functions-called-once", "-g"), bench.qbbuild.FLAT, listing=tmp_path / f"{name}.map", before=("debug", "codeview")
        )
        bench.LOADERS[built.exe] = built.files
        flat[(name, "bq32", "O2")] = (name, built.exe, tmp_path / name / f"{name}.{bench.qbbuild.FLAT}.obj", ["5050"], None, tmp_path / f"{name}.map", "Kern")
        whole[name] = bench.qbbuild.run(built, tmp_path / f"whole{name}", conf=bench.qb32.BENCH_CONF, budget_ms=bench.qb32.BUDGET_MS).instructions
    counts = bench.measure_flat(flat, tmp_path / "flat")
    assert all("instructions" in one for one in counts.values()), counts
    assert whole["MORE"] != whole["PLAIN"], "the experiment moves the whole run"
    assert counts[("MORE", "bq32", "O2")]["instructions"] == counts[("PLAIN", "bq32", "O2")]["instructions"], (counts, whole)


def test_a_kernels_count_does_not_hold_the_code_after_it(tmp_path):
    """The link map gives a function no length, so a kernel that was the last function of its module took in the module's
    main code after it and every call from there into the runtime: an empty kernel (`Kern% = g%`, 2 instructions) beside a
    PRINT in main read 785 (bq32: lru 895 against bq16's 145).  The kernel's length is the image's CodeView procedure
    record."""
    source = tmp_path / "EMPTY.BAS"
    source.write_text(
        "DECLARE FUNCTION Kern% ()\nDIM SHARED g%\ng% = LEN(COMMAND$) + 7\nr% = Kern%\nPRINT r%\nEND\nFUNCTION Kern%\n  Kern% = g%\nEND FUNCTION\n"
    )
    objects = bench.qbbuild.runtimes(tmp_path / "runtime")
    (tmp_path / "b").mkdir()
    built = bench.qbbuild.one_target(
        source, tmp_path / "b", objects, ("-O2", "-fno-inline-functions", "-fno-inline-functions-called-once", "-g"), bench.qbbuild.FLAT, listing=tmp_path / "e.map", before=("debug", "codeview")
    )
    bench.LOADERS[built.exe] = built.files
    flat = {("EMPTY", "bq32", "O2"): ("EMPTY", built.exe, tmp_path / "b" / f"EMPTY.{bench.qbbuild.FLAT}.obj", ["7"], None, tmp_path / "e.map", "Kern")}
    counts = bench.measure_flat(flat, tmp_path / "flat")
    assert counts[("EMPTY", "bq32", "O2")]["instructions"] <= 5, counts


def test_a_file_readers_count_does_not_move_with_what_dos_does(tmp_path):
    """A flat kernel that reads a file runs DOS and the extender inside its window, and their instruction count follows where
    in the file and in memory a read falls: 93,464 of the kernel below were 73,383 of DOS's, and the same program reading from
    another offset moved the number (grep, #1603: -119; stft, jpeg: up to 5).  The kernel's count leaves out what ran in code
    the link map does not name, so the same code reading from several offsets counts the same."""
    data = tmp_path / "DATA.BIN"
    data.write_bytes(bytes(range(256)) * 800)
    flat = {}
    objects = bench.qbbuild.runtimes(tmp_path / "runtime")
    for start in (1, 2, 7, 129, 1001):
        name = f"S{start}"
        source = tmp_path / f"{name}.BAS"
        source.write_text(
            "DECLARE FUNCTION Kern% ()\nPRINT Kern%\nEND\n"
            f'FUNCTION Kern%\n  OPEN "DATA.BIN" FOR BINARY AS #1\n  SEEK #1, {start}\n  buf$ = SPACE$(1000)\n  n% = 0\n'
            "  FOR i& = 1 TO 150\n    GET #1, , buf$\n    n% = n% + 1\n  NEXT\n  CLOSE #1\n  Kern% = n%\nEND FUNCTION\n"
        )
        (tmp_path / name).mkdir()
        built = bench.qbbuild.one_target(
            source, tmp_path / name, objects, ("-O2", "-fno-inline-functions", "-fno-inline-functions-called-once", "-g"), bench.qbbuild.FLAT, listing=tmp_path / f"{name}.map", before=("debug", "codeview")
        )
        bench.LOADERS[built.exe] = built.files
        flat[(name, "bq32", "O2")] = (name, built.exe, tmp_path / name / f"{name}.{bench.qbbuild.FLAT}.obj", ["150"], {"files": (data,)}, tmp_path / f"{name}.map", "Kern")
    counts = bench.measure_flat(flat, tmp_path / "flat")
    ran = bench.qbbuild.run(
        bench.qbbuild.Built(built.exe, built.files), tmp_path / "ran", (data,), bench.qb32.BENCH_CONF, bench.qb32.BUDGET_MS, profile=tmp_path / f"{name}.map"
    )
    assert ran.outside["KERN"] > 3 * ran.inclusive["KERN"], ran
    assert all("instructions" in one for one in counts.values()), counts
    assert len({one["instructions"] for one in counts.values()}) == 1, counts

def test_a_bq32_program_that_runs_long_enough_for_a_timer_tick_at_the_flat_rate_fails():
    """A tick is 55 ms of the emulated clock: at 100,000 instructions a millisecond that was 5.5M instructions, and each of the
    13 to 114 ticks in a long kernel (stft, grep) ran the extender's interrupt handler (~120 instructions, its length by where it
    landed) inside the counted function: a few bytes of code anywhere moved the row by tens (#1603: -40 on stft_single). The check read the
    committed counts; the counts are measured now, so the judge reads them."""
    limit = bench.qb32.BENCH_RATE * 55
    fib = bench.BENCH / "fib"

    def rows(instructions):
        return {("fib", one.language, "O2"): {"instructions": instructions} if one.language == "bq32" else {"skip": "not this test's"} for one in bench.variants(fib)}

    assert not [one for one in bench.judge(rows(limit - 1), ["fib"], ["O2"], {}, set()) if "timer tick" in one]
    assert [one for one in bench.judge(rows(limit), ["fib"], ["O2"], {}, set()) if "timer tick" in one]


def test_the_levels_are_measured_at_once_and_a_failure_at_any_of_them_reaches_the_result(monkeypatch, tmp_path):
    """The levels went one after the other (96 s of 4 x 24 s). Run side by side, an error at one level must still be in the result and
    every level must be there once."""
    from concurrent.futures import ThreadPoolExecutor

    def measure_opt(selected, opt, work, timing=False):
        return {("b", "c", opt): {"error": "boom"} if opt == "O2" else {"instructions": 1}}

    monkeypatch.setattr(bench, "measure_opt", measure_opt)
    monkeypatch.setattr(bench, "ProcessPoolExecutor", ThreadPoolExecutor)  # a process pool cannot carry a patched function
    monkeypatch.setattr(bench, "mark_known", lambda measured, variants, opts: measured)
    monkeypatch.setattr(bench, "benchmarks", lambda selected: [])
    got = bench.measure_all([], ["O1", "O2", "O3", "Os"], tmp_path)
    assert sorted(got) == [("b", "c", o) for o in ("O1", "O2", "O3", "Os")]
    assert got[("b", "c", "O2")] == {"error": "boom"} and got[("b", "c", "O1")] == {"instructions": 1}


def test_a_timed_run_measures_the_levels_one_after_the_other(monkeypatch, tmp_path):
    seen = []
    monkeypatch.setattr(bench, "measure_opt", lambda selected, opt, work, timing=False: seen.append(opt) or {})
    monkeypatch.setattr(bench, "net_times", lambda out: out)
    monkeypatch.setattr(bench, "mark_known", lambda measured, variants, opts: measured)
    monkeypatch.setattr(bench, "benchmarks", lambda selected: [])
    bench.measure_all([], ["O1", "O2"], tmp_path, timing=True)
    assert seen == ["O1", "O2"]


def _main(monkeypatch, capsys, argv, problems, tmp_path):
    monkeypatch.setattr(sys, "argv", ["bench.py", *argv, "--work", str(tmp_path)])
    monkeypatch.setattr(bench, "measure_all", lambda *a, **k: {})
    monkeypatch.setattr(bench, "judge", lambda *a: list(problems))
    monkeypatch.setattr(bench, "base_rows", lambda args: ({}, set(), "0" * 40))
    monkeypatch.setattr(bench, "standing", lambda *a: [])
    monkeypatch.setattr(bench, "benchmarks", lambda selected: [])
    code = bench.main()
    return code, capsys.readouterr().out


WORSE = "b c -O2: instructions 110 > 100 (worse)"


def test_a_worse_count_fails_the_gate_and_nothing_else_does(monkeypatch, capsys, tmp_path):
    code, out = _main(monkeypatch, capsys, [], [WORSE], tmp_path)
    assert code == 1 and f"FAIL {WORSE}" in out
    code, out = _main(monkeypatch, capsys, [], [], tmp_path)
    assert code == 0 and "BETTER" not in out and "0 problems" in out


def test_record_keeps_the_heads_counts_without_measuring_a_base(monkeypatch, capsys, tmp_path):
    """The counts were recorded after the base was measured: a main push saved them 40 minutes in, and five pull requests measured the base alone."""
    def no_base(args):
        raise AssertionError("record measures no base")

    monkeypatch.setattr(bench.basecounts, "CACHE", tmp_path / "cache")
    monkeypatch.setattr(bench, "base_rows", no_base)
    code, out = _main(monkeypatch, capsys, ["--record"], [], tmp_path)
    assert code == 0 and list((tmp_path / "cache").glob("*-own.json")) and "problems" not in out


def shard_file(folder, index, problems=(), measured=None):
    folder.mkdir(parents=True, exist_ok=True)
    data = {"measured": measured or {}, "base": {}, "same": [], "problems": list(problems), "base_sha": "a" * 40}
    (folder / f"shard-{index}.json").write_text(json.dumps(data))


def test_the_merge_names_a_shard_that_left_no_result_and_fails(tmp_path, capsys):
    """A shard that died before writing its file would otherwise leave its rows unjudged and the run green."""
    shard_file(tmp_path, 1)
    shard_file(tmp_path, 3)
    assert bench.merge(tmp_path, 3, ["O2"], False) == 1
    assert "bench shard 2 of 3" in capsys.readouterr().out


def test_the_merge_prints_every_shards_problems_last_and_fails_on_one(tmp_path, capsys):
    shard_file(tmp_path, 1, problems=[WORSE])
    shard_file(tmp_path, 2)
    assert bench.merge(tmp_path, 2, ["O2"], False) == 1
    lines = capsys.readouterr().out.splitlines()
    assert lines[-2] == f"FAIL {WORSE}" and lines[-1].endswith("1 problems")


def test_a_shard_judges_and_writes_only_its_own_slice(monkeypatch, capsys, tmp_path):
    monkeypatch.setattr(bench.shards, "of", lambda index, count: ["fib"])
    monkeypatch.setattr(sys, "argv", ["bench.py", "--shard", "2/3", "--json", str(tmp_path / "out"), "--work", str(tmp_path)])
    monkeypatch.setattr(bench, "measure_all", lambda select, *a, **k: {("fib", "c", "O2"): {"instructions": 5}} if select == ["fib"] else {})
    monkeypatch.setattr(bench, "judge", lambda *a: [])
    monkeypatch.setattr(bench, "base_rows", lambda args, only=None: ({("fib", "c", "O2"): {"instructions": 5}}, {"fib"}, "b" * 40))
    assert bench.main() == 0
    assert json.loads((tmp_path / "out/shard-2.json").read_text())["measured"] == {"fib/c/O2": {"instructions": 5}}


def test_a_bless_records_references_only_and_needs_a_reason():
    import subprocess

    for argv in (["--bless"], ["--bless", "--references"], ["--bless", "--reason", "r"]):
        done = subprocess.run([sys.executable, str(Path(bench.__file__)), *argv], capture_output=True, text=True)
        assert done.returncode == 2 and "--reason" in done.stderr, (argv, done.stderr)


def test_the_gate_fails_with_the_reason_when_the_base_cannot_be_measured(monkeypatch, capsys, tmp_path):
    def broken(args):
        raise SystemExit("measure: the base abc does not build")

    monkeypatch.setattr(bench, "base_rows", broken)
    monkeypatch.setattr(sys, "argv", ["bench.py", "--work", str(tmp_path)])
    monkeypatch.setattr(bench, "measure_all", lambda *a, **k: {})
    assert bench.main() == 1
    assert "FAIL BASE: the base could not be measured: SystemExit: measure: the base abc does not build" in capsys.readouterr().out


def test_two_prs_that_each_change_one_rows_count_are_judged_each_on_its_own_delta(tmp_path):
    """Every PR that moved a count re-blessed bench/NAME/expected.toml, so the second PR to touch a benchmark conflicted with the first (all
    five of a train did, on jpeg_fixed). Now no file holds the count: each PR is measured against the base it branched from."""
    fib = bench.BENCH / "fib"

    def counts(instructions):
        return {("fib", one.language, "O2"): {"instructions": instructions, "memory_operands": 40} if one.language == "c" else {"skip": "no"} for one in bench.variants(fib)}

    base_a, base_b = counts(100), counts(90)  # base_b: after the first PR landed, 100 -> 90
    first, second = counts(90), counts(95)  # the second branched from the old base: 100 -> 95
    judged = lambda measured, base: bench.judge(measured, ["fib"], ["O2"], {key: row for key, row in base.items() if "skip" not in row}, {"fib"})
    assert judged(first, base_a) == [] and judged(second, base_a) == []
    assert "instructions 95 > 90 (worse)" in judged(second, base_b)[0]  # rebased onto the first, it is judged on that base


def _c_build_commands(monkeypatch, tmp_path, name, reads_attribute=True):
    """The llrm-c commands `bench.build` runs for the C benchmark `name` at -O2."""
    variant = next(one for directory in bench.benchmarks([name]) for one in bench.variants(directory) if one.language == "c")
    seen = []
    monkeypatch.setattr(bench, "reads_noinline_attribute", lambda compiler: reads_attribute)

    def run(command, *args, **kwargs):
        seen.append(command)
        Path(command[command.index("-o") + 1]).write_bytes(b"")
        return type("Done", (), {"returncode": 0, "stdout": "", "stderr": ""})()

    monkeypatch.setattr(bench.cpulimit, "run", run)
    monkeypatch.setattr(bench.dosbatch, "link_c", lambda *a, **k: None)
    bench.build(variant, "O2", tmp_path, "x")
    return seen


def test_a_c_kernel_is_built_with_the_inlining_the_m32_table_and_gcc_have(monkeypatch, tmp_path):
    """-fno-inline-functions kept the kernel a call and also kept `rectwo` from inlining itself, as gcc and clang do: its m16 row was
    4.2x gcc's clocks, 2.6x with the inlining on. The run's build calls the kernel through a volatile pointer (vsgcc's wrap.py)."""
    sized, counted = _c_build_commands(monkeypatch, tmp_path, "rectwo")
    assert "-fno-inline-functions" not in sized and "-fno-inline-functions" not in counted
    assert "(*volatile bench_rectwo_via)" in Path(counted[1]).read_text()


def test_the_object_that_is_sized_has_no_pointer_in_it(monkeypatch, tmp_path):
    """The pointer is 18 bytes of code and 4 of data in every C row's object: the sizes are of the kernel `noinline` and `main` as written."""
    sized, counted = _c_build_commands(monkeypatch, tmp_path, "rectwo")
    text = Path(sized[1]).read_text()
    assert "__attribute__((noinline)) long bench_rectwo(" in text and "volatile" not in text
    assert sized[sized.index("-o") + 1].endswith("x.obj") and counted[counted.index("-o") + 1].endswith("x.counted.obj")


def test_a_compiler_that_cannot_read_the_noinline_attribute_gets_the_flag_not_a_failing_source(monkeypatch, tmp_path):
    """The main dist's llrm-c hands C to wccq, which fails `__attribute__((noinline))` as E1169: all 48 C rows of the m16 -Os table were errors on main
    and the C kernels went uncompared. Such a compiler is given the plain source and -fno-inline-functions."""
    (sized,) = _c_build_commands(monkeypatch, tmp_path, "rectwo", reads_attribute=False)
    assert "__attribute__" not in Path(sized[1]).read_text() and "-fno-inline-functions" in sized


def test_the_m32_table_builds_llrm_with_no_inlining_flag_either():
    """The two targets' tables compare the same inlining: build.sh passes llrm no -fno-inline-functions."""
    text = (bench.ROOT / "crates/target/llrm-x86-m32/vsgcc/build.sh").read_text()
    assert [line for line in text.splitlines() if "$LLRM" in line and "-fno-inline-functions" in line] == []


def test_a_pr_that_makes_a_row_worse_than_its_base_fails_and_only_a_waiver_for_that_count_excuses_it(monkeypatch):
    """With no count file left, the base is the only thing a regression can be judged against: it must still fail the step."""
    fib = bench.BENCH / "fib"

    def rows(instructions):
        return {("fib", one.language, "O2"): {"instructions": instructions, "memory_operands": 40} if one.language == "c" else {"skip": "no"} for one in bench.variants(fib)}

    base = {key: row for key, row in rows(100).items() if "skip" not in row}
    monkeypatch.setattr(bench.basecounts, "waivers", lambda root=None: [])
    own = lambda problems: [one for one in problems if "instructions 10" in one and "> 100 (worse)" in one]  # the ratio to a reference is judged too
    assert own(bench.judge(rows(101), ["fib"], ["O2"], base, {"fib"})) == ["fib c -O2: instructions 101 > 100 (worse)"]
    waiver = [{"label": "fib c -O2", "counter": "instructions", "to": 101, "reason": "r", "file": "w.toml"}]
    monkeypatch.setattr(bench.basecounts, "waivers", lambda root=None: waiver)
    assert own(bench.judge(rows(101), ["fib"], ["O2"], base, {"fib"})) == []
    assert own(bench.judge(rows(102), ["fib"], ["O2"], base, {"fib"})) == ["fib c -O2: instructions 102 > 100 (worse)"]


def test_a_waived_count_is_excused_in_the_ratio_to_each_reference_too():
    """#1816 waived matmul c -O2 and still failed `instructions vs ow 0.258x > blessed 0.255x`: the ratio is the count over a constant, so a
    waived count always moved it and a waiver on a row with references could not pass."""
    want, ref = {"instructions": 100}, {"instructions": 400}
    waiver = [{"label": "matmul c -O2", "counter": "instructions", "to": 103, "reason": "r", "file": "w.toml"}]
    assert bench.ratio_problems("matmul c -O2", "ow", {"instructions": 103}, want, ref)
    assert bench.ratio_problems("matmul c -O2", "ow", {"instructions": 103}, want, ref, waiver) == []
    assert bench.ratio_problems("matmul c -O2", "ow", {"instructions": 104}, want, ref, waiver), "another count"
    assert bench.ratio_problems("matmul bas -O2", "ow", {"instructions": 103}, want, ref, waiver), "another row"


def test_lang_keeps_the_other_languages_out_of_the_run(monkeypatch):
    """A BASIC build failing on the head (#2672) stopped the whole bench: the m16 clocks of a C change could not be taken."""
    monkeypatch.setattr(bench, "ONLY", ["c"])
    assert {one.language for one in bench.variants(bench.BENCH / "fib")} == {"c"}
    monkeypatch.setattr(bench, "ONLY", None)
    assert {"c", "bas"} <= {one.language for one in bench.variants(bench.BENCH / "fib")}


def test_a_row_whose_kernel_the_compiler_folded_away_is_marked_and_out_of_the_geomeans(tmp_path, monkeypatch):
    """parity branch, loop, qlight and qmove at bq32 -Os ran, printed the right text and counted 0 instructions: the kernel is a constant. A zero
    is no ratio and no clocks, and the row was silently absent from the table."""
    root = tmp_path
    for name, instructions in (("kept", 10), ("gone", 0)):
        (root / name).mkdir()
        (root / name / "bench.toml").write_text('[region]\nc = "f"\n')
        bench.write_expected(root / name, {(name, "bcc", "O2"): {"instructions": 20, "memory_operands": 4, "code_bytes": 40, "data_bytes": 0, "bss_bytes": 0}}, name, "t")
    measured = {(n, "c", "O2"): {"instructions": i, "memory_operands": 2, "code_bytes": 20, "data_bytes": 0, "bss_bytes": 0} for n, i in (("kept", 10), ("gone", 0))}
    monkeypatch.setattr(bench, "BENCH", root)
    text = "\n".join(bench.standing(measured, [], ["O2"]))
    assert "instructions -50.0% (n=1)" in text and "folded, out of the geomeans: gone" in text, text
    assert bench.clock_lines({("gone", "c", "O2"): {"instructions": 0}}, ["gone"]) == ["gone c -O2: folded (the kernel is never entered)"]
    assert not bench.folded({"error": "run: timeout", "instructions": 0}) and not bench.folded({"instructions": 3})


def test_a_function_marked_by_name_stays_a_call_while_the_runtime_helpers_inline():
    """`-fno-inline=NAME` (DEVIATE 1 of driver/flags.rs): without `-fno-inline-functions`, fib's and frames' kernels inline into main, and the harness
    has no kernel to enter ("region never entered", 112 of 287 rows of the flag table). Marked by name, the kernel is a call and crc's helpers
    (B$FMID, B$FASC) still inline into it."""
    import re
    import subprocess
    import tempfile

    def asm(program: str, *extra: str) -> str:
        with tempfile.TemporaryDirectory() as where:
            out = Path(where) / "p.s"
            done = subprocess.run(
                [str(bench.BIN / "llrm-qb"), str(bench.BENCH / program / f"{program}.bas"), "--dialect", "qb45", "-fqb-runtime=llrm", "-O2", "-finline-functions", "-fno-inline-functions-called-once", *extra, "-S", "-o", str(out)],
                capture_output=True,
                text=True,
            )
            assert done.returncode == 0, done.stderr
            return out.read_text()

    def calls(text: str, name: str) -> int:
        return len(re.findall(rf"call.*\b{name}\b", text))

    assert calls(asm("fib"), "BENCHFIB") == 0, "unmarked, the kernel inlines into main"
    assert calls(asm("fib", "-fno-inline=BenchFib"), "BENCHFIB") == 1
    crc = asm("crc", "-fno-inline=BenchCrc")
    kernel = crc.split("BENCHCRC proc")[1].split("BENCHCRC endp")[0]
    assert calls(crc, "BENCHCRC") == 1 and calls(kernel, r"B\$FMID") + calls(kernel, r"B\$FASC") == 0


def test_a_name_no_function_has_is_an_error_not_a_flag_that_did_nothing():
    import subprocess

    done = subprocess.run(
        [str(bench.BIN / "llrm-qb"), str(bench.BENCH / "fib" / "fib.bas"), "--dialect", "qb45", "-fqb-runtime=llrm", "-fno-inline=NoSuchKernel", "-o", "/dev/null"],
        capture_output=True,
        text=True,
    )
    assert done.returncode != 0 and "defines no function of that name" in done.stderr


def test_a_float_nib_row_is_compiled_with_excess_precision_standard_and_records_it():
    """nbody_single (Nib f32) printed 86015 for 86016 at -O1/-O2 once LoopRotate kept the loop-carried floats in x87 registers (#3218): fast mode allows it, the program's output
    holds with the rounding on. A float row is measured with it, and says so."""
    float_nib = next(v for v in bench.variants(bench.BENCH / "nbody_single") if v.language == "nib")
    assert bench.FLOAT_FLAG in bench.nib_flags(float_nib) and bench.float_flags(float_nib) == [bench.FLOAT_FLAG]
    integer_nib = next(v for v in bench.variants(bench.BENCH / "sieve") if v.language == "nib")
    assert bench.float_flags(integer_nib) == [] and bench.FLOAT_FLAG not in bench.nib_flags(integer_nib)
    row = {"instructions": 1, "memory_operands": 1, "code_bytes": 1, "data_bytes": 0, "bss_bytes": 0}
    measured = {("nbody_single", v.language, "O2"): {"skip": "not under test"} for v in bench.variants(bench.BENCH / "nbody_single")}
    measured[("nbody_single", "nib", "O2")] = row
    problems = bench.judge(measured, ["nbody_single"], ["O2"], base={})
    assert any("records None" in p and "nib -O2" in p for p in problems), problems
    measured[("nbody_single", "nib", "O2")] = {**row, "flags": [bench.FLOAT_FLAG]}
    assert not any("float row" in p for p in bench.judge(measured, ["nbody_single"], ["O2"], base={}))


def test_the_parity_kernels_that_folded_to_a_constant_read_their_inputs_at_run_time():
    """branch, loop, qlight and qmove took literal arguments: the optimiser returned the constant (0 to 2 instructions, a 0.03x "gain" at m16 -Os and
    six "region never entered" rows). Their C inputs are volatile reads and their BASIC inputs READ from DATA, so every build runs the body."""
    for name in ("branch", "loop", "qlight", "qmove"):
        assert "volatile" in (bench.BENCH / "parity" / name / f"{name}.c").read_text()
        assert re.search(r"^read ", (bench.BENCH / "parity" / name / f"{name}.bas").read_text(), re.M)


def test_a_c_bench_row_builds_and_links(tmp_path):
    """llrm-c built C benches that dosbatch then linked for llrm-cc's `main_` (6524193a71): all 48 `c` rows failed with `main_` undefined in start.asm."""
    if not (bench.BIN / "llrm-cc").is_file():
        pytest.skip("no llrm-cc in the dist")
    variant = next(one for one in bench.variants(bench.BENCH / "bintree") if one.language == "c")
    built = bench.build(variant, "Os", tmp_path, "bintree")
    assert not isinstance(built, str), built
