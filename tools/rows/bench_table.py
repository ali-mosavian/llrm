"""The bench gate's cells (every front end) of two compilers: instructions and code bytes per program, language and level.

    bench_table.py BASE.json MAIN.json     the files `tools/bench/bench.py --no-judge --json` writes
"""
import json
import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import rowcheck  # noqa: E402


def cells(path):
    with open(path) as handle:
        return json.load(handle)


def compare(base, main):
    """The cells present in both, grouped by language and level, with each counter that differs."""
    groups, changed = {}, []
    for key in sorted(set(base) & set(main)):
        language, level = key.rsplit("/", 2)[1:]
        group = groups.setdefault((language, level), [0, 0])
        group[0] += 1
        for counter in ("instructions", "code_bytes"):
            before, after = base[key].get(counter), main[key].get(counter)
            if before is None or after is None or before == after:
                continue
            group[1] += 1
            changed.append((key, counter, before, after))
    return groups, changed


def render(base, main, allow_failed=False):
    line, bad = rowcheck.census({"base": base, "main": main}, counters=(), checked=True)
    lines = rowcheck.require(line, bad, allow_failed) + [""]
    for side, cells_ in (("base", base), ("main", main)):
        bare = sorted(key for key, cell_ in cells_.items() if "error" not in cell_ and "instructions" not in cell_ and cell_.get("checked"))
        if bare:
            lines.append(f"{side}: {len(bare)} rows with no instructions (run and output-checked): " + ", ".join(bare))
    groups, changed = compare(base, main)
    lines += ["| language | level | cells | counters changed |", "|---|---|--:|--:|"]
    lines += [f"| {language} | {level} | {n} | {c} |" for (language, level), (n, c) in sorted(groups.items())]
    only = sorted(set(base) ^ set(main))
    lines.append(f"\ncells in one side only: {len(only)}")
    lines += [f"{key} {counter}: {before} -> {after} ({(after / before - 1) * 100:+.2f}%)" for key, counter, before, after in changed]
    return "\n".join(lines)


if __name__ == "__main__":
    print(render(cells(sys.argv[1]), cells(sys.argv[2]), "--allow-failed" in sys.argv))
