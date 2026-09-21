from pathlib import Path
from dataclasses import replace

import pytest
from iced_x86 import Register

from tests import corpus
from qbopt.model import ir
from qbopt.model import lir
from qbopt.model import mir
from qbopt.backend import cpu
from qbopt.backend import lower
from qbopt.backend import select
from qbopt.frontend import blocks
from qbopt.backend import peephole
from qbopt.backend import addressforms
from qbopt.frontend.declen import decode
from qbopt.objectfile.module import Addr
from qbopt.objectfile.module import Space


@pytest.mark.parametrize("width", [2, 4])
def test_mandel_sum_uses_67h_lea_before_preserving_a_copy(width: int) -> None:
    """Mandelbrot emitted ``mov sum,xx; add sum,yy`` before its escape test.

    Both GCC and Clang spell the dead-flags sum as one LEA. In 16-bit mode
    the dword form needs 67h, but that is still cheaper than preserving and
    updating a third value; the word form has the same modulo-16-bit result.
    """
    registers = (Register.DI, Register.DX, Register.SI) if width == 2 else (Register.EDI, Register.EDX, Register.ESI)
    dest, left, right = (ir.Reg(register, width) for register in registers)
    copy = lir.Insn(
        47,
        (47, 47),
        ir.Semantics(ir.Operation.MOVE, "mov", (dest,), (left,)),
        (3,),
        (1,),
        widths=((1, width), (3, width)),
    )
    addition = lir.Insn(
        47,
        (47, 48),
        ir.Semantics(ir.Operation.BINARY, "add", (dest,), (dest, right)),
        (4,),
        (3, 2),
        widths=((2, width), (3, width), (4, width)),
    )
    compare = lir.Insn(
        48,
        (48, 48),
        ir.Semantics(ir.Operation.COMPARE, "cmp", (), (dest, ir.Imm(1024, width))),
        (),
        (4,),
        widths=((4, width),),
    )
    body = lir.LirBody("mandel", 47, (lir.LirBlock(47, (copy, addition, compare), ()),), {}, {})

    result = peephole.addresses(body, cpu="386").insns

    assert [one.what.name for one in result] == ["lea", "cmp"]
    assert result[0].defines == (4,)
    assert result[0].uses == (1, 2)
    assert result[0].widths == ((1, width), (2, width), (4, width))
    address = result[0].what.sources[0]
    assert isinstance(address, ir.Address)
    assert {address.through, address.index} == {Register.EDX, Register.ESI}
    encoded = select.emit(result[0].what)
    assert encoded is not None and 0x67 in encoded.code[:2]


def test_sum_lea_preserves_observed_add_flags() -> None:
    """A conditional branch after the sum still reads ADD's flags."""
    dest, left, right = (ir.Reg(register, 4) for register in (Register.EDI, Register.EDX, Register.ESI))
    copy = lir.Insn(
        47,
        (47, 47),
        ir.Semantics(ir.Operation.MOVE, "mov", (dest,), (left,)),
        (3,),
        (1,),
    )
    addition = lir.Insn(
        47,
        (47, 48),
        ir.Semantics(ir.Operation.BINARY, "add", (dest,), (dest, right)),
        (4,),
        (3, 2),
    )
    branch = lir.Insn(48, (48, 48), ir.Semantics(ir.Operation.BRANCH, "je", (), (), 60), (), ())
    body = lir.LirBody("flagged", 47, (lir.LirBlock(47, (copy, addition, branch), (60,)),), {}, {})

    result = peephole.addresses(body, cpu="386").insns

    assert [one.what.name for one in result] == ["mov", "add", "je"]


