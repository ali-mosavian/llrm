"""Backend packing must preserve the final bytes of adjacent scalar stores."""

from pathlib import Path
from dataclasses import replace

import corpus
import pytest
from iced_x86 import Code, Register

from qbopt import wholeseg
from qbopt.backend import storecombine
from qbopt.model import ir, lir
from qbopt.objectfile.module import Addr, Space


@pytest.mark.parametrize("tag", ["q-O", "p-g2", "v-g3"])
def test_bools_final_words_are_one_dword_store(tag):
    """BOOLS wrote x=-1 and t=2 separately despite adjacent statically known words."""
    result = wholeseg.emitted(Path(f"fixtures/omf/bools-{tag}.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    assert any(one.insn.code == Code.MOV_RM32_IMM32 and one.insn.immediate32 == 0x0002ffff
               for block in corpus.partitioned(result.data) for one in block.insns)


@pytest.mark.parametrize("guard", [None, "gap", "segment", "indexed", "external", "symbol",
                                   "call", "read", "requirements", "block"])
def test_only_adjacent_unobserved_static_literals_are_packed(guard):
    """Packing x=-1,t=2 must not cross a read, call or uncertain address."""
    cell = ir.Mem(Addr(Space.SEGMENT, 14, 5), 2, disp_width=2)
    low = lir.Insn(0, (0, 6), ir.Semantics(ir.Operation.MOVE, "mov", (cell,), (ir.Imm(-1, 2),)), (), ())
    high_cell = replace(cell, addr=cell.addr.plus(2))
    high = replace(low, at=8, covers=(8, 14), what=replace(low.what, dests=(high_cell,), sources=(ir.Imm(2, 2),)))
    marker = lir.Insn(6, (6, 8), ir.Semantics(ir.Operation.NOTHING, "", (), ()), (), ())
    if guard == "gap":
        high = replace(high, what=replace(high.what, dests=(replace(high_cell, addr=cell.addr.plus(4)),)))
    if guard == "segment":
        high = replace(high, what=replace(high.what, dests=(replace(high_cell, addr=replace(high_cell.addr, index=6)),)))
    if guard == "indexed":
        low = replace(low, what=replace(low.what, dests=(replace(cell, through=Register.BX),)))
    if guard == "external":
        low = replace(low, what=replace(low.what, dests=(replace(cell, addr=replace(cell.addr, space=Space.EXTERNAL)),)))
    if guard == "symbol":
        high = replace(high, what=replace(high.what, sources=(ir.Imm(2, 2, cell.addr),)))
    if guard == "call":
        marker = replace(marker, what=ir.Semantics(ir.Operation.CALL, "call", (), ()))
    if guard == "read":
        marker = replace(marker, what=ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg(Register.AX, 2),), (cell,)))
    if guard == "requirements":
        high = replace(high, requires=((ir.Held(1, 2), Register.AX),))
    blocks = (lir.LirBlock(0, (low, marker, high), ()),)
    if guard == "block":
        blocks = (lir.LirBlock(0, (low, marker), (8,)), lir.LirBlock(8, (high,), ()))
    body = lir.LirBody("stores", 0, blocks, {}, {})
    result = storecombine.combined(body)
    if guard is not None:
        assert result == body
    else:
        assert result.insns[0].what.dests == (replace(cell, width=4),)
        assert result.insns[0].what.sources == (ir.Imm(0x2ffff, 4),)
        assert result.insns[-1].what.op is ir.Operation.NOTHING
        assert [one.covers for one in result.insns] == [one.covers for one in body.insns]
