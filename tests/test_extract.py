"""Bit extraction has value operands; only lowering chooses machine form."""

from qbopt.model import ir
from qbopt.model import lir
from qbopt.model import mir
from qbopt.backend import lower


def test_restore_declares_its_input_and_high_result() -> None:
    """Nbody's preserved high half must reach its assigned location instead of an uninitialized spill."""
    from iced_x86 import Register

    source, high = mir.Value(1, 0), mir.Value(2, 0)
    node = ir.Restore(at=0, end=4, pair=0, effects=ir.RESTORE_EFFECTS[0])
    op = mir.Op(
        0,
        ir.Operation.BARRIER,
        "restore",
        (high,),
        (source,),
        kind=mir.Kind.OPAQUE,
        source_backed=True,
        id=1,
    )
    body = mir.MirBody(0, (mir.MirBlock(0, (), (op,), ()),), {source: Register.EAX, high: Register.EDX})
    lowering = lower.Lowering(body, {source.id, high.id}, {}, (), nodes={op.id: node})
    assert lowering._abi(op) == ((ir.Held(source.id, 4), Register.EAX),)
    assert lowering._idiom(op) == ((ir.Held(high.id, 2), Register.DX),)


def test_lowering_preserves_relocated_store_ownership() -> None:
    """NESTED printed T=0 instead of 675 when its promoted store lost its fixup."""
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    value = mir.Value(1, 0)
    cell = mir.MemRef(Addr(Space.SEGMENT, 0x88, 5), 2)
    op = mir.Op(
        10,
        ir.Operation.BINARY,
        "mov",
        (),
        (value,),
        kind=mir.Kind.STORE,
        args=(mir.Held(value, 2),),
        results=(mir.Cell(cell),),
        stores=(cell,),
        covers=(10, 10),
        id=31,
        symbol=True,
    )
    body = mir.MirBody(10, (mir.MirBlock(10, (), (op,), ()),))
    (instruction,) = lower.Lowering(body, {value.id}, {}, ()).expand(op)
    assert instruction.what.op is ir.Operation.MOVE
    assert instruction.symbol is True
    assert instruction.symbol is True and instruction.id == 31


def test_high_word_extraction_lowers_without_clobbering_flags() -> None:
    """LNGMIX's explicit high-part extraction refused as an unselectable restore."""
    source, result = mir.Value(1, 0), mir.Value(2, 1)
    op = mir.Op(
        10,
        ir.Operation.RESTORE,
        "extract",
        (result,),
        (source,),
        kind=mir.Kind.EXTRACT,
        args=(mir.Held(source, 4), mir.Const(16, 4)),
        results=(mir.Held(result, 2),),
        covers=(10, 14),
    )
    body = mir.MirBody(10, (mir.MirBlock(10, (), (op,), ()),))
    expanded = lower.Lowering(body, {source.id, result.id}, {}, ()).expand(op)
    assert [one.what.name if one.what else None for one in expanded] == ["push", "pop", "pop"]
    assert expanded[0].uses == (source.id,)
    assert expanded[-1].defines == (result.id,)
    assert expanded[0].covers == (10, 14)
    assert all(one.covers == (10, 10) for one in expanded[1:])
    assert expanded[1].defines[0] not in (source.id, result.id)
    assert expanded[1].node is None
    assert expanded[1].id is None
    assert expanded[1].covers == (10, 10)


def test_inserted_move_does_not_inherit_disjoint_input_bytes() -> None:
    """LNGMIX deletion refused because an inserted move also claimed 12 push bytes."""
    parent = mir.Op(0x71, ir.Operation.MOVE, "mov", (), (), covers=(0x71, 0x76), extra_covers=((0x5F, 0x6B),), id=14)
    move = ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(2, 2),), (ir.Held(1, 2),))
    inserted = lir.Insn(0x71, (0x71, 0x71), move, (2,), (1,), op=parent)
    assert inserted.covers == (0x71, 0x71)
    assert inserted.extra_covers == ()
    assert inserted.id is None
