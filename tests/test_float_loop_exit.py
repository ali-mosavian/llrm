"""FPCSE's ten exact iterations should become one checked final iteration."""

from pathlib import Path
from dataclasses import replace

import pytest

import corpus
from qbopt.model import mir
from qbopt.analysis import loops
from qbopt.analysis import floatfacts


@pytest.mark.parametrize("effect", ["value", "memory", "barrier"])
def test_checkpoint_with_additional_effects_is_not_ignored(effect):
    from qbopt.model import ir

    op = mir.Op(0, ir.Operation.NOTHING, "", (), (), kind=mir.Kind.FCHECK)
    match effect:
        case "value":
            op = replace(op, defines=(mir.Value(1, 0),))
        case "memory":
            op = replace(op, stores=(mir.MemRef(None, 2),))
        case "barrier":
            op = replace(op, op=ir.Operation.BARRIER)
    assert not floatfacts.checkpoint(op)
    assert floatfacts.repeated((op,), 1, {}, frozenset()) is None


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_emitted_final_answer_has_the_correct_symbol(tag):
    """FPCSE seeds 438.75 and retains the strict final iteration to reach 487.5."""
    from qbopt import wholeseg

    path = Path(f"fixtures/omf/fpcse-{tag}.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    accumulator = next(
        op.stores[0] for block in body.blocks for op in reversed(block.ops) if op.kind is mir.Kind.FSTORE
    )
    counter = next(op.stores[0] for block in body.blocks if block.phis for op in block.ops if op.stores)
    states = []

    def watch(stage, name, state):
        if stage == "mir-r01-floatloop" and isinstance(state, mir.MirBody):
            states.append(state)

    result = wholeseg.emitted(path.read_bytes(), watch=watch)
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    (specialized,) = states
    assert not loops.loops(specialized.blocks, specialized.entry)
    ops = [op for block in specialized.blocks for op in block.ops]
    (seed,) = [op for op in ops if op.kind is mir.Kind.STORE and op.args == (mir.Const(0x43DB6000, 4),)]
    assert seed.stores[0].addr == accumulator.addr and seed.symbol
    assert any(op.kind is mir.Kind.FSTORE and accumulator in op.stores for op in ops)
    (final_counter,) = [op for op in ops if op.kind is mir.Kind.STORE and op.args == (mir.Const(11, 2),)]
    assert final_counter.stores[0].addr == counter.addr


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_exact_loop_retains_checkpoint_and_final_iteration(tag):
    """FPCSE computes 487.5, but previously repeated its exact FP body ten times."""
    from qbopt.optimize import floatloop

    path = Path(f"fixtures/omf/fpcse-{tag}.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    latch = body.block(next(iter(loops.loops(body.blocks, body.entry)[0].latches)))
    changed = floatloop.specialized(body, found.dgroup, found.calls)
    assert changed is not body
    assert not loops.loops(changed.blocks, changed.entry)
    emitted = next(block for block in changed.blocks if block.at == latch.at)
    original = tuple(op for op in latch.ops if op.floating)
    assert tuple(op for op in emitted.ops if op.floating) == original
    assert tuple(op for op in emitted.ops if op.floating or op.kind is mir.Kind.FCHECK) == tuple(
        op for op in latch.ops if op.floating or op.kind is mir.Kind.FCHECK
    )
    assert emitted.ops[0] == original[0]
    accumulator = next(op.stores[0] for op in reversed(latch.ops) if op.kind is mir.Kind.FSTORE)
    seed = next(op for op in emitted.ops if op.kind is mir.Kind.STORE and accumulator in op.stores)
    assert seed.args == (mir.Const(0x43DB6000, 4),)  # 438.75 before the final iteration
    facts = floatfacts.known(changed, found.dgroup, found.calls)
    stored = next(op for op in reversed(emitted.ops) if op.kind is mir.Kind.FSTORE)
    assert floatfacts.encoded(facts[stored.args[0].value], stored.floating.result) == 0x43F3C000


def test_floatloop_can_be_disabled_for_stage_bisection():
    """FPCSE's generic-unroll test needs the loop before FloatLoop consumes it.

    Every MIR pass must be individually suppressible: otherwise a stage dump
    cannot distinguish an earlier exact specialization from a later unroll.
    """
    from qbopt.optimize import transform

    path = Path("fixtures/omf/fpcse-p-g2.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    enabled = transform.applied(body, found.dgroup, found.calls, found=found, only="floatloop")
    disabled = transform.applied(
        body,
        found.dgroup,
        found.calls,
        found=found,
        floatloop_=False,
        only="floatloop",
    )
    assert not loops.loops(enabled.blocks, enabled.entry)
    assert loops.loops(disabled.blocks, disabled.entry)


@pytest.mark.parametrize("handles_errors", [True, False])
@pytest.mark.parametrize("phase", ["sink", "dead"])
def test_checkpoint_keeps_initial_memory_and_counter_stores_for_an_error_handler(phase, handles_errors):
    """A pending FP exception reaching ON ERROR must still see FPCSE's s=0 and first counter value."""
    from qbopt.optimize import floatloop
    from qbopt.optimize import transform
    from qbopt.optimize import loopmotion

    path = Path("fixtures/omf/fpcse-p-g2.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    header = next(block for block in body.blocks if block.phis)
    counter_store = next(op for op in header.ops if op.stores)
    if phase == "sink":
        sunk = loopmotion.sunk_stores(body, found.dgroup, handles_errors=handles_errors)
        assert (counter_store in next(block for block in sunk.blocks if block.at == header.at).ops) == handles_errors
        return
    changed = floatloop.specialized(body, found.dgroup, found.calls)
    changed = transform.without_dead_stores(changed, found.dgroup, found.calls, handles_errors=handles_errors)
    entry = next(block for block in changed.blocks if block.at == changed.entry)
    kept = any(op.kind is mir.Kind.STORE and op.args == (mir.Const(0, 4),) for op in entry.ops)
    assert kept or not handles_errors


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
            return replace(op, args=(mir.Const(0x40E00000, 4),))  # 6/7 is not exact
        if change in {"zero_trip", "one_trip"} and op.kind is mir.Kind.SUB and op.args[-1] == mir.Const(10, 2):
            return replace(op, args=(op.args[0], mir.Const(int(change == "one_trip"), 2)))
        if change == "call" and op.kind is mir.Kind.FADD:
            return replace(op, kind=mir.Kind.CALL, floating=None)
        if change == "flags" and op.kind is mir.Kind.ARG:
            return replace(op, uses=(*op.uses, condition))
        return op

    body = replace(body, blocks=tuple(replace(block, ops=tuple(map(altered, block.ops))) for block in body.blocks))
    assert floatloop.specialized(body, found.dgroup, found.calls) is body
