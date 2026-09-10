import pytest
from itertools import count
from types import SimpleNamespace

from qbopt.model import ir, mir
from qbopt.backend import lower


@pytest.mark.parametrize("width", [2, 4])
@pytest.mark.parametrize("scale", [2, 8, 32768])
@pytest.mark.parametrize("preserve", [False, True])
def test_power_of_two_product_selection(width: int, scale: int, preserve: bool) -> None:
    """NESTED and HARR emitted IMUL for doubled addresses even with dead flags."""
    source, result = mir.Value(1, 0), mir.Value(2, 1)
    op = mir.Op(
        10, ir.Operation.MULTIPLY, "imul", (result,), (source,),
        kind=mir.Kind.MUL,
        args=(mir.Held(source, width), mir.Const(scale, width)),
        results=(mir.Held(result, width),),
        covers=(10, 14),
    )
    body = mir.MirBody(10, (mir.MirBlock(10, (), (op,), ()),))
    (instruction,) = lower.Lowering(body, {1, 2}, {}, ()).expand(op, preserve_flags=preserve)
    assert instruction.what.name == ("imul" if preserve else "shl")
    assert instruction.defines == (2,)
    assert set(instruction.uses) == {1}
    if not preserve:
        assert instruction.what.sources == (ir.Held(1, width), ir.Imm(scale.bit_length() - 1, 1))


@pytest.mark.parametrize("scale", [0, 1, -2, 65536])
def test_non_shift_multipliers_are_not_reinterpreted(scale: int) -> None:
    """Identity and out-of-range products do not become arithmetic chains."""
    op = mir.Op(
        10, ir.Operation.MULTIPLY, "imul", (), (), kind=mir.Kind.MUL,
        args=(mir.Held(mir.Value(1, 0), 2), mir.Const(scale, 2)),
        results=(mir.Held(mir.Value(2, 1), 2),),
    )
    assert lower._scaled(op, SimpleNamespace(fresh=count(1000).__next__)) is None
