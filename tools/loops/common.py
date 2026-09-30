"""Shared pieces for writing cases: the edge trip counts and input builders."""

from spec import Fill, Input

EDGE_TRIPS = (0, 1, 2, 3, 15, 16, 17, 255, 256, 32766, 32767, 32768, 65535)


def trips(limit: int) -> list[int]:
    """The edge trip counts an array of `limit` elements can take."""
    return [one for one in EDGE_TRIPS if one <= limit]


def inputs(arrays, rows: list[tuple[int, ...]], lo: int = 0, span: int = 0) -> tuple[Input, ...]:
    """One input per row of scalar arguments; every array filled from a seed
    that differs per array and per row."""
    return tuple(
        Input(tuple(row), tuple((a.name, Fill(seed=11 + 97 * at + 13 * k, lo=lo, span=span)) for k, a in enumerate(arrays)))
        for at, row in enumerate(rows)
    )
