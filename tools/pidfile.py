"""A process records its pid where the scripts that wait for it can read it, and they wait with `kill -0` on that pid, never by matching a command line: a pattern also matches
every session whose prompt or command line names it (a kill by pattern took three sessions down in one day)."""
from __future__ import annotations

import contextlib
import os
from pathlib import Path


@contextlib.contextmanager
def recorded(path: Path | str):
    """Writes this process's pid to `path` for the length of the block and removes it after."""
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(f"{os.getpid()}\n")
    try:
        yield
    finally:
        path.unlink(missing_ok=True)


def alive(path: Path | str) -> bool:
    """Whether the pid recorded in `path` is a running process."""
    try:
        os.kill(int(Path(path).read_text().split()[0]), 0)
    except (OSError, ValueError, IndexError):
        return False
    return True
