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
    first_ref = mir.MemRef(Addr(Space.FRAME, 4), 2, typed=("pointer4", False))
    second_ref = mir.MemRef(Addr(Space.FRAME, 6), 2, typed=("pointer4", False))

    def load(at: int, value: mir.Value, ref: mir.MemRef) -> lir.Insn:
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

    original = (load(1, first_value, first_ref), load(2, second_value, second_ref))

    selected = farload.selected(original)

    assert selected[0].what is not None and selected[0].what.name == "les"
    assert selected[0].what.sources[0].width == 4
    assert selected[0].defines == (1, 2)
    assert selected[1].what is not None and selected[1].what.op is ir.Operation.NOTHING
