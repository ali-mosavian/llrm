"""How a row of the m32 FPU table is judged against the best external (oversight's rule, 2026-11-06, O1 byte axis 2026-11-09).

Values are (clocks, code bytes). O1 has two axes, a row is worse when either is behind the best reference on that axis;
O2 and O3 are one metric, clocks + RATE x bytes; Os is code bytes.
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[4] / "tools"))
import virtual_ok  # noqa: E402

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

    O1 has a per-axis bar; a virtual cell (no real compiler attains both axes) passes by tools/virtual_ok.py, the one rule of
    the owner tables. Other levels have one metric.
    """
    refs = [gcc, clang]
    if not worse(level, value, refs):
        return "ok"
    return "VIRTUAL-OK" if level == "O1" and virtual_ok.virtual_ok(value, refs, clang) else "WORSE"
