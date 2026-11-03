#!/usr/bin/env python3
"""Judge a bench json against a base json with the bench's own rule (bench.against, bench.rate_for): one copy of it.

    cellcheck.py HEAD.json BASE.json

Rows are 'program/language/level'. Both files need a `.harness` sidecar and the same digest: rows of two harnesses are not comparable.
-O1 is reported, not gated. A cell the head lacks counts as failed. Exit 1 on any failure."""
from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bench  # noqa: E402


def digest(path: str) -> str | None:
    side = Path(path + ".harness")
    words = side.read_text().split() if side.exists() else []
    return words[0] if words else None


def check(head: dict, base: dict) -> list[str]:
    """One line per failed cell of `base`: NOHEAD, or what bench.against says of head's row."""
    lines = []
    for key, want in sorted(base.items()):
        got = head.get(key)
        if got is None:
            lines.append(f"{key}: NOHEAD")
            continue
        _, language, level = key.rsplit("/", 2)
        if level == "O1":
            continue
        lines += bench.against(key, got, want, rate=bench.rate_for(language, level))
    return lines


def main(argv: list[str]) -> int:
    head_file, base_file = argv
    head, base = (json.load(open(f)) for f in (head_file, base_file))
    base = base.get("rows", base)
    cells = f"{len(base)} cells over {len({k.rsplit('/', 2)[0] for k in base})} programs"
    one, two = digest(head_file), digest(base_file)
    if one is None or two is None or one != two:
        print(f"{cells}, ALL NOBASE: {'no .harness sidecar' if None in (one, two) else f'harness digests differ ({one} vs {two})'}")
        return 1
    lines = check(head, base)
    print(*lines, f"{cells}, {len(lines)} fail", sep="\n")
    return bool(lines)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
