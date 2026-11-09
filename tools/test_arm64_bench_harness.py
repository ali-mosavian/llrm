"""The arm64 bench harness (tools/arm64) runs from the repo, not from a scratch directory."""

import glob
import os
import shutil
import subprocess
import sys
import tempfile
import unittest

HERE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "arm64")
LLVM_BIN = os.environ.get("LLVM_BIN", os.path.expanduser("~/scratch/llvm-build-aarch64/bin"))


class Harness(unittest.TestCase):
    def test_no_file_reads_from_a_personal_scratch_directory(self):
        # The harness lived in ~/scratch/w15-bar: a judge table whose instrument sits there cannot be re-run.
        bad = []
        for path in glob.glob(os.path.join(HERE, "*")):
            if os.path.isfile(path) and "scratch/w15" in open(path, errors="replace").read():
                bad.append(os.path.basename(path))
        self.assertEqual(bad, [])

    def test_one_cell_builds_and_has_kernel_bytes(self):
        tools = [os.path.join(LLVM_BIN, "clang"), os.path.join(LLVM_BIN, "ld.lld")]
        if not all(os.path.exists(t) for t in tools) or not shutil.which("aarch64-linux-gnu-nm"):
            self.skipTest("clang/ld.lld (LLVM_BIN) or aarch64 binutils missing")
        with tempfile.TemporaryDirectory() as out:
            done = subprocess.run(
                ["bash", os.path.join(HERE, "build2.sh"), "fib", "2", "clang"],
                env={**os.environ, "OUT": out, "LLVM_BIN": LLVM_BIN}, capture_output=True, text=True,
            )
            self.assertEqual(done.returncode, 0, done.stderr)
            sys.path.insert(0, HERE)
            import matrix_kb

            self.assertGreater(matrix_kb.kernel_bytes(os.path.join(out, "fib.2.clang.o")), 0)
            self.assertTrue(os.path.exists(os.path.join(out, "fib.2.clang.elf")))

    def test_a_source_reads_bench_io_h_from_the_repo_not_from_a_shared_copy(self):
        # Parallel A/B jobs rewrote one copy of bench_io.h beside the output while others compiled: 2 builds
        # in 40 failed with "call to undeclared function 'bench_read'" (stft_fixed O1, jpeg_fixed O3).
        tools = [os.path.join(LLVM_BIN, "clang"), os.path.join(LLVM_BIN, "ld.lld")]
        if not all(os.path.exists(t) for t in tools) or not shutil.which("aarch64-linux-gnu-nm"):
            self.skipTest("clang/ld.lld (LLVM_BIN) or aarch64 binutils missing")
        with tempfile.TemporaryDirectory() as out:
            done = subprocess.run(
                ["bash", os.path.join(HERE, "build2.sh"), "jpeg_fixed", "2", "clang"],
                env={**os.environ, "OUT": out, "LLVM_BIN": LLVM_BIN}, capture_output=True, text=True,
            )
            self.assertEqual(done.returncode, 0, done.stderr)
            self.assertFalse(os.path.exists(os.path.join(out, "bench_io.h")), "a shared copy of the header")
            self.assertNotIn("../bench_io.h", open(os.path.join(out, "src", "jpeg_fixed.c")).read())


