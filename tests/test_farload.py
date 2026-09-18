"""Pre-allocation selection of complete far-pointer loads."""

from qbopt.model import ir
from qbopt.model import lir
from qbopt.model import mir
from qbopt.backend import farload
from qbopt.objectfile.module import Addr
from qbopt.objectfile.module import Space


def test_typed_words_without_object_provenance_are_not_joined() -> None:
    """qcport host.c and mdl_ai.c carried ``pointer4`` type tags on adjacent
    loads whose object provenance was unknown.  Far-load selection dereferenced
    that missing proof and crashed the build instead of conservatively keeping
    the two valid loads.
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

    assert farload.selected(original) == original
