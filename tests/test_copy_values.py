from dataclasses import replace

import pytest

from qbopt.model import ir
from qbopt.model import mir
from qbopt.optimize import transform


def test_cse_does_not_confuse_word_and_dword_sign_extensions() -> None:
    """A byte -1 extends to 0xffff or 0xffffffff, not one interchangeable value."""
    source = mir.Value(1, 0, variable=1)
    word = mir.Value(2, 2, variable=2)
    dword = mir.Value(3, 5, variable=3)
    define = mir.Op(0, ir.Operation.MOVE, "mov", (source,), (), kind=mir.Kind.COPY,
                    args=(mir.Const(255, 1),), results=(mir.Held(source, 1),), covers=(0, 2))
    first = mir.Op(2, ir.Operation.EXTEND, "movsx", (word,), (source,), kind=mir.Kind.CONVERT,
                   args=(mir.Held(source, 1),), results=(mir.Held(word, 2),), covers=(2, 5))
    second = replace(first, at=5, defines=(dword,), results=(mir.Held(dword, 4),), covers=(5, 9))
    use = mir.Op(9, ir.Operation.PUSH, "push", (), (dword,), kind=mir.Kind.ARG,
                 args=(mir.Held(dword, 4),))
    body = mir.MirBody(0, (mir.MirBlock(0, (), (define, first, second, use), ()),))
    after = transform.subexpressions(body)
    assert after.blocks[0].ops[-1].args == (mir.Held(dword, 4),)
    assert any(dword in op.defines for op in after.blocks[0].ops)


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
