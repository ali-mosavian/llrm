"""`python3 -m unittest discover -s crates/target/llrm-z80/tests/programs`"""

import importlib.util
import tempfile
import unittest
from pathlib import Path

run_spec = importlib.util.spec_from_file_location("run", Path(__file__).resolve().parent / "run.py")
run = importlib.util.module_from_spec(run_spec)
run_spec.loader.exec_module(run)
spec = importlib.util.spec_from_file_location("sweep_guard", Path(__file__).resolve().parent / "sweep_guard.py")
guard = importlib.util.module_from_spec(spec)
spec.loader.exec_module(guard)


class CompileLimit(unittest.TestCase):
    def test_a_compile_that_waits_past_the_limit_without_computing_is_not_slow(self):
        """fixed_point -O3 read `SLOW llc over 120 s` for a 3 s compile: the load guard stopped the sweep's process group and the
        wall clock ran. The limit is CPU time; a process that sleeps 3 s on a 1 s limit finished."""
        done = run.llc_limited(["sleep", "3"], ".", 1)
        self.assertEqual(done.returncode, 0)

    def test_a_compile_that_computes_past_the_limit_is_stopped_and_named(self):
        done = run.llc_limited(["python3", "-c", "while True: pass"], ".", 1)
        self.assertEqual(done.returncode, run.CPU_LIMIT)


class NotABar(unittest.TestCase):
    def test_the_bitfield_row_names_its_twin_and_the_twin_has_a_bar(self):
        """volatile_bitfield read +223 B against SDCC for a struct SDCC lays out differently (`f:15` at bit 3 under clang, at a
        byte under SDCC): an instrument mismatch, not a code gap. Its SDCC columns are not a bar; the byte-aligned twin is."""
        corpus = run.ROOT / "tests" / "run" / "c"
        why = guard.incomparable((corpus / "volatile_bitfield.c").read_text())
        self.assertIn("twin volatile_bitfield_aligned", why or "")
        self.assertTrue((corpus / "volatile_bitfield_aligned.c").exists())
        self.assertIsNone(guard.incomparable((corpus / "volatile_bitfield_aligned.c").read_text()))


class RowCount(unittest.TestCase):
    def test_a_program_with_no_row_fails_the_run_and_is_named(self):
        """The sweep printed `112 programs 0 failed` with 3 rows missing (2026-11-09): a program that left neither an
        `ours` row nor a FAILED row was not counted."""
        results = [("a", [("ours", 10, 20)]), ("b", []), ("c", [("sdcc-size", 9, 30)])]
        line, ok = guard.summary(["a", "b", "c"], results)
        self.assertFalse(ok)
        self.assertEqual(line, "3 programs 2 failed; rows 1 of 3; no row: b(NOROW) c(NOROW)")

    def test_a_program_the_pool_never_returned_is_dropped(self):
        line, ok = guard.summary(["a", "b"], [("a", [("ours", 1, 1)])])
        self.assertFalse(ok)
        self.assertIn("b(NOROW)", line)

    def test_every_row_present_passes(self):
        line, ok = guard.summary(["a", "b"], [("a", [("ours", 1, 1)]), ("b", [("ours", 2, 2), ("sdcc-size", 3, 3)])])
        self.assertTrue(ok)
        self.assertEqual(line, "2 programs 0 failed; rows 2 of 2")

    def test_a_recorded_reason_excuses_a_missing_sdcc_row_but_not_a_silent_one(self):
        """A program SDCC rejects has a reason and no bar; a program with neither row nor reason is a dropped row."""
        sdcc = lambda tag: tag.startswith("sdcc")
        results = [("a", []), ("b", [])]
        line, ok = guard.summary(["a", "b"], results, {}, sdcc, excused={"a"})
        self.assertFalse(ok)
        self.assertEqual(line, "2 programs 1 failed; rows 0 of 2; no row: b(NOROW)")

    def test_the_sweeps_own_reason_replaces_the_default(self):
        line, ok = guard.summary(["a"], [("a", [("FAILED", 0, 0)])], {"a": "LINKFAIL"})
        self.assertFalse(ok)
        self.assertIn("a(LINKFAIL)", line)


