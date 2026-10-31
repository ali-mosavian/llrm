"""Bench counts against the base's, not against counts committed per program (tools/bench/basecounts.py)."""

from __future__ import annotations

import pytest
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
    base = {FIB: row(100), ("fib", "ow", "O2"): row(10), ("sieve", "c", "O2"): {"error": "x"}}
    got = {FIB: row(100), ("new", "c", "O2"): row(5), ("fib", "ow", "O2"): row(99), ("sieve", "c", "O2"): row(7)}
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


def test_a_bas_row_that_gets_worse_fails_and_a_waiver_excuses_it():
    """llrm-qb's BASIC is ours: its rows were judged against stored tables --bless no longer writes, so a deliberate change could be neither waived nor blessed."""
    key = ("fib", "bas", "O2")
    waiver = [{"label": "fib bas -O2", "counter": "instructions", "to": 120, "reason": "trade", "file": "x.toml"}]
    assert bc.judge({key: row(120)}, {key: row(100)}, [])[0] == [("fib bas -O2", "instructions", 100, 120)]
    assert bc.judge({key: row(120)}, {key: row(100)}, waiver)[0] == []


def test_waivers_are_read_one_file_each(tmp_path):
    (tmp_path / bc.WAIVERS).mkdir(parents=True)
    (tmp_path / bc.WAIVERS / "a.toml").write_text('reason = "r"\n[[row]]\nlabel = "fib c -O2"\ncounter = "instructions"\nto = 7\n')
    (tmp_path / bc.WAIVERS / "b.toml").write_text('reason = "s"\n[[row]]\nlabel = "fib c -O2"\ncounter = "instructions"\nto = 9\n')
    assert [(w["file"], w["to"]) for w in bc.waivers(tmp_path)] == [("a.toml", 7), ("b.toml", 9)]


def test_the_base_is_measured_once_per_tree_and_a_missing_level_is_measured_again(tmp_path):
    made = []

    def measure_with(directory, opts):
        made.append((directory, tuple(opts)))
        rows = {("fib", "c", opt): row(1) for opt in opts} | {("fib", "ow", "O2"): row(9)}
        return rows, {"fib": "h1"}

    args = dict(tree_of=lambda sha: "t" + sha, build=lambda sha: Path("/b") / sha, measure_with=measure_with, cache=tmp_path)
    rows, hashes = bc.base_counts("s1", ["O2"], **args)
    assert bc.base_counts("s1", ["O2"], **args) == (rows, hashes) and made == [(Path("/b/s1"), ("O2",))]
    assert ("fib", "ow", "O2") not in rows, "the reference compilers' rows are not the base's"
    assert hashes == {"fib": "h1"}
    bc.base_counts("s1", ["O2", "Os"], **args)
    assert made[-1] == (Path("/b/s1"), ("O2", "Os")) and len(made) == 2
    bc.base_counts("s2", ["O2"], **args)
    assert len(made) == 3, "another tree is another base"
    assert json.loads((tmp_path / "ts1-own.json").read_text())["rows"]["fib/c/Os"]["instructions"] == 1


def test_an_expected_output_or_a_generated_listing_added_to_a_program_does_not_make_it_a_different_program(tmp_path):
    """b7f86c5d53 added `<name>.bcout` to 11 benchmarks and d405319be6 a `grep.asm`: all 12 hashed differently from main, counted as edited, and
    the judge skipped every row of them (182 rows over main unseen, 0 problems printed). Only what the compilers read is the program."""
    (tmp_path / "bench/fib").mkdir(parents=True)
    (tmp_path / "bench/fib/bench.toml").write_text("repetitions = 1\n")
    (tmp_path / "bench/fib/fib.c").write_text("int a;\n")
    first = bc.programs(tmp_path)
    for name in ("fib.bcout", "fib.out", "fib.ci.out", "fib.asm"):
        (tmp_path / "bench/fib" / name).write_text("5\n")
        assert bc.programs(tmp_path) == first, f"{name} is not the program"
    (tmp_path / "bench/fib/bench.toml").write_text("repetitions = 2\n")
    assert bc.programs(tmp_path) != first, "a setting is"


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

    monkeypatch.setattr(bc, "run_streaming", fake)
    rows, hashes = bc.measure_in_subprocess(tmp_path / "b/target/release", ["O2"], tmp_path / "w")
    assert rows == {("fib", "c", "O2"): row(7)} and hashes == {}
    command, cwd, binary = seen[0]
    assert command[1] == str(tree / "tools/bench/bench.py") and cwd == tree and binary == str(tmp_path / "b/target/release")
    assert "--no-judge" not in command, "a bench.py without the flag is not given it"


