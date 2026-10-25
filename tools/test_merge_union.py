"""Two PRs that append a test to the same `*_tests.rs` once merged with a conflict; `merge=union` merged them and dropped a closing `}` (#4570: llrm-core did not
compile, no conflict reported). The repo's `.gitattributes` names `tests-append`, whose driver keeps each appended block whole."""

import os

import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))
import parity_squash as ps  # noqa: E402

TESTS = "crates/x/src/area_tests.rs"
TEST = "#[test]\nfn {name}() {{\n    let module = parse({name});\n    let done = selected(&module);\n    assert!(done.volatile, \"volatile\");\n}}\n"
BASE = TEST.format(name="volatile_load")


def git(repo: Path, *args: str, check: bool = True) -> subprocess.CompletedProcess:
    env = dict(os.environ)
    env.pop("GIT_CONFIG_COUNT", None)
    ps.with_merge_driver(env)
    env["GIT_CONFIG_VALUE_0"] = f"python3 {ROOT}/tools/merge_tests_append.py %O %A %B"
    done = subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", *args], cwd=repo, capture_output=True, text=True, env=env)
    assert not check or done.returncode == 0, done.stderr
    return done


def appended(repo: Path, name: str) -> None:
    path = repo / TESTS
    path.write_text(path.read_text() + f"\nconst {name.upper()}: &str = \"{name}\";\n\n" + TEST.format(name=name))
    git(repo, "commit", "-qam", name)


def two_prs(tmp_path: Path, attribute: str) -> Path:
    repo = tmp_path / "repo"
    (repo / TESTS).parent.mkdir(parents=True)
    git(tmp_path, "init", "-q", "-b", "main", str(repo))
    (repo / TESTS).write_text(BASE)
    (repo / ".gitattributes").write_text(f"*_tests.rs merge={attribute}\n")
    git(repo, "add", "-A")
    git(repo, "commit", "-qm", "base")
    git(repo, "checkout", "-qb", "second")
    appended(repo, "volatile_store")
    git(repo, "checkout", "-q", "main")
    appended(repo, "volatile_copy")
    return repo


def parses(text: str, tmp_path: Path) -> bool:
    (tmp_path / "x.rs").write_text(text)
    return subprocess.run(["rustc", "-Zunpretty=normal", "--edition", "2024", str(tmp_path / "x.rs")], capture_output=True, env={**os.environ, "RUSTC_BOOTSTRAP": "1"}).returncode == 0


def test_two_tests_appended_after_the_same_test_merge_whole(tmp_path):
    """With `merge=union` the first block lost its tail (`let done`, `assert!`, `}`) to the second: 'unclosed delimiter'."""
    repo = two_prs(tmp_path, "tests-append")
    merged = git(repo, "merge", "--no-edit", "-q", "second", check=False)
    text = (repo / TESTS).read_text()
    assert merged.returncode == 0, merged.stdout + merged.stderr
    assert "<<<<<<<" not in text
    assert text.count("assert!(done.volatile") == 3 and text.count("\n}\n") == 3
    assert text.index("fn volatile_copy") < text.index("fn volatile_store")
    assert parses(text, tmp_path)


def test_union_is_what_dropped_the_brace(tmp_path):
    """The defect, kept as a fixture: the same two appends under `merge=union` merge without a conflict into text that does not parse."""
    repo = two_prs(tmp_path, "union")
    git(repo, "merge", "--no-edit", "-q", "second", check=False)
    assert not parses((repo / TESTS).read_text(), tmp_path)


def test_two_edits_of_one_line_still_conflict(tmp_path):
    repo = two_prs(tmp_path, "tests-append")
    git(repo, "reset", "-q", "--hard", "HEAD~1")
    git(repo, "checkout", "-q", "second")
    git(repo, "reset", "-q", "--hard", "HEAD~1")
    for branch, word in (("second", "one"), ("main", "two")):
        git(repo, "checkout", "-q", branch)
        path = repo / TESTS
        path.write_text(path.read_text().replace('"volatile"', f'"{word}"'))
        git(repo, "commit", "-qam", word)
    assert git(repo, "merge", "--no-edit", "-q", "second", check=False).returncode != 0


def test_gitattributes_names_the_driver_for_tests_files():
    lines = (ROOT / ".gitattributes").read_text().splitlines()
    assert "*_tests.rs merge=tests-append" in lines and "tests.rs merge=tests-append" in lines
    assert not any("tests.rs" in line and line.endswith("merge=union") for line in lines)
