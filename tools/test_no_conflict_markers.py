"""No tracked file holds a merge-conflict marker.

A merge of generated pages (`docs/optimizations`) left `<<<<<<< HEAD` ... `>>>>>>> origin/feat/parity` in two pages
that landed on feat/parity; only a gate's diff check saw them.
"""
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MARKER = re.compile(r"^(<{7}|>{7})( |$)")


def test_no_tracked_text_file_has_a_conflict_marker():
    files = subprocess.run(["git", "ls-files", "-z"], cwd=ROOT, capture_output=True, check=True).stdout.decode().split("\0")
    found = []
    for name in filter(None, files):
        path = ROOT / name
        if path.suffix not in {".md", ".rs", ".py", ".toml", ".sh", ".nib", ".c", ".h", ".txt"} or not path.is_file():
            continue
        for number, line in enumerate(path.read_text(errors="replace").splitlines(), 1):
            if MARKER.match(line):
                found.append(f"{name}:{number}: {line[:40]}")
    assert not found, "conflict markers:\n" + "\n".join(found)
