"""The byte ratchet's judgement. No gate step compared code bytes of the 8-bit target, so a shared-pass change grew its
programs unseen; each case here is a way such a growth, or a build that produced nothing, could pass."""
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import byte_ratchet  # noqa: E402

LEVELS = ["O1", "Os"]


def judge(ceilings, measured):
    return byte_ratchet.judge("t", LEVELS, ceilings, measured)


def test_growth_names_program_level_bytes_and_ceiling():
    failures, below = judge({"a": [10, 20]}, {"a": [10, 23]})
    assert failures == ["t a -Os: 23 B, ceiling 20 B (+3)"] and below == 0


def test_a_smaller_cell_passes_and_is_counted():
    assert judge({"a": [10, 20]}, {"a": [9, 20]}) == ([], 1)


def test_a_cell_that_no_longer_builds_is_not_zero_bytes():
    failures, _ = judge({"a": [10, 20]}, {"a": [10, byte_ratchet.Unbuilt("llc", "refused")]})
    assert failures == ["t a -Os: did not build (llc: refused), ceiling 20 B"]


def test_a_listed_unbuilt_cell_passes_and_fails_once_it_builds():
    assert judge({"a": ["no:llc", 20]}, {"a": [byte_ratchet.Unbuilt("llc", "x"), 20]}) == ([], 0)
    failures, _ = judge({"a": ["no:llc", 20]}, {"a": [7, 20]})
    assert failures == ["t a -O1: builds now (7 B) but is listed no:llc"]


def test_a_program_without_a_row_and_a_row_without_a_program_fail():
    failures, _ = judge({"gone": [1, 1]}, {"new": [1, 1]})
    assert failures == ["t gone: listed, but not in the corpus", "t new: in the corpus, with no ceiling"]


def test_the_ceiling_file_reads_back(tmp_path):
    path = tmp_path / "c.tsv"
    rows = {"a": [10, "no:llc"]}
    byte_ratchet.write(path, LEVELS, rows)
    assert byte_ratchet.read(path, LEVELS) == rows


def test_a_ceiling_file_of_other_levels_is_refused(tmp_path):
    path = tmp_path / "c.tsv"
    byte_ratchet.write(path, ["O1", "O2"], {"a": [1, 2]})
    with pytest.raises(SystemExit):
        byte_ratchet.read(path, LEVELS)


def test_a_missing_compiler_stops_the_run(tmp_path):
    with pytest.raises(SystemExit, match="llrm-none-llc"):
        byte_ratchet.tools({"compiler": "llrm-none-llc", "needs": []}, tmp_path)


def test_a_missing_assembler_stops_the_run(tmp_path):
    (tmp_path / "cc").touch(mode=0o755)
    with pytest.raises(SystemExit, match="no-such-assembler"):
        byte_ratchet.tools({"compiler": "cc", "needs": ["no-such-assembler"]}, tmp_path)
