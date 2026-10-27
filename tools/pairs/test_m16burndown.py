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
