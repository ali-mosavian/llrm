from pathlib import Path

import pytest

from tests import corpus
from qbopt.model import ir
from qbopt.model import mir
from qbopt.backend import select
from qbopt.frontend import blocks
from qbopt.frontend import declen
from qbopt.objectfile import module
from qbopt.frontend import fppatches


def test_renderer_dot_product_has_explicit_register_addition() -> None:
    # r_cull_box's final dot-product addition at 02ba was an unknown
    # memory write, severing the numeric and loop-counter value chains.
    found = corpus.loaded(Path("fixtures/regressions/r_walk-borland.obj"))
    assert found is not None
    mapped = blocks.code_map(found)
    assert not isinstance(mapped, str)
    found = module.of(fppatches.native_records(found, mapped.starts))
    assert found is not None
    mapped = blocks.code_map(found)
    assert not isinstance(mapped, str)
    body = next(body for _, body in mir.bodies(found, blocks.partition(found, mapped)) if body.entry == 0)
    addition = next(op for block in body.blocks for op in block.ops if op.at == 0x2BA)
    assert not addition.barrier
    assert addition.kind is mir.Kind.FADD
    assert not addition.loads and not addition.stores
    assert addition.stack == 0
    assert addition.floating is not None
    assert addition.floating.inputs == ("extended80", "extended80")
    assert addition.floating.precision == "dynamic"
    assert addition.floating.rounding == "dynamic"


@pytest.mark.parametrize(
    "encoded,name,dest,source",
    [
        ("d8c1", "fadd", 0, 1),
        ("dcc1", "fadd", 1, 0),
        ("d8c9", "fmul", 0, 1),
        ("dcc9", "fmul", 1, 0),
        ("d8e1", "fsub", 0, 1),
        ("dce9", "fsub", 1, 0),
        ("d8f1", "fdiv", 0, 1),
        ("dcf9", "fdiv", 1, 0),
    ],
)
def test_register_arithmetic_preserves_operand_order_and_encoding(
    encoded: str,
    name: str,
    dest: int,
    source: int,
) -> None:
    code = bytes.fromhex(encoded)
    decoded = declen.decode(code, 0)
    assert decoded is not None
    what = ir.instruction_semantics(decoded, lambda *_: None)
    assert what.op is ir.Operation.FLOAT_ARITH
    assert what.name == name
    assert what.dests == (ir.St(dest),)
    assert what.sources == (ir.St(dest), ir.St(source))
    emitted = select.emit(what)
    assert emitted is not None
    assert emitted.code == code
