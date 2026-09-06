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
