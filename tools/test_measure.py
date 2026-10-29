"""measure.py: a rise past tolerance against the merge-base's measurement fails, nothing the branch does touches a shared file."""
import importlib
import importlib.util
import os
import subprocess
import sys
import re
import tomllib
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
measure = importlib.import_module("measure")
# The general limits: the rows the user accepted a rise on are tested apart.
TOL = {name: limit for name, limit in measure.tolerances().items() if name != "accepted"}


@pytest.fixture(autouse=True)
def room(monkeypatch):
    """The tests build into a temporary directory: whatever is free there is not what they are about."""
    monkeypatch.setenv("LLRM_MIN_FREE_GB", "0")


def made(compile=None, axes=None, passes=None):
    return {"method": "m", "compile": compile or {}, "axes": axes or {}, "passes": passes or {}}


def files(**per_file):
    return {f"{name} {level}": n for name, n in per_file.items() for level in ("-O1", "-O2", "-O3", "-Os")}


def test_a_branch_equal_to_its_base_passes():
    base = made(files(a=1000, b=2000), {"functions O2": [50, 100, 210]}, {"a O2 x": [5.0, 10.0, 30.0, 300.0]})
    assert measure.rises(base, base, TOL)[1] == []


def test_one_file_costing_much_more_fails_though_the_geomean_barely_moves():
    """66 files, one 10% dearer: the geomean moves 0.15%, which a geomean limit alone passes."""
    base = made(files(**{f"p{i}": 1000 for i in range(66)}))
    now = made(dict(base["compile"]) | {"p0 -O2": 1100})
    assert any("p0 -O2" in line for line in measure.rises(base, now, TOL)[1])


def test_a_file_dearer_only_at_o3_fails():
    """Recursion inlining admitted to gcc's -O3 growth made quicksort's -O3 compile x9.6 (205M -> 1,972M instructions): -O3 was not one of the
    levels measured, so the gate passed it."""
    base = made(files(**{f"p{i}": 1000 for i in range(66)}))
    now = made(dict(base["compile"]) | {"p0 -O3": 9600})
    assert any("p0 -O3" in line for line in measure.rises(base, now, TOL)[1])


def test_every_file_a_little_dearer_fails_on_the_geomean():
    base = made(files(**{f"p{i}": 1000 for i in range(66)}))
    now = made({k: v + 10 for k, v in base["compile"].items()})  # +1% each, none past the worst limit
    assert any("geomean" in line for line in measure.rises(base, now, TOL)[1])


