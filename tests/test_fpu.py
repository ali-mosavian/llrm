"""
qbopt/fpu.py: the emulator's interrupt, replaced by what it stands for.

This is the one rewrite in the pass that changes what the output needs to
run -- from any 8086 to one with a coprocessor -- so it is off unless asked
for, and what is tested here is mostly that it refuses.
"""

from pathlib import Path

import pytest
from iced_x86 import Decoder

import corpus
from qbopt import fpu
from qbopt.declen import ESC
from qbopt.declen import Stands
from qbopt.declen import BITNESS
from qbopt.declen import EMULATED
from qbopt.declen import INTERRUPT
from qbopt.rewrite import instructions

FIXTURES = sorted(Path("fixtures/omf").glob("*.obj"))
WAIT = 0x9B


def sites(obj: Path) -> tuple:
    found = corpus.loaded(obj)
    assert found is not None
    reached = instructions(found)
    if isinstance(reached, str):
        return found, []
    return found, [one for one in reached if fpu.emulated_at(found.code, one.at)]


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_a_conversion_decodes_back_to_the_same_instruction(obj: Path) -> None:
    """The gate, and not a formality.

    declen.py decodes the site as the x87 instruction it stands for. The
    bytes emitted here must decode to that same instruction -- same mnemonic,
    same operands -- or the program computes something else on a machine
    that has the coprocessor to run it.
    """
    found, reached = sites(obj)
    for insn in reached:
        made = fpu.native(found.code, insn)
        if made is None:
            continue
        back = next(iter(Decoder(BITNESS, made, ip=insn.at)))
        assert not back.is_invalid
        assert back.mnemonic == insn.insn.mnemonic, f"{obj.stem} {insn.at:#x}"
        assert back.op_count == insn.insn.op_count
        assert str(back) == str(insn.insn), f"{obj.stem} {insn.at:#x}: {back} != {insn.insn}"


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_a_conversion_is_exactly_one_byte_shorter(obj: Path) -> None:
    """Two bytes of interrupt become one of ESC opcode, and nothing else
    moves -- which is what lets the displacement's fixup shift by exactly
    one rather than be recomputed."""
    found, reached = sites(obj)
    for insn in reached:
        made = fpu.native(found.code, insn)
        if made is not None:
            assert len(made) == insn.length - 1, f"{obj.stem} {insn.at:#x}"


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_the_segment_override_form_is_always_refused(obj: Path) -> None:
    """int 3Ch stands in for an override whose segment is patched at run
    time, so the object does not say which one it is -- declen.py decodes
    the site as though there were none. Emitting a native instruction there
    would mean choosing a segment on no evidence."""
    found, reached = sites(obj)
    for insn in reached:
        if found.code[insn.at + 1] == Stands.SEGMENTED:
            assert fpu.native(found.code, insn) is None, f"{obj.stem} {insn.at:#x}"


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_nothing_that_is_not_an_emulator_site_is_touched(obj: Path) -> None:
    found = corpus.loaded(obj)
    assert found is not None
    reached = instructions(found)
    if isinstance(reached, str):
        return
    for insn in reached:
        if found.code[insn.at : insn.at + 1] != bytes([INTERRUPT]):
            assert fpu.native(found.code, insn) is None, f"{obj.stem} {insn.at:#x}"


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_what_is_emitted_is_an_esc_opcode_or_a_wait(obj: Path) -> None:
    """The only two things the convertible forms stand in for."""
    found, reached = sites(obj)
    for insn in reached:
        made = fpu.native(found.code, insn)
        if made is not None:
            assert made[0] in ESC or made[0] == WAIT, f"{obj.stem} {insn.at:#x}: {made.hex()}"
            assert found.code[insn.at + 1] in EMULATED or found.code[insn.at + 1] == Stands.FWAIT


# fixtures/omf holds no object built under /FPi -- checked, not assumed:
# zero of the 110 contain an emulator site -- so every parametrized test
# above is vacuous today and would stay green if this module returned None
# for everything. These are what actually exercise it, built by hand from
# the protocol Open Watcom's fppatche.h names and decoded by declen.py.
#
#   cd 35 46 c8   int 35h with the operand inline   ->   d9 46 c8
#
EMULATOR_SITES = (
    (bytes([INTERRUPT, 0x35, 0x46, 0xC8]), "fld dword ptr [bp-38h]", bytes([0xD9, 0x46, 0xC8])),
    (bytes([INTERRUPT, 0x39, 0x04]), None, bytes([0xDD, 0x04])),
    (bytes([INTERRUPT, Stands.FWAIT]), "wait", bytes([WAIT])),
)


@pytest.mark.parametrize(("raw", "text", "want"), EMULATOR_SITES, ids=lambda v: str(v)[:24])
def test_a_hand_built_site_converts_to_the_documented_bytes(raw: bytes, text: str | None, want: bytes) -> None:
    from qbopt.declen import decode

    found = decode(raw, 0)
    assert found is not None, f"declen decoded nothing from {raw.hex()}"
    made = fpu.native(raw, found)
    assert made == want, f"{raw.hex()} -> {made.hex() if made else None}, wanted {want.hex()}"
    if text is not None:
        assert str(found.insn) == text


def test_a_hand_built_segment_override_is_refused() -> None:
    """int 3Ch, with a real ESC opcode following it."""
    from qbopt.declen import decode

    raw = bytes([INTERRUPT, Stands.SEGMENTED, 0xD9, 0x06, 0x00, 0x00])
    found = decode(raw, 0)
    assert found is not None
    assert fpu.native(raw, found) is None


def test_an_ordinary_interrupt_is_refused() -> None:
    from qbopt.declen import decode

    raw = bytes([INTERRUPT, 0x21, 0x90, 0x90])
    found = decode(raw, 0)
    assert found is not None
    assert fpu.native(raw, found) is None
