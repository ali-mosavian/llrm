"""
The decoder, on bytes built here rather than on a compiled program.

The decoding is iced's now, so these are not a test of a table qbopt wrote. What
they check is the part qbopt depends on and could get wrong: that the forms BC
emits come back with the lengths and field offsets the rest of the pass reads,
and that a second decoder agrees.
"""

import re
import shutil
import subprocess

import pytest

from helpers import hx
from qbopt.declen import run
from qbopt.declen import decode
from qbopt.declen import length

FORMS = [
    ("A1 5E 00", 3, "mov ax,moffs16"),
    ("66 A1 5E 00", 4, "mov eax,moffs32 -- 66 does not change moffs"),
    ("8B 16 60 00", 4, "mov dx,[disp16]"),
    ("8B 46 E8", 3, "mov ax,[bp+disp8]"),
    ("8B 86 00 01", 4, "mov ax,[bp+disp16]"),
    ("8B C1", 2, "mov ax,cx -- register direct"),
    ("83 D2 00", 3, "adc dx,imm8"),
    ("81 C2 00 01", 4, "add dx,imm16"),
    ("66 81 C2 00 01 00 00", 7, "add edx,imm32 under 66"),
    ("F7 D8", 2, "neg ax -- F7 /3 takes no immediate"),
    ("F7 06 5E 00 34 12", 6, "test [disp16],imm16 -- F7 /0 does"),
    ("F6 06 5E 00 34", 5, "test [disp16],imm8"),
    ("0F 85 1A 16", 4, "jnz near"),
    ("0F B6 C1", 3, "movzx"),
    ("0F A4 C1 04", 4, "shld r/m,r,imm8"),
    ("66 FF 36 5E 00", 5, "push dword [disp16]"),
    ("9A 0F 00 15 00", 5, "call far ptr16:16"),
    ("EA 0F 00 15 00", 5, "jmp far ptr16:16"),
    ("C2 08 00", 3, "ret imm16"),
    ("C8 04 00 00", 4, "enter imm16,imm8"),
    ("C4 46 E8", 3, "les ax,[bp+disp8]"),
    ("26 8A 07", 3, "a segment prefix is a prefix"),
    ("F3 A4", 2, "rep movsb"),
    ("67 66 8D 04 80", 5, "lea eax,[eax+eax*4] -- 32-bit addressing"),
]


@pytest.mark.parametrize(("enc", "want", "what"), FORMS, ids=[f[2] for f in FORMS])
def test_the_forms_bc_and_the_runtime_emit(enc: str, want: int, what: str) -> None:
    assert length(hx(enc), 0) == want, what


@pytest.mark.parametrize(
    ("enc", "disp_at", "disp_len", "imm_at", "imm_len"),
    [
        ("66 A1 5E 00", 2, 2, None, 0),  # the moffs is a displacement, not an immediate
        ("8B 46 E8", 2, 1, None, 0),
        ("66 C7 06 00 00 78 56 34 12", 3, 2, 5, 4),  # mov dword [disp16], imm32
        ("7D 09", None, 0, 1, 1),
        ("8B C1", None, 0, None, 0),
    ],
)
def test_where_the_operand_fields_sit(
    enc: str, disp_at: int | None, disp_len: int, imm_at: int | None, imm_len: int
) -> None:
    # These are the offsets a fixup patches, and so the key into a module's
    # operands. Everything downstream is wrong if they are.
    insn = decode(hx(enc), 0)
    assert insn is not None
    assert (insn.disp_at, insn.disp_len) == (disp_at, disp_len)
    assert (insn.imm_at, insn.imm_len) == (imm_at, imm_len)


@pytest.mark.parametrize(
    ("enc", "reads", "writes"),
    [
        ("74 02", True, False),  # jz reads ZF
        ("23 06 5A 00", False, True),  # and writes the lot
        ("8B C1", False, False),  # mov touches none
        ("66 50", False, False),  # nor push
        ("D1 E0", False, True),  # shl -- and OF is left undefined, which counts
    ],
)
def test_what_an_instruction_does_to_the_flags(enc: str, reads: bool, writes: bool) -> None:
    insn = decode(hx(enc), 0)
    assert insn is not None
    assert bool(insn.reads) is reads
    assert bool(insn.writes) is writes


