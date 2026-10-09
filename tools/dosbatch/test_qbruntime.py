"""The QB runtime harness rejects an unproved replacement library."""

from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import qbruntime  # noqa: E402
import run_tests  # noqa: E402


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


class ArchiveTests(unittest.TestCase):
    def test_archive_rebuild_removes_the_old_library_first(self):
        """LIB ignored replacement members, so a changed runtime still linked the old archive."""
        with tempfile.TemporaryDirectory() as temporary:
            work = Path(temporary)
            stale = work / qbruntime.ARCHIVE_NAME
            stale.write_bytes(b"old archive")
            qbruntime.remove_existing_archive(work)
            self.assertFalse(stale.exists())


class InventoryTests(unittest.TestCase):
    def test_milestone_one_inventory_names_the_25_basic_benchmarks_once(self):
        """A glob omitted nbody_fixed, so the claimed milestone inventory had 24 links."""
        sources = qbruntime.milestone_sources()
        self.assertEqual(len(sources), 25)
        self.assertEqual({source.parent.name for source in sources}, set(qbruntime.MILESTONE_ONE))

    def test_linker_inventory_deduplicates_only_the_reported_b_symbols(self):
        """A broad symbol scan recorded private names that LINK did not require and hid a missing entry."""
        log = (
            "I000.OBJ(p.bas) : error L2029 : 'B$SASS' : unresolved external\n"
            "Unresolved external _main in module P\n"
            "I001.OBJ(q.bas) : error L2029 : 'B$SASS' : unresolved external\n"
            "I000.OBJ(p.bas) : error L2029 : 'B$FLEN' : unresolved external\n"
        )
        self.assertEqual(qbruntime.undefined_symbols(log), ["B$FLEN", "B$SASS"])


class DemoSourceTests(unittest.TestCase):
    def test_missing_demo_directory_says_which_environment_variable_to_set(self):
        """A missing demo tree looked like an empty successful test selection."""
        with tempfile.TemporaryDirectory() as work:
            found, reason = qbruntime.demo_sources(Path(work))
        self.assertEqual(found, {})
        self.assertEqual(reason, "QB45_DEMOS_DIR is unset or lacks NIBBLES.BAS and GORILLA.BAS")


@unittest.skipUnless(qbruntime.dosbatch.QB45.is_dir(), "QB45_DIR is unavailable")
class RuntimeFibTests(unittest.TestCase):
    def test_fib_matches_bcom45_byte_for_byte(self):
        """A separate SS frame made fib's local addresses read as zero through DS."""
        source = next(source for source in qbruntime.milestone_sources() if source.stem == "fib")
        with tempfile.TemporaryDirectory() as temporary:
            work = Path(temporary)
            object_ = work / "fib.obj"
            error = run_tests.compile_one(run_tests.Program(source, ["-O2"], None, "qb45"), object_)
            self.assertIsNone(error)
            archive, _ = qbruntime.build(work / "archive")
            result = qbruntime.differential(object_, archive, work / "differential", "fib")
        self.assertEqual(result.reference.status, "ok")
        self.assertEqual(result.candidate.status, "ok")
        self.assertEqual(result.difference, "")


@unittest.skipUnless(qbruntime.dosbatch.QB45.is_dir(), "QB45_DIR is unavailable")
class RuntimeCrcTests(unittest.TestCase):
    def test_crc_matches_bcom45_byte_for_byte(self):
        """Stack arguments to regparm3 produced -435612498; cleanup then lost the far return."""
        source = next(source for source in qbruntime.milestone_sources() if source.stem == "crc")
        with tempfile.TemporaryDirectory() as temporary:
            work = Path(temporary)
            object_ = work / "crc.obj"
            error = run_tests.compile_one(run_tests.Program(source, ["-O2"], None, "qb45"), object_)
            self.assertIsNone(error)
            archive, _ = qbruntime.build(work / "archive")
            result = qbruntime.differential(object_, archive, work / "differential", "crc")
        self.assertEqual(result.reference.status, "ok")
        self.assertEqual(result.candidate.status, "ok")
        self.assertEqual(result.difference, "")