class Verdict(unittest.TestCase):
    def test_a_wrong_output_names_the_first_value_that_differs(self):
        """`WRONG` alone left 11 SDCC cells unreadable: a 32-bit `double` or a missing `long long` looks like a miscompile."""
        sweep_spec = importlib.util.spec_from_file_location("sweep_under_test", Path(__file__).resolve().parent / "sweep.py")
        sweep = importlib.util.module_from_spec(sweep_spec)
        sweep_spec.loader.exec_module(sweep)
        with self.assertRaises(sweep.Void) as raised:
            sweep.verdict([1, 2, 9], [1, 2, 3])
        self.assertEqual(str(raised.exception), "WRONG value 2: got 9, want 3")
        with self.assertRaises(sweep.Void) as raised:
            sweep.verdict([1], [1, 2])
        self.assertEqual(str(raised.exception), "WRONG value 1: got none, want 2")
        sweep.verdict([1, 2], [1, 2])


class Preinclude(unittest.TestCase):
    def test_sdcc_gets_the_type_macros_its_preprocessor_lacks(self):
        """`typedef __INT32_TYPE__ s32;` made 5 programs read as "SDCC cannot compile this" (no bar) when SDCC has no
        such predefined macro; the sweep now hands SDCC clang's type, size and byte-order macros."""
        import os, shutil, tempfile
        clang = os.environ.get("LLRM_PIN_CLANG", os.path.expanduser("~/scratch/llvm-build/bin/clang"))
        if not os.path.exists(clang):
            self.skipTest("no pin clang")
        sweep_spec = importlib.util.spec_from_file_location("sweep_pre", Path(__file__).resolve().parent / "sweep.py")
        sweep = importlib.util.module_from_spec(sweep_spec)
        sweep_spec.loader.exec_module(sweep)
        d = Path(tempfile.mkdtemp())
        try:
            flags = sweep.sdcc_preinclude(clang, d / "predefined.h")
            text = (d / "predefined.h").read_text()
        finally:
            shutil.rmtree(d, True)
        for macro in ("__INT32_TYPE__", "__SIZE_TYPE__", "__BYTE_ORDER__", "__INT_MAX__"):
            self.assertIn(f"#define {macro} ", text)
        self.assertTrue(flags[0].startswith("-Wp-include,"))
        self.assertLess(len([f for f in flags if f.startswith("-D")]), 10)  # sdcc aborts past ~90 -D flags


class Branches(unittest.TestCase):
    def load(self):
        spec = importlib.util.spec_from_file_location("sweep_branch", Path(__file__).resolve().parent / "sweep.py")
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module

    def test_sdcc_sees_the_macros_that_pick_the_branch_ours_takes(self):
        """variadic has `#ifdef __clang__`: ours built that branch, SDCC (no such macro) built `#else`, a different program,
        and the bar read 547 B against 331 B for code that was not the same."""
        sweep = self.load()
        source = "#ifdef __clang__\nint a;\n#else\nint b;\n#endif\n#ifdef REAL_MODE\n#endif\n/* __INT32_TYPE__ in a comment */\n"
        defined = {"__clang__": "1", "__INT32_TYPE__": "long", "__GNUC__": "4"}
        self.assertEqual(sweep.branch_macros(source, defined), {"__clang__": "1"})  # REAL_MODE is nobody's macro; a comment is no test

    def test_a_program_without_a_conditional_gets_no_header(self):
        import tempfile
        sweep = self.load()
        with tempfile.TemporaryDirectory() as d:
            self.assertEqual(sweep.branch_header("int x;\n", {"__clang__": "1"}, Path(d) / "b.h"), [])
            flags = sweep.branch_header("#if defined(__clang__) && __BYTE_ORDER__ == 1\n#endif\n",
                                        {"__clang__": "1", "__BYTE_ORDER__": "1234"}, Path(d) / "b.h")
            text = (Path(d) / "b.h").read_text()
        self.assertTrue(flags[0].startswith("-Wp-include,"))
        self.assertIn("#define __clang__ 1", text)
        self.assertIn("#define __BYTE_ORDER__ 1234", text)


class Imports(unittest.TestCase):
    def test_measure_is_importable_without_running_its_command_line(self):
        """`tests/kernels/measure.py` called `main()` at import, so the sweep could not load its helpers and exec'd a split
        copy of the source (`--help` printed the kernel tool's usage)."""
        import sys
        here = Path(__file__).resolve().parent
        spec = importlib.util.spec_from_file_location("measure_under_test", here.parent / "kernels" / "measure.py")
        module = importlib.util.module_from_spec(spec)
        argv, sys.argv = sys.argv, ["measure.py"]  # the kernel tool exits when its required arguments are missing
        try:
            spec.loader.exec_module(module)
        finally:
            sys.argv = argv
        self.assertTrue(callable(module.runtime_bytes))


if __name__ == "__main__":
    unittest.main()


