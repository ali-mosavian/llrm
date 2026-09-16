"""
qbopt/backend/coalesce.py's own gate: a join is a claim that two values are one,
and the claim has to hold for every reader of either of them.
"""

import pytest

from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import coalesce


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_matrix_diagonal_stride_needs_no_register_copies(tag):
    """MATRIX copied its diagonal pointer out and back on each of twenty iterations."""
    from pathlib import Path

    from iced_x86 import OpKind
    from iced_x86 import Mnemonic

    from qbopt import wholeseg
    from qbopt.analysis import loops
    from qbopt.objectfile import omf
    from qbopt.frontend import blocks
    from qbopt.objectfile import module

    result = wholeseg.emitted(Path(f"fixtures/omf/matrix-{tag}.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    found = module.of(omf.parse(result.data))
    partition = blocks.partition(found, blocks.code_map(found))
    diagonal = max(loops.loops(partition, 0x30), key=lambda loop: loop.header)
    copies = [
        one
        for block in partition
        if block.at in diagonal.body
        for one in block.insns
        if one.insn.mnemonic == Mnemonic.MOV and one.insn.op0_kind == one.insn.op1_kind == OpKind.REGISTER
    ]
    assert not copies


def test_retained_resource_identity_has_a_legal_encoding():
    """HARR's hoisted selector copy became unencodable mov es,es across a coverage gap."""
    from iced_x86 import Register

    from qbopt.backend import select
    from qbopt.backend import allocate

    body = lir.LirBody("resource-copy", 0, (lir.LirBlock(0, (_move(3, 1, 1),)),), {}, {1: Register.ES})
    result = allocate.applied(body, allocate.allocate(body, body.pins))
    assert len(result.insns) == 1
    assert result.insns[0].covers == body.insns[0].covers
    assert select.emit(result.insns[0].what, 3) is not None
    assert select.emit(result.insns[0].what, 3).code == b"\x90"


def test_equal_resource_values_coalesce_without_consuming_a_gpr():
    """Address-space values pinned to ES were excluded by the GPR-only coalescing domain."""
    from dataclasses import replace

    from iced_x86 import Register

    from qbopt.backend import target
    from qbopt.backend import allocate

    load = replace(
        _define(0, 1),
        what=ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(1, 2),), (ir.Mem(None, 2, Register.BP, 0, 2),)),
    )
    general = tuple(_define(3 + index * 3, 10 + index) for index in range(6))
    reads = tuple(_use(30 + index * 3, 10 + index) for index in range(6))
    body = lir.LirBody(
        "resources",
        0,
        (lir.LirBlock(0, (load, *general, _move(21, 2, 1), _use(26, 1), _use(28, 2), *reads)),),
        {},
        {1: Register.ES, 2: Register.ES},
    )
    done = coalesce.joined(body)
    assert len(done.insns) == len(body.insns) - 1
    result = allocate.allocate(done, done.pins)
    assert not result.spilled
    assert Register.ES in result.where.values()
    assert set(target.AVAILABLE) <= {allocate._whole(reg) for reg in result.where.values()}


@pytest.mark.parametrize("other", ["different_resource", "clobber"])
def test_resource_constraints_survive_coalescing(other):
    from iced_x86 import Register

    from qbopt.backend import allocate

    insns = (_define(0, 1), _move(3, 2, 1), _use(6, 2))
    pins = {1: Register.ES, 2: Register.FS if other == "different_resource" else Register.ES}
    if other == "clobber":
        call = lir.Insn(
            5, (5, 5), ir.Semantics(ir.Operation.CALL, "call", (), ()), (), (), clobbers=frozenset({Register.ES})
        )
        insns = (*insns[:2], call, insns[2])
    body = lir.LirBody("resource-safety", 0, (lir.LirBlock(0, insns),), {}, pins)
    done = coalesce.joined(body)
    if other == "different_resource":
        assert len(done.insns) == len(body.insns)
    else:
        assert allocate.allocate(done, done.pins).spilled


def test_a_copy_can_share_a_register_while_its_equal_source_is_still_read() -> None:
    body = lir.LirBody("equal", 0, (lir.LirBlock(0, (_define(0, 1), _move(3, 2, 1), _use(5, 1), _use(6, 2))),), {}, {})
    done = coalesce.joined(body)
    assert len(done.insns) == 3
    assert done.insns[-1].uses == done.insns[-2].uses


