#!/usr/bin/env python3
"""cmpcells.py BASE.tsv NEW.tsv: cells worse on bytes, instrs or ms.

Rows: cell<TAB>bytes<TAB>instrs<TAB>ms ('-' = not measured). Bytes and
instructions (perf instructions:u per repetition) are hard gates: any increase
is worse. ms is the minimum of 10 runs, flagged past 3 % because a shared box
moves it by that much; read it only with the instrs row beside it."""
import sys

MS_SLACK = 1.03


def read(path):
    rows = {}
    for line in open(path):
        f = line.rstrip("\n").split("\t")
        if len(f) == 4:
            rows[f[0]] = [None if v == "-" else float(v) for v in f[1:]]
    return rows


def worse(base, new):
    out = []
    for cell, (b, i, m) in new.items():
        ob, oi, om = base.get(cell, (None, None, None))
        why = []
        if ob is not None and b is not None and b > ob:
            why.append(f"bytes {ob:.0f}->{b:.0f}")
        if oi is not None and i is not None and i > oi:
            why.append(f"instrs {oi:.0f}->{i:.0f}")
        if om is not None and m is not None and m > om * MS_SLACK:
            why.append(f"ms {om:.0f}->{m:.0f}")
        if why:
            out.append((cell, ", ".join(why)))
    return out


if __name__ == "__main__":
    for cell, why in worse(read(sys.argv[1]), read(sys.argv[2])):
        print(f"WORSE {cell} {why}")
