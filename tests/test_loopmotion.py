from pathlib import Path
from dataclasses import replace

import pytest

import corpus
from qbopt.model import mir
from qbopt.abi import runtime
from qbopt.analysis import loops
from qbopt.optimize import promote
from qbopt.objectfile import module
from qbopt.optimize import loopmotion


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_indexed_record_accumulators_store_only_after_loop(tag):
    """UDTRNG wrote both promoted record accumulators on every iteration instead of once at exit.

    This was a strict xfail until scalar promotion and loop cleanup began
    removing the intermediate stores for all three compiler layouts.  Keep it
    as an ordinary production regression: losing that cooperation is a real
    hot-loop regression, not an optional optimization.
    """
    from qbopt import wholeseg

    states = []

    def watch(stage, name, body):
        if isinstance(body, mir.MirBody):
            states.append(body)

    result = wholeseg.emitted(Path(f"fixtures/regressions/udtrng-{tag}.obj").read_bytes(), watch=watch)
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    body = states[-1]
    hot = {at for loop in loops.loops(body.blocks, body.entry) for at in loop.body}
    assert not [
        (op.at, ref)
        for block in body.blocks
        if block.at in hot
        for op in block.ops
        for ref in op.stores
        if ref.base is not None and ref.width == 4
    ]


@pytest.mark.parametrize("address", ["invariant", "counter", "undefined"])
def test_indexed_exit_store_requires_a_dominating_invariant_address(address):
    """UDTRNG's final store may use its pre-loop address, never a changing or uncomputed index."""
    from qbopt import wholeseg

    def watch(stage, name, body):
        nonlocal captured
        if stage == "mir-r02-hoist" and isinstance(body, mir.MirBody):
            captured = body

    captured = None
    with pytest.MonkeyPatch.context() as patch:
        # Capture after scalar promotion made the record fields independent
        # recurrences, but before the automatic sink or later exact-loop
        # cloning erases the CFG.
        patch.setattr(loopmotion, "sunk_stores", lambda body, *args: body)
        result = wholeseg.emitted(Path("fixtures/regressions/udtrng-p-g2.obj").read_bytes(), watch=watch)
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    assert captured is not None
    body = captured
    (loop,) = loops.loops(body.blocks, body.entry)
    store = [
        op
        for block in body.blocks
        if block.at in loop.body
        for op in block.ops
        if op.stores and op.stores[0].base is not None and op.stores[0].width == 4
    ][-1]
    reference = store.stores[0]
    if address != "invariant":
        base = body.block(loop.header).phis[0].result if address == "counter" else mir.Value(99999, 0)
        altered = replace(reference, base=base)
        changed = replace(
            store,
            stores=(altered,),
            results=(mir.Cell(altered),),
            uses=tuple(base if value == reference.base else value for value in store.uses),
        )
        body = replace(
            body,
            blocks=tuple(
                replace(block, ops=tuple(changed if op is store else op for op in block.ops)) for block in body.blocks
            ),
        )
    found = corpus.loaded(Path("fixtures/regressions/udtrng-p-g2.obj"))
    after = loopmotion.sunk_stores(body, found.dgroup, module.landmarks(found))
    remaining = [
        op for block in after.blocks if block.at in loop.body for op in block.ops if op.at == store.at and op.stores
    ]
    assert bool(remaining) is (address != "invariant")


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
@pytest.mark.parametrize("nonempty", [False, True])
def test_harr_constant_column_exit_is_stored_once_only_after_a_nonempty_loop(tag, nonempty, monkeypatch):
    """HARR wrote c=11 once per row after its inner counter became a constant."""
    from qbopt.optimize import transform

    path = Path(f"fixtures/omf/harr-{tag}.obj")
    found = corpus.loaded(path)
    partition = corpus.partitioned(path)
    body = mir.bodies(found, partition)[0][1]
    with monkeypatch.context() as patch:
        patch.setattr(loopmotion, "_invariant_value", lambda *args: None, raising=False)
        body = transform.applied(body, found.dgroup, found.calls, blocks=partition, found=found)
    hot = {at for loop in loops.loops(body.blocks, body.entry) for at in loop.body}
    stores = [
        op
        for block in body.blocks
        if block.at in hot
        for op in block.ops
        if op.kind is mir.Kind.STORE and op.args == (mir.Const(11, 2),)
    ]
    assert len(stores) == 1
    if not nonempty:
        monkeypatch.setattr(loopmotion.induction, "nonempty", lambda *args: False)
    result = loopmotion.sunk_stores(body, found.dgroup, module.landmarks(found))
    remaining = [op for block in result.blocks if block.at in hot for op in block.ops if op.id == stores[0].id]
    assert bool(remaining) is not nonempty
    assert sum(op.id == stores[0].id for block in result.blocks for op in block.ops) == 1


