from tools import innerloops


def _record(kind: int, body: bytes) -> bytes:
    return bytes([kind]) + (len(body) + 1).to_bytes(2, "little") + body + b"\0"


def _object(code: bytes, name: str, at: int = 0) -> bytes:
    lnames = b"".join(bytes([len(one)]) + one for one in (b"S_CODE", b"BC_CODE"))
    return (
        _record(0x96, lnames)
        + _record(0x98, bytes([0x60]) + len(code).to_bytes(2, "little") + bytes([1, 2, 1]))
        + _record(0x90, bytes([0, 1, len(name)]) + name.encode() + at.to_bytes(2, "little") + b"\0")
        + _record(0xA0, bytes([1, 0, 0]) + code)
    )


def test_counts_only_the_innermost_call_free_loop_as_decoded():
    """The scoreboard read a dump taken before encoding, not the object's code."""
    code = bytes.fromhex(
        "31c9"  # xor cx,cx
        "b80a00"  # outer: mov ax,10
        "26030d"  # inner: add cx,es:[di]
        "83c702"  # add di,2
        "48"  # dec ax
        "75f7"  # jne inner
        "4b"  # dec bx
        "75f0"  # jne outer
        "9a00000000"  # again: call far 0:0
        "4b"  # dec bx
        "75f8"  # jne again
        "cb"  # retf
    )
    found = innerloops.loops(_object(code, "SUM"))
    assert [(one.name, one.size, one.memory) for one in found] == [("SUM#0", 4, 1)]


def test_a_main_modules_header_is_not_decoded_as_code():
    """stride's header bytes ran on into its loop, so the loop's first
    instruction decoded off its boundary and the loop went missing."""
    header = b"blSTRIDE  " + bytes(34) + b"\xff\xff\x80\x10"
    code = header + bytes.fromhex(
        "660fbfc3"  # loop: movsx eax,bx
        "83c305"  # add bx,5
        "75f7"  # jne loop
        "cb"  # retf
    )
    found = innerloops.loops(_object(code, "M"))
    assert [(one.at, one.size) for one in found] == [(48, 3)]


def test_data_between_procedures_is_not_decoded_as_code():
    """Bytes after a return are not code unless something reaches them."""
    code = bytes.fromhex(
        "c3"  # ret
        "ff"  # a data byte
        "660fbfc3"  # SUM: movsx eax,bx
        "83c305"  # add bx,5
        "75f7"  # jne SUM
        "c3"  # ret
    )
    found = innerloops.loops(_object(code, "SUM", at=2))
    assert [(one.at, one.size) for one in found] == [(2, 3)]



def test_a_loop_is_its_blocks_not_the_addresses_it_spans():
    """A rotated loop whose exit block sits between its test and its body
    (Open Watcom's layout) was read as the address range from test to back
    jump, so the exit's `mov` and `retf` counted as two loop instructions."""
    code = bytes.fromhex(
        "31c9"  # xor cx,cx
        "39d9"  # head: cmp cx,bx
        "7c03"  # jl body
        "89c8"  # mov ax,cx
        "cb"  # retf
        "030d"  # body: add cx,[di]
        "83c702"  # add di,2
        "ebf2"  # jmp head
    )
    found = innerloops.loops(_object(code, "SUM"))
    assert [(one.name, one.size, one.memory) for one in found] == [("SUM#0", 5, 1)]


def test_a_backward_jump_that_is_no_back_edge_makes_no_loop():
    """A jump back to a shared tail its source is not dominated by (llrm's
    layout of a divide's slow path) was taken for a back edge, and the walk
    back from it took in the whole function: an 848-instruction "loop"."""
    code = bytes.fromhex(
        "85c0"  # test ax,ax
        "7405"  # je slow
        "31c0"  # tail: xor ax,ax
        "cb"  # retf
        "90"  # nop
        "90"  # nop
        "40"  # slow: inc ax
        "ebf8"  # jmp tail
    )
    assert innerloops.loops(_object(code, "F"), calls=True) == []


def test_a_static_procedure_s_loop_is_not_charged_to_the_public_before_it():
    """C's static helpers have no public name, so their loops were named for
    the case function laid out before them: fixscale showed 48 loops."""
    code = bytes.fromhex(
        "e80100"  # PUB: call helper
        "cb"  # retf
        "49"  # helper: dec cx
        "75fd"  # jne helper
        "c3"  # ret
    )
    found = innerloops.loops(_object(code, "PUB"))
    assert [one.name for one in found] == ["sub_0004#0"]


def _sieve(tmp_path, *flags):
    import subprocess
    import sys

    sys.path.insert(0, str(__import__("pathlib").Path(__file__).resolve().parents[1] / "tools"))
    import llrmbin

    source = tmp_path / "sieve.c"
    source.write_text("extern void report(long v);\nstatic unsigned char s[4096];\nlong bench_sieve(int n) {\n    long c = 0; int i, j;\n    for (i = 2; i < n; i++) if (!s[i]) { c++; for (j = i + i; j < 4096; j += i) s[j] = 1; }\n    return c;\n}\n")
    out = tmp_path / "sieve.obj"
    subprocess.run([str(llrmbin.bin_dir() / "llrm-c"), "-m32", "-mabi=sysv", "-march=i486", "-O2", *flags, str(source), "-o", str(out)], check=True, capture_output=True)
    return out.read_bytes()


def test_a_32_bit_omf_object_yields_its_inner_loop(tmp_path):
    """innerloops decoded everything as 16-bit code and returned no loops for a -m32 object, without a word: sieve's loop at 0x59..0x63 is
    plain in objdump."""
    found = innerloops.loops(_sieve(tmp_path))
    assert found and all(one.size >= 3 for one in found), found
    assert any("sieve" in one.name for one in found), [one.name for one in found]
    # decoded as 32-bit code: 16-bit decoding of the same bytes found a loop of other instructions (a `mov` through `sp` and `bp`)
    assert any(" e" in line.replace("[e", " e") for one in found for line in one.lines), [one.lines for one in found]


def test_a_32_bit_elf_object_yields_its_inner_loop(tmp_path):
    found = innerloops.loops(_sieve(tmp_path, "-fobject-format=elf"))
    assert found and any("sieve" in one.name for one in found), found
    assert any(" e" in line.replace("[e", " e") for one in found for line in one.lines), [one.lines for one in found]


def test_an_elf_objects_width_is_32_unless_said_otherwise(tmp_path):
    """gcc-ia16's ELF32 objects hold 16-bit code and say nothing of it, so `bits=16` is how they are read; the default is the native 32."""
    data = _sieve(tmp_path, "-fobject-format=elf")
    wide = [line for one in innerloops.loops(data) for line in one.lines]
    narrow = [line for one in innerloops.loops(data, bits=16) for line in one.lines]
    # Decoded as 32-bit code: some line names a 32-bit register, whichever the allocator chose for this loop.
    assert any(" e" in line.replace("[e", " e") for line in wide)
    assert wide != narrow