def test_a_source_redefined_while_its_copy_is_live_cannot_share() -> None:
    insns = (_define(0, 1), _move(3, 2, 1), _define(5, 1), _use(8, 1), _use(9, 2))
    body = lir.LirBody("different", 0, (lir.LirBlock(0, insns),), {}, {})
    done = coalesce.joined(body)
    assert len(done.insns) == len(insns)
    assert done.insns[-1].uses != done.insns[-2].uses


def test_parallel_copy_sources_interfere_before_any_destination_is_written() -> None:
    """fibonacci64 returned failure (1) instead of success (0).

    Its backedge simultaneously assigned `current = next` and
    `previous = current`.  Liveness read those phi copies in printed order,
    so it missed that old `current` and `next` coexist before the group and
    coalesced them.  Both Fibonacci state variables then advanced to `next`.
    """
    from dataclasses import replace

    group = (replace(_move(8, 1, 2), group=1), replace(_move(8, 3, 1), group=1))
    insns = (_define(0, 1), _define(1, 3), _move(3, 2, 1), _define(5, 2), *group, _use(9, 1), _use(10, 3))
    body = lir.LirBody("parallel-sources", 0, (lir.LirBlock(0, insns),), {}, {})
    assert 2 in coalesce._interference(body)[1]
    from qbopt.analysis import intervals

    live = intervals.intervals(body)
    assert live[1].overlaps(live[2])


def test_parallel_copy_destinations_interfere_after_all_are_written() -> None:
    """crosscall's total and counter became one value because both began at zero.

    Phi elimination copied that zero to both loop-carried values in one
    parallel group.  Coalescing both destinations with their common source
    made the loop compare its accumulated total with the bound instead of
    comparing its counter.
    """
    from dataclasses import replace

    group = (replace(_move(1, 2, 1), group=1), replace(_move(1, 3, 1), group=1))
    body = lir.LirBody(
        "parallel-destinations",
        0,
        (lir.LirBlock(0, (_define(0, 1), *group, _use(4, 2), _use(5, 3))),),
        {},
        {},
    )
    assert 3 in coalesce._interference(body)[2]
    done = coalesce.joined(body)
    assert done.insns[-2].uses != done.insns[-1].uses


def test_different_entry_values_cannot_share_even_if_copied_later() -> None:
    insns = (_use(0, 1), _use(1, 2), _move(2, 2, 1), _use(4, 2))
    body = lir.LirBody("inputs", 0, (lir.LirBlock(0, insns),), {}, {})
    assert len(coalesce.joined(body).insns) == len(insns)


def test_a_narrow_copy_is_not_equality_of_a_wide_source() -> None:
    from dataclasses import replace

    wide = replace(_use(5, 1), what=ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Held(1, 4),)))
    insns = (_define(0, 1), _move(3, 2, 1), wide, _use(6, 2))
    body = lir.LirBody("partial", 0, (lir.LirBlock(0, insns),), {}, {})
    assert len(coalesce.joined(body).insns) == len(insns)


def test_a_coalesced_address_keeps_its_memory_operand_defined() -> None:
    memory = ir.Mem(addr=None, width=2, base=ir.Held(2, 2))
    load = lir.Insn(
        at=5,
        covers=(5, 7),
        what=ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(3, 2),), (memory,)),
        defines=(3,),
        uses=(2,),
        op=None,
    )
    body = lir.LirBody(
        "address", 0, (lir.LirBlock(0, (_define(0, 1), _move(3, 2, 1), load, _use(7, 1), _use(8, 3))),), {}, {}
    )
    done = coalesce.joined(body)
    made = {value for one in done.insns for value in one.defines}
    for one in done.insns:
        for operand in (*one.what.dests, *one.what.sources):
            assert {value.value for value in ir.values(operand)} <= made


def test_pinned_return_cannot_absorb_an_incompatible_address_class() -> None:
    """nbody's FVAL pointer lost its AX-to-SI copy, leaving FLD with an invalid base."""
    from iced_x86 import Register

    load = lir.Insn(
        at=5,
        covers=(5, 7),
        what=ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (ir.St(0),), (ir.Mem(None, 8, base=ir.Held(2, 2)),)),
        defines=(),
        uses=(2,),
        op=None,
    )
    body = lir.LirBody("pointer", 0, (lir.LirBlock(0, (_define(0, 1), _move(3, 2, 1), load)),), {}, {})
    assert len(coalesce.joined(body, {1: Register.EAX}).insns) == 3
    from dataclasses import replace

    assert len(coalesce.joined(replace(body, pins={1: Register.EAX})).insns) == 3


