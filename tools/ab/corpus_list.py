#!/usr/bin/env python3
"""corpus_list.py LEVELS TARGET...: the cell list `cells.sh` reads, one line `ID TARGET -LEVEL FILE.c` per program, target and level.

The programs are `tests/run/c` and the benchmarks (`benchlist`). A program that is not for a target stays in the list: `cells.sh` prints
`N/A` for it and counts it, so the applicable cells are the list less the N/A lines. A list that dropped them itself would hide that count.
"""
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))

import benchlist  # noqa: E402


def programs(run_dir: Path = ROOT / "tests/run/c", bench: Path = benchlist.BENCH) -> list[tuple[str, Path]]:
    """(id, path) of every program: `run/NAME` and `bench/DIR/NAME`."""
    found = [(f"run/{path.stem}", path) for path in sorted(run_dir.glob("*.c"))]
    return found + [(f"bench/{path.parent.name}/{path.stem}", path) for path in benchlist.sources(".c", bench)]


def lines(levels: list[str], targets: list[str], corpus: list[tuple[str, Path]]) -> list[str]:
    return [f"{ident} {target} -{level} {path}" for target in targets for level in levels for ident, path in corpus]


def main(argv: list[str]) -> int:
    if len(argv) < 3:
        print(__doc__, file=sys.stderr)
        return 2
    levels = [level.removeprefix("-") for level in argv[1].split(",")]
    print("\n".join(lines(levels, argv[2:], programs())))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
