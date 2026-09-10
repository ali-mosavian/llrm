"""FPCSE's constant PRINT addresses need no temporary register."""

from collections import Counter
from dataclasses import replace

import pytest

from qbopt.backend import lower
from qbopt.backend import allocate, frame
from qbopt.model import ir, lir
from qbopt.objectfile.module import Addr, Space


@pytest.mark.parametrize("symbolic", [False, True])
def test_immediate_push_keeps_definition_relocation_owner(symbolic):
    owner = object()
    immediate = ir.Imm(0, 2, Addr(Space.SEGMENT, 8, 5) if symbolic else None)
    copy = lir.Insn(0, (0, 3), ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(1, 2),), (immediate,)),
                    (1,), (), op=owner, symbol=True if symbolic else False)
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
    insns = ((copy, lir.Insn(3, (3, 3), None, (), ()), push)
             if obstacle == "instruction" else (copy, push))
    assert lower._immediate_arguments(insns, Counter({1: 2 if obstacle == "live" else 1})) == insns


@pytest.mark.parametrize("keep", ["none", "read", "original", "source_load", "opaque"])
def test_dead_inserted_copy_chain_does_not_erase_observable_values(keep):
    """Direct FPCSE pushes exposed an unused allocator reload/copy chain."""
    load = lir.Insn(0, (0, 0), ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(1, 2),),
                    (frame.Frame(0).cell(1, 2),)), (1,), (), spill_reload=keep != "source_load")
    copy = lir.Insn(1, (1, 2) if keep == "original" else (1, 1),
                    ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(2, 2),), (ir.Held(1, 2),)), (2,), (1,))
    insns = (load, copy)
    if keep in ("read", "opaque"):
        insns += (lir.Insn(2, (2, 3), None, (), (2,) if keep == "read" else ()),)
    body = lir.LirBody("dead", 0, (lir.LirBlock(0, insns, ()),), {}, {})
    result = allocate._dead_insertions(body)
    assert result.insns == (() if keep == "none" else (load,) if keep == "source_load" else insns)