def _constant_sum_body(amount: int, *, symbolic_add: bool = False) -> lir.LirBody:
    dest = ir.Reg(Register.BX, 2)
    source = ir.Reg(Register.AX, 2)
    copy = lir.Insn(
        10,
        (10, 10),
        ir.Semantics(ir.Operation.MOVE, "mov", (dest,), (source,)),
        (3,),
        (1,),
        widths=((1, 2), (3, 2)),
    )
    addition = lir.Insn(
        10,
        (10, 11),
        ir.Semantics(ir.Operation.BINARY, "add", (dest,), (dest, ir.Imm(amount, 2))),
        (4,),
        (3,),
        widths=((3, 2), (4, 2)),
        symbol=True if symbolic_add else None,
    )
    store = lir.Insn(
        11,
        (11, 11),
        ir.Semantics(
            ir.Operation.MOVE,
            "mov",
            (ir.Mem(Addr(Space.FRAME, -4), 2, through=Register.BP, offset=-4),),
            (dest,),
        ),
        (),
        (4,),
        widths=((4, 2),),
    )
    compare = lir.Insn(
        12,
        (12, 12),
        ir.Semantics(ir.Operation.COMPARE, "cmp", (), (source, ir.Imm(0, 2))),
        (),
        (1,),
        widths=((1, 2),),
    )
    return lir.LirBody("constant_sum", 10, (lir.LirBlock(10, (copy, addition, store, compare), ()),), {}, {})


@pytest.mark.parametrize("amount,displacement", [(1, 1), (0xFFFF, -1)])
def test_constant_sum_uses_67h_lea_without_code_growth(amount: int, displacement: int) -> None:
    """Matmul initialized each local element with ``mov bx,ax; add bx,1``.

    The ADD flags die at the following store.  A word destination takes the
    low sixteen bits of the 32-bit effective address, so one 67h LEA has the
    same wrapping result while removing the copied two-address operation.
    """
    body = _constant_sum_body(amount)

    result = peephole.addresses(body, cpu="386").insns

    assert [one.what.name for one in result] == ["lea", "mov", "cmp"]
    assert result[0].what.sources == (ir.Address(None, through=Register.EAX, offset=displacement),)
    encoded = select.emit(result[0].what)
    assert encoded is not None and 0x67 in encoded.code[:2]


def test_constant_sum_keeps_a_shorter_move_add_encoding() -> None:
    """A 32-bit LEA displacement must not grow a shorter word-immediate pair."""
    result = peephole.addresses(_constant_sum_body(0x1234), cpu="386").insns

    assert [one.what.name for one in result] == ["mov", "add", "mov", "cmp"]


def test_constant_sum_preserves_unrolled_source_anchor_on_lea() -> None:
    """Matmul's unrolled ADD clone owned its conservative source anchor.

    The first constant-LEA regression used an ordinary LIR instruction, so
    it passed while every production candidate was rejected.  The ADD itself
    survives as the equivalent LEA and must keep that ownership marker; the
    copied predecessor may still be removed.
    """
    result = peephole.addresses(_constant_sum_body(1, symbolic_add=True), cpu="386").insns

    assert [one.what.name for one in result] == ["lea", "mov", "cmp"]
    assert result[0].symbol is True


def test_constant_sum_does_not_drop_a_predecessor_source_anchor() -> None:
    """A fold cannot discard source ownership held by the removed copy."""
    body = _constant_sum_body(1)
    block = body.blocks[0]
    body = replace(body, blocks=(replace(block, insns=(replace(block.insns[0], symbol=True), *block.insns[1:])),))

    result = peephole.addresses(body, cpu="386").insns

    assert [one.what.name for one in result] == ["mov", "add", "mov", "cmp"]
    assert result[0].symbol is True


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


