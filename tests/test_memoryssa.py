"""MemorySSA names memory state across straight lines, joins and loops."""

from qbopt.analysis import memoryssa
from qbopt.model import ir, mir
from qbopt.objectfile.module import Addr, Space


CELL = mir.MemRef(Addr(Space.SEGMENT, 0x20, 1), 2)


def operation(
    at: int, *, loads: tuple[mir.MemRef, ...] = (),
    stores: tuple[mir.MemRef, ...] = (), barrier: bool = False,
) -> mir.Op:
    return mir.Op(
        at,
        ir.Operation.BARRIER if barrier else ir.Operation.MOVE,
        "",
        (),
        (),
        loads=loads,
        stores=stores,
        kind=mir.Kind.OPAQUE if barrier else mir.Kind.LOAD if loads and not stores else mir.Kind.STORE,
    )


def test_a_load_uses_the_nearest_memory_definition() -> None:
    store = operation(0, stores=(CELL,))
    load = operation(1, loads=(CELL,))
    body = mir.MirBody(0, (mir.MirBlock(0, (), (store, load), ()),))

    graph = memoryssa.built(body)
    write = graph.at(memoryssa.Site(0, 0))
    read = graph.at(memoryssa.Site(0, 1))

    assert write.kind is memoryssa.Kind.DEF
    assert write.defining == graph.live.id
    assert read.kind is memoryssa.Kind.USE
    assert read.defining == write.id


def test_a_join_gets_one_memory_phi() -> None:
    left = operation(1, stores=(CELL,))
    right = operation(2, stores=(CELL,))
    load = operation(3, loads=(CELL,))
    body = mir.MirBody(
        0,
        (
            mir.MirBlock(0, (), (), (1, 2)),
            mir.MirBlock(1, (), (left,), (3,)),
            mir.MirBlock(2, (), (right,), (3,)),
            mir.MirBlock(3, (), (load,), ()),
        ),
    )

    graph = memoryssa.built(body)
    phi = graph.phis[3]

    assert phi.kind is memoryssa.Kind.PHI
    assert dict(phi.incoming) == {
        1: graph.at(memoryssa.Site(1, 0)).id,
        2: graph.at(memoryssa.Site(2, 0)).id,
    }
    assert graph.at(memoryssa.Site(3, 0)).defining == phi.id


def test_a_loop_header_phi_carries_the_backedge_definition() -> None:
    load = operation(1, loads=(CELL,))
    store = operation(2, stores=(CELL,))
    body = mir.MirBody(
        0,
        (
            mir.MirBlock(0, (), (), (1,)),
            mir.MirBlock(1, (), (load,), (2, 3)),
            mir.MirBlock(2, (), (store,), (1,)),
            mir.MirBlock(3, (), (), ()),
        ),
    )

    graph = memoryssa.built(body)
    phi = graph.phis[1]

    assert dict(phi.incoming) == {0: graph.live.id, 2: graph.at(memoryssa.Site(2, 0)).id}
    assert graph.at(memoryssa.Site(1, 0)).defining == phi.id


def test_an_opaque_barrier_defines_memory_even_without_named_cells() -> None:
    barrier = operation(0, barrier=True)
    body = mir.MirBody(0, (mir.MirBlock(0, (), (barrier,), ()),))

    access = memoryssa.built(body).at(memoryssa.Site(0, 0))

    assert access.kind is memoryssa.Kind.DEF


def test_a_call_with_no_named_cells_still_defines_memory() -> None:
    call = mir.Op(0, ir.Operation.CALL, "", (), (), kind=mir.Kind.CALL)
    body = mir.MirBody(0, (mir.MirBlock(0, (), (call, operation(1, loads=(CELL,))), ()),))
    graph = memoryssa.built(body)
    assert graph.at(memoryssa.Site(0, 0)).kind is memoryssa.Kind.DEF
    assert graph.at(memoryssa.Site(0, 1)).defining == graph.at(memoryssa.Site(0, 0)).id


def test_entry_backedge_keeps_the_invocation_memory_state() -> None:
    body = mir.MirBody(0, (mir.MirBlock(0, (), (operation(0, stores=(CELL,)),), (0,)),))
    graph = memoryssa.built(body)
    assert dict(graph.phis[0].incoming) == {
        None: graph.live.id, 0: graph.at(memoryssa.Site(0, 0)).id,
    }


def test_read_only_loop_needs_no_memory_phi() -> None:
    body = mir.MirBody(0, (
        mir.MirBlock(0, (), (), (1,)),
        mir.MirBlock(1, (), (operation(1, loads=(CELL,)),), (2,)),
        mir.MirBlock(2, (), (), (1,)),
    ))
    graph = memoryssa.built(body)
    assert not graph.phis
    assert graph.at(memoryssa.Site(1, 0)).defining == graph.live.id