def test_a_base_that_does_not_measure_says_why_on_one_line_the_gate_prints(monkeypatch, tmp_path):
    """Train 5's gate printed `BASE: dry run failed: SystemExit: se did not measure:`: the reason was on the lines below, which the gate's
    filter drops."""
    import subprocess

    monkeypatch.setattr(bc, "run_streaming", lambda *a, **k: subprocess.CompletedProcess(a, 1, "", "Traceback\nBuildError: llrm-c: unrecognized arguments: -mbss-begin\n"))
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


def _repo(tmp_path):
    import subprocess

    def git(*args):
        return subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", *args], cwd=tmp_path, check=True, capture_output=True, text=True).stdout.strip()

    git("init", "-q", "-b", "main")
    (tmp_path / "crates").mkdir()
    (tmp_path / "docs").mkdir()
    (tmp_path / "crates/a.rs").write_text("1")
    (tmp_path / "docs/a.md").write_text("1")
    git("add", "-A")
    git("commit", "-q", "-m", "one")
    return git


def test_a_commit_that_changes_only_docs_has_its_parents_key_and_one_that_changes_code_does_not(tmp_path):
    """The cache of a base's counts is keyed by the tree: a docs-only commit is a new tree with the same counts, a miss that built and
    measured the base again for 20 minutes."""
    git = _repo(tmp_path)
    first = bc.compiler_key("HEAD", tmp_path)
    (tmp_path / "docs/a.md").write_text("2")
    git("commit", "-q", "-am", "docs")
    assert bc.compiler_key("HEAD", tmp_path) == first
    (tmp_path / "crates/a.rs").write_text("2")
    git("commit", "-q", "-am", "code")
    assert bc.compiler_key("HEAD", tmp_path) != first


def test_the_counts_a_main_push_records_are_the_next_pull_requests_base_without_measuring_it(tmp_path):
    rows = {"fib/c/O2": {"instructions": 7}, "fib/ow/O2": {"instructions": 9}}
    cache = tmp_path / "cache"
    bc.record(bc.rows_of(rows), cache, "k1", bc.ROOT)

    def never(*a):
        raise AssertionError("the base was built or measured")

    have, hashes = bc.base_counts("sha", ["O2"], lambda sha: "k1", never, never, cache=cache)
    assert have == {("fib", "c", "O2"): {"instructions": 7}} and "fib" in hashes, "ours only: the reference's row is not recorded"


def test_a_shard_measures_the_slice_of_a_missing_base_and_stores_none_of_it(tmp_path):
    """A stored slice would pass for the whole base: every later run would find its levels present and judge the other rows against nothing."""
    seen = []

    def measure(directory, opts, names=None):
        seen.append(names)
        return {("fib", "c", "O2"): {"instructions": 7}}, {"fib": "h"}

    have, _ = bc.base_counts("sha", ["O2"], lambda sha: "k2", lambda sha: Path("/d"), measure, cache=tmp_path, only=["fib"])
    assert seen == [["fib"]] and have == {("fib", "c", "O2"): {"instructions": 7}} and list(tmp_path.iterdir()) == []


def test_stale_waivers_are_one_line_of_file_names():
    """Twenty-nine BASE-STALE lines sat above the FAIL lines of #1771 and hid them."""
    stale = [f"a.toml: row{i} instructions to {i}" for i in range(29)] + ["b.toml: x instructions to 1"]
    assert bc.stale_summary(stale) == ["BASE-STALE 30 waivers in 2 files, delete them: a.toml b.toml"]
    assert bc.stale_summary([]) == []


