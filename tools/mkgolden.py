"""
What each suite program must print, computed here rather than captured.

A golden captured from a compiler is a record of what that compiler did, which
is not the same as what the program means. These are authored: 32-bit
two's-complement arithmetic in Python, and QuickBASIC's own PRINT formatting,
which puts a space where the sign of a non-negative number would go.
"""

import sys
from pathlib import Path

from qbprint import num
from qbprint import s32
from qbprint import seq
from qbprint import qbfloat

ROOT = Path(__file__).resolve().parents[1]
SUITE = ROOT / "suite"


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
    # a literal power-of-two divisor, the only shape calls.py's shift-with-bias
    # fires on -- every divisor above is a variable and reaches none of it
    for n, (x, y) in enumerate(((-7, 2), (-7, 512), (-1000000, 512), (1000000, 512)), start=1):
        lines.append(f"SHDIV{n}={num(divide(x, y))}")
        lines.append(f"SHMOD{n}={num(remainder(x, y))}")
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


def fpdeep() -> list[str]:
    """Two-deep float expressions over an indexed array.

    Every value is exact in SINGLE and integral after CLNG, so plain Python
    arithmetic is the same arithmetic: 12, 28 and 60 against a k of 4 give
    halves, three-quarters and seven-eighths, and nothing here rounds.
    """
    lines = []
    for i, v in enumerate((12, 28, 60), start=1):
        lines.append(f"SQ{seq(i)} ={num(v * v)}")
        lines.append(f"RATIO{seq(i)} ={num(v * v // (v + v))}")
        lines.append(f"MIX{seq(i)} ={num((v - 4) * 1024 // (v + 4))}")
    return [*lines, f"DSQ={num(144)}", f"DRATIO={num(6)}", "DONE"]


