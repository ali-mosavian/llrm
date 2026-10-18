#!/usr/bin/env python3
"""Today's alias answers against LLVM's BasicAA, pair by pair, on dumped pass inputs.

    tools/aliascompare.py [--llrm-mir BIN] FILE.ll...

For each input: `llrm-mir --aliases` (the answer of the points-to facts for every pair of pointer operands of loads, stores and calls) and
`opt -passes=aa-eval -print-all-alias-modref-info` (the pinned LLVM, 24.0.0git d1106deb71cc, via tools/difftest/difftest.py's
normaliser and its LLVM_BIN) on the same text. Only pairs both print are compared. A table of (ours, LLVM's) counts follows, with the
opt version in its header. Ours NoAlias where LLVM says MayAlias is a candidate miscompile of today's code; ours MayAlias where LLVM
says NoAlias is a lost precision.
"""

import argparse
import collections
import importlib.util
import os
import re
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent


def difftest():
    """tools/difftest/difftest.py, or the directory named by DIFFTEST_DIR until it is on main."""
    for base in filter(None, [os.environ.get("DIFFTEST_DIR"), str(HERE / "difftest")]):
        path = Path(base) / "difftest.py"
        if path.is_file():
            spec = importlib.util.spec_from_file_location("difftest", path)
            module = importlib.util.module_from_spec(spec)
            sys.modules["difftest"] = module
            spec.loader.exec_module(module)
            return module
    sys.exit("aliascompare: tools/difftest/difftest.py is not here; set DIFFTEST_DIR to the directory that has it")


PAIR = re.compile(r"^\s+(NoAlias|MayAlias|PartialAlias|MustAlias):\s+(.*)$")
NAME = re.compile(r'[%@](?:"[^"]*"|[\w.$-]+)')
FUNCTION = re.compile(r"^Function: ([^:]+):")


def llvm_answers(dt, text: str) -> dict[tuple[str, frozenset[str]], str]:
    done = subprocess.run(
        [str(dt.LLVM_BIN / "opt"), "-passes=aa-eval", "-print-all-alias-modref-info", "-disable-output", "-"],
        input=dt.normalise(text),
        capture_output=True,
        text=True,
    )
    found: dict[tuple[str, frozenset[str]], str] = {}
    current = ""
    for line in done.stderr.splitlines():
        if (head := FUNCTION.match(line)) is not None:
            current = head[1].strip('"')
        elif (pair := PAIR.match(line)) is not None and len(names := NAME.findall(pair[2])) == 2:
            found[(current, frozenset(name.replace('"', "") for name in names))] = pair[1]
    return found


def llrm_answers(binary: str, path: Path) -> dict[tuple[str, frozenset[str]], str]:
    done = subprocess.run([binary, "--aliases", str(path)], capture_output=True, text=True)
    found = {}
    for line in done.stdout.splitlines():
        function, answer, first, second = line.split("\t")
        found[(function, frozenset((first, second)))] = answer
    return found


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("files", nargs="+", type=Path)
    parser.add_argument("--llrm-mir", default=str(ROOT / "target/release/llrm-mir"))
    args = parser.parse_args()
    dt = difftest()
    version = subprocess.run([str(dt.LLVM_BIN / "opt"), "--version"], capture_output=True, text=True).stdout.split("\n")[1].strip()
    print(f"opt: {dt.LLVM_BIN / 'opt'} ({version})")
    table: collections.Counter[tuple[str, str]] = collections.Counter()
    unmatched = 0
    for path in args.files:
        text = path.read_text()
        ours, theirs = llrm_answers(args.llrm_mir, path), llvm_answers(dt, text)
        if not ours and not theirs:
            continue  # no two pointers to compare
        if not ours or not theirs:
            print(f"FAILED ROW (no answers from {'llrm' if not ours else 'opt'}): {path}")
            continue
        for key, answer in ours.items():
            if key in theirs:
                table[(answer, theirs[key])] += 1
            else:
                unmatched += 1
    print("ours -> LLVM: pairs")
    for (ours, theirs), count in sorted(table.items()):
        flag = "  <- candidate miscompile" if ours == "NoAlias" and theirs != "NoAlias" else ""
        print(f"  {ours:9} {theirs:12} {count:8}{flag}")
    print(f"pairs llrm printed that aa-eval did not: {unmatched}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
