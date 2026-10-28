#!/usr/bin/env python3
"""Frame stores that reach past the frame's saved BP, in the asm of every m16 program and of the QB runtime.

    slotscan.py corpus BIN_DIR ISEL [ROOT]    every .c (llrm-c -O2 and -Os) and .bas (llrm-qb -O2) under ROOT (the repo)
    slotscan.py runtime BIN_DIR ISEL          the QB runtime's C files (llrm-c -O2 and -Os, the include set of tools/dosbatch/qbruntime.py)

BIN_DIR holds llrm-c and llrm-qb, ISEL is old, dag or shadow. A hit is a store `op SIZE ptr [bp-N]` with SIZE > N: it writes the saved BP or the return
address (grep.c's dword zero, B$FLOF and tone.c's dword spill into a word slot). Prints `jobs N programs-with-hits M` and one `HIT` line per function;
a file that does not compile is a `NOCOMPILE` line, and counts as a red. Exit status is 1 on any hit or NOCOMPILE.

Blind spot: a wide store into a slot that is not next to BP overwrites a neighbour and is not seen (the old `dword_zero_into_word_slot` heuristic found
those and had false positives on QB descriptor initialisers).
"""
from __future__ import annotations

import os
import re
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WIDTH = {"byte": 1, "word": 2, "dword": 4, "qword": 8, "tbyte": 10}
# These only read the cell, or store from the x87 stack as one unit with its own width.
READERS = {"cmp", "test", "push", "fld", "fild", "fadd", "fmul", "fsub", "fsubr", "fdiv", "fdivr", "fcom", "fcomp", "fiadd", "fimul", "fisub", "fidiv",
           "fldcw", "call", "jmp", "mul", "imul", "div", "idiv", "fxch", "lea", "pushd", "pushw"}
STORE = re.compile(r"\s*(\w+)\s+(byte|word|dword|qword|tbyte) ptr \[bp-(\d+)\]\s*(?:,|$)")
RUNTIME_DIRS = ["qb", "qb/dos/m16", "qb/dev", "qb/dev/dos/m16", "qb/dev/dos", "shared/dos/m16", "shared"]


def hits(asm: str) -> list[str]:
    """`FUNCTION  the first offending line` for each procedure of `asm` that stores past BP."""
    found = []
    parts = re.split(r"(?m)^(\S.*\bproc\b.*)$", asm)
    for head, body in zip(parts[1::2], parts[2::2]):
        for line in body.split("endp")[0].splitlines():
            store = STORE.match(line)
            if store and store[1] not in READERS and WIDTH[store[2]] > int(store[3]):
                found.append(f"{head.split()[0]}  {line.strip()}")
                break
    return found


def compiled(cmd: list[str]) -> tuple[str | None, str]:
    with tempfile.TemporaryDirectory() as work:
        done = subprocess.run([*cmd, "-S", "-o", f"{work}/x.s"], capture_output=True, text=True, timeout=600)
        try:
            return Path(f"{work}/x.s").read_text(), done.stderr
        except OSError:
            return None, done.stderr


def corpus_jobs(bins: Path, isel: str, root: Path) -> list[tuple[str, str, list[str]]]:
    jobs = []
    for source in sorted(root.rglob("*.c")):
        if "target" in source.parts or ".git" in source.parts or ".venv" in source.parts:
            continue
        for level in ("-O2", "-Os"):
            cmd = [str(bins / "llrm-c"), str(source), "-I", str(root / "bench"), "-I", str(source.parent), "-m16", "-march=i386", level, f"--isel={isel}"]
            jobs.append((f"c-m16{level}", str(source), cmd))
    for source in sorted(root.rglob("*.bas")):
        if "target" in source.parts or ".git" in source.parts or ".venv" in source.parts:
            continue
        jobs.append(("qb-O2", str(source), [str(bins / "llrm-qb"), str(source), "-O2", f"--isel={isel}"]))
    return jobs


def runtime_jobs(bins: Path, isel: str) -> list[tuple[str, str, list[str]]]:
    runtime = ROOT / "runtime"
    includes = [flag for one in RUNTIME_DIRS for flag in ("-I", str(runtime / one))]
    sources: set[Path] = set()
    for one in ("qb", "qb/dos/m16", "qb/dev/dos/m16", "qb/dev/dos", "shared/dos/m16"):
        sources.update((runtime / one).glob("*.c"))
    return [
        (f"rt{level}", str(source), [str(bins / "llrm-c"), str(source), *includes, "-m16", "-march=i386", level, "-mbss-begin", f"--isel={isel}"])
        for source in sorted(sources)
        for level in ("-O2", "-Os")
    ]


def main(argv: list[str]) -> int:
    if len(argv) < 4 or argv[1] not in ("corpus", "runtime"):
        print(__doc__)
        return 2
    mode, bins, isel = argv[1], Path(argv[2]), argv[3]
    env = {"LLRM_OW": os.path.expanduser("~/scratch/ow-src"), "LLRM_PIN_CLANG": os.path.expanduser("~/scratch/llvm-build/bin/clang")}
    os.environ.update({key: value for key, value in env.items() if key not in os.environ})
    jobs = corpus_jobs(bins, isel, Path(argv[4]) if len(argv) > 4 else ROOT) if mode == "corpus" else runtime_jobs(bins, isel)

    def one(job: tuple[str, str, list[str]]) -> tuple[str, str, list[str] | None, str]:
        tag, source, cmd = job
        asm, error = compiled(cmd)
        return tag, source, None if asm is None else hits(asm), error

    with ThreadPoolExecutor(6) as pool:
        results = list(pool.map(one, jobs))
    bad = [one for one in results if one[2]]
    print(f"# slotscan {mode} isel={isel} llrm-c {bins / 'llrm-c'}")
    print(f"jobs {len(results)} programs-with-hits {len(bad)}")
    for tag, source, found, _ in bad:
        print(f"HIT {tag} {source}: {'; '.join(found)}")
    for tag, source, found, error in results:
        if found is None and mode == "runtime":
            print(f"NOCOMPILE {tag} {source}: {(error.strip().splitlines() or [''])[0][:160]}")
    refused = [one for one in results if one[2] is None and mode == "runtime"]
    return int(bool(bad or refused))


if __name__ == "__main__":
    sys.exit(main(sys.argv))