def test_based_local_constant_offset_folds_into_memory_displacement() -> None:
    """Matmul emitted ``mov di,si; add di,2; mov ax,ss:[di]`` for every
    element after the first in a local row.

    The copied-and-incremented value is only an address spelling.  A literal
    SS-relative cell has no relocation whose addend could be changed, so the
    same 16-bit wrapping addition belongs directly in ``ss:[si+2]``.
    """
    original = ir.Semantics(
        ir.Operation.MOVE,
        "mov",
        (ir.Held(3, 2),),
        (
            ir.Mem(
                Addr(Space.LITERAL, 0, segment=Register.SS),
                2,
                base=ir.Held(2, 2),
            ),
        ),
    )

    changed = addressforms.selected(original, {2: (ir.Held(1, 2), 6)})

    assert changed is not None
    cell = changed.sources[0]
    assert isinstance(cell, ir.Mem)
    assert cell.base == ir.Held(1, 2)
    assert cell.addr == Addr(Space.LITERAL, 6, segment=Register.SS)
    selected = replace(cell, through=Register.SI)
    emitted = select.move_from(Register.AX, selected)
    assert emitted is not None
    instruction = decode(emitted.code, 0)
    assert instruction is not None
    assert instruction.insn.memory_segment == Register.SS
    assert instruction.insn.memory_base == Register.SI
    assert instruction.insn.memory_displacement == 6


def test_based_constant_offset_address_computation_is_fully_folded() -> None:
    """Matmul's cell changed from ``ss:[derived]`` to ``ss:[base+6]`` but
    lowering still emitted the symbolic ADD which computed ``derived``.

    Once every read of the result is an encodable cell base and its flags are
    dead, the address computation itself is part of the fold.  It must not
    survive for machine DCE to guess whether its symbolic provenance is a
    relocation.
    """
    base = mir.Value(1, 0)
    address = mir.Value(2, 0)
    loaded = mir.Value(3, 0)
    add = mir.Op(
        1,
        ir.Operation.BINARY,
        "add",
        (address,),
        (base,),
        kind=mir.Kind.ADD,
        args=(mir.Held(base, 2), mir.Const(6, 2)),
        results=(mir.Held(address, 2),),
        symbol=True,
    )
    ref = mir.MemRef(
        Addr(Space.LITERAL, 0, segment=Register.SS),
        2,
        base=address,
        space=Space.FRAME,
        base_width=2,
    )
    load = mir.Op(
        2,
        ir.Operation.MOVE,
        "mov",
        (loaded,),
        (address,),
        kind=mir.Kind.LOAD,
        args=(mir.Cell(ref),),
        results=(mir.Held(loaded, 2),),
        loads=(ref,),
    )
    body = mir.MirBody(0, (mir.MirBlock(0, (), (add, load), ()),))

    _forms, folded, _promoted = addressforms.indexed(body, set())

    assert folded == frozenset({address.id})


def test_whole_word_merge_does_not_keep_folded_address_arithmetic() -> None:
    """qmove's BASIC frontend computed ``vector[1]`` with a word ADD while
    C put the same constant displacement directly on the cell.  Selection
    folded the BASIC cells to ``[vector+4]`` but retained ``mov ax,si / add
    ax,4`` because the raised ADD carried the machine's upper-half merge.

    A word result does not semantically preserve that unrepresented upper
    half: ``mir.partial`` distinguishes it from a true partial write.  Once
    cells are the only readers and flags are dead, the congruence hint must
    not keep the otherwise folded address computation alive.
    """
    base = mir.Value(1, 0)
    address = mir.Value(2, 0)
    loaded = mir.Value(3, 0)
    add = mir.Op(
        1,
        ir.Operation.BINARY,
        "add",
        (address,),
        (base,),
        kind=mir.Kind.ADD,
        args=(mir.Held(base, 2), mir.Const(4, 2)),
        results=(mir.Held(address, 2),),
        merges={base: address},
        symbol=True,
    )
    ref = mir.MemRef(
        Addr(Space.LITERAL, 0),
        4,
        base=address,
        space=Space.LITERAL,
        base_width=2,
    )
    load = mir.Op(
        2,
        ir.Operation.MOVE,
        "mov",
        (loaded,),
        (address,),
        kind=mir.Kind.LOAD,
        args=(mir.Cell(ref),),
        results=(mir.Held(loaded, 4),),
        loads=(ref,),
    )
    body = mir.MirBody(0, (mir.MirBlock(0, (), (add, load), ()),))

    forms, folded, _promoted = addressforms.indexed(body, set())

    assert forms == {}
    assert folded == frozenset({address.id})


