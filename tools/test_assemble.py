"""A fast batch took every queued PR and left out the ones that conflicted with an earlier member: a full smoke for the others each time.
`assemble` keeps the PRs that merge one after another, and the rest wait."""

import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import parity_squash as ps  # noqa: E402


def sh(repo: Path, *args: str) -> str:
    done = subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", *args], cwd=repo, capture_output=True, text=True)
    assert done.returncode == 0, done.stderr
    return done.stdout.strip()


def branch(repo: Path, name: str, path: str, text: str) -> str:
    sh(repo, "checkout", "-q", "-b", name, "main")
    (repo / path).write_text(text)
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", name)
    return sh(repo, "rev-parse", "HEAD")


def test_the_pr_that_conflicts_with_an_earlier_member_waits_and_the_third_joins(tmp_path):
    """Three PRs, the first two edit one line of one file: before, the batch took all three and #2 was left out after the smoke."""
    repo = tmp_path / "repo"
    sh(tmp_path, "init", "-q", "-b", "main", str(repo))
    (repo / "shared.txt").write_text("one\ntwo\nthree\n")
    (repo / "other.txt").write_text("x\n")
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", "base")
    heads = {
        "1": branch(repo, "a", "shared.txt", "one\nTWO\nthree\n"),
        "2": branch(repo, "b", "shared.txt", "one\n2\nthree\n"),
        "3": branch(repo, "c", "other.txt", "y\n"),
    }
    sh(repo, "checkout", "-q", "main")
    kept, waits = ps.assemble(["1", "2", "3"], heads, base="main", cwd=repo)
    assert kept == ["1", "3"]
    assert list(waits) == ["2"] and "shared.txt" in waits["2"]


def test_the_limit_counts_kept_prs_only(tmp_path):
    repo = tmp_path / "repo"
    sh(tmp_path, "init", "-q", "-b", "main", str(repo))
    (repo / "f").write_text("x\n")
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", "base")
    heads = {str(n): branch(repo, f"b{n}", f"f{n}", "y\n") for n in range(1, 5)}
    sh(repo, "checkout", "-q", "main")
    kept, waits = ps.assemble(list(heads), heads, base="main", cwd=repo, limit=2)
    assert kept == ["1", "2"] and not waits


def test_two_prs_adding_a_test_of_one_name_do_not_share_a_batch(tmp_path):
    """#4678 and #4680 added the same test in different places: no conflict, so both joined, and the batch failed 'defined multiple times'."""
    repo = tmp_path / "repo"
    sh(tmp_path, "init", "-q", "-b", "main", str(repo))
    (repo / "t.rs").write_text("fn first() {}\n\n\n\n\n\n\n\n\nfn last() {}\n")
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", "base")
    sh(repo, "checkout", "-qb", "a")
    (repo / "t.rs").write_text("fn first() {}\nfn a_frame_cell_is_not_encodable() {}\n\n\n\n\n\n\n\n\nfn last() {}\n")
    sh(repo, "commit", "-qam", "a")
    heads = {"1": sh(repo, "rev-parse", "HEAD")}
    sh(repo, "checkout", "-q", "main")
    sh(repo, "checkout", "-qb", "b")
    (repo / "t.rs").write_text("fn first() {}\n\n\n\n\n\n\n\n\nfn last() {}\nfn a_frame_cell_is_not_encodable() {}\n")
    sh(repo, "commit", "-qam", "b")
    heads["2"] = sh(repo, "rev-parse", "HEAD")
    sh(repo, "checkout", "-q", "main")
    kept, waits = ps.assemble(["1", "2"], heads, base="main", cwd=repo)
    assert kept == ["1"] and "a_frame_cell_is_not_encodable" in waits["2"] and "#1" in waits["2"]


def test_a_build_refusal_names_the_error_of_the_prs_own_alone_build_not_the_batchs(tmp_path):
    """The 16:56 batch refused #4743 with `floatassign.rs: unused import live`, the error of #4648 in the same batch; #4743 alone built clean. The first PR breaks the build, the second is clean."""
    repo = tmp_path / "repo"
    sh(tmp_path, "init", "-q", "-b", "main", str(repo))
    (repo / "a.txt").write_text("a\n")
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", "base")
    sh(repo, "checkout", "-q", "main")
    current = []

    def merge_alone(number: str) -> bool:
        current[:] = [number]
        return True

    def check() -> str:
        return "floatassign.rs:124:38: unused import: `live`" if current == ["4648"] else ""

    errors: dict[str, str] = {}
    assert ps.build_offenders(["4648", "4743"], merge_alone, check, errors) == ["4648"]
    assert errors == {"4648": "floatassign.rs:124:38: unused import: `live`"}
