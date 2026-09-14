"""Cutting a live range at a loop exit.

Nothing covered this pass before. What is here is what the nested-base
rename ran into: a pointer computed before a loop, untouched by it, and
dereferenced after -- which is the shape `only` selects for, since a value
the loop does not touch is exactly what the allocator would rather cut
than spill.
"""

from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import splitkit
from qbopt.objectfile.module import Addr
from qbopt.objectfile.module import Space


def _insn(at: int, what: ir.Semantics, defines=(), uses=()) -> lir.Insn:
    return lir.Insn(at=at, covers=(at, at + 2), what=what, defines=defines, uses=uses, op=None)


def _pointer_across_a_loop() -> lir.LirBody:
    """v3 is made before the loop, read in it and after it, as a cell's base."""
    where = Addr(Space.SEGMENT, 0x10, base=Register.SI)
    cell = ir.Mem(where, 2, Register.NONE, 0, 2, base=ir.Held(3, 2))
    return lir.LirBody(
        name="one",
        entry=0,
        blocks=(
            lir.LirBlock(
                at=0,
                insns=(
                    _insn(0, ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(3, 2),), (ir.Imm(0x40, 2),)), (3,), ()),
                    _insn(2, ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(4, 2),), (ir.Imm(1, 2),)), (4,), ()),
                    _insn(4, ir.Semantics(ir.Operation.JUMP, "jmp", (), (), 0x10)),
                ),
                succ=(0x10,),
            ),
            lir.LirBlock(
                at=0x10,
                insns=(
                    _insn(
                        0x10,
                        ir.Semantics(ir.Operation.BINARY, "add", (ir.Held(4, 2),), (ir.Held(4, 2), ir.Imm(1, 2))),
                        (4,),
                        (4,),
                    ),
                    _insn(0x11, ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(6, 2),), (cell,)), (6,), (3,)),
                    _insn(0x12, ir.Semantics(ir.Operation.BRANCH, "jne", (), (), 0x10), (), ()),
                ),
                succ=(0x10, 0x20),
            ),
            lir.LirBlock(
                at=0x20,
                insns=(
                    _insn(0x20, ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(5, 2),), (cell,)), (5,), (3,)),
                    _insn(0x22, ir.Semantics(ir.Operation.RETURN, "ret", (), ()), (), (5,)),
                ),
                succ=(),
            ),
        ),
        origin={},
        pins={},
    )


def test_a_cut_range_renames_the_cell_it_is_the_base_of() -> None:
    """A cut renamed `uses` and left the cell naming the old value.

    `_settled` looked for a Held in `Mem.through`, which holds a register
    now -- so the copy defined v6, the load said it used v6, and the byte
    it encoded still read v3, which nothing defines after the cut.
    """
    body = splitkit.split(_pointer_across_a_loop(), frozenset({3}))
    loads = [
        one
        for block in body.blocks
        for one in block.insns
        if one.what and any(isinstance(x, ir.Mem) for x in one.what.sources)
    ]
    assert any(one.uses != (3,) for one in loads), "nothing was cut; the fixture does not reach the rename"
    for load in loads:
        cell = load.what.sources[0]
        assert load.uses == (cell.base.value,), f"uses {load.uses}, cell on {cell.base}"
        assert cell.through == Register.NONE, "the rename placed it"
    assert (cell.addr, cell.width, cell.offset, cell.disp_width) == (
        Addr(Space.SEGMENT, 0x10, base=Register.SI),
        2,
        0,
        2,
    )
