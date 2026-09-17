from pathlib import Path
from dataclasses import replace

import pytest
from iced_x86 import Register

from tests import corpus
from qbopt.model import ir
from qbopt.model import mir
from qbopt.backend import lower
from qbopt.backend import select
from qbopt.frontend import blocks
from qbopt.frontend.declen import decode


def test_c_indexed_store_retains_relocated_displacement() -> None:
    # d_faces at 09F4 lost +0CA0h and refused emission: one fixup, zero fields.
    module = corpus.loaded(Path("fixtures/regressions/d_faces-borland.obj"))
    assert module is not None
    mapped = blocks.code_map(module)
    assert not isinstance(mapped, str)
    raised = mir.bodies(module, list(blocks.partition(module, mapped)))
    body = next(body for _, body in raised if body.entry == 0x38A)
    op = next(op for block in body.blocks for op in block.ops if op.at == 0x9F4)
    assert op.stores[0].addr is not None
    what = lower.current(op, node=raised.source.nodes.get(op.id))
    assert what is not None
    made = select.emit(what)
    assert made is not None
    instruction = decode(made.code, 0)
    assert instruction is not None
    assert instruction.insn.memory_base == Register.BX
    assert instruction.disp_len == 2
    assert made.fields == (instruction.disp_at,)
    assert instruction.insn.memory_displacement == 0
    assert isinstance(what.dests[0], ir.Mem)
    assert what.dests[0].offset == 0xCA0


@pytest.mark.parametrize("at,register", [(0x4E, Register.DI), (0x85, Register.SI), (0x2B5, Register.DI)])
def test_c_float_load_keeps_far_segment(at: int, register: int) -> None:
    # Rewritten r_walk read DS instead of ES and qrender reported string space corrupt.
    module = corpus.loaded(Path("fixtures/regressions/r_walk-borland.obj"))
    assert module is not None
    mapped = blocks.code_map(module)
    assert not isinstance(mapped, str)
    raised = mir.bodies(module, list(blocks.partition(module, mapped)))
    body = next(body for _, body in raised if body.entry == 0)
    op = next(op for block in body.blocks for op in block.ops if op.at == at)
    what = lower.current(op, lower.as_a_value, node=raised.source.nodes.get(op.id))
    assert what is not None
    cell = what.sources[0]
    assert isinstance(cell, ir.Mem) and cell.base is not None
    made = select.emit(replace(what, sources=(replace(cell, through=register),)))
    assert made is not None
    instruction = decode(made.code, 0)
    assert instruction is not None
    assert instruction.insn.memory_segment == Register.ES
    assert instruction.insn.memory_base == register
    original = decode(module.code, at)
    assert original is not None
    assert instruction.insn.memory_displacement == original.insn.memory_displacement