def chain() -> list[str]:
    """A divide whose divisor is computed by another divide.

    BASIC truncates toward zero where Python's // floors, and the negative
    pair here is exactly the difference -- the same helper suite/nbody.bas
    needs, for the same reason.
    """

    def divide(top: int, bottom: int) -> int:
        return -(-top // bottom) if (top < 0) != (bottom < 0) else top // bottom

    def rem(top: int, bottom: int) -> int:
        return top - divide(top, bottom) * bottom

    a, b, d = 1073741831, 39678839, 100003
    return [
        f"ONE={num(rem(rem(a, b), 1))}",
        f"CONST={num(rem(rem(a, 39678839), 1))}",
        f"CONST2={num(rem(rem(a, 39678839), d))}",
        f"MODMOD={num(rem(rem(a, b), d))}",
        f"DIVDIV={num(divide(divide(a, b), 3))}",
        f"NEGMOD={num(rem(rem(-a, b), d))}",
        f"NEGDIV={num(divide(divide(-a, b), 3))}",
        "DONE",
    ]


def nbody() -> list[str]:
    """The fixed-point integrator of suite/nbody.bas, in the arithmetic it means.

    BASIC's \\ truncates toward zero where Python's // floors, which differs on
    every negative operand here -- and half the deltas are negative.
    """

    def divide(top: int, bottom: int) -> int:
        return -(-top // bottom) if (top < 0) != (bottom < 0) else top // bottom

    bodies, one, damp = 6, 512, 4
    soften, pull = one * one, one
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
                dist2 = delta_x * delta_x + delta_y * delta_y + soften
                falloff = divide(pull, divide(dist2, one * one) + 1)
                acc_x += divide(delta_x * falloff, one)
                acc_y += divide(delta_y * falloff, one)
            vel_x[body] += acc_x
            vel_y[body] += acc_y
            vel_x[body] -= divide(vel_x[body], 1 << damp)
            vel_y[body] -= divide(vel_y[body], 1 << damp)
        for body in range(bodies):
            pos_x[body] += vel_x[body]
            pos_y[body] += vel_y[body]

    lines = []
    for body in range(bodies):
        for name, values in (("PX", pos_x), ("PY", pos_y), ("VX", vel_x), ("VY", vel_y)):
            lines.append(f"{name}{body}={num(values[body])}")
    return [*lines, "DONE"]


def fixmul() -> list[str]:
    """(int32)(((int64) a * b) >> fixShift) -- SHRD across edx:eax, not BASIC's \\.

    Python's >> on an int of any sign is a floor shift, which is exactly what
    SHRD produces for the corresponding 32-bit window of a two's-complement
    value: extracting it needs no sign correction of its own, at any width.
    """
    cases = [
        (65536, 131072, 16),
        (-65536, 131072, 16),
        (-65536, -131072, 16),
        (2147483647, 2, 16),
        (-2147483648, 65536, 16),
        (65536, 131072, 8),
        (65536, 131072, 16),
    ]
    lines = []
    for n, (a, b, shift) in enumerate(cases, start=1):
        result = s32((a * b) >> shift)
        lines.append(f"M{n}={num(result)}")
        lines.append(f"F{n}={qbfloat(result / (1 << shift))}")
    return [*lines, "DONE"]


def arrays() -> list[str]:
    """suite/arrays.bas: array elements and a chained, unstored subexpression.

    The two shapes the corpus has zero of and calls.py cannot absorb through
    today -- an operand that is `x(i)`, not a named static, and a term like
    `s \\ 1000 + 1` that only ever exists in a register.
    """

    def divide(top: int, bottom: int) -> int:
        return -(-top // bottom) if (top < 0) != (bottom < 0) else top // bottom

    n = 4
    x = [s32((i + 1) * 100000) for i in range(n)]
    y = [s32((i + 2) * 100000) for i in range(n)]

    lines = []
    for i in range(n):
        p = s32(x[i] * y[i])
        s = s32(divide(p, 1000) + 1000000)
        f = divide(50000, divide(s, 1000) + 1)
        r = divide(s32(x[i] * f), 512)
        lines.append(f"P{i}=" + num(p))
        lines.append(f"S{i}=" + num(s))
        lines.append(f"F{i}=" + num(f))
        lines.append(f"R{i}=" + num(r))
    return [*lines, "DONE"]


def udt() -> list[str]:
    """suite/udt.bas: a TYPE with two LONG fields, plain and arrayed."""
    c = (305419896, 252645135)
    pts = [(1, 2), (3, 4), (5, 6)]
    return [
        seq(*c),
        seq(*(v for p in pts for v in p)),
        "DONE",
    ]


def arrudt() -> list[str]:
    """suite/arrudt.bas: an array of TYPE Coord, module-level and BPREL."""
    return [seq(11, 33), seq(22, 44), seq(55, 77), seq(66, 88), "DONE"]


def nestud() -> list[str]:
    """suite/nestud.bas: a TYPE nested inside another, plain and arrayed,
    at module and procedure scope, plus a single-field TYPE used bare."""
    return [
        num(111),
        "AB",
        seq(1, 2),
        "CD",
        "EF",
        num(333),
        "GH",
        seq(4, 5),
        "IJ",
        "KL",
        num(999),
        "DONE",
    ]


def arrprm() -> list[str]:
    """suite/arrprm.bas: a SUB taking an array parameter, plain and of a TYPE."""
    return [seq(7, 8), seq(1, 2, 3, 4), "DONE"]


def byref2() -> list[str]:
    """suite/byref2.bas: BYREF SINGLE and DOUBLE parameters."""
    return [qbfloat(2.0), qbfloat(16.0), "DONE"]


def cmpof() -> list[str]:
    """suite/cmpof.bas: the comparisons B$CPI4 answers backwards.

    Authored from what the program means, like every other golden here --
    which is the whole point in this one case, since BC's own build prints
    something else. See the program's own header for the mechanism, and
    configs.DIVERGES for why that disagreement is expected rather than a
    failure.
    """
    pairs = {
        "E": (305430527, 305430528),
        "F": (2147450879, 2147450880),
        "G": (-268402689, -268402688),
    }
    ops = {
        "LT": lambda x, y: x < y,
        "LE": lambda x, y: x <= y,
        "GT": lambda x, y: x > y,
        "GE": lambda x, y: x >= y,
        "EQ": lambda x, y: x == y,
        "NE": lambda x, y: x != y,
    }
    lines = [
        f"{name}{tag}={seq(-test(left, right), -test(right, left))}"
        for name, (left, right) in pairs.items()
        for tag, test in ops.items()
    ]
    return [*lines, "DONE"]


PROGRAMS = {
    "arith": arith,
    "cmpof": cmpof,
    "procs": procs,
    "jumps": jumps,
    "cmpord": cmpord,
    "flags": flags,
    "divmod": divmod_,
    "nots": nots,
    "fpemu": fpemu,
    "fpdeep": fpdeep,
    "chain": chain,
    "nbody": nbody,
    "fixmul": fixmul,
    "arrays": arrays,
    "udt": udt,
    "arrudt": arrudt,
    "nestud": nestud,
    "arrprm": arrprm,
    "byref2": byref2,
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
