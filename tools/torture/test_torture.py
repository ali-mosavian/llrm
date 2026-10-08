"""`uv run --project tools python -m unittest discover -s tools/torture`"""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import torture  # noqa: E402


class CauseTests(unittest.TestCase):
    def test_a_front_end_error_is_counted_by_what_it_says_not_where_or_of_what(self):
        text = "llrm-c: wccq failed on /x/y.c:\n/x/y.c(10): Warning! W131: No prototype found for function 'f'\n/x/y.c(12): Error! E1054: Missing operand\n"
        self.assertEqual(torture.cause(text), "wccq: Missing operand")
        self.assertEqual(torture.cause("llrm-c: foo_: place n45 has incomplete extent"), "foo_: place nN has incomplete extent")

    def test_a_refusal_is_matched_on_the_cause_and_names_its_reason(self):
        rules = torture.expected()
        self.assertIn("front end", torture.refusal("wccq: Expecting '_' but found '_'", rules))
        self.assertIsNone(torture.refusal("_main: .X is neither defined nor imported", rules))



class OptionTests(unittest.TestCase):
    def test_a_program_that_needs_fwrapv_is_built_with_it(self):
        """950704-1 checks for signed overflow after the add and says so with dg-additional-options "-fwrapv": built without
        it the optimiser dropped the check and the program aborted (wrong, at -O2 and -Os)."""
        self.assertEqual(torture.program_options('/* { dg-additional-options "-fwrapv" } */\nint x;'), ["-fwrapv"])
        self.assertEqual(torture.program_options('/* { dg-additional-options "-O3 -fwrapv -fno-tree-ccp" } */'), ["-fwrapv"])
        self.assertEqual(torture.program_options("int x;"), [])


class DifferencesTests(unittest.TestCase):
    def test_a_program_that_differs_by_design_names_its_reason(self):
        self.assertIn("ISO C11", torture.differences()["pr32244-1"])
        self.assertIn("pr34971", torture.differences())


if __name__ == "__main__":
    unittest.main()
