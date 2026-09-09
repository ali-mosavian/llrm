"""arrprm printed 0 0 for 7 8 when spilling moved its runtime frame entry."""

from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import frame
from qbopt.backend import prologue


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
