"""The VIRTUAL-OK rule of the owner and FPU tables (oversight, 2026-11-09), once.

A point is a tuple of costs, one per axis, lower is better (bytes, clocks: the caller fixes the order). Each axis has a bar:
the best of the references on it. A row behind a bar is worse, unless it is VIRTUAL.
"""


def is_virtual(refs):
    """No single reference attains the bar on every axis (one has the best clocks, another the best bytes)."""
    bars = tuple(min(axis) for axis in zip(*refs))
    return bool(refs) and all(r != bars for r in refs)


def virtual_ok(value, refs, pin):
    """A row behind the per-axis bars passes when the cell is virtual, `value` is Pareto-optimal among the references
    (none is at least as good on every axis and better on one), and no axis is behind `pin`, the reference the goal is
    stated against (clang for the pin's x86; llvm-mos for the 6502, as the pin has no MOS target)."""
    if not is_virtual(refs):
        return False
    dominated = any(all(r[i] <= value[i] for i in range(len(value))) and r != value for r in refs)
    behind_pin = any(value[i] > pin[i] for i in range(len(value)))
    return not dominated and not behind_pin
