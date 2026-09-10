"""FPCSE's constant PRINT addresses need no temporary register."""

from collections import Counter
from dataclasses import replace

import pytest

from qbopt.backend import lower
from qbopt.backend import allocate, frame
from qbopt.model import ir, lir
from qbopt.objectfile.module import Addr, Space


def test_fpdeep_literal_addresses_do_not_spill():
    """FPDEEP QB cost rose 1572 to 1652 when shared PRINT addresses spilled."""
    from pathlib import Path
    import corpus
    from qbopt import wholeseg

    result = wholeseg.emitted(Path("fixtures/omf/fpdeep-q-O.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    instructions = [str(one.insn) for block in corpus.partitioned(result.data) for one in block.insns]
    assert not any("[bp-" in one for one in instructions)


@pytest.mark.parametrize("symbolic", [False, True])
@pytest.mark.parametrize("width", [2, 4])
def test_shared_literal_pushes_keep_each_site_and_address(symbolic, width):
    owner = object()
    immediate = ir.Imm(0, width, Addr(Space.SEGMENT, 8, 5) if symbolic else None)
    copy = lir.Insn(0, (0, 3), ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(1, width),),
                    (immediate,)), (1,), (), op=owner)
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
        assert not one.uses and not one.defines


@pytest.mark.parametrize("hazard", ["other_use", "exposed", "width", "requires", "before_definition"])
def test_rematerialization_retains_other_observations(hazard):
    from iced_x86 import Register
    copy = lir.Insn(0, (0, 3), ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(1, 2),),
                    (ir.Imm(7, 2),)), (1,), ())
    push = lir.Insn(3, (3, 4), ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Held(1, 2),)), (), (1,))
    if hazard == "width":
        push = replace(push, what=replace(push.what, sources=(ir.Held(1, 4),)))
    if hazard == "requires":
        push = replace(push, requires=((ir.Held(1, 2), Register.AX),))
    insns = (push, copy) if hazard == "before_definition" else (copy, push)
    result = lower._rematerialized_arguments(insns, Counter({1: 2 if hazard == "other_use" else 1}),
                                            {1} if hazard == "exposed" else set())
    assert copy in result
    if hazard in ("width", "requires", "before_definition"):
        assert result == insns


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