def test_relocated_constant_offset_address_computation_is_not_deleted() -> None:
    """A relocation owns a SEGMENT cell's displacement and cannot absorb
    arithmetic performed on its dynamic base.

    Recognizing the copied base is still useful to lowering, but deleting
    its defining ADD would leave the relocated memory operand reading an
    undefined value because ``selected`` deliberately cannot move that
    constant into the relocation's addend.
    """
    base = mir.Value(1, 0)
    address = mir.Value(2, 0)
    loaded = mir.Value(3, 0)
    add = mir.Op(
        1,
        ir.Operation.BINARY,
        "add",
        (address,),
        (base,),
        kind=mir.Kind.ADD,
        args=(mir.Held(base, 2), mir.Const(6, 2)),
        results=(mir.Held(address, 2),),
        symbol=True,
    )
    ref = mir.MemRef(
        Addr(Space.SEGMENT, 20, index=1),
        2,
        base=address,
        space=Space.SEGMENT,
        base_width=2,
    )
    load = mir.Op(
        2,
        ir.Operation.MOVE,
        "mov",
        (loaded,),
        (address,),
        kind=mir.Kind.LOAD,
        args=(mir.Cell(ref),),
        results=(mir.Held(loaded, 2),),
        loads=(ref,),
    )
    body = mir.MirBody(0, (mir.MirBlock(0, (), (add, load), ()),))

    _forms, folded, _promoted = addressforms.indexed(body, set())

    assert address.id not in folded


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

    forms, folded, _promoted = addressforms.indexed(body, set())

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
    forms, folded, _promoted = addressforms.indexed(body, set())
    cell = ir.Mem(Addr(Space.LITERAL, 0, segment=Register.SS), 2, base=ir.Held(address.id, 2))

    changed = addressforms.scaled(ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(loaded.id, 2),), (cell,)), forms)

    assert folded == frozenset({frame.id, address.id})
    assert changed is not None
    indexed = changed.sources[0]
    assert isinstance(indexed, ir.Mem)
    assert indexed.addr == Addr(Space.LITERAL, -132, segment=Register.SS)
    assert indexed.through == Register.BP
    assert indexed.base is None
    assert indexed.index == ir.Held(index.id, 2)


def test_selected_indexed_frame_cell_keeps_bp_and_its_dynamic_index() -> None:
    """Modern nbody's six bodies all collapsed onto element zero at final selection.

    MIR still carried the dynamic byte offset, but the frame encoding retained
    only BP and `-96`, producing `[bp-96]` instead of `[bp+si-96]`.
    """
    cell = ir.Mem(
        Addr(Space.FRAME, -96),
        4,
        through=Register.BP,
        base=ir.Held(1, 2),
        index_through=Register.SI,
    )

    made = select.move_from(Register.EAX, cell)

    assert made is not None
    instruction = decode(made.code, 0)
    assert instruction is not None
    assert instruction.insn.memory_base == Register.BP
    assert instruction.insn.memory_index == Register.SI
    assert instruction.insn.memory_displacement & 65535 == (-96) & 65535


