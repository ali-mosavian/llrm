from pathlib import Path
from dataclasses import replace

import pytest

from iced_x86 import Register

from qbopt.model import ir, lir
from qbopt.backend import peephole


@pytest.mark.parametrize("change", [None, Register.CH, Register.AH])
def test_repeated_copy_requires_unchanged_source_and_destination(change):
    """LNGMXX copied ECX into EAX twice around CDQ; partial writes must prevent reuse."""
    move = lir.Insn(0, (0, 1), ir.Semantics(ir.Operation.MOVE, "mov",
                    (ir.Reg(Register.EAX, 4),), (ir.Reg(Register.ECX, 4),)), (), ())
    extend = lir.Insn(1, (1, 2), ir.Semantics(ir.Operation.EXTEND, "cdq",
                      (ir.Reg(Register.EDX, 4),), (ir.Reg(Register.EAX, 4),)), (), ())
    if change is not None:
        extend = replace(extend, clobbers=frozenset({change}))
    final = replace(move, at=2, covers=(2, 3))
    body = lir.LirBody("copies", 0, (lir.LirBlock(0, (move, extend, final), ()),), {}, {})
    result = peephole.constants(body)
    assert len(result.insns) == (2 if change is None else 3)


def test_copied_value_survives_overwriting_its_original_register():
    """A copied value is a snapshot, not an alias of the register it came from."""
    def copy(at, dest, source):
        return lir.Insn(at, (at, at + 1), ir.Semantics(ir.Operation.MOVE, "mov",
                        (ir.Reg(dest, 4),), (source,)), (), ())
    insns = (
        copy(0, Register.EAX, ir.Reg(Register.ECX, 4)),
        copy(1, Register.EDX, ir.Reg(Register.ECX, 4)),
        copy(2, Register.ECX, ir.Imm(7, 4)),
        copy(3, Register.EAX, ir.Reg(Register.EDX, 4)),
        copy(4, Register.EAX, ir.Reg(Register.ECX, 4)),
    )
    body = lir.LirBody("snapshot", 0, (lir.LirBlock(0, insns, ()),), {}, {})
    result = peephole.constants(body)
    assert [one.what for one in result.insns] == [one.what for one in insns if one.at != 3]


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_lngmxx_does_not_reload_dividend_after_sign_extension(tag):
    """LNGMXX's CDQ leaves its dividend intact, but lowering reloaded it before IDIV."""
    import corpus
    from iced_x86 import Mnemonic
    from qbopt import wholeseg
    result = wholeseg.emitted(Path(f"fixtures/omf/lngmxx-{tag}.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    insns = [one.insn for block in corpus.partitioned(result.data) for one in block.insns]
    divides = [index for index, one in enumerate(insns) if one.mnemonic == Mnemonic.IDIV]
    assert len(divides) == 1
    assert insns[divides[0] - 1].mnemonic == Mnemonic.CDQ


def test_nbody_repeated_fixed_constant_is_removed():
    """Nbody materialized 512 twice before one divide, with a non-clobbering CDQ between them."""
    from qbopt import wholeseg
    from qbopt.objectfile import module, omf
    from qbopt.frontend import blocks
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
    from qbopt.objectfile.module import Addr, Space
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
