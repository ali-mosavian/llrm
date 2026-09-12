from qbopt.model import ir
from qbopt.model import mir
from qbopt.analysis import avail
from qbopt.objectfile.module import Addr
from qbopt.objectfile.module import Space


def test_explicit_load_operand_is_not_a_preserved_half() -> None:
    # avail.Held shadowed mir.Held, hiding every explicitly read SSA operand.
    source = mir.Value(1, 0)
    result = mir.Value(2, 1)
    cell = mir.MemRef(addr=Addr(Space.LITERAL, 0, 0), width=2)
    op = mir.Op(
        1,
        ir.Operation.MOVE,
        "",
        (result,),
        (source,),
        kind=mir.Kind.LOAD,
        loads=(cell,),
        args=(mir.Cell(cell), mir.Held(source, 2)),
        results=(mir.Held(result, 2),),
    )
    assert avail._preserved(op) == set()
    assert avail.loaded_into(op) is None
