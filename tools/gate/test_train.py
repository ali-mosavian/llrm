"""The merge train: batches, bounced conflicts, the split of a red batch, ENV verdicts, and that it never pushes."""

import sys
import subprocess
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import pytest
import train  # noqa: E402


@pytest.fixture(autouse=True)
def _own_logs(tmp_path, monkeypatch):
    """A test that gates writes its logs under its own directory, not the real ~/scratch/train-logs."""
    monkeypatch.setattr(train, "LOGS", tmp_path / "train-logs")

NAME = "Clau" + "de"  # spelt in halves: the smoke tier refuses a diff that carries the word


def pr(n, files=("a.rs",), sha=None):
    return train.Pr(n, sha or f"{n:040d}", list(files))


def test_batches_hold_at_most_the_limit_in_order_and_a_gate_tool_pr_goes_alone():
    prs = [pr(1), pr(2), pr(3, ["tools/gate/gate.py"]), pr(4), pr(5), pr(6)]
    got = [[p.number for p in b] for b in train.batches(prs, 2)]
    assert got == [[1, 2], [3], [4, 5], [6]]
    assert sorted(n for b in got for n in b) == [1, 2, 3, 4, 5, 6], "no PR is lost"


def _repo(tmp_path):
    def git(*args):
        return subprocess.run(["git", *args], cwd=tmp_path, capture_output=True, text=True, check=True, env={**__import__("os").environ, "GIT_AUTHOR_NAME": "a", "GIT_AUTHOR_EMAIL": "a@a", "GIT_COMMITTER_NAME": "a", "GIT_COMMITTER_EMAIL": "a@a"}).stdout.strip()

    git("init", "-q", "-b", "main")
    (tmp_path / "f").write_text("one\n")
    git("add", "f")
    git("commit", "-q", "-m", "base")
    git("update-ref", "refs/remotes/origin/main", "HEAD")
    return git


def _branch(git, tmp_path, name, path, text):
    git("checkout", "-q", "-b", name, "main")
    (tmp_path / path).write_text(text)
    git("add", path)
    git("commit", "-q", "-m", f"edit {path}")
    sha = git("rev-parse", "HEAD")
    git("checkout", "-q", "main")
    return sha


def test_a_pr_that_conflicts_with_the_batch_is_bounced_and_the_others_still_merge(tmp_path):
    git = _repo(tmp_path)
    a = _branch(git, tmp_path, "a", "f", "from a\n")
    b = _branch(git, tmp_path, "b", "f", "from b\n")  # the same line as a
    c = _branch(git, tmp_path, "c", "g", "from c\n")
    git("checkout", "-q", "--detach", "main")
    tip, bounced = train.build_tip([pr(1, sha=a), pr(2, sha=b), pr(3, sha=c)], tmp_path)
    assert [p.number for p, _ in bounced] == [2]
    assert (tmp_path / "f").read_text() == "from a\n" and (tmp_path / "g").read_text() == "from c\n"
    assert git("rev-parse", "HEAD") == tip


def test_a_green_batch_prints_the_tip_and_the_push_line_and_pushes_nothing():
    said = []
    code = train.run([pr(1), pr(2)], lambda prs: ("tipsha", []), lambda tip, steps: train.Verdict(True), lambda steps: train.Verdict(True), said.append)
    assert code == 0
    assert said == ["TRAIN PASS TIP=tipsha PRs=#1 #2", "git push origin tipsha:main"]


def test_a_red_batch_names_the_culprit_with_what_it_printed_and_lands_the_rest_gated_whole():
    gated = []

    def gate_at(tip, steps):
        gated.append((tip, steps))
        bad = "2" in tip
        return train.Verdict(not bad, ["pytest"] if bad else [], "FAILED test_x: boom" if bad else "")

    tips = lambda prs: ("tip" + "".join(str(p.number) for p in prs), [])  # noqa: E731
    said = []
    code = train.run([pr(1), pr(2), pr(3)], tips, gate_at, lambda steps: train.Verdict(True), said.append)
    assert code == 0
    assert any(s.startswith("TRAIN FAIL #2") for s in said)
    assert "TRAIN PASS TIP=tip13 PRs=#1 #3" in said, said
    assert gated[-1] == ("tip13", None), "the offered tip was gated whole, not on the failed steps"


