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

    def test_prepare_rewrites_only_the_sources_that_name_double(self):
        """--same-width must leave an integer program byte for byte (its cell would otherwise move) and give a float program the same-width copy."""
        import tempfile
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            (tmp / "ints.c").write_text("int f(void) { return 2; }\n")
            (tmp / "floats.c").write_text("double f(double y) { return y * 2.0; }\n")
            (tmp / "floats.out").write_text("4\n")
            made = sw.prepare([tmp / "ints.c", tmp / "floats.c"], tmp / "out")
            self.assertEqual(made[0], tmp / "ints.c")
            self.assertEqual(made[1], tmp / "out" / "floats.c")
            self.assertEqual(sw.double_literals(made[1].read_text()), [])
            self.assertEqual((tmp / "out" / "floats.out").read_text(), "4\n")

    def test_a_long_double_program_is_not_rewritten_and_a_wrong_f32_output_is_width_void(self):
        """--same-width over the corpus failed 11 programs: `long double` became `long float` (CLANG error), and values that
        depend on f64 (double_to_int64, float_bits) read WRONG at f32. Neither is a lost program."""
        import tempfile
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            (tmp / "ld.c").write_text("long double f(double y) { return y; }\n")
            self.assertEqual(sw.prepare([tmp / "ld.c"], tmp / "out"), [tmp / "ld.c"])
        reasons = {"a": {"ours": "WRONG value 0: got 1, want 2"}, "b": {"ours": "CLANG error"}, "c": {}}
        self.assertEqual(sw.width_void({"a", "b", "c"}, reasons), {"a"})


if __name__ == "__main__":
    unittest.main()
