"""`python3 -m unittest discover -s crates/target/llrm-z80/tests/programs`"""

import importlib.util
import re
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("run", HERE / "run.py")
run = importlib.util.module_from_spec(spec)
spec.loader.exec_module(run)


def classified(text):
    with tempfile.TemporaryDirectory() as d:
        source = Path(d) / "p.c"
        source.write_text(text)
        return run.program_flags(source)[1]


class WidthClass(unittest.TestCase):
    def test_a_width_token_in_a_later_alternative_or_line_marks_the_program(self):
        """select_of_constants, shl_of_a_zero_extended_dividend and mulconst_i64 hold x86/m64 `int` and `long` values; the
        classifier read the first alternative of the first line only and reported them WRONG (11-07, three false miscompiles)."""
        self.assertTrue(classified("// flags: -O2 | -O2 -m32\nint main(void){return 0;}\n"))
        self.assertTrue(classified("// flags: -O2\n// flags: -Os -m32\nint main(void){return 0;}\n"))
        self.assertTrue(classified("// flags: -O2\nint main(void){return 0;}\n// x86-widths: long is 64 bits\n"))
        self.assertFalse(classified("// flags: -O2 | -Os\nint main(void){return 0;}\n"))

    def test_a_program_whose_output_exceeds_a_32_bit_long_names_its_width(self):
        """`report(long)` carries 32 bits here: a .out value past them can only come from a 64-bit long, and without a width
        token it reads as a miscompile (mulconst_i64: want 2147483648, got -2147483648). Values that fit but depend on a
        32-bit int (select_of_constants) are not caught here."""
        root = run.ROOT / "tests/run/c"
        sources = sorted(root.glob("*.c"))
        self.assertGreater(len(sources), 100, root)
        unmarked = []
        for source in sources:
            want = run.expected(source.with_suffix(".out")) if source.with_suffix(".out").exists() else None
            if want and not run.program_flags(source)[1] and any(not -(2**31) <= v < 2**31 for v in want):
                unmarked.append(source.name)
        self.assertEqual(unmarked, [], "add a `// x86-widths: WHY` line (or an x86 flags token) to " + ", ".join(unmarked))


if __name__ == "__main__":
    unittest.main()
