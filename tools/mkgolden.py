"""
What each suite program must print, computed here rather than captured.

A golden captured from a compiler is a record of what that compiler did, which
is not the same as what the program means. These are authored: 32-bit
two's-complement arithmetic in Python, and QuickBASIC's own PRINT formatting,
which puts a space where the sign of a non-negative number would go.
"""

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SUITE = ROOT / "suite"


def s32(v: int) -> int:
    return (v + 0x80000000) % 0x100000000 - 0x80000000


def num(v: int) -> str:
    return (" " if v >= 0 else "") + str(v)


def seq(*vals: int) -> str:
    # PRINT a; b puts the trailing space of a next to the sign-space of b, so
    # two non-negative numbers come out two spaces apart
    return " ".join(num(v) for v in vals)


def arith() -> list[str]:
    a, b, one = 305419896, 252645135, 1
    r = s32(s32(s32(a & b) ^ a) + b)
    return [
        f"AND={num(a & b)}",
        f"OR={num(a | b)}",
        f"XOR={num(a ^ b)}",
        f"ADD={num(s32(a + b))}",
        f"SUB={num(s32(a - b))}",
        f"NEG={num(s32(-a))}",
        f"CHAIN={num(r)}",
        f"CARRY={num(s32(65535 + one))}",
        f"BORROW={num(s32(65536 - one))}",
        f"INTS={seq(258, 772)}",
        "DONE",
    ]


PROGRAMS = {"arith": arith}


def main() -> int:
    (SUITE / "golden").mkdir(parents=True, exist_ok=True)
    for name, model in PROGRAMS.items():
        out = SUITE / "golden" / f"{name}.txt"
        out.write_text("\n".join(model()) + "\n")
        print(f"{out.relative_to(ROOT)}: {len(model())} lines")
    return 0


if __name__ == "__main__":
    sys.exit(main())
