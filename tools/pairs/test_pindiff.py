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


def test_a_binary_behind_the_integration_branch_is_reported():
    """An llrm-llc built on an older parity reported dag-vs-old m16 gaps that the DAG fixes since had closed (2026-10-27): the header named
    only md5 and the build's sha, so nothing said the binary was 40 commits stale."""
    import subprocess
    import tempfile

    pindiff = load("pindiff")
    with tempfile.TemporaryDirectory() as tmp:
        git = lambda *a: subprocess.run(["git", "-C", tmp, "-c", "user.name=t", "-c", "user.email=t@t", *a], capture_output=True, text=True, check=True).stdout.strip()
        git("init", "-q", "-b", "main")
        shas = []
        for i in range(4):
            git("commit", "-q", "--allow-empty", "-m", str(i))
            shas.append(git("rev-parse", "HEAD"))
        assert pindiff.commits_behind(f"llrm-llc {shas[1]}", "main", tmp) == (shas[1], 2)
        assert pindiff.commits_behind("llrm-llc unknown", "main", tmp) == (None, None)
        assert "WARNING" in pindiff.staleness(f"llrm-llc {shas[0]}", "main", tmp, limit=2)
        assert pindiff.staleness(f"llrm-llc {shas[2]}", "main", tmp, limit=2) == ""


def test_two_objects_differing_only_in_alignment_have_no_padding_free_delta():
    """Function alignment moved a small row's bytes in steps of 16: six m32 rows read +12 B (16 B more padding before `main`, 4 B less in it)
    and were chased as a per-module cost (2026-10-27)."""
    pindiff = load("pindiff")
    omf = load("omfcode")
    body = bytes.fromhex("b890000000" "b090" "c3")  # mov eax,0x90; mov al,0x90; ret: 0x90 inside an immediate is code
    one = lnames("", "X_TEXT", "CODE") + segdef(2, 3) + ledata(1, body + b"\x90" * 5 + body)
    other = lnames("", "X_TEXT", "CODE") + segdef(2, 3) + ledata(1, body + b"\x66\x90" + bytes.fromhex("0f1f440000") + body)
    sizes = [omf.code_bytes_of(obj)[0] for obj in (one, other)]
    assert sizes[0] != sizes[1]
    free = [pindiff.padding_free(omf.code_blob_of(obj), 32) for obj in (one, other)]
    assert free[0] == free[1] == 2 * len(body)


def test_a_shift_by_the_type_width_in_raw_ir_is_flagged():
    """x_fir at m16 compared the pin's `xor eax,eax` (22 insns) with ours: `unsigned s >> 16` is `lshr i16 %s, 16`, UB, and the pin folded
    the function away. The row read +190 B at -O2 and was chased as a gap (2026-10-27)."""
    guard = load("irguard")
    assert guard.undefined_shifts("  %33 = lshr i16 %32, 16\n  %9 = shl nuw i8 %a, 9\n  %x = ashr exact i32 %v, 32") == [
        "lshr i16 %32, 16",
        "shl nuw i8 %a, 9",
        "ashr exact i32 %v, 32",
    ]
    assert guard.undefined_shifts("  %1 = lshr i16 %0, 15\n  %2 = shl i32 %a, 16\n  %3 = lshr i32 %b, %c") == []
