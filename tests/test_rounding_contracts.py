"""Qrender QGLDIFF could not lower its runtime INT rounding calls."""

from dataclasses import replace
from pathlib import Path

import pytest
import corpus

from qbopt.abi import runtime
from qbopt.backend import lower
from qbopt.model import mir


@pytest.mark.parametrize("symbol", ["B$STR4", "B$STR8"])
def test_vbdos_string_conversion_keeps_unproved_effects(symbol):
    """H_BENCH formatting was refused; an input bound must not invent purity or cleanup."""
    rule = runtime.per_call({0: symbol}, "vbdos")[0]
    assert rule.inputs == frozenset({runtime.Reg.AX, runtime.Reg.BX, runtime.Reg.CX,
                                    runtime.Reg.DX, runtime.Reg.SI, runtime.Reg.DI})
    assert rule.cleanup is None
    assert rule.clobbers == runtime.EVERY
    assert rule.reads is runtime.Memory.ANY and rule.writes is runtime.Memory.ANY
    assert rule.control is runtime.Control.UNKNOWN and rule.raises_error
    assert runtime.per_call({0: symbol}, "qb45")[0].inputs is None


@pytest.mark.parametrize("symbol", ["B$INT4", "B$INT8"])
def test_vbdos_rounding_keeps_unknown_effects(symbol):
    rule = runtime.per_call({0: symbol}, "vbdos")[0]
    assert rule.cleanup == 0
    assert rule.inputs == frozenset({runtime.Reg.AX, runtime.Reg.BX, runtime.Reg.CX,
                                    runtime.Reg.DX, runtime.Reg.SI, runtime.Reg.DI})
    assert rule.clobbers == runtime.EVERY
    assert rule.reads is runtime.Memory.ANY and rule.writes is runtime.Memory.ANY
    assert rule.control is runtime.Control.UNKNOWN and rule.raises_error
    assert runtime.per_call({0: symbol}, "qb45")[0].inputs is None


@pytest.mark.parametrize("module,symbol", [
    ("qgldiff", "B$INT4"), ("qglface", "B$POW8"),
    ("h-bench", "B$STR4"), ("h-bench", "B$STR8"),
])
def test_qrender_math_calls_lower_without_replacement(module, symbol):
    """QGLDIFF INT4, QGLFACE POW8 and H_BENCH STR calls refused optimized OBJ emission."""
    path = Path(f"fixtures/regressions/qrender-{module}-v-g3.obj")
    found = corpus.loaded(path)
    rules = runtime.for_module(found)
    seen = 0
    for name, body in mir.bodies(found, corpus.partitioned(path), rules):
        for block in body.blocks:
            for op in block.ops:
                if found.calls.get(op.at) != symbol:
                    continue
                isolated = replace(body, entry=block.at,
                    blocks=(replace(block, phis=(), ops=(op,), succ=()),))
                result = lower.lowered(name, isolated, found.calls, found.absorbed, rules)
                assert any(one.at == op.at and one.what.name == "call" for one in result.insns)
                seen += 1
    assert seen > 0
