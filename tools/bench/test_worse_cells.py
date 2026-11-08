import pytest

import worse_cells


def folder(root, name, expected=None, mark=None):
    where = root / name
    where.mkdir(parents=True)
    if mark is not None:
        (where / "bench.toml").write_text(f"references = {mark}\n")
    if expected is not None:
        (where / "expected.toml").write_text(expected)


def test_a_program_under_parity_finds_its_references(tmp_path):
    """The first table skipped the 11 parity programs: it looked for bench/parity/expected.toml, not bench/parity/<name>/."""
    folder(tmp_path, "parity/loop", "[ow.O2]\ncode_bytes = 171\nclocks = 900\n")
    rows, none = worse_cells.worse({"parity/loop/c/O2": {"code_bytes": 202, "clocks": 800}}, tmp_path)
    assert rows == [("parity/loop/c/O2", ["B 202 vs 171(ow) +18.1%"])] and none == []


def test_a_program_without_references_is_listed_as_no_ref(tmp_path):
    folder(tmp_path, "sfm16_dbg", mark="false")
    rows, none = worse_cells.worse({"sfm16_dbg/c/O2": {"code_bytes": 10, "clocks": 10}}, tmp_path)
    assert rows == [] and none == ["sfm16_dbg"]


def test_a_program_with_no_lookup_and_no_mark_fails_loudly(tmp_path):
    folder(tmp_path, "mystery")
    with pytest.raises(worse_cells.NoReferenceLookup):
        worse_cells.worse({"mystery/c/O2": {"code_bytes": 10, "clocks": 10}}, tmp_path)
