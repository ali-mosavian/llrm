"""Audited instruction core-clock bounds, separate from scoreboard estimates.

Sources and limitations: docs/timing-audit.md. Bounds exclude decode,
prefix, memory and scheduling costs; they are not whole-program timings.
"""
from dataclasses import dataclass


@dataclass(frozen=True)
class Clocks:
    minimum: int
    maximum: int


def signed_multiply(cpu: str, width: int, *, full: bool = False) -> Clocks | None:
    if width not in (2, 4):
        return None
    match cpu:
        case "386": return Clocks(9, 22 if width == 2 else 38)
        case "486": return Clocks(13, 26 if width == 2 else 42)
        case "P5":
            clocks = 11 if full and width == 2 else 10
            return Clocks(clocks, clocks)
    return None


def signed_divide(cpu: str, width: int) -> Clocks | None:
    if width not in (2, 4):
        return None
    match cpu:
        case "386" | "486": clocks = 27 if width == 2 else 43
        case "P5": clocks = 30 if width == 2 else 46
        case _: return None
    return Clocks(clocks, clocks)
