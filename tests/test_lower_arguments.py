"""FPCSE's constant PRINT addresses need no temporary register."""

import re
from collections import Counter
from dataclasses import replace

import pytest
from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import frame
from qbopt.backend import lower
from qbopt.backend import allocate
from qbopt.objectfile.module import Addr
from qbopt.objectfile.module import Space


def test_fpdeep_literal_addresses_do_not_spill():
    """FPDEEP QB cost rose 1572 to 1652 when shared PRINT addresses spilled.

    Float-to-integer lowering now legitimately uses dword conversion slots;
    test the address-spill symptom (a word store/push), not every BP access.
    """
    from pathlib import Path

    import corpus
    from qbopt import wholeseg

    result = wholeseg.emitted(Path("fixtures/omf/fpdeep-q-O.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    instructions = [str(one.insn) for block in corpus.partitioned(result.data) for one in block.insns]
    spill = re.compile(r"^(?:mov (?:word ptr )?\[bp-|push (?:word ptr )?\[bp-)")
    assert not any(spill.match(one) for one in instructions)


@pytest.mark.parametrize("symbolic", [False, True])
@pytest.mark.parametrize("width", [2, 4])
def test_shared_literal_pushes_keep_each_site_and_address(symbolic, width):
    owner = object()
    immediate = ir.Imm(0, width, Addr(Space.SEGMENT, 8, 5) if symbolic else None)
    copy = lir.Insn(
        0, (0, 3), ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(1, width),), (immediate,)), (1,), (), op=owner
    )
    push = lir.Insn(3, (3, 4), ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Held(1, width),)), (), (1,))
    call = lir.Insn(4, (4, 9), None, (), ())
    second = replace(push, at=9, covers=(9, 10))
    result = lower._rematerialized_arguments((copy, push, call, second), Counter({1: 2}), set())
    assert len(result) == 3
    first, kept_call, last = result
    assert kept_call == call
    assert first.covers == (0, 4) and last.covers == (9, 10)
    assert first.at == 3 and last.at == 9
    for one in (first, last):
        assert one.what.sources == (immediate,)
        assert one.op is owner and one.symbol is symbolic
        assert one.rematerialized
        assert not one.uses and not one.defines


@pytest.mark.parametrize("hazard", ["other_use", "exposed", "width", "requires", "before_definition"])
def test_rematerialization_retains_other_observations(hazard):
    from iced_x86 import Register

    copy = lir.Insn(0, (0, 3), ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(1, 2),), (ir.Imm(7, 2),)), (1,), ())
    push = lir.Insn(3, (3, 4), ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Held(1, 2),)), (), (1,))
    if hazard == "width":
        push = replace(push, what=replace(push.what, sources=(ir.Held(1, 4),)))
    if hazard == "requires":
        push = replace(push, requires=((ir.Held(1, 2), Register.AX),))
    insns = (push, copy) if hazard == "before_definition" else (copy, push)
    result = lower._rematerialized_arguments(
        insns, Counter({1: 2 if hazard == "other_use" else 1}), {1} if hazard == "exposed" else set()
    )
    assert copy in result
    if hazard in ("width", "requires", "before_definition"):
        assert result == insns


def test_rematerialization_keeps_value_shared_with_a_call_input():
    """EVTRAP put the handler offset on the stack before AX held it.

    B$ONTA receives that offset both as its far-address stack argument and
    in AX.  Recreating the PUSH independently left AX to be recreated at
    the call, reversing the required ``push cs / mov ax,offset / push ax``
    registration sequence.
    """
    address = ir.Imm(0, 2, Addr(Space.SEGMENT, 0xF0, 1))
    copy = lir.Insn(
        0, (0, 3), ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(1, 2),), (address,)), (1,), ()
    )
    push = lir.Insn(3, (3, 4), ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Held(1, 2),)), (), (1,))
    call = lir.Insn(
        4,
        (4, 9),
        ir.Semantics(ir.Operation.CALL, "call", (), (ir.Held(1, 2),)),
        (),
        (1,),
    )
    assert lower._rematerialized_arguments((copy, push, call), Counter({1: 2}), set()) == (copy, push, call)


@pytest.mark.parametrize("symbolic", [False, True])
def test_immediate_push_keeps_definition_relocation_owner(symbolic):
    owner = object()
    immediate = ir.Imm(0, 2, Addr(Space.SEGMENT, 8, 5) if symbolic else None)
    copy = lir.Insn(
        0,
        (0, 3),
        ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(1, 2),), (immediate,)),
        (1,),
        (),
        op=owner,
        symbol=True if symbolic else False,
    )
    push = lir.Insn(3, (3, 4), ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Held(1, 2),)), (), (1,))
    result = lower._immediate_arguments((copy, push), Counter({1: 1}))
    assert len(result) == 1
    assert result[0].what == ir.Semantics(ir.Operation.PUSH, "push", (), (immediate,))
    assert result[0].op is owner and result[0].symbol == copy.symbol
    assert result[0].covers == (0, 4)
    assert not result[0].defines and not result[0].uses


