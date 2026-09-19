"""Fast regressions for post-allocation memory operand folding."""

import pytest
from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import masm
from qbopt.backend import peephole
from qbopt.objectfile.module import Addr
from qbopt.objectfile.module import Space


def _insn(
    at: int,
    op: ir.Operation,
    name: str,
    dests: tuple[ir.Loc, ...] = (),
    sources: tuple[ir.Loc, ...] = (),
    target: int | None = None,
) -> lir.Insn:
    return lir.Insn(at, None, ir.Semantics(op, name, dests, sources, target), (), ())


def _body(head: tuple[lir.Insn, ...], eax: ir.Reg, ecx: ir.Reg) -> lir.LirBody:
    overwrite = _insn(4, ir.Operation.MOVE, "mov", (eax,), (ecx,))
    jump = _insn(5, ir.Operation.JUMP, "jmp", target=0)
    return lir.LirBody(
        "delayed-memory-read",
        0,
        (lir.LirBlock(0, head, (1,)), lir.LirBlock(1, (overwrite, jump), (0,))),
        {},
        {},
    )


def test_memory_round_trip_folds_across_an_independent_operand_load() -> None:
    """C nbody used four instructions where BCC writes ``mov vel; add [pos],reg``.

    Loading the addend between the position load and its add is independent:
    it neither changes the position cell nor needs the value loaded from it.
    The late fold may delay the position read to the add and remove both its
    register load and store.
    """
    eax, ecx = ir.Reg(Register.EAX, 4), ir.Reg(Register.ECX, 4)
    position = ir.Mem(Addr(Space.FRAME, -4), 4, Register.BP, 0, 2)
    velocity = ir.Mem(Addr(Space.FRAME, -8), 4, Register.BP, 0, 2)
    head = (
        _insn(0, ir.Operation.MOVE, "mov", (eax,), (position,)),
        _insn(1, ir.Operation.MOVE, "mov", (ecx,), (velocity,)),
        _insn(2, ir.Operation.BINARY, "add", (eax,), (eax, ecx)),
        _insn(3, ir.Operation.MOVE, "mov", (position,), (eax,)),
    )

    result = peephole.fused(_body(head, eax, ecx)).blocks[0]
    printed = [line for one in result.insns for line in masm._instruction(one.what, {}, 0)]

    assert printed == ["mov ecx, dword ptr [bp-8]", "add dword ptr [bp-4], ecx"]


def test_narrow_load_folds_into_its_only_widening_use() -> None:
    """C matmul emitted ``mov cx,[array]; movsx edx,cx`` eight times."""
    from qbopt.backend import verify

    cell = ir.Mem(Addr(Space.FRAME, -8), 2, Register.BP, 0, 2)
    narrow = lir.Insn(
        0,
        None,
        ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg(Register.CX, 2),), (cell,)),
        (1,),
        (),
    )
    wide = lir.Insn(
        1,
        None,
        ir.Semantics(
            ir.Operation.EXTEND,
            "movsx",
            (ir.Reg(Register.EDX, 4),),
            (ir.Reg(Register.CX, 2),),
        ),
        (2,),
        (1,),
    )
    use = _insn(
        2,
        ir.Operation.BINARY,
        "add",
        (ir.Reg(Register.EAX, 4),),
        (ir.Reg(Register.EAX, 4), ir.Reg(Register.EDX, 4)),
    )
    body = lir.LirBody("load-extend", 0, (lir.LirBlock(0, (narrow, wide, use)),), {}, {})

    result = peephole.extensions(body)
    emitted = [one for one in result.insns if one.what.op is not ir.Operation.NOTHING]

    assert emitted[0].what == ir.Semantics(ir.Operation.EXTEND, "movsx", (ir.Reg(Register.EDX, 4),), (cell,))
    assert emitted[0].defines == (2,)
    assert len(emitted) == 2
    assert not verify.verify(result)


def test_shared_narrow_load_is_not_folded_into_one_widening_use() -> None:
    """The load must remain when another instruction still reads its narrow value."""
    cell = ir.Mem(Addr(Space.FRAME, -8), 2, Register.BP, 0, 2)
    narrow = lir.Insn(
        0,
        None,
        ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg(Register.CX, 2),), (cell,)),
        (1,),
        (),
    )
    wide = lir.Insn(
        1,
        None,
        ir.Semantics(
            ir.Operation.EXTEND,
            "movsx",
            (ir.Reg(Register.EDX, 4),),
            (ir.Reg(Register.CX, 2),),
        ),
        (2,),
        (1,),
    )
    other = lir.Insn(2, None, ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Reg(Register.CX, 2),)), (), (1,))
    body = lir.LirBody("shared-load", 0, (lir.LirBlock(0, (narrow, wide, other)),), {}, {})

    assert peephole.extensions(body) == body