class RuntimeStamp(unittest.TestCase):
    def fake_llc(self, stamp):
        import os, stat, tempfile
        path = Path(tempfile.mkdtemp()) / "llc"
        path.write_text(f"#!/bin/sh\n[ \"$1\" = --runtime-stamp ] && echo {stamp} && exit 0\nexit 2\n")
        path.chmod(path.stat().st_mode | stat.S_IEXEC)
        return str(path)

    def test_a_runtime_the_compiler_was_not_built_with_is_named(self):
        """A divide fix compared as two compilers on one working-tree runtime showed 0 diff (2026-11-12): the sweep links the file,
        not the compiler's copy. A compiler whose stamp is not the file's is refused."""
        stamp = run.fnv1a64(run.RUNTIME.read_bytes())
        self.assertIsNone(run.runtime_mismatch(self.fake_llc(stamp), run.RUNTIME))
        why = run.runtime_mismatch(self.fake_llc("0123456789abcdef"), run.RUNTIME)
        self.assertIn("0123456789abcdef", why or "")
        self.assertIn(stamp, why or "")

    def test_a_compiler_that_reports_no_stamp_is_refused_not_trusted(self):
        why = run.runtime_mismatch(self.fake_llc(""), run.RUNTIME)
        self.assertIsNotNone(why)

    def test_the_stamp_is_fnv1a_64(self):
        self.assertEqual(run.fnv1a64(b""), "cbf29ce484222325")
        self.assertEqual(run.fnv1a64(b"a"), "af63dc4c8601ec8c")


class SweepInstrument(unittest.TestCase):
    def sweep(self):
        spec = importlib.util.spec_from_file_location("sweep_instrument", Path(__file__).resolve().parent / "sweep.py")
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module

    def test_a_sweep_without_the_soft_float_library_is_refused(self):
        """The owner sweep of 2026-11-12 ran without `Z80_LIBC_EXTRA`: 13 float programs failed to link and read as 'no row'. A run
        that needs the library and has none is refused, with the way to build it; `--sdcc-only` and `--no-softfloat` opt out."""
        why = guard.softfloat_library_missing({}, sdcc_only=False, no_softfloat=False)
        self.assertIn("z80_lib.sh", why or "")
        self.assertIsNone(guard.softfloat_library_missing({"Z80_LIBC_EXTRA": "/lib"}, False, False))
        self.assertIsNone(guard.softfloat_library_missing({}, sdcc_only=True, no_softfloat=False))
        self.assertIsNone(guard.softfloat_library_missing({}, sdcc_only=False, no_softfloat=True))

    def test_the_sdcc_compile_gets_the_timeout_the_run_asked_for(self):
        """div_constants_long's `-ma` variants (--max-allocs-per-node 200000) need over 300 s: the fixed limit dropped 2 of 4 SDCC
        columns of that cell (and 1 of div_constants_ulong and long_shift_by_a_call_result) and the best-bar read from the rest."""
        import subprocess
        from types import SimpleNamespace
        from unittest import mock

        sweep = self.sweep()
        seen = []

        def timed_out(command, cwd, input=None, timeout=None):
            seen.append(timeout)
            raise subprocess.TimeoutExpired(command, timeout)

        ctx = SimpleNamespace(sdcc_timeout=3600, defines=[], predefined={})
        with mock.patch.object(sweep.run, "sh", timed_out), mock.patch.object(sweep, "assemble"), mock.patch.object(
            sweep, "branch_header", return_value=[]
        ):
            with tempfile.TemporaryDirectory() as work:
                source = Path(work) / "p.c"
                source.write_text("int main(void) { return 0; }\n")
                with self.assertRaises(sweep.Void):
                    sweep.sdcc_cell(source, Path(work), "sdcc-x", [], ctx, [])
        self.assertEqual(seen, [3600])


class LibraryChecks(unittest.TestCase):
    def test_a_failing_library_script_fails_the_sweep_and_is_named(self):
        """f64 divide gave 3.0/1.0 = 4 for a day (2026-11-13): tests/z80/f64_div_clk.sh failed, and no runner ran it."""
        with tempfile.TemporaryDirectory() as d:
            for name, body in (("good.sh", "exit 0"), ("bad.sh", "echo 'f64 div: xor differs'; exit 1")):
                Path(d, name).write_text(body)
            failed = guard.failed_library_checks(d, "llc", checks=("good.sh", "bad.sh"))
        self.assertEqual(failed, {"bad.sh": "f64 div: xor differs"})

    def test_the_run_checks_are_the_three_scripts_that_hid_it(self):
        self.assertEqual([Path(s).name for s in guard.LIBRARY_CHECKS], ["f64_div_clk.sh", "f64_mul_clk.sh", "z80_f64_split_test.sh"])
