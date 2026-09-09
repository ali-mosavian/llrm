from dataclasses import replace
from pathlib import Path

import pytest

import corpus
from qbopt import loops
from qbopt import loopmotion
from qbopt import mir
from qbopt import module
from qbopt import promote
from qbopt import runtime


def hotlop() -> tuple[mir.MirBody, frozenset[int], dict, mir.MemRef]:
    path = Path("fixtures/omf/hotlop-p-g2.obj")
    found = corpus.loaded(path)
    assert found is not None
    body = mir.bodies(found, corpus.partitioned(path), runtime.for_module(found))[0][1]
    counter = next(op.stores[0] for block in body.blocks for op in block.ops if op.at == 0x5E)
    bounds = module.landmarks(found)
    return promote.promoted(body, found.dgroup, bounds), found.dgroup, bounds, counter


def test_counter_is_written_once_at_the_exit_not_every_iteration() -> None:
    body, dgroup, bounds, counter = hotlop()
    result = loopmotion.sunk_stores(body, dgroup, bounds)
    hot = {at for loop in loops.loops(result.blocks, result.entry) for at in loop.body}
    writes = [block.at for block in result.blocks for op in block.ops if counter in op.stores]
    assert writes == [0x66]
    assert not hot.intersection(writes)
    assert [(op.kind, op.args) for block in body.blocks for op in block.ops if counter in op.stores] == [
        (op.kind, op.args) for block in result.blocks for op in block.ops if counter in op.stores
    ]


def test_emitted_exit_store_still_addresses_the_counter() -> None:
    _, _, _, counter = hotlop()
    output, _ = corpus.rewritten(Path("fixtures/omf/hotlop-p-g2.obj"), dry_run=False)
    found = corpus.loaded(output)
    assert found is not None
    bodies = mir.bodies(found, corpus.partitioned(output), runtime.for_module(found))
    stores = [ref for _, body in bodies for block in body.blocks for op in block.ops for ref in op.stores]
    assert sum(ref.addr == counter.addr for ref in stores) == 1


@pytest.mark.parametrize("observer", ["read", "write", "call", "escape", "opaque"])
def test_an_observer_in_the_loop_keeps_the_store(observer: str) -> None:
    body, dgroup, bounds, counter = hotlop()
    header = next(block for block in body.blocks if block.at == 0x5E)
    store = next(op for op in header.ops if counter in op.stores)
    kind = {
        "read": mir.Kind.LOAD,
        "write": mir.Kind.STORE,
        "call": mir.Kind.CALL,
        "escape": mir.Kind.ESCAPE,
        "opaque": mir.Kind.OPAQUE,
    }[observer]
    extra = replace(
        store,
        kind=kind,
        loads=(counter,) if observer == "read" else (),
        stores=(counter,) if observer == "write" else (),
    )
    changed = replace(header, ops=(extra,) + header.ops)
    body = replace(body, blocks=tuple(changed if block.at == header.at else block for block in body.blocks))
    result = loopmotion.sunk_stores(body, dgroup, bounds)
    assert next(block for block in result.blocks if block.at == header.at).ops == changed.ops


def test_an_exit_reachable_without_the_store_gets_no_new_write() -> None:
    body, dgroup, bounds, _ = hotlop()
    body = replace(
        body,
        blocks=tuple(
            replace(block, succ=block.succ + (0x66,)) if block.at == body.entry else block for block in body.blocks
        ),
    )
    assert loopmotion.sunk_stores(body, dgroup, bounds) == body


def test_a_conditional_accumulator_store_stays_in_the_loop() -> None:
    body, dgroup, bounds, _ = hotlop()
    block = next(block for block in body.blocks if block.at == 0x48)
    result = loopmotion.sunk_stores(body, dgroup, bounds)
    assert next(one for one in result.blocks if one.at == block.at) == block
