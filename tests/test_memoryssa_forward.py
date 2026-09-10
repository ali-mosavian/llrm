"""A preheader store must serve a loop read across an unrelated backedge write."""

from qbopt.analysis import avail
from qbopt.model import ir, mir
from qbopt.objectfile.module import Addr, Space
from qbopt.optimize import transform
from dataclasses import replace


def loop_body(alias: bool = False) -> mir.MirBody:
    cell = mir.MemRef(Addr(Space.SEGMENT, 0x20, 1), 2)
    other = cell if alias else mir.MemRef(Addr(Space.SEGMENT, 0x30, 1), 2)
    source, result = mir.Value(1, 0), mir.Value(2, 1)
    store = mir.Op(0, ir.Operation.MOVE, "", (), (source,), stores=(cell,), kind=mir.Kind.STORE)
    load = mir.Op(1, ir.Operation.MOVE, "", (result,), (), loads=(cell,),
                  kind=mir.Kind.LOAD, args=(mir.Cell(cell),))
    write = mir.Op(2, ir.Operation.MOVE, "", (), (), stores=(other,), kind=mir.Kind.STORE)
    return mir.MirBody(0, (
        mir.MirBlock(0, (), (store,), (1,)),
        mir.MirBlock(1, (), (load,), (2, 3)),
        mir.MirBlock(2, (), (write,), (1,)),
        mir.MirBlock(3, (), (), ()),
    ))


def test_preheader_store_serves_a_loop_read() -> None:
    body = loop_body()
    found = avail.forwardable(body, frozenset(), {}, frozenset({1}))
    assert len(found) == 1
    assert found[0].value == body.blocks[0].ops[0].uses[0]
    after = transform.forwarded(body, frozenset(), {})
    assert not after.blocks[1].ops[0].loads
    assert isinstance(after.blocks[1].ops[0].args[0], mir.Held)


def test_aliasing_backedge_keeps_the_load() -> None:
    body = loop_body(alias=True)
    assert not avail.forwardable(body, frozenset(), {}, frozenset({1}))
    assert transform.forwarded(body, frozenset(), {}).blocks[1].ops[0].loads


def test_store_on_only_one_entry_path_cannot_supply_the_load() -> None:
    body = loop_body()
    body = replace(body, entry=4, blocks=body.blocks + (mir.MirBlock(4, (), (), (0, 1)),))
    assert not avail.forwardable(body, frozenset(), {}, frozenset({1}))


def test_call_on_backedge_invalidates_the_preheader_store() -> None:
    body = loop_body()
    call = mir.Op(2, ir.Operation.CALL, "", (), (), kind=mir.Kind.CALL)
    body = replace(body, blocks=tuple(
        replace(block, ops=(call,)) if block.at == 2 else block for block in body.blocks
    ))
    assert not avail.forwardable(body, frozenset(), {}, frozenset({1}))


def test_call_with_proven_disjoint_writes_preserves_the_loop_value() -> None:
    """A runtime call's raised write exclusions must survive MemorySSA queries."""
    body = loop_body()
    cell = body.blocks[0].ops[0].stores[0]
    effect = mir.MemRef(None, 0, excludes=((cell.addr, cell.width),))
    call = mir.Op(2, ir.Operation.CALL, "", (), (), kind=mir.Kind.CALL, stores=(effect,))
    body = replace(body, blocks=tuple(
        replace(block, ops=(call,)) if block.at == 2 else block for block in body.blocks
    ))
    after = transform.forwarded(body, frozenset(), {})
    assert not after.blocks[1].ops[0].loads


def test_call_exclusion_must_cover_the_entire_read() -> None:
    body = loop_body()
    cell = body.blocks[0].ops[0].stores[0]
    effect = mir.MemRef(None, 0, excludes=((cell.addr, 1),))
    call = mir.Op(2, ir.Operation.CALL, "", (), (), kind=mir.Kind.CALL, stores=(effect,))
    body = replace(body, blocks=tuple(
        replace(block, ops=(call,)) if block.at == 2 else block for block in body.blocks
    ))
    assert transform.forwarded(body, frozenset(), {}).blocks[1].ops[0].loads


def test_escaped_object_origin_does_not_exclude_an_interior_read() -> None:
    """An escaped pointer at 0x1e may write the word at 0x20 inside its object."""
    body = loop_body()
    effect = mir.MemRef(None, 0, beyond=(1, frozenset({(1, 0x1e)})))
    call = mir.Op(2, ir.Operation.CALL, "", (), (), kind=mir.Kind.CALL, stores=(effect,))
    body = replace(body, blocks=tuple(
        replace(block, ops=(call,)) if block.at == 2 else block for block in body.blocks
    ))
    assert transform.forwarded(body, frozenset(), {}).blocks[1].ops[0].loads
