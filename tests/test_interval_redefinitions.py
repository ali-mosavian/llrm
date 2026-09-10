"""Call results are new values even when coalescing reuses their register id."""

from dataclasses import replace

from iced_x86 import Register

from qbopt.backend import allocate
from qbopt.model import ir, lir


def test_call_redefinition_is_not_a_value_surviving_the_clobber() -> None:
    """SYS_PARSE_ARGS refused unspillable value#1591 at HOST_SHUTDOWN (00c2)."""
    held = ir.Held(1, 2)
    before = lir.Insn(0, (0, 1), ir.Semantics(ir.Operation.MOVE, "mov", (held,), (ir.Imm(7, 2),)),
                      defines=(1,), uses=())
    call = lir.Insn(1, (1, 6), ir.Semantics(ir.Operation.CALL, "call", (), (held,)),
                    defines=(1,), uses=(1,), clobbers=frozenset({Register.ESI}))
    after = lir.Insn(6, (6, 7), ir.Semantics(ir.Operation.PUSH, "push", (), (held,)),
                     defines=(), uses=(1,))
    body = lir.LirBody("call", 0, (lir.LirBlock(0, (before, call, after), ()),), origin={}, pins={})
    assigned = allocate.allocate(body, {1: Register.SI}, frozenset({1}))
    assert not assigned.spilled and assigned.where[1] == Register.SI

    surviving = replace(body, blocks=(replace(body.blocks[0], insns=(before, replace(call, defines=()), after)),))
    assert allocate.allocate(surviving, {1: Register.SI}).spilled == frozenset({1})