@pytest.mark.parametrize("proof", ["complete", "no_seed", "no_bounds"])
def test_nbody_conditional_accumulator_stores_sink(monkeypatch, proof):
    """Nbody's promoted accumulators still wrote memory on every other-body update."""
    from qbopt.optimize import transform

    path = Path("fixtures/regressions/nbody-stack-p-g2.obj")
    found = corpus.loaded(path)
    partition = corpus.partitioned(path)
    body = mir.bodies(found, partition)[0][1]
    with monkeypatch.context() as patch:
        patch.setattr(loopmotion, "sunk_stores", lambda body, *args: body)
        # Needs the stores drop_stores proves unobservable.
        patch.setattr("qbopt.analysis.observers.private", lambda *args: None)
        body = transform.applied(body, found.dgroup, found.calls, blocks=partition, found=found)
    stores = [op for block in body.blocks for op in block.ops if op.at in (0x1E1, 0x211) and op.stores]
    assert len(stores) == 2
    if proof == "no_seed":
        body = replace(
            body,
            blocks=tuple(
                replace(block, ops=tuple(op for op in block.ops if not (op.at in (0xF0, 0xFC) and op.stores)))
                for block in body.blocks
            ),
        )
    if proof == "no_bounds":
        monkeypatch.setattr(loopmotion.ranges, "bounded", lambda body: {})

        # Raising now supplies an independent bounded-address proof as well.
        def strip(op):
            def argument(arg):
                return mir.Cell(replace(arg.ref, excludes=())) if isinstance(arg, mir.Cell) else arg

            return replace(
                op,
                loads=tuple(replace(ref, excludes=()) for ref in op.loads),
                stores=tuple(replace(ref, excludes=()) for ref in op.stores),
                args=tuple(map(argument, op.args)),
                results=tuple(map(argument, op.results)),
            )

        body = replace(body, blocks=tuple(replace(block, ops=tuple(map(strip, block.ops))) for block in body.blocks))
    from qbopt.abi import runtime

    handles_errors = runtime.handles_errors(map(runtime.contract, found.calls.values()))
    bounds = None if proof == "no_bounds" else module.landmarks(found)
    result = loopmotion.sunk_stores(body, found.dgroup, bounds, handles_errors)
    for store in stores:
        owners = [block.at for block in result.blocks for op in block.ops if op.id == store.id]
        assert owners == ([0x227] if proof == "complete" else [0x117])


@pytest.mark.parametrize("nonempty", [False, True])
def test_lngmxx_invariant_temporaries_sink_only_when_loop_executes(monkeypatch, nonempty: bool) -> None:
    """LNGMXX wrote invariant quotient halves ten times; a zero-trip loop must not acquire those stores."""
    from qbopt.optimize import transform

    path = Path("fixtures/omf/lngmxx-p-g2.obj")
    found = corpus.loaded(path)
    partition = corpus.partitioned(path)
    body = mir.bodies(found, partition)[0][1]
    with monkeypatch.context() as patch:
        patch.setattr(loopmotion, "sunk_stores", lambda body, *args: body)
        # Needs the stores drop_stores proves unobservable.
        patch.setattr("qbopt.analysis.observers.private", lambda *args: None)
        body = transform.applied(body, found.dgroup, found.calls, blocks=partition, found=found)
    if not nonempty:
        body = replace(
            body,
            blocks=tuple(
                replace(
                    block,
                    ops=tuple(
                        replace(op, args=(op.args[0], mir.Const(0, 2))) if op.at == 0x94 else op for op in block.ops
                    ),
                )
                for block in body.blocks
            ),
        )
    hot = {at for loop in loops.loops(body.blocks, body.entry) for at in loop.body}

    def writes(candidate):
        return [
            op
            for block in candidate.blocks
            if block.at in hot
            for op in block.ops
            if any(ref.addr and ref.addr.space is module.Space.FRAME for ref in op.stores)
        ]

    before = writes(body)
    assert sum(op.stores[0].width for op in before) == 4
    result = loopmotion.sunk_stores(body, found.dgroup, module.landmarks(found))
    assert writes(result) == ([] if nonempty else before)


