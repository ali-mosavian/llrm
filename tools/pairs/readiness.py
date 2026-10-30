#!/usr/bin/env python3
"""The main-merge readiness table, one command: every m16 program larger (-Os, -O2) or slower (clocks) on a feat/parity commit than on main.

    tools/pairs/readiness.py [NEW_SHA] [--base 854bb7efc4] [--out FILE] [--levels -Os -O2] [--no-clocks]

NEW_SHA defaults to origin/feat/parity. Each commit's dist is tools/pairs/pair_build.sh's `SCRATCH/sbp-<full sha>/dist`, built only if absent (a dist
already there is verified and kept, never rebuilt). Sizes are tools/sizes.py's (bytes of OMF code and data, demos left out); clocks are
tools/pairs/clocksweep.py's (the 486-priced instruction stream of every runnable program at -Os). Measured tables are cached per dist sha under
SCRATCH/readiness-cache, so a second run on the same two commits measures nothing. The owner column is left blank for the flag ablation.
"""

from __future__ import annotations

import os
import re
import sys
import json
import argparse
import subprocess
import importlib.util
from pathlib import Path

BRANCH = os.environ.get("LAND_BRANCH", "release/next")  # the integration branch (tools/landing/branch.sh)

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))
sys.path.insert(0, str(ROOT / "tools" / "pairs"))
BASE = "854bb7efc4"


def git(*args: str) -> str:
    return subprocess.run(["git", *args], cwd=ROOT, capture_output=True, text=True, check=True).stdout.strip()


def existing_dist(full: str, scratch: Path) -> Path | None:
    """A dist of `full` another session built under a shortened name (sbp-36ad593b8): its `ok` names the full sha it was built from, which is the proof; the directory name alone is not."""
    for name in (full, full[:10], full[:9], full[:8]):
        ok = scratch / f"sbp-{name}" / "ok"
        if ok.is_file() and (scratch / f"sbp-{name}" / "dist" / "llrm-c").is_file() and f"sha: {full}" in ok.read_text().splitlines():
            return scratch / f"sbp-{name}" / "dist"
    return None


def dist_of(sha: str, scratch: Path) -> Path:
    """`SCRATCH/sbp-<full sha>/dist`, built by pair_build.sh if absent (it verifies and keeps one that is there)."""
    full = git("rev-parse", sha)
    if found := existing_dist(full, scratch):
        return found
    done = subprocess.run(["bash", str(ROOT / "tools/pairs/pair_build.sh"), str(scratch), full], cwd=ROOT, capture_output=True, text=True)
    if done.returncode:
        sys.exit(f"readiness: no dist for {full[:10]}: {(done.stderr or done.stdout).strip()[-300:]}")
    return scratch / f"sbp-{full}" / "dist"


def sizes_module():
    spec = importlib.util.spec_from_file_location("sizes", ROOT / "tools" / "sizes.py")
    module = importlib.util.module_from_spec(spec)
    argv, sys.argv = sys.argv, ["sizes"]
    try:
        spec.loader.exec_module(module)
    finally:
        sys.argv = argv
    return module


def cached(path: Path, compute):
    """`path`'s JSON if it is there, else `compute()` stored in it: a table of a dist is measured once."""
    if path.exists():
        return json.loads(path.read_text())
    value = compute()
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value))
    return value


def size_table(dist: Path, level: str, cache: Path) -> dict:
    return cached(cache / f"sizes{level}-{dist.parent.name}.json", lambda: {k: v for k, v in sizes_module().table(dist, level, demos=False).items()})


def clock_rows(dist: Path, cache: Path) -> dict:
    def measure() -> dict:
        out = cache / f"clocks-{dist.parent.name}.tmp.json"
        subprocess.run([sys.executable, str(ROOT / "tools/pairs/clocksweep.py"), "measure", str(out)], env={**os.environ, "LLRM_BIN": str(dist)}, check=True)
        rows = json.loads(out.read_text())
        out.unlink()
        return rows

    return cached(cache / f"clocks-{dist.parent.name}.json", measure)


def cells(listing: str, function: str) -> int:
    """Distinct `[bp-N]` stack cells of `function` in a -S listing: the diagnostic the flip-conditions note quotes (quicksort.nib `_sort`: main 24, tip 39). Not a size."""
    body = listing.split(f"{function} proc", 1)[-1].split(f"{function} endp", 1)[0]
    return len(set(re.findall(r"\[bp-(\d+)\]", body)))


def growers(base: dict, new: dict) -> tuple[list[tuple[str, int, int]], int, int, int]:
    """([(program, base bytes, new bytes) larger in new, worst first], programs compared, total base, total new). A program one side cannot compile is not compared."""
    both = [name for name in base if base[name] and new.get(name)]
    larger = sorted(((name, base[name], new[name]) for name in both if new[name] > base[name]), key=lambda row: row[1] - row[2])
    return larger, len(both), sum(base[name] for name in both), sum(new[name] for name in both)