def test_a_base_stored_under_the_old_hash_is_judged_by_the_new_one_not_skipped_whole(tmp_path):
    """programs() stopped hashing `.out` files; a base cached under the old hash then differed from the head in EVERY program (main has an `.out` for
    each): `same` was empty, 424 measurements judged nothing and 218 waivers read stale, with 0 problems printed. A cache that does not say
    which hash it holds is re-hashed from the base commit."""
    import subprocess

    def git(*args):
        return subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", *args], cwd=tmp_path, check=True, capture_output=True, text=True).stdout.strip()

    git("init", "-q", "-b", "main")
    (tmp_path / "bench/fib").mkdir(parents=True)
    (tmp_path / "bench/fib/bench.toml").write_text("repetitions = 1\n")
    (tmp_path / "bench/fib/fib.c").write_text("int a;\n")
    (tmp_path / "bench/fib/fib.out").write_text("5\n")
    git("add", "-A")
    git("commit", "-q", "-m", "base")
    sha = git("rev-parse", "HEAD")
    old = hashlib_of_every_file(tmp_path / "bench/fib")
    cache = tmp_path / "cache"
    cache.mkdir()
    (cache / f"{sha}-own.json").write_text(json.dumps({"rows": {"fib/c/O2": {"instructions": 1}}, "programs": {"fib": old}}))
    (tmp_path / "bench/fib/fib.bcout").write_text("5\n")
    (tmp_path / "bench/fib/fib.out").write_text("6\n")
    _, hashes = bc.base_counts(sha, ["O2"], lambda s: s, None, None, cache=cache, root=tmp_path)
    assert hashes == bc.programs(tmp_path), "the head's outputs changed and nothing it compiles"


def hashlib_of_every_file(directory):
    import hashlib

    digest = hashlib.sha256()
    for one in sorted(directory.iterdir()):
        digest.update(one.name.encode() + b"\0" + one.read_bytes())
    return digest.hexdigest()[:12]


def test_the_judge_fails_a_run_that_judged_nothing_and_a_program_that_yielded_no_row():
    """The state of the run above: 424 measurements, `same` empty, no row compared, 0 problems. Every program the head and base share must yield a
    judged row, and the run must say how many rows it judged and which programs it left out as edited."""
    base = {("fib", "c", "O2"): row(100), ("sieve", "c", "O2"): {"error": "x"}, ("edited", "c", "O2"): row(1)}
    got = {("fib", "c", "O2"): row(100), ("sieve", "c", "O2"): row(7), ("edited", "c", "O2"): row(9)}
    judged, programs, left_out, problems = bc.coverage(got, base, {"fib", "sieve"})
    assert (judged, programs, left_out) == (1, 1, ["edited"])
    assert problems == ["COVERAGE sieve: no row was judged (rows skipped: error)"]
    assert bc.coverage(got, base, set())[3] == ["COVERAGE: no row was judged: no program is the same in the base and the head"]
    assert bc.coverage(got, base, {"fib", "edited"})[3] == []


def test_a_program_edited_since_the_base_is_judged_against_the_base_compiler_on_its_edited_sources(tmp_path):
    """jpeg_single and 8 more were left out of the judge (their sources differ from main's): their rows grew 11693 vs 6016 unseen. Now the
    base compiler measures the head's sources and those counts are the base; a row it cannot build is named, not judged."""
    base = {FIB: row(100), ("jpeg", "nib", "O2"): row(50)}
    asked = []

    def measure(names):
        asked.append(names)
        return {("jpeg", "nib", "O2"): row(60), ("jpeg", "c", "O2"): {"error": "no front end"}}

    got, same, unbuilt = bc.with_edited(base, {"fib": "a", "jpeg": "old"}, {"fib": "a", "jpeg": "new"}, ["O2"], measure, tmp_path, "t")
    assert same == {"fib", "jpeg"} and asked == [["jpeg"]] and unbuilt == ["jpeg c -O2"]
    worse, _, _ = bc.judge({FIB: row(100), ("jpeg", "nib", "O2"): row(70)}, got, [], same)
    assert worse == [("jpeg nib -O2", "instructions", 60, 70)]
    bc.with_edited(base, {"jpeg": "old"}, {"jpeg": "new"}, ["O2"], measure, tmp_path, "t")
    assert asked == [["jpeg"]], "measured once per tree and edit"


