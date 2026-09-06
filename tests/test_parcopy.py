"""
qbopt/parcopy.py: moves that happen at once, written in an order that
computes them.

The bug it exists for: pressx-v-evt emitted a phi edge's six moves in the
order the phis were listed, and one of them read a register an earlier one
had already overwritten -- `r24 <- [bp-8]` and then `r27 <- r24`. R came out
6460 for 7500, and nothing structural about the object was wrong.
"""

import pytest
from iced_x86 import Register

from qbopt import ir
from qbopt import lir
from qbopt import parcopy


def _move(into, out_of, group=None, at=0x100) -> lir.Insn:
    what = ir.Semantics(ir.Operation.MOVE, "mov", (into,), (out_of,))
    return lir.Insn(at=at, covers=(at, at), what=what, defines=(), uses=(), group=group, op=None)


def _reg(one) -> ir.Reg:
    return ir.Reg(one, 2)


def _slot(offset: int) -> ir.Mem:
    return ir.Mem(f"[bp-{offset:#x}]", 2, Register.BP, 0, 2)


def _other(at=0x200) -> lir.Insn:
    what = ir.Semantics(ir.Operation.PUSH, "push", (), (_reg(Register.AX),))
    return lir.Insn(at=at, covers=(at, at + 1), what=what, defines=(), uses=(), op=None)


def _body(*insns) -> lir.LirBody:
    return lir.LirBody("one", 0, (lir.LirBlock(at=0, insns=insns, succ=()),), origin={}, pins={})


def _order(body) -> list[str]:
    return [
        f"{parcopy._into(one)}<-{parcopy._outof(one)}" if one.group is None and one.what.op is ir.Operation.MOVE
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


def test_a_cycle_is_refused_by_name_rather_than_written_wrongly() -> None:
    """Two moves that each read the other's destination need a temporary or
    an exchange. Refused, so a caller falls back rather than emitting an
    order that computes something else."""
    with pytest.raises(parcopy.Tangled, match="temporary"):
        parcopy.scheduled(
            _body(
                _move(_reg(Register.AX), _reg(Register.CX), group=1),
                _move(_reg(Register.CX), _reg(Register.AX), group=1),
            )
        )


def test_something_that_is_not_a_move_in_a_group_is_refused() -> None:
    from dataclasses import replace

    with pytest.raises(parcopy.Malformed):
        parcopy.scheduled(_body(replace(_other(0x100), group=1)))


def test_the_scheduler_runs_after_the_allocation_and_before_the_prologue() -> None:
    """Which moves in a copy conflict is a question about locations, so it
    cannot be asked before the allocator has chosen them -- and it has to
    be answered before anything reads the code as a sequence."""
    from qbopt import allocate
    from qbopt import flow
    from qbopt import prologue

    order = [type(one) for one in flow.machine({}, None, {})]
    assert order.index(parcopy.ParallelCopy) == order.index(allocate.RegAlloc) + 1
    assert order.index(parcopy.ParallelCopy) < order.index(prologue.Prologue)


def test_nothing_leaves_the_machine_pipeline_still_grouped() -> None:
    from pathlib import Path

    from qbopt import blocks as split
    from qbopt import flow
    from qbopt import frame as frames
    from qbopt import lower
    from qbopt import mir
    from qbopt import module
    from qbopt import omf
    from qbopt import transform
    from qbopt.blocks import code_map

    found = module.of(omf.parse(Path("fixtures/omf/pressx-v-evt.obj").read_bytes()))
    blocks = split.partition(found, code_map(found))
    name, body = next(iter(mir.bodies(found, blocks)))
    body = transform.widened(
        transform.applied(body, found.dgroup, found.calls, blocks=blocks, found=found)
    )
    low = lower.lowered(name, body, found.calls, set(found.absorbed))
    for phase in flow.machine(flow._pinned(body), frames.of(low), found.calls):
        low = phase.transform(low)
    left = [one.at for block in low.blocks for one in block.insns if one.group is not None]
    assert not left, f"copies still marked simultaneous at {[hex(x) for x in left]}"


def test_a_tangled_copy_falls_back_and_says_so() -> None:
    """A refusal the emitter names, not a program written in the wrong
    order. The fallback is BC's layout and is not marked as this pass's
    own final output."""
    from pathlib import Path

    from qbopt import omf
    from qbopt import wholeseg

    was = parcopy.ParallelCopy.transform

    def tangles(self, body):
        raise parcopy.Tangled("injected: they all read each other")

    parcopy.ParallelCopy.transform = tangles
    try:
        got = wholeseg.emitted(Path("fixtures/omf/pressx-v-evt.obj").read_bytes())
    finally:
        parcopy.ParallelCopy.transform = was
    assert got.outcome is wholeseg.Emission.MIR
    assert got.reason == wholeseg.REBUILT
    assert got.fallback_reason and "Tangled" in got.fallback_reason
    assert omf.finalised_at(omf.parse(got.data)) is None
