"""What a block reads before it writes."""

from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import liveness
from qbopt.backend.peephole import _lanes

AX, DX = ir.Reg(Register.AX, 2), ir.Reg(Register.DX, 2)


def _insn(at, op, name, dests=(), sources=()):
    return lir.Insn(at, (at, at), ir.Semantics(op, name, dests, sources), (), ())


def test_a_register_written_before_an_unknown_instruction_is_not_live_into_its_block():
    """A return, which nothing decodes, made its whole block live-in for every
    lane: the `mov edx,eax` before it did not count, so DX looked live back
    through all of sieve's loops and its flag test kept `movzx dx,[m]; or dx,dx`."""
    body = lir.LirBody("f", 1, (
        lir.LirBlock(1, (_insn(1, ir.Operation.MOVE, "mov", (DX,), (AX,)), _insn(2, ir.Operation.RETURN, "ret")), ()),
    ), {}, {})
    into, _successors, _universe = liveness.live_into(body)
    assert not _lanes(Register.DX) & into[1]
    assert _lanes(Register.AX) <= into[1]
