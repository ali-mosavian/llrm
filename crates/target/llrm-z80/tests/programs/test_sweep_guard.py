"""`python3 -m unittest discover -s crates/target/llrm-z80/tests/programs`"""

import importlib.util
import unittest
from pathlib import Path

spec = importlib.util.spec_from_file_location("sweep_guard", Path(__file__).resolve().parent / "sweep_guard.py")
guard = importlib.util.module_from_spec(spec)
spec.loader.exec_module(guard)


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