def test_a_failure_that_origin_main_has_too_is_an_environment_verdict_blaming_no_pr():
    said = []
    code = train.run([pr(1), pr(2)], lambda prs: ("t", []), lambda tip, steps: train.Verdict(False, ["run"], "x"), lambda steps: train.Verdict(False, steps), said.append)
    assert code == 3 and said == ["TRAIN ENV: run fail on origin/main alone; no PR is blamed"]


def test_a_batch_whose_every_pr_conflicts_runs_no_gate():
    ran = []
    code = train.run([pr(1)], lambda prs: ("t", [(prs[0], "conflicts")]), lambda tip, steps: ran.append(1), lambda steps: None, lambda s: None)
    assert code == 1 and not ran


def test_preflight_names_a_missing_target_dir_and_too_little_disk(tmp_path, monkeypatch):
    assert any("CARGO_TARGET_DIR" in p for p in train.preflight(tmp_path, tmp_path, {}))
    monkeypatch.setattr(train, "MIN_FREE_GB", 10**9)
    assert any("GB free" in p for p in train.preflight(tmp_path, tmp_path, {"CARGO_TARGET_DIR": "x"}))
    monkeypatch.setattr(train, "MIN_FREE_GB", 0)
    assert train.preflight(tmp_path, tmp_path, {"CARGO_TARGET_DIR": "x"}) == []


def test_a_pr_with_no_smoke_a_foreign_base_or_the_assistants_name_does_not_board(tmp_path):
    git = _repo(tmp_path)
    sha = _branch(git, tmp_path, "a", "g", "fine\n")
    info = {"baseRefName": "main", "title": "feat: g", "body": ""}
    assert train.pr_problems(info, sha, True, tmp_path) == []
    assert any("smoke" in p for p in train.pr_problems(info, sha, False, tmp_path))
    assert any("not main" in p for p in train.pr_problems({**info, "baseRefName": "other"}, sha, True, tmp_path))
    assert any("name the assistant" in p for p in train.pr_problems({**info, "body": f"by {NAME}"}, sha, True, tmp_path))


def test_the_tip_offered_is_the_blessed_one_and_a_bless_that_fails_its_strict_gate_offers_nothing():
    said = []
    ok = lambda tip, steps: train.Verdict(True)  # noqa: E731
    code = train.run([pr(1), pr(2)], lambda prs: ("tip", []), ok, lambda s: train.Verdict(True), said.append, blessed=lambda tip, prs: ("tip+bless", train.Verdict(True)))
    assert code == 0 and said[0] == "TRAIN PASS TIP=tip+bless PRs=#1 #2" and said[1] == "git push origin tip+bless:main"
    said.clear()
    code = train.run([pr(1)], lambda prs: ("tip", []), ok, lambda s: train.Verdict(True), said.append, blessed=lambda tip, prs: ("tip+bless", train.Verdict(False, ["bench"], "FAIL x")))
    assert code == 1 and not any("TRAIN PASS" in s for s in said) and any("strict bench gate" in s for s in said)
    said.clear()
    train.run([pr(1)], lambda prs: ("tip", []), ok, lambda s: train.Verdict(True), said.append, blessed=lambda tip, prs: (tip, None))
    assert said[0] == "TRAIN PASS TIP=tip PRs=#1", "nothing got better: the gated tip is offered as it is"


def test_the_batch_gate_runs_bench_in_batch_mode_and_the_blessed_tip_is_checked_strictly(monkeypatch, tmp_path):
    seen = []

    def fake(command, **kw):
        seen.append(kw["env"].get("LLRM_BENCH_BATCH"))
        return subprocess.CompletedProcess(command, 0, "GATE full PASS at x in 1s\n", "")

    monkeypatch.setattr(train, "git", lambda *a, **k: subprocess.CompletedProcess(a, 0, "", ""))
    monkeypatch.setattr(train.subprocess, "run", fake)
    monkeypatch.setenv("LLRM_BENCH_BATCH", "stale")
    train.full_gate("tip", tmp_path)
    train.full_gate("tip", tmp_path, ["bench"], strict=True)
    assert seen == ["1", None]


