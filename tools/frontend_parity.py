"""Show final allocated BASIC and C kernels without hiding codegen work.

    uv run python tools/frontend_parity.py algebra branch loop

Only BASIC's explicit B$ENRA/B$EXSA frame calls and both languages' return
instructions are omitted.  Parameter offsets, physical registers, branches,
loads, stores, spills, helpers, and every block after an early exit remain
visible.  This is deliberately a raw listing, not a normalized score: a
frontend-dependent instruction cannot disappear behind an equivalence rule.
"""

from __future__ import annotations

import sys
import argparse
from pathlib import Path
from dataclasses import dataclass

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tests"))

import corpus
from qbopt import wholeseg
from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import masm
from qbopt.cfront import compile as cfront

SOURCE = ROOT / "bench" / "parity"
FIXTURES = ROOT / "fixtures" / "parity"


@dataclass(frozen=True)
class Pair:
    basic: str
    c: str


PAIRS = {
    "scalar": Pair("PARITYSCALAR", "_parity_scalar"),
    "algebra": Pair("PARITYALGEBRA", "_parity_algebra"),
    "branch": Pair("PARITYBRANCH", "_parity_branch"),
    "control": Pair("PARITYCONTROL", "_parity_control"),
    "loop": Pair("PARITYLOOP", "_parity_loop"),
    "memory": Pair("PARITYMEMORY", "_parity_memory"),
}


def _call_name(one: lir.Insn, calls: dict[int, str]) -> str | None:
    return (
        calls.get(one.op.at)
        if one.op is not None and one.what is not None and one.what.op is ir.Operation.CALL
        else None
    )


def basic_core(body: lir.LirBody, calls: dict[int, str]) -> tuple[tuple[int, str], ...]:
    """The complete allocated kernel after BASIC's frame-entry call.

    B$EXSA can occur before a later laid-out branch block, so this filters
    individual ABI calls instead of truncating the body at the first exit.
    """
    started = False
    out = []
    for block in body.blocks:
        for one in block.insns:
            callee = _call_name(one, calls)
            if not started:
                started = callee == "B$ENRA"
                continue
            if (
                callee in {"B$ENRA", "B$EXSA"}
                or one.what is None
                or one.what.op
                in {
                    ir.Operation.NOTHING,
                    ir.Operation.RETURN,
                }
            ):
                continue
            out.append((block.at, _line(one, calls)))
    return tuple(out)


def c_core(body: lir.LirBody) -> tuple[tuple[int, str], ...]:
    return tuple(
        (block.at, _line(one, {}))
        for block in body.blocks
        for one in block.insns
        if one.what is not None and one.what.op not in {ir.Operation.NOTHING, ir.Operation.RETURN}
    )


def _line(one: lir.Insn, calls: dict[int, str]) -> str:
    if callee := _call_name(one, calls):
        return f"call {callee}"
    lines = masm._instruction(one.what, {}, 0)
    return " ; ".join(lines)


def pair(name: str) -> tuple[tuple[tuple[int, str], ...], tuple[tuple[int, str], ...]]:
    spec = PAIRS[name]
    fixture = FIXTURES / f"{name}-v-g3.obj"
    found = corpus.loaded(fixture)
    basic_bodies = {}

    def basic_watch(stage, procedure, body):
        if stage == "jumps" and procedure == f"procedure {spec.basic}":
            basic_bodies[procedure] = body

    emitted = wholeseg.emitted(fixture.read_bytes(), watch=basic_watch)
    if emitted.outcome is not wholeseg.Emission.LIR:
        raise RuntimeError(f"{name} BASIC emission: {emitted.reason}")
    basic = basic_core(next(iter(basic_bodies.values())), found.calls)

    c_bodies = {}

    def c_watch(stage, procedure, body):
        if stage == "lir-jumps" and procedure == spec.c:
            c_bodies[procedure] = body

    source = SOURCE / f"{name}.c"
    cfront.assembled(cfront.recorded(source, []), source.stem, optimise=True, watch=c_watch)
    return basic, c_core(next(iter(c_bodies.values())))


def table(name: str, basic: tuple[tuple[int, str], ...], c: tuple[tuple[int, str], ...]) -> str:
    left = [f"{at:04x}  {line}" for at, line in basic]
    right = [f"{at:04x}  {line}" for at, line in c]
    width = max([len("BASIC"), *(len(one) for one in left)]) + 3
    rows = [f"{name.upper()}", f"{'BASIC':<{width}}C"]
    for index in range(max(len(left), len(right))):
        rows.append(
            f"{(left[index] if index < len(left) else ''):<{width}}{right[index] if index < len(right) else ''}"
        )
    return "\n".join(rows)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("pairs", nargs="*", choices=tuple(PAIRS), default=tuple(PAIRS))
    args = parser.parse_args()
    print("\n\n".join(table(name, *pair(name)) for name in args.pairs))


if __name__ == "__main__":
    main()
