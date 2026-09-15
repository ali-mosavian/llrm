"""Jumps the block order made redundant are gone from the printed procedure.

Over qcport: 335 `jcc` over a `jmp`, 262 `jmp` to the next label, 69 jumps
to a block that only jumps.
"""

from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import masm
from qbopt.backend import jumps

AX, BX, CX = (ir.Reg(one, 2) for one in (Register.AX, Register.BX, Register.CX))


def _insn(at, op, name, dests=(), sources=(), target=None):
    return lir.Insn(at, (at, 1), ir.Semantics(op, name, dests, sources, target), (), ())


def _compare(at):
    return _insn(at, ir.Operation.COMPARE, "cmp", (), (AX, BX))


def _branch(at, name, target):
    return _insn(at, ir.Operation.BRANCH, name, target=target)


def _jump(at, target):
    return _insn(at, ir.Operation.JUMP, "jmp", target=target)


def _move(at, source):
    return _insn(at, ir.Operation.MOVE, "mov", (AX,), (source,))


def _return(at):
    return _insn(at, ir.Operation.RETURN, "ret")


def _printed(*blocks):
    body = lir.LirBody("f", blocks[0].at, blocks, {}, {})
    procedure = masm.Procedure("_f", True, False, jumps.threaded(body), 0, {})
    return [line.strip() for line in masm._procedure(procedure, {}, 0)][1:-1]


def test_branch_to_a_block_that_only_jumps_goes_to_its_target():
    assert _printed(
        lir.LirBlock(1, (_compare(1), _branch(2, "je", 7)), (7, 4)),
        lir.LirBlock(4, (_move(4, CX), _return(5)), ()),
        lir.LirBlock(7, (_jump(7, 9),), (9,)),
        lir.LirBlock(8, (_move(8, BX), _return(8)), ()),
        lir.LirBlock(9, (_return(9),), ()),
    ) == ["L0_1:", "cmp ax, bx", "je L0_9", "L0_4:", "mov ax, cx", "ret", "L0_9:", "ret"]


def test_branch_over_a_jump_is_inverted():
    assert _printed(
        lir.LirBlock(1, (_compare(1), _branch(2, "je", 9)), (9, 4)),
        lir.LirBlock(4, (_jump(4, 12),), (12,)),
        lir.LirBlock(9, (_move(9, CX), _return(10)), ()),
        lir.LirBlock(12, (_move(12, BX), _return(13)), ()),
    ) == ["L0_1:", "cmp ax, bx", "jne L0_12", "L0_9:", "mov ax, cx", "ret", "L0_12:", "mov ax, bx", "ret"]


def test_branch_then_jump_in_one_block_is_inverted():
    assert _printed(
        lir.LirBlock(1, (_compare(1), _branch(2, "je", 4), _jump(3, 9)), (4, 9)),
        lir.LirBlock(4, (_move(4, CX), _return(5)), ()),
        lir.LirBlock(9, (_move(9, BX), _return(10)), ()),
    ) == ["L0_1:", "cmp ax, bx", "jne L0_9", "L0_4:", "mov ax, cx", "ret", "L0_9:", "mov ax, bx", "ret"]


def test_jump_to_the_next_block_is_dropped():
    assert _printed(
        lir.LirBlock(1, (_move(1, BX), _jump(2, 4)), (4,)),
        lir.LirBlock(4, (_return(4),), ()),
    ) == ["L0_1:", "mov ax, bx", "L0_4:", "ret"]


def test_jump_over_a_block_nothing_reaches_is_dropped():
    """cfg_trim kept `jne L0_18; jmp L0_13` over an empty block 12 nothing enters:
    unreachable blocks went only once some other rule had fired."""
    assert _printed(
        lir.LirBlock(1, (_move(1, BX), _jump(2, 9)), (9,)),
        lir.LirBlock(5, (), ()),
        lir.LirBlock(9, (_return(9),), ()),
    ) == ["L0_1:", "mov ax, bx", "L0_9:", "ret"]


def test_a_block_that_jumps_to_itself_stays():
    """`for (;;);` -- following jumps must not go round forever."""
    assert _printed(lir.LirBlock(1, (_jump(1, 1),), (1,))) == ["L0_1:", "jmp L0_1"]