def test_the_tip_and_main_are_gated_in_targets_and_logs_of_their_own(monkeypatch, tmp_path):
    """The train's tip gate and its re-run on main shared one target: main's pytest read the tip's llrm-c, which names the tip's tree
    ('train-tree/... is not in the subpath of train-main', 14 collection errors), and its gate logs overwrote the tip's failure."""
    targets = []

    def fake(command, **kw):
        targets.append(kw["env"]["CARGO_TARGET_DIR"])
        return subprocess.CompletedProcess(command, 1, "GATE full FAIL: pytest at x in 1s\n[time] pytest FAIL 1s wall 1s cpu\n", "")

    monkeypatch.setattr(train, "git", lambda *a, **k: subprocess.CompletedProcess(a, 0, "", ""))
    monkeypatch.setattr(train.subprocess, "run", fake)
    monkeypatch.setattr(train, "LOGS", tmp_path / "logs")
    monkeypatch.setenv("CARGO_TARGET_DIR", "/t/target")
    tip, main = tmp_path / "train-tree", tmp_path / "train-main"
    train.full_gate("a" * 40, tip)
    train.full_gate("b" * 40, main, ["pytest"])
    assert targets == ["/t/target-train-tree", "/t/target-train-main"]
    assert sorted(p.name for p in (tmp_path / "logs").iterdir()) == [f"{'a' * 12}-train-tree-full.log", f"{'b' * 12}-train-main-pytest.log"]


def test_a_gating_test_writes_its_logs_under_its_own_directory(tmp_path):
    """#1784's tests wrote tip-test_*.log into ~/scratch/train-logs, where the real train keeps its evidence."""
    assert train.LOGS.is_relative_to(tmp_path), train.LOGS


def test_trains_of_different_names_share_no_lock_worktree_or_target(tmp_path):
    """One train at a time left 17 PRs waiting: the lock, the tip's and main's worktrees were fixed paths, so a second train would have
    removed the first one's trees (and the target is named after the tree, so a tree of its own is a target of its own)."""
    first, second = train.places("train", tmp_path), train.places("train5", tmp_path)
    assert train.places("train", tmp_path) == (tmp_path / "scratch/train.lock", tmp_path / "scratch/train-tree", tmp_path / "scratch/train-main")
    assert not set(first) & set(second) and len({p.name for p in (*first, *second)}) == 6


def test_prs_that_bless_the_same_benchmark_do_not_bounce_each_other_and_the_batchs_file_stands(tmp_path):
    """Train 5 bounced four of its five PRs: each blessed its own counts in bench/jpeg_fixed/expected.toml, and two edits of one file conflict."""
    git = _repo(tmp_path)
    (tmp_path / "bench/j").mkdir(parents=True)
    (tmp_path / "bench/j/expected.toml").write_text("instructions = 100\n")
    git("add", "-A")
    git("commit", "-q", "-m", "bench")
    git("update-ref", "refs/remotes/origin/main", "HEAD")
    a = _branch(git, tmp_path, "a", "bench/j/expected.toml", "instructions = 90\n")
    b = _branch(git, tmp_path, "b", "bench/j/expected.toml", "instructions = 95\n")
    c = _branch(git, tmp_path, "c", "f", "from c\n")
    git("checkout", "-q", "--detach", "main")
    tip, bounced = train.build_tip([pr(1, sha=a), pr(2, sha=b), pr(3, sha=c)], tmp_path)
    assert bounced == [], bounced
    assert (tmp_path / "bench/j/expected.toml").read_text() == "instructions = 90\n" and (tmp_path / "f").read_text() == "from c\n"
    assert git("rev-parse", "HEAD") == tip and not git("status", "--porcelain")


