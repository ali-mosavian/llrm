#!/usr/bin/env python3
"""The 16-bit -Os table of two builds: bytes per program, the functions that grew, and (with --clocks) the 486-priced clocks of tools/bench.

    tools/pairs/m16os.py BASE_DIST NEW_DIST [--level -Os] [--top 30] [--clocks] [--out FILE]

BASE_DIST and NEW_DIST are dist directories (llrm-llc, llrm-qb, llrm-nib, nibfront). The header names both builds: the sha in the directory name or
`llrm-llc --version`, and the md5 of each binary. Bytes are `tools/sizes.py`'s measure (OMF code and data emitted, linked-in runtime not counted), per
program, the demos left out. A program one side cannot compile is listed, not compared. For the `--top` growers each function's instruction count in the
`-S` listing is compared (a proxy: bytes per function are not in the object), the functions that grew most first, and those only one side has.
`--clocks` runs tools/bench/bench.py (unicorn instruction streams priced by the 486 table) with each build and joins its rows by name.
"""

from __future__ import annotations

import os
import re
import sys
import json
import hashlib
import argparse
import subprocess
import tempfile
import importlib.util
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor

FOLDED_AT = 2  # a kernel of at most this many instructions is a constant the optimiser returned: a folded row, never a speedup
ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))


def sizes_module():
    spec = importlib.util.spec_from_file_location("sizes", ROOT / "tools" / "sizes.py")
    module = importlib.util.module_from_spec(spec)
    argv, sys.argv = sys.argv, ["sizes"]
    try:
        spec.loader.exec_module(module)
    finally:
        sys.argv = argv
    return module


def md5(path: Path) -> str:
    return hashlib.md5(path.read_bytes()).hexdigest()[:12] if path.is_file() else "-"


def identify(dist: Path) -> str:
    """`DIST: sha (from llrm-llc --version, else the directory's name), md5 of llrm-llc llrm-qb llrm-nib`."""
    done = subprocess.run([str(dist / "llrm-llc"), "--version"], capture_output=True, text=True)
    words = done.stdout.split()
    named = re.search(r"sbp-([0-9a-f]{40})", str(dist.resolve()))
    sha = words[1] if done.returncode == 0 and len(words) == 2 else (named[1] + " (from the directory name, no build stamp)" if named else "unknown")
    return f"{dist}: {sha}; md5 " + " ".join(f"{name} {md5(dist / name)}" for name in ("llrm-llc", "llrm-qb", "llrm-nib"))


PROC = re.compile(r"^(\S+)\s+proc\b")
ENDP = re.compile(r"^(\S+)\s+endp\b")


def functions(listing: str) -> dict[str, int]:
    """name -> instructions between its `proc` and `endp` (labels, directives and comments are not instructions)."""
    found: dict[str, int] = {}
    current = None
    for raw in listing.splitlines():
        line = raw.strip()
        if not line or line.startswith((";", ".")):
            continue
        opened, closed = PROC.match(line), ENDP.match(line)
        if opened:
            current = opened[1]
            found[current] = 0
        elif closed:
            current = None
        elif current is not None and not line.endswith(":") and not line.startswith(("public", "extern")):
            found[current] += 1
    return found


def listing(command: list[str], level: str) -> dict[str, int] | None:
    with tempfile.TemporaryDirectory() as directory:
        out = f"{directory}/x.s"
        done = subprocess.run([*command, level, "-S", "-o", out], capture_output=True, text=True, timeout=600)
        return functions(Path(out).read_text()) if done.returncode == 0 and Path(out).exists() else None


