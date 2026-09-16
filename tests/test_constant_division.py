"""Constant division must disappear without losing answers or original bytes."""

from pathlib import Path

import pytest

import corpus
from qbopt.model import ir
from qbopt.model import mir
from qbopt.analysis import consts
from qbopt import wholeseg


@pytest.mark.parametrize(
    "dividend,divisor,expected",
    [
        (100000, 7, (14285, 5)),
        (-17, 5, (-3, -2)),
        (17, -5, (-3, 2)),
        (-17, -5, (3, -2)),
        (0, 3, (0, 0)),
        (1, 0, None),
        (-2147483648, -1, None),
    ],
)
def test_signed_division(dividend: int, divisor: int, expected: tuple[int, int] | None) -> None:
    op = mir.Op(
        0,
        ir.Operation.MOVE,
        "",
        (),
        (),
        kind=mir.Kind.DIVMOD,
        args=(mir.Const(dividend, 4), mir.Const(divisor, 4)),
        results=(mir.Held(mir.Value(1, 0), 4), mir.Held(mir.Value(2, 1), 4)),
    )
    assert consts.division(op, {}, {}) == (
        tuple(number & 0xFFFFFFFF for number in expected) if expected is not None else None
    )


@pytest.mark.parametrize(
    ("kind", "dividend", "divisor", "expected"),
    [
        (mir.Kind.DIVMOD, -(2**63) + 17, 5, (-1844674407370955158, -1)),
        (mir.Kind.UDIVMOD, 2**64 - 1, 7, (2635249153387078802, 1)),
    ],
)
def test_int64_division_never_rounds_through_binary64(kind, dividend, divisor, expected) -> None:
    """A float intermediary changed the quotient once an integer exceeded 53 bits."""
    op = mir.Op(
        0,
        ir.Operation.NOTHING,
        "",
        (),
        (),
        kind=kind,
        args=(mir.Const(dividend, 8), mir.Const(divisor, 8)),
        results=(mir.Held(mir.Value(1, 0), 8), mir.Held(mir.Value(2, 0), 8)),
    )
    assert consts.division(op, {}, {}) == tuple(number & ((1 << 64) - 1) for number in expected)


def test_lngmix_emits_without_constant_divides() -> None:
    """LNGMIX retained both constant divides; deleting them then lost 12 push bytes."""
    result = wholeseg.emitted(Path("fixtures/omf/lngmix-p-g2.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    instructions = [one.insn for block in corpus.partitioned(result.data) for one in block.insns]
    assert instructions
    assert not any(str(one).startswith("idiv ") for one in instructions)
    assert not {"B$DVI4", "B$RMI4"}.intersection(corpus.loaded(result.data).calls.values())
