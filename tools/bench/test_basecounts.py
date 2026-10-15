"""Bench counts against the base's, not against counts committed per program (tools/bench/basecounts.py)."""

from __future__ import annotations

import sys
import json
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import basecounts as bc  # noqa: E402

FIB = ("fib", "bq32", "O2")


def row(instructions, **more):
    return {"instructions": instructions, "memory_operands": 10, "code_bytes": 100, **more}


def test_a_row_worse_than_the_bases_fails_and_a_better_one_passes_and_nothing_is_committed():
    base = {FIB: row(100), ("fib", "c", "O2"): row(50)}
    worse, better, stale = bc.judge({FIB: row(101), ("fib", "c", "O2"): row(40)}, base, [])
    assert worse == [("fib bq32 -O2", "instructions", 100, 101)] and better == [("fib c -O2", "instructions", 50, 40)] and stale == []
    assert bc.judge({FIB: row(100)}, base, []) == ([], [], [])


def test_rows_the_base_lacks_and_the_reference_compilers_and_failed_rows_are_not_judged():
    base = {FIB: row(100), ("fib", "bas", "O2"): row(10), ("sieve", "c", "O2"): {"error": "x"}}
    got = {FIB: row(100), ("new", "c", "O2"): row(5), ("fib", "bas", "O2"): row(99), ("sieve", "c", "O2"): row(7)}
    assert bc.judge(got, base, []) == ([], [], [])


def test_a_waiver_excuses_the_one_row_counter_and_count_it_names_and_goes_stale_when_unused():
    """A deliberate trade is one file per PR: it holds only for the count it names, so a later change to the row is judged again."""
    base = {FIB: row(100)}
    waiver = [{"label": "fib bq32 -O2", "counter": "instructions", "to": 120, "reason": "trade", "file": "x.toml"}]
    assert bc.judge({FIB: row(120)}, base, waiver) == ([], [], [])
    worse, _, _ = bc.judge({FIB: row(121)}, base, waiver)
    assert worse == [("fib bq32 -O2", "instructions", 100, 121)], "another count is not excused"
    _, _, stale = bc.judge({FIB: row(100)}, base, waiver)
    assert stale == ["x.toml: fib bq32 -O2 instructions to 120"], "a waiver that excused nothing is stale"


def test_waivers_are_read_one_file_each(tmp_path):
    (tmp_path / "bench/waivers").mkdir(parents=True)
    (tmp_path / "bench/waivers/a.toml").write_text('reason = "r"\n[[row]]\nlabel = "fib c -O2"\ncounter = "instructions"\nto = 7\n')
    (tmp_path / "bench/waivers/b.toml").write_text('reason = "s"\n[[row]]\nlabel = "fib c -O2"\ncounter = "instructions"\nto = 9\n')
    assert [(w["file"], w["to"]) for w in bc.waivers(tmp_path)] == [("a.toml", 7), ("b.toml", 9)]


def test_the_base_is_measured_once_per_tree_and_method_and_a_missing_level_is_measured_again(tmp_path):
    made = []

    def measure_with(directory, opts):
        made.append((directory, tuple(opts)))
        return {(name, "c", opt): row(1) for name in ("fib",) for opt in opts} | {("fib", "bas", "O2"): row(9)}

    args = dict(tree_of=lambda sha: "t" + sha, build=lambda sha: Path("/b") / sha, measure_with=measure_with, cache=tmp_path, which="m")
    first = bc.base_counts("s1", ["O2"], **args)
    assert bc.base_counts("s1", ["O2"], **args) == first and made == [(Path("/b/s1"), ("O2",))]
    assert ("fib", "bas", "O2") not in first, "the reference compilers' rows are not the base's"
    bc.base_counts("s1", ["O2", "Os"], **args)
    assert made[-1] == (Path("/b/s1"), ("O2", "Os")) and len(made) == 2
    bc.base_counts("s2", ["O2"], **args)
    assert len(made) == 3, "another tree is another base"
    assert (tmp_path / "ts1-m.json").is_file() and json.loads((tmp_path / "ts1-m.json").read_text())["fib/c/Os"]["instructions"] == 1


