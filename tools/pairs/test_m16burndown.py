"""The m16 -Os burn-down lists every grower: a program one byte bigger on the new build was not in any list (rule 3 of the size bar: no program above main)."""
from __future__ import annotations

import importlib.util
from pathlib import Path

HERE = Path(__file__).resolve().parent


def load():
    spec = importlib.util.spec_from_file_location("m16burndown", HERE / "m16burndown.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_a_planted_one_byte_grower_is_listed():
    burndown = load()
    base = {"a": 100, "b": 100, "c": 100}
    new = {"a": 100, "b": 101, "c": 90}
    assert burndown.growers(list(base), base, new) == ["b"]


def test_the_largest_grower_comes_first():
    burndown = load()
    base = {"a": 10, "b": 10}
    new = {"a": 12, "b": 40}
    assert burndown.growers(list(base), base, new) == ["b", "a"]


def test_a_row_with_no_rule_is_open_and_rests_on_no_oracle():
    burndown = load()
    rules = [{"match": "lru/*", "status": "cause known", "oracle": "m32 llc"}]
    assert burndown.status_of("lru/lru.c", rules) == ("cause known", "m32 llc")
    assert burndown.status_of("sieve/sieve.c", rules) == ("open", "none")


def test_the_mode_defaults_to_fast_and_the_board_refuses_another_spelling():
    """SIZES_EXCESS unset gave main SINGLE as fast and the tip as standard: fpbench read 126 B of growth, 74 B of it the deliberate rounding."""
    import pytest

    burndown = load()
    assert burndown.excess_mode({}) == "fast"
    assert burndown.excess_mode({"SIZES_EXCESS": "standard"}) == "standard"
    with pytest.raises(SystemExit):
        burndown.excess_mode({"SIZES_EXCESS": "strict"})


def test_the_selector_flag_reaches_every_program_of_the_new_dist(monkeypatch, tmp_path):
    """The board measured the default (old) selector while w12 measured --isel=dag: grep.bas read +122 B, +26 B at the dag."""
    sizes = load().m16os.sizes_module()
    monkeypatch.delenv("SIZES_ISEL", raising=False)
    pinned = sum("--isel=dag" in c for _, c in sizes.programs(tmp_path, demos=False))
    monkeypatch.setenv("SIZES_ISEL", "dag")
    listed = sizes.programs(tmp_path, demos=False)
    assert listed and all("--isel=dag" in c for _, c in listed) and pinned < len(listed)


def test_a_dist_without_the_selector_flag_is_told_apart(tmp_path):
    """llrm-qb prints its usage to stderr: reading stdout alone called every dist flag-less and left the base's selector unstamped."""
    burndown = load()
    for name, usage, expected in (("old", "usage: llrm-qb [-g]", False), ("new", "usage: llrm-qb [--isel=old|dag]", True)):
        (tmp_path / name).mkdir()
        tool = tmp_path / name / "llrm-qb"
        tool.write_text(f"#!/bin/sh\necho '{usage}' >&2\n")
        tool.chmod(0o755)
        assert burndown.knows_isel(tmp_path / name) is expected