def test_the_judge_counts_the_rows_its_waivers_excused_and_fails_waivers_it_never_read():
    """424 measurements: 0 problems said nothing of the 267 rows that passed under a waiver, so a judge that read no waiver at all looked the
    same. The summary names how many rows ran under which file; waivers on file with none applied and none stale are a problem."""
    base = {FIB: row(100), ("sieve", "c", "O2"): row(10)}
    got = {FIB: row(120), ("sieve", "c", "O2"): row(10)}
    waivers = [{"label": "fib bq32 -O2", "counter": "instructions", "to": 120, "reason": "r", "file": "a.toml"}]
    rows = bc.waived_rows(got, base, waivers)
    assert rows == [("a.toml", "fib bq32 -O2", "instructions", 100, 120)]
    lines, problems = bc.waived_lines(rows, waivers, [])
    assert lines == ["WAIVED 1 rows of 1 waivers (limit = measured): a.toml 1"] and problems == []
    lines, problems = bc.waived_lines([], waivers, [])
    assert problems == ["WAIVERS: none was applied and none is stale: the waivers are not being read"]
    assert bc.waived_lines([], waivers, ["a.toml: x"])[1] == [] and bc.waived_lines([], [], [])[1] == []


def test_the_bases_progress_lines_reach_the_gates_log_as_they_are_printed(tmp_path, capsys):
    """run 5's bench-base: the base measured in a subprocess whose stderr was captured, so 25 min of stage lines showed the gate's watchdog only
    `base counts`, and it killed the step after a quiet stretch. The lines are passed on as they come."""
    tree = tmp_path / "tree"
    (tree / "tools/bench").mkdir(parents=True)
    (tree / "tools/bench/bench.py").write_text(
        "import sys, json\nprint('bench: [1s] O2: built 3 programs', file=sys.stderr, flush=True)\n"
        "out = sys.argv[sys.argv.index('--json') + 1]\nopen(out, 'w').write(json.dumps({'fib/c/O2': {'instructions': 1}}))\n"
    )
    rows, _ = bc.measure_in_subprocess(tmp_path / "x" / "release", ["O2"], tmp_path / "work", tree=tree)
    assert capsys.readouterr().err.strip() == "bench: [1s] O2: built 3 programs" and rows == {("fib", "c", "O2"): {"instructions": 1}}


def test_the_base_bench_runs_with_the_fork_start_method_whatever_the_pythons_default(tmp_path, monkeypatch):
    """Run 6's bench-base: Python 3.14 on the runner defaults to forkserver, and the base's nested process pools sat at 0 CPU for 10 min until
    the gate killed the step (here, forced to forkserver, the same phase ran 3x slower). The base bench starts with `fork`, as it was written."""
    default = tmp_path / "py314"
    default.mkdir()
    (default / "sitecustomize.py").write_text("import multiprocessing\nmultiprocessing.set_start_method('forkserver', force=True)\n")
    monkeypatch.setenv("PYTHONPATH", str(default))
    tree = tmp_path / "tree"
    (tree / "tools/bench").mkdir(parents=True)
    (tree / "tools/bench/bench.py").write_text(
        "import sys, json, multiprocessing\nout = sys.argv[sys.argv.index('--json') + 1]\n"
        "open(out, 'w').write(json.dumps({f'start/{multiprocessing.get_start_method()}/O2': {'instructions': 1}}))\n"
    )
    rows, _ = bc.measure_in_subprocess(tmp_path / "x" / "release", ["O2"], tmp_path / "work", tree=tree)
    assert list(rows) == [("start", "fork", "O2")]


