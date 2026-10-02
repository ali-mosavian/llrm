"""`uv run --project tools python -m unittest discover -s tools/loops`"""

import sys
import unittest
from argparse import Namespace
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import run  # noqa: E402


class ReferenceTests(unittest.TestCase):
    def test_a_run_without_references_may_not_write_the_known_shortfalls(self):
        """A --no-refs run judged every case "ideal" and reported 26,880 new
        shortfalls against a file written with references."""
        langs = ["c", "bas", "nib"]
        self.assertFalse(run.lacks_references(Namespace(no_refs=False, inline=False), langs))
        self.assertTrue(run.lacks_references(Namespace(no_refs=True, inline=False), langs))
        self.assertTrue(run.lacks_references(Namespace(no_refs=False, inline=True), langs))
        self.assertTrue(run.lacks_references(Namespace(no_refs=False, inline=False), ["bas"]))

    def test_a_loop_is_not_judged_ideal_where_no_reference_was_built(self):
        """Without reference facts every bound read as one no compiler met, so
        every ivs shortfall was filed as ivs-ideal."""
        result = run.Result()
        self.assertIsNone(run.ivs_kind(result, "case", "486-O2", 1, references=False))
        self.assertEqual(run.ivs_kind(result, "case", "486-O2", 1, references=True), "ivs-ideal")


if __name__ == "__main__":
    unittest.main()
