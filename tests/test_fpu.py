"""
qbopt/backend/fpu.py: the emulator's interrupt, replaced by what it stands for.

This is the one rewrite in the pass that changes what the output needs to
run -- from any 8086 to one with a coprocessor -- so it is off unless asked
for, and what is tested here is mostly that it refuses.
"""

from pathlib import Path

import pytest
from iced_x86 import Decoder

import corpus
from qbopt.backend import fpu
from qbopt.frontend.declen import ESC
from qbopt.frontend.declen import Stands
from qbopt.frontend.declen import BITNESS
from qbopt.frontend.declen import EMULATED
from qbopt.frontend.declen import INTERRUPT
from qbopt.frontend.blocks import instructions

FIXTURES = sorted(Path("fixtures/omf").glob("*.obj"))
WAIT = 0x9B


def test_vbdos_platform_float_access_keeps_the_emulators_es_override() -> None:
    """Native qrender printed plat_zofs 0 for -288 after losing INT 3Ch's ES override."""
    from iced_x86 import Register, Encoder
    from qbopt.frontend.blocks import code_map, partition

    found = corpus.loaded(Path("fixtures/regressions/qrender-ent-v-g3.obj"))
    decoded = {one.at: one for block in partition(found, code_map(found)) for one in block.insns}
    for at in (0x729, 0x744):
        insn = decoded[at]
        assert insn.segment_override == Register.ES
        encoder = Encoder(16)
        encoder.encode(insn.insn, 0)
        code = encoder.take_buffer()
        assert code[0] == 0x26
        from qbopt.backend.select import Emitted
        wrapped = fpu.wrapped(Emitted(code), Stands.SEGMENTED)
        assert wrapped.code == found.code[at:insn.end]


@pytest.mark.parametrize("native,protocol,wanted,shift", [
    ("d9860000", 0x35, "cd35860000", 1),
    ("dd860000", 0x39, "cd39860000", 1),
    ("d9860000", 0x3c, "cd3cd9860000", 2),
    ("26d9860000", 0x3c, "cd3cd9860000", 1),
])
def test_emulator_reencoding_moves_relocation_fields(native: str, protocol: int, wanted: str, shift: int) -> None:
    """nbody's FLD must follow allocation without changing its emulator protocol."""
    from qbopt.backend.select import Emitted

    displacement = len(bytes.fromhex(native)) - 2
    made = fpu.wrapped(Emitted(bytes.fromhex(native), displacement_at=displacement,
                               fields=(displacement,)), protocol)
    assert made.code == bytes.fromhex(wanted)
    assert made.displacement_at == displacement + shift
    assert made.fields == (displacement + shift,)


def test_emulator_wait_remains_an_emulator_wait() -> None:
    from qbopt.backend.select import Emitted

    assert fpu.wrapped(Emitted(b"\x9b"), 0x3d).code == b"\xcd\x3d"
    assert fpu.wrapped(Emitted(b"\x67\xd9\x00"), 0x35) is None
    assert fpu.wrapped(Emitted(b"\x36\xd9\x07"), 0x3c) is None


def test_unknown_compiler_does_not_inherit_vbdos_emulator_segment() -> None:
    """The qrender ES fix must not invent a segment for another runtime dialect."""
    from dataclasses import replace
    from iced_x86 import Register
    from qbopt.frontend.blocks import decoded_instruction

    found = corpus.loaded(Path("fixtures/regressions/qrender-ent-v-g3.obj"))
    unknown = replace(found, records=[record for record in found.records if record.type != 0x88])
    assert decoded_instruction(unknown, 0x729).segment_override == Register.NONE


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


# The fpemu-* fixtures carry 503 emulator sites across fifteen
# configurations, so the parametrized tests above are real. They were not:
# before those objects existed, zero of the 110 fixtures held a site and
# every one of those tests would have stayed green if this module returned
# None for everything.
#
# These stay anyway. They pin the protocol itself -- the one Open Watcom's
# fppatche.h names -- rather than whatever BC happened to emit, and they are
# the only cover for int 3Dh, which fpemu does not produce.
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
    from qbopt.frontend.declen import decode

    found = decode(raw, 0)
    assert found is not None, f"declen decoded nothing from {raw.hex()}"
    made = fpu.native(raw, found)
    assert made == want, f"{raw.hex()} -> {made.hex() if made else None}, wanted {want.hex()}"
    if text is not None:
        assert str(found.insn) == text


def test_a_hand_built_segment_override_is_refused() -> None:
    """int 3Ch, with a real ESC opcode following it."""
    from qbopt.frontend.declen import decode

    raw = bytes([INTERRUPT, Stands.SEGMENTED, 0xD9, 0x06, 0x00, 0x00])
    found = decode(raw, 0)
    assert found is not None
    assert fpu.native(raw, found) is None


def test_an_ordinary_interrupt_is_refused() -> None:
    from qbopt.frontend.declen import decode

    raw = bytes([INTERRUPT, 0x21, 0x90, 0x90])
    found = decode(raw, 0)
    assert found is not None
    assert fpu.native(raw, found) is None


def test_the_fixtures_really_carry_emulator_sites() -> None:
    """What makes every parametrized test in this module mean something.

    They were all vacuous until suite/fpemu.bas was built into fixtures/omf:
    no object in the corpus was compiled under /FPi, so there was nothing for
    them to walk. A test that would pass against a function returning None is
    not evidence, and this is what says it no longer is.
    """
    total = 0
    for obj in FIXTURES:
        found, reached = sites(obj)
        total += sum(1 for one in reached if fpu.native(found.code, one) is not None)
    assert total > 400, f"only {total} convertible sites in the whole corpus"
