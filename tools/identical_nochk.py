#!/usr/bin/env python3
"""Whether two llrm-cc builds write the same bytes: every program, every tests/run/c program and every QCport module, at each level.

    tools/identical.py OLD_COMPILER NEW_COMPILER [--levels=-O1,-O2,-Os,-Omax] [--programs-only] [--old-head REV] [--new-head REV]

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
sys.path.insert(0, str(Path(__file__).resolve().parent))
import corpora  # noqa: E402
import programs  # noqa: E402
import wrap  # noqa: E402

KNOWN_PROGRAMS = 66
KNOWN_QCPORT = 65
KNOWN_RUN = 97
KNOWN_BENCH = 120
BENCH_LEVELS = ("O2", "Os", "O3")
MODULES = ("host", "render", "model", "game", "sound", "ui")
# Oracles the corpus run does not set: the list only shrinks. Every other LLRM_CHECK_ oracle in the source is set by the run, and a new one
# needs no edit here: CHECKS was a list each PR appended to, and every PR that added an oracle conflicted with the others on it. An oracle
# here is a gap the next dependency bug falls through (qcport/triggers -O1 sat behind one of the others for weeks): each of these fails on
# the corpus on main today, and leaves with the fix named.
#   COLORS and FRAME: fail on the corpus (634 and 21 of 1,323 programs and modules; 0 with the oracle off).
#   STALE: a pass that says it adds no memory operation outlives a frozen globals-aa (#2094).
NOT_RUN = frozenset("COLORS FRAME STALE CAPTURES_CALLS CAPTURES_BLIND CAPTURES_COUNT CAPTURES_MODULE".split())


def checks(root: Path = ROOT) -> tuple[str, ...]:
    """The oracles the run sets: every quoted "LLRM_CHECK_X" in the crates' source (env_set, var_os) but those in NOT_RUN."""
    found = {m for path in (root / "crates").rglob("*.rs") for m in re.findall(r'"LLRM_CHECK_([A-Z_0-9]+)"', path.read_text())}
    return tuple(sorted(found - NOT_RUN))


CHECKS = ()


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


def bench_compiles(work: Path, levels: tuple[str, ...] = BENCH_LEVELS) -> dict[str, tuple[str, list[str]]]:
    """Name -> (tool, arguments): every compile of llrm's own the bench harness makes of each program in each language, with the
    harness's flags (`bench.compiles`, the one place they are spelled), at each of `levels`. The programs of vsgcc and tests/run are
    compiled with other flags than the bench's (-fno-inline-functions, a volatile pointer to the kernel), so a change to what the bench
    measures was 'identical' on a corpus that never compiled it so."""
    sys.path.insert(0, str(HERE / "bench"))
    import bench  # imported here: it needs the emulators and the built binaries

    found: dict[str, tuple[str, list[str]]] = {}
    for directory in bench.benchmarks([]):
        for variant in bench.variants(directory):
            for opt in levels:
                stem = f"{variant.name.replace('/', '_')}.{variant.language}.{opt}"
                for n, (tool, arguments, _) in enumerate(bench.compiles(variant, opt, work, stem)):
                    found[f"bench/{variant.name}/{variant.language}/{opt}#{n}"] = (tool, arguments)
    if len({name.split("#")[0] for name in found}) < KNOWN_BENCH:
        sys.exit(f"identical: {len(found)} bench compiles, {KNOWN_BENCH} programs are known")
    return found


def same(job: tuple[str, str, str, list[str]], old: str, new: str, work: Path) -> tuple[str, str, str | None]:
    """One compile by `old` and by `new` (the tool beside each compiler): the objects are the same bytes. `arguments` end in `-o OBJ`."""
    name, level, tool, arguments = job
    outs = []
    for tag, compiler in (("old", old), ("new", new)):
        out = work / f"{name.replace('/', '_')}{level}.{tag}.obj"
        # A value already set (LLRM_CHECK_ORDER=SEED) stays, so a run can be repeated under other seeds.
        env = (
            {**os.environ, **{f"LLRM_CHECK_{one}": "1" for one in CHECKS if f"LLRM_CHECK_{one}" not in os.environ}}
            if tag == "new"
            else {k: v for k, v in os.environ.items() if not k.startswith("LLRM_CHECK_")}
        )
        at = len(arguments) - 1 - arguments[::-1].index("-o")
        command = [str(Path(compiler).with_name(tool)), *arguments[:at], "-o", str(out), *arguments[at + 2 :]]
        done = subprocess.run(command, capture_output=True, text=True, env=env)
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
    parser.add_argument("--no-bench", action="store_true", help="leave out the bench harness's own compiles")
    parser.add_argument("--show", type=int, default=20, help="how many differing objects to name")
    parser.add_argument("--jobs", type=int, default=int(os.environ.get("JOBS", "6")))
    parser.add_argument("--allow-other-corpus", action="store_true", help="read a QCport tree whose hash is not the registered one (tools/corpora.py)")
    args = parser.parse_args()
    if not args.stale_ok:
        why = stale(args.old, args.old_head, "OLD") or stale(args.new, args.new_head, "NEW")
        if why:
            sys.exit(f"identical: {why}; rebuild it, or say --old-head/--new-head, or --stale-ok")
    qcport, headers = os.environ.get("QCPORT"), os.environ.get("QCPORT_INC")
    if qcport and not args.programs_only:
        qcport = str(corpora.resolve("qcport", qcport, args.allow_other_corpus))
        print(corpora.header("qcport", qcport))
    with tempfile.TemporaryDirectory(dir=os.environ.get("CARGO_TARGET_DIR")) as tmp:
        work = Path(tmp)
        files = corpus(work, Path(qcport).expanduser() if qcport else None, Path(headers).expanduser() if headers else None, args.programs_only)
        levels = args.levels.split(",")
        jobs = [
            (name, level, "llrm-cc", [*([level] if level else []), *flags, str(source), "-o", "OBJ"])
            for name, (source, flags) in files.items()
            for level in (levels if not name.startswith("run/") else [""])
        ]
        if not args.no_bench:
            benches = bench_compiles(work)
            jobs += [(name, "", tool, arguments) for name, (tool, arguments) in benches.items()]
            files = files | {name: None for name in benches}
        with ThreadPoolExecutor(args.jobs) as pool:
            results = list(pool.map(lambda job: same(job, args.old, args.new, work), jobs))
    bad = [r for r in results if r[2]]
    print(f"{len(files)} files, {len(results)} objects compared, {len(bad)} differ or failed")
    for name, level, why in bad[: args.show]:
        print(f"  {name} {level}: {why}")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
