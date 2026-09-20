"""Choose the destructive operand without changing arithmetic or constraints."""

from dataclasses import replace
from pathlib import Path

import pytest
from iced_x86 import Mnemonic, OpKind, RegisterExt

from qbopt.frontend import blocks
from qbopt.model import ir, lir
from qbopt.objectfile import module, omf
from qbopt.backend import twoaddr
from qbopt import wholeseg


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
    copy, tied = twoaddr._untied(chosen, iter(range(1000, 2000)).__next__)
    assert copy.what.sources == (ir.Held(2, 4),)
    assert tied.what.sources == (ir.Held(3, 4), ir.Held(1, 4))


def test_multiply_reuses_the_dying_operand() -> None:
    """Matmul tied ``imul`` to its live, spilled factor and reloaded it eight times."""
    one = addition("imul")
    one = replace(one, what=replace(one.what, op=ir.Operation.MULTIPLY))

    chosen = twoaddr._commuted(one, frozenset({1, 3}))

    assert chosen.what.sources == tuple(reversed(one.what.sources))


@pytest.mark.parametrize("name", ["sub", "adc", "sbb", "shl"])
def test_noncommutative_or_implicit_arithmetic_is_not_swapped(name):
    one = addition(name)
    assert twoaddr._commuted(one, frozenset({1, 3})) is one


def test_live_operands_and_grouped_operations_keep_their_order():
    one = addition()
    assert twoaddr._commuted(one, frozenset({1, 2, 3})) is one
    grouped = replace(one, group=1)
    assert twoaddr._commuted(grouped, frozenset({1, 3})) is grouped


@pytest.mark.parametrize("alive,swapped", [(frozenset({3}), True), (frozenset({2, 3}), False),
                                          (frozenset({1, 2, 3}), False)])
def test_result_copy_affinity_does_not_override_liveness(alive, swapped):
    """LOCALP's backedge copy favors its accumulator only when its old value can be overwritten."""
    one = addition()
    chosen = twoaddr._commuted(one, alive, {3: {2}})
    assert chosen.what.sources == (tuple(reversed(one.what.sources)) if swapped else one.what.sources)


def test_crc32_ties_the_operand_that_can_join_its_loop_phi() -> None:
    """CRC32 tied XOR to its shifted temporary, then copied the result around the backedge.

    Both operands die at XOR, but only the masked operand can share the
    result's loop-carried value: the shifted operand overlaps it earlier in
    the iteration.  Prefer the tie that leaves phi coalescing legal.
    """
    one = addition("xor")
    copies = {3: {4}, 4: {3}}
    interference = {1: {4}, 4: {1}}

    chosen = twoaddr._commuted(one, frozenset({3}), copies, interference)

    assert chosen.what.sources == tuple(reversed(one.what.sources))


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_localp_updates_the_accumulator_without_a_loop_copy(tag):
    """LOCALP copied every LONG sum back because ADD tied to the temporary index."""
    result = wholeseg.emitted(Path(f"fixtures/regressions/localp-{tag}.obj".lower()).read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    found = module.of(omf.parse(result.data))
    mapped = blocks.code_map(found)
    assert not isinstance(mapped, str), mapped
    insns = [one for block in blocks.partition(found, mapped) for one in block.insns]
    backedge, = [one for one in insns if one.insn.mnemonic == Mnemonic.JLE
                 and one.insn.near_branch_target < one.at]
    loop = [one.insn for one in insns if backedge.insn.near_branch_target <= one.at < backedge.at]
    assert any(one.mnemonic == Mnemonic.ADD for one in loop)
    assert not any(one.mnemonic == Mnemonic.MOV and one.op0_kind == one.op1_kind == OpKind.REGISTER
                   and RegisterExt.size(one.op0_register) == 4 for one in loop)
