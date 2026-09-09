"""Known scalar arguments should not be read back from memory for a call."""

from pathlib import Path

import pytest
from iced_x86 import Mnemonic, OpKind

from qbopt import wholeseg
from qbopt.frontend import blocks
from qbopt.objectfile import module, omf


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_nots_prints_its_known_first_result_without_reloading(tag):
    """NOTS stored EDCBA987h then reloaded both words for PRINT instead of passing constants."""
    result = wholeseg.emitted(Path(f"fixtures/omf/nots-{tag}.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    found = module.of(omf.parse(result.data))
    insns = blocks.instructions(found)
    index = next(index for index, one in enumerate(insns) if found.calls.get(one.at) == "B$PEI4")
    arg = insns[index - 1].insn
    assert arg.mnemonic == Mnemonic.PUSH
    assert arg.op0_kind in (OpKind.IMMEDIATE8TO16, OpKind.IMMEDIATE16, OpKind.IMMEDIATE32, OpKind.IMMEDIATE8TO32)


@pytest.mark.parametrize("known_width", [1, 2, 4])
def test_argument_folding_requires_every_byte_and_keeps_stack_write(known_width):
    """A partial word fact must not invent the rest of a long argument."""
    from qbopt.analysis import consts
    from qbopt.model import ir, mir
    from qbopt.optimize import transform
    ref = mir.MemRef(module.Addr(module.Space.SEGMENT, 6, 5), 4)
    stack = mir.MemRef(None, 4, space=module.Space.STACK)
    op = mir.Op(0, ir.Operation.PUSH, "push", (), (), kind=mir.Kind.ARG,
                args=(mir.Cell(ref),), loads=(ref,), stores=(stack,))
    memory = consts._fragments(ref, consts.Known(0x12345678, known_width))
    result = transform._constant_argument(op, {}, memory)
    if known_width < 4:
        assert result is op
    else:
        assert result.args == (mir.Const(0x12345678, 4),)
        assert result.stores == (stack,) and result.loads == ()
