#!/usr/bin/env python3
"""Whether two llrm-c builds write the same bytes: every program and every QCport module, at each level.

    tools/identical.py OLD_LLRM_C NEW_LLRM_C [--levels=-O1,-O2,-Os,-Omax] [--programs-only]

The corpus is this tool's own: the vsgcc programs (programs.py, wrapped as vsgcc compiles them) and, from QCPORT and QCPORT_INC, QCport's
modules. It fails rather than skips: a corpus smaller than the known one (66 programs, 65 modules) is an error, and so is QCPORT unset
without `--programs-only` (an earlier identity check borrowed compile-cost.py's list, which drops QCport when the variables are unset,
and said 'identical' of 66 files instead of 131). NEW runs with the LLRM_CHECK_* modes of the caches it keeps on, which assert that what
was kept is what working it out again gives. Prints how many objects it compared. Exit 1 on a difference or a failed compile.
"""
from __future__ import annotations

import argparse
import os
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
VSGCC = next((ROOT / "crates/target").glob("*/vsgcc"))
sys.path.insert(0, str(VSGCC))
import programs  # noqa: E402
import wrap  # noqa: E402

KNOWN_PROGRAMS = 66
KNOWN_QCPORT = 65
MODULES = ("host", "render", "model", "game", "sound", "ui")
CHECKS = ("CALLEES", "SIBLINGS", "POSTINGS", "HOLDERS", "JUMPS", "FLOATSKIP", "REPLAY", "PRESERVED", "UNREPORTED", "SPLIT", "LIVE", "BYTES", "GROUPS")


def corpus(work: Path, qcport: Path | None, headers: Path | None, programs_only: bool = False) -> dict[str, tuple[Path, list[str]]]:
    """Name -> (source, flags): every program, and QCport's modules; an error where either is fewer than it is known to be."""
    found: dict[str, tuple[Path, list[str]]] = {}
    for name, source in programs.sources().items():
        wrapped = work / f"{name}.c"
        wrapped.write_text(wrap.wrapped(name, source.read_text()))
        found[name] = (wrapped, programs.LLRM_FLAGS)
    if len(found) < KNOWN_PROGRAMS:
        sys.exit(f"identical: {len(found)} programs, {KNOWN_PROGRAMS} are known")
    if programs_only:
        return found
    if qcport is None or headers is None:
        sys.exit("identical: QCPORT and QCPORT_INC are not set; set them, or ask for --programs-only")
    include = [flag for d in (*MODULES, "qgl") for flag in ("-I", str(qcport / d))] + ["-I", str(headers)]
    modules = {f"qcport/{source.stem}": (source, include) for d in MODULES for source in sorted((qcport / d).glob("*.c"))}
    if len(modules) < KNOWN_QCPORT:
        sys.exit(f"identical: {len(modules)} QCport modules under {qcport}, {KNOWN_QCPORT} are known")
    return found | modules


def same(job: tuple[str, str, Path, list[str]], old: str, new: str, work: Path) -> tuple[str, str, str | None]:
    name, level, source, flags = job
    outs = []
    for tag, compiler in (("old", old), ("new", new)):
        out = work / f"{name.replace('/', '_')}{level}.{tag}.obj"
        env = {**os.environ, **{f"LLRM_CHECK_{one}": "1" for one in CHECKS}} if tag == "new" else {k: v for k, v in os.environ.items() if not k.startswith("LLRM_CHECK_")}
        done = subprocess.run([compiler, level, *flags, str(source), "-o", str(out)], capture_output=True, text=True, env=env)
        if done.returncode:
            return name, level, f"{tag} failed: {done.stderr[-200:]}"
        outs.append(out.read_bytes())
    return name, level, None if outs[0] == outs[1] else "different"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("old")
    parser.add_argument("new")
    parser.add_argument("--levels", default="-O1,-O2,-Os,-Omax", help="comma-separated, e.g. -O2,-Os")
    parser.add_argument("--programs-only", action="store_true")
    parser.add_argument("--jobs", type=int, default=int(os.environ.get("JOBS", "6")))
    args = parser.parse_args()
    qcport, headers = os.environ.get("QCPORT"), os.environ.get("QCPORT_INC")
    with tempfile.TemporaryDirectory(dir=os.environ.get("CARGO_TARGET_DIR")) as tmp:
        work = Path(tmp)
        files = corpus(work, Path(qcport).expanduser() if qcport else None, Path(headers).expanduser() if headers else None, args.programs_only)
        levels = args.levels.split(",")
        jobs = [(name, level, source, flags) for name, (source, flags) in files.items() for level in levels]
        with ThreadPoolExecutor(args.jobs) as pool:
            results = list(pool.map(lambda job: same(job, args.old, args.new, work), jobs))
    bad = [r for r in results if r[2]]
    print(f"{len(files)} files x {len(levels)} levels = {len(results)} objects compared, {len(bad)} differ or failed")
    for name, level, why in bad[:20]:
        print(f"  {name} {level}: {why}")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
