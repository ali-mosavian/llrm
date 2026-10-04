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

    def test_dialect_picks_the_compiler_dialect(self):
        """hugerg and hugelp need PDS /Ah: built as QB 4.5 they died in the lowering with a Python repr."""
        self.assertEqual(self.read("' dialect: pds71\nPRINT 1\n"), {"dialect": "pds71"})

    def test_a_comment_after_the_code_is_not_a_header(self):
        """A `known:` in the body would mark a program known by accident."""
        self.assertEqual(self.read("PRINT 1\n' known: #9\n"), {})


class ExampleTests(unittest.TestCase):
    def test_a_mask_hides_only_what_varies(self):
        """ticker prints its busy-loop count, a machine speed: 2483958 spins one run, another the next."""
        got = run_tests.masked(["tick 9: 2483958 spins", "36 ticks"], r"\d+(?= spins)")
        self.assertEqual(got, ["tick 9: N spins", "36 ticks"])

    def test_without_a_mask_the_output_is_untouched(self):
        self.assertEqual(run_tests.masked(["12 spins"], ""), ["12 spins"])


class DiffTests(unittest.TestCase):
    def test_a_program_that_printed_nothing_is_a_difference(self):
        """A run that produced no output must not read as a pass."""
        self.assertNotEqual(run_tests.first_difference(run_tests.lines("T= 1\n"), run_tests.lines("")), "")

    def test_line_endings_and_trailing_blanks_are_not(self):
        self.assertEqual(run_tests.first_difference(run_tests.lines("A \r\nB\n\n"), run_tests.lines("A\nB")), "")


if __name__ == "__main__":
    unittest.main()


class CorpusTests(unittest.TestCase):
    def test_a_program_whose_corpus_is_unavailable_is_skipped_with_the_reason(self):
        """tests/run would have failed grep on every machine without the 10 MB file and a network."""
        from unittest import mock

        with tempfile.TemporaryDirectory() as where:
            source = Path(where) / "g.c"
            source.write_text("// data: @dickens\nint main(void){return 0;}\n")
            program = run_tests.Program(source, [], None, data=("@dickens",))
            with mock.patch.object(run_tests.corpus, "path", side_effect=run_tests.corpus.Unavailable("dickens could not be fetched (offline?)")):
                self.assertIn("offline", run_tests.unavailable(program))
            with mock.patch.object(run_tests.corpus, "path", return_value=Path(where) / "dickens"):
                self.assertIsNone(run_tests.unavailable(program))
                self.assertEqual(run_tests.data_files(program), (Path(where) / "dickens",))


class PlaceTests(unittest.TestCase):
    def test_a_data_file_is_placed_once_and_linked_where_it_can_be(self):
        with tempfile.TemporaryDirectory() as where:
            source, a = Path(where) / "big", Path(where) / "A"
            source.write_bytes(b"x" * 100)
            run_tests.dosbatch.place(source, a)
            self.assertEqual(a.stat().st_ino, source.stat().st_ino)
            source.write_bytes(b"y" * 100)
            a2 = Path(where) / "A"
            run_tests.dosbatch.place(Path(where) / "other", a2)  # exists: nothing to do, not even a read of `other`
            self.assertTrue(a2.exists())
