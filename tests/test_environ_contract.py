import pytest

from qbopt.abi import runtime


def test_vbdos_environ_keeps_general_inputs_and_unknown_effects() -> None:
    """Qrender S_GET_BLASTER refused at B$FEVS, leaving snd unoptimized."""
    rule = runtime.per_call({0: "B$FEVS"}, "vbdos")[0]
    assert rule.inputs == frozenset(
        {runtime.Reg.AX, runtime.Reg.BX, runtime.Reg.CX, runtime.Reg.DX, runtime.Reg.SI, runtime.Reg.DI}
    )
    assert rule.cleanup is None
    assert rule.clobbers == runtime.EVERY
    assert rule.reads is runtime.Memory.ANY and rule.writes is runtime.Memory.ANY
    assert rule.control is runtime.Control.UNKNOWN
    assert rule.raises_error and runtime.barrier(rule)


@pytest.mark.parametrize("family", ["qb45", "pds71", ""])
def test_environ_evidence_does_not_claim_other_runtime_versions(family: str) -> None:
    assert runtime.per_call({0: "B$FEVS"}, family)[0].inputs is None