def test_named_data_address_is_a_relocatable_immediate() -> None:
    """Modern nbody's first string address could not be written to OMF.

    A segment symbol is an offset value, so ``mov ax, offset label`` carries
    the relocation in an immediate.  Spelling it ``lea ax, label`` leaves the
    fresh object writer looking for a displacement field the instruction
    cannot encode in real-mode's grouped data model.
    """
    result = mir.Value(2, 1)
    address = Addr(Space.SEGMENT, 4, 7)
    operation = mir.Op(
        1,
        ir.Operation.ADDRESS,
        "lea",
        (result,),
        (),
        kind=mir.Kind.ADDRESS,
        args=(mir.Cell(mir.MemRef(address, 2, space=Space.SEGMENT)),),
        results=(mir.Held(result, 2),),
    )

    lowered = lower.semantics(operation, place=lower.as_a_value)

    assert lowered is not None
    assert lowered.op is ir.Operation.MOVE
    assert lowered.name == "mov"
    assert lowered.sources == (ir.Imm(0, 2, address),)


def test_constant_frame_array_address_folds_to_a_displacement() -> None:
    """Unrolled C nbody spilled twelve ``&local[0] + constant`` addresses.

    A constant offset from a known frame object is already an encodable BP
    displacement.  It must not become a loop-invariant value that allocation
    has to preserve in a register or spill slot.
    """
    frame = mir.Value(1, 0)
    address = mir.Value(2, 0)
    loaded = mir.Value(3, 0)
    frame_address = mir.Op(
        1,
        ir.Operation.ADDRESS,
        "lea",
        (frame,),
        (),
        kind=mir.Kind.ADDRESS,
        args=(mir.FrameAddress(-36, 2, (-36, -4)),),
        results=(mir.Held(frame, 2),),
    )
    add = mir.Op(
        2,
        ir.Operation.BINARY,
        "add",
        (address,),
        (frame,),
        kind=mir.Kind.ADD,
        args=(mir.Held(frame, 2), mir.Const(8, 2)),
        results=(mir.Held(address, 2),),
    )
    ref = mir.MemRef(
        Addr(Space.LITERAL, 0),
        8,
        base=address,
        space=Space.FRAME,
        base_width=2,
        within=((-36, -4),),
    )
    load = mir.Op(
        3,
        ir.Operation.FLOAT_LOAD,
        "fld",
        (loaded,),
        (address,),
        kind=mir.Kind.FLOAD,
        args=(mir.Cell(ref),),
        results=(mir.Held(loaded, 10),),
        loads=(ref,),
    )
    body = mir.MirBody(0, (mir.MirBlock(0, (), (frame_address, add, load), ()),))
    forms, folded, _promoted = addressforms.indexed(body, set())
    cell = ir.Mem(Addr(Space.LITERAL, 0, segment=Register.SS), 8, base=ir.Held(address.id, 2))

    changed = addressforms.scaled(ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (ir.St(0),), (cell,)), forms)

    assert folded == frozenset({frame.id, address.id})
    assert changed is not None
    direct = changed.sources[0]
    assert isinstance(direct, ir.Mem)
    assert direct.addr == Addr(Space.FRAME, -28)
    assert direct.through == Register.NONE
    assert direct.base is None
    assert direct.index is None
    selected = select.emit(changed)
    assert selected is not None
    instruction = decode(selected.code, 0)
    assert instruction is not None
    assert instruction.insn.memory_segment == Register.SS
    assert instruction.insn.memory_base == Register.BP
    assert instruction.insn.memory_displacement & 65535 == (-28) & 65535


