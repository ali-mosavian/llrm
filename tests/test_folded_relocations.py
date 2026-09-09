"""Numeric constants folded from loads must not inherit the load's fixup."""

from pathlib import Path

import pytest
from iced_x86 import Code

from qbopt import wholeseg
from qbopt.frontend import blocks
from qbopt.objectfile import module, omf


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_arith_folded_constant_has_no_data_address_fixup(tag):
    """ARITH's MOV ESI,12345678h retained a relocation to a, corrupting its low word at LINK."""
    result = wholeseg.emitted(Path(f"fixtures/omf/arith-{tag}.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    found = module.of(omf.parse(result.data))
    constants = [one for one in blocks.instructions(found)
                 if one.insn.code == Code.MOV_R32_IMM32 and one.insn.immediate32 == 0x12345678]
    assert constants
    fixups = [one for one in omf.fixups(found.records) if one.seg == found.seg]
    assert not any(insn.imm_at <= fixup.offset < insn.imm_at + insn.imm_len
                   for insn in constants for fixup in fixups)
