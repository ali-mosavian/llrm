"""HARR should not advance a counter used only to test the loop bound."""

from pathlib import Path

import pytest

import corpus
from qbopt import wholeseg
from qbopt.analysis import loops


@pytest.mark.xfail(reason="IVWORD's invariant branch load stays in the loop on p-g2 and q-O", strict=True)
@pytest.mark.parametrize("tag", ["p-g2", "q-O"])
def test_invariant_branch_load_moves_out_but_its_test_stays(tag, monkeypatch):
    """IVWORD reloaded unchanged branchChoice every trip because its test prevented LICM."""
    from qbopt.optimize import unswitch

    monkeypatch.setattr(unswitch, "optimized", lambda body, *args, **kwargs: body)
    result = wholeseg.emitted(Path(f"fixtures/regressions/ivword-{tag}.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    blocks = corpus.partitioned(result.data)
    inside = set().union(*(loop.body for loop in loops.loops(blocks)))
    assert inside
    instructions = [str(one.insn) for block in blocks if block.at in inside for one in block.insns]
    assert not any(one.startswith("mov ") and "[" in one.split(",", 1)[-1] for one in instructions)
    assert any(one.startswith(("and ", "test ", "or ")) for one in instructions)


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
@pytest.mark.parametrize("program", ["ivarm", "ivword"])
def test_internal_branch_reuses_the_value_recurrence(tag, program, monkeypatch):
    """IVARM kept a second counter solely for ten trips around a conditional store."""
    from qbopt.optimize import unswitch

    monkeypatch.setattr(unswitch, "optimized", lambda body, *args, **kwargs: body)
    result = wholeseg.emitted(Path(f"fixtures/regressions/{program}-{tag}.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    instructions = [str(one.insn) for block in corpus.partitioned(result.data) for one in block.insns]
    assert not any(one.startswith("inc ") for one in instructions)
    comparisons = [index for index, one in enumerate(instructions) if one.startswith("cmp ") and one.endswith(",25h")]
    assert len(comparisons) == 1
    assert instructions[comparisons[0] + 1].startswith("jne ")


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
@pytest.mark.parametrize("program,branches,increments", [("harr", 1, 1), ("matrix", 2, 1), ("nested", 2, 0)])
def test_harr_reuses_an_existing_recurrence_for_termination(
    tag: str, program: str, branches: int, increments: int
) -> None:
    """HARR advanced both c and r+c on each inner iteration; one recurrence suffices."""
    result = wholeseg.emitted(Path(f"fixtures/omf/{program}-{tag}.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    instructions = [str(one.insn) for block in corpus.partitioned(result.data) for one in block.insns]
    assert sum(one.startswith("jne ") for one in instructions) == branches
    assert sum(one.startswith("inc ") for one in instructions) == increments


def test_harr_initializes_the_reused_counter_before_its_exit_bound():
    """HARR printed 12327 instead of 1100 when the bound read SI before SI was initialized."""
    result = wholeseg.emitted(Path("fixtures/omf/harr-v-g3.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    instructions = [str(one.insn) for block in corpus.partitioned(result.data) for one in block.insns]
    bound = instructions.index("mov dx,si")
    assert any(text.startswith("mov si,") for text in instructions[:bound]), instructions[:bound]


def test_indvar_simplify_reads_through_an_lcssa_exit(monkeypatch) -> None:
    """LCSSA made HARR's redundant loop counter look externally observed."""
    from qbopt.model import mir
    from qbopt.optimize import lcssa
    from qbopt.optimize import indvars
    from qbopt.optimize import transform

    path = Path("fixtures/omf/harr-v-g3.obj")
    found = corpus.loaded(path)
    partition = corpus.partitioned(path)
    with monkeypatch.context() as context:
        context.setattr(indvars, "simplified", lambda body: body)
        body = transform.applied(
            mir.bodies(found, partition)[0][1],
            found.dgroup,
            found.calls,
            blocks=partition,
            found=found,
            lcssa_=False,
        )
    closed = lcssa.closed(body)

    assert closed != body
    assert indvars.simplified(closed) != closed


@pytest.mark.parametrize("hazard", ["observed-counter", "zero-trip", "wrapping-exit", "short-period"])
def test_counter_elimination_requires_a_complete_trip_count_and_no_body_use(monkeypatch, hazard):
    from dataclasses import replace

    from qbopt.model import mir
    from qbopt.analysis import loops
    from qbopt.analysis import consts
    from qbopt.optimize import indvars
    from qbopt.analysis import induction
    from qbopt.optimize import transform

    path = Path("fixtures/omf/harr-v-g3.obj")
    found = corpus.loaded(path)
    partition = corpus.partitioned(path)
    with monkeypatch.context() as context:
        # Counting to zero rewrites the very compare each hazard edits.
        context.setattr(indvars, "simplified", lambda body: body)
        context.setattr(indvars, "zeroed", lambda body: body)
        body = transform.applied(
            mir.bodies(found, partition)[0][1], found.dgroup, found.calls, blocks=partition, found=found
        )
    loop = next(loop for loop in loops.loops(body.blocks, body.entry) if len(loop.body) == 2)
    facts = consts.known(body)
    counter = next(
        counter
        for counter in induction.basics(body, loop).values()
        if induction._last_counter(body, loop, counter, facts, counter.start.width) is not None
    )
    header = next(block for block in body.blocks if block.at == loop.header)
    value = next(phi.result for phi in header.phis if phi.result.id == counter.value)
    alternative_updates = {
        phi.incoming[next(iter(loop.latches))] for phi in header.phis if phi.result.id != counter.value
    }
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


def _trip_counts(data: bytes) -> list[int]:
    """How often each emitted counted loop runs: its counter's start, step and exit test, simulated."""
    from iced_x86 import Mnemonic
    from iced_x86 import OpKind

    insns = [one.insn for block in corpus.partitioned(data) for one in block.insns]
    immediates = (OpKind.IMMEDIATE8, OpKind.IMMEDIATE8TO16, OpKind.IMMEDIATE16)
    counts = []
    for index, branch in enumerate(insns):
        if branch.mnemonic not in (Mnemonic.JLE, Mnemonic.JL, Mnemonic.JNE) or branch.near_branch_target >= branch.ip:
            continue
        inside = [one for one in insns[:index] if one.ip >= branch.near_branch_target]
        steps = [
            one for one in inside if one.mnemonic in (Mnemonic.INC, Mnemonic.DEC) and one.op0_kind == OpKind.REGISTER
        ]
        if not steps:
            continue
        step = steps[-1]
        register, delta = step.op0_register, 1 if step.mnemonic == Mnemonic.INC else -1
        # The counter can move between registers inside the loop: `mov dx,cx / inc dx / mov cx,dx`.
        held = {register} | {
            one.op1_register for one in inside
            if one.mnemonic == Mnemonic.MOV and one.op1_kind == OpKind.REGISTER and one.op0_register == register
        }
        test = insns[index - 1]
        if test.mnemonic == Mnemonic.MOV and test.op0_kind == OpKind.REGISTER and test.op0_register in held:
            test = insns[index - 2]
        if test.mnemonic == Mnemonic.CMP and test.op0_kind == OpKind.REGISTER and test.op0_register in held \
                and test.op1_kind in immediates:
            bound = (test.immediate16 ^ 0x8000) - 0x8000
        # `or r,r` tests for zero as `test r,r` does; the peephole writes it for `cmp r,0`.
        elif test is step or (test.mnemonic in (Mnemonic.TEST, Mnemonic.OR) and test.op0_register in held
                              and test.op0_kind == test.op1_kind == OpKind.REGISTER
                              and test.op0_register == test.op1_register):
            bound = 0
        else:
            continue
        before = [one for one in insns[:index] if one.ip < branch.near_branch_target]
        start = next((one for one in reversed(before)
                      if one.mnemonic == Mnemonic.MOV and one.op0_kind == OpKind.REGISTER
                      and one.op0_register in held and one.op1_kind in immediates), None)
        if start is None:
            continue
        value, trips = (start.immediate16 ^ 0x8000) - 0x8000, 0
        while trips < 1 << 17:
            trips += 1
            value = ((value + delta + 0x8000) & 0xFFFF) - 0x8000
            taken = {Mnemonic.JLE: value <= bound, Mnemonic.JL: value < bound, Mnemonic.JNE: value != bound}
            if not taken[branch.mnemonic]:
                break
        counts.append(trips)
    return sorted(counts)


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
@pytest.mark.parametrize("program,trips", [("spill", [10]), ("segld", [5, 20])])
def test_counting_one_loop_to_zero_leaves_a_loop_sharing_its_start_alone(tag, program, trips):
    """SPILL printed T= 4620 and SEGLD T= 975: both loops of each nest start at 1, one
    constant, and counting one to zero rewrote that constant, so the other ran from -10
    (or -5) up to its own bound."""
    result = wholeseg.emitted(Path(f"fixtures/omf/{program}-{tag}.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    assert _trip_counts(result.data) == trips
