"""Qrender script loading refused FREEFILE (0a23) and fixed-string load (0a3f)."""

import pytest

from qbopt.abi import runtime


def test_peos_register_interface_does_not_claim_fixed_stack_cleanup():
    """Qrender INPUT epilogue at 0aa2 refused; terminal INPUT can relocate SP."""
    contract = runtime.per_call({0: "B$PEOS"}, "vbdos")[0]
    assert contract.inputs == frozenset({runtime.Reg.AX, runtime.Reg.BX, runtime.Reg.CX,
                                         runtime.Reg.DX, runtime.Reg.SI, runtime.Reg.DI})
    assert contract.cleanup is None
    assert contract.clobbers == runtime.EVERY
    assert contract.control is runtime.Control.UNKNOWN
    assert contract.writes is runtime.Memory.ANY


@pytest.mark.parametrize("name,cleanup", [
    ("B$FREF", 0), ("B$LDFS", 6), ("B$OPEN", 8), ("B$DSKI", 2),
    ("B$FEOF", 2), ("B$CLOS", None), ("B$ERAS", 2), ("B$FLEN", 2),
])
def test_vbdos_file_setup_retains_unknown_effects(name, cleanup):
    """Qrender refused file calls and common's far-string LEN; CLOSE varies in arity."""
    contract = runtime.per_call({0: name}, "vbdos")[0]
    assert contract.cleanup == cleanup
    assert contract.inputs == frozenset({runtime.Reg.AX, runtime.Reg.BX, runtime.Reg.CX,
                                         runtime.Reg.DX, runtime.Reg.SI, runtime.Reg.DI})
    assert contract.clobbers == runtime.EVERY
    assert contract.reads is runtime.Memory.ANY
    assert contract.writes is runtime.Memory.ANY
    assert contract.control is runtime.Control.UNKNOWN
    assert contract.raises_error
