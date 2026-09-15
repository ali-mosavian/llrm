"""A copy leaves its loop only when nothing inside the loop reads it."""

from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import copysink

AX, BX, CX, DX, SI, DI = (ir.Reg(one, 2) for one in (
    Register.AX, Register.BX, Register.CX, Register.DX, Register.SI, Register.DI))


def _insn(at, op, name, dests=(), sources=(), target=None):
    # Inserted, as the C path's are: an instruction standing for BC's bytes stays where `lir.without` finds no heir.
    return lir.Insn(at, (at, at), ir.Semantics(op, name, dests, sources, target), (), ())


def _move(at, dest, source):
    return _insn(at, ir.Operation.MOVE, "mov", (dest,), (source,))


def _compare(at, left, right):
    return _insn(at, ir.Operation.COMPARE, "cmp", (), (left, right))


def _branch(at, name, target):
    return _insn(at, ir.Operation.BRANCH, name, target=target)


def _jump(at, target):
    return _insn(at, ir.Operation.JUMP, "jmp", target=target)


def _return(at):
    return _insn(at, ir.Operation.RETURN, "ret")


def _copies(body):
    return {block.at: [one.what.dests[0] for one in block.insns if one.what.name == "mov" and one.what.sources == (DX,)]
            for block in body.blocks}


def test_copy_read_by_an_inner_loop_stays():
    """Shellsort's gap loop: `mov cx,dx` in the inner loop's header saved `i`
    for the inner loop's latch. The outer loop's exit test is never on that
    path, so asking only its successors found `cx` dead, the copy went after
    the outer loop, and the latch restored `i` from a `cx` nothing had set."""
    body = lir.LirBody("f", 1, (
        lir.LirBlock(1, (_jump(1, 31),), (31,)),
        lir.LirBlock(31, (_compare(31, AX, BX), _branch(32, "jle", 91)), (35, 91)),
        lir.LirBlock(35, (_move(35, DX, AX),), (38,)),
        lir.LirBlock(38, (_move(38, CX, DX), _compare(39, DX, BX), _branch(40, "jge", 86)), (42, 86)),
        lir.LirBlock(42, (_move(42, DX, BX),), (77,)),
        lir.LirBlock(77, (_move(77, DX, CX), _jump(78, 38)), (38,)),
        lir.LirBlock(86, (_move(86, SI, AX), _jump(87, 31)), (31,)),
        lir.LirBlock(91, (_return(91),), ()),
    ), {}, {})
    assert _copies(copysink.sunk(body))[38] == [CX]


def test_copy_read_only_after_its_loop_moves_to_the_exit():
    """Plasmablobs: `mov di,dx` on the way back to the header ran every pass for one read after the loop."""
    body = lir.LirBody("f", 1, (
        lir.LirBlock(1, (_jump(1, 3),), (3,)),
        lir.LirBlock(3, (_compare(3, AX, BX), _branch(4, "jge", 9)), (5, 9)),
        lir.LirBlock(5, (_move(5, DX, AX), _move(6, DI, DX), _jump(7, 3)), (3,)),
        lir.LirBlock(9, (_return(9),), ()),
    ), {}, {})
    copies = _copies(copysink.sunk(body))
    assert copies[5] == [] and copies[9] == [DI]
