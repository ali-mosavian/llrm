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


def procs() -> list[str]:
    a, b = 305419896, 252645135
    r = a & b
    return [
        f"AND={num(r)}",
        f"TWICE={num(s32(r + r))}",
        f"NESTED={num(s32(s32(r + r) * 2))}",
        "DONE",
    ]


def jumps() -> list[str]:
    a, b = 305419896, 252645135
    branch = {1: a & b, 2: a | b, 3: a ^ b}
    case = {1: s32(a + b), 2: s32(a - b), 3: s32(-a)}
    lines = []
    for k in (1, 2, 3):
        lines.append(f"ON{seq(k)} ={num(branch[k])}")
        lines.append(f"CASE{seq(k)} ={num(case[k])}")
    return [*lines, "DONE"]


def cmpord() -> list[str]:
    pairs = {
        "A": (-1, 1),
        "B": (-2147483648, 2147483647),
        "C": (65535, 65536),
        "D": (0x12340000, 0x1234FFFF),
    }
    ops = {
        "LT": lambda x, y: x < y,
        "LE": lambda x, y: x <= y,
        "GT": lambda x, y: x > y,
        "GE": lambda x, y: x >= y,
        "EQ": lambda x, y: x == y,
        "NE": lambda x, y: x != y,
    }
    # BASIC's true is -1
    lines = []
    for name, (left, right) in pairs.items():
        for tag, test in ops.items():
            lines.append(f"{name}{tag}={seq(-test(left, right), -test(right, left))}")
    return [*lines, "DONE"]


def flags() -> list[str]:
    return [
        f"SPLIT={'zero' if (65535 & 61680) == 0 else 'nonzero'}",
        f"FUSED={'zero' if (65535 & 61680) == 0 else 'nonzero'}",
        f"MIRROR={'zero' if s32(-65536) & s32(-65536) == 0 else 'nonzero'}",
        "BOTH=zero",
        f"SIGN={'neg' if s32(65536 - 1) < 0 else 'pos'}",
        "DONE",
    ]


def divmod_() -> list[str]:
    def divide(x: int, y: int) -> int:
        return -(-x // y) if (x < 0) != (y < 0) else x // y

    def remainder(x: int, y: int) -> int:
        return x - divide(x, y) * y

    a, b = 305419896, 252645135
    lines = []
    for n, (x, y) in enumerate(((7, 2), (-7, 2), (-7, -2), (7, -2)), start=1):
        lines.append(f"DIV{n}={num(divide(x, y))}")
        lines.append(f"MOD{n}={num(remainder(x, y))}")
    lines.append(f"DIVBIG={num(divide(b, a))}")
    lines.append(f"MODBIG={num(remainder(b, a))}")
    lines.append(f"MULSMALL={num(s32(a * 3))}")
    # and it does not raise on multiply overflow either -- it wraps, which is
    # what imul does, which is why multiply is absorbed and divide is not
    lines.append(f"MULOVF={num(0)}")
    return [*lines, "DONE"]


def nots() -> list[str]:
    a, b = 305419896, 252645135
    return [
        f"NOT={num(s32(~a))}",
        f"EQV={num(s32(~(a ^ b)))}",
        f"IMP={num(s32(~a | b))}",
        f"NAND={num(s32(~(a & b)))}",
        f"NOTOR={num(s32(~a | b))}",
        "DONE",
    ]


def ctrap() -> list[str]:
    """C's answers, with the two traps defined rather than raised.

    A zero divisor gives zero, which C leaves undefined and qbopt defines.
    -2147483648 \\ -1 gives -2147483648, the wrapping answer idiv would produce
    if it did not fault. Neither raises, so `caught` stays zero -- which is
    where this deliberately disagrees with the program BC built.
    """
    low = -2147483648
    return [
        f"DIVZERO={seq(0, 0)}",
        f"MODZERO={seq(0, 0)}",
        f"DIVEDGE={seq(0, low)}",
        f"MODEDGE={seq(0, 0)}",
        "DONE",
    ]


PROGRAMS = {
    "arith": arith,
    "procs": procs,
    "jumps": jumps,
    "cmpord": cmpord,
    "flags": flags,
    "divmod": divmod_,
    "nots": nots,
    "ctrap": ctrap,
}


def main() -> int:
    (SUITE / "golden").mkdir(parents=True, exist_ok=True)
    for name, model in PROGRAMS.items():
        out = SUITE / "golden" / f"{name}.txt"
        out.write_text("\n".join(model()) + "\n")
        print(f"{out.relative_to(ROOT)}: {len(model())} lines")
    return 0


if __name__ == "__main__":
    sys.exit(main())
