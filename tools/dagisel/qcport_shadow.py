#!/usr/bin/env python3
"""The DAG path over a corpus's modules (QCport's by default): functions selected, functions refused by kind, modules that do not compile.

    tools/dagisel/qcport_shadow.py LLRM_C [--mode m32|m16] [--level Os] [--jobs N]

Each module is compiled with `--isel=shadow` and `LLRM_DEBUG=isel`; every function writes one `[isel]` row, and one
`refused:` line when the DAG path could not select it. The denominator is every function of every module that compiled;
a module that did not compile is counted apart (and its functions are not in the denominator). QCport is read from $QCPORT
(default ~/scratch/qcport/src) and its Borland headers from $QCPORT_INC.
"""
from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
import tempfile
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

QCPORT = Path(os.environ.get("QCPORT", Path.home() / "scratch/qcport/src"))
QCINC = Path(os.environ.get("QCPORT_INC", Path.home() / "scratch/qctc/inc"))
FLAGS = {"m32": ["-m32", "-mabi=sysv", "-march=i486"], "m16": ["-m16", "-march=i386"]}
ROW = re.compile(r"^\[isel\] (\S+) selector=")
REFUSED = re.compile(r"^\[isel\] (\S+) refused: (.*)$")


def kind(why: str) -> str:
    """The refusal without the names that vary: `--isel=dag: Emit(Unmapped("LEA32r"))` -> `Emit(Unmapped(LEA32r))`; builder reasons stay."""
    why = why.removeprefix("--isel=dag: ")
    return re.sub(r'"', "", why)[:100]


def parse(stderr: str) -> tuple[set[str], dict[str, str]]:
    """(functions, {function: why}) from the isel channel."""
    functions, refused = set(), {}
    for line in stderr.splitlines():
        if found := REFUSED.match(line):
            refused[found[1]] = kind(found[2])
        elif found := ROW.match(line):
            functions.add(found[1])
    return functions, refused


def compile_module(binary: str, source: Path, mode: str, level: str, work: Path):
    if source.is_relative_to(QCPORT):
        includes = [one for d in ("host", "render", "model", "game", "sound", "ui", "qgl") for one in ("-I", str(QCPORT / d))] + ["-I", str(QCINC)]
    else:
        includes = ["-I", str(source.parent)]
    env = {**os.environ, "LLRM_DEBUG": "isel"}
    done = subprocess.run([binary, f"-{level}", *FLAGS[mode], "--isel=shadow", *includes, str(source), "-o", str(work / (source.stem + ".o"))], capture_output=True, text=True, env=env, timeout=900)
    if done.returncode != 0:
        return source.stem, None, (done.stderr.strip().splitlines() or [f"exit {done.returncode}"])[-1][:120]
    return source.stem, parse(done.stderr), ""


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("binary")
    parser.add_argument("--mode", default="m32")
    parser.add_argument("--level", default="Os")
    parser.add_argument("--src", action="append", default=[], help="a directory of .c files to use instead of QCport's (repeatable)")
    parser.add_argument("--jobs", type=int, default=int(os.environ.get("JOBS", "6")))
    args = parser.parse_args()
    if args.src:
        sources = sorted(p for d in args.src for p in Path(d).glob("*.c"))
    else:
        sources = sorted(p for d in ("host", "render", "model", "game", "sound", "ui") for p in (QCPORT / d).glob("*.c"))
    if not sources:
        print(f"no QCport sources under {QCPORT}")
        return 2
    work = Path(tempfile.mkdtemp(prefix="qcport-shadow-"))
    total, selected, failed, kinds, per_module = 0, 0, [], Counter(), {}
    with ThreadPoolExecutor(args.jobs) as pool:
        for name, found, why in pool.map(lambda s: compile_module(args.binary, s, args.mode, args.level, work), sources):
            if found is None:
                failed.append((name, why))
                continue
            functions, refused = found
            total += len(functions)
            selected += len(functions) - len(refused)
            kinds.update(refused.values())
            per_module[name] = (len(functions), len(refused))
    print(f"shadow {args.mode} -{args.level}: {len(sources)} modules, {len(sources) - len(failed)} compiled, {len(failed)} did not")
    print(f"functions: {total} in the compiled modules; selected by the DAG path {selected}; refused {total - selected}")
    for why, count in kinds.most_common():
        print(f"  {count:5d}  {why}")
    for name, why in failed:
        print(f"  module {name} did not compile: {why}")
    return 0 if total == selected and not failed else 1


if __name__ == "__main__":
    sys.exit(main())
