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


def test_a_piece_leaves_its_region_after_its_last_use() -> None:
    """PLASMA's outer counter piece was copied back at its preheader's jump.

    Live past its last use, the piece overlapped the pixel pointer defined
    after it there, so keeping the counter in a register cost the pointer
    its register and the split was refused. LLVM leaves the interval after
    the last use.
    """
    from qbopt.analysis import intervals

    def move(at, into, value):
        return _insn(at, ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(into, 2),), (ir.Imm(value, 2),)), (into,), ())

    def push(at, value):
        return _insn(at, ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Held(value, 2),)), (), (value,))

    add = ir.Semantics(ir.Operation.BINARY, "add", (ir.Held(3, 2),), (ir.Held(3, 2), ir.Imm(1, 2)))
    body = lir.LirBody(
        name="one",
        entry=0,
        blocks=(
            lir.LirBlock(
                at=0,
                insns=(move(0, 3, 1), _insn(2, ir.Semantics(ir.Operation.JUMP, "jmp", (), (), 0x10))),
                succ=(0x10,),
            ),
            lir.LirBlock(
                at=0x10,
                insns=(
                    _insn(0x10, add, (3,), (3,)),
                    move(0x12, 7, 2),
                    _insn(0x14, ir.Semantics(ir.Operation.JUMP, "jmp", (), (), 0x20)),
                ),
                succ=(0x20,),
            ),
            lir.LirBlock(at=0x20, insns=(push(0x20, 3), push(0x21, 7)), succ=()),
        ),
        origin={},
        pins={},
    )
    cut = splitkit._carved(body, 3, 9, 2, splitkit.Region(frozenset({0x10}), None))
    live = intervals.intervals(cut)
    assert not live[9].overlaps(live[7])


def _pushed(body: lir.LirBody, path: tuple[int, ...]) -> list[int]:
    """What the pushes along `path` write, running moves, adds and pushes."""
    held: dict[int, int] = {}
    pushed = []

    def read(operand):
        return operand.value if isinstance(operand, ir.Imm) else held[operand.value]

    blocks = {block.at: block for block in body.blocks}
    for at in path:
        for one in blocks[at].insns:
            what = one.what
            if what.op is ir.Operation.MOVE:
                held[what.dests[0].value] = read(what.sources[0])
            elif what.op is ir.Operation.BINARY:
                held[what.dests[0].value] = read(what.sources[0]) + read(what.sources[1])
            elif what.op is ir.Operation.PUSH:
                pushed.append(read(what.sources[0]))
    return pushed


def test_a_region_block_also_reached_from_inside_it_keeps_the_inside_value() -> None:
    """RENDER's row counter was copied in again at a block its own increment reached.

    The block was entered from outside the region as well, and the copy at
    its top overwrote the incremented piece with the stale original on the
    inside path, so the loop never ended.
    """

    def move(at, into, value):
        return _insn(at, ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(into, 2),), (ir.Imm(value, 2),)), (into,), ())

    add = ir.Semantics(ir.Operation.BINARY, "add", (ir.Held(3, 2),), (ir.Held(3, 2), ir.Imm(1, 2)))
    push = ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Held(3, 2),))
    body = lir.LirBody(
        name="one",
        entry=0,
        blocks=(
            lir.LirBlock(
                at=0,
                insns=(move(0, 3, 1), _insn(2, ir.Semantics(ir.Operation.BRANCH, "jne", (), (), 0x20))),
                succ=(0x10, 0x20),
            ),
            lir.LirBlock(
                at=0x10,
                insns=(_insn(0x10, add, (3,), (3,)), _insn(0x12, ir.Semantics(ir.Operation.JUMP, "jmp", (), (), 0x20))),
                succ=(0x20,),
            ),
            lir.LirBlock(at=0x20, insns=(_insn(0x20, push, (), (3,)),), succ=()),
        ),
        origin={},
        pins={},
    )
    cut = splitkit._carved(body, 3, 9, 2, splitkit.Region(frozenset({0x10, 0x20}), None))
    assert _pushed(cut, (0, 0x10, 0x20)) == [2]
    assert _pushed(cut, (0, 0x20)) == [1]


