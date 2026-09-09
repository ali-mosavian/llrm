"""Stack allocation needs encodings BC does not itself select."""

import pytest
from iced_x86 import Decoder

from qbopt.model import ir
from qbopt.backend import select


@pytest.mark.parametrize("name,base", [("fadd", 0xc0), ("fmul", 0xc8),
    ("fsub", 0xe0), ("fsubr", 0xe8), ("fdiv", 0xf0), ("fdivr", 0xf8)])
@pytest.mark.parametrize("index", [1, 3, 7])
@pytest.mark.parametrize("top", [True, False])
def test_register_arithmetic_preserves_operand_direction(name, base, index, top):
    """Shared FPCSE values need non-popping arithmetic, not another memory load."""
    dest, source = (ir.St(0), ir.St(index)) if top else (ir.St(index), ir.St(0))
    what = ir.Semantics(ir.Operation.FLOAT_ARITH, name, (dest,), (dest, source))
    result = select.emit(what)
    assert result is not None
    # The DC forms reverse the subtraction/division opcode sense, not operands.
    byte = base if top or name in ("fadd", "fmul") else base ^ 8
    assert result.code == bytes((0xd8 if top else 0xdc, byte + index))
    instruction = next(iter(Decoder(16, result.code)))
    assert str(instruction).startswith(name + " ")


@pytest.mark.parametrize("index", [0, 1, 7])
def test_stack_duplicate_and_exchange(index):
    """A live floating producer needs duplication or exchange, not recomputation."""
    load = select.emit(ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (ir.St(0),), (ir.St(index),)))
    exchange = select.emit(ir.Semantics(ir.Operation.EXCHANGE, "fxch",
        (ir.St(0), ir.St(index)), (ir.St(0), ir.St(index))))
    assert load is not None and load.code == bytes((0xd9, 0xc0 + index))
    assert exchange is not None and exchange.code == bytes((0xd9, 0xc8 + index))


@pytest.mark.parametrize("dest,source", [(1, 2), (-1, 0), (0, 8)])
def test_unencodable_stack_arithmetic_is_refused(dest, source):
    what = ir.Semantics(ir.Operation.FLOAT_ARITH, "fsub", (ir.St(dest),), (ir.St(dest), ir.St(source)))
    assert select.emit(what) is None
