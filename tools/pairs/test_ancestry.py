"""A pair whose base is not an ancestor of its head is refused, or flagged in the report header when the caller says it is deliberate."""
import subprocess
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import ancestry  # noqa: E402

HERE = Path(__file__).resolve().parent


@pytest.fixture
def commits(tmp_path, monkeypatch):
    """main: a - b; a branch from a: c. So a is an ancestor of b, and c is not."""
    git = lambda *args: subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", *args], cwd=tmp_path, check=True, capture_output=True, text=True).stdout.strip()  # noqa: E731
    git("init", "-q", "-b", "main")
    sha = {}
    for name, branch in (("a", None), ("b", None), ("c", "side")):
        if branch:
            git("checkout", "-q", "-b", branch, sha["a"])
        (tmp_path / "f").write_text(name)
        git("add", "f")
        git("commit", "-q", "-m", name)
        sha[name] = git("rev-parse", "HEAD")
    monkeypatch.setenv("LLRM_PAIR_REPO", str(tmp_path))
    monkeypatch.delenv("LLRM_PAIR_CROSS_BRANCH", raising=False)
    return sha


def test_an_ancestor_base_is_accepted_and_the_header_says_so(commits):
    assert "is an ancestor of" in ancestry.header(commits["a"], commits["b"])


def test_a_base_from_another_branch_is_refused_with_the_merge_base(commits):
    """w4's InstCombine pair compared a base that was not an ancestor of its head: the delta was a different commit's."""
    with pytest.raises(SystemExit) as refused:
        ancestry.header(commits["c"], commits["b"])
    assert "pair refused" in str(refused.value) and commits["a"][:10] in str(refused.value) and "--cross-branch" in str(refused.value)


def test_a_deliberate_cross_branch_pair_is_flagged_not_refused(commits, monkeypatch):
    assert "CROSS-BRANCH" in ancestry.header(commits["c"], commits["b"], cross_branch=True)
    monkeypatch.setenv("LLRM_PAIR_CROSS_BRANCH", "1")
    assert "CROSS-BRANCH" in ancestry.header(commits["c"], commits["b"])


def test_the_command_exits_nonzero_for_a_refused_pair(commits):
    done = subprocess.run([sys.executable, str(HERE / "ancestry.py"), commits["c"], commits["b"]], capture_output=True, text=True, env={"LLRM_PAIR_REPO": ancestry.repo().as_posix(), "PATH": "/usr/bin:/bin"})
    assert done.returncode == 2 and "pair refused" in done.stderr


def test_pair_sh_refuses_before_it_builds_anything(commits, tmp_path):
    """pair.sh checks the ancestry first: a refused pair leaves no build, only its header file."""
    scratch = tmp_path / "scratch"
    scratch.mkdir()
    done = subprocess.run(["bash", str(HERE / "pair.sh"), "t", commits["c"], commits["b"]], capture_output=True, text=True, env={"LLRM_PAIR_REPO": str(tmp_path), "PAIR_SCRATCH": str(scratch), "PAIR_WT": "x", "PATH": "/usr/bin:/bin"})
    assert done.returncode == 2 and "pair refused" in done.stderr, done.stderr
    assert not list(scratch.glob("sbp-*")) and not list(scratch.glob("sb-*"))
