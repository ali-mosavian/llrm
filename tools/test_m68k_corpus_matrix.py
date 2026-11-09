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
    def test_code_bytes_sums_every_code_section_and_no_data(self):
        """Counting `.text` alone dropped main's `.text.startup` (the kernel matrix had the same trap)."""
        self.assertEqual(mx.code_bytes(LISTING), 0x18)

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

    def test_plain_branches_become_relaxing_jumps(self):
        self.assertEqual(mx.relaxing_branches("\tbne\tL1\n\tbsr\tf"), "\tjne\tL1\n\tbsr\tf")


if __name__ == "__main__":
    unittest.main()