def test_chained_constant_frame_addresses_fold_to_one_displacement() -> None:
    """Peeled C nbody spilled ``&x[4] - 16`` instead of encoding ``[bp-20]``.

    Strength reduction writes a convenient one-past-the-end address and
    derives several fixed elements from it.  The intermediate address is an
    implementation detail, not a value that needs a register.
    """
    frame, end, element, loaded = (mir.Value(index, 0) for index in range(1, 5))
    frame_address = mir.Op(
        1,
        ir.Operation.ADDRESS,
        "lea",
        (frame,),
        (),
        kind=mir.Kind.ADDRESS,
        args=(mir.FrameAddress(-36, 2, (-36, -4)),),
        results=(mir.Held(frame, 2),),
    )

    def add(at: int, result: mir.Value, source: mir.Value, amount: int) -> mir.Op:
        return mir.Op(
            at,
            ir.Operation.BINARY,
            "add",
            (result,),
            (source,),
            kind=mir.Kind.ADD,
            args=(mir.Held(source, 2), mir.Const(amount, 2)),
            results=(mir.Held(result, 2),),
        )

    ref = mir.MemRef(
        Addr(Space.LITERAL, 0),
        8,
        base=element,
        space=Space.FRAME,
        base_width=2,
        within=((-36, -4),),
    )
    load = mir.Op(
        4,
        ir.Operation.FLOAT_LOAD,
        "fld",
        (loaded,),
        (element,),
        kind=mir.Kind.FLOAD,
        args=(mir.Cell(ref),),
        results=(mir.Held(loaded, 10),),
        loads=(ref,),
    )
    body = mir.MirBody(
        0,
        (mir.MirBlock(0, (), (frame_address, add(2, end, frame, 32), add(3, element, end, 65520), load), ()),),
    )

    forms, folded, _promoted = addressforms.indexed(body, set())
    cell = ir.Mem(Addr(Space.LITERAL, 0, segment=Register.SS), 8, base=ir.Held(element.id, 2))
    changed = addressforms.scaled(ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (ir.St(0),), (cell,)), forms)

    assert folded == frozenset({frame.id, end.id, element.id})
    assert changed is not None
    direct = changed.sources[0]
    assert isinstance(direct, ir.Mem)
    assert direct.addr == Addr(Space.FRAME, -20)
    assert direct.base is None


def test_frame_address_root_survives_a_live_derived_value() -> None:
    """Peeled C matmul lowered three ADDs from an undefined frame-address root.

    A pure constant-derived address is only part of the memory fold when its
    complete use chain reaches foldable cells.  Merely recognizing the child
    as a frame address must not delete the parent while the child remains an
    ordinary observable value.
    """
    frame, derived = (mir.Value(index, 0) for index in range(1, 3))
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
        (derived,),
        (frame,),
        kind=mir.Kind.ADD,
        args=(mir.Held(frame, 2), mir.Const(14, 2)),
        results=(mir.Held(derived, 2),),
    )
    body = mir.MirBody(0, (mir.MirBlock(0, (), (frame_address, add), ()),))

    forms, folded, _promoted = addressforms.indexed(body, {derived.id})

    assert forms == {}
    assert folded == frozenset()


def test_direct_frame_array_address_folds_into_its_memory_operand() -> None:
    """Unrolled C nbody emitted six LEAs for element zero's fixed addresses."""
    frame = mir.Value(1, 0)
    loaded = mir.Value(2, 0)
    frame_address = mir.Op(
        1,
        ir.Operation.ADDRESS,
        "lea",
        (frame,),
        (),
        kind=mir.Kind.ADDRESS,
        args=(mir.FrameAddress(-36, 2, (-36, -4)),),
        results=(mir.Held(frame, 2),),
    )
    ref = mir.MemRef(
        Addr(Space.LITERAL, 0),
        8,
        base=frame,
        space=Space.FRAME,
        base_width=2,
        within=((-36, -4),),
    )
    load = mir.Op(
        2,
        ir.Operation.FLOAT_LOAD,
        "fld",
        (loaded,),
        (frame,),
        kind=mir.Kind.FLOAD,
        args=(mir.Cell(ref),),
        results=(mir.Held(loaded, 10),),
        loads=(ref,),
    )
    body = mir.MirBody(0, (mir.MirBlock(0, (), (frame_address, load), ()),))
    forms, folded, _promoted = addressforms.indexed(body, set())
    cell = ir.Mem(Addr(Space.LITERAL, 0, segment=Register.SS), 8, base=ir.Held(frame.id, 2))

    changed = addressforms.scaled(ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (ir.St(0),), (cell,)), forms)

    assert folded == frozenset({frame.id})
    assert changed is not None
    direct = changed.sources[0]
    assert isinstance(direct, ir.Mem)
    assert direct.addr == Addr(Space.FRAME, -36)
    assert direct.base is None
    selected = select.emit(changed)
    assert selected is not None
    instruction = decode(selected.code, 0)
    assert instruction is not None
    assert instruction.insn.memory_segment == Register.SS
    assert instruction.insn.memory_base == Register.BP
    assert instruction.insn.memory_displacement & 65535 == (-36) & 65535


