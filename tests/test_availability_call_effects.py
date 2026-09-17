from dataclasses import replace

import pytest

from qbopt.model import ir
from qbopt.model import mir
from qbopt.analysis import avail
from qbopt.objectfile.module import Addr
from qbopt.objectfile.module import Space


def body_with_call(effect: mir.MemRef, *, complete: bool = False) -> mir.MirBody:
    cell = mir.MemRef(Addr(Space.SEGMENT, 16, 1), 2)
    source, result = mir.Value(1, 0), mir.Value(2, 2)
    store = mir.Op(
        0,
        ir.Operation.MOVE,
        "",
        (),
        (source,),
        kind=mir.Kind.STORE,
        args=(mir.Held(source, 2),),
        results=(mir.Cell(cell),),
        stores=(cell,),
    )
    call = mir.Op(
        1,
        ir.Operation.CALL,
        "",
        (),
        (),
        kind=mir.Kind.CALL,
        loads=(effect,),
        stores=(effect,),
        memory_complete=complete,
    )
    load = mir.Op(
        2,
        ir.Operation.MOVE,
        "",
        (result,),
        (),
        kind=mir.Kind.LOAD,
        args=(mir.Cell(cell),),
        results=(mir.Held(result, 2),),
        loads=(cell,),
    )
    return mir.MirBody(0, (mir.MirBlock(0, (), (store, call, load), ()),))


def test_runtime_name_cannot_override_unknown_mir_effects() -> None:
    body = body_with_call(mir.MemRef(None, 0))
    assert not avail.forwardable(body, frozenset({1}), {1: "B$MUI4"}, frozenset({2}))


def test_disjoint_mir_effects_keep_values_without_runtime_names() -> None:
    body = body_with_call(mir.MemRef(None, 0, beyond=(1, frozenset())), complete=True)
    forwarded = avail.forwardable(body, frozenset({1}), {}, frozenset({2}))
    assert len(forwarded) == 1
    assert forwarded[0].value == body.blocks[0].ops[0].uses[0]


@pytest.mark.parametrize("disjoint", [False, True])
def test_dead_store_uses_call_memory_effects(disjoint: bool) -> None:
    effect = mir.MemRef(None, 0, beyond=(1, frozenset()) if disjoint else None)
    body = body_with_call(effect, complete=True)
    store, call, _ = body.blocks[0].ops
    body = replace(body, blocks=(replace(body.blocks[0], ops=(store, call, replace(store, at=2))),))
    removed = avail.dead_stores(body, frozenset({1}), {} if disjoint else {1: "B$MUI4"})
    assert bool(removed) is disjoint


@pytest.mark.parametrize(
    "complete,overlap,reused",
    [
        (False, False, False),
        (True, False, True),
        (True, True, False),
    ],
)
def test_opaque_memory_footprint_preserves_only_disjoint_values(
    complete: bool,
    overlap: bool,
    reused: bool,
) -> None:
    # A known status-word store may preserve another local, never its own
    # old contents; a barrier with no verified footprint remains unknown.
    effect = mir.MemRef(Addr(Space.SEGMENT, 16 if overlap else 18, 1), 2)
    body = body_with_call(effect)
    store, call, load = body.blocks[0].ops
    barrier = replace(call, op=ir.Operation.BARRIER, kind=mir.Kind.OPAQUE, memory_complete=complete)
    body = replace(body, blocks=(replace(body.blocks[0], ops=(store, barrier, load)),))
    assert bool(avail.forwardable(body, frozenset({1}), {}, frozenset({2}))) is reused


def test_complete_write_only_call_does_not_make_prior_store_observable() -> None:
    """A setter with a complete disjoint footprint used to read all memory.

    That kept a caller-local store alive even though the call neither read nor
    wrote it and the following store overwrote it.
    """
    body = body_with_call(mir.MemRef(Addr(Space.SEGMENT, 18, 1), 2))
    store, call, _load = body.blocks[0].ops
    call = replace(call, loads=(), memory_complete=True)
    body = replace(body, blocks=(replace(body.blocks[0], ops=(store, call, replace(store, at=2))),))

    assert avail.dead_stores(body, frozenset({1}), {}) == (store,)
