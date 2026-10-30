"""One-push landing: a verified batch is merged into a tip and pushed to feat/parity as a plain fast-forward, so GitHub marks each PR merged."""

import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import parity_squash as ps  # noqa: E402


def sh(repo: Path, *args: str) -> str:
    done = subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", *args], cwd=repo, capture_output=True, text=True)
    assert done.returncode == 0, done.stderr
    return done.stdout.strip()


def scratch(tmp_path):
    origin = tmp_path / "origin.git"
    sh(tmp_path, "init", "-q", "--bare", "-b", "feat/parity", str(origin))
    repo = tmp_path / "repo"
    sh(tmp_path, "clone", "-q", str(origin), str(repo))
    sh(repo, "checkout", "-q", "-b", "feat/parity")
    (repo / "a.txt").write_text("a\n")
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", "base")
    sh(repo, "push", "-q", "origin", "feat/parity")
    heads = {}
    for number, name in (("1", "b.txt"), ("2", "c.txt")):
        sh(repo, "checkout", "-q", "-b", f"pr{number}", "feat/parity")
        (repo / name).write_text(f"{number}\n")
        sh(repo, "add", "-A")
        sh(repo, "commit", "-qm", f"pr{number}")
        sh(repo, "push", "-q", "origin", f"pr{number}")
        heads[number] = sh(repo, "rev-parse", "HEAD")
    sh(repo, "checkout", "-q", "feat/parity")
    return origin, repo, heads


def test_the_tip_holds_every_head_and_feat_parity_moves_by_one_fast_forward(tmp_path):
    """Per-PR squash cost 24 s each (700 s of a 29-PR batch): one push of the merged tip lands the batch, and each head is an ancestor of the new feat/parity."""
    origin, repo, heads = scratch(tmp_path)
    old = sh(repo, "rev-parse", "origin/feat/parity")
    tip, why = ps.push_tip(heads, cwd=repo, prepare=lambda cwd: False, verify=lambda cwd: "")
    assert tip and not why, why
    assert sh(origin, "rev-parse", "feat/parity") == tip
    for head in heads.values():
        sh(origin, "merge-base", "--is-ancestor", head, tip)
    sh(origin, "merge-base", "--is-ancestor", old, tip)


def test_a_rejected_push_names_why_and_moves_nothing(tmp_path):
    """feat/parity moved under the batch: the push must not be forced; the caller squashes per PR instead."""
    origin, repo, heads = scratch(tmp_path)
    other = tmp_path / "other"
    sh(tmp_path, "clone", "-q", str(origin), str(other))
    (other / "z.txt").write_text("z\n")
    sh(other, "add", "-A")
    sh(other, "commit", "-qm", "moved")
    sh(other, "push", "-q", "origin", "HEAD:feat/parity")
    moved = sh(origin, "rev-parse", "feat/parity")
    tip, why = ps.push_tip(heads, cwd=repo, base="feat/parity", prepare=lambda cwd: False, verify=lambda cwd: "")
    assert not tip and "push refused" in why, (tip, why)
    assert sh(origin, "rev-parse", "feat/parity") == moved


def test_a_tip_that_fails_the_cheap_checks_is_not_pushed(tmp_path):
    origin, repo, heads = scratch(tmp_path)
    before = sh(origin, "rev-parse", "feat/parity")
    tip, why = ps.push_tip(heads, cwd=repo, prepare=lambda cwd: False, verify=lambda cwd: "fmt --check fails on the tip")
    assert not tip and "fmt" in why
    assert sh(origin, "rev-parse", "feat/parity") == before
