"""An unoptimised rebuild keeps every x87 store BC wrote, in order, and spends no more x87 instructions.

The float allocator re-encodes every float operation, so nothing else says a
change to it kept code nobody optimised. Loads may fold into memory operands;
the stores are what the program observes. The only additions allowed are the
integer conversions materialised through a frame slot.
"""

import re
from pathlib import Path

import pytest
from iced_x86 import Mnemonic

import corpus
from qbopt import wholeseg

_X87 = {value for name, value in vars(Mnemonic).items() if name.startswith("F") and name != "FEMMS"}
_STORES = {Mnemonic.FSTP, Mnemonic.FST, Mnemonic.FISTP, Mnemonic.FIST}
_MATERIALISED = {Mnemonic.FILD, Mnemonic.FISTP}
_OBJECTS = sorted(
    path
    for path in Path("fixtures/omf").glob("*.obj")
    if path.name.startswith(("fpcse-", "fpcsex-", "fpdeep-", "fpemu-", "byref2-q-O"))
) + [Path("fixtures/bench/nbodys-v-g3.obj")]


def _x87(data: bytes) -> list[tuple[int, str]]:
    # Which register reaches a cell is the allocator's answer, not an x87 fact.
    return [
        (one.insn.mnemonic, re.sub(r"\[[^\]]*\]", "[m]", re.sub(r"\b[a-z]s:", "", str(one.insn))))
        for block in corpus.partitioned(data)
        for one in block.insns
        if one.insn.mnemonic in _X87
    ]


@pytest.mark.parametrize("path", _OBJECTS, ids=lambda path: path.stem)
def test_an_unoptimised_rebuild_keeps_bcs_x87_stores(path: Path) -> None:
    """A reused Python id dropped `fld`, `fmul` and both `fstp`s of FPCSE's loop, silently."""
    data = path.read_bytes()
    rebuilt = wholeseg.emitted(data, optimise=False)
    assert rebuilt.outcome is wholeseg.Emission.LIR, rebuilt.reason
    before, after = _x87(data), _x87(rebuilt.data)
    remaining = iter(one for one in after if one[0] in _STORES)
    for one in (one for one in before if one[0] in _STORES):
        for other in remaining:
            if other == one:
                break
            assert other[0] in _MATERIALISED, f"{path.stem}: {other[1]} before {one[1]}"
        else:
            pytest.fail(f"{path.stem}: {one[1]} is gone")
    assert all(other[0] in _MATERIALISED for other in remaining)
    spent = [one for one in after if one[0] not in _MATERIALISED]
    assert len(spent) <= len([one for one in before if one[0] not in _MATERIALISED])
