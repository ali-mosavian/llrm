from pathlib import Path
from dataclasses import replace

import pytest

from iced_x86 import Register

from qbopt import ir, lir, peephole


def test_nbody_repeated_fixed_constant_is_removed():
    """Nbody materialized 512 twice before one divide, with a non-clobbering CDQ between them."""
    from qbopt import wholeseg, module, omf, blocks
    from iced_x86 import Code
    result = wholeseg.emitted(Path("fixtures/regressions/nbody-stack-p-g2.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    found = module.of(omf.parse(result.data))
    assert sum(one.insn.code == Code.MOV_R32_IMM32 and one.insn.immediate32 == 512
               for one in blocks.instructions(found)) == 1


def test_partial_write_invalidates_constant():
    def move(at, dest, source):
        return lir.Insn(at=at, covers=(at, at+1), defines=(), uses=(),
                        what=ir.Semantics(ir.Operation.MOVE, "mov", (dest,), (source,)))
    first = move(0, ir.Reg(Register.EAX, 4), ir.Imm(512, 4))
    change = move(1, ir.Reg(Register.AH, 1), ir.Imm(0, 1))
    again = move(2, ir.Reg(Register.EAX, 4), ir.Imm(512, 4))
    body = lir.LirBody("partial", 0, (lir.LirBlock(0, (first, change, again), ()),), {}, {})
    assert len(peephole.constants(body).blocks[0].insns) == 3


@pytest.mark.parametrize("interruption", ["none", "extend", "extend_write", "extend_clobber", "call", "clobber", "unknown", "relocation", "block"])
def test_constant_knowledge_is_local_and_invalidated(interruption):
    from qbopt.module import Addr, Space
    source = ir.Imm(512, 4)
    if interruption == "relocation":
        source = ir.Imm(512, 4, Addr(Space.SEGMENT, 0, 5))
    what = ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg(Register.EAX, 4),), (source,))
    first = lir.Insn(0, (0, 1), what, (), ())
    last = replace(first, at=2, covers=(2, 3))
    middle = lir.Insn(1, (1, 2), ir.Semantics(ir.Operation.MOVE, "mov",
                     (ir.Reg(Register.BX, 2),), (ir.Imm(7, 2),)), (), ())
    if interruption == "call":
        middle = replace(middle, what=ir.Semantics(ir.Operation.CALL, "call", (), ()))
    if interruption in ("extend", "extend_clobber"):
        middle = replace(middle, what=ir.Semantics(ir.Operation.EXTEND, "cdq",
                         (ir.Reg(Register.EDX, 4),), (ir.Reg(Register.EAX, 4),)))
        if interruption == "extend_clobber":
            middle = replace(middle, clobbers=frozenset({Register.AH}))
    if interruption == "extend_write":
        middle = replace(middle, what=ir.Semantics(ir.Operation.EXTEND, "movsx",
                         (ir.Reg(Register.EAX, 4),), (ir.Reg(Register.AX, 2),)))
    if interruption == "clobber":
        middle = replace(middle, clobbers=frozenset({Register.EAX}))
    if interruption == "unknown":
        middle = replace(middle, what=None)
    blocks = (lir.LirBlock(0, (first, middle, last), ()),)
    if interruption == "block":
        blocks = (lir.LirBlock(0, (first, middle), (2,)), lir.LirBlock(2, (last,), ()))
    result = peephole.constants(lir.LirBody("constants", 0, blocks, {}, {}))
    assert sum(len(block.insns) for block in result.blocks) == (2 if interruption in ("none", "extend") else 3)
