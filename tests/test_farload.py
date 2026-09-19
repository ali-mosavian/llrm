"""Pre-allocation selection of complete far-pointer loads."""

from qbopt.model import ir
from qbopt.model import lir
from qbopt.model import mir
from qbopt.backend import farload
from qbopt.objectfile.module import Addr
from qbopt.objectfile.module import Space


def test_typed_far_pointer_words_without_object_provenance_are_joined() -> None:
    """qcport's dynamic far-struct fields emitted two loads per far pointer.

    Their dynamic owner has no allocation provenance, but the C type and exact
    adjacent effective addresses still prove that they are the two words of
    one far pointer.  Keeping the pair separate made allocation rematerialize
    the struct base for both halves; indexed.lru_use grew from 66 to 81
    instructions and acquired a spill slot.
    """
    first_value, second_value = mir.Value(1, 1), mir.Value(2, 1)
    first_ref = mir.MemRef(Addr(Space.FAR, 4), 2, typed=("pointer4", False))
    second_ref = mir.MemRef(Addr(Space.FAR, 6), 2, typed=("pointer4", False))

    def load(at: int, value: mir.Value, ref: mir.MemRef, cell: ir.Mem) -> lir.Insn:
        destination = ir.Held(value.id, 2)
        op = mir.Op(
            at,
            ir.Operation.MOVE,
            "mov",
            (value,),
            (),
            loads=(ref,),
            kind=mir.Kind.LOAD,
            args=(mir.Cell(ref),),
            results=(mir.Held(value, 2),),
        )
        return lir.Insn(
            at,
            (at, at),
            ir.Semantics(ir.Operation.MOVE, "mov", (destination,), (cell,)),
            (value.id,),
            tuple(one.value for one in ir.values(cell)),
            op=op,
        )

    original = (
        load(1, first_value, first_ref, ir.Mem(first_ref.addr, 2, base=ir.Held(10, 2), selector=ir.Held(11, 2))),
        load(2, second_value, second_ref, ir.Mem(second_ref.addr, 2, base=ir.Held(10, 2), selector=ir.Held(11, 2))),
    )

    selected = farload.selected(original)

    assert selected[0].what is not None and selected[0].what.name == "les"
    assert selected[0].what.sources[0].width == 4
    assert selected[0].defines == (1, 2)
    assert selected[1].what is not None and selected[1].what.op is ir.Operation.NOTHING


def test_fixed_far_pointer_load_defers_fusion_until_after_allocation() -> None:
    """indexed.lru_use's fixed parameter pair became an eager LES.

    That fixed ES at the definition, so allocation copied ES to the long-lived
    FS value and stored the offset in a new frame slot.  A fixed BP-relative
    address cannot be split by allocation; retaining its independent words
    lets ordinary rematerialization act on either half, after which the final
    physical peephole can still select LES/LFS/LGS when the pair stays adjacent.
    """
    values = (mir.Value(1, 1), mir.Value(2, 1))

    def load(at: int, value: mir.Value, displacement: int) -> lir.Insn:
        ref = mir.MemRef(Addr(Space.FRAME, displacement), 2, typed=("pointer4", False))
        destination = ir.Held(value.id, 2)
        cell = ir.Mem(ref.addr, 2)
        op = mir.Op(
            at,
            ir.Operation.MOVE,
            "mov",
            (value,),
            (),
            loads=(ref,),
            kind=mir.Kind.LOAD,
            args=(mir.Cell(ref),),
            results=(mir.Held(value, 2),),
        )
        return lir.Insn(
            at,
            (at, at),
            ir.Semantics(ir.Operation.MOVE, "mov", (destination,), (cell,)),
            (value.id,),
            (),
            op=op,
        )

    original = (load(1, values[0], 4), load(2, values[1], 6))

    assert farload.selected(original) == original
