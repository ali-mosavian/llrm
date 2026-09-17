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
from qbopt.backend import addressforms
from qbopt.frontend.declen import decode
from qbopt.objectfile.module import Addr
from qbopt.objectfile.module import Space


@pytest.mark.parametrize("offset", [4, 8, -12, 65535])
@pytest.mark.full
def test_far_float_address_folds_offset_without_changing_selector(offset: int) -> None:
    found = corpus.loaded(Path("fixtures/regressions/r_walk-borland.obj"))
    assert found is not None
    mapped = blocks.code_map(found)
    assert not isinstance(mapped, str)
    body = next(body for _, body in mir.bodies(found, blocks.partition(found, mapped)) if body.entry == 0x2F6)
    op = next(op for block in body.blocks for op in block.ops if op.at == 0x305)
    original = lower.current(op, lower.as_a_value)
    assert original is not None
    cell = original.sources[-1]
    assert isinstance(cell, ir.Mem) and cell.base is not None
    changed = addressforms.selected(original, {cell.base.value: (ir.Held(9999, 2), offset)})
    assert changed is not None
    folded = changed.sources[-1]
    assert isinstance(folded, ir.Mem) and folded.base == ir.Held(9999, 2)
    selected = select.emit(replace(changed, sources=(*changed.sources[:-1], replace(folded, through=Register.SI))))
    assert selected is not None
    instruction = decode(selected.code, 0)
    assert instruction is not None
    assert instruction.insn.memory_segment == Register.ES
    assert instruction.insn.memory_base == Register.SI
    assert instruction.insn.memory_displacement & 65535 == (cell.offset + offset) & 65535


def test_based_local_array_folds_add_into_word_addressing() -> None:
    """shellsort formed `base + (index << 1)` in a third register before every local-array read."""
    index = mir.Value(1, 0)
    shifted = mir.Value(2, 1)
    base = mir.Value(3, 0)
    address = mir.Value(4, 2)
    loaded = mir.Value(5, 3)
    shift = mir.Op(
        1,
        ir.Operation.BINARY,
        "shl",
        (shifted,),
        (index,),
        kind=mir.Kind.SHL,
        args=(mir.Held(index, 2), mir.Const(1, 2)),
        results=(mir.Held(shifted, 2),),
    )
    add = mir.Op(
        2,
        ir.Operation.BINARY,
        "add",
        (address,),
        (base, shifted),
        kind=mir.Kind.ADD,
        args=(mir.Held(base, 2), mir.Held(shifted, 2)),
        results=(mir.Held(address, 2),),
    )
    cell = mir.MemRef(Addr(Space.LITERAL, 0), 2, base=address, space=Space.LITERAL, base_width=2)
    load = mir.Op(
        3,
        ir.Operation.MOVE,
        "mov",
        (loaded,),
        (address,),
        kind=mir.Kind.LOAD,
        args=(mir.Cell(cell),),
        results=(mir.Held(loaded, 2),),
        loads=(cell,),
    )
    body = mir.MirBody(0, (mir.MirBlock(0, (), (shift, add, load), ()),))

    forms, folded = addressforms.indexed(body, set())

    assert forms == {address.id: (ir.Held(base.id, 2), ir.Held(shifted.id, 2), 1)}
    assert folded == frozenset({address.id})


def test_indexed_frame_array_uses_bp_as_the_encoded_base() -> None:
    """C shellsort emitted ``lea bx,[bp-132]`` in every hot array block.

    The dynamic byte offset already fits the index half of 16-bit addressing;
    materialising the fixed local-array base in a second register is excess
    work because ``[bp+si-132]`` encodes the same wrapped address directly.
    """
    index = mir.Value(1, 0)
    frame = mir.Value(2, 0)
    address = mir.Value(3, 0)
    loaded = mir.Value(4, 0)
    frame_address = mir.Op(
        1,
        ir.Operation.ADDRESS,
        "lea",
        (frame,),
        (),
        kind=mir.Kind.ADDRESS,
        args=(mir.FrameAddress(-132, 2, (-132, -4)),),
        results=(mir.Held(frame, 2),),
    )
    add = mir.Op(
        2,
        ir.Operation.BINARY,
        "add",
        (address,),
        (frame, index),
        kind=mir.Kind.ADD,
        args=(mir.Held(frame, 2), mir.Held(index, 2)),
        results=(mir.Held(address, 2),),
    )
    ref = mir.MemRef(
        Addr(Space.LITERAL, 0),
        2,
        base=address,
        space=Space.FRAME,
        base_width=2,
        within=((-132, -4),),
    )
    load = mir.Op(
        3,
        ir.Operation.MOVE,
        "mov",
        (loaded,),
        (address,),
        kind=mir.Kind.LOAD,
        args=(mir.Cell(ref),),
        results=(mir.Held(loaded, 2),),
        loads=(ref,),
    )
    body = mir.MirBody(0, (mir.MirBlock(0, (), (frame_address, add, load), ()),))
    forms, folded = addressforms.indexed(body, set())
    cell = ir.Mem(Addr(Space.LITERAL, 0, segment=Register.SS), 2, base=ir.Held(address.id, 2))

    changed = addressforms.scaled(ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(loaded.id, 2),), (cell,)), forms)

    assert folded == frozenset({address.id})
    assert changed is not None
    indexed = changed.sources[0]
    assert isinstance(indexed, ir.Mem)
    assert indexed.addr == Addr(Space.LITERAL, -132, segment=Register.SS)
    assert indexed.through == Register.BP
    assert indexed.base is None
    assert indexed.index == ir.Held(index.id, 2)
