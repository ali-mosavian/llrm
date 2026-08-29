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


def fpemu() -> list[str]:
    x, y = 1073741831, 1024
    q = x // y
    a, b = float(q), float(y)
    return [
        f"DIV={num(q)}",
        f"FADD={num(int(a + b))}",
        f"FSUB={num(int(a - b))}",
        f"MOD={num(x - q * y)}",
        f"FMUL={num(int(a * b))}",
        f"FDIV={num(int(a / b))}",
        f"FHALF={num(int(b / 2048 * 10))}",
        f"FSQR={num(int(a**0.5))}",
        f"SADD={num(int(q + 4))}",
        f"SMUL={num(int(q * 4))}",
        f"AND={num(x & 1073741824)}",
        f"FCMP={seq(-(a > b), -(b > a))}",
        "DONE",
    ]


def nbody() -> list[str]:
    """The fixed-point integrator of suite/nbody.bas, in the arithmetic it means.

    BASIC's \\ truncates toward zero where Python's // floors, which differs on
    every negative operand here -- and half the deltas are negative.
    """

    def shift(value: int, by: int) -> int:
        return -((-value) // by) if value < 0 else value // by

    def divide(top: int, bottom: int) -> int:
        return -(-top // bottom) if (top < 0) != (bottom < 0) else top // bottom

    bodies, one, soften, pull = 6, 65536, 65536, 65536
    pos_x = [(i * 7 - 15) * one for i in range(bodies)]
    pos_y = [(i * 5 - 12) * one for i in range(bodies)]
    vel_x = [0] * bodies
    vel_y = [0] * bodies

    for _ in range(100):
        for body in range(bodies):
            acc_x = acc_y = 0
            for other in range(bodies):
                if other == body:
                    continue
                delta_x = pos_x[other] - pos_x[body]
                delta_y = pos_y[other] - pos_y[body]
                dist2 = shift(delta_x, 256) ** 2 + shift(delta_y, 256) ** 2 + soften
                falloff = divide(pull, shift(dist2, one) + 1)
                acc_x += shift(shift(delta_x, 256) * falloff, 256)
                acc_y += shift(shift(delta_y, 256) * falloff, 256)
            vel_x[body] += acc_x
            vel_y[body] += acc_y
        for body in range(bodies):
            pos_x[body] += vel_x[body]
            pos_y[body] += vel_y[body]

    lines = []
    for body in range(bodies):
        for name, values in (("PX", pos_x), ("PY", pos_y), ("VX", vel_x), ("VY", vel_y)):
            lines.append(f"{name}{body}={num(values[body])}")
    return [*lines, "DONE"]


def fixmul() -> list[str]:
    """(int32)(((int64) a * b) >> 16) -- SHRD across edx:eax, not BASIC's \\.

    Python's >> on an int of any sign is a floor shift, which is exactly what
    SHRD produces for the corresponding 32-bit window of a two's-complement
    value: extracting bits 16..47 needs no sign correction of its own.
    """
    cases = [(65536, 131072), (-65536, 131072), (-65536, -131072), (2147483647, 2), (-2147483648, 65536)]
    lines = [f"M{n}={num(s32((a * b) >> 16))}" for n, (a, b) in enumerate(cases, start=1)]
    return [*lines, "DONE"]


PROGRAMS = {
    "arith": arith,
    "procs": procs,
    "jumps": jumps,
    "cmpord": cmpord,
    "flags": flags,
    "divmod": divmod_,
    "nots": nots,
    "fpemu": fpemu,
    "nbody": nbody,
    "fixmul": fixmul,
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
