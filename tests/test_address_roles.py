"""Fast regressions for native 16-bit address-role selection."""

from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import select
from qbopt.backend import allocate
from qbopt.objectfile.module import Addr
from qbopt.objectfile.module import Space


def _instruction(at: int, what: ir.Semantics, defines: tuple[int, ...] = (), uses: tuple[int, ...] = ()) -> lir.Insn:
    return lir.Insn(at, (at, at), what, defines, uses)


def _frame_load(at: int, value: int, displacement: int) -> lir.Insn:
    return _instruction(
        at,
        ir.Semantics(
            ir.Operation.MOVE,
            "mov",
            (ir.Held(value, 2),),
            (ir.Mem(Addr(Space.FRAME, displacement), 2, Register.BP),),
        ),
        (value,),
    )


def test_shared_word_index_takes_bx_instead_of_spilling_a_base() -> None:
    """QCport's far_put could not allocate two live pointer bases: both were
    forced into BX while their shared index was forced into SI/DI, and the
    rematerialized base then had no register and could not be spilled again.

    ``[bx+si]`` is commutative.  Give the shared side BX and the two pointer
    bases SI/DI; this native form is cheaper than 67h, spilling or recomputing.
    """
    index = _instruction(
        3,
        ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(3, 2),), (ir.Imm(1, 2),)),
        (3,),
    )
    source = ir.Mem(
        Addr(Space.LITERAL, 0),
        2,
        base=ir.Held(1, 2),
        index=ir.Held(3, 2),
        scale=1,
    )
    destination = ir.Mem(
        Addr(Space.FAR, 0, segment=Register.ES),
        2,
        base=ir.Held(2, 2),
        index=ir.Held(3, 2),
        scale=1,
    )
    read = _instruction(
        4,
        ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(4, 2),), (source,)),
        (4,),
        (1, 3),
    )
    write = _instruction(
        5,
        ir.Semantics(ir.Operation.MOVE, "mov", (destination,), (ir.Held(4, 2),)),
        (),
        (2, 3, 4),
    )
    body = lir.LirBody(
        "shared-index",
        0,
        (lir.LirBlock(0, (_frame_load(1, 1, 4), _frame_load(2, 2, 6), index, read, write), ()),),
        {},
        {},
    )

    assignment = allocate.allocate(body)
    assert not assignment.spilled, assignment
    placed = allocate.applied(body, assignment)
    accesses = [one.what for one in placed.insns if one.at in (4, 5)]
    assert all(select.emit(what) is not None for what in accesses)


def test_word_address_role_keeps_a_call_crossing_base_out_of_bx() -> None:
    """qcport ls_init retained an invariant pointer through a user call.
    A tied one-base/one-index component chose its BX side by value id, even
    though the call destroys BX and preserves the low words of SI and DI.
    Address-role orientation must account for that hard lifetime constraint.
    """
    call = lir.Insn(
        2,
        (2, 2),
        ir.Semantics(ir.Operation.CALL, "call"),
        (),
        (),
        clobbers=frozenset({Register.EAX, Register.EBX, Register.ECX, Register.EDX}),
        clobbers_high=frozenset({Register.ESI, Register.EDI}),
    )
    index = _instruction(
        3,
        ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(2, 2),), (ir.Imm(1, 2),)),
        (2,),
    )
    cell = ir.Mem(Addr(Space.LITERAL, 0), 2, base=ir.Held(1, 2), index=ir.Held(2, 2), scale=1)
    read = _instruction(
        4,
        ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(3, 2),), (cell,)),
        (3,),
        (1, 2),
    )
    body = lir.LirBody(
        "call-crossing-base",
        0,
        (lir.LirBlock(0, (_frame_load(1, 1, 4), call, index, read), ()),),
        {},
        {},
    )

    classes = allocate.classes(body)

    assert classes[1] == frozenset(allocate.target.WORD_INDEXES)
    assert classes[2] == frozenset(allocate.target.WORD_BASES)


def test_unallocatable_retention_plan_falls_back_to_ordinary_spilling() -> None:
    """qcport r_draw_world retained four loop bases, then made a pair of
    one-instruction address reloads unable to use either SI or DI.  Retention
    is a profitability candidate, not a correctness constraint: if its fully
    rewritten plan is unallocatable, discard it and use the ordinary spill
    plan rather than failing compilation.
    """
    definitions = tuple(_frame_load(at, value, -2 * value) for at, value in enumerate((1, 2, 3), start=1))
    reads = tuple(
        _instruction(
            3 + value,
            ir.Semantics(
                ir.Operation.MOVE,
                "mov",
                (ir.Held(10 + value, 2),),
                (ir.Mem(Addr(Space.LITERAL, value), 2, index=ir.Held(value, 2)),),
            ),
            (10 + value,),
            (value,),
        )
        for value in (1, 2, 3)
    )
    body = lir.LirBody(
        "retention-fallback",
        0,
        (lir.LirBlock(0, (*definitions, *reads), ()),),
        {},
        {},
    )

    result, retained = allocate._assigned_plan(
        body,
        {},
        frozenset({2}),
        frozenset({1, 3}),
        cpu="386",
    )

    assert retained == frozenset()
    assert 2 not in result.spilled
