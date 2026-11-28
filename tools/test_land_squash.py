"""Every PR lands as one squash commit `<title> (#N)` with an empty body. PUSH_MODE merged each head onto release/next and pushed the tip: a head that already held
release/next fast-forwarded, and #10964 landed all 18 of its commits, two merges among them (366 such first-parent commits since 10-28)."""

import json
import re
import subprocess
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import parity_squash as ps  # noqa: E402

HEAD = "a" * 40


@pytest.fixture(autouse=True)
def vouched(tmp_path, monkeypatch):
    """PRs 7 and 8 vouched at HEAD; the tests that ask for no vouch empty it."""
    path = tmp_path / "vouched.state"
    path.write_text(f"7 {HEAD} 2026-11-28T17:14Z coordinator\n8 {HEAD} 2026-11-28T17:14Z coordinator\n")
    monkeypatch.setattr(ps, "VOUCHED", path, raising=False)
    return path


def stub(view: dict):
    """A `run` for squash(): `gh pr view` answers `view`, every other call succeeds; the calls are kept."""
    calls = []

    def run(cmd, **kwargs):
        calls.append(cmd)
        out = json.dumps(view) if cmd[:3] == ["gh", "pr", "view"] else ""
        return subprocess.CompletedProcess(cmd, 0, out, "")

    return run, calls


def merges(calls):
    return [cmd for cmd in calls if cmd[:3] == ["gh", "pr", "merge"]]


def info(title="fix(x): one thing"):
    return {"headRefOid": HEAD, "headRefName": "w/x", "title": title, "mergeable": "MERGEABLE"}


def test_a_member_lands_as_one_squash_titled_with_its_number_and_no_body():
    run, calls = stub({"isDraft": False, "headRefOid": HEAD, "state": "OPEN"})
    assert ps.squash("7", info(), "", run=run)
    [cmd] = merges(calls)
    assert "--squash" in cmd
    assert cmd[cmd.index("--subject") + 1] == "fix(x): one thing (#7)"
    assert cmd[cmd.index("--body") + 1] == ""


def test_a_title_that_names_its_number_is_not_numbered_twice():
    run, calls = stub({"isDraft": False, "headRefOid": HEAD, "state": "OPEN"})
    ps.squash("7", info("fix(x): one thing (#7)"), "", run=run)
    [cmd] = merges(calls)
    assert cmd[cmd.index("--subject") + 1] == "fix(x): one thing (#7)"


def test_a_pr_drafted_while_the_batch_ran_is_not_merged(capsys):
    """Oversight drafted #10789 mid-run; the runner had read it ready at the start and squashed on."""
    run, calls = stub({"isDraft": True, "headRefOid": HEAD, "state": "OPEN"})
    assert ps.squash("7", info(), "", run=run) is False
    assert merges(calls) == []
    assert "draft" in capsys.readouterr().out


def test_a_head_pushed_while_the_batch_ran_is_left_out_not_merged():
    run, calls = stub({"isDraft": False, "headRefOid": "b" * 40, "state": "OPEN"})
    assert ps.squash("8", info(), "", run=run) is False
    assert merges(calls) == []
    assert "8" in ps.HEAD_MOVED


def test_the_runner_never_pushes_the_integration_branch_itself():
    """release/next moves only by GitHub's squash: a push of a merged tip lands a head's commits as they are."""
    source = (Path(__file__).resolve().parent / "parity_squash.py").read_text()
    pushes = [line.strip() for line in source.splitlines() if re.search(r'"push"', line) and re.search(r"refs/heads/\{(ref|BRANCH)\}|:\{?BRANCH", line)]
    assert pushes == []


def sh(repo: Path, *args: str) -> str:
    done = subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", *args], cwd=repo, capture_output=True, text=True)
    assert done.returncode == 0, done.stderr
    return done.stdout.strip()


def test_the_landed_tree_is_compared_with_the_tested_one(tmp_path):
    sh(tmp_path, "init", "-q", "-b", "main")
    (tmp_path / "a.txt").write_text("a\n")
    sh(tmp_path, "add", "-A")
    sh(tmp_path, "commit", "-qm", "tested")
    tested = sh(tmp_path, "rev-parse", "HEAD^{tree}")
    assert ps.tree_mismatch(tested, "HEAD", cwd=tmp_path) == ""
    (tmp_path / "a.txt").write_text("b\n")
    sh(tmp_path, "commit", "-qam", "landed something else")
    assert tested[:9] in ps.tree_mismatch(tested, "HEAD", cwd=tmp_path)


def test_an_unvouched_pr_is_skipped_not_merged_and_not_refused(vouched, capsys):
    """#10789 was ready, never vouched, and landed: ready is not a vouch."""
    vouched.write_text("")
    run, calls = stub({"isDraft": False, "headRefOid": HEAD, "state": "OPEN"})
    assert ps.squash("7", info(), "", run=run) is None
    assert merges(calls) == []
    out = capsys.readouterr().out
    assert "unvouched" in out and "REFUSED" not in out


def test_a_vouch_for_another_head_is_no_vouch(vouched):
    vouched.write_text(f"7 {'c' * 40} 2026-11-28T17:14Z coordinator\n")
    run, calls = stub({"isDraft": False, "headRefOid": HEAD, "state": "OPEN"})
    assert ps.squash("7", info(), "", run=run) is None
    assert merges(calls) == []


def test_the_vouch_that_let_a_pr_land_is_logged(capsys):
    run, calls = stub({"isDraft": False, "headRefOid": HEAD, "state": "OPEN"})
    assert ps.squash("7", info(), "", run=run)
    assert f"#7: vouched: 7 {HEAD} 2026-11-28T17:14Z coordinator" in capsys.readouterr().out


def test_a_known_red_line_goes_to_the_log_never_the_squash_body(capsys):
    """Rule 11-04: a squash body is empty. --known-red put "Known red: sc (2438)" in the commit."""
    run, calls = stub({"isDraft": False, "headRefOid": HEAD, "state": "OPEN"})
    assert ps.squash("7", info(), "Known red: sc (2438)", run=run)
    [cmd] = merges(calls)
    assert cmd[cmd.index("--body") + 1] == ""
    assert "Known red: sc (2438)" in capsys.readouterr().out


def test_a_tree_the_runner_formatted_after_the_test_is_still_compared(tmp_path):
    """A head the runner formatted after its test landed with "tree not compared": the expected tree is the tested commit with the formatted head merged in."""
    sh(tmp_path, "init", "-q", "-b", "main")
    (tmp_path / "a.rs").write_text("fn a(){}\n")
    sh(tmp_path, "add", "-A")
    sh(tmp_path, "commit", "-qm", "base")
    sh(tmp_path, "checkout", "-qb", "pr")
    (tmp_path / "b.rs").write_text("fn b(){}\n")
    sh(tmp_path, "add", "-A")
    sh(tmp_path, "commit", "-qm", "pr")
    tested = sh(tmp_path, "rev-parse", "HEAD")
    (tmp_path / "b.rs").write_text("fn b() {}\n")
    sh(tmp_path, "commit", "-qam", "style: fmt")
    formatted = sh(tmp_path, "rev-parse", "HEAD")
    tree, why = ps.expected_tree(tested, [formatted], cwd=tmp_path)
    assert not why and tree == sh(tmp_path, "rev-parse", f"{formatted}^{{tree}}")
    assert tree != sh(tmp_path, "rev-parse", f"{tested}^{{tree}}")
