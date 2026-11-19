"""`uv run --project tools python -m pytest tools/bench/test_shards.py`"""

import sys

import pytest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import shards  # noqa: E402

COST = {"a": 10.0, "b": 9.0, "c": 5.0, "d": 4.0, "e": 1.0}


def test_every_benchmark_is_in_exactly_one_shard_and_the_split_never_changes():
    one = shards.split(list(COST), 2, COST)
    assert sorted(sum(one, [])) == sorted(COST) and one == shards.split(list(reversed(COST)), 2, COST)
    assert [sum(COST[n] for n in part) for part in one] == [15.0, 14.0]


def test_a_benchmark_the_table_lacks_counts_as_the_dearest_not_as_free():
    """A new benchmark costed 0 would sit with the heaviest shard and make it heavier than the table says."""
    one = shards.split(["a", "b", "new"], 2, {"a": 10.0, "b": 9.0})
    assert sorted(sum(one, [])) == ["a", "b", "new"] and all(len(part) in (1, 2) for part in one) and ["b"] not in one


def test_the_checked_in_table_names_every_benchmark_and_only_those():
    """Rebuild it with `python tools/bench/shards.py measure`; a missing benchmark would be costed as the dearest."""
    assert sorted(shards.costs()) == sorted(shards.names())


def test_the_table_lists_names_sorted_with_their_seconds():
    assert shards.table({"grep": 24.24, "fib": 3.0}) == '"fib" = 3.0\n"grep" = 24.2\n'


def test_a_run_with_cells_that_did_not_build_still_costs_its_cpu_seconds(tmp_path):
    """bench.py exits 1 (NOHEAD) when a cell did not build; `measure` stopped on jpeg_fixed (bq32 O2/Os) and wrote no table."""
    finished = [sys.executable, "-c", "import sys; sum(range(10**6)); print('bench: [1s] measured 3 rows', file=sys.stderr); sys.exit(1)"]
    assert shards.cpu_seconds("x", tmp_path, finished) > 0


def test_a_run_that_never_measured_is_no_cost_and_stops_the_table(tmp_path):
    early = [sys.executable, "-c", "import sys; print('boom', file=sys.stderr); sys.exit(1)"]
    with pytest.raises(SystemExit):
        shards.cpu_seconds("x", tmp_path, early)
