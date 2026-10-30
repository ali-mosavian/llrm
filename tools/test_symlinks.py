"""`uv run --project tools python -m pytest tools/test_symlinks.py`

Every tracked symlink resolves. A directory move leaves a link's text pointing at the old path; neither cargo nor the
path-index test reads links, so the re-org left two dangling until the landing runner's link stage read them.
"""
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def dangling(entries: list[str], root: Path = ROOT) -> list[str]:
    """`git ls-files -s` lines of mode 120000 whose target does not exist."""
    links = [line.split("\t", 1)[1] for line in entries if line.startswith("120000 ")]
    return [f"{link} -> {Path(root / link).readlink()}" for link in links if not (root / link).exists()]


def test_a_link_to_a_moved_file_is_reported(tmp_path):
    (tmp_path / "moved").mkdir()
    (tmp_path / "new.txt").write_text("x")
    (tmp_path / "moved" / "a.txt").symlink_to("../old/a.txt")
    (tmp_path / "moved" / "b.txt").symlink_to("../new.txt")
    listing = ["120000 aa 0\tmoved/a.txt", "120000 bb 0\tmoved/b.txt", "100644 cc 0\tnew.txt"]
    assert dangling(listing, tmp_path) == ["moved/a.txt -> ../old/a.txt"]


def test_every_tracked_symlink_resolves():
    out = subprocess.check_output(["git", "ls-files", "-s"], cwd=ROOT, text=True)
    assert not dangling(out.splitlines()), "tracked symlinks point at nothing"
