from pathlib import Path
from dataclasses import replace

import pytest
from iced_x86 import FlowControl

from qbopt import flow
from tests import corpus
from qbopt.model import lir
from qbopt.model import mir
from qbopt.abi import runtime
from qbopt.backend import frame
from qbopt.backend import lower
from qbopt.abi import nativecalls
from qbopt.frontend import blocks
from qbopt.frontend import extent
from qbopt.backend import prologue
from qbopt.backend import nativeframe


@pytest.mark.parametrize(
    "start,floor,reserve", [(0, -30, 8), (0x334, -50, 0x33C), (0x604, -76, 0x60C), (0x6CB, 0, 0x6CE)]
)
def test_native_spills_start_below_locals_and_saved_registers(start: int, floor: int, reserve: int) -> None:
    module = corpus.loaded(Path("fixtures/regressions/r_walk-borland.obj"))
    assert module is not None
    instructions = blocks.instructions(module)
    assert not isinstance(instructions, str)
    found = nativeframe.entry(tuple(insn for insn in instructions if insn.at >= start))
    assert found is not None
    assert found.floor == floor
    assert found.reserve_at == reserve


def test_native_frame_does_not_guess_missing_setup() -> None:
    module = corpus.loaded(Path("fixtures/regressions/r_walk-borland.obj"))
    assert module is not None
    instructions = blocks.instructions(module)
    assert not isinstance(instructions, str)
    assert nativeframe.entry(tuple(instructions[1:])) is None


def test_native_spills_reserve_after_saves_and_release_before_pops() -> None:
    module = corpus.loaded(Path("fixtures/regressions/r_walk-borland.obj"))
    assert module is not None
    partition = extent.partition(module)
    assert not isinstance(partition, str)
    mapped = blocks.code_map(module)
    assert not isinstance(mapped, str)
    body = next(body for body in partition.bodies if body.seed == 0x604)
    parts = tuple(
        block
        for block in blocks.partition(module, mapped)
        if any(start <= block.at < end for start, end in body.ranges)
    )
    plan = nativeframe.plan(parts, body.seed)
    assert plan is not None
    assert plan.releases == {0x6C5}
    low = lir.LirBody(
        "native",
        body.seed,
        tuple(
            lir.LirBlock(
                block.at,
                tuple(lir.Insn(insn.at, (insn.at, insn.end), None, (), ()) for insn in block.insns),
                block.succ,
            )
            for block in parts
        ),
        {},
        {},
    )
    slots = frame.of(low, native=plan)
    assert slots.slot(1, 4) == -80
    changed = prologue.reserved(low, slots)
    adjustments = [one for one in changed.insns if one.frame_adjust]
    assert [(one.at, one.what.name if one.what else None) for one in adjustments] == [(0x60C, "sub"), (0x6C5, "add")]
    missing_pop = tuple(
        replace(block, insns=tuple(insn for insn in block.insns if insn.at != 0x6C5)) for block in parts
    )
    assert nativeframe.plan(missing_pop, body.seed) is None
    missing_anchor = replace(
        low,
        blocks=tuple(
            replace(block, insns=tuple(one for one in block.insns if one.at != 0x6C5)) for block in low.blocks
        ),
    )
    with pytest.raises(prologue.Refused, match="release anchor"):
        prologue.reserved(missing_anchor, slots)
    assert nativeframe.balanced(parts, plan, {0x6BF: 0})
    # Address folding changed allocation: POP's spill store shared its address
    # and the frame release gate refused a unique real restore as duplicated.
    spill = lir.Insn(0x6C5, (0x6C5, 0x6C5), None, (), ())
    expanded = replace(
        low,
        blocks=tuple(
            replace(
                block,
                insns=tuple(item for one in block.insns for item in ((one, spill) if one.at == 0x6C5 else (one,))),
            )
            for block in low.blocks
        ),
    )
    released = prologue.reserved(expanded, slots)
    assert sum(one.frame_adjust for one in released.insns) == 2
    assert not nativeframe.balanced(parts, plan, {})
    assert not nativeframe.balanced(parts, plan, {0x6BF: 2})
    missing_cleanup = tuple(
        replace(block, insns=tuple(insn for insn in block.insns if insn.at != 0x6C2)) for block in parts
    )
    assert not nativeframe.balanced(missing_cleanup, plan, {0x6BF: 0})


@pytest.mark.parametrize("start", [0, 0x2F6, 0x6CB])
def test_native_leaf_stack_balances_on_all_exits(start: int) -> None:
    module = corpus.loaded(Path("fixtures/regressions/r_walk-borland.obj"))
    assert module is not None
    partition = extent.partition(module)
    assert not isinstance(partition, str)
    mapped = blocks.code_map(module)
    assert not isinstance(mapped, str)
    body = next(body for body in partition.bodies if body.seed == start)
    parts = tuple(
        block for block in blocks.partition(module, mapped) if any(lo <= block.at < hi for lo, hi in body.ranges)
    )
    plan = nativeframe.plan(parts, start)
    assert plan is not None
    assert all(insn.flow != FlowControl.CALL for block in parts for insn in block.insns)
    assert nativeframe.balanced(parts, plan, {})


