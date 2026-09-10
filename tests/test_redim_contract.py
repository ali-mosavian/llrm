"""Qrender V_OPEN_SCRIPT refused REDIM at 09d4; cleanup depends on rank."""

from dataclasses import replace
from pathlib import Path

import pytest

from qbopt.abi import runtime
from qbopt.objectfile import module, omf


def loaded():
    return module.of(omf.parse(Path("fixtures/regressions/qrender-view-v-g3.obj").read_bytes()))


@pytest.mark.parametrize("rank", [1, 2, 3])
def test_redim_cleanup_uses_rank_not_type_flags(rank):
    found = loaded()
    found = replace(found, code=found.code[:0x9cf] + bytes([rank]) + found.code[0x9d0:])
    contract = runtime.for_module(found)[0x9d4]
    assert contract.cleanup == 6 + 4 * rank
    assert contract.inputs == frozenset({runtime.Reg.AX, runtime.Reg.BX, runtime.Reg.CX,
                                         runtime.Reg.DX, runtime.Reg.SI, runtime.Reg.DI})
    assert contract.clobbers == runtime.EVERY
    assert contract.writes is runtime.Memory.ANY
    assert contract.control is runtime.Control.UNKNOWN


@pytest.mark.parametrize("variant", ["relocated_rank", "entry_at_descriptor", "unknown_rank"])
def test_unproven_redim_rank_keeps_unknown_cleanup(variant):
    found = loaded()
    match variant:
        case "relocated_rank":
            found = replace(found, fixup_at={**found.fixup_at, 0x9cf: next(iter(found.fixup_at.values()))})
        case "entry_at_descriptor":
            found = replace(found, publics=found.publics | {0x9d1})
        case "unknown_rank":
            found = replace(found, code=found.code[:0x9ce] + b"\x50\x90\x90" + found.code[0x9d1:])
    assert runtime.for_module(found)[0x9d4].cleanup is None