@pytest.mark.parametrize("obstacle", ["live", "gap", "different_value", "different_width", "instruction"])
def test_immediate_push_requires_exclusive_use_and_adjacent_ownership(obstacle):
    copy = lir.Insn(0, (0, 3), ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(1, 2),), (ir.Imm(7, 2),)), (1,), ())
    push = lir.Insn(3, (3, 4), ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Held(1, 2),)), (), (1,))
    match obstacle:
        case "gap":
            push = replace(push, covers=(4, 5))
        case "different_value":
            push = replace(push, what=replace(push.what, sources=(ir.Held(2, 2),)), uses=(2,))
        case "different_width":
            push = replace(push, what=replace(push.what, sources=(ir.Held(1, 4),)))
    insns = (copy, lir.Insn(3, (3, 3), None, (), ()), push) if obstacle == "instruction" else (copy, push)
    assert lower._immediate_arguments(insns, Counter({1: 2 if obstacle == "live" else 1})) == insns


def test_single_use_argument_load_folds_into_its_push():
    """ls_face_styles loaded both incoming arguments into registers used only
    by PUSH, costing two instructions in every loop iteration."""
    first = ir.Mem(Addr(Space.FRAME, 8), 2, Register.BP, offset=8, disp_width=1)
    second = ir.Mem(Addr(Space.FRAME, 6), 2, Register.BP, offset=6, disp_width=1)
    load_first = lir.Insn(0, (0, 3), ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(1, 2),), (first,)), (1,), ())
    load_second = lir.Insn(3, (3, 6), ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(2, 2),), (second,)), (2,), ())
    push_other = lir.Insn(6, (6, 7), ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Held(3, 2),)), (), (3,))
    push_first = lir.Insn(7, (7, 8), ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Held(1, 2),)), (), (1,))
    push_second = lir.Insn(8, (8, 9), ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Held(2, 2),)), (), (2,))

    result = lower._memory_arguments(
        (load_first, load_second, push_other, push_first, push_second), Counter({1: 1, 2: 1, 3: 1}), set()
    )

    assert [one.what.sources for one in result] == [(ir.Held(3, 2),), (first,), (second,)]
    assert [one.covers for one in result] == [(0, 7), (7, 8), (8, 9)]
    assert all(not one.defines and not one.uses for one in result[1:])


@pytest.mark.parametrize("obstacle", ["store", "call", "local", "stack_relative", "other_use", "exposed"])
def test_memory_argument_folding_preserves_observable_loads(obstacle):
    offset = -2 if obstacle == "local" else 8
    source = ir.Mem(
        Addr(Space.FRAME, offset),
        2,
        Register.SP if obstacle == "stack_relative" else Register.BP,
        offset=offset,
        disp_width=1,
    )
    load = lir.Insn(0, (0, 3), ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(1, 2),), (source,)), (1,), ())
    push = lir.Insn(6, (6, 7), ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Held(1, 2),)), (), (1,))
    middle = ()
    if obstacle == "store":
        target = ir.Mem(Addr(Space.FRAME, 8), 2, Register.BP, offset=8, disp_width=1)
        middle = (lir.Insn(3, (3, 6), ir.Semantics(ir.Operation.MOVE, "mov", (target,), (ir.Held(2, 2),)), (), (2,)),)
    elif obstacle == "call":
        middle = (lir.Insn(3, (3, 6), ir.Semantics(ir.Operation.CALL, "call"), (), ()),)

    result = lower._memory_arguments(
        (load, *middle, push),
        Counter({1: 2 if obstacle == "other_use" else 1}),
        {1} if obstacle == "exposed" else set(),
    )

    assert result == (load, *middle, push)


@pytest.mark.parametrize("keep", ["none", "read", "original", "source_load", "opaque"])
def test_dead_inserted_copy_chain_does_not_erase_observable_values(keep):
    """Direct FPCSE pushes exposed an unused allocator reload/copy chain."""
    load = lir.Insn(
        0,
        (0, 0),
        ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(1, 2),), (frame.Frame(0).cell(1, 2),)),
        (1,),
        (),
        spill_reload=keep != "source_load",
    )
    copy = lir.Insn(
        1,
        (1, 2) if keep == "original" else (1, 1),
        ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(2, 2),), (ir.Held(1, 2),)),
        (2,),
        (1,),
    )
    insns = (load, copy)
    if keep in ("read", "opaque"):
        insns += (lir.Insn(2, (2, 3), None, (), (2,) if keep == "read" else ()),)
    body = lir.LirBody("dead", 0, (lir.LirBlock(0, insns, ()),), {}, {})
    result = allocate._dead_insertions(body)
    assert result.insns == (() if keep == "none" else (load,) if keep == "source_load" else insns)
