#!/usr/bin/env python3
"""Whether two llrm-c builds write the same object bytes over the vsgcc programs, per target and level.

    [OBJSWEEP_FLAGS="--isel=dag"] tools/pairs/objsweep.py OLD_LLRM_C NEW_LLRM_C [--src DIR] [--levels O1,O2,O3,Os] [--targets m32,m16] [--jobs N]

DIR holds the wrapped sources vsgcc compiles (`ROWS_WORK/<name>/b/*.c`, written by crates/target/llrm-x86-m32/vsgcc/build.sh).
Prints one line: `objsweep old <md5> new <md5>: N objects changed (targets, levels)`, then the changed objects with their sizes and every
compile that failed on a side with the first line of its error. Each compile also runs with LLRM_OBJECT_ALL_WRITERS: the object goes through every writer of its
architecture (OMF, ELF, COFF, Mach-O) and each result is compared, so a change in a writer the target does not choose by default shows too. `--arm64 DIR` sweeps
`.ll` files through `llrm-arm64-llc` the same way. A compile that fails on both sides is counted apart and never as unchanged:
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
SELECTOR_PATHS = re.compile(r"selectiondag|dagisel|/isel|isel\.rs|/shadow|crates/target/|schedul|crates/codegen/llrm-codegen/src/backend/")


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


CC_TARGETS = {"m32": "x86_32-wat", "m16": "x86_16-wat"}  # llrm-cc's LLRM_CC_TARGET rows
FLAGS = {"m32": ["-m32", "-mabi=sysv", "-march=i486"], "m16": ["-m16", "-march=i386"], "m64": ["-m64"]}  # llrm-c rejects -march=i286 on m16, and takes no -m64: name it only for llrm-cc


# The -g leg: the debug formats each target writes. An object holds its debug sections, so comparing whole objects compares them.
DEBUG_LEGS = {
    "m32": [["-g", "-fobject-format=elf"], ["-gdwarf-2", "-fobject-format=elf"], ["-gcodeview", "-fobject-format=coff"], ["-g", "-fobject-format=omf"]],
    "m16": [["-g"], ["-gtd"], ["-gcodeview"], ["-gdwarf-5"]],
    "m64": [["-g", "-fobject-format=elf"]],
}


def writer_results(directory: Path) -> dict[str, str]:
    """The md5 of each `<format>.bin` the compile wrote through LLRM_OBJECT_ALL_WRITERS: every writer of the object's architecture, not only the one the target chose."""
    # A writer that refused the object leaves `<format>.err`: its message is compared like bytes, so a refusal on one side shows.
    return {file.name: hashlib.md5(file.read_bytes()).hexdigest()[:12] for pattern in ("*.bin", "*.err") for file in sorted(directory.glob(pattern))}


def run_compiler(command: list[str], out: Path, writers: dict[str, str] | None, environment: dict[str, str] | None = None) -> tuple[bytes | None, str]:
    """(object bytes, '') or (None, first line of stderr); fills `writers` with the other formats' md5s when given."""
    side = out.with_suffix(".writers")
    side.mkdir(exist_ok=True)
    env = {**(environment or os.environ), "LLRM_OBJECT_ALL_WRITERS": str(side)} if writers is not None else environment
    done = subprocess.run(command, capture_output=True, text=True, timeout=600, env=env)
    if writers is not None:
        writers.update(writer_results(side))
    for file in side.glob("*"):
        file.unlink()
    side.rmdir()
    if done.returncode != 0 or not out.exists():
        return None, (done.stderr.strip().splitlines() or [f"exit {done.returncode}"])[-1][:120]
    data = out.read_bytes()
    out.unlink()
    return data, ""


def compile_one(binary: str, source: Path, target: str, level: str, out: Path, isel: str = "old", writers: dict[str, str] | None = None, extra: list[str] | None = None) -> tuple[bytes | None, str]:
    """(object bytes, '') or (None, first line of stderr). `out` ends in `.o`: llrm-c writes any other name as assembly text."""
    cc = Path(binary).name == "llrm-cc"
    # llrm-cc links without -c, takes its Watcom target from LLRM_CC_TARGET (none at m64) and the m16 clang only at m16.
    environment = None
    if cc:
        environment = {key: value for key, value in os.environ.items() if key not in ("LLRM_CC_TARGET", "LLRM_CLANG_M16") or (key == "LLRM_CLANG_M16" and target == "m16")}
        if target in CC_TARGETS:
            environment["LLRM_CC_TARGET"] = CC_TARGETS[target]
    command = [binary, *FLAGS[target], f"--isel={isel}", *os.environ.get("OBJSWEEP_FLAGS", "").split(), *(extra or []), f"-{level}", *(["-c"] if cc else []), "-o", str(out), str(source)]
    return run_compiler(command, out, writers, environment)


def compile_arm64(binary: str, source: Path, out: Path, writers: dict[str, str] | None = None) -> tuple[bytes | None, str]:
    """`llrm-arm64-llc` on a `.ll` file: the object of its triple's platform (Mach-O or ELF), and with `writers` the other format's too."""
    return run_compiler([binary, "-o", str(out), str(source)], out, writers)