def test_a_cut_range_renames_what_an_instruction_requires_and_delivers() -> None:
    """SETCROSFADEPAL's `out dx,al` still required the counter after its piece took over.

    A requirement names the value the instruction reads in a register it
    does not mention, and a delivery the one it writes there. Left on the
    original, the allocator pinned a value the instruction no longer reads
    and left the piece it does read wherever it fell.
    """
    from iced_x86 import Register as R

    call = lir.Insn(
        at=0x10,
        covers=(0x10, 0x12),
        what=ir.Semantics(ir.Operation.CALL, "call", (), ()),
        defines=(3,),
        uses=(),
        delivers=((ir.Held(3, 2), R.DI),),
        widths=((3, 2),),
    )
    out = lir.Insn(
        at=0x12,
        covers=(0x12, 0x13),
        what=None,
        defines=(),
        uses=(3,),
        requires=((ir.Held(3, 1), R.AL),),
        widths=((3, 1),),
    )
    body = lir.LirBody(
        name="one",
        entry=0,
        blocks=(
            lir.LirBlock(at=0, insns=(_insn(0, ir.Semantics(ir.Operation.JUMP, "jmp", (), (), 0x10)),), succ=(0x10,)),
            lir.LirBlock(
                at=0x10,
                insns=(call, out, _insn(0x14, ir.Semantics(ir.Operation.JUMP, "jmp", (), (), 0x20))),
                succ=(0x20,),
            ),
            lir.LirBlock(
                at=0x20,
                insns=(_insn(0x20, ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Held(3, 2),)), (), (3,)),),
                succ=(),
            ),
        ),
        origin={},
        pins={},
    )
    cut = splitkit._carved(body, 3, 9, 2, splitkit.Region(frozenset({0x10}), None))
    for one in (one for block in cut.blocks for one in block.insns):
        assert {held.value for held, _register in one.requires} <= set(one.uses), one
        assert {held.value for held, _register in one.delivers} <= set(one.defines), one
        assert {value for value, _width in one.widths} <= {*one.uses, *one.defines}, one


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


def _counting_loop() -> lir.LirBody:
    """v4 counts to 10: set before the loop, tested in its header, bumped in its latch, read after."""
    return lir.LirBody(
        name="count",
        entry=0,
        blocks=(
            lir.LirBlock(
                at=0,
                insns=(
                    _insn(0, ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(4, 2),), (ir.Imm(0, 2),)), (4,), ()),
                    _insn(2, ir.Semantics(ir.Operation.JUMP, "jmp", (), (), 0x10)),
                ),
                succ=(0x10,),
            ),
            lir.LirBlock(
                at=0x10,
                insns=(
                    _insn(
                        0x10, ir.Semantics(ir.Operation.COMPARE, "cmp", (), (ir.Held(4, 2), ir.Imm(10, 2))), (), (4,)
                    ),
                    _insn(0x12, ir.Semantics(ir.Operation.BRANCH, "jge", (), (), 0x30)),
                ),
                succ=(0x20, 0x30),
            ),
            lir.LirBlock(
                at=0x20,
                insns=(
                    _insn(
                        0x20,
                        ir.Semantics(ir.Operation.BINARY, "add", (ir.Held(4, 2),), (ir.Held(4, 2), ir.Imm(1, 2))),
                        (4,),
                        (4,),
                    ),
                    _insn(0x22, ir.Semantics(ir.Operation.JUMP, "jmp", (), (), 0x10)),
                ),
                succ=(0x10,),
            ),
            lir.LirBlock(
                at=0x30,
                insns=(
                    _insn(0x30, ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(5, 2),), (ir.Held(4, 2),)), (5,), (4,)),
                    _insn(0x32, ir.Semantics(ir.Operation.RETURN, "ret", (), ()), (), (5,)),
                ),
                succ=(),
            ),
        ),
        origin={},
        pins={},
    )


def _run(body: lir.LirBody) -> dict[int, int]:
    """Values after executing the body: moves, add, cmp, jge, jmp and ret."""
    blocks = {block.at: block for block in body.blocks}
    values: dict[int, int] = {}
    at, flags = body.entry, 0

    def read(x) -> int:
        return x.value if isinstance(x, ir.Imm) else values[x.value]

    for _step in range(1000):
        block, following = blocks[at], None
        for one in block.insns:
            what = one.what
            match what.op:
                case ir.Operation.MOVE:
                    values[what.dests[0].value] = read(what.sources[0])
                case ir.Operation.BINARY:
                    values[what.dests[0].value] = read(what.sources[0]) + read(what.sources[1])
                case ir.Operation.COMPARE:
                    flags = read(what.sources[0]) - read(what.sources[1])
                case ir.Operation.BRANCH:
                    following = what.target if flags >= 0 else next(one for one in block.succ if one != what.target)
                case ir.Operation.JUMP:
                    following = what.target
                case ir.Operation.RETURN:
                    return values
        at = following if following is not None else block.succ[0]
    raise AssertionError("the loop never ended")


def test_a_cut_loop_counter_keeps_its_latch_value() -> None:
    """A piece carved over a loop's header and latch took its copy in at the
    header's top, which the back edge runs too: the latch's increment was
    overwritten with the value from before it, and pal_bestfit under --opt
    looped forever."""
    assert _run(_counting_loop())[5] == 10
    body = splitkit.split(_counting_loop(), frozenset({4}))
    assert any(4 not in one.uses for one in body.insns if one.at in (0x10, 0x20) and one.uses), "nothing was cut"
    assert _run(body)[5] == 10