def test_the_two_judges_name_where_they_differ():
    old = ["fib bq32 -O2: instructions 101 > 100 (worse)", "fib c -O2: code_bytes 5 < 6 (better: --bless --reason)", "fib bas -O2: instructions 2 > 1 (worse)"]
    worse = [("fib bq32 -O2", "instructions", 100, 101)]
    assert bc.differences(old, worse, [("fib c -O2", "code_bytes", 6, 5)]) == [], "bas is a reference: not compared"
    assert bc.differences(old, [], []) == [
        "fib bq32 -O2 instructions: expected.toml says worse, the base says same",
        "fib c -O2 code_bytes: expected.toml says better, the base says same",
    ]


def test_a_dry_run_that_fails_prints_why_and_never_fails_the_gate(monkeypatch):
    import types

    import bench

    boom = types.SimpleNamespace(git=lambda *a: (_ for _ in ()).throw(RuntimeError("no git")))
    monkeypatch.setitem(sys.modules, "measure", boom)
    lines = bench.base_dry_run({}, types.SimpleNamespace(opt=["O2"], work=Path("/x")), [])
    assert lines == ["BASE: dry run failed: RuntimeError: no git"]


def test_a_base_that_does_not_measure_says_why_on_one_line_the_gate_prints(monkeypatch, tmp_path):
    """Train 5's gate printed `BASE: dry run failed: SystemExit: se did not measure:`: the reason was on the lines below, which the gate's
    filter drops."""
    import subprocess
    import types

    monkeypatch.setattr(bc.subprocess, "run", lambda *a, **k: subprocess.CompletedProcess(a, 1, "", "Traceback\nBuildError: llrm-c: unrecognized arguments: -mbss-begin\n"))
    try:
        bc.measure_in_subprocess(tmp_path, ["O2"], tmp_path / "w")
    except SystemExit as why:
        assert "\n" not in str(why) and "unrecognized arguments: -mbss-begin" in str(why), str(why)
    else:
        raise AssertionError("a base that wrote nothing was taken for measured")
    import bench

    boom = types.SimpleNamespace(git=lambda *a: (_ for _ in ()).throw(SystemExit("a\nb")))
    monkeypatch.setitem(sys.modules, "measure", boom)
    (line,) = bench.base_dry_run({}, types.SimpleNamespace(opt=["O2"], work=Path("/x")), [])
    assert line == "BASE: dry run failed: SystemExit: a b"

def test_a_waiver_excuses_the_one_count_the_committed_judge_would_fail_and_the_bless_spends_it(tmp_path, monkeypatch):
    """Train 5's batch of clean PRs moved jpeg_fixed bq32 -Os by +2 together: the train may not raise a count, so the owner's waiver (the
    row, the counter, the count it may reach) lets exactly that count through, and the bless that writes it removes the waiver."""
    import bench

    want = {"instructions": 100, "code_bytes": 5}
    waiver = [{"label": "jpeg_fixed bq32 -Os", "counter": "instructions", "to": 102, "reason": "r", "file": "w.toml"}]
    assert bench.against("jpeg_fixed bq32 -Os", {"instructions": 102, "code_bytes": 5}, want, waiver) == []
    assert bench.against("jpeg_fixed bq32 -Os", {"instructions": 103, "code_bytes": 5}, want, waiver)[0].endswith("(worse)"), "another count"
    assert bench.against("jpeg_fixed bq32 -O2", {"instructions": 102, "code_bytes": 5}, want, waiver)[0].endswith("(worse)"), "another row"
    assert bench.against("jpeg_fixed bq32 -Os", {"instructions": 102, "code_bytes": 6}, want, waiver)[0].endswith("(worse)"), "another counter"
    (tmp_path / "bench/waivers").mkdir(parents=True)
    (tmp_path / "bench/waivers/w.toml").write_text('reason = "r"\n[[row]]\nlabel = "jpeg_fixed bq32 -Os"\ncounter = "instructions"\nto = 102\n')
    (tmp_path / "bench/waivers/other.toml").write_text('reason = "r"\n[[row]]\nlabel = "fib c -O2"\ncounter = "instructions"\nto = 5\n')
    measured = {("jpeg_fixed", "bq32", "Os"): {"instructions": 102}, ("fib", "c", "O2"): {"instructions": 9}}
    assert bc.consume_waivers(tmp_path, measured) == ["w.toml"]
    assert [p.name for p in (tmp_path / "bench/waivers").glob("*.toml")] == ["other.toml"], "a waiver whose row is not at its count stays"
