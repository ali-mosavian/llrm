"""pindiff's instruments: the CPU guard, the OMF code-byte count and the instruction count each had a failure that made a report wrong."""
from __future__ import annotations

import importlib.util
import struct
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent


def load(name: str):
    spec = importlib.util.spec_from_file_location(name, HERE / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_a_pentium4_ir_is_refused_for_an_i486_llc():
    """`clang -m32` IR says pentium4 and llc honours it over -mcpu=i486: cmov and P4 unrolling made every m32 pin diff of 2026-10-27 compare
    two different targets (nbody x87 rows, a 4x LCG unroll, 6 hand-offs sent on them)."""
    guard = load("irguard")
    with pytest.raises(SystemExit, match="pentium4"):
        guard.check('attributes #0 = { "target-cpu"="pentium4" }', "i486", "x.ll")
    guard.check('attributes #0 = { "target-cpu"="i486" }', "i486", "x.ll")
    guard.check("define void @f() { ret void }", "i486", "no attribute at all")


def record(kind: int, body: bytes) -> bytes:
    """An OMF record: type, length (body and checksum byte), body, checksum."""
    return bytes([kind]) + struct.pack("<H", len(body) + 1) + body + b"\0"


def lnames(*names: str) -> bytes:
    return record(0x96, b"".join(bytes([len(name)]) + name.encode() for name in names))


def segdef(name: int, klass: int) -> bytes:
    return record(0x98, bytes([0x48]) + struct.pack("<H", 0) + bytes([name, klass, 1]))


def ledata(segment: int, data: bytes) -> bytes:
    return record(0xA0, bytes([segment]) + struct.pack("<H", 0) + data)


def test_the_code_bytes_of_an_omf_object_leave_the_data_out():
    """m16_bytes counted every LEDATA byte, initialised data included: a module with a 1 KiB table read 1 KiB bigger than the pin's `.text`."""
    omf = load("omfcode")
    obj = lnames("", "X_TEXT", "CODE", "_DATA", "DATA") + segdef(2, 3) + segdef(4, 5) + ledata(1, b"\x90" * 10) + ledata(2, b"\0" * 100)
    assert omf.code_bytes_of(obj) == (10, 100)


def test_the_instruction_count_skips_directives_labels_and_comments():
    """A `.p2align`, a label or `align 16` is not an instruction: counting them put +245 `align` at the head of a gap list."""
    pindiff = load("pindiff")
    text = "\n".join(
        [
            "_f proc near",
            "L0_1:",
            "    mov eax, 1   ; comment",
            "    align 16",
            "    .p2align 4",
            "    rep stosb",
            "    ret # note",
            "_f endp",
        ]
    )
    assert pindiff.instructions(text) == 3


def test_a_module_retargeted_twice_names_the_stack_alignment_once():
    """pindiff hands the pin's `opt` output to `llc`, which retargets again: at m32 (`S32`) the second pass wrote a second `!9999` module flag
    and the pin's llc stopped on `module flag identifiers must be unique` for all 69 programs."""
    pin = load("pin")
    of = pin.target("m32")
    once = pin.retarget("define void @f() {\n  ret void\n}\n", of)
    twice = pin.retarget(once, of)
    assert once.count("override-stack-alignment") == 1
    assert twice.count("override-stack-alignment") == 1


def test_the_default_gives_clang_no_passes_and_reopt_gives_it_the_level():
    """The first ladder ran ours over clang -O3 IR the pin's opt had already run: ours then optimised twice, and nbody_single, crc and recchop
    read as growth that the raw IR (w3's table: crc 58 vs 62) does not show."""
    pindiff = load("pindiff")
    raw = pindiff.front_end_flags("O2", reopt=False)
    assert "-disable-llvm-passes" in raw and "-O2" in raw
    assert "-disable-llvm-passes" not in pindiff.front_end_flags("O2", reopt=True)
