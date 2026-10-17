"""Which clang a tool measured: the pinned one by default, its version string in the output, and a different one only when named.

The per-pass table of d_faces -O2 ran the distro's clang 20.1.8 where the plan's reference is the pinned 24.0.0git d1106deb71cc, because
`--clang` defaulted to `clang` on PATH: the clang column was another compiler's.
"""

from __future__ import annotations

import shutil
import subprocess
from pathlib import Path

PINNED = Path.home() / "scratch/llvm-build/bin/clang"


def default() -> str:
    """The pinned clang when it is built here, else the one on PATH."""
    return str(PINNED) if PINNED.is_file() else (shutil.which("clang") or "clang")


def version(clang: str) -> str:
    """The first line of `clang --version`."""
    return subprocess.run([clang, "--version"], capture_output=True, text=True).stdout.splitlines()[0].strip()


def header(clang: str) -> str:
    return f"clang: {clang} ({version(clang)})"


def require_pinned(clang: str, allow_other: bool = False) -> None:
    """SystemExit when `clang` is a different compiler from the pinned one, unless `allow_other` names it."""
    if allow_other or not PINNED.is_file():
        return
    if version(clang) != version(str(PINNED)):
        raise SystemExit(f"{clang} is {version(clang)}, not the pinned {version(str(PINNED))}: name it with --allow-other-clang if that is the one to measure")
