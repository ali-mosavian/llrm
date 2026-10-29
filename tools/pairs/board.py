#!/usr/bin/env python3
"""The compile board: d_faces compile cost, one flag set, perf instructions:u, minimum of 3, for several llrm-cc builds and the pinned clang.

    QCPORT=<qb-quake src> tools/pairs/board.py --clang CLANG NAME=LLRM_CC... [--levels O1,O2,O3,Os]

Every llrm build is measured with and without -fno-inline-functions, and so is clang, at each level; -m32 -march=i486, an object written to a file, the Borland
keywords defined away. A compile that fails stops the run (the count of a failed compile is a few million, not a measurement). For each llrm build the
phases of `LLRM_DEBUG=time` (own Minstr, a step's analyses under their own names) follow, when the build prints them.
"""
from __future__ import annotations

import collections
import hashlib
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

KEYWORDS = [f"-D{k}=" for k in "far huge near pascal cdecl __far _far __near _near".split()]
REGALLOC = {"regalloc", "split", "facts", "spill", "classes", "after", "recolor"}


def run(cmd: list[str], env: dict | None = None) -> subprocess.CompletedProcess:
    done = subprocess.run(cmd, capture_output=True, text=True, env={**os.environ, **(env or {})})
    if done.returncode:
        sys.exit(f"failed: {' '.join(cmd[:4])}\n{done.stderr[-400:]}")
    return done


def md5(path) -> str:
    return hashlib.md5(Path(path).resolve().read_bytes()).hexdigest()


def header(source, includes: list[str], clang: str, builds: list, up: str) -> list[str]:
    """Every input of the numbers, one `field: value` line each: the file measured and what is in it, the pinned clang (path as given, the
    binary it names, its md5 and `--version`), the include list, each llrm build's path and md5, and the host's `uptime`. Two boards are
    comparable only when these lines are."""
    version = run([clang, "--version"]).stdout.splitlines()[0]
    lines = [
        f"input: {source} md5 {md5(source)}",
        f"clang: {clang} -> {Path(clang).resolve()}",
        f"clang md5: {md5(clang)}",
        f"clang version: {version}",
        f"includes: {' '.join(includes)}",
    ]
    lines += [f"llrm {name}: {llrm} md5 {md5(llrm)}" for name, llrm in builds]
    lines.append(f"uptime: {up}")
    return lines


LLRM_CC_ENV = {"LLRM_CC_TARGET": "x86_32-wat"}  # llrm-cc takes its target row from the environment


def instructions(cmd: list[str], env: dict | None = None) -> float:
    """Minimum of three runs, in M."""
    best = None
    for _ in range(3):
        err = run(["perf", "stat", "-x,", "-e", "instructions:u", *cmd], env).stderr
        found = re.search(r"^(\d+),,instructions", err, re.M)
        assert found, err[-200:]
        best = int(found[1]) if best is None else min(best, int(found[1]))
    return best / 1e6


def phases(llrm: str, level: str, args: list[str]) -> dict[str, float]:
    err = run([llrm, "-c", "-m32", "-march=i486", level, *args], {**LLRM_CC_ENV, "LLRM_DEBUG": "time", "LLRM_TIME_TOP": "700"}).stderr
    if "[instr] by own work" not in err:
        return {}
    own: dict[str, float] = collections.Counter()
    for line in err.split("[instr] by own work")[1].split("nesting")[0].splitlines():
        found = re.search(r"([\d.]+) Minstr own\s+[\d.]+ Minstr total\s+\d+x (\S+)", line)
        if found:
            own["regalloc" if found[2] in REGALLOC else found[2]] += float(found[1])
    return dict(own)


def main() -> int:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
    import stamp

    argv, named, allow = stamp.options(sys.argv[1:])
    if "--clang" not in argv or "QCPORT" not in os.environ:
        print(__doc__)
        return 2
    clang = argv[argv.index("--clang") + 1]
    levels = argv[argv.index("--levels") + 1].split(",") if "--levels" in argv else ["O1", "O2", "O3", "Os"]
    builds = [a.split("=", 1) for a in argv if "=" in a and not a.startswith("-")]
    for name, llrm in builds:
        # NAME=PATH@SHA names the commit that build must come from; --expect-sha names it for a lone build.
        path, _, sha = llrm.partition("@")
        stamp.check(path, sha or (named["sha"] if len(builds) == 1 else None), allow, name)
    builds = [(name, llrm.partition("@")[0]) for name, llrm in builds]
    qc = Path(os.environ["QCPORT"])
    includes = [f for d in ("", "render", "game", "host", "qgl") for f in ("-I", str(qc / d))]
    source = str(qc / "render/d_faces.c")
    sink = os.path.join(tempfile.mkdtemp(prefix="board-"), "out.o")
    print("d_faces compile, M instructions:u (min of 3), -m32 -march=i486")
    print("\n".join(header(source, includes, clang, builds, run(["uptime"]).stdout.strip())))
    columns = ["level", *(f"{n} {v}" for n, _ in builds for v in ("inline", "noinl")), "clang inline", "clang noinl"]
    print(" | ".join(columns))
    table = {}
    for level in levels:
        row = {}
        for name, llrm in builds:
            for tag, extra in (("inline", []), ("noinl", ["-fno-inline-functions"])):
                row[f"{name} {tag}"] = instructions([llrm, "-c", "-m32", "-mabi=sysv", "-march=i486", f"-{level}", *extra, *KEYWORDS, *includes, "-o", sink, source], LLRM_CC_ENV)
        for tag, extra in (("inline", []), ("noinl", ["-fno-inline-functions"])):
            row[f"clang {tag}"] = instructions([clang, "-w", "-m32", "-march=i486", f"-{level}", *extra, *KEYWORDS, *includes, "-c", "-o", sink, source])
        table[level] = row
        print(f"-{level} | " + " | ".join(f"{row[h]:.0f}" for h in columns[1:]), flush=True)
    for level in levels:
        for name, llrm in builds:
            own = phases(llrm, f"-{level}", [*KEYWORDS, *includes, "-o", sink, source])
            top = sorted(own.items(), key=lambda kv: -kv[1])[:8]
            print(f"-{level} {name} phases (own M): " + (", ".join(f"{k} {v:.0f}" for k, v in top) or "not printed by this build"))
    return 0


if __name__ == "__main__":
    sys.exit(main())
