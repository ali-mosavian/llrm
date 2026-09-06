"""
qbopt/coalesce.py's own gate: a join is a claim that two values are one,
and the claim has to hold for every reader of either of them.
"""

from qbopt import coalesce
from qbopt import ir
from qbopt import lir


def _move(at: int, into: int, out_of: int) -> lir.Insn:
    what = ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(into, 2),), (ir.Held(out_of, 2),))
    return lir.Insn(at=at, covers=(at, at + 2), what=what, defines=(into,), uses=(out_of,), op=None)


def _use(at: int, reads: int) -> lir.Insn:
    what = ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Held(reads, 2),))
    return lir.Insn(at=at, covers=(at, at + 1), what=what, defines=(), uses=(reads,), op=None)


def _define(at: int, makes: int) -> lir.Insn:
    what = ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(makes, 2),), (ir.Imm(makes, 2),))
    return lir.Insn(at=at, covers=(at, at + 3), what=what, defines=(makes,), uses=(), op=None)


def _jump(at: int, to: int) -> lir.Insn:
    return lir.Insn(
        at=at, covers=(at, at + 2), what=ir.Semantics(ir.Operation.JUMP, "jmp", (), (), to),
        defines=(), uses=(), op=None,
    )


def test_two_copies_into_one_value_leave_every_read_defined() -> None:
    """bools-p-evt: `0x7c` read v63 and nothing defined it.

    Phi elimination writes one copy per predecessor, so two of them define
    the phi's result. The coalescer joined the first -- swap[2] = 63 --
    and then joined the second against what that made, swap[63] = 61. It
    then renamed in one pass: a read of v2 became v63, while the only
    definition of v63 became v61. The value read is not the value written,
    and the allocator saw a use live from the top of the body: twelve
    values wanted a stack slot every round and five objects stopped being
    written.

    A join is transitive or it is not a join.
    """
    # One arm each, and the copy phi elimination writes at the end of it.
    body = lir.LirBody(
        name="two arms",
        entry=0,
        blocks=(
            lir.LirBlock(at=0, insns=(_define(0, 61), _move(3, 2, 61), _jump(5, 0x20)), succ=(0x20,)),
            lir.LirBlock(at=0x10, insns=(_define(0x10, 63), _move(0x13, 2, 63), _jump(0x15, 0x20)), succ=(0x20,)),
            lir.LirBlock(at=0x20, insns=(_use(0x20, 2),), succ=()),
        ),
        origin={},
        pins={},
    )
    done = coalesce.joined(body)
    made = {one for block in done.blocks for insn in block.insns for one in insn.defines}
    read = {one for block in done.blocks for insn in block.insns for one in insn.uses}
    assert not (read - made), f"read with nothing defining it: {sorted(read - made)}"


def test_a_join_that_would_make_a_class_uncolourable_is_refused() -> None:
    """divmod-p-g2's 244th legal join left the allocator nothing to place.

    Disjoint live intervals make a join legal, not colourable. That join
    merged v423 into v437 -- both wanting eax, neither pinned against the
    other -- into a class of 33 segments spanning the whole body, with 16
    interference neighbours against six registers. Everything the
    allocator tried afterwards spilled, and there is no undo here, so the
    question has to be asked before the join.
    """
    from pathlib import Path

    from qbopt import allocate
    from qbopt import blocks as split
    from qbopt import flow
    from qbopt import frame as frames
    from qbopt import lower
    from qbopt import mir
    from qbopt import module
    from qbopt import omf
    from qbopt.blocks import code_map

    found = module.of(omf.parse(Path("fixtures/omf/divmod-p-g2.obj").read_bytes()))
    blocks = split.partition(found, code_map(found))
    (name, body), = list(mir.bodies(found, blocks))
    low = lower.lowered(name, body, found.calls)
    pinned = flow._pinned(body)
    for phase in flow.machine(pinned, None, found.calls):
        low = phase.transform(low)
    assert low, "the allocator refused the body"
