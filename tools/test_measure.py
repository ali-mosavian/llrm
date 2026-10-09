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
    base = made(files(a=1000, b=2000), {"functions O2": [100, 210]}, {"a O2 x": [10.0, 30.0, 300.0]})
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
    base = made(files(a=1000, b=2000), {"functions O2": [100, 210]}, {"a O2 x": [10.0, 30.0, 300.0]})
    now = made({k: v // 2 for k, v in base["compile"].items()}, {"functions O2": [100, 200]}, {"a O2 x": [10.0, 20.0, 300.0]})
    assert measure.rises(base, now, TOL)[1] == []


def test_an_axis_that_gains_superlinear_work_fails():
    """Work at 2N beyond twice the work at N: the quadratic term. +10% of the base's cost at 2N in it is a pass gone quadratic."""
    base = made(axes={"live O2": [100, 300]})
    assert measure.rises(base, made(axes={"live O2": [100, 301]}), TOL)[1] == []
    assert any("live O2" in line for line in measure.rises(base, made(axes={"live O2": [100, 330]}), TOL)[1])


def test_a_linear_saving_does_not_fail_an_axis_whose_2n_over_n_rose():
    """regparm16's copyprop made linear work cheaper (straight -4.7%, byte-identical); the ratio 2N/N rose with the denominator
    (branches 2.826 > 2.780) and the gate failed it. Nothing got slower: the excess fell, and so did the cost at 2N."""
    base = made(axes={"branches O2": [100, 278]})
    now = made(axes={"branches O2": [90.0, 254.34]})
    assert 254.34 / 90.0 > 278 / 100 * 1.01
    assert measure.rises(base, now, TOL)[1] == []


def test_a_linear_addition_fails_an_axis_by_its_cost_at_2n_alone():
    base = made(axes={"straight O2": [100, 200]})
    assert any("cost at 2N" in line for line in measure.rises(base, made(axes={"straight O2": [110, 220]}), TOL)[1])


def test_a_step_is_judged_against_the_base_by_its_excess_with_the_edge_free():
    base = made(passes={"a O2 x": [10.0, 30.0, 300.0], "b O2 y": [3.0, 9.0, 300.0]})
    assert measure.rises(base, made(passes={"a O2 x": [10.0, 30.5, 300.0]}), TOL)[1] == []  # within step_excess
    assert any("a O2 x" in line for line in measure.rises(base, made(passes={"a O2 x": [10.0, 35.0, 300.0]}), TOL)[1])  # 5 Minstr of 300 gone quadratic
    assert measure.rises(base, made(passes={"a O2 x": [8.0, 24.0, 300.0]}), TOL)[1] == []  # a linear saving: the ratio is 3.0 again, the excess fell
    edge = made(passes={"c O2 z": [3.0, 9.0, 300.0]})  # 3% of the compile: past `high`
    assert any("c O2 z" in line for line in measure.rises(made(), made(passes={"c O2 z": [3.0, 9.0 * 300 / 300 + 3.0, 100.0]}), TOL)[1])  # new, past linear
    assert measure.rises(made(), made(passes={"c O2 z": [1.0, 3.0, 300.0]}), TOL)[1] == []  # new, under `high`
    assert measure.rises(base, made(), TOL)[1] == []  # and gone
    del edge


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
    monkeypatch.setattr(measure, "git", lambda *args, **kw: "")
    monkeypatch.setattr(measure.subprocess, "run", lambda command, **kw: ran.append(command) or subprocess.CompletedProcess(command, 0, "", ""))
    assert measure.built("0" * 40) == tmp_path / "target" / "release"
    assert ran == [["bash", "-c", measure.gate.BUILD]]


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
