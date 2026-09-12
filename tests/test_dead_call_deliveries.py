from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import allocate
from qbopt.backend import constrain


def test_dead_call_result_does_not_create_an_undefined_copy() -> None:
    # r_walk gained 46 spill bytes and post-call reads from slots with no stores.
    call = lir.Insn(
        0, (0, 3), ir.Semantics(ir.Operation.CALL, "call", (), ()), (1,), (), delivers=((ir.Held(1, 2), Register.AX),)
    )
    body = lir.LirBody("dead result", 0, (lir.LirBlock(0, (call,), ()),), {}, {})
    narrowed, pins = allocate.narrowed(body, {1: Register.EAX})
    lowered, _ = constrain.constrained(narrowed, pins)
    assert len(lowered.insns) == 1
    assert lowered.insns[0].defines == ()
    assert lowered.insns[0].delivers == ()
    assert Register.EAX in lowered.insns[0].clobbers