def bench(dist: Path, level: str, work: Path) -> dict:
    """bench.py's rows for this build (its --json), run against LLRM_BIN=dist."""
    work.mkdir(parents=True, exist_ok=True)
    out = work / "bench.json"
    env = dict(os.environ, LLRM_BIN=str(dist))
    subprocess.run(
        ["uv", "run", "--project", str(ROOT / "tools"), "python", str(ROOT / "tools/bench/bench.py"), "--opt", level.lstrip("-"), "--no-judge", "--json", str(out), "--work", str(work / "w")],
        env=env, cwd=ROOT, check=True, capture_output=True, text=True,
    )
    return json.loads(out.read_text())


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("base", type=Path)
    parser.add_argument("new", type=Path)
    parser.add_argument("--level", default="-Os")
    parser.add_argument("--top", type=int, default=30)
    parser.add_argument("--clocks", action="store_true")
    parser.add_argument("--out", type=Path)
    args = parser.parse_args()
    lines = [f"m16 {args.level}: bytes per program (tools/sizes.py measure), base against new", f"base {identify(args.base)}", f"new  {identify(args.new)}", ""]
    sizes = sizes_module()
    base, new = sizes.table(args.base, args.level, demos=False), sizes.table(args.new, args.level, demos=False)
    both = [name for name in base if base[name] and new.get(name)]
    lines.append(f"programs compared {len(both)}; base cannot compile {[n for n in base if not base[n]]}; new cannot compile {[n for n in new if not new.get(n)]}")
    lines.append(f"TOTAL {sum(base[n] for n in both)} -> {sum(new[n] for n in both)} B; grew {sum(new[n] > base[n] for n in both)}, shrank {sum(new[n] < base[n] for n in both)}, same {sum(new[n] == base[n] for n in both)}")
    grew = sorted((n for n in both if new[n] > base[n]), key=lambda n: base[n] - new[n])
    lines += ["", "program bytes (base -> new, delta, percent):"]
    lines += [f"  {n:40} {base[n]:>7} -> {new[n]:>7} ({new[n] - base[n]:+}, {(new[n] - base[n]) / base[n] * 100:+.0f}%)" for n in grew]
    lines += ["", f"the {args.top} largest growers: functions (instructions in the -S listing, a proxy for bytes):"]
    commands_base, commands_new = dict(sizes.programs(args.base, demos=False)), dict(sizes.programs(args.new, demos=False))
    with ThreadPoolExecutor(8) as pool:
        top = grew[: args.top]
        pairs = list(pool.map(lambda n: (listing(commands_base[n], args.level), listing(commands_new[n], args.level)), top))
    for name, (was, now) in zip(top, pairs):
        lines.append(f"  {name} ({base[name]} -> {new[name]} B)")
        if was is None or now is None:
            lines.append("    no listing on one side")
            continue
        moved = sorted(((now.get(f, 0) - was.get(f, 0), f) for f in set(was) | set(now) if now.get(f, 0) != was.get(f, 0)), reverse=True)
        for delta, function in moved[:4]:
            lines.append(f"    {function}: {was.get(function, 'absent')} -> {now.get(function, 'absent')} ({delta:+})")
    if args.clocks:
        before, after = bench(args.base, args.level, Path(tempfile.mkdtemp(prefix="m16os-base-"))), bench(args.new, args.level, Path(tempfile.mkdtemp(prefix="m16os-new-")))
        lines += ["", "bench rows, slowest first (rows with a folded kernel are listed after, never ratioed; clocks and code bytes, base / new / ratio; rows one side cannot build or count follow):"]
        timed = lambda row: isinstance(row, dict) and "clocks" in row and row.get("instructions", 3) > FOLDED_AT
        both = sorted((k for k in before if timed(before[k]) and timed(after.get(k))), key=lambda k: before[k]["clocks"] / after[k]["clocks"])
        lines += [f"  {k:34} clocks {before[k]['clocks']:>11} {after[k]['clocks']:>11} {after[k]['clocks'] / before[k]['clocks']:5.2f}x   bytes {before[k]['code_bytes']:>5} {after[k]['code_bytes']:>5} {after[k]['code_bytes'] / before[k]['code_bytes']:5.2f}x" for k in both]
        lines.append(f"  clocks: {sum(after[k]['clocks'] > before[k]['clocks'] for k in both)} slower, {sum(after[k]['clocks'] < before[k]['clocks'] for k in both)} faster of {len(both)}")
        said = lambda rows, k: "absent" if k not in rows else "counted" if timed(rows[k]) else "folded" if rows[k].get("instructions", 3) <= FOLDED_AT and "error" not in rows[k] else str(rows[k].get("error", "no clocks"))[:90]
        lines += [f"  {k}: base {said(before, k)}; new {said(after, k)}" for k in sorted(set(before) | set(after)) if k not in both]
    text = "\n".join(lines) + "\n"
    if args.out:
        args.out.write_text(text)
    print(text)


if __name__ == "__main__":
    main()
