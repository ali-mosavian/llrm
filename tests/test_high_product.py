"""LNGMXX's division by seven needs the signed high product, not truncated MUL."""
import pytest
from dataclasses import replace
from iced_x86 import Register

from qbopt.analysis import consts
from qbopt.backend import lower, target
from qbopt.model import ir, mir


def product(width, first, second):
    result = mir.Value(901, 0)
    return mir.Op(0, ir.Operation.MULTIPLY, "", (result,), (), kind=mir.Kind.SMULHI,
                  args=(mir.Const(first, width), mir.Const(second, width)),
                  results=(mir.Held(result, width),))


@pytest.mark.parametrize("width", [2, 4])
def test_signed_high_product_constants(width):
    bits = width * 8
    mask = (1 << bits) - 1
    for first in (0, 1, -1, -(1 << (bits - 1)), (1 << (bits - 1)) - 1):
        for second in (0, 7, -7, -(1 << (bits - 1))):
            op = product(width, first, second)
            body = mir.MirBody(0, (mir.MirBlock(0, (), (op,), ()),))
            assert consts.known(body)[op.results[0].value].n == ((first * second) >> bits) & mask


@pytest.mark.parametrize("width", [2, 4])
def test_high_product_lowering_retains_low_result_clobber(width):
    op = product(width, -7, 11)
    body = mir.MirBody(0, (mir.MirBlock(0, (), (op,), ()),))
    parts = lower.Lowering(body, {901}, {}, {}, {}).expand(op)
    multiply = parts[-1].what
    assert multiply.name == "imul"
    assert len(multiply.dests) == 2
    assert multiply.dests[1] == ir.Held(901, width)
    assert multiply.dests[0] != multiply.dests[1]
    assert all(isinstance(arg, ir.Held) for arg in multiply.sources)
    assert all(arg.width == width for arg in (*multiply.sources, *multiply.dests))
    requirements = target.requirements(multiply)
    assert requirements[target.Occurrence("dest", 0)] == Register.EAX
    assert requirements[target.Occurrence("dest", 1)] == Register.EDX
    assert requirements[target.Occurrence("source", 0)] == Register.EAX


def test_high_product_cannot_cross_a_live_condition():
    flag = mir.Value(902, 0, flags=True)
    with pytest.raises(lower.Unlowered, match="live condition"):
        lower._check_inserted_conditions((product(4, -7, 11),), frozenset({flag}))


@pytest.mark.parametrize("width", [1, 8])
def test_high_product_rejects_unsupported_width(width):
    op = product(width, -7, 11)
    body = mir.MirBody(0, (mir.MirBlock(0, (), (op,), ()),))
    assert op.results[0].value not in consts.known(body)
    with pytest.raises(lower.Unlowered, match="signed high product"):
        lower.Lowering(body, {901}, {}, {}, {}).expand(op)


def test_high_product_does_not_infer_a_mixed_width_result():
    op = replace(product(4, -7, 11), args=(mir.Const(-7, 2), mir.Const(11, 4)))
    body = mir.MirBody(0, (mir.MirBlock(0, (), (op,), ()),))
    assert op.results[0].value not in consts.known(body)
    with pytest.raises(lower.Unlowered, match="signed high product"):
        lower.Lowering(body, {901}, {}, {}, {}).expand(op)
