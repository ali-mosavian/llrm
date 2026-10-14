"""`uv run --project tools python -m unittest discover -s tools/loops`"""

import sys
import unittest
from argparse import Namespace
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).parent))

import run  # noqa: E402
import mir  # noqa: E402


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


class MirParseTests(unittest.TestCase):
    PARSE_ERROR = "/w/01-globalopt.run.ll: /w/01-globalopt.run.ll:line 6: expected `{`, found `nearcode`"

    def test_a_dump_llrm_mir_cannot_parse_is_not_a_wrong_answer(self):
        """llrm-mir refused the compiler's own dump (`nearcode` was not a
        function attribute it knew), the run called it a trap, and every nib
        case with a near-entered procedure was reported to disagree with the
        oracle at both ends (#1634)."""
        self.assertIsInstance(mir.refusal("/w/x.run.ll", self.PARSE_ERROR), mir.Unparsable)
        self.assertNotIsInstance(mir.refusal("/w/x.run.ll", "fuel exhausted"), mir.Unparsable)
        batch = mock.Mock(config=mock.Mock(tag="486-O2", inline=False), lang="nib", cases=[], expected=[1], work=Path("/w"), stem="n")
        result = run.Result()
        with mock.patch.object(mir, "stages", return_value=(Path("a/01-globalopt.ll"), Path("a/28-homes.ll"))), \
             mock.patch.object(mir, "called", return_value=[]), \
             mock.patch.object(mir, "run", side_effect=mir.Unparsable(self.PARSE_ERROR)):
            run.check_mir(batch, result)
        self.assertEqual(len(result.wrong), 1, result.wrong)
        self.assertIn("does not parse", result.wrong[0])
        self.assertNotIn("disagrees", result.wrong[0])


if __name__ == "__main__":
    unittest.main()
