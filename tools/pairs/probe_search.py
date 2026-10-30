"""Searching a range of commits with probes that may fail to build.

A probe is `probe(commit) -> ("ok", counts) | ("failed", reason)`. A failed probe is unknown: it is never a measurement, so it is never good and never bad, and a
search never narrows through it (the 2026-10-24 m16 bisect took a commit that did not build for a good one and named the first commit that did build as the culprit).
`first_bad` names a pair of adjacent commits only when both were measured; otherwise it says what was left undetermined.
"""
from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass

Probe = Callable[[str], tuple[str, object]]
REACH = 3  # a failed commit is stood in for by the next one, up to this far


def nearest_ok(commits: list[str], index: int, probe: Probe, below: int | None = None) -> tuple[int, dict] | None:
    """The first measured commit at index, index+1, ... (fewer than `below`, REACH of them at most), or None."""
    for at in range(index, min(index + REACH + 1, len(commits), below if below is not None else len(commits))):
        status, what = probe(commits[at])
        if status == "ok":
            return at, what
    return None


@dataclass
class Found:
    """`last_good` and `first_bad` are indices of measured commits; adjacent when `exact`. `undetermined`: the range between them has no measurable commit left to probe."""

    last_good: int
    first_bad: int
    exact: bool
    undetermined: bool


def first_bad(commits: list[str], probe: Probe, good: int, bad: int, is_bad: Callable[[dict], bool]) -> Found:
    """Narrow (good, bad), two measured indices, to two measured commits side by side where possible."""
    lo, hi = good, bad
    while hi - lo > 1:
        hit = nearest_ok(commits, (lo + hi) // 2, probe, below=hi)
        if hit is None:
            # nothing between the two builds at the middle: try the lower half of the range once
            hit = nearest_ok(commits, lo + 1, probe, below=hi)
        if hit is None:
            return Found(lo, hi, False, True)
        at, counts = hit
        if is_bad(counts):
            hi = at
        else:
            lo = at
    return Found(lo, hi, True, False)
