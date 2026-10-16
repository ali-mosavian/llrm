#!/usr/bin/env python3
"""Whether two llrm-c builds write the same bytes: every program, every tests/run/c program and every QCport module, at each level.

    tools/identical.py OLD_LLRM_C NEW_LLRM_C [--levels=-O1,-O2,-Os,-Omax] [--programs-only] [--old-head REV] [--new-head REV]

A binary older than its head's commit is a stale build (a build that failed leaves the last one): it aborts, unless --stale-ok.

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
import re
import sys
import tempfile
import time
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
KNOWN_RUN = 97
MODULES = ("host", "render", "model", "game", "sound", "ui")
# Oracles the corpus run does not set (each needs inputs the corpus lacks): the list only shrinks. Every other LLRM_CHECK_ oracle in the
# source is set by the run, and a new one needs no edit here: CHECKS was a list each PR appended to, and every PR that added an oracle
# conflicted with the others on it.
NOT_RUN = frozenset("CACHES CALLBACKS CLASSES CLOBBERED CLOBBERS COALESCE COLORS COUNTED CYCLES DATAFLOW EFFECT FACTS FOLDS FRAME INITIALIZED INTERVALS MODULES NEEDS OCCURRENCES POINTVALUES RANGES REQUIRED SCOPES SHAPE SIMULATION STALE THRASH UNCHANGED".split())


def checks(root: Path = ROOT) -> tuple[str, ...]:
    """The oracles the run sets: every quoted "LLRM_CHECK_X" in the crates' source (env_set, var_os) but those in NOT_RUN."""
    found = {m for path in (root / "crates").rglob("*.rs") for m in re.findall(r'"LLRM_CHECK_([A-Z_0-9]+)"', path.read_text())}
    return tuple(sorted(found - NOT_RUN))


CHECKS = checks()


def run_programs(root: Path = ROOT) -> dict[str, tuple[Path, list[str]]]:
    """tests/run/c's programs, one entry per `// flags:` variant (each names its own level): the gate's run tests were the programs whose
    miscompile an identity check on the other two lists could not see."""
    found: dict[str, tuple[Path, list[str]]] = {}
    for source in sorted((root / "tests/run/c").glob("*.c")):
        header = next((line for line in source.read_text().splitlines()[:3] if line.startswith("// flags:")), None)
        variants = [v.split() for v in header.removeprefix("// flags:").split("|")] if header else [["-O2"]]
        for n, flags in enumerate(variants):
            found[f"run/{source.stem}#{n}"] = (source, flags)
    return found


def corpus(work: Path, qcport: Path | None, headers: Path | None, programs_only: bool = False) -> dict[str, tuple[Path, list[str]]]:
    """Name -> (source, flags): every program, and QCport's modules; an error where either is fewer than it is known to be."""
    found: dict[str, tuple[Path, list[str]]] = {}
    for name, source in programs.sources().items():
        wrapped = work / f"{name}.c"
        wrapped.write_text(wrap.wrapped(name, source.read_text()))
        found[name] = (wrapped, programs.LLRM_FLAGS)
    if len(found) < KNOWN_PROGRAMS:
        sys.exit(f"identical: {len(found)} programs, {KNOWN_PROGRAMS} are known")
    run = run_programs()
    if len({source for source, _ in run.values()}) < KNOWN_RUN:
        sys.exit(f"identical: {len(run)} tests/run programs, {KNOWN_RUN} are known")
    found |= run
    if programs_only:
        return found
    if qcport is None or headers is None:
        sys.exit("identical: QCPORT and QCPORT_INC are not set; set them, or ask for --programs-only")
    include = [flag for d in (*MODULES, "qgl") for flag in ("-I", str(qcport / d))] + ["-I", str(headers)]
    modules = {f"qcport/{source.stem}": (source, include) for d in MODULES for source in sorted((qcport / d).glob("*.c"))}
    if len(modules) < KNOWN_QCPORT:
        sys.exit(f"identical: {len(modules)} QCport modules under {qcport}, {KNOWN_QCPORT} are known")
    return found | modules


def stale(binary: str, rev: str, what: str, root: Path = ROOT) -> str | None:
    """Why `binary` cannot be what `rev` builds: missing, or older than the commit. A build that failed leaves the last one in place."""
    path = Path(binary)
    if not path.is_file():
        return f"{what} {binary} does not exist"
    done = subprocess.run(["git", "-C", str(root), "log", "-1", "--format=%ct", rev], capture_output=True, text=True)
    if done.returncode or not done.stdout.strip():
        return f"{what}: no such revision {rev}"
    if path.stat().st_mtime < int(done.stdout):
        age = time.strftime("%F %T", time.localtime(path.stat().st_mtime))
        return f"{what} {binary} was built {age}, before {rev} was committed: it is a stale build, not {rev}"
    return None


def same(job: tuple[str, str, Path, list[str]], old: str, new: str, work: Path) -> tuple[str, str, str | None]:
    name, level, source, flags = job
    outs = []
    for tag, compiler in (("old", old), ("new", new)):
        out = work / f"{name.replace('/', '_')}{level}.{tag}.obj"
        env = {**os.environ, **{f"LLRM_CHECK_{one}": "1" for one in CHECKS}} if tag == "new" else {k: v for k, v in os.environ.items() if not k.startswith("LLRM_CHECK_")}
        done = subprocess.run([compiler, *([level] if level else []), *flags, str(source), "-o", str(out)], capture_output=True, text=True, env=env)
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
    parser.add_argument("--old-head", default="origin/main", help="the revision OLD was built from")
    parser.add_argument("--new-head", default="HEAD", help="the revision NEW was built from")
    parser.add_argument("--stale-ok", action="store_true", help="do not check the binaries' ages against the heads")
    parser.add_argument("--jobs", type=int, default=int(os.environ.get("JOBS", "6")))
    args = parser.parse_args()
    if not args.stale_ok:
        why = stale(args.old, args.old_head, "OLD") or stale(args.new, args.new_head, "NEW")
        if why:
            sys.exit(f"identical: {why}; rebuild it, or say --old-head/--new-head, or --stale-ok")
    qcport, headers = os.environ.get("QCPORT"), os.environ.get("QCPORT_INC")
    with tempfile.TemporaryDirectory(dir=os.environ.get("CARGO_TARGET_DIR")) as tmp:
        work = Path(tmp)
        files = corpus(work, Path(qcport).expanduser() if qcport else None, Path(headers).expanduser() if headers else None, args.programs_only)
        levels = args.levels.split(",")
        jobs = [(name, level, source, flags) for name, (source, flags) in files.items() for level in (levels if not name.startswith("run/") else [""])]
        with ThreadPoolExecutor(args.jobs) as pool:
            results = list(pool.map(lambda job: same(job, args.old, args.new, work), jobs))
    bad = [r for r in results if r[2]]
    print(f"{len(files)} files, {len(results)} objects compared, {len(bad)} differ or failed")
    for name, level, why in bad[:20]:
        print(f"  {name} {level}: {why}")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
