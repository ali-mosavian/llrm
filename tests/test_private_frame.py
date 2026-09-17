"""Exit ownership for canonical current-activation frame objects."""

from dataclasses import replace

from qbopt.model import ir
from qbopt.model import mir
from qbopt.model import memory
from qbopt.analysis import avail
from qbopt.analysis import observers
from qbopt.objectfile.module import Addr
from qbopt.objectfile.module import Space


def _call(at: int) -> mir.Op:
    return mir.Op(at, ir.Operation.CALL, "call", (), (), kind=mir.Kind.CALL)


def _pointer_frame_store(published: bool) -> tuple[mir.MirBody, mir.Op, mir.MemRef]:
    pointer = mir.Value(1, 0, variable=1, version=1)
    object_ = memory.Object(memory.Kind.FRAME, (7, -132, -4), extent=128)
    provenance = memory.Provenance.one(object_, 0, 4)
    cell = mir.MemRef(
        Addr(Space.LITERAL, 0),
        4,
        base=pointer,
        space=Space.FRAME,
        base_width=2,
        provenance=provenance,
    )
    store = mir.Op(
        1,
        ir.Operation.MOVE,
        "mov",
        (),
        (pointer,),
        kind=mir.Kind.STORE,
        args=(mir.Const(37, 4),),
        results=(mir.Cell(cell),),
        stores=(cell,),
    )
    ops = [store]
    if published:
        ops.extend(
            (
                mir.Op(
                    2,
                    ir.Operation.PUSH,
                    "push",
                    (),
                    (pointer,),
                    kind=mir.Kind.ARG,
                    args=(mir.Held(pointer, 2),),
                ),
                _call(3),
            )
        )
    body = mir.MirBody(
        0,
        (mir.MirBlock(0, (), tuple(ops), ()),),
        sealed=True,
        pointer_values=frozenset({pointer}),
        pointer_seeds={pointer: provenance},
    )
    return body, store, cell


def test_a_nonescaping_pointer_derived_frame_store_is_dead_at_return() -> None:
    """Matmul retained 192 stores after SROA promoted every later array load.

    Its local matrices were reached through exact pointer-derived frame
    slices rather than direct ``[bp+n]`` operands.  The representation must
    not make an unescaped current-activation object observable after return.
    """
    body, store, cell = _pointer_frame_store(False)
    private = observers.private(body, None, None)
    assert private is not None and private(cell)
    assert store in avail.dead_stores(body, frozenset(), {}, private)


def test_a_published_pointer_derived_frame_store_stays_observable() -> None:
    """A callee may read a local through a published pointer during the call."""
    body, store, cell = _pointer_frame_store(True)
    private = observers.private(body, None, None)
    assert private is not None and not private(cell)
    assert store not in avail.dead_stores(body, frozenset(), {}, private)


def test_a_direct_frame_address_conservatively_publishes_canonical_objects() -> None:
    """A direct address has no SSA identity that can safely select one object."""
    body, store, cell = _pointer_frame_store(False)
    publish = mir.Op(
        2,
        ir.Operation.PUSH,
        "push",
        (),
        (),
        kind=mir.Kind.ARG,
        args=(mir.FrameAddress(-132, 2, (-132, -4)),),
    )
    body = replace(body, blocks=(replace(body.blocks[0], ops=(store, publish, _call(3))),))
    private = observers.private(body, None, None)
    assert private is not None and not private(cell)
    assert store not in avail.dead_stores(body, frozenset(), {}, private)


def test_a_canonical_pointer_load_reads_a_private_frame_store() -> None:
    """Matmul returned 2990729762: DSE hid local array stores from pointer loads.

    Canonical provenance makes an indirect reference just as named as a
    direct frame operand.  Privacy protects it from unknown outside readers;
    it must not protect it from an overlapping load inside the body.
    """
    body, store, cell = _pointer_frame_store(False)
    loaded = mir.Value(2, 2, variable=2, version=1)
    assert cell.base is not None
    load = mir.Op(
        2,
        ir.Operation.MOVE,
        "mov",
        (loaded,),
        (cell.base,),
        kind=mir.Kind.LOAD,
        args=(mir.Cell(cell),),
        results=(mir.Held(loaded, cell.width),),
        loads=(cell,),
    )
    body = replace(body, blocks=(replace(body.blocks[0], ops=(store, load)),))
    private = observers.private(body, None, None)
    assert private is not None and private(cell)
    assert store not in avail.dead_stores(body, frozenset(), {}, private)
