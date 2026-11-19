#!/usr/bin/env python3
"""Gate lines for the wasm cell scripts' row files, which must not read a missing cell as a pass.

`wasm_rows.py ab FILE`: rows of wasm-ab.sh, `prog level base head delta` or `prog level FAIL base|head|both`; prints
`WORSE n HEAD-REFUSED m BASE-REFUSED k BOTH-REFUSED j`; exits 1 on a worse cell or a cell only the head refuses.
`wasm_rows.py sweep FILE`: rows of wasm-sweep.sh, `LEVEL file ours=N pin=M flags=...`, N or M `NA` when no object; prints
`WORSE n UNMEASURED k` and lists the unmeasured; exits 1 on a worse cell, 2 when only cells are unmeasured.
"""
import re
import sys


def ab(lines):
    worse, head, base, both = [], [], [], []
    for line in lines:
        if not line.strip() or line.startswith(("#", "end")):
            continue
        fields = line.split()
        if len(fields) >= 4 and fields[2] == "FAIL":
            {"head": head, "base": base, "both": both}.get(fields[3], head).append(fields[0] + " " + fields[1])
        elif len(fields) == 5 and int(fields[4]) > 0:
            worse.append(" ".join(fields[:2]))
        elif len(fields) == 3 and fields[2] == "FAIL":
            head.append(" ".join(fields[:2]))  # a row that does not say which side is read as the head's
    return worse, head, base, both


def sweep(lines):
    worse, unmeasured = [], []
    for line in lines:
        found = re.match(r"(\w+) (\S+) ours=(\S+) pin=(\S+)", line)
        if not found:
            continue
        level, program, ours, pin = found.groups()
        if not (ours.isdigit() and pin.isdigit()):
            unmeasured.append(f"{program} {level}")
        elif int(ours) > int(pin):
            worse.append(f"{program} {level}")
    return worse, unmeasured


def main(argv):
    mode, path = argv[1], argv[2]
    lines = open(path).read().splitlines()
    if mode == "ab":
        worse, head, base, both = ab(lines)
        print(f"WORSE {len(worse)} HEAD-REFUSED {len(head)} BASE-REFUSED {len(base)} BOTH-REFUSED {len(both)}")
        for name in head:
            print("head refused:", name)
        return 1 if worse or head else 0
    worse, unmeasured = sweep(lines)
    print(f"WORSE {len(worse)} UNMEASURED {len(unmeasured)}")
    for name in unmeasured:
        print("unmeasured:", name)
    return 1 if worse else 2 if unmeasured else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
