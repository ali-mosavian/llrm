"""`uv run --project tools python -m unittest discover -s tools/dosbatch`"""

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import run_tests  # noqa: E402


class HeaderTests(unittest.TestCase):
    def read(self, text: str):
        with tempfile.TemporaryDirectory() as where:
            source = Path(where) / "p.bas"
            source.write_text(text)
            return run_tests.header(source)

    def test_flags_and_known_come_from_the_leading_comment(self):
        self.assertEqual(self.read("' flags: -Os --cpu P5\n' known: #123\nPRINT 1\n"), {"flags": "-Os --cpu P5", "known": "#123"})

    def test_a_comment_after_the_code_is_not_a_header(self):
        """A `known:` in the body would mark a program known by accident."""
        self.assertEqual(self.read("PRINT 1\n' known: #9\n"), {})


class DiffTests(unittest.TestCase):
    def test_a_program_that_printed_nothing_is_a_difference(self):
        """A run that produced no output must not read as a pass."""
        self.assertNotEqual(run_tests.first_difference(run_tests.lines("T= 1\n"), run_tests.lines("")), "")

    def test_line_endings_and_trailing_blanks_are_not(self):
        self.assertEqual(run_tests.first_difference(run_tests.lines("A \r\nB\n\n"), run_tests.lines("A\nB")), "")


if __name__ == "__main__":
    unittest.main()
