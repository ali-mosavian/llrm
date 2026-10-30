"""A ledger bucket sums inclusive costs: a function listed with one it calls
counts the callee twice (Together and the Sroa it runs did: 9.3 M, scalar low
and residual high in every ledger since e45020064b)."""
import re


def clean(sym, pin=False):
    """A symbol as the ledger tables name it (ours: no `???:`; the pin's: no file prefix)."""
    sym = re.sub(r"\s*\[[^\]]*\]\s*$", "", sym)
    if pin:
        sym = re.sub(r"^\S*?:(?=\S)", "", sym, count=1) if not sym.startswith(("llvm", "(")) else sym
        return re.sub(r"^/[^ ]*?\.(?:cpp|h):", "", sym)
    return re.sub(r"^\?\?\?:", "", sym)


def normalized(name, pin=False):
    return clean(re.sub(r"\s*\([\d,]+x\)\s*(?=\[|$)", " ", name), pin)


def parse_calling(path, pin=False):
    """{caller: {callee}} from `callgrind_annotate --inclusive=yes --tree=calling`."""
    edges, caller = {}, None
    for line in open(path):
        m = re.match(r"\s*[\d,]+ \(\s*[\d.]+%\)\s+(\*|>)\s+(.*)$", line)
        if not m:
            continue
        name = normalized(m.group(2), pin)
        if m.group(1) == "*":
            caller = name
            edges.setdefault(caller, set())
        elif caller is not None:
            edges[caller].add(name)
    return edges


def edge_costs(path, pin=False):
    """{caller: {callee: Ir}} from the same listing: what each direct call cost (summed over its lines)."""
    costs, caller = {}, None
    for line in open(path):
        m = re.match(r"\s*([\d,]+) \(\s*[\d.]+%\)\s+(\*|>)\s+(.*)$", line)
        if not m:
            continue
        name = normalized(m.group(3), pin)
        if m.group(2) == "*":
            caller = name
            costs.setdefault(caller, {})
        elif caller is not None:
            costs[caller][name] = costs[caller].get(name, 0) + int(m.group(1).replace(",", ""))
    return costs


def analyses_under(costs, passes, analysis, by_callee=None):
    """{pass: Ir} each listed pass spent in direct callees `analysis` (a regex) names: analyses built inside the pass, which the
    pin builds in a frame of its own. `by_callee`, a Counter, gets each callee's share."""
    wanted = re.compile(analysis)
    moved = {}
    for name in passes:
        spent = 0
        for callee, ir in costs.get(name, {}).items():
            if wanted.search(callee):
                spent += ir
                if by_callee is not None:
                    by_callee[callee] += ir
        if spent:
            moved[name] = spent
    return moved


def nested_pairs(edges, listed):
    """(outer, inner) for each listed function that reaches another listed one."""
    listed = set(listed)
    pairs = []
    for outer in sorted(listed):
        seen, todo = set(), list(edges.get(outer, ()))
        while todo:
            name = todo.pop()
            if name in seen:
                continue
            seen.add(name)
            todo.extend(edges.get(name, ()))
        pairs += [(outer, inner) for inner in sorted(seen & listed) if inner != outer]
    return pairs


def overlaps(edges, buckets):
    """What counts a cost twice: a function listed in two buckets, and a listed
    function (in the same bucket or another) that reaches another listed one."""
    twice = []
    owner = {}
    for bucket, names in buckets.items():
        for name in sorted(names):
            if owner.setdefault(name, bucket) != bucket:
                twice.append(f"{name} is in {owner[name]!r} and {bucket!r}")
    listed = set().union(*buckets.values()) if buckets else set()
    twice += [f"{a} calls {b}" for a, b in nested_pairs(edges, listed)]
    return twice


def check(edges, buckets):
    """Refuses overlapping buckets; the line the ledger prints otherwise."""
    found = overlaps(edges, buckets)
    if found:
        raise SystemExit(f"ledger: {len(found)} overlap(s), a cost is counted twice: " + "; ".join(found[:5]))
    return f"double-count check: 0 overlaps over {len(buckets)} buckets"
