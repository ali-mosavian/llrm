#!/usr/bin/env python3
"""C programs of calls under conditions, for the differential against the pin's clang.

    tools/difftest/gen.py [--seed N] [--shape SHAPE]

`emit(seed, shape)` returns the text of one C file: `g` over ints, and callees `c0..` that are declared only. The
driver defines each callee so that it prints its name; the traces of two compilers must agree. A wrong callee, a merged
call or a lost branch shows as a different trace. The same seed and shape give the same text.
"""
from __future__ import annotations

import argparse
import random

SHAPES = ("ladder", "sibling", "loop_exit", "widths")
CMP = ("==", "!=", "<", ">", "<=", ">=")


def _cond(rng: random.Random, args: list[str]) -> str:
    """A test of one or two of the arguments."""
    one = f"{rng.choice(args)} {rng.choice(CMP)} {rng.randint(-2, 9)}"
    if rng.random() < 0.4:
        other = f"({rng.choice(args)} & {rng.choice((1, 2, 4))})"
        return f"{one} {rng.choice(('&&', '||'))} {other}"
    return one


def _call(rng: random.Random, callees: int, args: list[str]) -> str:
    return f"c{rng.randrange(callees)}({', '.join(args[:2])})"


def _header(callees: int, width: str) -> str:
    return "".join(f"int c{i}({width}, {width});\n" for i in range(callees))


def ladder(rng: random.Random) -> str:
    """An if/else chain whose every arm calls a callee, each call the last thing done: sibling calls."""
    callees = rng.randint(2, 5)
    args = ["a", "b"]
    arms = rng.randint(2, 6)
    body = ""
    for i in range(arms):
        body += f"    {'if' if i == 0 else 'else if'} ({_cond(rng, args)}) return {_call(rng, callees, args)};\n"
    body += f"    return {rng.choice(('0', _call(rng, callees, args)))};\n"
    return _header(callees, "int") + f"int g(int a, int b)\n{{\n{body}}}\n"


def sibling(rng: random.Random) -> str:
    """A void function calling under nested conditions, with early returns."""
    callees = rng.randint(2, 4)
    args = ["a", "b"]
    lines = []
    for _ in range(rng.randint(2, 5)):
        lines.append(f"    if ({_cond(rng, args)}) {{")
        if rng.random() < 0.5:
            lines.append(f"        if ({_cond(rng, args)}) {{ {_call(rng, callees, args)}; return; }}")
        lines.append(f"        {_call(rng, callees, args)};")
        lines.append("    }")
    if rng.random() < 0.7:
        lines.append(f"    {_call(rng, callees, args)};")
    return _header(callees, "int") + "void g(int a, int b)\n{\n" + "\n".join(lines) + "\n}\n"


def loop_exit(rng: random.Random) -> str:
    """A loop with an early exit that calls, and a call after it."""
    callees = rng.randint(2, 4)
    bound = rng.randint(2, 5)
    body = (
        f"    int i;\n    for (i = 0; i < {bound}; i++) {{\n"
        f"        if (a + i {rng.choice(CMP)} b) return {_call(rng, callees, ['i', 'a'])};\n"
        f"        if ((a ^ i) & {rng.choice((1, 2, 3))}) {_call(rng, callees, ['b', 'i'])};\n    }}\n"
        f"    return {_call(rng, callees, ['a', 'b'])};\n"
    )
    return _header(callees, "int") + f"int g(int a, int b)\n{{\n{body}}}\n"


def widths(rng: random.Random) -> str:
    """Narrow arguments that widen into the callees' parameters."""
    callees = rng.randint(2, 4)
    kind = rng.choice(("short", "char", "unsigned char", "unsigned short"))
    lines = [f"    {kind} x = ({kind})a;", f"    {kind} y = ({kind})(b + 1);"]
    for _ in range(rng.randint(2, 4)):
        lines.append(f"    if (x {rng.choice(CMP)} y) return {_call(rng, callees, ['x', 'y'])};")
    lines.append(f"    return {_call(rng, callees, ['y', 'x'])};")
    return _header(callees, "int") + "int g(int a, int b)\n{\n" + "\n".join(lines) + "\n}\n"


def emit(seed: int, shape: str) -> str:
    """One program: `shape` of `SHAPES`, drawn from `seed`."""
    if shape not in SHAPES:
        raise ValueError(f"unknown shape {shape!r}; one of {', '.join(SHAPES)}")
    rng = random.Random(f"{shape}:{seed}")
    return {"ladder": ladder, "sibling": sibling, "loop_exit": loop_exit, "widths": widths}[shape](rng)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--seed", type=int, default=0)
    ap.add_argument("--shape", choices=SHAPES, default="ladder")
    args = ap.parse_args()
    print(emit(args.seed, args.shape), end="")


if __name__ == "__main__":
    main()
