"""HUGELP refused after LICM exposed SUB reg,0xfffffffe (the lower bound -2)."""

import pytest
from iced_x86 import Register

from qbopt.backend import select
from qbopt.model import ir
from qbopt.objectfile.module import Addr, Space


@pytest.mark.parametrize("number", [0xfffffffe, 0x80000000])
@pytest.mark.parametrize("name", ["add", "sub"])
@pytest.mark.parametrize("memory", [False, True])
def test_arithmetic_encodes_the_same_32_bit_pattern_in_either_signed_notation(number, name, memory):
    operand = ir.Mem(Addr(Space.FRAME, -4), 4) if memory else Register.EDX
    encode = select.arith_into_imm if memory else select.arith_imm
    positive = encode(name, operand, number)
    negative = encode(name, operand, number - (1 << 32))
    assert positive is not None and negative is not None
    assert positive.code == negative.code
