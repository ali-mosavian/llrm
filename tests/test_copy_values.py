from dataclasses import replace

import pytest

from qbopt.model import ir
from qbopt.model import mir
from qbopt.optimize import transform


@pytest.mark.parametrize("read_width", [2, 4])
def test_copy_elimination_preserves_observed_upper_bits(read_width: int) -> None:
    """HARR carries redundant low-word copies; a wide reader must still retain the preserved upper word."""
    source, result, previous = mir.Value(10, 0), mir.Value(11, 2), mir.Value(12, 0)
    define = mir.Op(
        0,
        ir.Operation.MOVE,
        "mov",
        (source,),
        (),
        kind=mir.Kind.COPY,
        args=(mir.Const(7, 2),),
        results=(mir.Held(source, 2),),
        covers=(0, 2),
    )
    copy = replace(
        define,
        at=2,
        defines=(result,),
        uses=(source, previous),
        args=(mir.Held(source, 2),),
        results=(mir.Held(result, 2),),
        merges={previous: result},
        covers=(2, 4),
    )
    use = mir.Op(4, ir.Operation.PUSH, "push", (), (result,), kind=mir.Kind.ARG, args=(mir.Held(result, read_width),))
    body = mir.MirBody(0, (mir.MirBlock(0, (), (define, copy, use), ()),))
    after = transform.subexpressions(body)
    if read_width == 2:
        assert all(result not in op.defines for block in after.blocks for op in block.ops)
        assert after.blocks[0].ops[-1].args == (mir.Held(source, 2),)
    else:
        assert after == body