def test_an_exception_file_prints_its_rows_every_run_and_fails_a_row_no_longer_worse_than_the_base():
    """A TEMPORARY trade-off exception (`exception = true`) stayed as a quiet BASE-STALE line after the fix it waited for landed, and no run
    named its rows. It lists each row with the base's count and the one it may reach, and a row that is no longer worse is a problem."""
    base = {FIB: row(100)}
    waivers = [{"label": "fib bq32 -O2", "counter": "instructions", "to": 120, "reason": "r", "file": "a.toml", "exception": True}]
    rows = bc.waived_rows({FIB: row(120)}, base, waivers)
    lines, problems = bc.waived_lines(rows, waivers, [])
    assert "EXCEPTION a.toml: fib bq32 -O2 instructions 100 -> 120" in lines and problems == []
    worse, better, stale = bc.judge({FIB: row(100)}, base, waivers)
    lines, problems = bc.waived_lines(bc.waived_rows({FIB: row(100)}, base, waivers), waivers, stale)
    assert problems == ["EXCEPTION a.toml: fib bq32 -O2 instructions to 120 is no longer worse than the base: delete it"]
    plain = [{**waivers[0], "exception": False}]
    assert bc.waived_lines([], plain, bc.judge({FIB: row(100)}, base, plain)[2])[1] == []


def test_the_two_jpeg_nib_os_rows_the_loopsimplify_fix_moves_are_excused_by_their_file():
    """release/next carried no waiver for jpeg_fixed / jpeg_single nib -Os code_bytes: the judge reported exactly these two. The file now excuses
    each at its own ceiling and not one byte above. (It pinned the first ceilings, 5362 / 5223, and went red when d846b3d0f0 lowered them to the
    measured 5337 / 5198.)"""
    rows = {("jpeg_fixed", "nib", "Os"): 5304, ("jpeg_single", "nib", "Os"): 5172}
    ceiling = {w["label"]: w["to"] for w in bc.waivers() if w["counter"] == "code_bytes"}
    assert {"jpeg_fixed nib -Os", "jpeg_single nib -Os"} <= set(ceiling)
    base = {key: row(100, code_bytes=size) for key, size in rows.items()}
    at = {key: row(100, code_bytes=ceiling[bc.label(key)]) for key in rows}
    over = {key: row(100, code_bytes=ceiling[bc.label(key)] + 1) for key in rows}
    assert bc.judge(at, base, bc.waivers())[0] == []
    assert len(bc.judge(over, base, bc.waivers())[0]) == 2


def test_two_runs_of_one_compiler_that_differ_in_any_count_are_named_and_equal_runs_pass():
    """Main read 78 instructions in one run and 2 in another (qlight c -Os, two programs); the baseline was trusted without a second look."""
    run = {FIB: row(100), ("fib", "c", "O2"): row(50, code_bytes=80), ("fib", "ow", "O2"): row(9)}
    assert bc.unstable_cells(run, dict(run)) == []
    flipped = {**run, ("fib", "c", "O2"): row(50, code_bytes=82), ("fib", "ow", "O2"): row(10)}
    assert bc.unstable_cells(run, flipped) == ["UNSTABLE fib c -O2: code_bytes 80 vs 82"], "the reference compilers do not move with ours and are left out"


def test_a_base_measured_on_other_sources_than_the_heads_is_refused_and_rebased_with_the_overlay():
    """The m16 list took main's counts from main's own tree: parity/qlight c read 15 clocks (constant-argument source) against 460 on the head's
    source, and 'regression' went out. An edited program without base-compiler rows on the head's sources is refused; with them its rows replace the base's."""
    base = {"q/c/O2": {"clocks": 15}, "s/c/O2": {"clocks": 7}}
    was, now = {"q": "a", "s": "b"}, {"q": "z", "s": "b"}
    with pytest.raises(SystemExit, match="q"):
        bc.rebased(base, {}, was, now)
    assert bc.rebased(base, {"q/c/O2": {"clocks": 460}}, was, now) == {"s/c/O2": {"clocks": 7}, "q/c/O2": {"clocks": 460}}
    assert bc.rebased(base, {}, was, was) == base, "unchanged sources: the base stands, no overlay needed"
