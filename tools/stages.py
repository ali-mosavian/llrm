"""`stage(name)`: one flushed, timestamped line at the START of a stage, so a batch log carries stage times.

The squash and the smoke printed their log at the end of the run: the 16:16 batch showed every stage at 16:23:48.
"""
from __future__ import annotations

import time

PREFIX = "STAGE"


def stage(name: str) -> None:
    mark(name)
    print(f"{time.strftime('%H:%M:%S')} {PREFIX} {name}", flush=True)


_seen: list[tuple[str, float]] = []


def mark(name: str, at: float | None = None) -> None:
    """Records the START of a stage for `summary`; `stage` calls it."""
    _seen.append((name, time.monotonic() if at is None else at))


def summary(end: float | None = None) -> str:
    """`STAGES <total>s: <name> <s> ...`, biggest first, from the stages marked since the last summary. A stage lasts until the next one starts; `#N` is dropped from per-PR names.
    The time after a `log #N` stage is the next PR's work before its squash, so it is `pre-squash`, not logging."""
    end = time.monotonic() if end is None else end
    spent: dict[str, float] = {}
    for (name, begun), (_, finished) in zip(_seen, [*_seen[1:], ("", end)]):
        key = "pre-squash" if name.startswith("log #") else name.split(" #")[0]
        spent[key] = spent.get(key, 0.0) + finished - begun
    _seen.clear()
    if not spent:
        return ""
    ranked = sorted(spent.items(), key=lambda item: -item[1])
    return f"{PREFIX}S {sum(spent.values()):.0f}s: " + " ".join(f"{name} {seconds:.0f}s" for name, seconds in ranked)
