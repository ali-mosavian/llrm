#!/usr/bin/env python3
"""llrm-c's compile cost against clang's and gcc's, per level, on d_faces and chosen bench programs.

    QCPORT=<qb-quake src> tools/compile-gap.py [--programs matmul,quicksort] [llrm-c]   (builds the dist llrm-c if none is given)

Prints user-space instructions of `-S` compiles (perf stat; best of 3 by cycles) for the three compilers, then for d_faces
llrm's phases in own instructions (`LLRM_DEBUG=time`'s `[instr]`, summed by phase) beside clang's `-ftime-report`
shares (optimizer, codegen) applied to its instruction count. All three get the bench flags (-fno-inline-functions) and the Borland keywords defined away. A compile that
fails stops the run: a count of a failed compile is a few million, not a measurement.
"""
from __future__ import annotations

import argparse
import collections
import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import pin_clang  # noqa: E402
import corpora  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
LEVELS = ("-O1", "-O2", "-O3", "-Os")
KEYWORDS = [f"-D{k}=" for k in "far huge near pascal cdecl __far _far __near _near".split()]
PHASES = {"regalloc", "split", "facts", "spill", "classes", "after", "recolor"}


def run(cmd: list[str], env: dict[str, str] | None = None) -> subprocess.CompletedProcess:
    done = subprocess.run(cmd, capture_output=True, text=True, env={**os.environ, **(env or {})})
    if done.returncode:
        sys.exit(f"failed: {' '.join(cmd[:4])}\n{done.stderr[-400:]}")
    return done


def instructions(cmd: list[str]) -> int:
    best = None
    for _ in range(3):
        counts = {}
        for line in run(["perf", "stat", "-x,", "-e", "instructions:u,cycles:u", *cmd]).stderr.splitlines():
            fields = line.split(",")
            if len(fields) > 2:
                counts[fields[2].split(":")[0]] = int(fields[0])
        if best is None or counts["cycles"] < best["cycles"]:
            best = counts
    return best["instructions"]


def phases(llrm: str, level: str, args: list[str]) -> dict[str, float]:
    """Own Minstr by phase (`[instr]` of LLRM_DEBUG=time, a step's analyses counted under their own names, so no
    instruction twice). Work done, the same on a loaded host; the ms columns of the same report are not."""
    err = run([llrm, "-m32", "-march=i486", level, *args], {"LLRM_DEBUG": "time", "LLRM_TIME_TOP": "700"}).stderr
    own: dict[str, float] = collections.Counter()
    for line in err.split("[instr] by own work")[1].split("nesting")[0].splitlines():
        found = re.search(r"([\d.]+) Minstr own\s+[\d.]+ Minstr total\s+\d+x (\S+)", line)
        if found:
            own["regalloc" if found[2] in PHASES else found[2]] += float(found[1])
    return own


def clang_phases(clang: str, level: str, args: list[str]) -> list[float]:
    """Clang's optimizer, codegen and whole-run seconds-times in ms (`-ftime-report`, quickest of three): shares of
    its time, to be applied to its instruction count; a timer, read within about 15%."""
    runs = []
    for _ in range(3):
        err = run([clang, "-w", "-m32", "-march=i486", level, *args, "-ftime-report"]).stderr
        runs.append([float(one) * 1000 for one in re.findall(r"Total Execution Time: ([\d.]+) seconds", err)])
    return min(runs, key=lambda one: one[5])


