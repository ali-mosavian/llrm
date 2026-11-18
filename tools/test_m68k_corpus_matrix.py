"""The corpus matrix names every cell it drops: a program with no row is a hole, a failure has a reason, a duplicate file counts once."""
import re
import sys
import tempfile
import unittest
from unittest import mock
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
    def setUp(self):
        # Several tests set LLRM_PIN_CLANG=/x/c and friends; left in os.environ they broke test_m68k_program_size run after them.
        env = mock.patch.dict("os.environ")
        env.start()
        self.addCleanup(env.stop)

    def test_code_bytes_sums_every_code_section(self):
        """Counting `.text` alone dropped main's `.text.startup` (the kernel matrix had the same trap)."""
        self.assertEqual(mx.sections(LISTING)[0], 0x18)

    def test_a_jump_table_in_rodata_is_counted(self):
        """bench/switch: gcc 364 B text + 48 B .rodata, ours 314 + 12: the size column read text alone and hid the table."""
        listing = LISTING + "  3 .rodata       00000030  00000000  00000000  00000050  2**1\n                  CONTENTS, ALLOC, LOAD, READONLY, DATA\n  4 .comment      00000026  00000000\n                  CONTENTS, READONLY\n"
        self.assertEqual(mx.code_bytes(listing), 0x18 + 0x4 + 0x30)

    def test_gcc_keeps_a_static_function_out_of_line(self):
        """frameless.c asks for -fno-inline-functions: gcc inlined all five statics into main (260 B against our 558 B, not like-for-like)."""
        self.assertEqual(mx.gcc_program_flags(["-DX", "-fno-inline-functions"]), ["-DX", "-fno-inline", "-std=gnu17"])

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

    def test_a_clang_only_flag_leaves_the_gcc_column_empty(self):
        """-ffixed-point: gcc read it as `-ffixed-REG`, warned (-w hid it) and built a program without fixed-point; its bytes sat in the gcc column."""
        from unittest import mock
        fake = {name: "/x/" + name for name in ("GCC_M68K", "LLRM_CC", "LLRM_PIN_CLANG", "LLVM_M68K")}
        with tempfile.TemporaryDirectory() as d, mock.patch.dict("os.environ", fake):
            src = Path(d) / "fp.c"
            src.write_text("int main(void) { return 0; }\n")
            row = mx.Tools().build("gcc", "68000", "O2", src, ["-ffixed-point"], Path(d))
        self.assertEqual((row[0], row[1]), (None, "no-gcc-column"))

    def test_the_header_stamps_every_compiler_and_the_dist_commit(self):
        """A table whose ours column came from a stale build was read as a regression: the md5 of each binary and the commit it was built
        from are in the header, and a run without the commit is refused."""
        import os
        with tempfile.TemporaryDirectory() as dist:
            cc = Path(dist) / "llrm-cc"
            cc.write_bytes(b"cc")
            os.environ.update({"GCC_M68K": "/x/g", "LLRM_CC": str(cc), "LLRM_PIN_CLANG": "/x/c", "LLVM_M68K": "/x/l"})
            os.environ.pop("DIST_COMMIT", None)
            with self.assertRaises(SystemExit):
                mx.Tools().stamps()
            os.environ["DIST_COMMIT"] = "abcdef1234"
            line = mx.Tools().stamps()
            self.assertIn("dist commit abcdef1234", line)
            self.assertIn("ours llrm-cc md5 " + __import__("hashlib").md5(b"cc").hexdigest()[:8], line)
            self.assertIn("ours llrm-m68k-llc md5 missing", line)

    def test_the_header_states_the_inline_setting_of_each_column(self):
        """-fno-inline for gcc against the program's own flag for ours and clang: the header says they are the nearest, not the same."""
        import os
        for name in ("GCC_M68K", "LLRM_CC", "LLRM_PIN_CLANG", "LLVM_M68K"):
            os.environ.setdefault(name, "/x/" + name)
        self.assertIn("-fno-inline", mx.Tools().column_lines().split("# inlining:", 1)[1])

    def test_the_header_rate_is_read_from_timings_times_per_cpu(self):
        """The 11-09 table said `2 B per clk saved` for all four CPUs; timings.times gives 0.5, 1.33 and 3.2: every O2/O3 verdict was void."""
        import m68k_worse_cells as wc
        note = mx.rate_note()
        self.assertIn(wc.rate_text(), note)
        self.assertNotIn("68000/68020/68040/68060 2 B", note)
        self.assertEqual({cpu: round(rate, 2) for cpu, rate in wc.rates(note).items()}, {cpu: round(rate, 2) for cpu, rate in wc.committed_rates().items()})

    def test_the_header_prints_every_columns_compile_lines(self):
        """A reader of a table saw only a hash of the setup; the three faults of 11-09 (hard float, rodata, inlining) were in command lines nobody printed."""
        import os
        for name in ("GCC_M68K", "LLRM_CC", "LLRM_PIN_CLANG", "LLVM_M68K"):
            os.environ.setdefault(name, "/x/" + name)
        lines = mx.Tools().column_lines()
        for needle in ("# ours:", "# gcc:", "# clang:", "-msoft-float", "-fomit-frame-pointer", "--fatal-warnings", "-relocation-model=static"):
            self.assertIn(needle, lines)

    def test_the_size_only_runtime_is_not_refused_for_calling_itself(self):
        """The corpus links the compiler-rt archive for bytes and never runs it, but asked for a runnable one: at -O1 the guard refused
        (`__udivsi3 -> udivmod`) and the whole corpus run died after six minutes with no rows."""
        from unittest import mock
        tools = mx.Tools.__new__(mx.Tools)
        tools.cc, tools.root, tools.lock = "/x/llrm-cc", "/x", __import__("threading").Lock()
        with mock.patch.object(mx.sweep, "cache_dir", return_value=Path("/x")), mock.patch.object(mx.sweep, "build_support", return_value=["a", "b", "archive"]) as built:
            self.assertEqual(tools.support("68020", "O1"), "archive")
        self.assertIs(built.call_args.kwargs.get("runnable"), False)

    def test_the_matrix_cpus_are_the_supported_cpus(self):
        """The matrix covered 68000/68020/68040/68060 while the backend supports six: 68010 and 68030 were never built, and 14 cells at 68010
        did not assemble for weeks. The list is read from `isa::Cpu::ALL`, so a CPU cannot drop out of the sweep silently."""
        text = (Path(mx.__file__).resolve().parent.parent / "crates/target/llrm-m68k/src/isa.rs").read_text()
        listed = re.search(r"pub const ALL: \[Cpu; \d+\] = \[(.*?)\];", text).group(1)
        supported = tuple(re.findall(r"M(68\d+)", listed))
        self.assertEqual(mx.CPUS, supported)

    def test_the_ours_object_is_the_shipped_c_object_not_gas_output(self):
        """select_of_constants 68020 -Os read 20948 B through `-S` and gas (`jsr f` as absolute long) against 18532 B shipped: copy_runs and
        idioms 68020 -O2 showed worse than gcc in the matrix and were smaller as shipped objects."""
        import os
        for name in ("GCC_M68K", "LLRM_CC", "LLRM_PIN_CLANG", "LLVM_M68K"):
            os.environ.setdefault(name, "/x/" + name)
        first = mx.Tools().commands("ours", "68020", "Os", "p.c", [], "out")[0]
        self.assertIn("-c", first)
        self.assertNotIn("-S", first)
        self.assertEqual(first[-1], "out.o")

    def test_gas_refusing_our_assembly_is_an_asm_error_and_fails_the_run(self):
        """nbody at 68000 -O1 printed `(260,%sp,%a5)`; gas refused it, the cell was an ordinary compile-fail row and the run exited 0, so the
        broken tip was only seen as a missing `ours` number by the judge."""
        import tempfile
        tools = mx.Tools.__new__(mx.Tools)
        tools.commands = lambda compiler, cpu, level, src, flags, out: [["llrm-cc"], ["llrm-cc-s"], ["as"]]
        tools.lock = __import__("threading").Lock()
        calls = []

        def run(cmd, env):
            calls.append(cmd[0])
            if cmd[0] in ("llrm-cc", "llrm-cc-s"):
                Path(tmp, "x_y.68000.O1.ours.s").write_text("\tnop\n")
                Path(tmp, "x_y.68000.O1.ours.o").write_bytes(b"")
                return True, False, ""
            return False, False, "Error: byte displacement out of range -- statement `move.l (260,%sp,%a5),-(%sp)' ignored"

        tools.run = run
        with tempfile.TemporaryDirectory() as tmp:
            size, reason, cause, _ = tools.build("ours", "68000", "O1", Path("x/y.c"), [], Path(tmp))
        self.assertEqual((size, reason), (None, "asm-error"))
        self.assertIn("byte displacement", cause)
        self.assertEqual(mx.exit_status([], {"asm-error": 2, "ok": 5}), 3)
        self.assertEqual(mx.exit_status([("68000", "O1", "p")], {"asm-error": 2}), 2)
        self.assertEqual(mx.exit_status([], {"ok": 5, "refused": 16}), 0)

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


class EnvironmentIsLeftAlone(unittest.TestCase):
    def test_the_matrix_tests_restore_os_environ(self):
        """`pytest tools/test_m68k_*.py` failed two test_m68k_program_size tests ("llrm-cc: /x/c: No such file"): the corpus tests left
        LLRM_PIN_CLANG=/x/c and three more variables in os.environ, so a later real compile read them."""
        import os
        before = dict(os.environ)
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(CorpusMatrix)
        unittest.TextTestRunner(stream=open(os.devnull, "w")).run(suite)
        self.assertEqual(dict(os.environ), before)
