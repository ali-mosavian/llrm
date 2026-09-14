"""
qbopt/backend/parcopy.py: moves that happen at once, written in an order that
computes them.

The bug it exists for: pressx-v-evt emitted a phi edge's six moves in the
order the phis were listed, and one of them read a register an earlier one
had already overwritten -- `r24 <- [bp-8]` and then `r27 <- r24`. R came out
6460 for 7500, and nothing structural about the object was wrong.
"""

import pytest
from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import parcopy
from qbopt.backend import select


def _move(into, out_of, group=None, at=0x100) -> lir.Insn:
    what = ir.Semantics(ir.Operation.MOVE, "mov", (into,), (out_of,))
    return lir.Insn(at=at, covers=(at, at), what=what, defines=(), uses=(), group=group, op=None)


def _reg(one) -> ir.Reg:
    return ir.Reg(one, 2)


def _slot(offset: int) -> ir.Mem:
    return ir.Mem(f"[bp-{offset:#x}]", 2, Register.BP, 0, 2)


def test_memory_copy_expands_after_dependency_ordering() -> None:
    """NESTED needs a spilled phi copied before another move overwrites its source."""
    source, destination = _slot(4), _slot(8)
    result = parcopy.scheduled(_body(_move(source, _reg(Register.AX), group=1), _move(destination, source, group=1)))
    instructions = result.blocks[0].insns
    assert [one.what.name for one in instructions] == ["push", "pop", "mov"]
    assert instructions[0].what.sources == (source,)
    assert instructions[1].what.dests == (destination,)
    assert all(one.group is None for one in instructions)


@pytest.mark.parametrize("width,prefix", [(2, b""), (4, b"\x66")])
def test_frame_copy_emits_balanced_stack_transfer(width: int, prefix: bytes) -> None:
    """NESTED refused emission when a phi needed a slot-to-slot copy."""
    from qbopt.objectfile.module import Space

    source = ir.Mem(ir.Addr(Space.FRAME, -4), width, Register.BP, 0, 2)
    destination = ir.Mem(ir.Addr(Space.FRAME, -8), width, Register.BP, 0, 2)
    instructions = parcopy.scheduled(_body(_move(destination, source, group=1))).blocks[0].insns
    emitted = [select.emit(one.what) for one in instructions]
    assert all(one is not None for one in emitted)
    assert [one.code for one in emitted] == [prefix + b"\xff\x76\xfc", prefix + b"\x8f\x46\xf8"]


def _other(at=0x200) -> lir.Insn:
    what = ir.Semantics(ir.Operation.PUSH, "push", (), (_reg(Register.AX),))
    return lir.Insn(at=at, covers=(at, at + 1), what=what, defines=(), uses=(), op=None)


def _body(*insns) -> lir.LirBody:
    return lir.LirBody("one", 0, (lir.LirBlock(at=0, insns=insns, succ=()),), origin={}, pins={})


def _order(body) -> list[str]:
    return [
        f"{parcopy._into(one)}<-{parcopy._outof(one)}"
        if one.group is None and one.what.op is ir.Operation.MOVE
        else one.what.name
        for block in parcopy.scheduled(body).blocks
        for one in block.insns
    ]


def test_a_move_goes_after_everything_that_reads_what_it_writes() -> None:
    """pressx's own shape: the reload into r24 must come last, because
    another move in the same group still reads the old r24."""
    got = _order(
        _body(
            _move(_reg(Register.DI), _slot(8), group=1),  # r24 <- [bp-8]
            _move(_reg(Register.BP), _reg(Register.DI), group=1),  # r27 <- r24
        )
    )
    di, bp = parcopy._named(_reg(Register.DI)), parcopy._named(_reg(Register.BP))
    slot = parcopy._named(_slot(8))
    assert got == [f"{bp}<-{di}", f"{di}<-{slot}"], got


def test_a_move_of_a_place_into_itself_is_dropped() -> None:
    got = _order(
        _body(
            _move(_reg(Register.AX), _reg(Register.AX), group=1),
            _move(_reg(Register.BX), _reg(Register.CX), group=1),
        )
    )
    assert got == [f"{parcopy._named(_reg(Register.BX))}<-{parcopy._named(_reg(Register.CX))}"], got


def test_what_is_not_in_a_group_keeps_its_place() -> None:
    body = _body(_other(0x100), _move(_reg(Register.AX), _reg(Register.CX), group=1), _other(0x300))
    got = _order(body)
    assert got == ["push", f"{parcopy._named(_reg(Register.AX))}<-{parcopy._named(_reg(Register.CX))}", "push"], got


