"""A copy leaves its loop only when nothing inside the loop reads it."""

from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import copysink

AX, BX, CX, DX, SI, DI = (
    ir.Reg(one, 2) for one in (Register.AX, Register.BX, Register.CX, Register.DX, Register.SI, Register.DI)
)


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
    return {
        block.at: [one.what.dests[0] for one in block.insns if one.what.name == "mov" and one.what.sources == (DX,)]
        for block in body.blocks
    }


def test_copy_read_by_an_inner_loop_stays():
    """Shellsort's gap loop: `mov cx,dx` in the inner loop's header saved `i`
    for the inner loop's latch. The outer loop's exit test is never on that
    path, so asking only its successors found `cx` dead, the copy went after
    the outer loop, and the latch restored `i` from a `cx` nothing had set."""
    body = lir.LirBody(
        "f",
        1,
        (
            lir.LirBlock(1, (_jump(1, 31),), (31,)),
            lir.LirBlock(31, (_compare(31, AX, BX), _branch(32, "jle", 91)), (35, 91)),
            lir.LirBlock(35, (_move(35, DX, AX),), (38,)),
            lir.LirBlock(38, (_move(38, CX, DX), _compare(39, DX, BX), _branch(40, "jge", 86)), (42, 86)),
            lir.LirBlock(42, (_move(42, DX, BX),), (77,)),
            lir.LirBlock(77, (_move(77, DX, CX), _jump(78, 38)), (38,)),
            lir.LirBlock(86, (_move(86, SI, AX), _jump(87, 31)), (31,)),
            lir.LirBlock(91, (_return(91),), ()),
        ),
        {},
        {},
    )
    assert _copies(copysink.sunk(body))[38] == [CX]


def test_copy_read_only_after_its_loop_moves_to_the_exit():
    """Plasmablobs: `mov di,dx` on the way back to the header ran every pass for one read after the loop."""
    body = lir.LirBody(
        "f",
        1,
        (
            lir.LirBlock(1, (_jump(1, 3),), (3,)),
            lir.LirBlock(3, (_compare(3, AX, BX), _branch(4, "jge", 9)), (5, 9)),
            lir.LirBlock(5, (_move(5, DX, AX), _move(6, DI, DX), _jump(7, 3)), (3,)),
            lir.LirBlock(9, (_return(9),), ()),
        ),
        {},
        {},
    )
    copies = _copies(copysink.sunk(body))
    assert copies[5] == [] and copies[9] == [DI]


def _raw_insn(at: int, what: ir.Semantics) -> lir.Insn:
    return lir.Insn(at=at, covers=(at, at + 2), what=what, defines=(), uses=(), op=None)


def _reg(register: Register) -> ir.Reg:
    return ir.Reg(register, 2)


def _pushed(body: lir.LirBody, path: tuple[int, ...]) -> list[int]:
    """What the pushes along `path` write, running register moves and adds."""
    held: dict[Register, int] = {}
    pushed = []

    def read(operand: ir.Loc) -> int:
        return operand.value if isinstance(operand, ir.Imm) else held[operand.register]

    blocks = {block.at: block for block in body.blocks}
    for at in path:
        for one in blocks[at].insns:
            what = one.what
            if what.op in (ir.Operation.MOVE, ir.Operation.BINARY):
                held[what.dests[0].register] = sum(read(source) for source in what.sources)
            elif what.op is ir.Operation.PUSH:
                pushed.append(read(what.sources[0]))
    return pushed


def test_a_copy_an_inner_loop_reads_again_stays_in_it() -> None:
    """PRECALCULATIONS' map index was copied back once per row instead of once per pixel.

    The copy ended the inner loop, whose next pass read it, but the outer
    loop's header wrote it before reading, so the copy was moved past both
    loops and deedlines drew its minimap from stale plasma data.
    """
    si, bx, dx = _reg(Register.SI), _reg(Register.BX), _reg(Register.DX)
    body = lir.LirBody(
        name="one",
        entry=0,
        blocks=(
            lir.LirBlock(
                at=0,
                insns=(_raw_insn(0, ir.Semantics(ir.Operation.MOVE, "mov", (si,), (ir.Imm(0, 2),))),),
                succ=(0x10,),
            ),
            lir.LirBlock(
                at=0x10,
                insns=(
                    _raw_insn(0x10, ir.Semantics(ir.Operation.PUSH, "push", (), (si,))),
                    _raw_insn(0x12, ir.Semantics(ir.Operation.MOVE, "mov", (bx,), (si,))),
                    _raw_insn(0x14, ir.Semantics(ir.Operation.BINARY, "add", (bx,), (bx, ir.Imm(1, 2)))),
                    _raw_insn(0x16, ir.Semantics(ir.Operation.COMPARE, "cmp", (), (bx, ir.Imm(3, 2)))),
                    _raw_insn(0x18, ir.Semantics(ir.Operation.MOVE, "mov", (si,), (bx,))),
                    _raw_insn(0x1A, ir.Semantics(ir.Operation.BRANCH, "jl", (), (), 0x10)),
                ),
                succ=(0x10, 0x20),
            ),
            lir.LirBlock(
                at=0x20,
                insns=(
                    _raw_insn(0x20, ir.Semantics(ir.Operation.COMPARE, "cmp", (), (dx, ir.Imm(0, 2)))),
                    _raw_insn(0x22, ir.Semantics(ir.Operation.BRANCH, "jne", (), (), 0)),
                ),
                succ=(0, 0x30),
            ),
            lir.LirBlock(
                at=0x30,
                insns=(_raw_insn(0x30, ir.Semantics(ir.Operation.RETURN, "ret", (), ())),),
                succ=(),
            ),
        ),
        origin={},
        pins={},
    )
    assert _pushed(copysink.sunk(body), (0, 0x10, 0x10, 0x10)) == [0, 1, 2]
