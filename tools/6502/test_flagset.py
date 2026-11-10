#!/usr/bin/env python3
"""The 6502 gcc progs column ran `-fno-tree-loop-distribute-patterns -fpack-struct=1` and none of the program's `// flags:`
(2026-11-09): 9 programs changed code with the first (ours makes memset/memcpy calls of the same loops), 11 were inlined
where the test says not to. These tests refuse such a cell and pin the real command lines."""
import os, subprocess, sys, tempfile, unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.join(HERE, ".."))
import external_flags, flagset, importlib.util

CORPUS = os.path.join(HERE, "..", "..", "tests", "run", "c")
OLD_GCC = ["-O2", "-fsigned-char", "-fpack-struct=1", "-fno-tree-loop-distribute-patterns", "-w", "-std=gnu17", "-D__INT8_TYPE__=signed char", "-I", "inc", "p.c", "-o", "p.s"]


def ours_argv(path, level="O2"):
    """What ours_cr runs for `path`: its compiler is a script that logs the argument line and fails."""
    with tempfile.TemporaryDirectory() as d:
        cc = os.path.join(d, "cc"); rt = os.path.join(d, "rt.s")
        open(cc, "w").write(f'#!/bin/sh\necho "$@" > {d}/argv\nexit 1\n'); os.chmod(cc, 0o755); open(rt, "w").close()
        env = dict(os.environ, OURS_RT=rt, OURS_PADS="0,37,101,200")
        subprocess.run([sys.executable, os.path.join(HERE, "ours_cr.py"), cc, level, os.path.join(d, "o.tsv"), os.path.basename(path)[:-2]],
                       capture_output=True, env=env, timeout=120, cwd=d)
        return open(os.path.join(d, "argv")).read().split()


def gcc_argv(path, level="-O2"):
    return [level, *flagset.CORPUS_COMMON, *external_flags.gcc_program_flags(flagset.program_flags(path)), "-w", "-std=gnu17", "p.c"]


class Parity(unittest.TestCase):
    def test_the_real_lines_of_a_no_inline_program_agree(self):
        """tail_calls has `-fno-inline-functions`: ours gets it as written, gcc as `-fno-inline`."""
        path = os.path.join(CORPUS, "tail_calls.c")
        ours, gcc = ours_argv(path), gcc_argv(path)
        self.assertIn("-fno-inline-functions", ours)
        self.assertIn("-fno-inline", gcc)
        self.assertEqual(flagset.mismatch(ours, gcc), "")

    def test_the_old_gcc_line_is_refused_for_the_inline_flag(self):
        """11 programs read inlined on the gcc side only."""
        why = flagset.mismatch(ours_argv(os.path.join(CORPUS, "tail_calls.c")), OLD_GCC)
        self.assertIn("-fno-inline-functions", why)

    def test_the_distribute_flag_on_one_side_is_refused(self):
        """9 programs (idioms, memcpy_libcall, pattern_fill ...) compiled to different code with it."""
        path = os.path.join(CORPUS, "idioms.c")
        why = flagset.mismatch(ours_argv(path), gcc_argv(path) + ["-fno-tree-loop-distribute-patterns"])
        self.assertIn("-fno-tree-loop-distribute-patterns", why)

    def test_a_pack_struct_on_one_side_is_refused(self):
        path = os.path.join(CORPUS, "idioms.c")
        self.assertIn("-fpack-struct=1", flagset.mismatch(ours_argv(path), gcc_argv(path) + ["-fpack-struct=1"]))

    def test_a_level_difference_is_refused(self):
        path = os.path.join(CORPUS, "idioms.c")
        self.assertIn("level differs", flagset.mismatch(ours_argv(path, "O2"), gcc_argv(path, "-Os")))

    def test_a_sanitizer_alternative_is_another_build(self):
        """idioms lists `-O2 -fsanitize=bounds` beside its plain builds: the union put bounds checks on both sides' measured code."""
        self.assertEqual(flagset.program_flags(os.path.join(CORPUS, "idioms.c")), [])

    def test_a_program_with_only_sanitizer_builds_keeps_them(self):
        """stackdeep has no plain build; its cell stays n/a for gcc (the flag is clang's)."""
        path = os.path.join(CORPUS, "stackdeep.c")
        self.assertEqual(flagset.program_flags(path), ["-fsanitize=stack"])

    def test_a_clang_only_flag_is_no_gcc_cell(self):
        with self.assertRaises(ValueError):
            external_flags.gcc_program_flags(["-fno-inline=twice"])


class KernelsHeader(unittest.TestCase):
    """gcc_6502_bench.py's table header named `-fno-tree-loop-distribute-patterns`; no cell was compiled with it."""
    def test_the_header_names_the_flags_the_compile_runs_with(self):
        spec = importlib.util.spec_from_file_location("gcc_6502_bench", os.path.join(HERE, "..", "gcc-columns", "gcc_6502_bench.py"))
        bench = importlib.util.module_from_spec(spec); spec.loader.exec_module(bench)
        self.assertNotIn("distribute", bench.header_flags(False))
        self.assertEqual(bench.header_flags(True), "-<level> (+ -fno-ivopts at -Os)")


if __name__ == "__main__":
    unittest.main()
