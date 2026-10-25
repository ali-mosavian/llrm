"""Two PRs that append a test to the same `*_tests.rs` merged with a conflict: five rounds of batches left PRs out for it and owners resolved it by hand.
The repo's `.gitattributes` must merge such files as the union of both sides."""

import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TESTS = "crates/x/src/area_tests.rs"
BASE = "#[cfg(test)]\nmod tests {\n    #[test]\n    fn first() {}\n}\n"


def git(repo: Path, *args: str, check: bool = True) -> subprocess.CompletedProcess:
    done = subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", *args], cwd=repo, capture_output=True, text=True)
    assert not check or done.returncode == 0, done.stderr
    return done


def appended(repo: Path, name: str) -> None:
    path = repo / TESTS
    path.write_text(path.read_text().removesuffix("}\n") + f"    #[test]\n    fn {name}() {{}}\n}}\n")
    git(repo, "commit", "-qam", name)


def two_prs(tmp_path: Path, attributes: bool) -> Path:
    repo = tmp_path / "repo"
    (repo / TESTS).parent.mkdir(parents=True)
    git(tmp_path, "init", "-q", "-b", "main", str(repo))
    (repo / TESTS).write_text(BASE)
    if attributes:
        shutil.copy(ROOT / ".gitattributes", repo / ".gitattributes")
    git(repo, "add", "-A")
    git(repo, "commit", "-qm", "base")
    git(repo, "checkout", "-qb", "second")
    appended(repo, "second")
    git(repo, "checkout", "-q", "main")
    appended(repo, "first_landed")
    return repo


def test_two_prs_appending_to_one_tests_file_merge_as_both(tmp_path):
    """Without the attribute the merge conflicted at the end of `dag_function_tests.rs` and the later PR was left out."""
    repo = two_prs(tmp_path, attributes=True)
    merged = git(repo, "merge", "--no-edit", "-q", "second", check=False)
    text = (repo / TESTS).read_text()
    assert merged.returncode == 0, merged.stdout + merged.stderr
    assert "<<<<<<<" not in text
    assert "fn first_landed()" in text and "fn second()" in text
    assert text.count("mod tests {") == 1 and text.endswith("}\n")


def test_the_fixture_conflicts_without_the_attribute(tmp_path):
    """The fixture is the conflict: with no attribute the same two appends do not merge."""
    repo = two_prs(tmp_path, attributes=False)
    assert git(repo, "merge", "--no-edit", "-q", "second", check=False).returncode != 0
