"""How a row of the m32 FPU table is judged against the best external (oversight's rule, 2026-11-06, O1 byte axis 2026-11-09).

Values are (clocks, code bytes). O1 has two axes, a row is worse when either is behind the best reference on that axis;
O2 and O3 are one metric, clocks + RATE x bytes; Os is code bytes.
"""

RATE = 0.312


def missing_reference(columns):
    """True when any reference column (gcc, clang) of a row has no cell: the row is refused, not judged against the next-best column."""
    return not columns or any(v is None for v in columns)


def metric(level, value):
    clocks, code = value
    if level == "O1":
        return clocks
    if level == "Os":
        return code
    return clocks + RATE * code


def worse(level, value, refs):
    """Worse than the best of `refs` (gcc, clang). A clock-only O1 passed a row that was smaller in clocks and bigger in bytes."""
    if level == "O1":
        return value[0] > min(r[0] for r in refs) or value[1] > min(r[1] for r in refs)
    return metric(level, value) > min(metric(level, r) for r in refs)


def verdict(level, value, gcc, clang):
    """"ok", "VIRTUAL-OK" or "WORSE" for one row. `clang` is the pin's clang.

    O1 has a per-axis bar. A row where no single real compiler attains both axes (gcc has the best clocks, clang the best bytes, or
    the reverse) is VIRTUAL: it passes when ours is Pareto-optimal among {ours, gcc, clang} and no worse than the pin on both axes
    (oversight, 2026-11-09). Where one compiler has both, ours behind it on either axis is WORSE. Other levels have one metric.
    """
    refs = [gcc, clang]
    if not worse(level, value, refs):
        return "ok"
    if level != "O1":
        return "WORSE"
    best_clocks = min(refs, key=lambda r: r[0])
    best_bytes = min(refs, key=lambda r: r[1])
    if best_clocks is best_bytes or (gcc[0] == clang[0] and gcc[1] == clang[1]):
        return "WORSE"
    dominated = any(r[0] <= value[0] and r[1] <= value[1] and r != value for r in refs)
    behind_pin = value[0] > clang[0] or value[1] > clang[1]
    return "WORSE" if dominated or behind_pin else "VIRTUAL-OK"