def summary(changed: list[tuple[str, int, int]], top: int = 5) -> list[str]:
    """Lines splitting the changed objects (name `prog target -level`, old size, new size) per target and level into larger, smaller and
    same-size (bytes differ, length does not), then the `top` largest growths."""
    cells: dict[str, list[int]] = {}
    for name, old, new in changed:
        cell = " ".join(name.split()[1:])
        row = cells.setdefault(cell, [0, 0, 0])
        row[0 if new > old else 1 if new < old else 2] += 1
    lines = [f"{cell}: larger {a}, smaller {b}, same-size {c}" for cell, (a, b, c) in sorted(cells.items())]
    lines.append("largest growths:")
    grown = sorted((c for c in changed if c[2] > c[1]), key=lambda c: c[1] - c[2])[:top]
    lines += [f"  {name}: {new - old:+d}" for name, old, new in grown]
    return lines


def md5(path: str) -> str:
    return hashlib.md5(Path(path).read_bytes()).hexdigest()[:12]


def sweep(args, sources: list[Path], isel: str) -> int:
    legs = lambda target: DEBUG_LEGS[target] if getattr(args, "debug", False) else [[]]  # noqa: E731
    cells = [(s, t, lv, tuple(leg)) for s in sources for t in args.targets.split(",") for lv in args.levels.split(",") for leg in legs(t)]
    scratch = Path(tempfile.mkdtemp(prefix="objsweep-"))

    def run(cell):
        source, target, level, leg = cell
        tag = f"{source.stem}-{target}-{level}" + "".join(re.sub(r"\W", "", flag) for flag in leg)
        old_writers, new_writers = {}, {}
        old = compile_one(args.old, source, target, level, scratch / f"{tag}.old.o", isel, old_writers, list(leg))
        new = compile_one(args.new, source, target, level, scratch / f"{tag}.new.o", isel, new_writers, list(leg))
        return cell, old, new, old_writers, new_writers

    changed, one_side, both, same = [], [], [], 0
    with ThreadPoolExecutor(args.jobs) as pool:
        for (source, target, level, leg), (a, ea), (b, eb), wa, wb in pool.map(run, cells):
            name = f"{source.stem} {target} -{level}" + "".join(f" {flag}" for flag in leg)
            if a is None and b is None:
                both.append((name, ea))
            elif a is None or b is None:
                one_side.append((name, f"old: {ea or 'ok'}; new: {eb or 'ok'}"))
            elif a == b and wa == wb:
                same += 1
            else:
                other = sorted(f for f in {*wa, *wb} if wa.get(f) != wb.get(f))
                changed.append((name, len(a), len(b), other))
    print(f"objsweep --isel={isel} old {md5(args.old)} new {md5(args.new)}: {len(changed)} objects changed ({args.targets}, {args.levels}); {same} identical, {len(both)} failed on both sides, {len(one_side)} failed on one side of {len(cells)}")
    for name, x, y, other in changed:
        print(f"  changed {name}: {x} -> {y} bytes ({y - x:+d})" + (f"; writers: {', '.join(other)}" if other else ""))
    print("split:")
    for line in summary([(name, x, y) for name, x, y, _ in changed]):
        print(f"  {line}")
    for name, why in one_side:
        print(f"  ONE-SIDED FAIL {name}: {why}")
    for name, why in both[:10]:
        print(f"  failed on both {name}: {why}")
    return 1 if changed or one_side else 0

def sweep_arm64(args, sources: list[Path]) -> int:
    """The `.ll` files of --arm64 through `llrm-arm64-llc` beside OLD and beside NEW: the Mach-O and ELF objects of each must match."""
    old_bin, new_bin = (str(Path(one).resolve().parent / "llrm-arm64-llc") for one in (args.old, args.new))
    missing = [one for one in (old_bin, new_bin) if not Path(one).exists()]
    if missing:
        print(f"objsweep --arm64: no llrm-arm64-llc at {', '.join(missing)}: the arm64 objects were NOT compared")
        return 1
    scratch = Path(tempfile.mkdtemp(prefix="objsweep-arm64-"))

    def run(source):
        old_writers, new_writers = {}, {}
        old = compile_arm64(old_bin, source, scratch / f"{source.stem}.old.o", old_writers)
        new = compile_arm64(new_bin, source, scratch / f"{source.stem}.new.o", new_writers)
        return source, old, new, old_writers, new_writers

    changed, failed, same = [], [], 0
    with ThreadPoolExecutor(args.jobs) as pool:
        for source, (a, ea), (b, eb), wa, wb in pool.map(run, sources):
            if a is None or b is None:
                failed.append((source.stem, f"old: {ea or 'ok'}; new: {eb or 'ok'}"))
            elif a == b and wa == wb:
                same += 1
            else:
                changed.append((source.stem, sorted(f for f in {*wa, *wb} if wa.get(f) != wb.get(f)) or ["object"]))
    print(f"objsweep --arm64: {len(changed)} objects changed; {same} identical, {len(failed)} failed of {len(sources)}")
    for name, formats in changed:
        print(f"  changed {name}: {', '.join(formats)}")
    for name, why in failed[:10]:
        print(f"  failed {name}: {why}")
    return 1 if changed else 0


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
    parser.add_argument("--debug", action="store_true", help="the -g leg: compile with each debug format of the target (DEBUG_LEGS) instead of without -g")
    parser.add_argument("--arm64", help="a directory of `.ll` files to compile with llrm-arm64-llc (beside OLD and NEW), as Mach-O and ELF")
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
    status = max(sweep(args, sources, isel) for isel in chosen)
    if args.arm64:
        status = max(status, sweep_arm64(args, sorted(Path(args.arm64).glob("*.ll"))))
    return status


if __name__ == "__main__":
    sys.exit(main())