def test_secondary_scaled_address_replaces_a_live_word_product_before_spilling() -> None:
    """indexed.lru_use spilled ``bnext`` while carrying ``b * 2``.

    The non-negative guard makes zero extension exact for every defined C
    lvalue access.  Select the target's 67h ``base32 + index32 * 2`` form at
    lowering, remove both word arithmetic values, and promote the existing
    base/index definitions rather than adding another live range.
    """
    index, base, product, address, loaded = (mir.Value(number, number) for number in range(1, 6))
    flags = mir.Value(6, 1, flags=True)
    index_ref = mir.MemRef(Addr(Space.FRAME, 6), 2, typed=("int2", True))
    base_ref = mir.MemRef(Addr(Space.FRAME, 8), 2, typed=("pointer4", True))
    read_index = mir.Op(
        1,
        ir.Operation.MOVE,
        "mov",
        (index,),
        (),
        kind=mir.Kind.LOAD,
        args=(mir.Cell(index_ref),),
        results=(mir.Held(index, 2),),
        loads=(index_ref,),
    )
    read_base = mir.Op(
        2,
        ir.Operation.MOVE,
        "mov",
        (base,),
        (),
        kind=mir.Kind.LOAD,
        args=(mir.Cell(base_ref),),
        results=(mir.Held(base, 2),),
        loads=(base_ref,),
    )
    compare = mir.Op(
        3,
        ir.Operation.COMPARE,
        "cmp",
        (flags,),
        (index,),
        kind=mir.Kind.SUB,
        args=(mir.Held(index, 2), mir.Const(0, 2)),
    )
    branch = mir.Op(
        4,
        ir.Operation.BRANCH,
        "jge",
        (),
        (flags,),
        kind=mir.Kind.BRANCH,
        test=mir.Kind.GE,
        target=7,
    )
    multiply = mir.Op(
        7,
        ir.Operation.MULTIPLY,
        "imul",
        (product,),
        (index,),
        kind=mir.Kind.MUL,
        args=(mir.Held(index, 2), mir.Const(2, 2)),
        results=(mir.Held(product, 2),),
    )
    addition = mir.Op(
        8,
        ir.Operation.BINARY,
        "add",
        (address,),
        (base, product),
        kind=mir.Kind.ADD,
        args=(mir.Held(base, 2), mir.Held(product, 2)),
        results=(mir.Held(address, 2),),
    )
    element = mir.MemRef(
        Addr(Space.FAR, 0),
        2,
        base=address,
        space=Space.FAR,
        base_width=2,
        typed=("int2", False),
    )
    load = mir.Op(
        9,
        ir.Operation.MOVE,
        "mov",
        (loaded,),
        (address,),
        kind=mir.Kind.LOAD,
        args=(mir.Cell(element),),
        results=(mir.Held(loaded, 2),),
        loads=(element,),
    )
    body = mir.MirBody(
        1,
        (
            mir.MirBlock(1, (), (read_index, read_base, compare, branch), (5, 7)),
            mir.MirBlock(5, (), (), ()),
            mir.MirBlock(7, (), (multiply, addition, load), ()),
        ),
    )

    target = cpu.profile("386")
    forms, folded, promoted = addressforms.indexed(body, set(), target.address_forms, target.operations)

    assert forms[address.id] == (ir.Held(base.id, 4), ir.Held(index.id, 4), 2)
    assert {product.id, address.id} <= folded
    assert promoted == frozenset({index.id, base.id})
