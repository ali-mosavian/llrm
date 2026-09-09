"""HARR should not advance a counter used only to test the loop bound."""

from pathlib import Path

import pytest
import corpus

from qbopt import wholeseg


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
@pytest.mark.parametrize("program,branches", [("harr", 1), ("matrix", 2), ("nested", 1)])
def test_harr_reuses_an_existing_recurrence_for_termination(tag, program, branches):
    """HARR advanced both c and r+c on each inner iteration; one recurrence suffices."""
    result = wholeseg.emitted(Path(f"fixtures/omf/{program}-{tag}.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    instructions = [str(one.insn) for block in corpus.partitioned(result.data) for one in block.insns]
    assert sum(one.startswith("jne ") for one in instructions) == branches
    assert sum(one.startswith("inc ") for one in instructions) == 1


def test_harr_initializes_the_reused_counter_before_its_exit_bound():
    """HARR printed 12327 instead of 1100 when the bound read SI before SI was initialized."""
    result = wholeseg.emitted(Path("fixtures/omf/harr-v-g3.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    instructions = [str(one.insn) for block in corpus.partitioned(result.data) for one in block.insns]
    assert instructions.index("mov si,ax") < instructions.index("mov dx,si")


@pytest.mark.parametrize("hazard", ["observed-counter", "zero-trip", "wrapping-exit", "short-period"])
def test_counter_elimination_requires_a_complete_trip_count_and_no_body_use(monkeypatch, hazard):
    from dataclasses import replace
    from qbopt.analysis import consts, induction, loops
    from qbopt.model import mir
    from qbopt.optimize import indvars, transform
    path = Path("fixtures/omf/harr-v-g3.obj")
    found = corpus.loaded(path)
    partition = corpus.partitioned(path)
    with monkeypatch.context() as context:
        context.setattr(indvars, "simplified", lambda body: body)
        body = transform.applied(mir.bodies(found, partition)[0][1], found.dgroup,
                                 found.calls, blocks=partition, found=found)
    loop = next(loop for loop in loops.loops(body.blocks, body.entry) if len(loop.body) == 2)
    facts = consts.known(body)
    counter = next(counter for counter in induction.basics(body, loop).values()
                   if induction._last_counter(body, loop, counter, facts, counter.start.width) is not None)
    header = next(block for block in body.blocks if block.at == loop.header)
    value = next(phi.result for phi in header.phis if phi.result.id == counter.value)
    alternative_updates = {phi.incoming[next(iter(loop.latches))] for phi in header.phis
                           if phi.result.id != counter.value}
    changed = []
    for block in body.blocks:
        ops = []
        for op in block.ops:
            if hazard == "observed-counter" and block.at in loop.body and op.stores:
                op = replace(op, args=(mir.Held(value, 2),), uses=(*op.uses, value))
            elif hazard == "short-period" and alternative_updates.intersection(op.defines) and len(op.args) == 2:
                op = replace(op, args=(op.args[0], mir.Const(32768, 2)))
            elif hazard in ("zero-trip", "wrapping-exit") and block.at == header.at and op.kind is mir.Kind.SUB:
                op = replace(op, args=(op.args[0], mir.Const(0 if hazard == "zero-trip" else 32767, 2)))
            ops.append(op)
        changed.append(replace(block, ops=tuple(ops)))
    body = replace(body, blocks=tuple(changed))
    assert indvars.simplified(body) is body
