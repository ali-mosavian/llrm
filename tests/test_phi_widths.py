"""Phi widths must be proven before copy propagation crosses a loop header."""

import pytest

from qbopt import ir
from qbopt import mir
from qbopt import transform


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
