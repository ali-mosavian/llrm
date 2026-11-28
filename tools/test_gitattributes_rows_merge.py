"""A row a merged branch deleted stays deleted: `merge=union` on the rows files brought retired rows back with no conflict (w3: f55e x MC2 merged to three
COMBINER-MC rows and the retired MC2 row; e3c006ded3, ea7f072c70, 59397be443 before it)."""

import shutil
import subprocess
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[1]
ROWS = ["docs/working/above-main-rows.md", "docs/working/selectiondag-plan.md", "docs/working/codegen-slot-table.md", "tests/target_facts.baseline"]


def merged(tmp_path: Path, name: str) -> tuple[bool, str]:
    """(conflicted, text) of merging a branch that deletes row B into one that edits row C, with this repository's .gitattributes."""

    def git(*args: str) -> subprocess.CompletedProcess:
        return subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", *args], cwd=tmp_path, capture_output=True, text=True)

    path = tmp_path / name
    git("init", "-q", "-b", "main")
    shutil.copy(REPO / ".gitattributes", tmp_path / ".gitattributes")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("| A | 1 |\n| B | 2 |\n| C | 3 |\n| D | 4 |\n")
    git("add", ".")
    git("commit", "-qm", "base")
    git("checkout", "-qb", "delete")
    path.write_text("| A | 1 |\n| C | 3 |\n| D | 4 |\n")
    git("commit", "-qam", "delete B")
    git("checkout", "-q", "main")
    path.write_text("| A | 1 |\n| B | 2 |\n| C | 30 |\n| D | 4 |\n")
    git("commit", "-qam", "edit C")
    conflicted = git("merge", "-q", "--no-edit", "delete").returncode != 0
    return conflicted, path.read_text()


@pytest.mark.parametrize("name", ROWS)
def test_a_deleted_row_is_never_brought_back_by_a_merge(tmp_path, name):
    conflicted, text = merged(tmp_path, name)
    assert conflicted or "| B | 2 |" not in text, f"{name}: the deleted row came back with no conflict:\n{text}"


def test_no_file_merges_by_union():
    """A file named here later is a rows file again: union keeps both sides' lines and drops a deletion without a conflict."""
    lines = [line for line in (REPO / ".gitattributes").read_text().splitlines() if "merge=union" in line and not line.lstrip().startswith("#")]
    assert lines == []
