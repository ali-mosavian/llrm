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


SCALAR_BASE = {"bq16": 165, "bq32": 159, "c": 66, "nib": 69}


def test_the_scalar_o2_waiver_passes_the_unrolled_size_and_fails_one_byte_more():
    """parity/scalar -O2 went from main's loop to the pin's full unroll (bq16 165 B to 353): waived at exactly that size, so one more byte fails."""
    waivers = bc.waivers()
    base = {("parity/scalar", lang, "O2"): {"code_bytes": was} for lang, was in SCALAR_BASE.items()}
    at = {"bq16": 353, "bq32": 249, "c": 249, "nib": 252}
    for lang, size in at.items():
        key = ("parity/scalar", lang, "O2")
        assert bc.judge({key: {"code_bytes": size}}, base, waivers)[0] == [], lang
        worse = bc.judge({key: {"code_bytes": size + 1}}, base, waivers)[0]
        assert worse == [(f"parity/scalar {lang} -O2", "code_bytes", SCALAR_BASE[lang], size + 1)], lang


def test_the_scalar_waivers_pass_their_limits_and_fail_one_more():
    """parity/scalar bq16 data 21 to 23 at -O2 and -Os, and -Os code 165 to 171 (LU2 +3, DAG default +4, the cint(val) data cell): waived at exactly those counts."""
    waivers = bc.waivers()
    for opt, rows in {"O2": {"data_bytes": (21, 23)}, "Os": {"code_bytes": (165, 171), "data_bytes": (21, 23)}}.items():
        key = ("parity/scalar", "bq16", opt)
        base = {key: {counter: was for counter, (was, _) in rows.items()}}
        assert bc.judge({key: {counter: to for counter, (_, to) in rows.items()}}, base, waivers)[0] == [], opt
        for counter, (was, to) in rows.items():
            at = {c: t for c, (_, t) in rows.items()} | {counter: to + 1}
            assert bc.judge({key: at}, base, waivers)[0] == [(f"parity/scalar bq16 -{opt}", counter, was, to + 1)], (opt, counter)


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