def slower(base: dict, new: dict) -> tuple[list[tuple[str, int, int]], int]:
    """([(program, base clocks, new clocks) slower in new, worst ratio first], programs measured on both). A program void on either side is not counted."""
    before, after = base["rows"], new["rows"]
    good = [name for name in sorted(set(before) & set(after)) if "clocks" in before[name] and "clocks" in after[name]]
    rows = [(name, before[name]["clocks"], after[name]["clocks"]) for name in good if after[name]["clocks"] > before[name]["clocks"]]
    return sorted(rows, key=lambda row: row[1] / row[2]), len(good)


def tiny(base: dict) -> set[str]:
    """Programs whose main run is under twice the least instruction count of their language (clocksweep's rule): within the start-up, or folded to a constant, so a ratio on them says nothing (x87_arguments.c: main folds the whole program, 237 clocks)."""
    good = {name: row for name, row in base["rows"].items() if "clocks" in row}
    floor: dict[str, int] = {}
    for row in good.values():
        floor[row["language"]] = min(floor.get(row["language"], 1 << 60), row["instructions"])
    return {name for name, row in good.items() if row["instructions"] < 2 * floor[row["language"]]}


FLAG_OWNERS = {"tree-ch": "w5", "lsr": "w3", "strength-reduce": "w3", "move-loop-invariants": "w2", "tree-pre": "w14", "sroa": "w14", "inline-functions": "w6"}  # the burn-down's flag -> owner sets (burndown-0652.md)


RA_OWNER = "w13 (RA/peephole)"  # coordinator's routing of the programs no pass flag moves; the peephole-class ones go to w1 after its bisect


def owner_of(name: str, rules: list[dict], ablation: dict[str, str]) -> str:
    """The owner column: the causes file's owner for the program (m16burndown_causes.toml, first match wins), else the owner of the flag whose removal shrinks it most (the ablation), else blank. `unassigned` and `untriaged` are blank: nobody holds the row. `RA/peephole` marks a program no pass flag moves by 10 B: nobody holds it either, but it is not untested."""
    import fnmatch

    for rule in rules:
        if fnmatch.fnmatch(name, rule["match"]) and rule.get("owner") not in (None, "unassigned"):
            return re.sub(r"^w-(\d+)$", r"w\1", rule["owner"])  # the causes file says w-2, the flag owners w2: one owner
    cause = ablation.get(name, "")
    if cause.startswith("multi: "):  # two flags move it about as much: the first one with an owner holds it
        return next((FLAG_OWNERS[flag] for flag in cause[len("multi: ") :].split("+") if flag in FLAG_OWNERS), "")
    return FLAG_OWNERS.get(cause) or (RA_OWNER if cause == "RA/peephole" else "")


def flag_first(raw: dict) -> dict[str, dict[str, int | None]]:
    """flag -> program -> bytes, from either json m16burndown wrote: that (`--ablation-cache`) or w13's program -> {base, tip, flag: bytes}."""
    if all(key.startswith(("-", "LLRM_")) for key in raw):
        return raw
    flags = {flag for row in raw.values() for flag in row if flag.startswith(("-", "LLRM_"))}
    return {flag: {name: row[flag] for name, row in raw.items() if flag in row} for flag in flags}


def ablation_baseline(new_sizes: dict, ablation_sizes=None) -> dict:
    """The sizes attribution subtracts the ablated sizes from: those of the commit the ablation was taken on (`ablation_sizes()`), else the new commit's. Against another commit's, a program that moved reads as a flag's gain: w14 showed +6 programs (+130 B) that were not there."""
    return {name: size for name, size in (ablation_sizes() if ablation_sizes else new_sizes).items() if size}


def by_owner(larger: list[tuple[str, int, int]], rules: list[dict], ablation: dict[str, str]) -> dict[str, list[tuple[str, int, int]]]:
    """owner -> its growers, the largest delta first; blank owners under `(none)`."""
    groups: dict[str, list[tuple[str, int, int]]] = {}
    for row in larger:
        groups.setdefault(owner_of(row[0], rules, ablation) or "(none)", []).append(row)
    return {owner: sorted(rows, key=lambda r: r[1] - r[2]) for owner, rows in sorted(groups.items(), key=lambda kv: sum(r[2] - r[1] for r in kv[1]), reverse=True)}


def owner_totals(larger: list[tuple[str, int, int]], rules: list[dict], ablation: dict[str, str]) -> dict[str, list[int]]:
    """owner -> [programs, bytes] of the larger list: what a later run is compared with."""
    return {owner: [len(rows), sum(b - a for _, a, b in rows)] for owner, rows in by_owner(larger, rules, ablation).items()}


def owner_deltas(before: dict[str, list[int]], after: dict[str, list[int]]) -> list[str]:
    """One line per owner whose programs or bytes moved between two runs, `owner: programs 12 -> 10 (-2), bytes +401 -> +300 (-101)`; owners new or gone count from zero."""
    lines = []
    for owner in sorted(set(before) | set(after)):
        (n0, b0), (n1, b1) = before.get(owner, [0, 0]), after.get(owner, [0, 0])
        if (n0, b0) != (n1, b1):
            lines.append(f"  {owner}: programs {n0} -> {n1} ({n1 - n0:+}), bytes {b0:+} -> {b1:+} ({b1 - b0:+})")
    return lines