def test_coalescing_keeps_the_pinned_return_as_representative() -> None:
    """nbody read FIST's result from BX, making its 100-step limit 1857."""
    from iced_x86 import Register

    body = lir.LirBody("return", 0, (lir.LirBlock(0, (_define(0, 1), _move(3, 2, 1), _use(5, 2))),), {}, {})
    done = coalesce.joined(body, {2: Register.EAX})
    assert done.insns[0].defines == (2,)
    assert done.insns[-1].uses == (2,)
    from qbopt.backend import target
    from qbopt.backend import allocate

    for register in (Register.EAX, Register.EBX, Register.ECX, Register.EDX):
        pins = {2: register}
        joined = coalesce.joined(body, pins)
        emitted = allocate.applied(joined, allocate.allocate(joined, pins))
        assert emitted.insns[-1].what.sources == (ir.Reg(target.named(register, 2), 2),)


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
        at=at,
        covers=(at, at + 2),
        what=ir.Semantics(ir.Operation.JUMP, "jmp", (), (), to),
        defines=(),
        uses=(),
        op=None,
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

    from qbopt import flow
    from qbopt.model import mir
    from qbopt.abi import runtime
    from qbopt.backend import lower
    from qbopt.objectfile import omf
    from qbopt.objectfile import module
    from qbopt.frontend import blocks as split
    from qbopt.frontend.blocks import code_map

    found = module.of(omf.parse(Path("fixtures/omf/divmod-p-g2.obj").read_bytes()))
    blocks = split.partition(found, code_map(found))
    name, body = mir.bodies(found, blocks)[0]
    low = lower.lowered(name, body, found.calls, set(found.absorbed), runtime.for_module(found))
    pinned = flow._pinned(body)
    for phase in flow.machine(pinned, None, found.calls):
        low = phase.transform(low)
    assert low, "the allocator refused the body"


@pytest.mark.parametrize("pinned", [False, True])
def test_a_pinned_neighbour_does_not_stop_georges_join(pinned) -> None:
    """NBODYS's loop counters went `mov si,dx / inc si / cmp si,5 / mov dx,si`.

    Briggs refused the counter's copy and George was never asked, because a
    pinned half of `push ebx / pop cx / pop dx` neighboured it. George holds
    around a pin when the pinned value counts as significant.
    """
    from iced_x86 import Register

    long_lived = range(10, 16)
    insns = [
        *(_define(at, one) for at, one in enumerate(long_lived)),
        _define(0x10, 50),
        _define(0x13, 1),
        _move(0x16, 2, 1),
        _use(0x18, 2),
        *(_use(0x20 + at, one) for at, one in enumerate((*long_lived, 50))),
    ]
    pins = {50: Register.BX} if pinned else {}
    body = lir.LirBody("counter", 0, (lir.LirBlock(0, tuple(insns)),), {}, pins)
    assert len(coalesce.joined(body).insns) == len(insns) - 1


def test_nbody_shifts_each_product_where_it_multiplied_it() -> None:
    """nbody's accumulated products went `mov edx,esi / sar edx,9 / add [bp-8],edx`.

    Briggs refused the copy: the product's class is long and busy. George's
    test accepts it, since every neighbour of the shift already neighbours
    the product.
    """
    from iced_x86 import OpKind
    from iced_x86 import Mnemonic
    from test_observers import _nbody_inner_loop

    loop = _nbody_inner_loop()
    accumulated = [
        index
        for index, one in enumerate(loop)
        if one.mnemonic == Mnemonic.ADD and one.op0_kind == OpKind.MEMORY and one.op1_kind == OpKind.REGISTER
    ]
    assert len(accumulated) == 2
    for index in accumulated:
        copied, shifted = loop[index - 2], loop[index - 1]
        assert shifted.mnemonic == Mnemonic.SAR
        assert not (copied.mnemonic == Mnemonic.MOV and copied.op0_register == shifted.op0_register)
