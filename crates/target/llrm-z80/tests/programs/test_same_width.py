"""`python3 -m unittest discover -s crates/target/llrm-z80/tests/programs`"""

import importlib.util
import unittest
from pathlib import Path

spec = importlib.util.spec_from_file_location("same_width", Path(__file__).resolve().parent / "same_width.py")
sw = importlib.util.module_from_spec(spec)
spec.loader.exec_module(sw)


class SameWidth(unittest.TestCase):
    def test_a_define_alone_leaves_the_double_literals_that_made_float_return_2530_b_too_big(self):
        """The positive control: `#define double float` over `y * 2.0 + x` still holds a double literal, which links the f64 library."""
        self.assertEqual(sw.double_literals("#define double float\ndouble f(double y) { return y * 2.0 + 1e3 - .5; }"), ["2.0", "1e3", ".5"])

    def test_every_floating_literal_is_single_precision_after_the_rewrite(self):
        out = sw.same_width("double f(double y) { return y * 2.0 + 1e3 - .5 + 3.; }")
        self.assertEqual(sw.double_literals(out), [])
        self.assertIn("y * 2.0f + 1e3f - .5f + 3.f", out)
        self.assertTrue(out.startswith("#define double float\n"))

    def test_integers_hex_suffixed_literals_comments_and_strings_stay(self):
        text = 'int a = 10, b = 0x1e3; float c = 1.5f; /* 2.0 */ // 3.0\nconst char *s = "4.0";\n'
        out = sw.same_width(text)
        self.assertEqual(out, "#define double float\n" + text)

    def test_a_literal_the_rewrite_missed_is_refused(self):
        """The guard: a literal form the rewrite does not suffix must stop the run, not reach the sweep as a width-wrong cell."""
        original = sw.double_literals
        sw.double_literals = lambda text: ["2.0"]
        try:
            with self.assertRaises(ValueError):
                sw.same_width("double x = 2.0;")
        finally:
            sw.double_literals = original

    def test_an_f64_helper_in_the_assembly_is_named_and_an_f32_one_is_not(self):
        """float_return -O1 called `__extendsfdf2`, `__adddf3`, `__truncdfsf2` from a source that read as float."""
        self.assertEqual(sw.f64_helpers("\tcall __extendsfdf2\n\tcall __adddf3\n\tcall __adddf3\n\tcall __truncdfsf2\n"), ["__adddf3", "__extendsfdf2", "__truncdfsf2"])
        self.assertEqual(sw.f64_helpers("\tcall __addsf3\n\tcall _report\n"), [])


if __name__ == "__main__":
    unittest.main()
