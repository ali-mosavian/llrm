#!/usr/bin/env python3
"""QCport's C modules under every checking oracle, then built and run against the all-Borland build.

    QCPORT=... QCPORT_INC=... QCPORT_BORLAND=... tools/oracle_qcport.py [--bin DIR] [--jobs N] [--levels=-O2,-Os]

The oracle sweep (tools/oracle_sweep.py) compiles BASIC and Nib programs; the C corpus it lacked is where an unsound memory
summary survived to main and was found only by running QCport's `sc`. Here each module of QCport's six source directories is
compiled at each level with every `LLRM_CHECK_*` oracle on (a panic, or an exit with no message, is a finding, as there); the -O2
objects, built with the oracles on, are linked and run by tools/qcport-run-cc.py against the Borland build, which must draw the same
frames, polygons and picture. Premise: the run drew (its summary line names frames and polygons, both above zero); a run that
printed nothing, or drew nothing, fails the tier whatever else it says. Exit 1 on a finding or a difference, 2 on a failed premise.
"""

from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import llrmbin  # noqa: E402
import oracle_sweep  # noqa: E402

MODULES = ("host", "render", "model", "game", "sound", "ui")
SUMMARY = re.compile(r"(\d+) modules at \S+ draw what the Borland build draws: (\d+) frames, (\d+) polys")


def flags(qcport: Path, include: Path) -> list[str]:
    return [f for part in (*MODULES, "qgl") for f in ("-I", str(qcport / part))] + ["-I", str(include)]


def oracle_env(base: dict[str, str] | None = None) -> dict[str, str]:
    """`base` with every oracle on and the reach counters (`LLRM_DEBUG=checks`) added to what the caller had."""
    env = dict(os.environ if base is None else base)
    env.update({f"LLRM_CHECK_{name}": "1" for name in oracle_sweep.CHECKS})
    env["LLRM_DEBUG"] = ",".join(filter(None, [env.get("LLRM_DEBUG"), "checks"]))
    return env


def compile_module(compiler: Path, source: Path, level: str, extra: list[str], out: Path) -> tuple[str | None, dict[str, int]]:
    """(the finding or None, the oracles reached) of one module at `level`; the object is `out`."""
    done = subprocess.run([str(compiler), level, *extra, str(source), "-o", str(out)], capture_output=True, text=True, env=oracle_env())
    reached = {name: int(sites) for name, sites in oracle_sweep.REACHED.findall(done.stderr)}
    return oracle_sweep.classify(done.returncode, oracle_sweep.REACHED.sub("", done.stderr)), reached


def drew(output: str) -> tuple[int, int] | None:
    """(frames, polys) of qcport-run-cc's summary line; None where it printed none, or the run drew nothing."""
    found = SUMMARY.search(output)
    return (int(found[2]), int(found[3])) if found and int(found[2]) and int(found[3]) else None


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--bin", type=Path, default=None, help="the llrm-c directory (default: LLRM_BIN, else the target dir's release)")
    parser.add_argument("--jobs", type=int, default=int(os.environ.get("JOBS", "8")))
    parser.add_argument("--levels", default="-O2,-Os")
    args = parser.parse_args()
    try:
        qcport, include = (Path(os.environ[name]).expanduser() for name in ("QCPORT", "QCPORT_INC"))
        os.environ["QCPORT_BORLAND"]
    except KeyError as missing:
        sys.exit(f"{missing.args[0]} is not set; see the top of {Path(__file__).name}")
    compiler = (args.bin or llrmbin.bin_dir()) / "llrm-c"
    extra = flags(qcport, include)
    sources = sorted(source for part in MODULES for source in (qcport / part).glob("*.c"))
    work = Path(tempfile.mkdtemp(prefix="oracle-qcport-", dir=os.environ.get("TMPDIR")))
    levels = args.levels.split(",")
    jobs = [(source, level) for level in levels for source in sources]
    objects = work / "objects"
    objects.mkdir()

    def one(job):
        source, level = job
        directory = objects if level == "-O2" else work / level
        directory.mkdir(exist_ok=True)
        return job, *compile_module(compiler, source, level, extra, directory / f"{source.stem}.obj")

    with ThreadPoolExecutor(args.jobs) as pool:
        results = list(pool.map(one, jobs))
    findings: dict[str, set[str]] = {}
    for (source, level), why, _ in results:
        if why:
            findings.setdefault(why, set()).add(f"{source.name} {level}")
    silent = oracle_sweep.unreached([(job, why, reached) for job, why, reached in results], [])
    print(f"{len(results)} compiles / {len(findings)} finding kinds")
    for why, where in sorted(findings.items(), key=lambda item: -len(item[1])):
        print(f"{len(where):4}  {why}\n      e.g. {sorted(where)[:3]}")
    if silent:
        print("never reached: " + ", ".join(silent) + " (no call site of the oracle was asked while it was on)")
        return 2
    if findings:
        return 1
    if "-O2" not in levels:
        print("the run needs -O2 objects: nothing to run")
        return 2
    run = subprocess.run([sys.executable, str(HERE / "qcport-run-cc.py")], capture_output=True, text=True, env=oracle_env({**os.environ, "QCPORT_OBJECTS": str(objects)}))
    print(run.stdout.strip())
    if run.returncode and not drew(run.stdout):
        print(run.stderr.strip()[-600:])
    if "a run drew nothing" in run.stdout:
        return 2
    if run.returncode:
        return 1
    if drew(run.stdout) is None:
        print("premise failed: the run drew nothing (no BENCH.TXT frames and polys)")
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
