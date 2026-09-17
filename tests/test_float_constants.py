from pathlib import Path

import pytest
from iced_x86 import Mnemonic

from tests import corpus
from qbopt.model import ir
from qbopt.model import lir
from qbopt.model import mir
from qbopt.backend import select
from qbopt.frontend import blocks
from qbopt.frontend import declen
from qbopt.objectfile import module
from qbopt.backend import floatalloc
from qbopt.frontend import fppatches


def test_renderer_zero_loads_are_exact_values_not_memory_barriers() -> None:
    # r_cull_box has nine FLDZ sites; treating them as unknown writes
    # invalidates the local counter despite this helper making no calls.
    found = corpus.loaded(Path("fixtures/regressions/r_walk-borland.obj"))
    assert found is not None
    mapped = blocks.code_map(found)
    assert not isinstance(mapped, str)
    found = module.of(fppatches.native_records(found, mapped.starts))
    assert found is not None
    mapped = blocks.code_map(found)
    assert not isinstance(mapped, str)
    raised = mir.bodies(found, blocks.partition(found, mapped))
    body = next(body for _, body in raised if body.entry == 0)
    constants = [
        op
        for block in body.blocks
        for op in block.ops
        if isinstance(raised.source.nodes.get(op.id), ir.Opaque)
        and raised.source.nodes[op.id].insn.insn.mnemonic == Mnemonic.FLDZ
    ]
    assert len(constants) == 9
    for op in constants:
        assert not op.barrier
        assert op.kind is mir.Kind.FLOAD
        assert op.args == (mir.Const(0, 2),)
        assert not op.loads and not op.stores
        assert op.stack == 1
        assert op.floating is not None
        assert op.floating.precision == "exact"
        assert op.floating.rounding == "none"


@pytest.mark.parametrize("value,encoded", [(0, b"\xd9\xee"), (1, b"\xd9\xe8")])
@pytest.mark.parametrize("width", [2, 4])
def test_exact_float_constants_need_no_frame(value: int, encoded: bytes, width: int) -> None:
    what = ir.Semantics(ir.Operation.FLOAT_LOAD, "fild", (ir.St(0),), (ir.Imm(value, width),))
    instruction = lir.Insn(0, (0, 2), what, (), ())
    body = lir.LirBody("constant", 0, (lir.LirBlock(0, (instruction,)),), {}, {})
    allocated = floatalloc.allocated(body)
    assert len(allocated.insns) == 1
    emitted = select.emit(allocated.insns[0].what)
    assert emitted is not None
    assert emitted.code == encoded


@pytest.mark.parametrize("value,encoded", [(0, b"\xd9\xee"), (1, b"\xd9\xe8")])
def test_constant_instructions_raise_exact_integer_conversion(value: int, encoded: bytes) -> None:
    decoded = declen.decode(encoded, 0)
    assert decoded is not None
    what = ir.instruction_semantics(decoded, lambda *_: None)
    assert what.op is ir.Operation.FLOAT_LOAD
    assert what.name == "fild"
    assert what.sources == (ir.Imm(value, 2),)
    assert what.dests == (ir.St(0),)
