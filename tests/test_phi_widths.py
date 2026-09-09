"""Phi widths must be proven before copy propagation crosses a loop header."""

import pytest

from qbopt import ir
from qbopt import mir
from qbopt import transform
from qbopt import lir, phielim


@pytest.mark.parametrize("critical", [False, True])
def test_phi_elimination_copies_the_whole_scalar(critical):
    """VBDOS nbody printed PX0=285219921 for 1258: phi copies truncated 32-bit accumulators."""
    def load(at, value):
        return lir.Insn(at=at, covers=(at, at+1), defines=(value,), uses=(),
                        what=ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(value, 4),), (ir.Imm(0x12345678, 4),)))
    read = lir.Insn(at=2, covers=(2, 3), defines=(4,), uses=(3, 1),
                    what=ir.Semantics(ir.Operation.BINARY, "add", (ir.Held(4, 4),), (ir.Held(3, 4), ir.Held(1, 4))))
    body = lir.LirBody(name="wide", entry=0, blocks=(
        lir.LirBlock(at=0, insns=(load(0, 1),), succ=(2, 1) if critical else (2,), phis=()),
        lir.LirBlock(at=1, insns=(load(1, 2),), succ=(2,), phis=()),
        lir.LirBlock(at=2, insns=(read,), succ=(), phis=(lir.Phi(3, ((0, 1), (1, 2))),)),
    ), origin={}, pins={})
    done = phielim.eliminated(body)
    copies = [op for block in done.blocks for op in block.insns if op.group is not None]
    assert len(copies) == 2
    assert all(arg.width == 4 for op in copies for arg in (*op.what.dests, *op.what.sources))


@pytest.mark.parametrize("incoming_width,expected", [(2, 2), (4, None)])
def test_phi_width_meets_incoming_definitions(incoming_width: int, expected: int | None) -> None:
    """LNGMIX copied its counter at the header because the phi had no known width."""
    seed, next_value, merged = mir.Value(1, 0), mir.Value(2, 1), mir.Value(3, 2)
    ops = tuple(
        mir.Op(
            at,
            ir.Operation.MOVE,
            "mov",
            (value,),
            (),
            kind=mir.Kind.COPY,
            args=(mir.Const(1, width),),
            results=(mir.Held(value, width),),
        )
        for at, value, width in [(0, seed, 2), (1, next_value, incoming_width)]
    )
    body = mir.MirBody(
        0,
        (
            mir.MirBlock(0, (), (ops[0],), (2,)),
            mir.MirBlock(1, (), (ops[1],), (2,)),
            mir.MirBlock(2, (mir.Phi(merged, {0: seed, 1: next_value}),), (), (1,)),
        ),
    )
    assert transform._widths(body).get(merged.id) == expected
