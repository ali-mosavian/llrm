"""arrprm printed 0 0 for 7 8 when spilling moved its runtime frame entry."""

from dataclasses import replace

from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import frame
from qbopt.backend import prologue
from qbopt.backend import peephole
from iced_x86 import Register
import pytest


@pytest.mark.parametrize("use", ["none", "load", "address", "opaque"])
def test_empty_spill_reservation_is_removed_only_without_remaining_uses(use):
    """QB FPCSE retained SUB SP,2 after its final dead spill reload vanished."""
    slots = frame.Frame(-16)
    cell = slots.cell(1, 2)
    body = procedure()
    if use != "none":
        what = None if use == "opaque" else ir.Semantics(ir.Operation.MOVE, "mov",
            (ir.Reg(Register.AX, 2),),
            (cell if use == "load" else ir.Imm(0, 2, cell.addr),))
        one = lir.Insn(4, (4, 4), what, (), ())
        body = replace(body, blocks=(replace(body.blocks[0], insns=body.blocks[0].insns + (one,)),))
    reserved = prologue.reserved(body, slots, {1: "B$ENRA", 2: "B$EXSA"})
    result = peephole.Peephole(slots)._frame(reserved)
    assert sum(one.frame_adjust for one in result.insns) == (0 if use == "none" else 2)


def procedure() -> lir.LirBody:
    def instruction(at: int, what: ir.Semantics) -> lir.Insn:
        return lir.Insn(at=at, covers=(at, at + 1), what=what, defines=(), uses=())

    insns = (
        instruction(0, ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(1, 2),), (ir.Imm(6, 2),))),
        instruction(1, ir.Semantics(ir.Operation.CALL, "call", (), (ir.Held(1, 2),))),
        instruction(2, ir.Semantics(ir.Operation.CALL, "call", (), ())),
        instruction(3, ir.Semantics(ir.Operation.RETURN, "retf", (), ())),
    )
    return lir.LirBody("procedure", 0, (lir.LirBlock(at=0, insns=insns, succ=()),), origin={}, pins={})


def test_spill_reservation_is_inside_the_runtime_frame() -> None:
    body = procedure()
    slots = frame.Frame(-16)
    slots.slot(1, 2)
    result = prologue.reserved(body, slots, {1: "B$ENRA", 2: "B$EXSA"})
    assert [one.what.name for one in result.blocks[0].insns] == ["mov", "call", "sub", "add", "call", "retf"]


def test_slots_are_below_runtime_metadata_and_declared_locals() -> None:
    slots = frame.of(procedure(), {1: "B$ENRA", 2: "B$EXSA"})
    assert slots.slot(9, 2) == -18


def test_explicit_end_needs_no_spill_frame_return() -> None:
    """EVTRAP main refused four spill bytes after its END edge was corrected."""
    body = procedure()
    body = replace(body, blocks=(replace(body.blocks[0], insns=body.blocks[0].insns[:3]),))
    slots = frame.Frame(-16)
    slots.slot(1, 4)
    result = prologue.reserved(body, slots, {2: "B$CEND"})
    assert result.blocks[0].insns[0].what.name == "sub"
