"""`uv run --project tools python -m unittest discover -s tools/torture`"""

import shutil
import sys
import unittest
from unittest import mock
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


class OptionTests(unittest.TestCase):
    def test_a_program_that_needs_fwrapv_is_built_with_it(self):
        """950704-1 checks for signed overflow after the add and says so with dg-additional-options "-fwrapv": built without
        it the optimiser dropped the check and the program aborted (wrong, at -O2 and -Os)."""
        self.assertEqual(torture.program_options('/* { dg-additional-options "-fwrapv" } */\nint x;'), ["-fwrapv"])
        self.assertEqual(torture.program_options('/* { dg-additional-options "-O3 -fwrapv -fno-tree-ccp" } */'), ["-fwrapv"])
        self.assertEqual(torture.program_options("int x;"), [])

class SymbolTests(unittest.TestCase):
    def test_an_undefined_symbol_is_refused_whichever_way_the_target_decorates_it(self):
        """m16 (cdecl, a leading underscore) reported `___builtin_prefetch` and `_sprintf`: the refusals name `__builtin_` and
        `sprintf`, so 114 builds of programs that need a builtin or libc routine the runner lacks counted as link findings."""
        rules = torture.expected()
        for symbol in ("__builtin_prefetch_", "___builtin_prefetch", "_sprintf", "sprintf_", "___builtin_ffs@3", "_sprintf@3"):
            found = [torture.refusal(f"undefined symbol {one}", rules) for one in torture.spellings(symbol)]
            self.assertTrue(any(found), symbol)
        self.assertFalse(any(torture.refusal(f"undefined symbol {one}", rules) for one in torture.spellings("_frobnicate")))


class WorkTests(unittest.TestCase):
    def test_two_runs_without_a_work_option_do_not_share_a_directory(self):
        """The default was one fixed directory: a gate running beside a full run deleted its files (`dosbatch.run` clears its
        work directory), the full run crashed on FileNotFoundError and the gate called dozens of programs wrong."""
        made = []
        real = Path.mkdir

        def recording(self, *args, **kwargs):
            made.append(self)
            return real(self, *args, **kwargs)

        for _ in range(2):
            with mock.patch.object(torture, "programs", return_value=[]), mock.patch.object(sys, "argv", ["torture.py"]), mock.patch.object(Path, "mkdir", recording):
                self.assertEqual(torture.main(), 2)
        self.assertEqual(len(made), 2)
        self.assertNotEqual(made[0], made[1])
        for one in made:
            shutil.rmtree(one, ignore_errors=True)


class DifferencesTests(unittest.TestCase):
    def test_a_program_that_differs_by_design_names_its_reason(self):
        self.assertIn("ISO C11", torture.differences()["pr32244-1"])
        self.assertIn("pr34971", torture.differences())


if __name__ == "__main__":
    unittest.main()
