#!/usr/bin/env python3
"""Rewrite main_clocks.toml and clock_limits.toml from a run of main: a clock bar is main's own clocks under the judge the head is measured by,
never a hand-kept number (the judge changed twice in 11-02 and the old values were two judges apart).

    rebase_clocks.py MAIN_ROWS.json     bench.py --json of main's compiler, -Os m16 rows (c, bas, bq16, nib)

Every `"name lang -Os" = N` line of both files gets N = main's clocks for that row; a limit therefore equals main's, which is what no waiver means."""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
LINE = re.compile(r'^("(?P<label>[^"]+) -Os") = (?P<n>\d+)(?P<rest>.*)$')


def label_to_key(label: str) -> str:
    name, language = label.rsplit(" ", 1)
    return f"{name}/{language}/Os"


def rebased(text: str, rows: dict) -> tuple[str, list[str]]:
    """(`text` with each row's value main's clocks, the labels `rows` has no clocks for: left as they were)."""
    out, missing = [], []
    for line in text.splitlines():
        found = LINE.match(line.split("#")[0].rstrip())
        key = label_to_key(found["label"]) if found else None
        if found and "clocks" in rows.get(key, {}):
            comment = f"  # main {rows[key]['clocks']}" if "#" in line else ""
            line = f'{found[1]} = {rows[key]["clocks"]}{comment}'
        elif found:
            missing.append(found["label"])
        out.append(line)
    return "\n".join(out) + "\n", missing


def main() -> int:
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    rows = json.loads(Path(sys.argv[1]).read_text())
    for name in ("main_clocks.toml", "clock_limits.toml"):
        path = HERE / name
        text, missing = rebased(path.read_text(), rows)
        path.write_text(text)
        print(f"{name}: rewritten" + (f"; no row for {len(missing)}: {', '.join(missing[:5])}" if missing else ""))
    return 0


if __name__ == "__main__":
    sys.exit(main())
