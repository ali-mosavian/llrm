"""The merge train: batches, bounced conflicts, the split of a red batch, ENV verdicts, and that it never pushes."""

import sys
import subprocess
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import train  # noqa: E402

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
