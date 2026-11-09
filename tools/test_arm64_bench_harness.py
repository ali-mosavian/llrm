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


class Table(unittest.TestCase):
    ROWS = "a\tO2\tours\t10\t100\t{m}\tok\na\tO2\tclang\t10\t90\t{m}\tok\na\tO2\tgcc\t10\t120\t{m}\texception\n"

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
        self.assertIn("# value check: 2 of 3 cells value == .out; 1 of 3 cells: value != .out on all columns", done.stdout)
        self.assertIn("a\tO2\tworse", done.stdout)  # 130 net against clang's 120
        self.assertIn("# O2: better 0, same 0, worse 1", done.stdout)


if __name__ == "__main__":
    unittest.main()
