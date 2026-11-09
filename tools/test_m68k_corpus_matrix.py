"""The corpus matrix names every cell it drops: a program with no row is a hole, a failure has a reason, a duplicate file counts once."""
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, __file__.rsplit("/", 1)[0])
import m68k_corpus_matrix as mx  # noqa: E402

LISTING = """Sections:
Idx Name          Size      VMA       LMA       File off  Algn
  0 .text         00000010  00000000  00000000  00000034  2**2
                  CONTENTS, ALLOC, LOAD, READONLY, CODE
  1 .data         00000004  00000000  00000000  00000044  2**2
                  CONTENTS, ALLOC, LOAD, DATA
  2 .text.startup 00000008  00000000  00000000  00000048  2**1
                  CONTENTS, ALLOC, LOAD, READONLY, CODE
"""


class CorpusMatrix(unittest.TestCase):
    def test_code_bytes_sums_every_code_section(self):
        """Counting `.text` alone dropped main's `.text.startup` (the kernel matrix had the same trap)."""
        self.assertEqual(mx.sections(LISTING)[0], 0x18)

    def test_a_jump_table_in_rodata_is_counted(self):
        """bench/switch: gcc 364 B text + 48 B .rodata, ours 314 + 12: the size column read text alone and hid the table."""
        listing = LISTING + "  3 .rodata       00000030  00000000  00000000  00000050  2**1\n                  CONTENTS, ALLOC, LOAD, READONLY, DATA\n  4 .comment      00000026  00000000\n                  CONTENTS, READONLY\n"
        self.assertEqual(mx.code_bytes(listing), 0x18 + 0x4 + 0x30)

    def test_gcc_keeps_a_static_function_out_of_line(self):
        """frameless.c asks for -fno-inline-functions: gcc inlined all five statics into main (260 B against our 558 B, not like-for-like)."""
        self.assertEqual(mx.gcc_program_flags(["-DX", "-fno-inline-functions"]), ["-DX", "-fno-inline"])

    def test_gcc_compiles_the_branch_of_a_source_that_tests_clang(self):
        """variadic.c: `#ifdef __clang__` is the real stdarg walk, the rest a macro walk. gcc built the macro branch (178 B at 68000 -Os);
        ours and clang built the stdarg one (gcc with it: 276 B), so the gcc column was a different program."""
        import os
        for name in ("GCC_M68K", "LLRM_CC", "LLRM_PIN_CLANG", "LLVM_M68K"):
            os.environ.setdefault(name, "/x/" + name)
        tools = mx.Tools()
        source = str(Path(__file__).resolve().parent.parent / "tests/run/c/variadic.c")
        self.assertIn("-D__clang__", tools.commands("gcc", "68000", "Os", source, [], "/tmp/o")[0])
        plain = str(Path(__file__).resolve().parent.parent / "tests/run/c/bit_test.c")
        self.assertNotIn("-D__clang__", tools.commands("gcc", "68000", "Os", plain, [], "/tmp/o")[0])

    def test_the_header_prints_every_columns_compile_lines(self):
        """A reader of a table saw only a hash of the setup; the three faults of 11-09 (hard float, rodata, inlining) were in command lines nobody printed."""
        import os
        for name in ("GCC_M68K", "LLRM_CC", "LLRM_PIN_CLANG", "LLVM_M68K"):
            os.environ.setdefault(name, "/x/" + name)
        lines = mx.Tools().column_lines()
        for needle in ("# ours:", "# gcc:", "# clang:", "-msoft-float", "-fomit-frame-pointer", "--fatal-warnings", "-relocation-model=static"):
            self.assertIn(needle, lines)

    def test_a_helper_the_archive_lacks_is_named(self):
        """gcc's 96-bit long double calls __addxf3, which our compiler-rt does not define: ld --unresolved-symbols=ignore-all drops it from
        the executable, so `nm -u exe` listed nothing, the library read 0 B and whole-program 'worse' was 43 KB for three programs."""
        import os, subprocess, tempfile
        root = os.environ.get("GCC_M68K")
        if not root:
            self.skipTest("GCC_M68K unset: needs the unpacked m68k gcc")
        for name in ("LLRM_CC", "LLRM_PIN_CLANG", "LLVM_M68K"):
            os.environ.setdefault(name, "/x/" + name)
        tools = mx.Tools()
        with tempfile.TemporaryDirectory() as tmp:
            src = os.path.join(tmp, "a.c")
            open(src, "w").write("extern long double __addxf3(long double, long double);\nlong double f(long double a, long double b) { return __addxf3(a, b); }\n")
            obj = os.path.join(tmp, "a.o")
            subprocess.run([tools.gcc, f"-B{root}/usr/lib/gcc-cross/m68k-linux-gnu/15/", "-mcpu=68020", "-msoft-float", "-O1", "-c", src, "-o", obj], check=True, env=tools.env)
            empty = os.path.join(tmp, "empty.a")
            subprocess.run([f"{root}/usr/bin/m68k-linux-gnu-ar", "rcs", empty], check=True, env=tools.env)
            tools.support = lambda cpu, level: empty  # an archive without the helper
            library, unresolved = tools.library("68020", "O1", Path(obj), Path(tmp))
            self.assertEqual((library, unresolved), (0, "__addxf3"))

    def test_a_long_double_program_has_no_gcc_reference(self):
        """sizeof(long double) is 12 for gcc and 8 for the pin and ours: long_double_layout read ours 43 KB worse in whole-program size
        (gcc's 96-bit helpers are not in our archive). A comment that says it does not count."""
        self.assertTrue(mx.long_double_abi_mismatch("long double x;"))
        self.assertFalse(mx.long_double_abi_mismatch("// long double is wide\nint x;"))
        self.assertFalse(mx.long_double_abi_mismatch("double x;"))

    def test_gcc_is_built_soft_float(self):
        """gcc defaults to 68881 hardware floating point at 68020/040/060 and ours has no FPU: nbody was 578 B for gcc against 946 B soft."""
        self.assertIn("-msoft-float", mx.GCC_FLOAT)

    def test_a_program_with_no_ours_row_is_a_hole(self):
        """The 22-kernel whitelist dropped 208 of 230 programs without a word: a denominator program with no row is refused."""
        have = {("m68k-68000", "run/a", "O1")}
        self.assertEqual(mx.holes(["run/a", "run/b"], ["68000"], ["O1"], have), [("68000", "O1", "run/b")])

    def test_ours_built_and_no_reference_is_not_ok(self):
        rows = [("ours", 10, "ok", ""), ("gcc", None, "compile-fail", "x"), ("clang", None, "compile-fail", "y")]
        self.assertEqual(mx.verdict(rows), "no-reference")

    def test_one_reference_is_enough(self):
        rows = [("ours", 10, "ok", ""), ("gcc", None, "compile-fail", "x"), ("clang", 9, "ok", "")]
        self.assertEqual(mx.verdict(rows), "ok")

    def test_ours_failure_names_its_reason(self):
        rows = [("ours", None, "refused", "ours: refused"), ("gcc", 9, "ok", ""), ("clang", 9, "ok", "")]
        self.assertEqual(mx.verdict(rows), "refused")

    def test_failure_reasons(self):
        self.assertEqual(mx.failure("ours", True, "")[0], "timeout")
        self.assertEqual(mx.failure("ours", False, "llrm-cc: refused: x")[0], "refused")
        self.assertEqual(mx.failure("gcc", False, "refused")[0], "compile-fail")

    def test_the_same_file_in_run_and_bench_counts_once(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "run").mkdir()
            (root / "bench/x").mkdir(parents=True)
            (root / "bench/x/bench.toml").write_text("")
            (root / "run/a.c").write_text("int a;\n")
            (root / "bench/x/a.c").write_text("int a;\n")
            kept, same = mx.programs(root / "run", root / "bench")
            self.assertEqual([i for i, _ in kept], ["run/a"])
            self.assertEqual(same, [("run/a", "bench/x/a")])

    def test_rewritten_listing_ends_in_a_newline(self):
        """Without it gas warned 'end of file not at end of a line' on every ours cell; a warning on our output can hide a bad line."""
        self.assertTrue(mx.relaxing_branches("\trts\n").endswith("\n"))

    def test_plain_branches_become_relaxing_jumps(self):
        self.assertEqual(mx.relaxing_branches("\tbne\tL1\n\tbsr\tf"), "\tjne\tL1\n\tbsr\tf\n")


if __name__ == "__main__":
    unittest.main()
