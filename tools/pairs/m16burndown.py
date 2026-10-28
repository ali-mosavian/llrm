#!/usr/bin/env python3
"""The m16 -Os burn-down: every program the tip makes bigger than main, with its cause and owner, summed per cause and per owner.

    tools/pairs/m16burndown.py BASE_DIST NEW_DIST --out FILE [--clocks | --bench BASE.json NEW.json] [--causes FILE]

One set: the programs of tools/sizes.py (tests/run/qb, bench, examples; demos left out) as this checkout's sources have them, built by both dists. Bytes are
sizes.py's measure. A program one side cannot compile is listed, not counted. Clocks are tools/bench/bench.py's 486-priced streams for the m16 languages of
the program's bench directory (bas, c, nib); `--clocks` runs the bench for each dist, `--bench` reads two earlier --json files. The causes and owners are
m16burndown_causes.toml: an `[[program]]` is a glob on the program name, the first match wins; a program matching none is `untriaged`, owner `unassigned`.
"""

from __future__ import annotations

import os
import math
import sys
import fnmatch
import argparse
import tempfile
import tomllib
import subprocess
import json
import re
from concurrent.futures import ThreadPoolExecutor
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


def excess_mode(environ: dict) -> str:
    """The `-fexcess-precision=` both dists build BASIC SINGLE with: $SIZES_EXCESS, else fast (main's own), and nothing else. sizes.py gives the one value to
    both sides; main compiled SINGLE as fast and the tip as standard when it was unset, so the board read the deliberate rounding as growth."""
    mode = environ.get("SIZES_EXCESS") or "fast"
    if mode not in ("fast", "standard"):
        raise SystemExit(f"SIZES_EXCESS={mode!r}: the board compares both sides in one mode, fast or standard")
    return mode


def knows_isel(dist: Path) -> bool:
    """Whether the dist's llrm-qb takes `--isel=`: main's does not, so it builds with its only selector."""
    import subprocess

    shown = subprocess.run([str(dist / "llrm-qb"), "-h"], capture_output=True, text=True)
    return "--isel" in shown.stdout + shown.stderr


def status_of(name: str, rules: list[dict]) -> tuple[str, str]:
    """(status, oracle) of the first rule matching `name`: status is open, cause known, pin-faithful or DEVIATE; oracle is what the verdict rests on
    (m32 llc, pin opt, or none). The pin's llc at code16 takes 32-bit pointers, so it is no 16-bit oracle: a verdict from it says `none`."""
    for rule in rules:
        if fnmatch.fnmatch(name, rule["match"]):
            return rule.get("status", "open"), rule.get("oracle", "none")
    return "open", "none"


def growers(both: list[str], base: dict, new: dict) -> list[str]:
    """The programs the new build makes bigger by even one byte, the largest growth first."""
    return sorted((n for n in both if new[n] > base[n]), key=lambda n: base[n] - new[n])


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


def disable_flags(dist: Path) -> list[str]:
    """Every `-fno-X` the compiler offers, from the list its own error message prints for an unknown pass."""
    done = subprocess.run([str(dist / "llrm-nib"), "-fno-m16burndown-probe", "/dev/null"], capture_output=True, text=True)
    named = re.search(r"the passes are (.*)", done.stderr)
    return ["-fno-" + word[2:].strip(", ") for word in named[1].split() if word.startswith("-f")] if named else []


# The off switches the pipeline reads from the environment (`env_flag!`): the second family, for what no -fno- pass flag reaches.
ENV_SWITCHES = ["LLRM_NO_SPLIT", "LLRM_NO_STACKS", "LLRM_NO_JUMPTHREADING", "LLRM_NO_COMBINE", "LLRM_NO_SROA", "LLRM_NO_MEM2REG"]


def switch_name(flag: str) -> str:
    return flag.removeprefix("-fno-").removeprefix("-disable-").removeprefix("LLRM_NO_").lower().replace("_", "-")


def ablate(dist: Path, level: str, names: list[str], flags: list[str]) -> dict[str, dict[str, int | None]]:
    """flag -> program -> bytes with that pass off, on `dist` alone. A compiler that does not know the flag gives None for its programs."""
    sizes = m16os.sizes_module()
    listed = [(n, c) for n, c in sizes.programs(dist, demos=False) if n in set(names)]
    found: dict[str, dict[str, int | None]] = {}
    for flag in flags:
        env = flag.startswith("LLRM_")
        if env:
            os.environ[flag] = "1"
        try:
            with ThreadPoolExecutor(8) as pool:
                found[flag] = dict(zip((n for n, _ in listed), pool.map(lambda one: sizes.measure(one[1] + ([] if env else [flag]), level), listed)))
        finally:
            os.environ.pop(flag, None)
        print(f"ablated {flag}", file=sys.stderr)
    return found


