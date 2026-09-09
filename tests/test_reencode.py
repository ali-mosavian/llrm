"""
qbopt/backend/reencode.py's own gate: re-encoding with a mapping that changes
nothing must give back the bytes that were read.
"""

from pathlib import Path

import pytest
from iced_x86 import Decoder
from iced_x86 import Register

import corpus
from qbopt.backend import reencode
from qbopt.frontend.declen import run
from qbopt.frontend.declen import Insn
from qbopt.frontend.declen import BITNESS

FIXTURES = sorted(Path("fixtures/omf").glob("*.obj"))


def only(hexes: str) -> tuple[Insn, bytes]:
    code = bytes.fromhex(hexes.replace(" ", ""))
    insns, _ = run(code, 0, len(code))
    return insns[0], code


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_re_encoding_nothing_gives_back_what_was_read(obj: Path) -> None:
    """The whole thing rests on this, and it is checkable before anything
    depends on the result -- which is the only moment it is free."""
    found = corpus.loaded(obj)
    assert found is not None
    for block in corpus.partitioned(obj):
        for insn in block.insns:
            got = reencode.with_registers(insn, {})
            if got is None:
                continue
            assert got.code == found.code[insn.at : insn.end], f"{obj.stem} {insn.at:#06x}"


def test_a_register_moves_at_its_own_width() -> None:
    """A mapping is between 32-bit roots because that is what an SSA value
    is a version of, but the instruction names ax. Moving eax to ecx has to
    give cx and not ecx, or the operand changes width and the instruction
    changes meaning."""
    insn, _ = only("66 8B 46 EE")  # mov eax,[bp-12h]
    got = reencode.with_registers(insn, {Register.EAX: Register.ECX})
    assert got is not None
    assert str(next(iter(Decoder(BITNESS, got.code, ip=0)))) == "mov ecx,[bp-12h]"

    narrow, _ = only("8B 46 EE")  # mov ax,[bp-12h]
    moved = reencode.with_registers(narrow, {Register.EAX: Register.ECX})
    assert moved is not None
    assert str(next(iter(Decoder(BITNESS, moved.code, ip=0)))) == "mov cx,[bp-12h]"


def test_an_accumulator_only_form_is_refused() -> None:
    """`mov eax,[1234h]` is the moffs form, two bytes shorter and available
    to the accumulator alone. Reaching for the general encoding instead
    would be a different length, which is the caller's problem to know
    about rather than this function's to hide."""
    insn, code = only("66 A1 34 12")
    assert reencode.with_registers(insn, {}) is not None, "it re-encodes as itself"
    assert reencode.with_registers(insn, {Register.EAX: Register.ECX}) is None


def test_an_emulated_float_site_is_refused() -> None:
    """declen.py decodes `int 34h`-`3Dh` as the float instruction it stands
    for, so re-encoding emits the native opcode -- `cd 39 04` becomes
    `dd 04`, a different program on a machine with no coprocessor."""
    insn, _ = only("CD 39 04")
    assert reencode.with_registers(insn, {}) is None


def test_a_branch_keeps_its_own_target_and_its_own_form() -> None:
    """Encoded at the address it sits at, or the displacement is measured
    from the wrong place -- and through Encoder rather than BlockEncoder,
    which shortens `e9 0b 00` to `eb 0c` and changes the length."""
    code = bytes.fromhex("E90B00")
    buffer = bytes(0x9A) + code  # run() takes offsets into the buffer, not addresses
    insns, _ = run(buffer, 0x9A, len(buffer))
    got = reencode.with_registers(insns[0], {})
    assert got is not None
    assert got.code == code, "e9 0b 00 stays three bytes; eb 0c would be two"


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_a_re_encoded_instruction_is_the_same_length(obj: Path) -> None:
    for block in corpus.partitioned(obj):
        for insn in block.insns:
            for mapping in ({}, {Register.EAX: Register.ECX}, {Register.ESI: Register.EDI}):
                got = reencode.with_registers(insn, mapping)
                if got is not None:
                    assert got.length == insn.length


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_a_displacement_is_reported_where_it_actually_landed(obj: Path) -> None:
    """A fixup is recorded against a byte offset, and it has to be moved to
    wherever the displacement ended up -- read off the encoded bytes rather
    than predicted."""
    found = corpus.loaded(obj)
    assert found is not None
    for block in corpus.partitioned(obj):
        for insn in block.insns:
            got = reencode.with_registers(insn, {})
            if got is None or insn.disp_at is None:
                continue
            assert got.displacement_at is not None
            assert insn.at + got.displacement_at == insn.disp_at


def test_encoding_somewhere_else_would_retarget_a_branch() -> None:
    """The bug directly, rather than only its absence.

    A relative branch's displacement is measured from its own ip, so
    encoding at zero when the instruction lives at 0x9a silently sends it
    somewhere else. 1,444 of the corpus's instructions came back different
    for this alone, and every one of them would have been a wrong jump.
    """
    from iced_x86 import Encoder

    code = bytes.fromhex("EB10")
    buffer = bytes(0x30) + code
    insns, _ = run(buffer, 0x30, len(buffer))
    here = insns[0].insn.copy()

    wrong = Encoder(BITNESS)
    wrong.encode(here, 0)
    assert wrong.take_buffer() != code, "encoding at zero changes the displacement"

    right = reencode.with_registers(insns[0], {})
    assert right is not None
    assert right.code == code


def test_a_memory_operands_base_moves_with_its_value() -> None:
    """An address computed from a moved value is computed from wherever it
    moved. Leaving the base behind reads a different address entirely."""
    insn, _ = only("66 8B 04")  # mov eax,[si]
    got = reencode.with_registers(insn, {Register.ESI: Register.EDI})
    assert got is not None
    assert "di" in str(next(iter(Decoder(BITNESS, got.code, ip=0))))
