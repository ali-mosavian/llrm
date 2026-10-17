#!/usr/bin/env python3
"""C source of `f` with about N statements in one of five shapes, for compile-cost ladders.

    gen.py SHAPE N        SHAPE: arith | mem | loop | diamond | calls

Same C89 text goes to every compiler (wccq takes no C99 declarations); unsigned arithmetic only, so there is no UB to
fold differently. Every value reaches the return or a store, so none is dead.
"""
from __future__ import annotations

import random
import sys

SHAPES = ("arith", "mem", "loop", "diamond", "calls")
OPS = ("+", "-", "^")


def arith(n: int, rnd: random.Random) -> str:
    out = ["unsigned f(unsigned a, unsigned b, unsigned c, unsigned d) {", "  unsigned v[8];", "  v[0] = a; v[1] = b; v[2] = c; v[3] = d; v[4] = a ^ b; v[5] = b ^ c; v[6] = c ^ d; v[7] = d ^ a;"]
    prev = "a + b"
    for i in range(n):
        x, y = rnd.randrange(8), rnd.randrange(8)
        expr = prev if i % 3 == 2 else f"(v[{x}] {rnd.choice(OPS)} (v[{y}] {rnd.choice(OPS)} {rnd.randrange(1, 99)}u)) * {rnd.randrange(1, 99) | 1}u"
        prev = expr
        out.append(f"  v[{i % 8}] = {expr};")
    out += ["  return v[0] ^ v[1] ^ v[2] ^ v[3] ^ v[4] ^ v[5] ^ v[6] ^ v[7];", "}"]
    return "\n".join(out)


def mem(n: int, rnd: random.Random) -> str:
    out = ["void f(unsigned *p, unsigned n) {"]
    for _ in range(n):
        d, a, b = (rnd.randrange(16) for _ in range(3))
        out.append(f"  p[{d}] = p[{a}] {rnd.choice(OPS)} p[{b}] + n;")
    out.append("}")
    return "\n".join(out)


def loop(n: int, rnd: random.Random) -> str:
    out = ["unsigned f(unsigned *a, unsigned n, unsigned x, unsigned y) {", "  unsigned s = 0, t = 1, i;",
           "  for (i = 0; i < n; i++) {"]
    for _ in range(n):
        out.append(f"    s += a[i] * (x * y + {rnd.randrange(1, 99)}u) {rnd.choice(('+', '^'))} (i << {rnd.randrange(1, 5)});")
    out += ["    t += s;", "  }", "  return s + t;", "}"]
    return "\n".join(out)


def diamond(n: int, rnd: random.Random) -> str:
    out = ["unsigned f(unsigned x, unsigned a, unsigned b) {", "  unsigned y = a;"]
    for i in range(max(1, n // 4)):
        out.append(f"  if (x & {1 << (i % 31)}u) y = y + a * {rnd.randrange(1, 9)}u; else y = y * b + {rnd.randrange(1, 9)}u;")
    out += ["  return y;", "}"]
    return "\n".join(out)


def calls(n: int, rnd: random.Random) -> str:
    out = ["unsigned ext(unsigned, unsigned);", "unsigned f(unsigned a, unsigned b) {", "  unsigned s = a;"]
    for _ in range(max(1, n // 3)):
        out.append(f"  s = s + ext(s ^ b, {rnd.randrange(1, 99)}u) * {rnd.randrange(1, 9)}u;")
    out += ["  return s;", "}"]
    return "\n".join(out)


def generate(shape: str, n: int) -> str:
    return globals()[shape](n, random.Random(n)) + "\n"


if __name__ == "__main__":
    if len(sys.argv) != 3 or sys.argv[1] not in SHAPES:
        sys.exit(__doc__)
    sys.stdout.write(generate(sys.argv[1], int(sys.argv[2])))