class Table(unittest.TestCase):
    ROWS = "a\tO2\tours\t10\t100\t{m}\tok\t-fomit-frame-pointer\na\tO2\tclang\t10\t90\t{m}\tok\t-fomit-frame-pointer\na\tO2\tgcc\t10\t120\t{m}\texception\t-fomit-frame-pointer\n"

    def run_table(self, rows_metric, flag):
        with tempfile.NamedTemporaryFile("w", suffix=".tsv", delete=False) as rows:
            rows.write(self.ROWS.format(m=rows_metric))
        try:
            return subprocess.run(
                [sys.executable, os.path.join(HERE, "table.py"), rows.name, "--metric", flag, "--count-commit", "abc123"], capture_output=True, text=True
            )
        finally:
            os.unlink(rows.name)

    def test_rows_counted_whole_are_refused_under_a_kernel_header(self):
        # A table headed "kernel" held whole-program counts (count.py without the kernel flag).
        done = self.run_table("whole", "kernel")
        self.assertNotEqual(done.returncode, 0)
        self.assertIn("counted as 'whole'", done.stderr)

    def test_matching_metric_gives_verdicts_and_a_header_naming_it(self):
        done = self.run_table("kernel", "kernel")
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertIn("# instrs metric: kernel (count.py --kernel; range [first function, report) minus main); count.py commit abc123", done.stdout)
        self.assertIn("# value check: 2 of 3 measured cells value == .out; 1 of 3: value != .out on all columns", done.stdout)
        self.assertIn("a\tO2\tworse", done.stdout)  # 130 net against clang's 120
        self.assertIn("# O2: better 0, same 0, worse 1", done.stdout)


    def test_an_unmeasured_cell_is_a_hard_line_not_a_gap(self):
        # A build that failed once left its cell out of the table: no row, no verdict, no trace.
        rows = (
            "a\tO2\tours\t10\t100\tkernel\tok\tF\na\tO2\tclang\t10\t90\tkernel\tok\tF\na\tO2\tgcc\t\t\tkernel\tunmeasured:build failed\t\n"
            "b\tO2\tours\t10\t100\tkernel\tok\tF\nb\tO2\tclang\t10\t90\tkernel\tok\tF\nb\tO2\tgcc\t10\t120\tkernel\tok\tF\n"
        )
        with tempfile.NamedTemporaryFile("w", suffix=".tsv", delete=False) as handle:
            handle.write(rows)
        try:
            done = subprocess.run(
                [sys.executable, os.path.join(HERE, "table.py"), handle.name, "--metric", "kernel", "--count-commit", "abc"],
                capture_output=True, text=True,
            )
        finally:
            os.unlink(handle.name)
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertIn("# UNMEASURED: 1 of 6 cells (listed below", done.stdout)
        self.assertIn("a\tO2\tUNMEASURED\tgcc\tunmeasured:build failed", done.stdout)
        self.assertNotIn("a\tO2\tworse", done.stdout)
        self.assertIn("b\tO2\tworse", done.stdout)

    def table(self, rows):
        with tempfile.NamedTemporaryFile("w", suffix=".tsv", delete=False) as handle:
            handle.write(rows)
        try:
            return subprocess.run(
                [sys.executable, os.path.join(HERE, "table.py"), handle.name, "--metric", "kernel", "--count-commit", "abc"], capture_output=True, text=True
            )
        finally:
            os.unlink(handle.name)

    def test_ours_built_without_the_flags_the_bar_has_is_refused(self):
        # 2026-11-09: "ours" had no -fomit-frame-pointer, the bar had it: 24 of 72 cells read 4-16 B worse than they are.
        done = self.table(
            "a\tO2\tours\t10\t100\tkernel\tok\t-fno-pic\na\tO2\tclang\t10\t90\tkernel\tok\t-fno-pic -fomit-frame-pointer\n"
            "a\tO2\tgcc\t10\t120\tkernel\tok\t-fno-pic -fomit-frame-pointer\n"
        )
        self.assertNotEqual(done.returncode, 0)
        self.assertIn("a O2: ours vs clang: -fomit-frame-pointer", done.stderr)

    def test_a_column_with_no_recorded_flag_set_is_refused(self):
        done = self.table("a\tO2\tours\t10\t100\tkernel\tok\tunknown\na\tO2\tclang\t10\t90\tkernel\tok\tunknown\n")
        self.assertNotEqual(done.returncode, 0)
        self.assertIn("no recorded flag set", done.stderr)

    def test_a_row_without_the_flag_field_is_refused(self):
        done = self.table("a\tO2\tours\t10\t100\tkernel\tok\n")
        self.assertNotEqual(done.returncode, 0)
        self.assertIn("a row has 8", done.stderr)


class FlagSet(unittest.TestCase):
    def test_the_neutral_set_is_the_same_for_gcc_clang_and_ours_whatever_the_spelling(self):
        sys.path.insert(0, HERE)
        import flagset

        neutral = "-fno-pic -fno-pie -mbranch-protection=none -fomit-frame-pointer -fno-stack-protector"
        gcc = neutral.split() + ["-fzero-init-padding-bits=standard", "-fno-tree-vectorize", "-O2", "-I", "x", "-DX=1", "-ffreestanding"]
        ours = neutral.split() + ["-target", "aarch64-unknown-linux-gnu", "-O2", "-ffreestanding"]
        self.assertEqual(flagset.canonical(gcc), flagset.canonical(ours + ["-fno-vectorize"]))

    def test_the_omitted_frame_pointer_is_a_difference(self):
        sys.path.insert(0, HERE)
        import flagset

        self.assertNotEqual(flagset.canonical(["-fno-pic"]), flagset.canonical(["-fno-pic", "-fomit-frame-pointer"]))


if __name__ == "__main__":
    unittest.main()
