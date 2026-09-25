"""
BASIC's own arithmetic wraparound and PRINT formatting, shared by every golden
author (tools/mkgolden.py, tools/fuzzgen.py) so there is exactly one
implementation of "what BASIC prints for a number" to get wrong or right.

Not derived from BC or llrm: s16/s32 are two's-complement wraparound, and
num()'s leading sign-or-space is measured off VBDOS directly, not read out of
either compiler's source.
"""


def s16(v: int) -> int:
    return (v + 0x8000) % 0x10000 - 0x8000


def s32(v: int) -> int:
    return (v + 0x80000000) % 0x100000000 - 0x80000000


def num(v: int) -> str:
    return (" " if v >= 0 else "") + str(v)


def seq(*vals: int) -> str:
    # PRINT a; b puts the trailing space of a next to the sign-space of b, so
    # two non-negative numbers come out two spaces apart
    return " ".join(num(v) for v in vals)


def qbfloat(v: float) -> str:
    """A DOUBLE the way QuickBASIC's PRINT writes one.

    Measured on VBDOS: 15 significant digits, no leading
    zero before the point (`.999984741210938`, not `0.999984741210938`), no
    point at all when the value is whole (`2`, not `2.0`) -- %.15g gives all
    three for free except the leading zero, which is BASIC's own habit and
    not Python's.
    """
    text = f"{v:.15g}"
    sign, digits = ("-", text[1:]) if text.startswith("-") else ("", text)
    return (" " if not sign else "-") + (digits[1:] if digits.startswith("0.") else digits)