@pytest.mark.parametrize("initialized", [True, False])
def test_nested_accumulator_seed_follows_outer_phi(monkeypatch, initialized: bool) -> None:
    """NESTED stored its accumulator 30 times; an outer phi carries the zero-trip seed."""
    from qbopt.optimize import transform

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
    """NESTED's accumulator is sunk past the outer loop or folded to its final 675."""
    from qbopt.optimize import transform

    path = Path("fixtures/omf/nested-p-g2.obj")
    found = corpus.loaded(path)
    partition = corpus.partitioned(path)
    body = mir.bodies(found, partition)[0][1]
    accumulator = next(op.stores[0] for block in body.blocks for op in block.ops if op.at == 0x7E)
    body = transform.applied(body, found.dgroup, found.calls, blocks=partition, found=found)
    hot = {at for loop in loops.loops(body.blocks, body.entry) for at in loop.body}
    writes = [block.at for block in body.blocks for op in block.ops if accumulator in op.stores]
    assert not hot.intersection(writes)
    if not hot:
        assert not writes
        assert any(
            arg == mir.Const(675, arg.width)
            for block in body.blocks
            for op in block.ops
            if op.kind is mir.Kind.ARG
            for arg in op.args
            if isinstance(arg, mir.Const)
        )
        assert any(count == 6 for _, count in body.repetitions)
        return
    assert writes == [body.entry, 0x9C]


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
@pytest.mark.parametrize("initialization", ["complete", "missing", "clobbered"])
def test_addrm_exit_store_requires_complete_initial_memory(tag, initialization, monkeypatch):
    """ADDRM wrote u 20 times; moving it must preserve memory even when the loop takes zero trips."""
    from qbopt.optimize import transform

    path = Path(f"fixtures/omf/addrm-{tag}.obj")
    found = corpus.loaded(path)
    partition = corpus.partitioned(path)
    body = mir.bodies(found, partition)[0][1]
    accumulator = next(
        ref for block in body.blocks for op in block.ops for ref in op.loads if ref.width == 4 and ref.base is None
    )
    with monkeypatch.context() as patch:
        patch.setattr(loopmotion, "sunk_stores", lambda body, *args: body)
        body = transform.applied(body, found.dgroup, found.calls, blocks=partition, found=found)
    entry = next(block for block in body.blocks if block.at == body.entry)
    if initialization == "missing":
        entry = replace(
            entry,
            ops=tuple(
                op for op in entry.ops if not any(mir.overlapping(ref, accumulator, found.dgroup) for ref in op.stores)
            ),
        )
    elif initialization == "clobbered":
        clobber = replace(
            entry.ops[-1],
            kind=mir.Kind.OPAQUE,
            defines=(),
            uses=(),
            args=(),
            results=(),
            loads=(),
            stores=(),
            source_backed=False,
        )
        entry = replace(entry, ops=entry.ops[:-1] + (clobber, entry.ops[-1]))
    body = replace(body, blocks=tuple(entry if block.at == entry.at else block for block in body.blocks))
    result = loopmotion.sunk_stores(body, found.dgroup, module.landmarks(found))
    hot = {at for loop in loops.loops(result.blocks, result.entry) for at in loop.body}
    writes = [block.at for block in result.blocks for op in block.ops if accumulator in op.stores]
    assert bool(hot.intersection(writes)) is (initialization != "complete")
    if initialization == "complete":
        assert any(at not in hot and at != body.entry for at in writes)


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


def test_a_float_loop_sinks_its_counter_store_without_an_error_handler():
    """NBODYS stored `other` every inner iteration: a float op kept the loop's stores for a handler it has not got."""
    from pathlib import Path

    from qbopt import wholeseg
    from qbopt.model import mir
    from qbopt.analysis import loops

    final = {}
    data = Path("fixtures/bench/nbodys-v-g3.obj").read_bytes()
    wholeseg.emitted(
        data,
        native_fpu=True,
        watch=lambda stage, name, low: final.__setitem__(name, low) if stage.startswith("mir-") else None,
    )
    body = next(
        one for one in final.values() if any(op.kind is mir.Kind.FMUL for block in one.blocks for op in block.ops)
    )
    blocks = {block.at: block for block in body.blocks}
    floating = [
        loop
        for loop in loops.loops(body.blocks, body.entry)
        if any(op.floating for at in loop.body for op in blocks[at].ops)
    ]
    innermost = min(floating, key=lambda loop: len(loop.body))
    assert not any(op.kind is mir.Kind.STORE for at in innermost.body for op in blocks[at].ops)
