"""FPCSE's ten exact iterations should become one checked final iteration."""

from pathlib import Path
from dataclasses import replace

import corpus
import pytest

from qbopt.analysis import floatfacts, loops
from qbopt.model import mir


@pytest.mark.parametrize("tag", ["p-g2", "v-g3"])
def test_emitted_seed_and_final_store_share_the_correct_symbol(tag):
    """FPCSE printed 48.75 instead of 487.5 when its new seed lost its relocation."""
    from qbopt import wholeseg
    path = Path(f"fixtures/omf/fpcse-{tag}.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    accumulator = next(op.stores[0] for block in body.blocks for op in reversed(block.ops)
                       if op.kind is mir.Kind.FSTORE)
    counter = next(op.stores[0] for block in body.blocks if block.phis for op in block.ops if op.stores)
    result = wholeseg.emitted(path.read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    emitted = corpus.loaded(result.data)
    bodies = mir.bodies(emitted, corpus.partitioned(result.data))
    ops = [op for _, body in bodies for block in body.blocks for op in block.ops]
    seed, = [op for op in ops if op.kind is mir.Kind.STORE and op.args == (mir.Const(0x43db6000, 4),)]
    assert seed.stores[0].addr == accumulator.addr
    assert any(op.kind is mir.Kind.FSTORE and op.stores[0].addr == accumulator.addr for op in ops)
    final_counter, = [op for op in ops if op.kind is mir.Kind.STORE and op.args == (mir.Const(11, 2),)]
    assert final_counter.stores[0].addr == counter.addr
    assert all(not loops.loops(body.blocks, body.entry) for _, body in bodies)


@pytest.mark.parametrize("tag", ["p-g2", "v-g3"])
def test_exact_loop_retains_checkpoint_and_final_iteration(tag):
    """FPCSE computes 487.5, but previously repeated its exact FP body ten times."""
    from qbopt.optimize import floatloop
    path = Path(f"fixtures/omf/fpcse-{tag}.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    latch = next(block for block in body.blocks if any(op.floating for op in block.ops))
    changed = floatloop.specialized(body, found.dgroup, found.calls)
    assert changed is not body
    assert not loops.loops(changed.blocks, changed.entry)
    emitted = next(block for block in changed.blocks if block.at == latch.at)
    original = tuple(op for op in latch.ops if op.floating)
    assert tuple(op for op in emitted.ops if op.floating) == original
    assert emitted.ops[0] == original[0]
    accumulator = next(op.stores[0] for op in reversed(latch.ops) if op.kind is mir.Kind.FSTORE)
    seed = next(op for op in emitted.ops if op.kind is mir.Kind.STORE and accumulator in op.stores)
    assert seed.args == (mir.Const(0x43db6000, 4),)  # 438.75 before the final iteration
    facts = floatfacts.known(changed, found.dgroup, found.calls)
    stored = next(op for op in reversed(emitted.ops) if op.kind is mir.Kind.FSTORE)
    assert floatfacts.encoded(facts[stored.args[0].value], stored.floating.result) == 0x43f3c000


@pytest.mark.parametrize("phase", ["sink", "dead"])
def test_checkpoint_keeps_initial_memory_and_counter_stores(phase):
    """A pending FP exception must still see FPCSE's s=0 and first counter value."""
    from qbopt.optimize import floatloop, loopmotion, transform
    path = Path("fixtures/omf/fpcse-p-g2.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    header = next(block for block in body.blocks if block.phis)
    counter_store = next(op for op in header.ops if op.stores)
    if phase == "sink":
        sunk = loopmotion.sunk_stores(body, found.dgroup)
        assert counter_store in next(block for block in sunk.blocks if block.at == header.at).ops
        return
    changed = floatloop.specialized(body, found.dgroup, found.calls)
    changed = transform.without_dead_stores(changed, found.dgroup, found.calls)
    entry = next(block for block in changed.blocks if block.at == changed.entry)
    assert any(op.kind is mir.Kind.STORE and op.args == (mir.Const(0, 4),) for op in entry.ops)


@pytest.mark.parametrize("change", ["inexact", "zero_trip", "one_trip", "call", "flags"])
def test_unproved_or_observable_iterations_remain(change):
    """Do not turn an inexact or externally observed recurrence into a guessed final iteration."""
    from qbopt.optimize import floatloop
    path = Path("fixtures/omf/fpcse-p-g2.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    header = next(block for block in body.blocks if block.phis)
    condition = next(op.defines[0] for op in header.ops if op.kind is mir.Kind.SUB)
    def altered(op):
        if change == "inexact" and op.kind is mir.Kind.STORE and op.args == (mir.Const(0x41000000, 4),):
            return replace(op, args=(mir.Const(0x40e00000, 4),))  # 6/7 is not exact
        if change in {"zero_trip", "one_trip"} and op.kind is mir.Kind.SUB and op.args[-1] == mir.Const(10, 2):
            return replace(op, args=(op.args[0], mir.Const(int(change == "one_trip"), 2)))
        if change == "call" and op.kind is mir.Kind.FADD:
            return replace(op, kind=mir.Kind.CALL, floating=None)
        if change == "flags" and op.kind is mir.Kind.ARG:
            return replace(op, uses=(*op.uses, condition))
        return op
    body = replace(body, blocks=tuple(replace(block, ops=tuple(map(altered, block.ops))) for block in body.blocks))
    assert floatloop.specialized(body, found.dgroup, found.calls) is body
