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


def test_address_of_a_canonical_frame_cell_publishes_its_store_to_a_call() -> None:
    """Q45P04 passed uninitialized BYREF slots after DSE deleted 100000 and 23.

    The QB HIR frontend represents ``address local`` as ADDRESS of a canonical
    Cell.  That spelling carries the same object identity as FrameAddress and
    must flow through the resulting pointer, otherwise the observer analysis
    falsely calls the local private while its callee reads it.
    """
    pointer = mir.Value(1, 1, variable=1, version=1)
    object_ = memory.Object(memory.Kind.FRAME, ("Q45P04", "$arg3"), extent=4)
    provenance = memory.Provenance.one(object_, 0, 4)
    cell = mir.MemRef(
        Addr(Space.FRAME, -10),
        4,
        space=Space.FRAME,
        provenance=provenance,
    )
    store = mir.Op(
        1,
        ir.Operation.MOVE,
        "mov",
        (),
        (),
        kind=mir.Kind.STORE,
        args=(mir.Const(100000, 4),),
        results=(mir.Cell(cell),),
        stores=(cell,),
    )
    address = mir.Op(
        2,
        ir.Operation.ADDRESS,
        "address",
        (pointer,),
        (),
        kind=mir.Kind.ADDRESS,
        args=(mir.Cell(replace(cell, width=2)),),
        results=(mir.Held(pointer, 2),),
    )
    pointee = mir.MemRef(None, 4, base=pointer, space=Space.LITERAL, base_width=2, pointer=True)
    call = mir.Op(
        3,
        ir.Operation.CALL,
        "call",
        (),
        (pointer,),
        kind=mir.Kind.CALL,
        args=(mir.Held(pointer, 2),),
        loads=(pointee,),
        stores=(pointee,),
    )
    body = mir.MirBody(0, (mir.MirBlock(0, (), (store, address, call), ()),), sealed=True)

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


def test_an_owned_allocation_becomes_private_only_after_its_last_load_is_gone() -> None:
    """PARITY's first optimization round must not delete stores needed by the next SROA round."""
    descriptor = mir.Symbol(Space.FRAME, 0, -38, 2)
    object_ = memory.Object(memory.Kind.ALLOCATION, (descriptor, 7, "root"), extent=4)
    cell = mir.MemRef(
        Addr(Space.FAR, 0),
        4,
        space=Space.FAR,
        allocation=descriptor,
        provenance=memory.Provenance.one(object_, 0, 4),
    )
    stored = mir.Op(
        1,
        ir.Operation.MOVE,
        "mov",
        (),
        (),
        kind=mir.Kind.STORE,
        args=(mir.Const(37, 4),),
        results=(mir.Cell(cell),),
        stores=(cell,),
    )
    value = mir.Value(2, 2, variable=2, version=1)
    loaded = mir.Op(
        2,
        ir.Operation.MOVE,
        "mov",
        (value,),
        (),
        kind=mir.Kind.LOAD,
        args=(mir.Cell(cell),),
        results=(mir.Held(value, 4),),
        loads=(cell,),
    )
    observed = mir.MirBody(0, (mir.MirBlock(0, (), (stored, loaded), ()),), sealed=True)
    private = observers.private(observed, None, None)
    assert private is not None and not private(cell)
    assert stored not in avail.dead_stores(observed, frozenset(), {}, private)

    unread = replace(observed, blocks=(replace(observed.blocks[0], ops=(stored,)),))
    private = observers.private(unread, None, None)
    assert private is not None and private(cell)
    assert stored in avail.dead_stores(unread, frozenset(), {}, private)


def _allocation_passed_by_descriptor(lifecycle: bool) -> tuple[mir.MirBody, mir.Op, mir.MemRef]:
    descriptor = mir.Symbol(Space.FRAME, 0, -38, 2)
    object_ = memory.Object(memory.Kind.ALLOCATION, (descriptor, 7, "root"), extent=4)
    cell = mir.MemRef(
        Addr(Space.FAR, 0),
        4,
        space=Space.FAR,
        allocation=descriptor,
        provenance=memory.Provenance.one(object_, 0, 4),
    )
    stored = mir.Op(
        1,
        ir.Operation.MOVE,
        "mov",
        (),
        (),
        kind=mir.Kind.STORE,
        args=(mir.Const(37, 4),),
        results=(mir.Cell(cell),),
        stores=(cell,),
    )
    pointer = mir.Value(2, 1, variable=1, version=1)
    address = mir.Op(
        2,
        ir.Operation.ADDRESS,
        "address",
        (pointer,),
        (),
        kind=mir.Kind.ADDRESS,
        args=(mir.FrameAddress(descriptor.offset, descriptor.width),),
        results=(mir.Held(pointer, descriptor.width),),
    )
    argument = mir.Op(
        3,
        ir.Operation.PUSH,
        "push",
        (),
        (pointer,),
        kind=mir.Kind.ARG,
        args=(mir.Held(pointer, descriptor.width),),
    )
    request = mir.ArrayRequest(descriptor, 4, ((0, 0),)) if lifecycle else None
    call = mir.Op(4, ir.Operation.CALL, "call", (), (), kind=mir.Kind.CALL, array=request)
    ops = (address, argument, call, stored) if lifecycle else (stored, address, argument, call)
    return mir.MirBody(0, (mir.MirBlock(0, (), ops, ()),), sealed=True), stored, cell


def test_passing_an_array_descriptor_publishes_its_current_allocation() -> None:
    """QBSP looped forever after DSE erased every initialized BSP child.

    The callee received the dynamic-array descriptor rather than its loaded
    data pointer.  That still exposes the allocation owned by the descriptor.
    """
    body, stored, cell = _allocation_passed_by_descriptor(False)
    private = observers.private(body, None, None)
    assert private is not None and not private(cell)
    assert stored not in avail.dead_stores(body, frozenset(), {}, private)


def test_allocating_through_a_descriptor_does_not_publish_the_new_allocation() -> None:
    """DIM receives the descriptor before the allocation generation exists."""
    body, stored, cell = _allocation_passed_by_descriptor(True)
    private = observers.private(body, None, None)
    assert private is not None and private(cell)
    assert stored in avail.dead_stores(body, frozenset(), {}, private)
