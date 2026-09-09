"""Only a proven zero selector bypasses VBDOS's unresolved entry helper."""

from pathlib import Path
from dataclasses import replace

import pytest

from qbopt import omf
from qbopt import module
from qbopt import runtime


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


@pytest.mark.parametrize("variant", ["nonzero", "relocated", "entry_at_call"])
def test_unknown_entry_path_keeps_unknown_contract(variant: str) -> None:
    found = loaded()
    if variant == "nonzero":
        found = replace(found, code=found.code[:0xEE] + b"\x01" + found.code[0xEF:])
    elif variant == "relocated":
        found = replace(found, fixup_at={**found.fixup_at, 0xEE: next(iter(found.fixup_at.values()))})
    else:
        found = replace(found, publics=found.publics | {0xF0})
    assert runtime.for_module(found)[0xF0].inputs is None
