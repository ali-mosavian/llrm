"""Choose the destructive operand without changing arithmetic or constraints."""

from dataclasses import replace
from pathlib import Path

import pytest
from iced_x86 import Mnemonic, OpKind

from qbopt import blocks, ir, lir, module, omf, twoaddr, wholeseg


def addition(name="add"):
    return lir.Insn(at=0, covers=(0, 2), defines=(3,), uses=(1, 2),
                    what=ir.Semantics(ir.Operation.BINARY, name, (ir.Held(3, 4),),
                                      (ir.Held(1, 4), ir.Held(2, 4))))


@pytest.mark.parametrize("name", ["add", "and", "or", "xor"])
def test_commutative_instruction_reuses_the_dying_operand(name):
    """LNGMXX copied its accumulator out and back each iteration to preserve the invariant addend."""
    one = addition(name)
    chosen = twoaddr._commuted(one, frozenset({1, 3}))
    assert chosen.what.sources == tuple(reversed(one.what.sources))
    assert chosen.uses == one.uses and chosen.defines == one.defines
    assert chosen.covers == one.covers
    copy, tied = twoaddr._untied(chosen)
    assert copy.what.sources == (ir.Held(2, 4),)
    assert tied.what.sources == (ir.Held(3, 4), ir.Held(1, 4))


@pytest.mark.parametrize("name", ["sub", "adc", "sbb", "shl"])
def test_noncommutative_or_implicit_arithmetic_is_not_swapped(name):
    one = addition(name)
    assert twoaddr._commuted(one, frozenset({1, 3})) is one


def test_live_operands_and_grouped_operations_keep_their_order():
    one = addition()
    assert twoaddr._commuted(one, frozenset({1, 2, 3})) is one
    grouped = replace(one, group=1)
    assert twoaddr._commuted(grouped, frozenset({1, 3})) is grouped


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_lngmxx_loop_has_no_accumulator_copy_roundtrip(tag):
    """LNGMXX emitted MOV temp,sum / ADD temp,invariant / MOV sum,temp on each of ten iterations."""
    result = wholeseg.emitted(Path(f"fixtures/omf/lngmxx-{tag}.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    found = module.of(omf.parse(result.data))
    reached = [one.insn for one in blocks.instructions(found)]
    backedges = [one for one in reached if one.is_jcc_short_or_near and one.near_branch_target < one.ip]
    assert backedges
    for branch in backedges:
        assert not any(one.mnemonic == Mnemonic.MOV and one.op0_kind == one.op1_kind == OpKind.REGISTER
                       for one in reached if branch.near_branch_target <= one.ip < branch.ip)