def test_a_shift_leaves_a_flag_undefined_and_that_counts_as_written() -> None:
    # The hand-written table had no notion of this, and a flag left undefined is
    # as dangerous to read as one left wrong.
    insn = decode(hx("D3 E0"), 0)  # shl ax,cl
    assert insn is not None
    assert insn.insn.rflags_undefined
    assert insn.writes & insn.insn.rflags_undefined == insn.insn.rflags_undefined


@pytest.mark.parametrize("enc", ["FF FF", "0F FF", "C4 C0"])
def test_bytes_that_are_not_an_instruction_decode_to_nothing(enc: str) -> None:
    assert decode(hx(enc), 0) is None


@pytest.mark.parametrize("op", ("8B", "A1", "81", "9A", "0F", "C8", "F7"))
@pytest.mark.parametrize("n", range(1, 6))
def test_never_runs_off_the_end(n: int, op: str) -> None:
    code = hx(op) * n
    got = length(code, 0)
    assert got is None or got <= len(code)


def test_a_run_stops_where_it_cannot_go_on() -> None:
    code = hx("90 90 FF FF 90")
    found, gave_up = run(code, 0, len(code))
    assert [insn.at for insn in found] == [0, 1]
    assert gave_up == 2


JOINED = ("wait", "lock", "rep", "repe", "repne", "repz", "repnz")
JOINED += ("cs", "ds", "es", "ss", "fs", "gs", "a16", "a32", "o16", "o32")


@pytest.fixture(scope="module")
def fuzz() -> tuple[bytes, list[tuple[int, str]]]:
    import random

    random.seed(20260828)
    blob = bytes(random.randrange(256) for _ in range(4000))
    found = subprocess.run(["ndisasm", "-b16", "-"], input=blob, capture_output=True, timeout=30, check=False)
    marks = []
    for line in found.stdout.decode("latin1").splitlines():
        seen = re.match(r"^([0-9A-F]{8})  (\S+)\s+(.*)$", line)
        if seen:
            marks.append((int(seen.group(1), 16), seen.group(3)))
    return blob, marks


@pytest.mark.skipif(shutil.which("ndisasm") is None, reason="ndisasm is not installed")
def test_random_bytes_agree_with_ndisasm(fuzz: tuple[bytes, list[tuple[int, str]]]) -> None:
    # Two decoders written by different people from the same manual.
    blob, marks = fuzz
    agree, wrong = 0, []
    for index in range(len(marks) - 1):
        at, text = marks[index]
        if text.startswith("db 0x") or text.split()[0] in JOINED:
            continue
        # ndisasm reads CD 34h..3Bh as an ordinary interrupt. Under /FPi it is
        # the emulator standing in for an ESC opcode, with the x87 operand
        # following inline, so the two decoders differ here on purpose.
        if blob[at] == 0xCD and at + 1 < len(blob) and 0x34 <= blob[at + 1] < 0x3C:
            continue
        want = marks[index + 1][0] - at
        got = length(blob, at)
        if got is None:
            continue
        if got == want:
            agree += 1
        else:
            wrong.append(f"{at:04X} {text!r}: got {got}, ndisasm says {want}")
    assert wrong == []
    assert agree > 1000


@pytest.mark.parametrize(
    ("enc", "length", "what"),
    [
        ("CD 35 46 C8", 4, "fld dword [bp-38h]"),
        ("CD 34 4E C8", 4, "fmul dword [bp-38h]"),
        ("CD 3A C1", 3, "faddp"),
        ("CD 36 06 5E 00", 5, "fiadd word [disp16]"),
    ],
)
def test_the_emulator_interrupt_is_an_x87_instruction(enc: str, length: int, what: str) -> None:
    # Under /FPi, BC emits int 34h..3Bh where the ESC opcode would go, with the
    # operand bytes following inline. A walk that reads the int as two bytes and
    # carries on lands in the middle of the operand -- which is why reachability
    # explained two of qb-qrender's fifteen modules before this, and all fifteen
    # after. There are 2130 of them in that program.
    insn = decode(hx(enc), 0)
    assert insn is not None
    assert insn.length == length, what
    assert insn.disp_at is None or insn.disp_at >= 2, "the operand follows the two-byte int"
    assert insn.writes & 0x3F == 0, "the x87 status word is not the flags register"
