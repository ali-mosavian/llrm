"""Set each tracked file's mtime to the time of the last commit that changed it, and to now if it differs from BASE.

    restore_mtime.py [ROOT [BASE]]    BASE: the commit the cached build was made from (none known: every file is "now")

A fresh checkout stamps every file "now", newer than any cached build output, so cargo rebuilds everything. With commit
times (and each directory the newest of its files), an unchanged file is older than the cached build. A file changed since BASE must still look newer than it, whatever
its commit's author date says (a branch cut last week, committed before the cache was made), so those are stamped now.
"""

import time

import os
import sys
import subprocess
from pathlib import Path


def restore(root: Path, base: str | None = None) -> int:
    tracked = set(subprocess.run(["git", "ls-files", "-z"], cwd=root, capture_output=True, text=True, check=True).stdout.split("\0")) - {""}
    log = subprocess.run(["git", "log", "--format=@%ct", "--name-only", "--no-renames", "-z"], cwd=root, capture_output=True, text=True, check=True).stdout
    done, when = set(), 0
    for field in log.split("\0"):
        for part in field.split("\n"):
            if part.startswith("@"):
                when = int(part[1:])
            elif part in tracked and part not in done:  # newest first: the first sighting is the last change
                os.utime(root / part, (when, when))
                done.add(part)
    diff = subprocess.run(["git", "diff", "--name-only", "-z", base, "HEAD"], cwd=root, capture_output=True, text=True) if base else None
    # No base, or one this clone does not have: nothing is known to be unchanged.
    changed = diff.stdout.split("\0") if diff and diff.returncode == 0 else tracked
    now = time.time()
    for part in changed:
        if part in tracked:
            os.utime(root / part, (now, now))
    # Cargo's rerun-if-changed on a directory looks at the directory's own mtime too: the newest tracked file below it.
    newest: dict[str, float] = {}
    for part in tracked:
        if (root / part).exists():
            stamp = (root / part).stat().st_mtime
            for parent in Path(part).parents:
                if str(parent) != ".":
                    newest[str(parent)] = max(newest.get(str(parent), 0), stamp)
    for directory, stamp in newest.items():
        os.utime(root / directory, (stamp, stamp))
    return len(done)


if __name__ == "__main__":
    print(f"mtimes restored: {restore(Path(sys.argv[1] if len(sys.argv) > 1 else '.'), sys.argv[2] if len(sys.argv) > 2 else None)}")
