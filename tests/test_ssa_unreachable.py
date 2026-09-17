"""SSA reconstruction must retain byte owners outside the reachable graph."""

from qbopt.analysis import ssa
from qbopt.model import ir, mir


def test_reconstruction_matches_blocks_by_identity_not_position():
    """FPDEEP crashed in SSA repair after CFG cleanup left an unreachable byte owner."""
    value = mir.Value(1, 0, variable=1, version=1)
    define = mir.Op(0, ir.Operation.MOVE, "mov", (value,), (), kind=mir.Kind.COPY,
                    args=(mir.Const(7, 2),), results=(mir.Held(value, 2),))
    dead = mir.Op(5, ir.Operation.NOTHING, "", (), (), kind=mir.Kind.NOTHING)
    read = mir.Op(10, ir.Operation.PUSH, "push", (), (value,), kind=mir.Kind.ARG,
                  args=(mir.Held(value, 2),))
    body = mir.MirBody(0, (mir.MirBlock(0, (), (define,), (10,)),
                          mir.MirBlock(5, (), (dead,), ()), mir.MirBlock(10, (), (read,), ())))
    result = ssa.constructed(body, frozenset({1}))
    assert [block.at for block in result.blocks] == [0, 5, 10]
    assert result.block(5) == body.block(5)
    assert result.block(10).ops[0].uses == result.block(0).ops[0].defines
    assert result.block(10).ops[0].args[0].value == result.block(0).ops[0].defines[0]
