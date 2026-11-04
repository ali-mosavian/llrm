"""cmpcells.py BASE.tsv NEW.tsv: cells worse on bytes, instrs or ms.

Rows: cell<TAB>bytes<TAB>instrs<TAB>ms ('-' = not measured). Bytes and
instrs are hard gates: any increase is WORSE. Instrs come from
instrcount.sh, exact for an unchanged binary, so no slack is needed. ms is
the minimum of 10 runs on a shared box and is advisory: past 15 % on a base of
100 ms or more with bytes and instrs equal, the cell prints CLOCKS-ONLY? (a
regression in clocks alone is possible: read it); with bytes or instrs up it
rides on that row."""

import sys

MS_SLACK = 1.15
MS_FLOOR = 100.0


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
        if om is not None and m is not None and om >= MS_FLOOR and m > om * MS_SLACK:
            why.append(f"ms {om:.0f}->{m:.0f}")
        if why:
            out.append((cell, ", ".join(why)))
    return out


if __name__ == "__main__":
    for cell, why in worse(read(sys.argv[1]), read(sys.argv[2])):
        gate = "bytes" in why or "instrs" in why
        print(f"{'WORSE' if gate else 'CLOCKS-ONLY?'} {cell} {why}")