def test_private_calls_and_explicit_pascal_cleanup_balance_recursive_body() -> None:
    module = corpus.loaded(Path("fixtures/regressions/r_walk-borland.obj"))
    assert module is not None
    partition = extent.partition(module)
    assert not isinstance(partition, str)
    mapped = blocks.code_map(module)
    assert not isinstance(mapped, str)
    parts = tuple(blocks.partition(module, mapped))
    # Fresh r_bsp R_EMIT_ENTITIES returns with RETF 28, unlike the private RETs.
    external = {at: 28 for at, name in module.calls.items() if name == "R_EMIT_ENTITIES"}
    cleanup = nativecalls.cleanups(module, partition, parts, external)
    assert cleanup[0x3C4] == 0
    assert cleanup[0x4FE] == 0
    assert cleanup[0x45C] == 28
    assert 0x45C not in nativecalls.cleanups(module, partition, parts, {})
    body = next(body for body in partition.bodies if body.seed == 0x334)
    owned = tuple(block for block in parts if any(lo <= block.at < hi for lo, hi in body.ranges))
    plan = nativeframe.plan(owned, body.seed)
    assert plan is not None
    assert nativeframe.balanced(owned, plan, cleanup)
    contracts = nativecalls.interfaces(module, partition, parts, runtime.for_module(module))
    assert contracts[0x3C4].cleanup == 0
    assert not contracts[0x3C4].established
    assert contracts[0x3C4].writes == runtime.Memory.ANY
    bodies = mir.bodies(module, list(parts), contracts)
    for name, raised in bodies:
        lowered = lower.lowered(
            name, raised, module.calls, module.absorbed, contracts, nodes=bodies.source.nodes
        )
        assert lowered.entry == raised.entry


def test_lowering_uses_the_same_per_site_clobbers_as_raising() -> None:
    from iced_x86 import Register

    module = corpus.loaded(Path("fixtures/regressions/r_walk-borland.obj"))
    assert module is not None
    partition = extent.partition(module)
    assert not isinstance(partition, str)
    mapped = blocks.code_map(module)
    assert not isinstance(mapped, str)
    parts = tuple(blocks.partition(module, mapped))
    contracts = nativecalls.interfaces(module, partition, parts, runtime.for_module(module))
    # This is a contract-routing test, not a claim about the real callee.
    contracts[0x6BF] = replace(contracts[0x6BF], clobbers=frozenset({runtime.Reg.AX}), clobbers_reached=True)
    bodies = mir.bodies(module, list(parts), contracts)
    name, body = next((name, body) for name, body in bodies if body.entry == 0x604)
    low = lower.lowered(name, body, module.calls, module.absorbed, contracts, nodes=bodies.source.nodes)
    calls = [one for one in low.insns if one.at == 0x6BF and one.what and one.what.name == "call"]
    assert len(calls) == 1
    assert calls[0].clobbers == {Register.EAX}


def test_native_register_saves_survive_allocation() -> None:
    module = corpus.loaded(Path("fixtures/regressions/r_walk-borland.obj"))
    assert module is not None
    partition = extent.partition(module)
    assert not isinstance(partition, str)
    mapped = blocks.code_map(module)
    assert not isinstance(mapped, str)
    parts = tuple(blocks.partition(module, mapped))
    contracts = nativecalls.interfaces(module, partition, parts, runtime.for_module(module))
    bodies = mir.bodies(module, list(parts), contracts)
    name, raised = next((name, body) for name, body in bodies if body.entry == 0x604)
    original = next(body for body in partition.bodies if body.seed == raised.entry)
    owned = tuple(block for block in parts if any(lo <= block.at < hi for lo, hi in original.ranges))
    plan = nativeframe.plan(owned, raised.entry)
    assert plan is not None
    low = lower.lowered(name, raised, module.calls, module.absorbed, contracts, nodes=bodies.source.nodes)
    slots = frame.of(low, native=plan)
    for phase in flow.machine(low.pins, slots, module.calls):
        low = phase.transform(low)
    from iced_x86 import Register

    from qbopt.model import ir

    saved = [one.what.sources[0] for one in low.insns if one.at in (0x60A, 0x60B) and one.what]
    assert saved == [ir.Reg(Register.SI, 2), ir.Reg(Register.DI, 2)]
    restored = [
        one.what.dests[0]
        for one in low.insns
        if one.at in (0x6C5, 0x6C6) and one.what and one.what.op == ir.Operation.POP
    ]
    assert restored == [ir.Reg(Register.DI, 2), ir.Reg(Register.SI, 2)]
