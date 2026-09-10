"""BOOLS retained a conditional jump after computing the constant true AND."""

from pathlib import Path
from dataclasses import replace

import corpus
import pytest

from qbopt import wholeseg
from qbopt.model import ir, mir
from qbopt.optimize import transform


@pytest.mark.parametrize("tag", ["q-O", "p-g2", "v-g3"])
def test_bools_constant_conditions_leave_no_conditional_jump(tag):
    result = wholeseg.emitted(Path(f"fixtures/omf/bools-{tag}.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    instructions = [str(one.insn) for block in corpus.partitioned(result.data) for one in block.insns]
    assert not any(one.startswith("j") and not one.startswith("jmp ") for one in instructions)


@pytest.mark.parametrize("kind,name,answer", [(mir.Kind.AND, "and", True),
                                            (mir.Kind.OR, "or", True),
                                            (mir.Kind.XOR, "xor", False)])
@pytest.mark.parametrize("width", [2, 4])
def test_zero_condition_uses_the_logical_result(kind, name, answer, width):
    result, flags = mir.Value(1, 0), mir.Value(2, 0, flags=True)
    logical = mir.Op(0, ir.Operation.BINARY, name, (result, flags), (), kind=kind,
                     args=(mir.Const(-1, width), mir.Const(-1, width)), results=(mir.Held(result, width),))
    branch = mir.Op(1, ir.Operation.BRANCH, "", (), (flags,), kind=mir.Kind.BRANCH,
                    test=mir.Kind.NE, target=10)
    block = mir.MirBlock(0, (), (logical, branch), (10, 20))
    assert transform._outcome(block, branch, {}, {}) is answer


@pytest.mark.parametrize("hazard", ["unknown", "width", "relational", "other_condition", "barrier"])
def test_logical_condition_requires_exact_known_operands(hazard):
    value, result, flags = mir.Value(1, 0), mir.Value(2, 0), mir.Value(3, 0, flags=True)
    logical = mir.Op(0, ir.Operation.BINARY, "and", (result, flags), (value,), kind=mir.Kind.AND,
                     args=(mir.Held(value, 4), mir.Const(1, 4)), results=(mir.Held(result, 4),))
    branch = mir.Op(1, ir.Operation.BRANCH, "", (), (flags,), kind=mir.Kind.BRANCH,
                    test=mir.Kind.NE, target=10)
    from qbopt.analysis import consts
    facts = {value: consts.Known(1, 4)}
    match hazard:
        case "unknown":
            facts = {}
        case "width":
            facts[value] = consts.Known(1, 2)
        case "relational":
            branch = replace(branch, test=mir.Kind.BELOW)
        case "other_condition":
            branch = replace(branch, uses=(mir.Value(99, 0, flags=True),))
        case "barrier":
            logical = replace(logical, op=ir.Operation.BARRIER)
    block = mir.MirBlock(0, (), (logical, branch), (10, 20))
    assert transform._outcome(block, branch, facts, {}) is None
