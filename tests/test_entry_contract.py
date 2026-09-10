"""Only a proven zero selector bypasses VBDOS's unresolved entry helper."""

from pathlib import Path
from dataclasses import replace

import pytest

from qbopt.objectfile import omf
from qbopt.objectfile import module
from qbopt.abi import runtime
from qbopt import wholeseg


def loaded() -> module.Module:
    return module.of(omf.parse(Path("fixtures/omf/procs-v-g3.obj").read_bytes()))


def test_zero_selector_entry_has_both_fixed_inputs() -> None:
    """PROCS /G3 refused B$ENRA even though BX=0 bypasses its unknown helper."""
    found = loaded()
    contracts = runtime.for_module(found)
    for at in (0xF0, 0x124):
        assert contracts[at].inputs == frozenset({runtime.Reg.BX, runtime.Reg.CX})
        assert contracts[at].clobbers == runtime.EVERY
        assert contracts[at].writes is runtime.Memory.ANY


def test_explicit_external_contract_is_shared_by_call_sites():
    """Qrender cross-module calls had no route for supplying an audited interface."""
    found = loaded()
    name = "PROJECT_HELPER"
    found = replace(found, calls={0x40: name, 0x50: name, 0x60: "UNKNOWN"})
    audited = replace(runtime.worst(name), inputs=frozenset(), cleanup=4,
                      evidence="audited linked object")
    contracts = runtime.for_module(found, external={name: audited})
    assert contracts[0x40] is audited
    assert contracts[0x50] is audited
    assert contracts[0x60].inputs is None
    assert runtime.for_module(found)[0x40].inputs is None


def test_external_contract_cannot_be_applied_under_another_name():
    with pytest.raises(ValueError, match="name"):
        runtime.for_module(loaded(), external={"WRONG": runtime.worst("RIGHT")})


def test_emitter_validates_supplied_external_interfaces():
    with pytest.raises(ValueError, match="name"):
        wholeseg.emitted(Path("fixtures/omf/procs-v-g3.obj").read_bytes(),
                         external_contracts={"WRONG": runtime.worst("RIGHT")})


@pytest.mark.parametrize("variant", ["nonzero", "relocated", "entry_at_call"])
def test_unknown_entry_path_keeps_conservative_contract(variant: str) -> None:
    """COM_PARSE_CONFIG refused ENRA at 022b; only proven BX=0 narrows its inputs."""
    found = loaded()
    if variant == "nonzero":
        found = replace(found, code=found.code[:0xEE] + b"\x01" + found.code[0xEF:])
    elif variant == "relocated":
        found = replace(found, fixup_at={**found.fixup_at, 0xEE: next(iter(found.fixup_at.values()))})
    else:
        found = replace(found, publics=found.publics | {0xF0})
    contract = runtime.for_module(found)[0xF0]
    assert contract.inputs == frozenset(
        {runtime.Reg.AX, runtime.Reg.BX, runtime.Reg.CX,
         runtime.Reg.DX, runtime.Reg.SI, runtime.Reg.DI}
    )
    assert contract.cleanup is None
    assert contract.clobbers == runtime.EVERY
    assert contract.writes is runtime.Memory.ANY
    assert contract.control is runtime.Control.UNKNOWN


def test_vbdos_exit_keeps_every_possible_general_input() -> None:
    """PROCS /G3 refused at B$EXSA despite a conservative six-register input bound."""
    result = wholeseg.emitted(Path("fixtures/omf/procs-v-g3.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    contract = runtime.for_module(loaded())[0x113]
    assert contract.inputs == frozenset(
        {runtime.Reg.AX, runtime.Reg.BX, runtime.Reg.CX, runtime.Reg.DX, runtime.Reg.SI, runtime.Reg.DI}
    )
    assert contract.clobbers == runtime.EVERY
    assert contract.writes is runtime.Memory.ANY
    assert contract.control is runtime.Control.UNKNOWN


def test_vbdos_output_channel_keeps_unknown_effects():
    """Qrender camera refused its PRINT channel selection at 0x8da (B$CHOU)."""
    contract = runtime.per_call({0: "B$CHOU"}, "vbdos")[0]
    assert contract.inputs == frozenset(
        {runtime.Reg.AX, runtime.Reg.BX, runtime.Reg.CX, runtime.Reg.DX, runtime.Reg.SI, runtime.Reg.DI}
    )
    assert contract.cleanup == 2
    assert contract.clobbers == runtime.EVERY
    assert contract.reads is runtime.Memory.ANY
    assert contract.writes is runtime.Memory.ANY
    assert contract.control is runtime.Control.UNKNOWN
    assert contract.raises_error


def test_vbdos_statement_exit_has_stack_neutral_interface():
    """Qrender COM_TOKENIZE refused B$EXTS at 008a after its string-length call."""
    contract = runtime.per_call({0: "B$EXTS"}, "vbdos")[0]
    assert contract.inputs == frozenset(
        {runtime.Reg.AX, runtime.Reg.BX, runtime.Reg.CX,
         runtime.Reg.DX, runtime.Reg.SI, runtime.Reg.DI}
    )
    assert contract.cleanup == 0
    assert contract.clobbers == runtime.EVERY
    assert contract.reads is runtime.Memory.ANY
    assert contract.writes is runtime.Memory.ANY
    assert contract.control is runtime.Control.UNKNOWN
    assert contract.raises_error


@pytest.mark.parametrize("name,cleanup", [
    ("B$PCR4", 4), ("B$PSR4", 4),
    ("B$PCR8", 8), ("B$PSR8", 8), ("B$PER8", 8),
])
def test_vbdos_float_print_interfaces(name, cleanup):
    """Qrender camera refused PRINT's comma-separated SINGLE at 0x8ea."""
    contract = runtime.per_call({0: name}, "vbdos")[0]
    assert contract.cleanup == cleanup
    assert contract.inputs == frozenset(
        {runtime.Reg.AX, runtime.Reg.BX, runtime.Reg.CX, runtime.Reg.DX, runtime.Reg.SI, runtime.Reg.DI}
    )
    assert contract.clobbers == runtime.EVERY
    assert contract.reads is runtime.Memory.ANY
    assert contract.writes is runtime.Memory.ANY
    assert contract.control is runtime.Control.UNKNOWN
    assert contract.raises_error