def test_the_bless_step_runs_bench_in_the_trees_own_target(monkeypatch, tmp_path):
    """Train 5's bless step ran bench in the shared target, whose binaries another tree had built: `stale binaries in .../release`, and
    the batch failed its strict bench gate."""
    seen = []

    def fake(command, **kw):
        seen.append((kw["env"]["CARGO_TARGET_DIR"], command[:6]))
        return subprocess.CompletedProcess(command, 0, "", "")

    monkeypatch.setattr(train, "git", lambda *a, **k: subprocess.CompletedProcess(a, 0, "", ""))
    monkeypatch.setattr(train.subprocess, "run", fake)
    monkeypatch.setenv("CARGO_TARGET_DIR", "/t/target")
    train.bless("a" * 40, [pr(1)], tmp_path / "train5-tree")
    # and in the tools' environment (uv): the train's own python has no unicorn, which bench imports (train 4 passed its gate, died here)
    assert seen == [("/t/target-train5-tree", ["uv", "run", "-q", "--project", "tools", "python"])]


def test_a_member_a_rebuilt_tip_leaves_out_is_reported_dropped_and_never_offered():
    """Train 4 offered `PRs=#1765 #1768 #1780` for a tip holding #1768 and #1780: after the bisect it rebuilt the tip of the green rest, and
    a member that conflicted only there was dropped without a word and still listed."""
    said = []
    one, two, three = pr(1), pr(2), pr(3)

    def tip_of(some):
        names = "".join(str(p.number) for p in some)
        if len(some) < 3 and two in some:  # 2 conflicts once it is merged without 3
            return f"tip{names.replace('2', '')}", [(two, "conflicts")]
        return f"tip{names}", []

    def gate_at(tip, steps):
        return train.Verdict("3" not in tip, [] if "3" not in tip else ["bench"], "FAIL x\n")

    code = train.run([one, two, three], tip_of, gate_at, lambda steps: train.Verdict(True), said.append)
    assert not any("#2" in line and "PRs=" in line for line in said), said
    assert any(line.startswith("TRAIN DROPPED #2") for line in said), said
    assert code == 0 and any(line == "TRAIN PASS TIP=tip1 PRs=#1" for line in said), said


def test_every_tip_of_a_run_is_built_on_the_commit_main_was_at_when_it_started(tmp_path):
    """Train 4's tips were built on origin/main as it moved: the one rebuilt after the bisect sat on a main that had gained #1819, #1765
    conflicted with it there and was dropped. A tip is built on the pinned commit, whatever the ref says now."""
    git = _repo(tmp_path)
    pinned = git("rev-parse", "HEAD")
    a = _branch(git, tmp_path, "a", "f", "from a\n")
    git("checkout", "-q", "main")
    (tmp_path / "f").write_text("main moved\n")
    git("commit", "-q", "-am", "main moves")
    git("update-ref", "refs/remotes/origin/main", "HEAD")
    git("checkout", "-q", "--detach", "main")
    tip, bounced = train.build_tip([pr(1, sha=a)], tmp_path, pinned)
    assert bounced == [] and (tmp_path / "f").read_text() == "from a\n", "built on the pinned commit, where a merges"
    moved, bounced = train.build_tip([pr(1, sha=a)], tmp_path)
    assert [p.number for p, _ in bounced] == [1], "on the moved ref it conflicts: what the train used to do"


def test_a_report_reaches_a_redirected_log_as_it_is_printed(tmp_path):
    """Trains 4 to 7 printed nothing to their logs until they ended (stdout to a file is block-buffered), so every watch on them was silent
    and a passed batch looked like a hung train."""
    import os
    import subprocess as sp

    log = tmp_path / "log"
    code = ("import sys; sys.path.insert(0, %r); import train, time; "
            "train.run([train.Pr(1, 'a' * 40, [])], lambda prs: ('t', [(prs[0], 'conflicts')]), lambda t, s: None, lambda s: None); time.sleep(30)") % str(Path(train.__file__).parent)
    with open(log, "w") as out:
        child = sp.Popen([sys.executable, "-c", code], stdout=out, stderr=sp.STDOUT)
        try:
            for _ in range(100):
                if "TRAIN" in log.read_text():
                    break
                import time as _time

                _time.sleep(0.05)
            assert "TRAIN BOUNCED #1" in log.read_text(), "printed but still in the buffer of a process that has not ended"
        finally:
            child.kill()
            child.wait()
