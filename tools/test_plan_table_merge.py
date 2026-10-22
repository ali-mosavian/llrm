"""Two branches that each add a row to a plan table must merge without a conflict.

Before `merge=union` in .gitattributes, selectiondag-plan.md and codegen-slot-table.md conflicted
whenever two PRs appended a row; each owner hand-merged it.
"""
import shutil
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parent.parent
TABLES = ["docs/working/selectiondag-plan.md", "docs/working/codegen-slot-table.md"]


def git(cwd, *args, check=True):
    return subprocess.run(["git", *args], cwd=cwd, check=check, capture_output=True, text=True)


@pytest.mark.parametrize("table", TABLES)
def test_two_branches_each_adding_a_row_merge_without_a_conflict(tmp_path, table):
    git(tmp_path, "init", "-q", "-b", "main")
    git(tmp_path, "config", "user.email", "t@t")
    git(tmp_path, "config", "user.name", "t")
    shutil.copy(ROOT / ".gitattributes", tmp_path / ".gitattributes")
    page = tmp_path / table
    page.parent.mkdir(parents=True)
    page.write_text("| # | name |\n|---|---|\n| 1 | a |\n\nend\n")
    git(tmp_path, "add", "-A")
    git(tmp_path, "commit", "-q", "-m", "base")
    for name in ("one", "two"):
        git(tmp_path, "checkout", "-q", "-b", name, "main")
        page.write_text(page.read_text().replace("| 1 | a |\n", f"| 1 | a |\n| {name} | row |\n"))
        git(tmp_path, "commit", "-qam", name)
    git(tmp_path, "checkout", "-q", "one")
    merged = git(tmp_path, "merge", "--no-edit", "two", check=False)
    assert merged.returncode == 0, merged.stdout
    text = page.read_text()
    assert "| one | row |" in text and "| two | row |" in text
