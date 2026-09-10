"""Qrender script loading refused FREEFILE (0a23) and fixed-string load (0a3f)."""

import pytest

from qbopt.abi import runtime


@pytest.mark.parametrize("name,cleanup", [
    ("B$FREF", 0), ("B$LDFS", 6), ("B$OPEN", 8), ("B$DSKI", 2),
])
def test_vbdos_file_setup_retains_unknown_effects(name, cleanup):
    """Script loading stopped at OPEN 0a4b and disk INPUT setup 0a5a."""
    contract = runtime.per_call({0: name}, "vbdos")[0]
    assert contract.cleanup == cleanup
    assert contract.inputs == frozenset({runtime.Reg.AX, runtime.Reg.BX, runtime.Reg.CX,
                                         runtime.Reg.DX, runtime.Reg.SI, runtime.Reg.DI})
    assert contract.clobbers == runtime.EVERY
    assert contract.reads is runtime.Memory.ANY
    assert contract.writes is runtime.Memory.ANY
    assert contract.control is runtime.Control.UNKNOWN
    assert contract.raises_error
