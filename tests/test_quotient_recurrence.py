from pathlib import Path

import corpus
import pytest

from qbopt import induction, ir, loops, mir, transform


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_stride_quotient_advances_without_division(tag: str) -> None:
    """STRIDE executed i / 5 on every iteration although its answers are 0, 1, ... 20."""
    path = Path(f"fixtures/omf/stride-{tag}.obj")
    module = corpus.loaded(path)
    partition = corpus.partitioned(path)
    body = mir.bodies(module, partition)[0][1]
    assert any(op.kind is mir.Kind.DIVMOD for block in body.blocks for op in block.ops)
    result = transform.applied(body, module.dgroup, module.calls, blocks=partition, found=module)
    assert not any(op.kind in (mir.Kind.DIV, mir.Kind.DIVMOD) for block in result.blocks for op in block.ops)


@pytest.mark.parametrize("start,step,bound,divisor,accepted", [
    (0, 5, 100, 5, True),
    (-100, 5, 0, 5, True),
    (100, -5, 0, 5, True),
    (0, 5, 100, -5, True),
    (1, 5, 101, 5, False),
    (0, 3, 100, 5, False),
    (32760, 5, 32767, 5, False),
    (-32760, -5, -32768, 5, False),
    (-32768, 1, -32760, -1, False),
    (0, 5, 100, 0, False),
])
def test_quotient_recurrence_requires_exact_nonwrapping_division(start, step, bound, divisor, accepted) -> None:
    """A wrapping dividend or rounded quotient is not the arithmetic progression STRIDE uses."""
    counter, flags, quotient, remainder = (mir.Value(n, 10, flags=n == 2) for n in range(1, 5))
    compare = mir.Op(
        10, ir.Operation.COMPARE, "cmp", (flags,), (counter,), kind=mir.Kind.SUB,
        args=(mir.Held(counter, 2), mir.Const(bound, 2)),
    )
    branch = mir.Op(
        11, ir.Operation.BRANCH, "", (), (flags,), kind=mir.Kind.BRANCH,
        target=20, test=mir.Kind.LE if step > 0 else mir.Kind.GE,
    )
    divide = mir.Op(
        20, ir.Operation.DIVIDE, "", (quotient, remainder), (counter,), kind=mir.Kind.DIVMOD,
        args=(mir.Held(counter, 2), mir.Const(divisor, 2)),
        results=(mir.Held(quotient, 2), mir.Held(remainder, 2)),
    )
    body = mir.MirBody(10, (
        mir.MirBlock(10, (), (compare, branch), (20, 30)),
        mir.MirBlock(20, (), (divide,), (10,)),
        mir.MirBlock(30, (), (), ()),
    ))
    loop = loops.Loop(10, frozenset({20}), frozenset({10, 20}))
    affine = induction.Affine(counter.id, mir.Const(start, 2), mir.Const(step, 2), 10)
    result = induction._quotients(body, loop, {counter.id: affine})
    assert bool(result) is accepted
    if accepted:
        assert result[0].of.start == mir.Const(start // divisor, 2)
        assert result[0].of.step == mir.Const(step // divisor, 2)
