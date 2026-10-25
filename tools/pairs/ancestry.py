"""A pair compares a BASE with a NEW build, and means: what NEW's commits changed. That holds only when BASE is an ancestor of NEW.

w4's InstCombine pair compared a base that was not an ancestor of its head, so the delta was another commit's. Every pair script asks here first and refuses the pair
(exit 2, with the reason) unless the caller says `--cross-branch` (or LLRM_PAIR_CROSS_BRANCH=1) and so accepts a delta that is not NEW's own; the report header then says so.

    python3 tools/pairs/ancestry.py BASE NEW [--cross-branch]
"""
from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def repo() -> Path:
    return Path(os.environ.get("LLRM_PAIR_REPO", ROOT))


def is_ancestor(base: str, new: str, cwd: Path | None = None) -> bool:
    return subprocess.run(["git", "merge-base", "--is-ancestor", base, new], cwd=cwd or repo(), capture_output=True).returncode == 0


def header(base: str, new: str, cross_branch: bool = False, cwd: Path | None = None) -> str:
    """The line a report starts with, or SystemExit(2) naming why the pair is refused."""
    if is_ancestor(base, new, cwd):
        return f"pair: base {base[:10]} is an ancestor of new {new[:10]}"
    if cross_branch or os.environ.get("LLRM_PAIR_CROSS_BRANCH") == "1":
        return f"pair: CROSS-BRANCH: base {base[:10]} is NOT an ancestor of new {new[:10]}; the delta is not new's own commits"
    raise SystemExit(
        f"pair refused: base {base[:10]} is not an ancestor of new {new[:10]}, so the delta is not what new's commits changed "
        f"(merge-base: {merge_base(base, new, cwd)[:10] or 'none'}). Rebase the pair on the merge-base, or pass --cross-branch (LLRM_PAIR_CROSS_BRANCH=1) to compare them anyway."
    ) from None


def merge_base(base: str, new: str, cwd: Path | None = None) -> str:
    done = subprocess.run(["git", "merge-base", base, new], cwd=cwd or repo(), capture_output=True, text=True)
    return done.stdout.strip()


def main(argv: list[str]) -> int:
    flag = "--cross-branch" in argv
    shas = [one for one in argv if one != "--cross-branch"]
    if len(shas) != 2:
        print(__doc__, file=sys.stderr)
        return 2
    try:
        print(header(shas[0], shas[1], flag))
    except SystemExit as why:
        print(why, file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
