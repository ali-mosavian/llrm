"""Constant arithmetic chains must preserve carry and demanded widths."""

from dataclasses import replace

import pytest

from qbopt.model import ir
from qbopt.model import mir
from qbopt.analysis import consts
from qbopt.optimize import transform


@pytest.mark.parametrize("left,right,expected", [(65535, 1, 6), (3, 4, 5)])
def test_constant_add_carry(left: int, right: int, expected: int) -> None:
    """LNGMIX kept a constant low ADD and high ADC because carry was unknown."""
    low, flags, high = mir.Value(1, 0), mir.Value(2, 0, flags=True), mir.Value(3, 1)
    ops = (
        mir.Op(
            0,
            ir.Operation.BINARY,
            "add",
            (low, flags),
            (),
            kind=mir.Kind.ADD,
            args=(mir.Const(left, 2), mir.Const(right, 2)),
            results=(mir.Held(low, 2),),
        ),
        mir.Op(
            1,
            ir.Operation.BINARY,
            "adc",
            (high,),
            (flags,),
            kind=mir.Kind.ADD_CARRY,
            args=(mir.Const(2, 2), mir.Const(3, 2)),
            results=(mir.Held(high, 2),),
        ),
    )
    body = mir.MirBody(0, (mir.MirBlock(0, (), ops, ()),))
    facts = consts.known(body)
    assert facts.get(high) == consts.Known(expected, 2)
    assert flags not in facts, "a carry fact is not a complete condition-code value"


def test_unknown_carry_does_not_fold() -> None:
    flags, result = mir.Value(1, 0, flags=True), mir.Value(2, 1)
    op = mir.Op(
        1,
        ir.Operation.BINARY,
        "adc",
        (result,),
        (flags,),
        kind=mir.Kind.ADD_CARRY,
        args=(mir.Const(2, 2), mir.Const(3, 2)),
        results=(mir.Held(result, 2),),
    )
    body = mir.MirBody(0, (mir.MirBlock(0, (), (op,), ()),))
    assert result not in consts.known(body)


@pytest.mark.parametrize("width,expected", [(4, consts.Known(0x1234, 2)), (2, None)])
def test_extract_requires_all_requested_bits(width: int, expected: consts.Known | None) -> None:
    source, result = mir.Value(1, 0), mir.Value(2, 1)
    op = mir.Op(
        1,
        ir.Operation.RESTORE,
        "extract",
        (result,),
        (source,),
        kind=mir.Kind.EXTRACT,
        args=(mir.Held(source, 4), mir.Const(16, 4)),
        results=(mir.Held(result, 2),),
    )
    assert consts._result(op, {source: consts.Known(0x12345678, width)}) == expected


def test_partial_constant_does_not_replace_a_wide_result() -> None:
    """A known low half must not turn an unknown high half into zero."""
    result = mir.Value(1, 0)
    op = mir.Op(
        0,
        ir.Operation.BINARY,
        "add",
        (result,),
        (),
        kind=mir.Kind.ADD,
        args=(mir.Const(1, 4), mir.Const(2, 4)),
        results=(mir.Held(result, 4),),
        covers=(0, 4),
    )
    assert transform._folded_op(op, {result: consts.Known(3, 2)}, {result}) == op
    memory = replace(op, results=(mir.Cell(mir.MemRef(None, 4)),))
    assert consts._carry(memory, {}, {}) is None
