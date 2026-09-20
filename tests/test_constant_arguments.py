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
    result = wholeseg.emitted(Path(f"fixtures/omf/nots-{tag}.obj".lower()).read_bytes())
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


def test_pressx_read_destinations_remain_relocated_after_argument_folding():
    """PRESSX /G3 printed 0 instead of 7500 when READ was passed address zero.

    Each B$RDI2 call receives the offset of one input variable.  Check the
    relocated operand at the call site, rather than the literal zero stored
    in an object before LINK applies that relocation.
    """
    data = Path("fixtures/omf/pressx-v-g3.obj").read_bytes()
    result = wholeseg.emitted(data)
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    found = module.of(omf.parse(result.data))
    insns = blocks.instructions(found)
    fixups = [one for one in omf.fixups(omf.parse(result.data)) if one.seg == found.seg]

    destinations = []
    for index, call in enumerate(insns):
        if found.calls.get(call.at) != "B$RDI2":
            continue
        argument = insns[index - 1]
        owned = [one for one in fixups if argument.at <= one.offset < call.at]
        assert len(owned) == 1, f"READ argument at {argument.at:#x} owns {owned}"
        destinations.append((owned[0].target, owned[0].index, owned[0].disp))

    assert destinations == [("segment", 5, offset) for offset in range(6, 22, 2)]


def test_divmod_error_handler_argument_keeps_its_original_code_fixup():
    """DIVMOD /G3 was refused when its error-handler offset became a data reference.

    B$OEGA receives a CS-relative handler offset.  Folding that offset into
    the push must move its original fixup, including its segment frame; a
    newly invented DGROUP-framed reference means something different.
    """
    data = Path("fixtures/omf/divmod-v-g3.obj").read_bytes()
    result = wholeseg.emitted(data)
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    found = module.of(omf.parse(result.data))
    insns = blocks.instructions(found)
    setup = next(index for index, one in enumerate(insns) if found.calls.get(one.at) == "B$OEGA")
    handler = next(one.at for one in insns if found.calls.get(one.at) == "B$FERR")
    argument, call = insns[setup - 1], insns[setup]
    owned = [
        one
        for one in omf.fixups(omf.parse(result.data))
        if one.seg == found.seg and argument.at <= one.offset < call.at
    ]
    assert len(owned) == 1
    assert (owned[0].target, owned[0].index, owned[0].disp) == ("segment", found.seg, handler)
