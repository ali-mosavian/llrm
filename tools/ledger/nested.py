"""A ledger bucket sums inclusive costs: a function listed with one it calls
counts the callee twice (Together and the Sroa it runs did: 9.3 M, scalar low
and residual high in every ledger since e45020064b)."""
import re


def normalized(name):
    name = re.sub(r"\s*\[[^\]]*\]\s*$", "", name)
    name = re.sub(r"\s*\([\d,]+x\)\s*$", "", name)
    return re.sub(r"^\?\?\?:", "", name)


def parse_calling(path):
    """{caller: {callee}} from `callgrind_annotate --inclusive=yes --tree=calling`."""
    edges, caller = {}, None
    for line in open(path):
        m = re.match(r"\s*[\d,]+ \(\s*[\d.]+%\)\s+(\*|>)\s+(.*)$", line)
        if not m:
            continue
        name = normalized(m.group(2))
        if m.group(1) == "*":
            caller = name
            edges.setdefault(caller, set())
        elif caller is not None:
            edges[caller].add(name)
    return edges


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
