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
