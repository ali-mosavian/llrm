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


    def test_the_front_ends_gnu_and_c99_limits_are_named_not_counted_as_findings(self):
        """Five more wccq errors (15 builds) were listed as compile findings; each is syntax Open Watcom lacks."""
        rules = torture.expected()
        for cause in ("wccq: Cannot use typedef '_' as a variable", "wccq: Incomplete enum declaration", "wccq: Type cast must be a scalar type",
                      "wccq: Assembler error: '_'", "wccq: Expression for '_' must be a '_' or '_'"):
            self.assertIsNotNone(torture.refusal(cause, rules), cause)


class DifferencesTests(unittest.TestCase):
    def test_a_program_that_differs_by_design_names_its_reason(self):
        self.assertIn("ISO C11", torture.differences()["pr32244-1"])
        self.assertIn("pr34971", torture.differences())


if __name__ == "__main__":
    unittest.main()
