#!/usr/bin/env python3
"""Self Ir by cost center, ours against the pin, from two callgrind profiles
(`callgrind_annotate --threshold=100`, one function per line), with each
center's top callers from the `--tree=caller` profile. A function belongs to
the first center whose pattern matches; the centers and "other" sum to the
program total (checked), so nothing is counted twice.

costcenters.py OURS_SELF PIN_SELF [OURS_CALLER PIN_CALLER]"""
import re
import sys
from collections import Counter, defaultdict

import nested

# The libc routines by name, not any symbol containing `memcpy`: the pass
# `llrm_transforms::memcpyopt` was counted here (10.9 M, as the top caller).
LIBC_MEM = r"(^|(?<!:):)_*(mem(cpy|move|set|cmp|chr)|bcmp|strlen|strcmp)(_\w+)?( \(|$)"

# (center, ours pattern, pin pattern); order is priority.
CENTERS = [
    ("SipHash (DefaultHasher, RandomState)", r"core::hash::sip|DefaultHasher|SipHasher|hash::random::", r"$^"),
    ("FxHash / other Hasher::write", r"^(?!.*(core::hash::sip|RandomState|DefaultHasher)).*(rustc_hash|::hash::Fx|FxHasher|Hasher>::write|BuildHasher)", r"llvm::hash_|xxh|llvm::hashing|::hash_combine"),
    ("hash table probing (hashbrown / DenseMap, StringMap)", r"hashbrown::|std::collections::hash", r"^(?!.*operator new).*(llvm::DenseMap|LookupBucketFor|llvm::StringMap|llvm::DenseSet|FoldingSet)"),
    ("malloc / free / realloc", r"\b(malloc|free|cfree|realloc|calloc|mi_malloc|mi_free|mi_realloc|mi_zalloc|mi_heap|mi_page)\w*|__rust_(alloc|dealloc|realloc)|alloc::alloc::", r"\b(malloc|free|cfree|realloc|calloc|_int_malloc|_int_free|malloc_consolidate)\b|operator (new|delete)|__libc_(malloc|free)"),
    ("memcpy / memmove / memset / memcmp", LIBC_MEM, LIBC_MEM),
    ("drop_in_place / Drop", r"drop_in_place|::drop\b", r"$^"),
    ("Rc / RefCell / Cell borrow", r"core::cell::|alloc::rc::|RefCell|BorrowRef|Rc<", r"$^"),
    ("Vec / SmallVector growth", r"raw_vec|RawVec|reserve_for_push|finish_grow|grow_one|Vec<.*>::(reserve|push|extend|insert|resize)", r"SmallVectorBase|grow_pod|SmallVectorTemplateBase.*grow|std::vector.*(_M_realloc|reserve|push_back)"),
    ("bounds / panic paths", r"panic_bounds|core::panicking|slice_index|unwrap_failed|expect_failed|option::unwrap", r"$^"),
    ("string formatting / printing", r"core::fmt|alloc::fmt|format_inner|to_string|ToString", r"raw_ostream|llvm::format|snprintf|vfprintf|printf|Twine"),
]


def read_self(path, pin):
    """{function: Ir}, program total."""
    costs, total = Counter(), 0
    for line in open(path):
        m = re.match(r"\s*([\d,]+) \(\s*[\d.]+%\)\s+(.*)$", line)
        if not m:
            continue
        ir, rest = int(m.group(1).replace(",", "")), m.group(2)
        if "PROGRAM TOTALS" in rest:
            total = ir
        else:
            costs[nested.clean(rest, pin)] += ir
    return costs, total


def own_name(symbol):
    """The symbol without generic arguments: `Vec<Rc<T>>::push::<U>` is Vec's push,
    not Rc's. A `<` that opens a qualified path (`<T as Trait>::f`) is kept."""
    out, depth = [], 0
    for at, ch in enumerate(symbol):
        if ch == "<":
            opens_arguments = at > 0 and (symbol[at - 1].isalnum() or symbol[at - 1] in "_:>")
            if depth or opens_arguments:
                depth += 1
                continue
        elif ch == ">" and depth:
            depth -= 1
            continue
        if not depth:
            out.append(ch)
    return "".join(out)


def centers_of(symbol, pin):
    symbol = own_name(symbol)
    return [name for name, ours, theirs in CENTERS if re.search(theirs if pin else ours, symbol)]


def center_of(symbol, pin):
    """The one center a symbol is in. Two matching is an error, not a priority:
    first-match-wins hid RandomState::hash_one and sip's write in both the
    SipHash and the Fx center."""
    found = centers_of(symbol, pin)
    if len(found) > 1:
        raise ValueError(f"{symbol[:100]} is in {len(found)} centers: {found}")
    return found[0] if found else "other"


def by_center(costs, total, pin):
    sums = Counter()
    for symbol, ir in costs.items():
        sums[center_of(symbol, pin)] += ir  # raises on a symbol in two centers
    # Callgrind attributes everything to some function: a gap is a lost line.
    assert abs(sum(sums.values()) - total) <= total * 0.002, f"centers sum to {sum(sums.values())}, total {total}"
    return sums


def callers(path, pin):
    """{center: Counter(caller -> Ir)} from `--tree=caller`: the `<` lines above a `*` line."""
    found, above = defaultdict(Counter), []
    for line in open(path):
        m = re.match(r"\s*([\d,]+) \(\s*[\d.]+%\)\s+(<|\*)\s+(.*)$", line)
        if not m:
            continue
        ir = int(m.group(1).replace(",", ""))
        name = nested.normalized(m.group(3), pin)
        if m.group(2) == "<":
            above.append((name, ir))
            continue
        center = center_of(name, pin)
        if center != "other":
            for caller, edge in above:
                if center_of(caller, pin) != center:
                    found[center][caller] += edge
        above = []
    return found


def report(ours, pin, ours_callers=None):
    (oc, ot), (pc, pt) = ours, pin
    o, p = by_center(oc, ot, False), by_center(pc, pt, True)
    out = [f"| cost center | ours M | % | pin M | % |", "|---|--:|--:|--:|--:|"]
    for name in [c[0] for c in CENTERS] + ["other"]:
        out.append(f"| {name} | {o[name]/1e6:.1f} | {100*o[name]/ot:.2f} | {p[name]/1e6:.1f} | {100*p[name]/pt:.2f} |")
    out.append(f"| total | {ot/1e6:.1f} | 100 | {pt/1e6:.1f} | 100 |")
    out.append("")
    out.append(f"cost-center check: 0 symbols in two centers over {len(oc)} ours + {len(pc)} pin symbols")
    if ours_callers:
        out.append("")
        for name in [c[0] for c in CENTERS]:
            top = ours_callers.get(name, Counter()).most_common(5)
            if top:
                out.append(f"{name}: " + "; ".join(f"{re.sub(r'<(\\w+::)*(\\w+)( as .*)?>::', r'\\2::', c)[:60]} {v/1e6:.2f}" for c, v in top))
    return "\n".join(out)


if __name__ == "__main__":
    args = sys.argv[1:]
    ours, pin = read_self(args[0], False), read_self(args[1], True)
    print(report(ours, pin, callers(args[2], False) if len(args) > 2 else None))
