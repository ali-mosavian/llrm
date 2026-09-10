"""Propagate a store's scalar value without deleting its memory observation."""

from pathlib import Path

import pytest
import corpus
from iced_x86 import Code, Register

from qbopt import wholeseg
from qbopt.model import ir, mir
from qbopt.optimize import transform


@pytest.mark.parametrize("tag", ["p-g2", "v-g3"])
def test_fpcse_initial_counter_store_does_not_need_a_dead_register(tag):
    """FPCSE retained MOV AX,1 solely to initialize i before the required WAIT."""
    result = wholeseg.emitted(Path(f"fixtures/omf/fpcse-{tag}.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    instructions = [one.insn for block in corpus.partitioned(result.data) for one in block.insns]
    assert not any(one.code == Code.MOV_R16_IMM16 and one.op0_register == Register.AX
                   and one.immediate16 == 1 for one in instructions)
    store = next(index for index, one in enumerate(instructions)
                 if one.code == Code.MOV_RM16_IMM16 and one.immediate16 == 1)
    assert instructions[store + 1].code == Code.WAIT


@pytest.mark.parametrize("width", [None, 2, 4])
@pytest.mark.parametrize("address_uses_value", [False, True])
def test_constant_store_requires_full_width_and_retains_address_uses(width, address_uses_value):
    """Replacing a stored value must not lose a same-valued address or invent its high half."""
    value = mir.Value(1, 0)
    ref = mir.MemRef(None, 4, base=value if address_uses_value else None)
    op = mir.Op(0, ir.Operation.MOVE, "mov", (), (value,), stores=(ref,),
                args=(mir.Held(value, 4),), results=(mir.Cell(ref),), kind=mir.Kind.STORE)
    facts = {} if width is None else {value: mir.Const(-1, width)}
    result = transform._constant_operands(op, facts)
    if width != 4:
        assert result is op
    else:
        assert result.args == (mir.Const(0xffffffff, 4),)
        assert result.stores == op.stores and result.results == op.results
        assert result.uses == ((value,) if address_uses_value else ())