def test_one_use_compare_folds_before_a_complete_return() -> None:
    """indexed.lru_use loaded a sign test into DI before immediately returning.

    The C epilogue restores the caller's DI from the stack, so DI's transient
    body value is dead on the return edge.  Treating callee-saved registers as
    semantic return operands retained ``mov di,[m]; or di,di`` instead of the
    direct memory comparison emitted by GCC and Clang.
    """
    from qbopt.model import mir

    di = ir.Reg(Register.DI, 2)
    cell = ir.Mem(Addr(Space.FRAME, -4), 2, Register.BP, 0, 2)
    load = lir.Insn(1, None, ir.Semantics(ir.Operation.MOVE, "mov", (di,), (cell,)), (1,), ())
    compare = lir.Insn(
        2,
        None,
        ir.Semantics(ir.Operation.COMPARE, "cmp", (), (di, ir.Imm(0, 2))),
        (),
        (1,),
    )
    branch = _insn(3, ir.Operation.BRANCH, "jge", target=2)
    returned = mir.Op(4, ir.Operation.RETURN, "", (), (), kind=mir.Kind.RETURN, reads_complete=True)
    ret = lir.Insn(4, None, ir.Semantics(ir.Operation.RETURN, ""), (), (), op=returned)
    body = lir.LirBody(
        "complete-return-fold",
        0,
        (
            lir.LirBlock(0, (load, compare, branch), (1, 2)),
            lir.LirBlock(1, (ret,), ()),
            lir.LirBlock(2, (ret,), ()),
        ),
        {},
        {},
    )

    result = peephole.fused(body)
    physical = [one.what for one in result.blocks[0].insns if one.what.op is not ir.Operation.NOTHING]

    assert physical == [ir.Semantics(ir.Operation.COMPARE, "cmp", (), (cell, ir.Imm(0, 2))), branch.what]


def test_dead_compare_load_may_overwrite_its_own_address_register() -> None:
    """lru_use's bnext test used DI for both the pointer and loaded value.

    Folding the one-use load preserves the old DI pointer instead of replacing
    it with the loaded word.  That is observable for a later use, but when DI
    is dead after the comparison it is precisely what permits ``cmp [di],0``.
    Read/modify/write folding must retain the stricter refusal.
    """
    di = ir.Reg(Register.DI, 2)
    cell = ir.Mem(Addr(Space.SEGMENT, 0, base=Register.DI), 2, Register.DI, 0, 2)
    load = _insn(1, ir.Operation.MOVE, "mov", (di,), (cell,))
    compare = _insn(2, ir.Operation.COMPARE, "cmp", sources=(di, ir.Imm(0, 2)))
    overwrite = _insn(3, ir.Operation.MOVE, "mov", (di,), (ir.Reg(Register.AX, 2),))
    body = lir.LirBody(
        "self-addressed-compare",
        0,
        (lir.LirBlock(0, (load, compare, overwrite, _insn(4, ir.Operation.JUMP, "jmp", target=0)), (0,)),),
        {},
        {},
    )

    result = peephole.fused(body)
    physical = [one.what for one in result.insns if one.what.op is not ir.Operation.NOTHING]

    assert physical[0] == ir.Semantics(ir.Operation.COMPARE, "cmp", (), (cell, ir.Imm(0, 2)))


@pytest.mark.parametrize("hazard", ["uses loaded value", "changes address", "writes memory"])
def test_memory_round_trip_does_not_cross_a_dependent_or_writing_instruction(hazard: str) -> None:
    """Delayed memory folding must not change which cell or value an add reads.

    A materialization can depend on the loaded value, replace a register that
    addresses the round-tripped cell, or write a possibly-aliasing cell.  The
    position read cannot move past any of those operations.
    """
    eax, ebx, ecx = (ir.Reg(one, 4) for one in (Register.EAX, Register.EBX, Register.ECX))
    through = Register.BX if hazard == "changes address" else Register.BP
    position = ir.Mem(
        Addr(
            Space.SEGMENT if through == Register.BX else Space.FRAME,
            0 if through == Register.BX else -4,
            base=through if through == Register.BX else Register.NONE,
        ),
        4,
        through,
        0,
        2,
    )
    velocity = ir.Mem(Addr(Space.FRAME, -8), 4, Register.BP, 0, 2)

    other = ecx
    if hazard == "uses loaded value":
        between = _insn(1, ir.Operation.MOVE, "mov", (ecx,), (ir.Mem(None, 4, Register.EAX, 0, 4),))
    elif hazard == "changes address":
        between = _insn(1, ir.Operation.MOVE, "mov", (ebx,), (velocity,))
        other = ebx
    else:
        between = _insn(1, ir.Operation.MOVE, "mov", (position,), (ecx,))
    head = (
        _insn(0, ir.Operation.MOVE, "mov", (eax,), (position,)),
        between,
        _insn(2, ir.Operation.BINARY, "add", (eax,), (eax, other)),
        _insn(3, ir.Operation.MOVE, "mov", (position,), (eax,)),
    )

    assert peephole.fused(_body(head, eax, ecx)).blocks[0].insns == head