def test_a_drop_is_not_a_failure_and_is_not_recorded_anywhere():
    """A baseline in the repository went stale downward and had to be refreshed by hand in every branch; the next branch's base has it."""
    base = made(files(a=1000, b=2000), {"functions O2": [50, 100, 210]}, {"a O2 x": [5.0, 10.0, 30.0, 300.0]})
    now = made({k: v // 2 for k, v in base["compile"].items()}, {"functions O2": [50, 100, 200]}, {"a O2 x": [5.0, 10.0, 20.0, 300.0]})
    assert measure.rises(base, now, TOL)[1] == []


def cost(a=0.0, b=0.0, k=0.0):
    """The costs at N/2, N and 2N of a + bN + kN^2 with N = 100."""
    return [a + b * n + k * n * n for n in (50, 100, 200)]


def test_an_axis_that_gains_superlinear_work_fails():
    """+10% of the base's cost at 2N in an N^2 term is a pass gone quadratic."""
    base = made(axes={"live O2": cost(100, 1000, 1)})
    assert measure.rises(base, made(axes={"live O2": cost(100, 1000, 1.001)}), TOL)[1] == []
    assert any("live O2" in line for line in measure.rises(base, made(axes={"live O2": cost(100, 1000, 1.4)}), TOL)[1])


def test_a_saving_of_fixed_or_linear_cost_does_not_fail_an_axis():
    """regparm16's copyprop PR removed a fixed cost (copyprop built its universe at every call) and linear work, byte-identical, and
    the gate failed it: 2N/N rose with the denominator, and c(2N) - 2c(N) = -a + 2kN^2 rose with the fixed cost removed ('callers O1
    lir peephole' failed while its cost at 2N fell to 0.47x). Neither moves c(2N) - 3c(N) + 2c(N/2)."""
    base = made(axes={"callers O1": cost(20000, 1000, 1)})
    assert measure.rises(base, made(axes={"callers O1": cost(0, 1000, 1)}), TOL)[1] == []  # the fixed cost gone
    assert measure.rises(base, made(axes={"callers O1": cost(20000, 400, 1)}), TOL)[1] == []  # linear work gone
    assert measure.rises(base, made(axes={"callers O1": cost(0, 400, 1)}), TOL)[1] == []
    ratio = lambda c: c[2] / c[1]
    assert ratio(cost(0, 400, 1)) > ratio(cost(20000, 1000, 1)) * 1.01  # the ratio rose: what the old gate failed


def test_an_axis_whose_every_point_got_cheaper_does_not_fail_for_a_rise_of_its_second_difference():
    """#1955 (a memo that filled a slot per id below the one asked of) made `cases O1` cheaper at 64, 128 and 256 cases, by more at 128
    than at 256, so the second difference rose by 0.0129 of the base's cost at 2N and the gate read a rise: 'a step is superlinear
    where it was not'. The claim is that no step became worse at scale; a row where each point fell has nothing worse in it."""
    base = made(axes={"cases O1": [542.4, 1079.1, 2214.1]})
    assert measure.rises(base, made(axes={"cases O1": [538.9, 1063.4, 2202.5]}), TOL)[1] == []
    gone_quadratic = made(axes={"cases O1": [538.9, 1063.4, 2260.0]})
    assert any("cases O1" in line for line in measure.rises(base, gone_quadratic, TOL)[1]), "a rise with the cost at 2N higher still fails"


def test_a_linear_addition_fails_an_axis_by_its_cost_at_2n_alone():
    base = made(axes={"straight O2": cost(0, 1000)})
    assert any("cost at 2N" in line for line in measure.rises(base, made(axes={"straight O2": cost(0, 1100)}), TOL)[1])


def test_a_step_is_judged_against_the_base_by_its_second_difference_with_the_edge_free():
    whole = 300.0
    step = lambda *c: [*c, whole]
    base = made(passes={"a O2 x": step(*cost(5, 0.1, 0.0001)), "b O2 y": step(*cost(3, 0.05, 0.0001))})
    assert measure.rises(base, made(passes={"a O2 x": step(*cost(5, 0.1, 0.00011))}), TOL)[1] == []  # within step_excess
    assert any("a O2 x" in line for line in measure.rises(base, made(passes={"a O2 x": step(*cost(5, 0.1, 0.0004))}), TOL)[1])  # gone quadratic
    assert measure.rises(base, made(passes={"a O2 x": step(*cost(0, 0.05, 0.0001))}), TOL)[1] == []  # fixed and linear cost gone
    new = lambda share, k: made(passes={"c O2 z": [*cost(0, 0, k), whole * share * 0 + whole]})
    assert measure.rises(base, made(), TOL)[1] == []  # gone
    assert any("c O2 z" in line for line in measure.rises(made(), new(1, 0.001), TOL)[1])  # new, quadratic, big
    assert measure.rises(made(), made(passes={"c O2 z": [*cost(0, 0.01, 0), 1.0]}), TOL)[1] == []  # new, linear


def test_two_branches_from_one_base_write_nothing_in_the_repository(tmp_path, monkeypatch):
    """Both used to rewrite tools/gate/*.json, and every merge made every other branch conflict on them."""
    monkeypatch.setattr(measure, "CACHE", tmp_path / "cache")
    before = subprocess.run(["git", "status", "--porcelain"], cwd=measure.ROOT, capture_output=True, text=True, check=True).stdout
    one = measure.save("a" * 40, made(files(a=1)))
    two = measure.save("b" * 40, made(files(a=2)))
    assert one != two and one.parent == two.parent == tmp_path / "cache"
    after = subprocess.run(["git", "status", "--porcelain"], cwd=measure.ROOT, capture_output=True, text=True, check=True).stdout
    assert before == after


def test_a_missing_base_is_built_and_measured_once_and_then_read(tmp_path, monkeypatch):
    monkeypatch.setattr(measure, "CACHE", tmp_path / "cache")
    monkeypatch.setattr(measure, "method", lambda: "m")
    calls = []
    monkeypatch.setattr(measure, "measure_all", lambda jobs: calls.append(os.environ.get("LLRM_BIN")) or made(files(a=1)))
    built = lambda sha: tmp_path / f"bin-{sha}"
    first = measure.base_measurement("c" * 40, 1, build=built)
    second = measure.base_measurement("c" * 40, 1, build=built)
    assert first == second == made(files(a=1)) and calls == [str(tmp_path / ("bin-" + "c" * 40))]
    assert "LLRM_BIN" not in os.environ or os.environ["LLRM_BIN"] != calls[0]


def test_a_measurement_by_another_method_is_not_used(tmp_path, monkeypatch):
    monkeypatch.setattr(measure, "CACHE", tmp_path / "cache")
    measure.save("d" * 40, {**made(files(a=1)), "method": "old"})
    monkeypatch.setattr(measure, "method", lambda: "new")
    monkeypatch.setattr(measure, "measure_all", lambda jobs: made(files(a=2)) | {"method": "new"})
    assert measure.base_measurement("d" * 40, 1, build=lambda sha: tmp_path)["compile"] == files(a=2)


def test_a_commit_that_is_the_reference_itself_is_compared_with_its_parent():
    head = measure.git("rev-parse", "HEAD")
    assert measure.base_of(head, head) == measure.git("rev-parse", "HEAD^1")


def test_a_base_is_built_the_way_the_gate_builds(tmp_path, monkeypatch):
    """`cargo build --bins` alone makes a different llrm-c from the gate's build (md5 and, for one step, 3.93 against 4.18 at 2N/N): a
    base built the other way would be measured with another compiler than the branch."""
    ran = []
    monkeypatch.setattr(measure, "PROFILE", "release")
    monkeypatch.setattr(measure, "BUILD", tmp_path)
    monkeypatch.setattr(measure, "CACHE", tmp_path / "cache")
    monkeypatch.setattr(measure, "checked_out", lambda sha, tree, source=None: tmp_path)
    monkeypatch.setattr(measure.subprocess, "run", lambda command, **kw: ran.append(command) or subprocess.CompletedProcess(command, 0, "", ""))
    assert measure.built("0" * 40) == tmp_path / "snapshots" / ("release" + "0" * 12)
    assert ran == [["bash", "-c", measure.gate.MEASURE_BUILD]]


def test_two_sessions_missing_the_same_base_build_it_once(tmp_path, monkeypatch):
    """Two PRs gated at once on one new base both built it into the one shared tree and wrote the file twice."""
    import threading
    import time

    monkeypatch.setattr(measure, "CACHE", tmp_path / "cache")
    monkeypatch.setattr(measure, "method", lambda: "m")
    built, answers = [], []

    def build(sha):
        built.append(sha)
        time.sleep(0.3)  # the other session arrives while this one builds
        return tmp_path

    monkeypatch.setattr(measure, "measure_all", lambda jobs: made(files(a=1)))
    threads = [threading.Thread(target=lambda: answers.append(measure.base_measurement("e" * 40, 1, build=build))) for _ in range(3)]
    for one in threads:
        one.start()
    for one in threads:
        one.join()
    assert len(built) == 1 and answers == [made(files(a=1))] * 3
    assert not list((tmp_path / "cache").glob("*.part")), "a half-written file was left"


def test_ten_steps_each_inside_the_tolerance_fail_against_the_anchor():
    """Each of ten commits adds 0.2% to every file: inside the 0.3% geomean tolerance against its parent, 2% against the anchor."""
    chain = [made(files(**{f"p{i}": int(1000 * 1.002**step) for i in range(20)})) for step in range(11)]
    assert all(measure.rises(a, b, TOL)[1] == [] for a, b in zip(chain, chain[1:]))
    assert any("geomean" in line for line in measure.rises(chain[0], chain[-1], TOL)[1])


def _repo(path, hours):
    env = {**os.environ, "GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@t", "GIT_COMMITTER_NAME": "t", "GIT_COMMITTER_EMAIL": "t@t"}
    subprocess.run(["git", "init", "-q", str(path)], check=True)
    for n, hour in enumerate(hours):
        when = f"{1_700_000_000 + hour * 3600} +0000"
        subprocess.run(["git", "commit", "-q", "--allow-empty", "-m", f"c{n}"], cwd=path, check=True, env={**env, "GIT_AUTHOR_DATE": when, "GIT_COMMITTER_DATE": when})
    return measure.git("rev-parse", "HEAD", cwd=path)


def test_the_anchor_is_fifty_merges_back_or_a_week_back_whichever_is_nearer(tmp_path):
    busy = _repo(tmp_path / "busy", range(80))  # 80 commits an hour apart: 50 back is nearer than a week
    assert measure.git("log", "-1", "--format=%s", measure.anchor_of(busy, cwd=tmp_path / "busy"), cwd=tmp_path / "busy") == "c29"
    quiet = _repo(tmp_path / "quiet", [day * 48 for day in range(10)])  # two days apart: a week back (4 commits) is nearer than 50
    assert measure.git("log", "-1", "--format=%s", measure.anchor_of(quiet, cwd=tmp_path / "quiet"), cwd=tmp_path / "quiet") == "c5"
    short = _repo(tmp_path / "short", [0, 1])  # fewer commits than either: the first
    assert measure.git("log", "-1", "--format=%s", measure.anchor_of(short, cwd=tmp_path / "short"), cwd=tmp_path / "short") == "c0"


def test_a_concave_base_made_linear_has_not_got_worse_and_a_real_n_squared_has():
    """'straight O1/O2 lir peephole' had D = -21.5 on main (its thrash ramp is concave up to a cap) and +8.2 on a branch whose step is
    nearly linear with a 2N cost 0.90x: the rise of D from -21.5 to +8.2 failed it. Only the superlinear work above nothing counts."""
    whole = 300.0
    step = lambda *c: [*c, whole]
    concave = made(passes={"s O2 x": step(*cost(5, 1.0, -0.0005))}, axes={"s O2": cost(0, 1000, -0.5)})
    linear = made(passes={"s O2 x": step(*cost(5, 0.9, 0))}, axes={"s O2": cost(0, 900, 0)})
    assert measure.rises(concave, linear, TOL)[1] == []
    quadratic = made(passes={"s O2 x": step(*cost(5, 0.9, 0.0004))}, axes={"s O2": cost(0, 900, 1.0)})
    bad = measure.rises(concave, quadratic, TOL)[1]
    assert any("s O2 x" in line for line in bad) and any("s O2:" in line for line in bad)


def test_a_flagged_row_names_the_numbers_it_compared(capsys):
    """regparm16 read 'live O1/O2 lir peephole 2N x1.066' from `check` and x1.027 from `show` on what it took to be the same
    measurements: the line gave a ratio and not the three costs and the base's, so which side was read wrong could not be told."""
    base = made(passes={"live O1 lir peephole": [52.2, 91.4, 169.6, 6000.0]})
    now = made(passes={"live O1 lir peephole": [52.5, 93.3, 180.8, 6000.0]})
    lines, bad = measure.rises(base, now, TOL)
    assert any("52.5/93.3/180.8 against 52.2/91.4/169.6" in line for line in bad + lines), (lines, bad)


def test_compare_reads_two_stored_measurements_and_agrees_with_rises(monkeypatch, capsys):
    """`check` measures and compares; `compare` only compares what is stored, by the same `rises`, so the two cannot read one pair of
    measurements differently."""
    base = made(passes={"live O1 lir peephole": [52.2, 91.4, 169.6, 6000.0]})
    now = made(passes={"live O1 lir peephole": [52.5, 93.3, 180.8, 6000.0]})
    store = {"b" * 40: base, "c" * 40: now}
    monkeypatch.setattr(measure, "git", lambda *a, **k: a[-1] * 40 if len(a[-1]) == 1 else a[-1])
    monkeypatch.setattr(measure, "method", lambda: "m")
    monkeypatch.setattr(measure, "stored", lambda sha, which: store.get(sha))
    assert measure.compare("b", "c") == 1
    assert "52.5/93.3/180.8 against 52.2/91.4/169.6" in capsys.readouterr().out
    assert measure.compare("b", "b") == 0


def test_two_clones_do_not_share_the_tree_the_base_is_built_in(tmp_path):
    """regparm16's `git checkout --detach <its commit>` failed in a tree another session's clone had made ('unable to read tree')."""
    a, b = measure.build_tree(tmp_path / "a", {}), measure.build_tree(tmp_path / "b", {})
    assert a != b and a.parent == b.parent and a == measure.build_tree(tmp_path / "a", {})
    assert measure.build_tree(tmp_path / "a", {"LLRM_MEASURE_BUILD": "/elsewhere"}) == Path("/elsewhere")


def test_worktrees_of_one_repository_share_the_tree_the_base_is_built_in_and_clones_do_not(tmp_path):
    """The build tree was keyed by the working tree's path: 48 worktrees of one repository made 48 trees of 10-37 GB, 489 GB, and filled
    /home. A commit made in one worktree is in all of them, so one tree serves them; a clone has other objects and keeps its own."""
    def sh(*args, cwd):
        return subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", *args], cwd=cwd, check=True, capture_output=True, text=True).stdout.strip()

    main = tmp_path / "main"
    main.mkdir()
    sh("init", "-q", "-b", "main", cwd=main)
    (main / "a").write_text("1")
    sh("add", "a", cwd=main)
    sh("commit", "-q", "-m", "one", cwd=main)
    sh("worktree", "add", "-q", "--detach", str(tmp_path / "wt"), cwd=main)
    sh("clone", "-q", str(main), str(tmp_path / "clone"), cwd=tmp_path)
    here, there, apart = (measure.build_tree(tmp_path / d, {}) for d in ("main", "wt", "clone"))
    assert here == there and here != apart


def test_the_build_cache_drops_the_least_recently_used_trees_past_its_cap(tmp_path):
    """489 GB of build trees filled /home and killed the gates: past the cap the trees used longest ago go, never the one in use."""
    for age, name in enumerate(["oldest", "old", "newer", "using"]):
        tree = tmp_path / name
        tree.mkdir()
        (tree / "blob").write_bytes(b"x" * 1000)
        (tree / ".used").touch()
        os.utime(tree / ".used", (1_000_000 + age, 1_000_000 + age))
    gone = measure.pruned(tmp_path, tmp_path / "using", 2500)
    assert [t.name for t in gone] == ["oldest", "old"]
    assert sorted(t.name for t in tmp_path.iterdir()) == ["newer", "using"]
    assert measure.pruned(tmp_path, tmp_path / "using", 10**9) == []


def test_the_build_cache_cap_defaults_to_no_more_than_a_third_of_what_filled_the_disk():
    """The cap was 120 GB and 13 trees (92 GB, 48 of them stale) sat in ~/.cache/llrm/measure-build while /home fell to 289 GB free."""
    assert measure.DEFAULT_BUILD_CAP_GB <= 40
    assert measure.BUILD_CAP == int(float(os.environ.get("LLRM_MEASURE_BUILD_CAP_GB") or measure.DEFAULT_BUILD_CAP_GB) * 2**30)


def test_a_commit_only_the_remote_has_is_checked_out_in_a_clone_measure_owns(tmp_path):
    """'unable to read tree': the build tree was a worktree of another session's repository. It is a clone measure.py makes of this
    repository, and a commit this repository has not got is fetched from its origin."""
    def sh(*args, cwd):
        return subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", *args], cwd=cwd, check=True, capture_output=True, text=True).stdout.strip()

    origin = tmp_path / "origin"
    origin.mkdir()
    sh("init", "-q", "-b", "main", cwd=origin)
    (origin / "a").write_text("1")
    sh("add", "a", cwd=origin)
    sh("commit", "-q", "-m", "one", cwd=origin)
    local = tmp_path / "local"
    sh("clone", "-q", str(origin), str(local), cwd=tmp_path)
    (origin / "a").write_text("2")
    sh("commit", "-q", "-am", "two", cwd=origin)
    only_remote = sh("rev-parse", "HEAD", cwd=origin)
    assert subprocess.run(["git", "cat-file", "-e", only_remote], cwd=local).returncode != 0
    tree = measure.checked_out(only_remote, tmp_path / "build" / "tree", local)
    assert (tree / "a").read_text() == "2" and sh("rev-parse", "HEAD", cwd=tree) == only_remote
    assert measure.checked_out(sh("rev-parse", "HEAD~1", cwd=origin), tree, local) == tree and (tree / "a").read_text() == "1"


def test_check_names_the_binary_it_measured_on_a_pass_as_on_a_failure(monkeypatch, capsys):
    """regparm16 read a passing `check` and could not tell what it had measured: a pass is evidence only if we know what was measured.
    The binary is on the first line and again on the verdict, whichever it is."""
    base = made(files(a=1000))
    for now_files, verdict in ((files(a=1000), 0), (files(a=2000), 1)):
        now = made(now_files) | {"binary": {"path": "/x/llrm-cc", "sha": "abc123def456"}}
        monkeypatch.setattr(measure, "measure_all", lambda jobs, now=now: now)
        monkeypatch.setattr(measure, "git", lambda *a, **k: "f" * 40)
        monkeypatch.setattr(measure, "base_of", lambda head, ref: "e" * 40)
        monkeypatch.setattr(measure, "base_measurement", lambda sha, jobs, build=None: base)
        monkeypatch.setattr(measure, "stored", lambda sha, which: {})
        assert measure.check(1, "origin/main") == verdict
        out = capsys.readouterr().out
        assert out.splitlines()[0].startswith("measured /x/llrm-cc (abc123def456)")
        assert "abc123def456" in out.splitlines()[-1] or verdict == 1


def test_the_cost_step_does_not_build_a_base_that_has_no_llrm_cc(monkeypatch):
    """Gate run 3's cost step: the base was main (llrm-c only) and `cargo build --bin llrm-cc` died with "no bin target named llrm-cc"; the
    base is now the nearest first-parent ancestor that has llrm-cc, and none is a named skip, not a build failure."""
    with_cc = {"c" * 40, "d" * 40}
    monkeypatch.setattr(measure, "has_cc", lambda sha: sha in with_cc)
    monkeypatch.setattr(measure, "git", lambda *a, **k: "\n".join(["h" * 40, "b" * 40, "c" * 40, "d" * 40]))
    assert measure.base_with_cc("h" * 40, "m" * 40) == "c" * 40
    assert measure.base_with_cc("h" * 40, "d" * 40) == "d" * 40
    monkeypatch.setattr(measure, "has_cc", lambda sha: False)
    assert measure.base_with_cc("h" * 40, "m" * 40) is None


def test_a_measurement_is_of_one_profile_and_the_dist_base_is_built_with_the_dist_profile(tmp_path, monkeypatch):
    """The creep run on main measures the shipped build (`dist`: one codegen unit, fat LTO, 11-13% fewer instructions than `release`); a
    release measurement is not comparable with it, and its base must be built the same way."""
    monkeypatch.setattr(measure, "PROFILE", "release")
    release = measure.method()
    monkeypatch.setattr(measure, "PROFILE", "dist")
    assert measure.method() != release
    ran = []
    monkeypatch.setattr(measure, "BUILD", tmp_path)
    monkeypatch.setattr(measure, "CACHE", tmp_path / "cache")
    monkeypatch.setattr(measure, "checked_out", lambda sha, tree, source=None: tmp_path)
    monkeypatch.setattr(measure.subprocess, "run", lambda command, **kw: ran.append(command) or subprocess.CompletedProcess(command, 0, "", ""))
    assert measure.built("0" * 40) == tmp_path / "snapshots" / ("dist" + "0" * 12)
    assert ran == [["bash", "-c", "env -u RUSTFLAGS cargo build --profile dist -q --bin llrm-cc --bin llrm-llc"]], "llrm-cc and llrm-llc only: the other binaries measure nothing"


def test_a_flagged_step_names_the_steps_of_its_axis_that_rose():
    """Work moved into an engine that runs inside another step's span flags that step though the total falls (regparm16, three times);
    the flag now says where the rise is."""
    whole = 300.0
    base = made(passes={"live O1 lir peephole": [5.0, 10.0, 20.0, whole], "live O1 regalloc engine": [5.0, 10.0, 20.0, whole], "live O1 isel": [5.0, 10.0, 20.0, whole]})
    now = made(passes={"live O1 lir peephole": [5.0, 10.0, 22.0, whole], "live O1 regalloc engine": [5.0, 10.0, 17.0, whole], "live O1 isel": [5.0, 10.0, 21.0, whole]})
    bad = measure.rises(base, now, TOL)[1]
    assert any("lir peephole" in line and "regalloc engine" not in line.split(";")[0] and "isel +1.0 Minstr" in line for line in bad), bad


def test_work_moved_into_a_new_row_is_judged_by_the_axis_not_by_the_row():
    """A value solve moved out of call-effects and points-to into an analysis of its own ('callers O2 analysis pointer-values'): the
    base has no such row, so the step rule took it to have had no superlinear work and failed the PR, though the axis total fell and
    the compile got cheaper. A row absent from the base has no step baseline: only the axis it belongs to can judge it."""
    whole = 1000.0
    quad = lambda k, share=1.0: [*cost(0, 0, k), whole]
    base = made(axes={"callers O2": cost(0, 100, 0.02)}, passes={"callers O2 analysis call-effects": quad(0.02)})
    moved = made(
        axes={"callers O2": cost(0, 100, 0.0199)},
        passes={"callers O2 analysis call-effects": quad(0.0001), "callers O2 analysis pointer-values": quad(0.0198)},
    )
    assert measure.rises(base, moved, TOL)[1] == []


def test_a_new_row_on_an_axis_whose_superlinear_work_rose_still_fails():
    whole = 1000.0
    quad = lambda k: [*cost(0, 0, k), whole]
    base = made(axes={"callers O2": cost(0, 100, 0.02)}, passes={"callers O2 analysis call-effects": quad(0.02)})
    grown = made(
        axes={"callers O2": cost(0, 100, 0.06)},
        passes={"callers O2 analysis call-effects": quad(0.02), "callers O2 analysis pointer-values": quad(0.04)},
    )
    assert measure.rises(base, grown, TOL)[1] != []


def test_a_row_the_user_accepted_a_rise_on_passes_up_to_its_ceiling_and_fails_above_it():
    """The header copy's compile cost was accepted on named rows: `[[measure.accepted]]` gives a row its own ceilings, and the general
    tolerance still judges every other row and anything past the ceiling."""
    base = made(files(**{f"p{i}": 1000 for i in range(66)}), {"nest O2": cost(100, 1000, 1)}, {"nest O2 mir lsr": [5.0, 10.0, 30.0, 300.0]})
    now = made(
        {k: v * 1.1 for k, v in base["compile"].items()},
        {"nest O2": [2 * c for c in cost(100, 1000, 1)]},
        {"nest O2 mir lsr": [5.0, 10.0, 60.0, 300.0]},
    )
    assert len(measure.rises(base, now, TOL)[1]) >= 3  # the geomean, the axis and the step all fail without the entry
    limits = {"geomean": 1.1001, "worst": 1.1001}
    every = {**TOL, "accepted": [
        {"row": f"programs {level}", "why": "decision", **limits} for level in ("-O1", "-O2", "-O3", "-Os")
    ] + [{"row": "nest O2", "why": "decision", "excess": 10.0, "worst": 2.5, "step_excess": 1.0, "pass_slack": 2.5}]}
    assert measure.rises(base, now, every)[1] == []
    above = {**every, "accepted": [{**one, **({"geomean": 1.0999} if one["row"] == "programs -O2" else {})} for one in every["accepted"]]}
    assert any("programs -O2: geomean" in line for line in measure.rises(base, now, above)[1])
    steeper = made(now["compile"], {"nest O2": [2 * c for c in cost(100, 1000, 40)]}, now["passes"])
    assert any("nest O2" in line for line in measure.rises(base, steeper, every)[1])
    assert measure.row_tolerance(TOL, "nest O2 mir lsr") is TOL  # a row nothing names keeps the general limits


def test_a_new_row_of_seven_tenths_of_the_compile_on_an_axis_that_rose_still_fails():
    """The axis rule reads 1% of the base's 2N cost, the step rule 0.5% of the compile: a new row between them on an axis that rose
    (and so did not move work out of other rows) passed both. The axis judges a new row only when its second difference did not rise."""
    whole = 1000.0
    quad = lambda k: [*cost(0, 0, k), whole]
    base = made(axes={"callers O2": cost(0, 100, 0.02)}, passes={"callers O2 analysis call-effects": quad(0.02)})
    grown = made(
        axes={"callers O2": cost(0, 100, 0.020467)},
        passes={"callers O2 analysis call-effects": quad(0.02), "callers O2 analysis pointer-values": [*cost(0, 0.1, 0.000467), whole]},
    )
    bad = measure.rises(base, grown, TOL)[1]
    assert any("pointer-values" in line for line in bad), bad


def test_the_tree_key_covers_the_toolchain_because_the_tree_pins_it():
    """A measurement is stored by tree: a rustc outside the tree would make two builds of one tree differ and share a key."""
    pinned = tomllib.loads((measure.ROOT / "rust-toolchain.toml").read_text())["toolchain"]["channel"]
    assert re.fullmatch(r"\d+\.\d+\.\d+", pinned), f"rust-toolchain.toml channel {pinned!r} is not an exact version: put the rustc version in measure.method()"


def _repo_with_two_commits_of_one_tree(tmp_path):
    def run(*args, env=None):
        return subprocess.run(["git", *args], cwd=tmp_path, capture_output=True, text=True, check=True, env={**os.environ, "GIT_AUTHOR_NAME": "a", "GIT_AUTHOR_EMAIL": "a@a", "GIT_COMMITTER_NAME": "a", "GIT_COMMITTER_EMAIL": "a@a"}).stdout.strip()

    run("init", "-q", "-b", "main")
    (tmp_path / "f").write_text("one\n")
    run("add", "f")
    run("commit", "-q", "-m", "gated head")
    head = run("rev-parse", "HEAD")
    squash = run("commit-tree", f"{head}^{{tree}}", "-m", "the squash that lands it")  # another commit, the same tree
    return head, squash


def test_the_squash_that_lands_a_gated_branch_has_the_measurement_the_gate_stored(tmp_path, monkeypatch):
    """Every gate printed 'no measurement of <main tip>; building and measuring it' (~half of measure's 458 s): the tip is the squash commit,
    the measurement had been stored under the branch head's sha."""
    monkeypatch.setattr(measure, "CACHE", tmp_path / "cache")
    monkeypatch.setattr(measure, "ROOT", tmp_path)
    head, squash = _repo_with_two_commits_of_one_tree(tmp_path)
    assert head != squash
    measure.save(head, made(files(a=1)))
    which = made(files(a=1))["method"]
    assert measure.stored(squash, which) == made(files(a=1))


def test_a_gated_head_not_yet_on_the_reference_is_stored_for_the_commit_that_lands_it(tmp_path, monkeypatch):
    """`check` stored the head only when already an ancestor of origin/main, which a branch under gate never is."""
    saved = []
    now = made(files(a=1000)) | {"binary": {"path": "/x/llrm-cc", "sha": "abc123def456"}}
    monkeypatch.setattr(measure, "measure_all", lambda jobs: now)
    monkeypatch.setattr(measure, "git", lambda *a, **k: "" if a[0] == "status" else "f" * 40)
    monkeypatch.setattr(measure, "base_of", lambda head, ref: "e" * 40)
    monkeypatch.setattr(measure, "base_measurement", lambda sha, jobs, build=None: made(files(a=1000)))
    monkeypatch.setattr(measure, "stored", lambda sha, which: None)
    monkeypatch.setattr(measure, "save", lambda sha, data: saved.append(sha))
    assert measure.check(1, "origin/main") == 0
    assert saved == ["f" * 40]


def test_measure_runs_half_the_cores_of_jobs_by_default_and_what_JOBS_says_otherwise(monkeypatch):
    """`--jobs` defaulted to 4: 1249 CPU-s of measurement ran four wide on 32 cores. Instruction counts do not depend on the width."""
    monkeypatch.delenv("JOBS", raising=False)
    monkeypatch.setattr(sys, "argv", ["measure.py", "show"])
    seen = []
    monkeypatch.setattr(measure, "measure_all", lambda jobs: seen.append(jobs) or {"method": "m"})
    monkeypatch.setattr(measure.gate.os, "cpu_count", lambda: 32)
    measure.main()
    assert seen == [16]


def test_a_step_that_rose_one_and_a_half_times_fails_whatever_its_share_of_the_compile():
    """`hir to mir` of the `cases` axis went 907 -> 13,840 Minstr (x15) unseen: it was 0.4% of a compile, and a step needed `high` of the
    compile's work to fail. The share does not excuse a rise of 1.5x and 20 Minstr."""
    tol = measure.tolerances()
    key = "cases O1 hir to mir"
    base = {key: [100.0, 400.0, 1500.0, 400_000.0]}  # 0.4% of the compile at 2N
    now = {key: [400.0, 1600.0, 6000.0, 405_000.0]}  # x4: quadratic, and still 1.5% of the compile (under `high`)
    lines, bad = measure.step_rises(base, now, tol)
    assert bad and "whatever its share" in bad[0] and "x4.000" in bad[0], bad
    small = {key: [100.0, 400.0, 1500.0, 400_000.0]}
    assert measure.step_rises(base, small, tol)[1] == [], "an unchanged small step passes"
    near = {key: [100.0, 400.0, 1500.0 * 1.4, 400_000.0]}
    assert measure.step_rises(base, near, tol)[1] == [], "x1.4 is within the slack"
    tiny = {"cases O1 ra": [1.0, 4.0, 6.0, 400_000.0]}
    grown = {"cases O1 ra": [1.0, 4.0, 18.0, 400_000.0]}
    assert measure.step_rises(tiny, grown, tol)[1] == [], "x3 of 6 Minstr is +12 Minstr: under the floor, run-order noise"


def test_a_row_the_user_accepted_may_rise_past_the_step_limit_up_to_its_ceiling():
    tol = {**measure.tolerances(), "accepted": [{"row": "cases O1", "why": "decided", "step_rise": 5.0}]}
    key = "cases O1 hir to mir"
    base = {key: [100.0, 400.0, 1500.0, 400_000.0]}
    assert measure.step_rises(base, {key: [100.0, 400.0, 6000.0, 405_000.0]}, tol)[1] == []
    assert measure.step_rises(base, {key: [100.0, 400.0, 9000.0, 405_000.0]}, tol)[1] != []


def test_the_cases_regression_as_measured_fails_the_gate_at_the_size_the_gate_uses():
    """`hir to mir` on `cases` at -O1: 62, 230, 884 Minstr at 64, 128, 256 cases (measured on main after #1720/#1721; x4 per doubling), against
    the base's 907 Minstr at 1024 scaled linearly (0.886 per case). `cases` is gated at N=128 (scaling_gate.SIZES), whose 2N is 256."""
    key = "cases O1 hir to mir"
    base = {key: [56.7, 113.4, 226.8, 1800.0]}
    now = {key: [61.9, 229.7, 884.1, 2900.0]}
    assert measure.step_rises(base, now, measure.tolerances())[1], "x3.9 at 2N went unseen"
    axis = measure.axis_rises({"cases O1": [100, 200, 400]}, {"cases O1": [100, 400, 1600]}, measure.tolerances())
    assert axis[1], "its axis (the whole compile's second difference) fails as well"


def test_a_wait_for_a_lock_another_session_holds_says_so_in_the_log(tmp_path, monkeypatch, capsys):
    """#1755's measure sat 10 minutes on another session's build lock, silent and idle, and the gate killed it as stalled. On virtual
    time: the version with a thread releasing the lock after 0.6 s of the wall clock failed once under load (the waiter had not waited
    long enough when the holder let go)."""
    import fcntl

    monkeypatch.setattr(measure, "CACHE", tmp_path)
    holder = open(tmp_path / "b.lock", "w")
    fcntl.flock(holder, fcntl.LOCK_EX)
    now = [0.0]

    def sleep(seconds):
        now[0] += seconds
        if now[0] >= 150:  # the holder lets go at 150 s
            holder.close()

    with measure.locked("b", every=60, clock=lambda: now[0], sleep=sleep):
        pass
    lines = capsys.readouterr().out.splitlines()
    assert lines and lines[0].startswith("measure: waiting 60s for the lock b") and len(lines) == 2, lines


# The `analysis through-memory` step of joins at 2N=128 (#1755): 254.6 -> 282.0 Minstr under mimalloc, equal under the system allocator.
STEP = "joins O2 analysis through-memory"


def rise_under_mimalloc():
    return made(passes={STEP: [38.4, 103.8, 254.6, 9000.0]}), made(passes={STEP: [38.6, 103.6, 282.0, 9000.0]})


def test_a_step_rise_the_system_allocator_does_not_show_is_a_heap_layout_effect_and_passes():
    """#1755 failed `analysis through-memory` +27 Minstr: identical input and output, the step equal under glibc malloc."""
    base, now = rise_under_mimalloc()
    assert measure.rises(base, now, TOL)[1], "the rise is real under the shipped allocator"
    system = lambda sha, cells: {STEP: [38.4, 103.8, 254.6, 9000.0] if sha == "b" else [38.6, 103.7, 254.9, 9000.0]}
    lines, bad = measure.confirmed(measure.rises(base, now, TOL)[1], base, now, "b", "h", TOL, system)
    assert bad == [] and "not confirmed under the system allocator" in lines[0], (lines, bad)


def test_a_step_rise_that_shows_under_both_allocators_fails_and_prints_both():
    base, now = rise_under_mimalloc()
    system = lambda sha, cells: {STEP: [38.4, 103.8, 254.6, 9000.0] if sha == "b" else [38.6, 103.6, 282.0, 9000.0]}
    lines, bad = measure.confirmed(measure.rises(base, now, TOL)[1], base, now, "b", "h", TOL, system)
    assert len(bad) == 1 and "254.6" in bad[0] and "282.0" in bad[0] and "system allocator" in bad[0], bad


# #1958 and #1984 (runs 39012195202, 39012197043): `unrolled Os mir lsr` 14.587/39.482/116.837 -> 16.503/50.241/168.967 Minstr under mimalloc, 149.1 under
# the system allocator on both sides; the axis total (instructions) carried that step's rise and failed "superlinear work grew by 0.0115 (> 0.01)".
LSR = "unrolled Os mir lsr"
LSR_BASE, LSR_NOW = [14.587, 39.482, 116.837, 2050.0], [16.503, 50.241, 168.967, 2050.0]
AXIS_BASE = [200_000_000, 560_000_000, 2_050_000_000]


def lsr_case(extra=(0, 0, 0)):
    rise = [(now - was) * 1e6 for now, was in zip(LSR_NOW, LSR_BASE)]
    axis_now = [round(a + r + e) for a, r, e in zip(AXIS_BASE, rise, extra)]
    return made(axes={"unrolled Os": AXIS_BASE}, passes={LSR: LSR_BASE}), made(axes={"unrolled Os": axis_now}, passes={LSR: LSR_NOW})


def equal_under_system(sha, cells):
    return {LSR: [17.337, 48.459, 149.115, 2050.0] if sha == "b" else [17.337, 48.611, 149.065, 2050.0]}


def test_an_axis_is_judged_without_the_steps_dropped_as_heap_layout_effects():
    """#1991: the step line was dropped as 'not confirmed under the system allocator' and the axis total, which holds that step, still failed PRs that never touched it."""
    base, now = lsr_case()
    found = measure.rises(base, now, TOL)[1]
    assert any(b.startswith("unrolled Os: superlinear") for b in found) and any(b.startswith(LSR) for b in found), found
    lines, bad = measure.confirmed(found, base, now, "b", "h", TOL, equal_under_system)
    assert bad == [], bad
    assert any("heap layout effect" in line and line.startswith(LSR) for line in lines) and any(line.startswith("unrolled Os: judged without mir lsr") for line in lines), lines


def test_a_real_axis_rise_beside_a_dropped_step_still_fails():
    """Only the dropped step's rise comes out of the axis: 40 Minstr of other work at 2N stays judged."""
    base, now = lsr_case(extra=(0, 0, 40_000_000))
    found = measure.rises(base, now, TOL)[1]
    lines, bad = measure.confirmed(found, base, now, "b", "h", TOL, equal_under_system)
    assert bad and {b.split(": ")[0] for b in bad} == {"unrolled Os"} and any("superlinear work grew" in b for b in bad), bad


def test_a_rise_the_system_allocator_cannot_check_stands_and_a_clean_run_builds_nothing():
    base, now = rise_under_mimalloc()
    found = measure.rises(base, now, TOL)[1]
    assert measure.confirmed(found, base, now, "b", "h", TOL, lambda sha, cells: None)[1] == found

    def never(sha, cells):
        raise AssertionError("a run with no step rise built the system allocator's compiler")

    assert measure.confirmed([], base, now, "b", "h", TOL, never) == ([], [])
    axis_only = ["joins O2: cost at 2N=128 x2 > 1.5"]
    assert measure.confirmed(axis_only, base, now, "b", "h", TOL, never)[1] == axis_only, "an axis is judged on the shipped binary"


def test_the_system_allocator_measurement_of_a_commit_is_built_and_made_once(tmp_path, monkeypatch):
    monkeypatch.setattr(measure, "CACHE", tmp_path / "cache")
    monkeypatch.setattr(measure, "method", lambda: "m")
    monkeypatch.setattr(measure, "tree_of", lambda sha: "t" + sha)
    built, ran = [], []
    monkeypatch.setattr(measure.scaling_gate, "pass_costs", lambda axis, level, work: ran.append((axis, level)) or {f"{axis} {level} s": (1.0, 2.0, 3.0, 4.0)})
    build = lambda sha: built.append(sha) or tmp_path
    first = measure.system_passes("c" * 40, {"joins O2"}, build)
    assert measure.system_passes("c" * 40, {"joins O2"}, build) == first == {"joins O2 s": [1.0, 2.0, 3.0, 4.0]}
    assert built == ["c" * 40] and ran == [("joins", "O2")]
    measure.system_passes("c" * 40, {"joins O2", "cases O1"}, build)
    assert ran == [("joins", "O2"), ("cases", "O1")], "only the cell not yet measured is"


def test_a_row_names_the_size_it_is_at_so_2n_is_not_read_off_the_halves():
    """The log said `2N x1.108 (38.592/103.553/281.974 ...)` and was read as a rise at 4N: the three numbers are N/2, N and 2N and
    the line named no size."""
    base, now = rise_under_mimalloc()
    lines, bad = measure.rises(base, now, TOL)
    assert "2N=128" in " ".join(lines) and "2N=128" in " ".join(bad), (lines, bad)


def test_a_step_then_linear_row_is_not_a_superlinear_slope_and_a_quadratic_one_is():
    """`live O1 peephole` read 16.2/23.4/44.9 Minstr at n=32/64/128: copyprop and regthrash ran at 32 only, so the work steps and is linear
    from 64 on; its second difference (7.1) was called a slope. The last doubling is x1.92: linear."""
    tol = measure.tolerances()
    key = "live O1 lir peephole"
    base = {key: [10.0, 23.4, 44.9, 50.0]}  # the same work from n=64; at n=32 the base ran less
    step = {key: [16.2, 23.4, 44.9, 50.0]}
    assert measure.step_rises(base, step, tol)[1] == [], "a step is not a slope"
    assert measure.grown(16.2, 23.4, 44.9) == 0.0 and measure.superlinear(16.2, 23.4, 44.9) > 0
    base_linear = {key: [10.0, 20.0, 40.0, 50.0]}
    quadratic = {key: [10.0, 40.0, 160.0, 170.0]}
    assert measure.step_rises(base_linear, quadratic, tol)[1], "N^2 still fails"
    assert measure.grown(10.0, 40.0, 160.0) > 0
    mixed = {key: [10.0, 22.0, 56.0, 60.0]}  # 20% of the work at 2N is the quadratic term: x2.5 in the last doubling
    assert measure.step_rises(base_linear, mixed, tol)[1], "a real quadratic part is not hidden"

def test_a_base_is_measured_from_a_snapshot_that_the_next_build_in_the_shared_tree_does_not_change(tmp_path, monkeypatch):
    """Train 5's measure step died on `stale binaries in .../measure-build/.../release`: another base was checked out in the shared tree
    and built while this one was measured, and the release directory it measured from changed under it."""
    monkeypatch.setattr(measure, "PROFILE", "release")
    monkeypatch.setattr(measure, "BUILD", tmp_path)
    monkeypatch.setattr(measure, "CACHE", tmp_path / "cache")
    monkeypatch.setattr(measure, "checked_out", lambda sha, tree, source=None: tmp_path)
    release = tmp_path / "target" / "release"

    def build(command, **kw):
        release.mkdir(parents=True, exist_ok=True)
        (release / "llrm-c").unlink(missing_ok=True)  # cargo links a new file, not the old inode
        (release / "llrm-c").write_text(kw["env"]["WHO"])
        (release / "llrm-c.d").write_text("llrm-c: src/lib.rs\n")
        return subprocess.CompletedProcess(command, 0, "", "")

    monkeypatch.setattr(measure.subprocess, "run", build)
    monkeypatch.setenv("WHO", "first")
    first = measure.built("a" * 40)
    monkeypatch.setenv("WHO", "second")
    second = measure.built("b" * 40)
    assert (first / "llrm-c").read_text() == "first" and (second / "llrm-c").read_text() == "second" and first != second
    assert not list(first.glob("*.d")), "dep-info is not linked: nothing is compared with a tree that has moved on"


def test_the_build_lock_is_held_for_the_build_and_not_for_the_measurement(tmp_path, monkeypatch):
    """A base was measured with the lock held (a bench run took 20 minutes of it): other sessions waited on a measurement, not a build."""
    import fcntl

    monkeypatch.setattr(measure, "CACHE", tmp_path / "cache")
    monkeypatch.setattr(measure, "method", lambda: "m")
    free = []

    def measuring(jobs):
        with open(tmp_path / "cache" / "build.lock", "w") as handle:
            try:
                fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
                free.append(True)
            except BlockingIOError:
                free.append(False)
        return made(files(a=1)) | {"method": "m"}

    monkeypatch.setattr(measure, "measure_all", measuring)
    measure.base_measurement("f" * 40, 1, build=lambda sha: tmp_path)
    assert free == [True]


def test_the_gate_measures_the_dist_build_and_refuses_a_binary_of_another_profile(tmp_path, monkeypatch):
    """#1977's pair read +0.46..0.61% on its gate shapes in release (16 codegen units: an edit re-partitions and flips unrelated inlining) and
    +0.01..0.04% in dist: the compiler under measurement is the dist one, and a release llrm-c in a dist run is an error, not a number."""
    assert measure.PROFILE == "dist"
    with pytest.raises(SystemExit, match="not a dist build"):
        measure.measured_dir({"LLRM_BIN": str(tmp_path / "release")})
    assert measure.measured_dir({"LLRM_BIN": str(tmp_path / "snapshots" / ("dist" + "a" * 12))}).name.startswith("dist")
    assert measure.measured_dir({"CARGO_TARGET_DIR": str(tmp_path)}) == tmp_path / "dist"


def test_a_base_built_beside_the_head_is_used_as_it_is(monkeypatch, tmp_path):
    """CI builds the base's llrm-c in a job of its own (hosted, parallel with the head's) and hands it over: measure does not build it again on a runner with six."""
    monkeypatch.setenv("LLRM_MEASURE_BASE_BIN", str(tmp_path / "dist-base"))
    monkeypatch.setattr(measure.subprocess, "run", lambda *a, **k: (_ for _ in ()).throw(AssertionError("built the base")))
    assert measure.built("0" * 40) == tmp_path / "dist-base"


def test_the_local_dist_binary_is_built_when_it_is_missing_and_left_alone_when_it_is_there(tmp_path, monkeypatch):
    ran = []
    monkeypatch.setattr(measure.subprocess, "run", lambda command, **kw: ran.append(command) or subprocess.CompletedProcess(command, 0, "", ""))
    assert measure.ensure_dist({"CARGO_TARGET_DIR": str(tmp_path)}) == tmp_path / "dist" and len(ran) == 1
    (tmp_path / "dist").mkdir()
    (tmp_path / "dist" / "llrm-cc").write_text("")
    measure.ensure_dist({"CARGO_TARGET_DIR": str(tmp_path)})
    assert len(ran) == 1


def test_the_base_is_built_beside_the_head_not_after_it(monkeypatch):
    """Two dist builds are minutes of one thread each (fat LTO): the cost job built them one after the other, +7 minutes."""
    import threading

    base_started = threading.Event()
    monkeypatch.setattr(measure, "git", lambda *a, **k: "f" * 40)
    monkeypatch.setattr(measure, "base_of", lambda head, ref: "e" * 40)
    monkeypatch.setattr(measure, "stored", lambda sha, which: None)
    monkeypatch.setattr(measure, "measure_all", lambda jobs: made(files(a=1000)) | {"binary": {"path": "/x", "sha": "s"}})
    monkeypatch.setattr(measure, "base_measurement", lambda sha, jobs, build=None: (build(sha), made(files(a=1000)))[1])
    monkeypatch.setattr(measure, "judged", lambda now, base, base_sha, head: 0)

    def build(sha):
        base_started.set()
        return Path("/b")

    def head():
        assert base_started.wait(5), "the head was built before the base had started"

    assert measure.check(1, "origin/main", build=build, head_dir=head) == 0


def test_the_bench_base_is_built_the_release_way_whatever_the_measurement_profile_is(tmp_path, monkeypatch):
    """#2013's bench shards: 'FAIL BASE: the base could not be measured': the bench's base needs every tool of the release build, and `built` gave it llrm-c
    of the dist profile alone."""
    ran = []
    monkeypatch.setattr(measure, "PROFILE", "dist")
    monkeypatch.setattr(measure, "BUILD", tmp_path)
    monkeypatch.setattr(measure, "CACHE", tmp_path / "cache")
    monkeypatch.setattr(measure, "checked_out", lambda sha, tree, source=None: tmp_path)
    monkeypatch.setattr(measure.subprocess, "run", lambda command, **kw: ran.append(command) or subprocess.CompletedProcess(command, 0, "", ""))
    monkeypatch.setenv("LLRM_MEASURE_BASE_BIN", str(tmp_path / "dist-base"))
    assert measure.built("0" * 40, "release") == tmp_path / "snapshots" / ("release" + "0" * 12), "the dist base handed over is not the bench's"
    assert ran == [["bash", "-c", measure.gate.MEASURE_BUILD]]


def test_the_bench_asks_for_a_release_base(monkeypatch):
    spec = importlib.util.spec_from_file_location("bench_tool", HERE / "bench" / "bench.py")
    bench = importlib.util.module_from_spec(spec)
    sys.modules["bench_tool"] = bench  # its dataclasses look their module up
    spec.loader.exec_module(bench)

    seen = []
    monkeypatch.setattr(measure, "built", lambda sha, profile=None: seen.append(profile) or Path("/b"))
    monkeypatch.setattr(measure, "git", lambda *a, **k: "f" * 40)
    monkeypatch.setattr(measure, "base_of", lambda head, ref: "e" * 40)
    monkeypatch.setattr(bench.basecounts, "base_counts", lambda sha, opts, tree_of, build, measure_with, timed, only=None: (build(sha), {}) and ({}, {}))
    bench.base_rows(type("A", (), {"opt": ["O2"], "work": Path("/w"), "time": False})())
    assert seen == ["release"]


def test_the_gates_own_links_of_the_release_build_are_not_the_measured_compiler(monkeypatch, tmp_path):
    """#2013's cost job: 'target/gate-bins is not a dist build': the gate sets LLRM_BIN to its links for every step."""
    built = []
    monkeypatch.setattr(measure, "PROFILE", "dist")
    monkeypatch.setattr(measure, "ensure_dist", lambda env=None: built.append(1) or tmp_path / "dist")
    monkeypatch.setenv("LLRM_BIN", str(tmp_path / "gate-bins"))
    measure.use_dist()
    assert os.environ["LLRM_BIN"] == str(tmp_path / "dist") and built == [1]
    monkeypatch.setenv("LLRM_BIN", str(tmp_path / "snapshots" / ("dist" + "a" * 12)))
    measure.use_dist()
    assert os.environ["LLRM_BIN"].endswith("a" * 12) and built == [1], "a dist directory is kept"


def test_a_cost_run_whose_base_or_head_is_not_the_dist_build_fails_loudly_not_measures_the_wrong_thing(tmp_path, monkeypatch):
    """#2013: the head was the gate's release links. Either side of a dist measurement in another profile is an error naming it, never a number."""
    monkeypatch.setattr(measure, "PROFILE", "dist")
    monkeypatch.setattr(measure, "CACHE", tmp_path / "cache")
    monkeypatch.setattr(measure, "stored", lambda sha, which: None)
    monkeypatch.delenv("LLRM_BIN", raising=False)
    # the head: LLRM_BIN is a release directory
    monkeypatch.setenv("LLRM_BIN", str(tmp_path / "release"))
    with pytest.raises(SystemExit, match="release is not a dist build"):
        measure.measure_all(1)
    # the base: the prebuilt directory CI hands over is a release one
    monkeypatch.delenv("LLRM_BIN")
    monkeypatch.setenv("LLRM_MEASURE_BASE_BIN", str(tmp_path / "release-base"))
    with pytest.raises(SystemExit, match="release-base is not a dist build"):
        measure.base_measurement("e" * 40, 1)
    # and the dist ones are accepted as far as the check of the profile goes
    assert measure.measured_dir({"LLRM_BIN": str(tmp_path / "dist-base")}).name == "dist-base"


def test_a_build_with_too_little_room_fails_with_the_free_space_and_the_limit(tmp_path, monkeypatch):
    """/home was 100% full on tinytim and every build failed with a linker error nobody could read."""
    monkeypatch.delenv("LLRM_MIN_FREE_GB")
    monkeypatch.delenv("RUNNER_ENVIRONMENT", raising=False)  # a hosted runner is exempt, and CI is one
    monkeypatch.setattr(measure.shutil, "disk_usage", lambda path: type("U", (), {"free": 5 * 2**30})())
    with pytest.raises(SystemExit, match=r"5 GB free under .*, 100 needed"):
        measure.require_space(tmp_path / "not" / "made" / "yet")
    monkeypatch.setenv("LLRM_MIN_FREE_GB", "4")
    measure.require_space(tmp_path)


def test_a_hosted_runner_is_not_refused_for_its_78_gb(monkeypatch, tmp_path):
    """#2109's bench shards on hosted runners: 'measure: 78 GB free under .../measure-build, 100 needed': the check is for the shared host, not a fresh VM."""
    monkeypatch.delenv("LLRM_MIN_FREE_GB")
    monkeypatch.setattr(measure.shutil, "disk_usage", lambda path: type("U", (), {"free": 78 * 2**30})())
    monkeypatch.setenv("RUNNER_ENVIRONMENT", "github-hosted")
    measure.require_space(tmp_path)
    monkeypatch.setenv("RUNNER_ENVIRONMENT", "self-hosted")
    with pytest.raises(SystemExit, match="78 GB free"):
        measure.require_space(tmp_path)


def test_the_build_the_bench_takes_its_base_from_makes_every_tool_the_bench_runs():
    """#2082's bench-1: 'FileNotFoundError: .../snapshots/distc2f125dc7d60/llrm-qb': the dist snapshot is llrm-c alone. Whichever profile the bench's base is
    built in builds all the binaries (`--bins`), or names every tool the bench runs."""
    import re

    ran = set(re.findall(r'"(llrm-[a-z]+)"', (HERE / "bench" / "bench.py").read_text()))
    assert {"llrm-c", "llrm-qb", "llrm-nib"} <= ran, ran
    recipe = measure.MEASURE_BUILDS[measure.BENCH_BASE_PROFILE]
    named = set(re.findall(r"--bin (llrm-[a-z]+)", recipe))
    assert "--bins" in recipe or ran <= named, (measure.BENCH_BASE_PROFILE, recipe, ran)
    assert "--bins" not in measure.MEASURE_BUILDS["dist"], "the dist build is the compiler's cost alone: the bench does not take its base from it"


MINE = "alim/feat/gvn-scalar-pr"


def gvn_readings():
    """The T-1 classes (#2295, #2179), each as (what it is, a call that says whether it fails under a tolerance table)."""
    qcport = lambda scale: made({f"qcport/m{i} -Os": int(1_000_000 * scale) for i in range(65)})
    qbase = made({f"qcport/m{i} -Os": 1_000_000 for i in range(65)})
    step = lambda key, ratio, whole=400.0: ({key: [50.0, 100.0, 200.0, whole]}, {key: [50.0 * ratio, 100.0 * ratio, 200.0 * ratio, whole]})
    return {
        "qcport -Os geomean": (1.0140, 1.0146, lambda tol, x: measure.rises(qbase, qcport(x), tol)[1]),
        "gvn at 2N": (1.64, 1.66, lambda tol, x: measure.step_rises(*step("cells O1 mir gvn", x), tol)[1]),
        "gvn step rise": (1.58, 1.70, lambda tol, x: measure.step_rises(*step("straight O1 mir gvn", x), tol)[1]),
        "straight Os compile": (
            1.0222,
            1.0230,
            lambda tol, x: measure.axis_rises({"straight Os": [500, 1000, 2000]}, {"straight Os": [500, 1000, 2000 * x]}, tol)[1],
        ),
        "callers Os call-effects": (1.14, 1.16, lambda tol, x: measure.step_rises(*step("callers Os analysis call-effects", x, 2000.0), tol)[1]),
    }


def test_each_gvn_exception_class_passes_on_its_pr_up_to_its_ceiling_and_fails_above_it_and_elsewhere():
    """T-1 (#2295): the scalar GVN's PR (#2179) alone may read these up to the ceilings; the same reading on another head fails at the
    normal limit, and above its ceiling it fails on the PR too."""
    mine, other = measure.tolerances(branch=MINE), measure.tolerances(branch="alim/feat/something-else")
    for name, (inside, above, fails) in gvn_readings().items():
        assert not fails(mine, inside), f"{name}: {inside} is inside the ceiling on its PR"
        assert fails(other, inside), f"{name}: {inside} fails on any other head"
        assert fails(mine, above), f"{name}: {above} is above the ceiling on its PR"


def test_the_gvn_gvn_exception_leaves_every_other_step_at_the_normal_limits():
    """The suffix rule is `mir gvn`, not every step: a fold step at the same reading fails on the PR."""
    mine = measure.tolerances(branch=MINE)
    base = {"cells O1 mir fold": [50.0, 100.0, 200.0, 400.0]}
    assert measure.step_rises(base, {"cells O1 mir fold": [80.0, 160.0, 320.0, 400.0]}, mine)[1]
    assert not [one for one in measure.tolerances(branch="x")["accepted"] if "branch" in one]

def test_a_step_rise_line_says_it_is_an_own_reading_on_this_build_profile(monkeypatch):
    """#2318: the gate read gvn x1.583 where a direct reading said x1.41, and neither said what it was: the same binaries give 1.583
    (own, dist), 1.507 (total, dist), 1.523 (own, release) and 1.460 (total, release)."""
    monkeypatch.setattr(measure, "PROFILE", "dist")
    key = "straight O1 mir gvn"
    base, now = {key: [10.0, 20.0, 44.3, 400.0]}, {key: [15.0, 30.0, 70.1, 410.0]}
    lines, bad = measure.step_rises(base, now, measure.tolerances())
    assert "(own, dist)" in lines[0] and "(own, dist)" in bad[0], (lines, bad)


def test_the_gate_reads_a_step_as_the_own_column_of_its_row_not_the_total(monkeypatch, tmp_path):
    """The same compile's `[instr]` row read by the gate and by hand (`awk '{print $2}'` own, `$5` total) is one number for each definition:
    the gate's is the own column, less the empty file's."""
    rows = lambda own, total: f"[instr]  {own:8.3f} Minstr own  {total:8.3f} Minstr total  1x mir gvn\n"
    seen = {}

    def sample(command, env):
        text = Path(command[-1]).read_text()
        own = 0.1 * len(text) ** 0.5 if text else 0.0
        seen[len(text)] = rows(own, own * 1.15)
        return 0, "", seen[len(text)]

    monkeypatch.setattr(measure.scaling_gate.scaling, "sample", sample)
    got = measure.scaling_gate.pass_costs("straight", "O1", tmp_path, command=lambda compiler, level, source: ["cc", str(source)])
    big = got["straight O1 mir gvn"][2]
    by_hand = seen[max(seen)].split()  # [instr] OWN Minstr own TOTAL Minstr total ...
    assert big == float(by_hand[1]) and big != float(by_hand[4]), got
