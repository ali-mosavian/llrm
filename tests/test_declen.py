"""
The instruction length decoder, on bytes built here rather than on a compiled
program.

dectest.py checks the same decoder against ndisasm on real programs, and that
is a different question: agreement between two implementations is not
correctness when both were written from the same wrong idea. These are small,
and exhaustive where they can be.
"""

import re
import shutil
import subprocess

import pytest

from helpers import hx
from qbopt.declen import BAD
from qbopt.declen import length
from qbopt.declen import T as TABLE

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

UNKNOWN = [op for op in range(256) if TABLE[op] == BAD]


@pytest.mark.parametrize(("enc", "want", "what"), FORMS, ids=[f[2] for f in FORMS])
def test_the_forms_bc_and_the_runtime_emit(enc: str, want: int, what: str) -> None:
    assert length(hx(enc), 0) == want, what


def test_some_opcodes_are_unknown() -> None:
    assert UNKNOWN


@pytest.mark.parametrize("op", UNKNOWN[:8], ids=lambda op: f"{op:02X}")
def test_unknown_opcodes_give_up_rather_than_guess(op: int) -> None:
    assert length(bytes([op, 0, 0, 0, 0, 0]), 0) is None


def test_an_unknown_two_byte_opcode_bails() -> None:
    assert length(hx("0F FF"), 0) is None


# A table typo shows up here and nowhere else until it corrupts a program.
@pytest.mark.parametrize("rm", range(8))
@pytest.mark.parametrize("mod", range(4))
def test_every_modrm_under_16_bit_addressing(mod: int, rm: int) -> None:
    body = bytes([0x8B, (mod << 6) | rm]) + b"\x11\x22\x33\x44"
    want = 2
    if mod == 0 and rm == 6:
        want = 4  # [disp16]
    elif mod == 1:
        want = 3  # disp8
    elif mod == 2:
        want = 4  # disp16
    assert length(body, 0) == want


@pytest.mark.parametrize("rm", range(8))
@pytest.mark.parametrize("mod", range(4))
def test_every_modrm_under_32_bit_addressing(mod: int, rm: int) -> None:
    body = bytes([0x67, 0x8B, (mod << 6) | rm, 0x24]) + b"\x11\x22\x33\x44"
    want = 3  # 67 + opcode + modrm
    if rm == 4 and mod != 3:
        want += 1  # a sib byte
    if mod == 0 and rm == 5:
        want += 4  # [disp32]
    elif mod == 1:
        want += 1
    elif mod == 2:
        want += 4
    assert length(body, 0) == want


@pytest.mark.parametrize("op", (0x8B, 0xA1, 0x81, 0x9A, 0x0F, 0xC8, 0xF7), ids=lambda op: f"{op:02X}")
@pytest.mark.parametrize("n", range(1, 6))
def test_never_runs_off_the_end(n: int, op: int) -> None:
    got = length(bytes([op]) * n, 0)
    assert got is None or got <= 15


# ndisasm renders wait, lock and the segment overrides joined to the instruction
# after them; this decoder treats them separately. The stream of boundaries is
# the same either way, so those lines are skipped rather than counted as
# disagreements about length.
JOINED = ("wait", "lock", "rep", "repe", "repne", "repz", "repnz")
JOINED += ("cs", "ds", "es", "ss", "fs", "gs", "a16", "a32", "o16", "o32")


@pytest.fixture(scope="module")
def fuzz() -> tuple[bytes, list[tuple[int, str]]]:
    import random

    random.seed(20260828)
    blob = bytes(random.randrange(256) for _ in range(4000))
    r = subprocess.run(["ndisasm", "-b16", "-"], input=blob, capture_output=True, timeout=30, check=False)
    marks = []
    for ln in r.stdout.decode("latin1").splitlines():
        m = re.match(r"^([0-9A-F]{8})  (\S+)\s+(.*)$", ln)
        if m:
            marks.append((int(m.group(1), 16), m.group(3)))
    return blob, marks


def _compare(blob: bytes, marks: list[tuple[int, str]]) -> tuple[int, list[str]]:
    agree, wrong = 0, []
    for k in range(len(marks) - 1):
        off, txt = marks[k]
        if txt.startswith("db 0x"):
            continue
        # where ndisasm cannot decode what follows a prefix it reports the
        # prefix alone, which is not a claim about length
        if txt.split()[0] in JOINED:
            continue
        want = marks[k + 1][0] - off
        got = length(blob, off)
        if got is None:
            continue
        if got == want:
            agree += 1
        else:
            wrong.append(f"{off:04X} {txt!r}: got {got}, ndisasm says {want}")
    return agree, wrong


@pytest.mark.skipif(shutil.which("ndisasm") is None, reason="ndisasm is not installed")
def test_random_bytes_agree_with_ndisasm(fuzz: tuple[bytes, list[tuple[int, str]]]) -> None:
    agree, wrong = _compare(*fuzz)
    assert wrong == []
    assert agree > 1000
