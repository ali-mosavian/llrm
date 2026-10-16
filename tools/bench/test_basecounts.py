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
    (tmp_path / bc.WAIVERS).mkdir(parents=True)
    (tmp_path / bc.WAIVERS / "a.toml").write_text('reason = "r"\n[[row]]\nlabel = "fib c -O2"\ncounter = "instructions"\nto = 7\n')
    (tmp_path / bc.WAIVERS / "b.toml").write_text('reason = "s"\n[[row]]\nlabel = "fib c -O2"\ncounter = "instructions"\nto = 9\n')
    assert [(w["file"], w["to"]) for w in bc.waivers(tmp_path)] == [("a.toml", 7), ("b.toml", 9)]


def test_the_base_is_measured_once_per_tree_and_a_missing_level_is_measured_again(tmp_path):
    made = []

    def measure_with(directory, opts):
        made.append((directory, tuple(opts)))
        rows = {("fib", "c", opt): row(1) for opt in opts} | {("fib", "bas", "O2"): row(9)}
        return rows, {"fib": "h1"}

    args = dict(tree_of=lambda sha: "t" + sha, build=lambda sha: Path("/b") / sha, measure_with=measure_with, cache=tmp_path)
    rows, hashes = bc.base_counts("s1", ["O2"], **args)
    assert bc.base_counts("s1", ["O2"], **args) == (rows, hashes) and made == [(Path("/b/s1"), ("O2",))]
    assert ("fib", "bas", "O2") not in rows, "the reference compilers' rows are not the base's"
    assert hashes == {"fib": "h1"}
    bc.base_counts("s1", ["O2", "Os"], **args)
    assert made[-1] == (Path("/b/s1"), ("O2", "Os")) and len(made) == 2
    bc.base_counts("s2", ["O2"], **args)
    assert len(made) == 3, "another tree is another base"
    assert json.loads((tmp_path / "ts1-own.json").read_text())["rows"]["fib/c/Os"]["instructions"] == 1


def test_a_program_that_differs_between_the_base_and_the_head_is_not_judged_and_the_baseline_file_is_not_part_of_it(tmp_path):
    """The base is measured by its own harness and programs: a row of an edited program compares two programs, not two compilers."""
    (tmp_path / "bench/fib").mkdir(parents=True)
    (tmp_path / "bench/fib/bench.toml").write_text("repetitions = 1\n")
    (tmp_path / "bench/fib/fib.c").write_text("int a;\n")
    first = bc.programs(tmp_path)
    (tmp_path / "bench/fib/expected.toml").write_text("blessed\n")
    assert bc.programs(tmp_path) == first, "a baseline is not the program"
    (tmp_path / "bench/fib/fib.c").write_text("int b;\n")
    assert bc.programs(tmp_path) != first
    base = {("fib", "c", "O2"): row(100), ("sieve", "c", "O2"): row(100)}
    got = {("fib", "c", "O2"): row(500), ("sieve", "c", "O2"): row(500)}
    worse, _, _ = bc.judge(got, base, [], same={"sieve"})
    assert [one[0] for one in worse] == ["sieve c -O2"], "fib's program changed: not judged"


def test_the_base_is_measured_by_its_own_trees_bench_not_the_heads(monkeypatch, tmp_path):
    """The head's harness may pass flags the base's compiler does not know (an old base died on `-mbss-begin`): the base runs its own."""
    import subprocess

    tree = tmp_path / "b/tree"
    (tree / "tools/bench").mkdir(parents=True)
    (tree / "tools/bench/bench.py").write_text("# an old bench.py: no judge-skipping flag\n")
    (tree / "bench").mkdir()
    seen = []

    def fake(command, **kw):
        seen.append((command, kw["cwd"], kw["env"]["LLRM_BIN"]))
        Path(command[command.index("--json") + 1]).write_text(json.dumps({"fib/c/O2": row(7)}))
        return subprocess.CompletedProcess(command, 1, "", "")  # an old bench exits 1 on problems: the rows are what counts

    monkeypatch.setattr(bc.subprocess, "run", fake)
    rows, hashes = bc.measure_in_subprocess(tmp_path / "b/target/release", ["O2"], tmp_path / "w")
    assert rows == {("fib", "c", "O2"): row(7)} and hashes == {}
    command, cwd, binary = seen[0]
    assert command[1] == str(tree / "tools/bench/bench.py") and cwd == tree and binary == str(tmp_path / "b/target/release")
    assert "--no-judge" not in command, "a bench.py without the flag is not given it"


def test_a_base_that_does_not_measure_says_why_on_one_line_the_gate_prints(monkeypatch, tmp_path):
    """Train 5's gate printed `BASE: dry run failed: SystemExit: se did not measure:`: the reason was on the lines below, which the gate's
    filter drops."""
    import subprocess

    monkeypatch.setattr(bc.subprocess, "run", lambda *a, **k: subprocess.CompletedProcess(a, 1, "", "Traceback\nBuildError: llrm-c: unrecognized arguments: -mbss-begin\n"))
    try:
        bc.measure_in_subprocess(tmp_path, ["O2"], tmp_path / "w")
    except SystemExit as why:
        assert "\n" not in str(why) and "unrecognized arguments: -mbss-begin" in str(why), str(why)
    else:
        raise AssertionError("a base that wrote nothing was taken for measured")


def test_a_waiver_excuses_the_one_count_it_names_and_no_other(monkeypatch):
    """Train 5's batch of clean PRs moved jpeg_fixed bq32 -Os by +2 together: the owner's waiver (the row, the counter, the count it may
    reach) lets exactly that count through."""
    import bench

    want = {"instructions": 100, "code_bytes": 5}
    waiver = [{"label": "jpeg_fixed bq32 -Os", "counter": "instructions", "to": 102, "reason": "r", "file": "w.toml"}]
    assert bench.against("jpeg_fixed bq32 -Os", {"instructions": 102, "code_bytes": 5}, want, waiver) == []
    assert bench.against("jpeg_fixed bq32 -Os", {"instructions": 103, "code_bytes": 5}, want, waiver)[0].endswith("(worse)"), "another count"
    assert bench.against("jpeg_fixed bq32 -O2", {"instructions": 102, "code_bytes": 5}, want, waiver)[0].endswith("(worse)"), "another row"
    assert bench.against("jpeg_fixed bq32 -Os", {"instructions": 102, "code_bytes": 6}, want, waiver)[0].endswith("(worse)"), "another counter"


def test_waivers_live_outside_bench_where_every_tool_that_lists_the_benchmarks_would_take_them_for_a_program():
    """#1820 put them in bench/waivers/: identical.py looked for bench/waivers/waivers.c, vsgcc's programs, test_scaling_gate,
    test_compile_cost and tests/run.rs all listed a benchmark called `waivers`."""
    assert not bc.WAIVERS.startswith("bench/") and (bc.ROOT / "tools/bench").is_dir()
    assert not (bc.ROOT / "bench/waivers").exists()
