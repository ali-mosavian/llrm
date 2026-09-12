from pathlib import Path
from dataclasses import replace

import pytest

import corpus
from qbopt.abi import runtime
from qbopt.abi import callsite
from qbopt.frontend.declen import decode


def test_external_pascal_interface_keeps_unknown_effects() -> None:
    """Qrender UGL calls were refused solely for an undeclared interface."""
    found = corpus.loaded(Path("fixtures/omf/fpcsex-p-g2.obj"))
    assert found is not None
    at = next(iter(found.calls))
    found = replace(found, calls={**found.calls, at: "EXTERNAL_RENDER"})
    rule = runtime.for_module(found)[at]
    assert rule.inputs == frozenset(
        {runtime.Reg.AX, runtime.Reg.BX, runtime.Reg.CX, runtime.Reg.DX, runtime.Reg.SI, runtime.Reg.DI}
    )
    assert rule.cleanup is None
    assert rule.clobbers == runtime.EVERY
    assert rule.reads is runtime.Memory.ANY and rule.writes is runtime.Memory.ANY
    assert not rule.established and runtime.barrier(rule)
    assert "assumed" in rule.evidence


@pytest.mark.parametrize("name", ["B$UNKNOWN", "b$unknown"])
def test_runtime_helpers_are_not_assumed_to_use_a_language_abi(name: str) -> None:
    found = corpus.loaded(Path("fixtures/omf/fpcsex-p-g2.obj"))
    assert found is not None
    at = next(iter(found.calls))
    found = replace(found, calls={at: name})
    assert runtime.for_module(found)[at].inputs is None


@pytest.mark.parametrize(
    "raw,expected",
    [
        ("83c408", True),
        ("81c40800", True),
        ("83c4fe", False),
        ("83c008", False),
        ("83ec08", False),
        ("90", False),
    ],
)
def test_caller_cleanup_requires_positive_stack_adjustment(raw: str, expected: bool) -> None:
    """A negative or unrelated ADD must not claim C argument cleanup."""
    assert callsite.caller_cleanup(decode(bytes.fromhex(raw), 0)) is expected


def test_explicit_external_contract_overrides_assumption() -> None:
    found = corpus.loaded(Path("fixtures/omf/fpcsex-p-g2.obj"))
    assert found is not None
    at = next(iter(found.calls))
    found = replace(found, calls={at: "EXTERNAL_RENDER"})
    explicit = replace(runtime.worst("EXTERNAL_RENDER"), inputs=frozenset(), cleanup=4)
    assert runtime.for_module(found, external={explicit.name: explicit})[at] == explicit