def attribute(tip: dict[str, int], ablated: dict[str, dict[str, int | None]], floor: int = 10) -> dict[str, str]:
    """program -> the flag whose removal shrinks it most (at least `floor` bytes), 'multi: a+b' where a second flag gets at least half as much, else 'untagged'.
    An upper bound: a pass switched off changes what the others see."""
    tagged = {}
    for name, size in tip.items():
        shrinks = sorted(((size - v[name], flag) for flag, v in ablated.items() if v.get(name)), reverse=True)
        best = [(gain, flag) for gain, flag in shrinks if gain >= floor]
        if not best:
            tagged[name] = "RA/peephole"
        elif len(best) > 1 and best[1][0] * 2 >= best[0][0]:
            tagged[name] = "multi: " + "+".join(sorted(switch_name(flag) for _, flag in best[:2]))
        else:
            tagged[name] = switch_name(best[0][1])
    return tagged


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
    parser.add_argument("--ablate", nargs="?", const="all", metavar="FLAGS", help="tag the growers by switching each pass off on the new dist (all of them, or the comma-separated -fno- flags); an upper bound, not a clean attribution")
    parser.add_argument("--ablation-cache", type=Path, help="read the ablation from this json when it exists, write it there when it does not")
    parser.add_argument("--isel", choices=["dag", "old"], default="dag", help="the selector (--isel=) both dists build with; a base that has no such flag builds with its default, and the header says so")
    parser.add_argument("--clocks", action="store_true")
    parser.add_argument("--bench", nargs=2, type=Path, metavar=("BASE_JSON", "NEW_JSON"))
    args = parser.parse_args()
    os.environ["SIZES_EXCESS"] = mode = excess_mode(os.environ)
    sizes = m16os.sizes_module()
    base_knows = knows_isel(args.base)
    if base_knows:
        os.environ["SIZES_ISEL"] = args.isel
    base = sizes.table(args.base, args.level, demos=False)
    os.environ["SIZES_ISEL"] = args.isel
    new = sizes.table(args.new, args.level, demos=False)
    # The default selector of the new dist, a column of its own: until the switch-over it is the old one.
    os.environ.pop("SIZES_ISEL")
    default = sizes.table(args.new, args.level, demos=False)
    os.environ["SIZES_ISEL"] = args.isel
    # The deliberate rounding: the new dist's BASIC programs at the other mode, a column of its own.
    os.environ["SIZES_EXCESS"] = "standard" if mode == "fast" else "fast"
    other = {n: v for n, v in sizes.table(args.new, args.level, demos=False).items() if n.endswith(".bas")}
    os.environ["SIZES_EXCESS"] = mode
    both = [n for n in base if base[n] and new.get(n)]
    before = after = {}
    if args.bench:
        before, after = (json.loads(p.read_text()) for p in args.bench)
    elif args.clocks:
        before = m16os.bench(args.base, args.level, Path(tempfile.mkdtemp(prefix="burndown-base-")))
        after = m16os.bench(args.new, args.level, Path(tempfile.mkdtemp(prefix="burndown-new-")))
    rules = tags(args.causes)
    flag_owner = {entry["flag"]: entry["owner"] for entry in tomllib.loads(args.causes.read_text()).get("flag", [])}
    ablated = tagged = None
    if args.ablate:
        cache = args.ablation_cache
        if cache and cache.exists():
            ablated = json.loads(cache.read_text())
            missing = [k for k in ENV_SWITCHES if k not in ablated] if args.ablate == "all" else []
            if missing:
                ablated |= ablate(args.new, args.level, [n for n in both if new[n] > base[n]], missing)
                cache.write_text(json.dumps(ablated))
            for same in ("-disable-lsr", "LLRM_NO_LICM"):  # the same switches as -fno-lsr and -fno-move-loop-invariants
                ablated.pop(same, None)
        else:
            ablated = ablate(args.new, args.level, [n for n in both if new[n] > base[n]], disable_flags(args.new) + ENV_SWITCHES if args.ablate == "all" else args.ablate.split(","))
            if cache:
                cache.write_text(json.dumps(ablated))
        tagged = attribute({n: new[n] for n in both if new[n] > base[n]}, ablated)
    grew = growers(both, base, new)
    level = args.level.lstrip("-")
    def classified(name: str) -> tuple[str, str]:
        cause, owner = tag_of(name, rules)
        if tagged and cause == "untriaged":
            cause = tagged[name]
            owner = "w-13/w-1" if cause == "RA/peephole" else flag_owner.get(cause.removeprefix("multi: ").split("+")[0], owner)
        return cause, owner

    rows = [(n, base[n], new[n], new[n] - base[n], clock_ratio(n, level, before, after), *classified(n), *status_of(n, rules)) for n in grew]
    excess = f"{mode} on both sides (the rounding column is the new dist at {'standard' if mode == 'fast' else 'fast'}, BASIC only)"
    out = [f"# m16 {args.level} burn-down", "", f"base {m16os.identify(args.base)}", f"new  {m16os.identify(args.new)}", f"SIZES_EXCESS {excess}", f"selector: base {args.isel if base_knows else 'its default (no --isel flag)'}, new {args.isel}; the default column is the new dist's own default", f"sources: this checkout at {source_state()}; the set is tools/sizes.py's programs, demos left out, {len(base)} programs",
           f"compared {len(both)}; base cannot compile {len(base) - sum(map(bool, base.values()))}, new cannot compile {len(new) - sum(map(bool, new.values()))}",
           f"TOTAL {sum(base[n] for n in both)} -> {sum(new[n] for n in both)} B ({sum(new[n] - base[n] for n in both):+}); {len(grew)} programs grew, {sum(new[n] < base[n] for n in both)} shrank, {sum(new[n] == base[n] for n in both)} same",
           f"LEFT {sum(r[3] for r in rows)} B in {len(rows)} growers (the bar: none)", ""]
    if ablated:
        out += ["## per flag (a pass switched off on the new dist; an upper bound, not a clean attribution)", "", "| flag | growers shrunk >= 10 B | bytes gained | all growers' total |", "|---|--:|--:|--:|"]
        for flag, v in sorted(ablated.items(), key=lambda kv: -sum(new[n] - x for n, x in kv[1].items() if x and new[n] - x >= 10)):
            gains = [new[n] - x for n, x in v.items() if x and new[n] - x >= 10]
            out.append(f"| {flag} | {len(gains)} | {sum(gains)} | {sum(x for x in v.values() if x) - sum(new[n] for n, x in v.items() if x):+} |")
        out.append("")
    for title, column in (("cause", 5), ("owner", 6)):
        totals: dict[str, list[tuple[int, str]]] = {}
        for row in rows:
            totals.setdefault(row[column], []).append((row[3], row[4]))
        out += [f"## per {title}", "", f"| {title} | programs left | bytes left | clocks tip/main (geomean, rows) |", "|---|--:|--:|--:|"]
        for key, v in sorted(totals.items(), key=lambda kv: -sum(d for d, _ in kv[1])):
            ratios = [float(c) for _, c in v if c not in ("-", "folded")]
            geo = f"{math.exp(sum(map(math.log, ratios)) / len(ratios)):.2f} ({len(ratios)})" if ratios else "-"
            out.append(f"| {key} | {len(v)} | {sum(d for d, _ in v)} | {geo} |")
        out.append("")
    languages: dict[str, list[int]] = {}
    for n in both:
        languages.setdefault(LANGUAGES.get(Path(n).suffix, "other"), []).append(n)
    out += ["## per language", "", "| language | compared | main B | tip B | delta | grew | bytes left in growers |", "|---|--:|--:|--:|--:|--:|--:|"]
    for language, names in sorted(languages.items()):
        grown = [n for n in names if new[n] > base[n]]
        out.append(f"| {language} | {len(names)} | {sum(base[n] for n in names)} | {sum(new[n] for n in names)} | {sum(new[n] - base[n] for n in names):+} | {len(grown)} | {sum(new[n] - base[n] for n in grown)} |")
    out.append("")
    out += ["## growers", "", "| program | main | tip | delta | default isel | rounding | clocks tip/main | cause | owner | status | oracle |", "|---|--:|--:|--:|--:|--:|--:|---|---|---|---|"]
    old_delta = lambda n, a: f"{default[n] - a:+}" if default.get(n) else "-"
    rounding = lambda n, b: f"{other[n] - b:+}" if other.get(n) else "-"
    out += [f"| {n} | {a} | {b} | {d:+} | {old_delta(n, a)} | {rounding(n, b)} | {c} | {cause} | {owner} | {status} | {oracle} |" for n, a, b, d, c, cause, owner, status, oracle in rows]
    out += ["", f"## ten biggest growers", ""] + [f"- {n} {d:+} B, {owner}, {status}" for n, _, _, d, _, _, owner, status, _ in sorted(rows, key=lambda r: -r[3])[:10]]
    args.out.write_text("\n".join(out) + "\n")
    print("\n".join(out[:12]))


if __name__ == "__main__":
    main()
