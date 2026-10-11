"""The QB runtime harness rejects an unproved replacement library."""

from __future__ import annotations

import sys
import tempfile
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))

import qbruntime  # noqa: E402


def test_every_runtime_choice_has_one_explicit_library():
    """A candidate run accidentally used BCOM45, so its output proved no replacement behavior."""
    candidate = Path("candidate.lib")
    empty = Path("empty.lib")
    assert qbruntime.library("bcom45", candidate, empty) is None
    assert qbruntime.library("llrmqb", candidate, empty) == candidate
    assert qbruntime.library("empty", candidate, empty) == empty
    with pytest.raises(ValueError, match="runtime"):
        qbruntime.library("other", candidate, empty)


def test_raw_output_preserves_spaces_and_line_endings():
    """The line-oriented checker accepted a candidate that lost a trailing space and CR byte."""
    assert qbruntime.first_byte_difference(b"A \r\n", b"A \r\n") == ""
    assert qbruntime.first_byte_difference(b"A \r\n", b"A\n") == "byte 2: want 32 got 10"
    assert qbruntime.first_byte_difference(b"A", b"A ") == "byte 2: want <end> got 32"


def test_screen_output_preserves_every_cell_and_row():
    """A display comparison that used stripped lines accepted a changed final screen cell."""
    assert qbruntime.first_screen_difference(("A ", "B"), ("A ", "B")) == ""
    assert qbruntime.first_screen_difference(("A ",), ("A",)) == "row 1: want 2 cells got 1"
    assert qbruntime.first_screen_difference(("A",), ("B",)) == "cell 1:1: want 'A' got 'B'"


def test_screen_delta_excludes_the_runners_own_screen_cells():
    """The two runtime mount names differed, but neither belonged to the program's display."""
    assert qbruntime.screen_delta(("bcom45",), ("bcom45X",)) == ("\0\0\0\0\0\0X",)


def test_screen_changes_rejects_missing_job_boundary_samples(tmp_path: Path):
    """A missing screen event once made a display comparison pass without display evidence."""
    assert qbruntime.screen_changes(tmp_path) is None


def test_runtime_jobs_use_one_8_3_stem_for_every_artifact():
    """Shellsort's nine-character source name made both DOS links report not built."""
    assert qbruntime.dos_stem("shellsort") == "shellsor"
    assert qbruntime.dos_stem("quicksort") == "quicksor"
    assert all(len(qbruntime.dos_stem(name)) <= 8 for name in qbruntime.MILESTONE_ONE)


def test_partial_exe_after_unresolved_runtime_symbol_is_not_a_run():
    """Grep's partial EXE crashed after LINK reported five unresolved runtime symbols."""
    assert qbruntime.dosbatch.link_failed("GREP.OBJ : error L2029 : 'B$OPEN' : unresolved external\n")
    assert not qbruntime.dosbatch.link_failed("LINK : warning L4021 : no stack segment\n")


def test_archive_rebuild_removes_the_old_library_first():
    """LIB ignored replacement members, so a changed runtime still linked the old archive."""
    with tempfile.TemporaryDirectory() as temporary:
        work = Path(temporary)
        stale = work / qbruntime.ARCHIVE_NAME
        stale.write_bytes(b"old archive")
        qbruntime.remove_existing_archive(work)
        assert not stale.exists()


def test_milestone_one_inventory_names_the_25_basic_benchmarks_once():
    """A glob omitted nbody_fixed, so the claimed milestone inventory had 24 links."""
    sources = qbruntime.milestone_sources()
    assert len(sources) == 25
    assert {source.parent.name for source in sources} == set(qbruntime.MILESTONE_ONE)


def test_linker_inventory_deduplicates_only_the_reported_b_symbols():
    """A broad symbol scan recorded private names that LINK did not require and hid a missing entry."""
    log = (
        "I000.OBJ(p.bas) : error L2029 : 'B$SASS' : unresolved external\n"
        "Unresolved external _main in module P\n"
        "I001.OBJ(q.bas) : error L2029 : 'B$SASS' : unresolved external\n"
        "I000.OBJ(p.bas) : error L2029 : 'B$FLEN' : unresolved external\n"
    )
    assert qbruntime.undefined_symbols(log) == ["B$FLEN", "B$SASS"]


def test_missing_demo_directory_says_which_environment_variable_to_set():
    """A missing demo tree looked like an empty successful test selection."""
    with tempfile.TemporaryDirectory() as work:
        found, reason = qbruntime.demo_sources(Path(work))
    assert found == {}
    assert reason == "QB45_DEMOS_DIR is unset or lacks NIBBLES.BAS and GORILLA.BAS"
