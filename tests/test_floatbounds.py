"""Runtime integer inputs need no concrete constant to prove exact conversion."""

from dataclasses import replace
from pathlib import Path

import corpus
import pytest

from qbopt.analysis import floatbounds
from qbopt.model import mir
from qbopt.model.floating import Format, Precision, Rounding, Semantics
from qbopt.optimize import transform


@pytest.mark.parametrize("format,expected", [(Format.SIGNED16, 1), (Format.SIGNED32, 1), (Format.BINARY32, 2)])
def test_unknown_integer_loads_share_a_value_but_unknown_floats_do_not(format, expected):
    """FPCSEX reloads its runtime input; integer conversion can share without assuming finite REALs."""
    path = Path("fixtures/omf/fpcsex-p-g2.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    block = next(block for block in body.blocks if any(op.kind is mir.Kind.FLOAD for op in block.ops))
    loads = [op for op in block.ops if op.kind is mir.Kind.FLOAD][:2]
    rule = Semantics((format,), Format.EXTENDED80, Precision.EXACT, Rounding.NONE)
    block = replace(block, ops=tuple(replace(op, floating=rule) for op in loads), phis=(), succ=())
    body = replace(body, blocks=(block,), entry=block.at, initial=())
    result = transform.subexpressions(body, found.dgroup)
    assert sum(op.kind is mir.Kind.FLOAD for one in result.blocks for op in one.ops) == expected


@pytest.mark.parametrize("kind,bounds,expected", [
    (mir.Kind.FADD, ((-32768, 32767), (-32768, 32767)), (-65536, 65534)),
    (mir.Kind.FMUL, ((-32768, 32767), (-32768, 32767)), None),
    (mir.Kind.FMUL, ((-100, 100), (-100, 100)), (-10000, 10000)),
    (mir.Kind.FSUB, ((-10, 10), (-10, 10)), (-20, 20)),
    (mir.Kind.FDIV, ((1, 3), (1, 3)), None),
    (mir.Kind.FADD, ((2**24, 2**24), (1, 1)), None),
])
def test_dynamic_arithmetic_requires_exactness_at_every_precision(kind, bounds, expected):
    rule = Semantics((Format.EXTENDED80,) * 2, Format.EXTENDED80, Precision.DYNAMIC, Rounding.DYNAMIC)
    assert floatbounds.evaluated(kind, rule, bounds) == expected
