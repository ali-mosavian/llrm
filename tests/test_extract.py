"""Bit extraction has value operands; only lowering chooses machine form."""

from qbopt import ir
from qbopt import lir
from qbopt import mir
from qbopt import lower
from qbopt import objwrite


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
    carried = objwrite._carried(expanded[1])
    assert carried.node is None
    assert carried.id is None
    assert carried.covers == (10, 10)
    assert carried.made == expanded[1].what


def test_inserted_move_does_not_inherit_disjoint_input_bytes() -> None:
    """LNGMIX deletion refused because an inserted move also claimed 12 push bytes."""
    parent = mir.Op(0x71, ir.Operation.MOVE, "mov", (), (), covers=(0x71, 0x76), extra_covers=((0x5F, 0x6B),), id=14)
    move = ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(2, 2),), (ir.Held(1, 2),))
    inserted = lir.Insn(0x71, (0x71, 0x71), move, (2,), (1,), op=parent)
    carried = objwrite._carried(inserted)
    assert carried.covers == (0x71, 0x71)
    assert carried.extra_covers == ()
    assert carried.id is None