def report(header: list[str], sized: dict[str, tuple], clocks: tuple | None, tiny_rows: set[str] = frozenset(), owners: tuple[list[dict], dict[str, str]] | None = None) -> list[str]:
    """The file: the headline first (totals, growers, slower), then one table per level and one for clocks; every table row ends in an empty owner column."""
    lines = list(header)
    for level, (larger, count, was, now) in sized.items():
        lines.append(f"HEADLINE m16 {level}: {was} -> {now} B ({now - was:+}); {len(larger)} of {count} programs larger")
    if clocks:
        rows, count = clocks
        lines.append(f"HEADLINE m16 -Os clocks: {len(rows)} of {count} programs slower ({sum(name in tiny_rows for name, _, _ in rows)} of them tiny: near the start-up or folded on main)")
    for level, (larger, count, was, now) in sized.items():
        if owners:
            lines += ["", f"larger at {level}, per owner (owner: programs, bytes):"]
            groups = by_owner(larger, *owners)
            lines += [f"  {owner}: {len(rows)} programs, {sum(b - a for _, a, b in rows):+} B" for owner, rows in groups.items()]
            for owner, rows in groups.items():
                lines += ["", f"larger at {level}, owner {owner} (program | main B | new B | delta | %):"]
                lines += [f"  {name:42} | {a:>7} | {b:>7} | {b - a:+6} | {(b - a) / a * 100:+6.1f}% |" for name, a, b in rows]
            continue
        lines += ["", f"larger at {level} (program | main B | new B | delta | % | owner):"]
        lines += [f"  {name:42} | {a:>7} | {b:>7} | {b - a:+6} | {(b - a) / a * 100:+6.1f}% |" for name, a, b in larger]
    if clocks:
        lines += ["", "slower at -Os (program | main clocks | new clocks | delta | % | owner):"]
        lines += [f"  {name + (' tiny' if name in tiny_rows else ''):42} | {a:>11} | {b:>11} | {b - a:+10} | {(b - a) / a * 100:+6.1f}% |" for name, a, b in clocks[0]]
    return lines


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("new", nargs="?", default=f"origin/{BRANCH}")
    parser.add_argument("--base", default=BASE)
    parser.add_argument("--out", type=Path)
    parser.add_argument("--levels", nargs="+", default=["-Os", "-O2"])
    parser.add_argument("--no-clocks", action="store_true")
    parser.add_argument("--previous", type=Path, help="the OUT.owners.json of an earlier run: the report ends with the per-owner delta (written beside every --out)")
    parser.add_argument("--causes", type=Path, help="m16burndown_causes.toml: fills the owner column and groups the larger lists by owner")
    parser.add_argument("--ablation-sha", help="the commit the --ablation file was taken on (default: NEW): attribution subtracts its sizes, so against another commit's sizes every program that moved reads as a flag's gain")
    parser.add_argument("--ablation", type=Path, help="m16burndown's ablation json (flag -> program -> bytes, plus base and tip): the flag owner for a program the causes file does not name")
    parser.add_argument("--scratch", type=Path, default=Path.home() / "scratch")
    args = parser.parse_args()
    cache = args.scratch / "readiness-cache"
    base_dist, new_dist = dist_of(args.base, args.scratch), dist_of(args.new, args.scratch)
    header = [f"main-merge readiness: main {git('rev-parse', args.base)[:10]} -> {git('rev-parse', args.new)[:10]}", f"dists {base_dist} {new_dist}"]
    sized = {level: growers(size_table(base_dist, level, cache), size_table(new_dist, level, cache)) for level in args.levels}
    base_clocks = None if args.no_clocks else clock_rows(base_dist, cache)
    clocks = None if args.no_clocks else slower(base_clocks, clock_rows(new_dist, cache))
    owners = None
    if args.causes:
        import tomllib

        ablation = {}
        if args.ablation:
            from m16burndown import attribute  # noqa: PLC0415

            ablation = attribute(ablation_baseline(size_table(new_dist, "-Os", cache), (lambda: size_table(dist_of(args.ablation_sha, args.scratch), "-Os", cache)) if args.ablation_sha else None), flag_first(json.loads(args.ablation.read_text())))
        owners = (tomllib.loads(args.causes.read_text()).get("program", []), ablation)
    lines = report(header, sized, clocks, tiny(base_clocks) if base_clocks else set(), owners)
    if owners and "-Os" in sized:
        totals = owner_totals(sized["-Os"][0], *owners)
        if args.out:
            args.out.with_suffix(".owners.json").write_text(json.dumps({"sha": git("rev-parse", args.new)[:10], "-Os": totals}))
        if args.previous:
            earlier = json.loads(args.previous.read_text())
            lines += ["", f"-Os larger, per owner, {earlier['sha']} -> {git('rev-parse', args.new)[:10]}:", *(owner_deltas(earlier["-Os"], totals) or ["  no owner moved"])]
    text = "\n".join(lines) + "\n"
    if args.out:
        args.out.write_text(text)
    print(text)
    return 0


if __name__ == "__main__":
    sys.exit(main())
