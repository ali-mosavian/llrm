#!/usr/bin/env python3
"""cmpcells.py BASE.tsv NEW.tsv: cells worse on bytes, instrs or ms.

Rows: cell<TAB>bytes<TAB>instrs<TAB>ms ('-' = not measured). Bytes are a hard
gate: any increase is worse. Instructions are perf instructions:u per
repetition, estimated as (11 reps - 1 rep) / 10, which moves by about 15 on an
unchanged binary: worse past 0.1 % and 64 instructions. ms is the minimum of 10
runs, flagged past 15 % and only where the base ran 100 ms or more, because a
shared box moves a shorter run by more than that; read it with the instrs row
beside it: an ms-only row is printed CLOCKS? (advisory), a bytes or
instrs row WORSE (the gate)."""

import sys

MS_SLACK = 1.15
MS_FLOOR = 100.0
INSTR_SLACK = 1.001
INSTR_NOISE = 64


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
        if oi is not None and i is not None and i > oi * INSTR_SLACK and i - oi > INSTR_NOISE:
            why.append(f"instrs {oi:.0f}->{i:.0f}")
        if om is not None and m is not None and om >= MS_FLOOR and m > om * MS_SLACK:
            why.append(f"ms {om:.0f}->{m:.0f}")
        if why:
            out.append((cell, ", ".join(why)))
    return out


if __name__ == "__main__":
    for cell, why in worse(read(sys.argv[1]), read(sys.argv[2])):
        gate = "bytes" in why or "instrs" in why
        print(f"{'WORSE' if gate else 'CLOCKS?'} {cell} {why}")
