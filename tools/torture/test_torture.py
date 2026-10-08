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



class SymbolTests(unittest.TestCase):
    def test_an_undefined_symbol_is_refused_whichever_way_the_target_decorates_it(self):
        """m16 (cdecl, a leading underscore) reported `___builtin_prefetch` and `_sprintf`: the refusals name `__builtin_` and
        `sprintf`, so 114 builds of programs that need a builtin or libc routine the runner lacks counted as link findings."""
        rules = torture.expected()
        for symbol in ("__builtin_prefetch_", "___builtin_prefetch", "_sprintf", "sprintf_"):
            found = [torture.refusal(f"undefined symbol {one}", rules) for one in torture.spellings(symbol)]
            self.assertTrue(any(found), symbol)
        self.assertFalse(any(torture.refusal(f"undefined symbol {one}", rules) for one in torture.spellings("_frobnicate")))


class DifferencesTests(unittest.TestCase):
    def test_a_program_that_differs_by_design_names_its_reason(self):
        self.assertIn("ISO C11", torture.differences()["pr32244-1"])
        self.assertIn("pr34971", torture.differences())


if __name__ == "__main__":
    unittest.main()
