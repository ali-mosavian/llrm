"""The compile sweep names each failure's kind and says how many cells never ran."""
import sys
import unittest

sys.path.insert(0, __file__.rsplit("/", 1)[0])
import m68k_compile_sweep as sw  # noqa: E402


class CompileSweep(unittest.TestCase):
    def test_a_backend_cannot_select_is_named_not_lumped_with_compile_fail(self):
        """clang column 'LLVM ERROR: Cannot select' and our own look the same in a bare 'failed' count."""
        self.assertEqual(sw.classify("llrm-cc", False, "LLVM ERROR: Cannot select: t18: ch = store"), "cannot-select")

    def test_panic_refusal_timeout_and_assembler_are_distinct(self):
        self.assertEqual(sw.classify("llrm-cc", False, "thread 'main' panicked at x"), "panic")
        self.assertEqual(sw.classify("llrm-cc", False, "llrm-cc: refused: -fsanitize"), "refused")
        self.assertEqual(sw.classify("llrm-cc", True, ""), "timeout")
        self.assertEqual(sw.classify("as", False, "Error: bad instruction"), "assemble-fail")
        self.assertEqual(sw.classify("llrm-cc", False, "error: expected ';'"), "compile-fail")

    def test_the_result_line_counts_untested_cells_and_names_them(self):
        """A far-pointer program was counted as a failure of ours; it is n/a by target and listed apart."""
        line = sw.summary({"ok": 10}, [("m68k-68000 O1", "bench/sum_three/sum_three")] * 6, 10, "stamp")
        self.assertIn("6 cells NOT TESTED", line)
        self.assertIn("bench/sum_three/sum_three", line)


if __name__ == "__main__":
    unittest.main()
