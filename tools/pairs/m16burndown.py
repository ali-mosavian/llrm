#!/usr/bin/env python3
"""The m16 -Os burn-down: every program the tip makes bigger than main, with its cause and owner, summed per cause and per owner.

    tools/pairs/m16burndown.py BASE_DIST NEW_DIST --out FILE [--clocks | --bench BASE.json NEW.json] [--causes FILE]

One set: the programs of tools/sizes.py (tests/run/qb, bench, examples; demos left out) as this checkout's sources have them, built by both dists. Bytes are
sizes.py's measure. A program one side cannot compile is listed, not counted. Clocks are tools/bench/bench.py's 486-priced streams for the m16 languages of
the program's bench directory (bas, c, nib); `--clocks` runs the bench for each dist, `--bench` reads two earlier --json files. The causes and owners are
m16burndown_causes.toml: an `[[program]]` is a glob on the program name, the first match wins; a program matching none is `untriaged`, owner `unassigned`.
"""

from __future__ import annotations

import sys
import fnmatch
import argparse
import tempfile
import tomllib
import subprocess
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import m16os  # noqa: E402

LANGUAGES = {".bas": "bas", ".c": "c", ".nib": "nib"}


def tags(path: Path) -> list[dict]:
    return tomllib.loads(path.read_text()).get("program", [])


def tag_of(name: str, rules: list[dict]) -> tuple[str, str]:
    for rule in rules:
        if fnmatch.fnmatch(name, rule["match"]):
            return rule["cause"], rule["owner"]
    return "untriaged", "unassigned"


def clock_ratio(name: str, level: str, before: dict, after: dict) -> str:
    """new/base clocks of the program's bench row, '-' where it has none (tests/run and examples programs are not in bench), 'folded' where a kernel is a constant."""
    directory, _, file = name.partition("/")
    if directory in ("qb", "examples"):
        return "-"
    language = LANGUAGES.get(Path(file).suffix)
    rows = [k for k in before if k.endswith(f"/{language}/{level}") and k.split("/")[-3] == directory]
    if not rows:
        return "-"
    was, now = before[rows[0]], after.get(rows[0], {})
    if now.get("instructions", 3) <= m16os.FOLDED_AT and "error" not in now:
        return "folded"
    if "clocks" not in was or "clocks" not in now:
        return "-"
    return f"{now['clocks'] / was['clocks']:.2f}"


def source_state() -> str:
    head = subprocess.run(["git", "rev-parse", "--short=10", "HEAD"], cwd=m16os.ROOT, capture_output=True, text=True).stdout.strip()
    dirty = subprocess.run(["git", "status", "--porcelain", "--untracked-files=no"], cwd=m16os.ROOT, capture_output=True, text=True).stdout.strip()
    return head + (" + uncommitted changes" if dirty else "")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("base", type=Path)
    parser.add_argument("new", type=Path)
    parser.add_argument("--level", default="-Os")
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--causes", type=Path, default=HERE / "m16burndown_causes.toml")
    parser.add_argument("--clocks", action="store_true")
    parser.add_argument("--bench", nargs=2, type=Path, metavar=("BASE_JSON", "NEW_JSON"))
    args = parser.parse_args()
    sizes = m16os.sizes_module()
    base, new = sizes.table(args.base, args.level, demos=False), sizes.table(args.new, args.level, demos=False)
    both = [n for n in base if base[n] and new.get(n)]
    before = after = {}
    if args.bench:
        before, after = (json.loads(p.read_text()) for p in args.bench)
    elif args.clocks:
        before = m16os.bench(args.base, args.level, Path(tempfile.mkdtemp(prefix="burndown-base-")))
        after = m16os.bench(args.new, args.level, Path(tempfile.mkdtemp(prefix="burndown-new-")))
    rules = tags(args.causes)
    grew = sorted((n for n in both if new[n] > base[n]), key=lambda n: base[n] - new[n])
    level = args.level.lstrip("-")
    rows = [(n, base[n], new[n], new[n] - base[n], clock_ratio(n, level, before, after), *tag_of(n, rules)) for n in grew]
    out = [f"# m16 {args.level} burn-down", "", f"base {m16os.identify(args.base)}", f"new  {m16os.identify(args.new)}", f"sources: this checkout at {source_state()}; the set is tools/sizes.py's programs, demos left out, {len(base)} programs",
           f"compared {len(both)}; base cannot compile {len(base) - sum(map(bool, base.values()))}, new cannot compile {len(new) - sum(map(bool, new.values()))}",
           f"TOTAL {sum(base[n] for n in both)} -> {sum(new[n] for n in both)} B ({sum(new[n] - base[n] for n in both):+}); {len(grew)} programs grew, {sum(new[n] < base[n] for n in both)} shrank, {sum(new[n] == base[n] for n in both)} same",
           f"LEFT {sum(r[3] for r in rows)} B in {len(rows)} growers (the bar: none)", ""]
    for title, column in (("cause", 5), ("owner", 6)):
        totals: dict[str, list[int]] = {}
        for row in rows:
            totals.setdefault(row[column], []).append(row[3])
        out += [f"## per {title}", "", f"| {title} | programs left | bytes left |", "|---|--:|--:|"]
        out += [f"| {key} | {len(v)} | {sum(v)} |" for key, v in sorted(totals.items(), key=lambda kv: -sum(kv[1]))] + [""]
    out += ["## growers", "", "| program | main | tip | delta | clocks tip/main | cause | owner |", "|---|--:|--:|--:|--:|---|---|"]
    out += [f"| {n} | {a} | {b} | {d:+} | {c} | {cause} | {owner} |" for n, a, b, d, c, cause, owner in rows]
    args.out.write_text("\n".join(out) + "\n")
    print("\n".join(out[:12]))


if __name__ == "__main__":
    main()
