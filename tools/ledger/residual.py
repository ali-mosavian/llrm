#!/usr/bin/env python3
"""What the ledger's residual is made of: the cost under `driver::compiled`
that no listed pass run (the buckets' functions) accounts for, by function.

residual.py CALLING LISTED [ROOT] [TOP]
CALLING: `callgrind_annotate --inclusive=yes --tree=calling` of our profile;
LISTED: the file `LEDGER_LISTED=` made ledger4.py write (one function per line).

A walk from ROOT hands each callee the inclusive cost of the call edge; a
listed function ends the walk (its cost is its bucket's); what a function does
not hand on is its own part. Edge costs are what callgrind recorded, so a
callee shared by two callers is shared by the edges, not counted twice."""
import re
import sys
from collections import Counter

import nested


def parse_edges(path):
    """{caller: [(callee, inclusive Ir of the edge)]} and {function: inclusive Ir}."""
    edges, inclusive, caller = {}, {}, None
    for line in open(path):
        m = re.match(r"\s*([\d,]+) \(\s*[\d.]+%\)\s+(\*|>)\s+(.*)$", line)
        if not m:
            continue
        ir, name = int(m.group(1).replace(",", "")), nested.normalized(m.group(3))
        if m.group(2) == "*":
            caller = name
            inclusive[name] = ir
            edges.setdefault(name, [])
        elif caller is not None:
            edges[caller].append((name, ir))
    return edges, inclusive


def attribute(edges, inclusive, listed, root, floor=0):
    """{function: own Ir outside the listed passes}. Flow runs down the call
    graph in reverse post-order (a call back up a recursion is ignored): a
    function reached with F of its inclusive I hands each callee the share
    F / I of the edge, and keeps the rest. A listed function ends the flow."""
    order, seen, stack = [], {root}, [(root, iter(edges.get(root, ())))]
    while stack:
        name, children = stack[-1]
        for callee, _edge in children:
            if callee not in seen:
                seen.add(callee)
                stack.append((callee, iter(edges.get(callee, ()))))
                break
        else:
            order.append(name)
            stack.pop()
    order.reverse()
    place = {name: at for at, name in enumerate(order)}
    flow, own = Counter({root: inclusive[root]}), Counter()
    for name in order:
        if name in listed:
            continue
        reached, whole = flow[name], max(inclusive.get(name, 0), 1)
        handed = 0
        for callee, edge in edges.get(name, ()):
            if callee == name or place.get(callee, -1) <= place[name]:
                continue
            share = min(edge * reached / whole, reached - handed)
            if share > floor:
                flow[callee] += share
                handed += share
        own[name] += reached - handed
    return own


if __name__ == "__main__":
    calling, listed = sys.argv[1], set(open(sys.argv[2]).read().split("\n"))
    root = sys.argv[3] if len(sys.argv) > 3 else "llrm_codegen::driver::compiled"
    top = int(sys.argv[4]) if len(sys.argv) > 4 else 30
    edges, inclusive = parse_edges(calling)
    own = attribute(edges, inclusive, listed, root)
    print(f"under {root}: {inclusive[root]/1e6:.1f} M; outside the listed passes: {sum(own.values())/1e6:.1f} M")
    for name, ir in own.most_common(top):
        print(f"{ir/1e6:8.2f}  {name[:150]}")