def test_two_groups_are_scheduled_apart() -> None:
    """One group's move may write what another's reads; they are not
    simultaneous with each other and must not be reordered together."""
    got = _order(
        _body(
            _move(_reg(Register.DI), _slot(8), group=1),
            _move(_reg(Register.BP), _reg(Register.DI), group=2),
        )
    )
    di, bp = parcopy._named(_reg(Register.DI)), parcopy._named(_reg(Register.BP))
    assert got == [f"{di}<-{parcopy._named(_slot(8))}", f"{bp}<-{di}"], got


def test_a_register_cycle_is_exchanged_and_a_slot_cycle_refused_by_name() -> None:
    """Two moves that each read the other's destination need a temporary or
    an exchange. Registers get the exchange; no instruction exchanges two
    slots, so that is refused rather than written in an order that computes
    something else."""
    swapped = parcopy.scheduled(
        _body(
            _move(_reg(Register.AX), _reg(Register.CX), group=1),
            _move(_reg(Register.CX), _reg(Register.AX), group=1),
        )
    ).blocks[0].insns
    assert [one.what.name for one in swapped] == ["xchg"]
    with pytest.raises(parcopy.Tangled, match="temporary"):
        parcopy.scheduled(_body(_move(_slot(4), _slot(8), group=1), _move(_slot(8), _slot(4), group=1)))


def test_something_that_is_not_a_move_in_a_group_is_refused() -> None:
    from dataclasses import replace

    with pytest.raises(parcopy.Malformed):
        parcopy.scheduled(_body(replace(_other(0x100), group=1)))


def test_the_scheduler_runs_after_the_allocation_and_before_the_prologue() -> None:
    """Which moves in a copy conflict is a question about locations, so it
    cannot be asked before the allocator has chosen them -- and it has to
    be answered before anything reads the code as a sequence."""
    from qbopt import flow
    from qbopt.backend import allocate
    from qbopt.backend import prologue

    order = [type(one) for one in flow.machine({}, None, {})]
    assert order.index(parcopy.ParallelCopy) == order.index(allocate.RegAlloc) + 1
    assert order.index(parcopy.ParallelCopy) < order.index(prologue.Prologue)


def test_nothing_leaves_the_machine_pipeline_still_grouped() -> None:
    from pathlib import Path

    from qbopt.model import mir
    from qbopt.objectfile import omf
    from qbopt import flow
    from qbopt.backend import lower
    from qbopt.objectfile import module
    from qbopt.abi import runtime
    from qbopt.optimize import transform
    from qbopt.frontend import blocks as split
    from qbopt.backend import frame as frames
    from qbopt.frontend.blocks import code_map

    # pressx-p-g2 rather than pressx-v-evt: the event build calls a
    # routine nothing is established about, so the lowering refuses it and
    # the pipeline this checks is never reached.
    found = module.of(omf.parse(Path("fixtures/omf/pressx-p-g2.obj").read_bytes()))
    blocks = split.partition(found, code_map(found))
    name, body = next(iter(mir.bodies(found, blocks)))
    body = transform.widened(transform.applied(body, found.dgroup, found.calls, blocks=blocks, found=found))
    low = lower.lowered(name, body, found.calls, set(found.absorbed), runtime.for_module(found))
    for phase in flow.machine(flow._pinned(body), frames.of(low), found.calls):
        low = phase.transform(low)
    left = [one.at for block in low.blocks for one in block.insns if one.group is not None]
    assert not left, f"copies still marked simultaneous at {[hex(x) for x in left]}"


def test_a_tangled_copy_is_refused_and_says_so() -> None:
    """A refusal the emitter names, not a program written in the wrong
    order. BC's own object comes back, unmarked."""
    from pathlib import Path

    from qbopt.objectfile import omf
    from qbopt import wholeseg

    was = parcopy.ParallelCopy.transform

    def tangles(self, body):
        raise parcopy.Tangled("injected: they all read each other")

    parcopy.ParallelCopy.transform = tangles
    try:
        got = wholeseg.emitted(Path("fixtures/omf/pressx-p-g2.obj").read_bytes())
    finally:
        parcopy.ParallelCopy.transform = was
    assert got.outcome is wholeseg.Emission.REFUSED
    assert got.data == Path("fixtures/omf/pressx-p-g2.obj").read_bytes()
    assert "Tangled" in got.reason
    assert omf.finalised_at(omf.parse(got.data)) is None


