"""Phi widths must be proven before copy propagation crosses a loop header."""

import pytest

from qbopt.model import ir
from qbopt.model import mir
from qbopt.optimize import transform
from qbopt.model import lir
from qbopt.backend import phielim


def test_split_fallthrough_gets_an_explicit_jump():
    """EVTRAP's split exit edge was emitted unreachable after the handler."""
    branch = lir.Insn(at=0, covers=(0, 2), defines=(), uses=(),
                     what=ir.Semantics(ir.Operation.BRANCH, "jz", (), (), 20))
    body = lir.LirBody(name="edge", entry=0, blocks=(
        lir.LirBlock(at=0, insns=(branch,), succ=(10, 20), phis=()),
    ), origin={}, pins={})
    done = phielim._split_edges(body, {(0, 10): [(2, 1)]}, {}, {}, {0: ()}, {2: 2})
    edge = done.blocks[-1]
    last = done.blocks[0].insns[-1]
    assert last.what.op is ir.Operation.JUMP
    assert last.what.target == edge.at
    assert last.at == branch.at


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


def test_phi_on_a_single_predecessor_exit_does_not_split_the_edge() -> None:
    """HARR gained an empty jump trampoline after LCSSA closed its loop exit."""
    branch = lir.Insn(
        at=0,
        covers=(0, 2),
        defines=(),
        uses=(),
        what=ir.Semantics(ir.Operation.BRANCH, "jz", (), (), 2),
    )
    use = lir.Insn(
        at=2,
        covers=(2, 3),
        defines=(),
        uses=(3,),
        what=ir.Semantics(ir.Operation.MOVE, "mov", (), (ir.Held(3, 2),)),
        widths=((3, 2),),
    )
    body = lir.LirBody(
        name="exit",
        entry=0,
        blocks=(
            lir.LirBlock(at=0, insns=(branch,), succ=(1, 2)),
            lir.LirBlock(at=1, insns=(), succ=()),
            lir.LirBlock(at=2, insns=(use,), succ=(), phis=(lir.Phi(3, ((0, 1),)),)),
        ),
        origin={},
        pins={},
    )

    done = phielim.eliminated(body)

    assert len(done.blocks) == len(body.blocks)
    exit_block = next(block for block in done.blocks if block.at == 2)
    assert exit_block.phis == ()
    assert all(insn.group is None for block in done.blocks for insn in block.insns)
    assert exit_block.insns[0].uses == (1,)
    assert exit_block.insns[0].widths == ((1, 2),)


def test_phi_source_live_on_the_other_branch_gets_an_edge_copy() -> None:
    """nbody spilled its inner counter after exit copies extended both accumulators.

    The phi result belongs only to the exit, but its source remains live on
    the continuing branch.  Keeping the copy before the branch makes both
    values overlap and prevents coalescing; on the edge they are one value.
    """
    branch = lir.Insn(
        at=0,
        covers=(0, 2),
        defines=(),
        uses=(),
        what=ir.Semantics(ir.Operation.BRANCH, "jz", (), (), 2),
    )
    use_source = lir.Insn(
        at=1,
        covers=(1, 2),
        defines=(),
        uses=(1,),
        what=ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Held(1, 2),)),
    )
    use_result = lir.Insn(
        at=2,
        covers=(2, 3),
        defines=(),
        uses=(3,),
        what=ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Held(3, 2),)),
    )
    body = lir.LirBody(
        name="live-source",
        entry=0,
        blocks=(
            lir.LirBlock(at=0, insns=(branch,), succ=(1, 2)),
            lir.LirBlock(at=1, insns=(use_source,), succ=()),
            lir.LirBlock(at=2, insns=(use_result,), succ=(), phis=(lir.Phi(3, ((0, 1), (4, 5))),)),
            lir.LirBlock(at=4, insns=(), succ=(2,)),
        ),
        origin={},
        pins={},
    )

    done = phielim.eliminated(body)

    assert len(done.blocks) == len(body.blocks) + 1
    edge = done.blocks[-1]
    assert edge.insns[0].group is not None
    assert edge.insns[0].defines == (3,)
    assert edge.insns[0].uses == (1,)


def test_phi_observation_on_an_immediate_alternate_edge_is_visible() -> None:
    """nbody's latch passed an accumulator to phis on both of its edges.

    A phi consumes its incoming value before its block's first instruction,
    so an edge walk that only inspects instructions misses the immediate
    alternate successor entirely.
    """
    body = lir.LirBody(
        name="alternate-phi",
        entry=0,
        blocks=(
            lir.LirBlock(at=0, insns=(), succ=(1, 2)),
            lir.LirBlock(at=1, insns=(), succ=(), phis=(lir.Phi(3, ((0, 7),)),)),
            lir.LirBlock(at=2, insns=(), succ=()),
        ),
        origin={},
        pins={},
    )
    at_of = {block.at: block for block in body.blocks}
    assert phielim._observed(body, at_of, 0, 2, 7)


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
