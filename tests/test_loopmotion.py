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


@pytest.mark.parametrize("nonempty", [False, True])
def test_lngmxx_invariant_temporaries_sink_only_when_loop_executes(monkeypatch, nonempty: bool) -> None:
    """LNGMXX wrote invariant quotient halves ten times; a zero-trip loop must not acquire those stores."""
    from qbopt import transform

    path = Path("fixtures/omf/lngmxx-p-g2.obj")
    found = corpus.loaded(path)
    partition = corpus.partitioned(path)
    body = mir.bodies(found, partition)[0][1]
    with monkeypatch.context() as patch:
        patch.setattr(loopmotion, "sunk_stores", lambda body, *args: body)
        body = transform.applied(body, found.dgroup, found.calls, blocks=partition, found=found)
    if not nonempty:
        body = replace(body, blocks=tuple(
            replace(block, ops=tuple(
                replace(op, args=(op.args[0], mir.Const(0, 2))) if op.at == 0x94 else op
                for op in block.ops
            )) for block in body.blocks
        ))
    hot = {at for loop in loops.loops(body.blocks, body.entry) for at in loop.body}
    def writes(candidate):
        return [op for block in candidate.blocks if block.at in hot for op in block.ops
                if any(ref.addr and ref.addr.space is module.Space.FRAME for ref in op.stores)]
    assert len(writes(body)) == 2
    result = loopmotion.sunk_stores(body, found.dgroup, module.landmarks(found))
    assert len(writes(result)) == (0 if nonempty else 2)


@pytest.mark.parametrize("initialized", [True, False])
def test_nested_accumulator_seed_follows_outer_phi(monkeypatch, initialized: bool) -> None:
    """NESTED stored its accumulator 30 times; an outer phi carries the zero-trip seed."""
    from qbopt import transform

    path = Path("fixtures/omf/nested-p-g2.obj")
    found = corpus.loaded(path)
    partition = corpus.partitioned(path)
    body = mir.bodies(found, partition)[0][1]
    sink = loopmotion.sunk_stores
    with monkeypatch.context() as patch:
        patch.setattr(loopmotion, "sunk_stores", lambda body, *args: body)
        body = transform.applied(body, found.dgroup, found.calls, blocks=partition, found=found)
    accumulator = next(op.stores[0] for block in body.blocks for op in block.ops if op.at == 0x7E and op.stores)
    if not initialized:
        body = replace(
            body,
            blocks=tuple(
                replace(block, ops=tuple(op for op in block.ops if accumulator not in op.stores))
                if block.at == body.entry
                else block
                for block in body.blocks
            ),
        )
    result = sink(body, found.dgroup, module.landmarks(found))
    inner = next(block for block in result.blocks if block.at == 0x5A)
    assert any(accumulator in op.stores for op in inner.ops) is not initialized


def test_nested_accumulator_is_stored_only_after_the_outer_loop() -> None:
    """NESTED wrote its sum once per row after inner-loop sinking; only the exit needs it."""
    from qbopt import transform

    path = Path("fixtures/omf/nested-p-g2.obj")
    found = corpus.loaded(path)
    partition = corpus.partitioned(path)
    body = mir.bodies(found, partition)[0][1]
    accumulator = next(op.stores[0] for block in body.blocks for op in block.ops if op.at == 0x7E)
    body = transform.applied(body, found.dgroup, found.calls, blocks=partition, found=found)
    hot = {at for loop in loops.loops(body.blocks, body.entry) for at in loop.body}
    writes = [block.at for block in body.blocks for op in block.ops if accumulator in op.stores]
    assert not hot.intersection(writes)
    assert writes == [body.entry, 0x9C]


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


def test_an_accumulator_without_zero_trip_initialization_stays_in_the_loop() -> None:
    body, dgroup, bounds, _ = hotlop()
    block = next(block for block in body.blocks if block.at == 0x48)
    refs = {ref for op in block.ops for ref in op.stores}
    body = replace(
        body,
        blocks=tuple(
            replace(one, ops=tuple(op for op in one.ops if not refs.intersection(op.stores)))
            if one.at == body.entry
            else one
            for one in body.blocks
        ),
    )
    result = loopmotion.sunk_stores(body, dgroup, bounds)
    assert next(one for one in result.blocks if one.at == block.at) == block


def test_rotated_accumulator_store_uses_exit_phi() -> None:
    """HOTLOP wrote its accumulator every iteration despite a promoted exit value."""
    body, dgroup, bounds, _ = hotlop()
    latch = next(block for block in body.blocks if block.at == 0x48)
    stores = [op for op in latch.ops if op.kind is mir.Kind.STORE]
    assert stores
    result = loopmotion.sunk_stores(body, dgroup, bounds)
    after = next(block for block in result.blocks if block.at == latch.at)
    exit_block = next(block for block in result.blocks if block.at == 0x66)
    header = next(block for block in result.blocks if block.at == 0x5E)
    for original in stores:
        assert original not in after.ops
        moved = next(op for op in exit_block.ops if op.stores == original.stores)
        value = next(arg.value for arg in moved.args if isinstance(arg, mir.Held))
        phi = next(phi for phi in header.phis if phi.result == value)
        assert set(phi.incoming) == {body.entry, latch.at}
        assert moved.covers == original.covers
