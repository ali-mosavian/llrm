#!/usr/bin/env python3
"""Whether two llrm-c builds write the same object bytes over the vsgcc programs, per target and level.

    tools/pairs/objsweep.py OLD_LLRM_C NEW_LLRM_C [--src DIR] [--levels O1,O2,O3,Os] [--targets m32,m16] [--jobs N]

DIR holds the wrapped sources vsgcc compiles (`ROWS_WORK/<name>/b/*.c`, written by crates/target/llrm-x86-m32/vsgcc/build.sh).
Prints one line: `objsweep old <md5> new <md5>: N objects changed (targets, levels)`, then the changed objects with their sizes and every
compile that failed on a side with the first line of its error. A compile that fails on both sides is counted apart and never as unchanged:
a sweep where half the compiles failed says nothing about those objects. Exit 1 on a difference or a failure on one side only.
"""
from __future__ import annotations

import argparse
import hashlib
import os
import re
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

# Code whose change moves the objects of the DAG selector or of the machine passes after it, not only of the old selector.
SELECTOR_PATHS = re.compile(r"selectiondag|dagisel|/isel|isel\.rs|/shadow|crates/target/|schedul|crates/backend/llrm-core/src/backend/")


def selectors(named: str | None, changed: list[str] | None) -> list[str]:
    """The selectors to sweep. Unnamed, a diff that touches selector, x86-action, scheduler or machine-pass code is swept under both: the old selector alone
    showed 0 objects changed where `--isel=dag` showed 38 of 284. A diff that cannot be read is refused, never assumed to leave the DAG alone."""
    if named:
        return ["old", "dag"] if named == "both" else [named]
    if changed is None:
        raise SystemExit("objsweep: cannot read the diff between the two builds' shas; name the selector: --isel=old|dag|both")
    return ["old", "dag"] if any(SELECTOR_PATHS.search(path) for path in changed) else ["old"]


def changed_files(old_sha: str, new_sha: str) -> list[str] | None:
    done = subprocess.run(["git", "diff", "--name-only", old_sha, new_sha], cwd=Path(__file__).resolve().parent, capture_output=True, text=True)
    return done.stdout.split() if done.returncode == 0 else None


FLAGS = {"m32": ["-m32", "-mabi=sysv", "-march=i486"], "m16": ["-m16", "-march=i386"]}  # llrm-c rejects -march=i286 on m16


def compile_one(binary: str, source: Path, target: str, level: str, out: Path, isel: str = "old") -> tuple[bytes | None, str]:
    """(object bytes, '') or (None, first line of stderr). `out` ends in `.o`: llrm-c writes any other name as assembly text."""
    done = subprocess.run([binary, *FLAGS[target], f"--isel={isel}", f"-{level}", "-o", str(out), str(source)], capture_output=True, text=True, timeout=600)
    if done.returncode != 0 or not out.exists():
        return None, (done.stderr.strip().splitlines() or [f"exit {done.returncode}"])[-1][:120]
    data = out.read_bytes()
    out.unlink()
    return data, ""


def md5(path: str) -> str:
    return hashlib.md5(Path(path).read_bytes()).hexdigest()[:12]


def sweep(args, sources: list[Path], isel: str) -> int:
    cells = [(s, t, lv) for s in sources for t in args.targets.split(",") for lv in args.levels.split(",")]
    scratch = Path(tempfile.mkdtemp(prefix="objsweep-"))

    def run(cell):
        source, target, level = cell
        tag = f"{source.stem}-{target}-{level}"
        old = compile_one(args.old, source, target, level, scratch / f"{tag}.old.o", isel)
        new = compile_one(args.new, source, target, level, scratch / f"{tag}.new.o", isel)
        return cell, old, new

    changed, one_side, both, same = [], [], [], 0
    with ThreadPoolExecutor(args.jobs) as pool:
        for (source, target, level), (a, ea), (b, eb) in pool.map(run, cells):
            name = f"{source.stem} {target} -{level}"
            if a is None and b is None:
                both.append((name, ea))
            elif a is None or b is None:
                one_side.append((name, f"old: {ea or 'ok'}; new: {eb or 'ok'}"))
            elif a == b:
                same += 1
            else:
                changed.append((name, len(a), len(b)))
    print(f"objsweep --isel={isel} old {md5(args.old)} new {md5(args.new)}: {len(changed)} objects changed ({args.targets}, {args.levels}); {same} identical, {len(both)} failed on both sides, {len(one_side)} failed on one side of {len(cells)}")
    for name, x, y in changed:
        print(f"  changed {name}: {x} -> {y} bytes ({y - x:+d})")
    for name, why in one_side:
        print(f"  ONE-SIDED FAIL {name}: {why}")
    for name, why in both[:10]:
        print(f"  failed on both {name}: {why}")
    return 1 if changed or one_side else 0



def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("old")
    parser.add_argument("new")
    parser.add_argument("--src", default=str(Path(os.environ.get("ROWS_WORK", Path.home() / "scratch/rows")) / "base/b"))
    parser.add_argument("--levels", default="O1,O2,O3,Os")
    parser.add_argument("--targets", default="m32,m16")
    parser.add_argument("--programs", default="", help="comma-separated names; default every .c in --src")
    parser.add_argument("--cross-branch", action="store_true", help="compare a base that is not an ancestor of the new build (the report header says so)")
    parser.add_argument("--isel", choices=["old", "dag", "both", "shadow"], help="the selector; unnamed, a diff touching selector or machine-pass code is swept under both")
    parser.add_argument("--jobs", type=int, default=int(os.environ.get("JOBS", "6")))
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
    import stamp

    args, named, allow = parser.parse_args(stamp.options(sys.argv[1:])[0]), *stamp.options(sys.argv[1:])[1:]
    old_sha = stamp.check(args.old, named["base_sha"], allow, "old")
    new_sha = stamp.check(args.new, named["sha"], allow, "new")
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import ancestry  # noqa: PLC0415

    print(ancestry.header(old_sha, new_sha, args.cross_branch))
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import voidpair

    if why := voidpair.void(args.old, args.new):
        print(why)
        return 2
    sources = sorted(Path(args.src).glob("*.c"))
    if args.programs:
        keep = set(args.programs.split(","))
        sources = [s for s in sources if s.stem in keep]
    if not sources:
        print(f"no sources under {args.src}")
        return 2
    old_sha_files = changed_files(old_sha, new_sha)
    chosen = selectors(args.isel, old_sha_files)
    if not args.isel:
        print(f"isel: {'+'.join(chosen)} (not named; the diff {'touches selector or machine-pass code' if len(chosen) > 1 else 'touches none'})")
    return max(sweep(args, sources, isel) for isel in chosen)


if __name__ == "__main__":
    sys.exit(main())