def dist_build() -> str:
    """The tree's llrm-c as shipped (fat LTO, one codegen unit): compile-time claims quote it, as a release build
    reads about 10% to 20% more instructions; gates stay on release."""
    built = subprocess.run(
        ["cargo", "build", "--profile", "dist", "--bin", "llrm-c", "--message-format=json"],
        cwd=ROOT, capture_output=True, text=True,
    )
    if built.returncode:
        sys.exit(f"dist build failed\n{built.stderr[-400:]}")
    for line in reversed(built.stdout.splitlines()):
        if '"executable":"' in line and '"name":"llrm-c"' in line:
            return json.loads(line)["executable"]
    sys.exit("no llrm-c in the build")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("llrm", nargs="?", help="an llrm-c to measure; by default the tree's, built with the dist profile")
    parser.add_argument("--programs", default="matmul,quicksort,nbody,sieve,switch")
    parser.add_argument(
        "--clang",
        default=None,
        help="the clang to compare with (a static build of the pinned tag, docs/architecture/compile-gap-series.md); by default the pin's, via tools/pin_clang.py (LLRM_PIN_CLANG)",
    )
    parser.add_argument(
        "--distro-clang",
        help="a second clang column, what users run (the distro's shared-libLLVM clang: +20% of startup and calls into libLLVM)",
    )
    parser.add_argument("--allow-other-corpus", action="store_true", help="measure a source tree whose hash is not the registered one (tools/corpora.py)")
    args = parser.parse_args()
    args.clang = args.clang or pin_clang.path()
    args.llrm = args.llrm or dist_build()
    qc = corpora.resolve("dfaces", os.environ.get("DFACES_SRC") or os.environ.get("QCPORT"), args.allow_other_corpus)
    print(corpora.header("dfaces", qc))
    includes = [flag for d in ("", "render", "game", "host", "qgl") for flag in ("-I", str(qc / d))]
    inputs = {"d_faces": (str(qc / "render/d_faces.c"), KEYWORDS + includes)}
    for name in filter(None, args.programs.split(",")):
        inputs[name] = (str(ROOT / "bench" / name / f"{name}.c"), KEYWORDS)
    # An object each compiler writes, not assembly text to the null device: llrm printed the text (21 M of d_faces -O2) instead of
    # encoding and writing the object (22 M), and clang and gcc would not have run their assemblers.
    target = os.path.join(tempfile.mkdtemp(prefix="compile-gap-"), "out.o")
    sink = ["-o", target]
    sink_cc = ["-c", *sink]
    flags = ["-m32", "-march=i486", "-fno-inline-functions"]
    for level in LEVELS:
        extra_column = " / distro clang" if args.distro_clang else ""
        print(f"{level}: compile M instructions   llrm / clang / gcc{extra_column}")
        for name, (source, extra) in inputs.items():
            cost = [
                instructions([cc, *flags, *(["-w"] if cc != args.llrm else []), level, *extra, *(sink if cc == args.llrm else sink_cc), source])
                / 1e6
                for cc in (args.llrm, args.clang, "gcc")
            ]
            users = ""
            if args.distro_clang:
                users = f" / {instructions([args.distro_clang, *flags, '-w', level, *extra, *sink_cc, source]) / 1e6:7.0f}"
            print(f"  {name:10} {cost[0]:7.0f} / {cost[1]:7.0f} / {cost[2]:7.0f}{users}   ({cost[0] / min(cost[1:]):.1f}x)")
        source, extra = inputs["d_faces"]
        ours = phases(args.llrm, level, [*extra, *sink, source])
        theirs = clang_phases(args.clang, level, [*extra, *sink_cc, source])
        whole = sum(ours.values())
        print("  d_faces llrm own Minstr: " + ", ".join(f"{k} {v:.0f} ({100 * v / whole:.0f}%)" for k, v in ours.most_common(6)))
        clang = instructions([args.clang, *flags, level, *KEYWORDS, *extra, *sink_cc, source]) / 1e6
        print(
            f"  d_faces clang: optimizer {theirs[0] / theirs[5] * clang:.0f} M ({100 * theirs[0] / theirs[5]:.0f}% of "
            f"its time), codegen {theirs[4] / theirs[5] * clang:.0f} M ({100 * theirs[4] / theirs[5]:.0f}%)"
        )


if __name__ == "__main__":
    main()
