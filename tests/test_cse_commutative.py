"""Commutativity numbers expressions without changing their evaluation order."""

from dataclasses import replace

import pytest

from qbopt.model import ir, mir
from qbopt.optimize import transform


@pytest.mark.parametrize("kind,name,commutes", [
    (mir.Kind.ADD, "add", True), (mir.Kind.MUL, "imul", True),
    (mir.Kind.AND, "and", True), (mir.Kind.OR, "or", True),
    (mir.Kind.XOR, "xor", True), (mir.Kind.EQ, "eq", True),
    (mir.Kind.NE, "ne", True), (mir.Kind.SUB, "sub", False),
    (mir.Kind.DIV, "idiv", False), (mir.Kind.SHL, "shl", False),
    (mir.Kind.LT, "lt", False),
    (mir.Kind.PTR_OFFSET, "ptr_offset", False),
])
def test_value_numbering_recognizes_only_commutative_integer_expressions(kind, name, commutes):
    """Reversed operands unnecessarily retained a second integer computation."""
    left, right, first, second = (mir.Value(index, index, variable=index) for index in range(1, 5))
    original = mir.Op(10, ir.Operation.BINARY, name, (first,), (left, right), kind=kind,
                      args=(mir.Held(left, 4), mir.Held(right, 4)), results=(mir.Held(first, 4),))
    reversed_op = replace(original, defines=(second,), args=tuple(reversed(original.args)),
                          results=(mir.Held(second, 4),))
    widths = {value.id: 4 for value in (left, right, first, second)}
    assert (transform._computation(original, {}, widths)
            == transform._computation(reversed_op, {}, widths)) is commutes
    assert original.args == (mir.Held(left, 4), mir.Held(right, 4))


@pytest.mark.parametrize("reads_flags", [False, True])
def test_reversed_addition_reuses_value_only_when_flags_are_unobserved(reads_flags):
    """Commutative numbering must not remove a later instruction's observed flags."""
    left, right, first, second = (mir.Value(index, index, variable=index) for index in range(1, 5))
    flags = mir.Value(5, 12, flags=True)
    definitions = tuple(mir.Op(at, ir.Operation.MOVE, "mov", (value,), (), kind=mir.Kind.COPY,
                               args=(mir.Const(number, 4),), results=(mir.Held(value, 4),))
                        for at, value, number in ((0, left, 7), (5, right, 9)))
    original = mir.Op(10, ir.Operation.BINARY, "add", (first,), (left, right), kind=mir.Kind.ADD,
                      args=(mir.Held(left, 4), mir.Held(right, 4)), results=(mir.Held(first, 4),))
    other = replace(original, at=12, defines=(second, flags), args=tuple(reversed(original.args)),
                    results=(mir.Held(second, 4),))
    original = replace(original, defines=(first, mir.Value(6, 10, flags=True)))
    use = mir.Op(14, ir.Operation.PUSH, "push", (), (second, flags) if reads_flags else (second,),
                 kind=mir.Kind.ARG, args=(mir.Held(second, 4),))
    body = mir.MirBody(0, (mir.MirBlock(0, (), (*definitions, original, other, use), ()),))
    after = transform.subexpressions(body)
    assert after.blocks[0].ops[-1].args == (mir.Held(second if reads_flags else first, 4),)


def test_strict_floating_operand_order_is_not_canonicalized():
    from qbopt.model.floating import Format, Precision, Rounding, Semantics
    left, right, result = (mir.Value(index, index, variable=index) for index in range(1, 4))
    rule = Semantics((Format.EXTENDED80, Format.EXTENDED80), Format.EXTENDED80,
                     Precision.DYNAMIC, Rounding.DYNAMIC)
    op = mir.Op(0, ir.Operation.FLOAT_ARITH_POP, "faddp", (result,), (left, right),
                kind=mir.Kind.FADD, args=(mir.Held(left, 10), mir.Held(right, 10)),
                results=(mir.Held(result, 10),), floating=rule)
    widths = {value.id: 10 for value in (left, right, result)}
    assert transform._computation(op, {}, widths) != transform._computation(
        replace(op, args=tuple(reversed(op.args))), {}, widths)
