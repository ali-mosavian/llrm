"""measure.py: a rise past tolerance against the merge-base's measurement fails, nothing the branch does touches a shared file."""
import importlib
import os
import subprocess
import sys
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
measure = importlib.import_module("measure")
TOL = measure.tolerances()


def made(compile=None, axes=None, passes=None):
    return {"method": "m", "compile": compile or {}, "axes": axes or {}, "passes": passes or {}}


def files(**per_file):
    return {f"{name} {level}": n for name, n in per_file.items() for level in ("-O1", "-O2", "-Os")}


def test_a_branch_equal_to_its_base_passes():
    base = made(files(a=1000, b=2000), {"functions O2": [50, 100, 210]}, {"a O2 x": [5.0, 10.0, 30.0, 300.0]})
    assert measure.rises(base, base, TOL)[1] == []


def test_one_file_costing_much_more_fails_though_the_geomean_barely_moves():
    """66 files, one 10% dearer: the geomean moves 0.15%, which a geomean limit alone passes."""
    base = made(files(**{f"p{i}": 1000 for i in range(66)}))
    now = made(dict(base["compile"]) | {"p0 -O2": 1100})
    assert any("p0 -O2" in line for line in measure.rises(base, now, TOL)[1])


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
    monkeypatch.setattr(measure, "BUILD", tmp_path)
    monkeypatch.setattr(measure, "checked_out", lambda sha, tree, source=None: tmp_path)
    monkeypatch.setattr(measure.subprocess, "run", lambda command, **kw: ran.append(command) or subprocess.CompletedProcess(command, 0, "", ""))
    assert measure.built("0" * 40) == tmp_path / "target" / "release"
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
        now = made(now_files) | {"binary": {"path": "/x/llrm-c", "sha": "abc123def456"}}
        monkeypatch.setattr(measure, "measure_all", lambda jobs, now=now: now)
        monkeypatch.setattr(measure, "git", lambda *a, **k: "f" * 40)
        monkeypatch.setattr(measure, "base_of", lambda head, ref: "e" * 40)
        monkeypatch.setattr(measure, "base_measurement", lambda sha, jobs: base)
        monkeypatch.setattr(measure, "stored", lambda sha, which: {})
        assert measure.check(1, "origin/main") == verdict
        out = capsys.readouterr().out
        assert out.splitlines()[0].startswith("measured /x/llrm-c (abc123def456)")
        assert "abc123def456" in out.splitlines()[-1] or verdict == 1


def test_a_measurement_is_of_one_profile_and_the_dist_base_is_built_with_the_dist_profile(tmp_path, monkeypatch):
    """The creep run on main measures the shipped build (`dist`: one codegen unit, fat LTO, 11-13% fewer instructions than `release`); a
    release measurement is not comparable with it, and its base must be built the same way."""
    release = measure.method()
    monkeypatch.setattr(measure, "PROFILE", "dist")
    assert measure.method() != release
    ran = []
    monkeypatch.setattr(measure, "BUILD", tmp_path)
    monkeypatch.setattr(measure, "checked_out", lambda sha, tree, source=None: tmp_path)
    monkeypatch.setattr(measure.subprocess, "run", lambda command, **kw: ran.append(command) or subprocess.CompletedProcess(command, 0, "", ""))
    assert measure.built("0" * 40) == tmp_path / "target" / "dist"
    assert ran == [["bash", "-c", "cargo build --profile dist -q --bins"]]


def test_a_flagged_step_names_the_steps_of_its_axis_that_rose():
    """Work moved into an engine that runs inside another step's span flags that step though the total falls (regparm16, three times);
    the flag now says where the rise is."""
    whole = 300.0
    base = made(passes={"live O1 lir peephole": [5.0, 10.0, 20.0, whole], "live O1 regalloc engine": [5.0, 10.0, 20.0, whole], "live O1 isel": [5.0, 10.0, 20.0, whole]})
    now = made(passes={"live O1 lir peephole": [5.0, 10.0, 22.0, whole], "live O1 regalloc engine": [5.0, 10.0, 17.0, whole], "live O1 isel": [5.0, 10.0, 21.0, whole]})
    bad = measure.rises(base, now, TOL)[1]
    assert any("lir peephole" in line and "regalloc engine" not in line.split(";")[0] and "isel +1.0 Minstr" in line for line in bad), bad
