"""The QB runtime harness rejects an unproved replacement library."""

from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import qbruntime  # noqa: E402


class RuntimeSelectionTests(unittest.TestCase):
    def test_every_runtime_choice_has_one_explicit_library(self):
        """A candidate run accidentally used BCOM45, so its output proved no replacement behavior."""
        candidate = Path("candidate.lib")
        empty = Path("empty.lib")
        self.assertEqual(qbruntime.library("bcom45", candidate, empty), None)
        self.assertEqual(qbruntime.library("llrmqb", candidate, empty), candidate)
        self.assertEqual(qbruntime.library("empty", candidate, empty), empty)
        with self.assertRaisesRegex(ValueError, "runtime"):
            qbruntime.library("other", candidate, empty)


class OutputTests(unittest.TestCase):
    def test_raw_output_preserves_spaces_and_line_endings(self):
        """The line-oriented checker accepted a candidate that lost a trailing space and CR byte."""
        self.assertEqual(qbruntime.first_byte_difference(b"A \r\n", b"A \r\n"), "")
        self.assertEqual(qbruntime.first_byte_difference(b"A \r\n", b"A\n"), "byte 2: want 32 got 10")
        self.assertEqual(qbruntime.first_byte_difference(b"A", b"A "), "byte 2: want <end> got 32")


class InventoryTests(unittest.TestCase):
    def test_milestone_one_inventory_names_the_25_basic_benchmarks_once(self):
        """A glob omitted nbody_fixed, so the claimed milestone inventory had 24 links."""
        sources = qbruntime.milestone_sources()
        self.assertEqual(len(sources), 25)
        self.assertEqual({source.parent.name for source in sources}, set(qbruntime.MILESTONE_ONE))

    def test_linker_inventory_deduplicates_only_the_reported_b_symbols(self):
        """A broad symbol scan recorded private names that LINK did not require and hid a missing entry."""
        log = "I000.OBJ(p.bas) : error L2029 : 'B$SASS' : unresolved external\nUnresolved external _main in module P\nI001.OBJ(q.bas) : error L2029 : 'B$SASS' : unresolved external\nI000.OBJ(p.bas) : error L2029 : 'B$FLEN' : unresolved external\n"
        self.assertEqual(qbruntime.undefined_symbols(log), ["B$FLEN", "B$SASS"])


class DemoSourceTests(unittest.TestCase):
    def test_missing_demo_directory_says_which_environment_variable_to_set(self):
        """A missing demo tree looked like an empty successful test selection."""
        with tempfile.TemporaryDirectory() as work:
            found, reason = qbruntime.demo_sources(Path(work))
        self.assertEqual(found, {})
        self.assertEqual(reason, "QB45_DEMOS_DIR is unset or lacks NIBBLES.BAS and GORILLA.BAS")
