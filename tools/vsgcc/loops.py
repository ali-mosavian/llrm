"""Hottest innermost loop of a program's kernel, per compiler: loops.py PROG VARIANT...

Loops are natural loops of the executed instruction graph (back edge = edge into a dominator),
weight = instructions executed inside the body, callees excluded. Prints the body in address
order with executions per iteration of the header.
"""
import sys
from collections import defaultdict

from iced_x86 import Decoder, Formatter, FormatterSyntax, Mnemonic

import harness


def dominators(nodes, preds, entry):
    dom = {n: set(nodes) for n in nodes}
    dom[entry] = {entry}
    changed = True
    order = sorted(nodes)
    while changed:
        changed = False
        for n in order:
            if n == entry:
                continue
            ps = [dom[p] for p in preds[n] if p in dom]
            new = set.intersection(*ps) | {n} if ps else {n}
            if new != dom[n]:
                dom[n] = new; changed = True
    return dom


def loops(r):
    hits, edges = r["hits"], r["edges"]
    nodes = set(hits)
    succ, pred = defaultdict(set), defaultdict(set)
    for (a, b) in edges:
        succ[a].add(b); pred[b].add(a)
    dom = dominators(nodes, pred, r["kern"])
    by_header = defaultdict(set)
    for (a, b) in edges:
        if b in dom.get(a, ()):
            body, work = {b, a}, [a]
            while work:
                n = work.pop()
                if n == b:
                    continue
                for p in pred[n]:
                    if p not in body:
                        body.add(p); work.append(p)
            by_header[b] |= body
    out = []
    for h, body in by_header.items():
        inner = [h2 for h2, b2 in by_header.items() if h2 != h and h2 in body and h not in b2]
        out.append((h, body, bool(inner), sum(hits[a] for a in body)))
    return out


def best(r):
    ls = [l for l in loops(r) if not l[2]]          # innermost only
    return max(ls, key=lambda l: l[3]) if ls else None


def show(r, label):
    total = r["ins"]
    b = best(r)
    f = Formatter(FormatterSyntax.INTEL)
    if b is None:
        print(f"## {label}: no loop (recursive or straight-line); instructions {total}")
        return None
    h, body, _, weight = b
    n = r["hits"][h]
    real = [a for a in sorted(body) if r["cache"][a][0].mnemonic != Mnemonic.NOP]
    per = sum(r["hits"][a] for a in real) / n
    mem = sum(r["hits"][a] * r["cache"][a][1] for a in real) / n
    print(f"## {label}: header {h:#x}, {n} iterations, {per:.2f} instructions and {mem:.2f} memory operands per iteration, {weight / total:.0%} of {total}")
    for a in sorted(body):
        ins = r["cache"][a][0]
        mark = " " if r["hits"][a] == n else ("*" if r["hits"][a] < n else "+")
        print(f"  {mark}{r['hits'][a] / n:6.2f}  {f.format(ins)}")
    return per, mem


if __name__ == "__main__":
    prog = sys.argv[1]
    for v in sys.argv[2:]:
        show(harness.run(prog, v), f"{prog} {v}")
