"""tools/qlb.py reads and writes Quick libraries as docs/formats/qlb.md says. `MINIMAL` is what Microsoft's LINK 3.69
makes of `/Q` on one 16-byte code segment that defines `__aulstart` and `FOO` (a hand-written OMF object, no library):
the whole format in 661 bytes. Against the real libraries (QB45_DIR, PDS71_DIR, VBDOS_DIR) every file must read, check and
write back byte for byte."""
import importlib.util
import os
import struct
import sys
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("qlb", HERE / "qlb.py")
qlb = importlib.util.module_from_spec(spec)
sys.modules["qlb"] = qlb
spec.loader.exec_module(qlb)

MINIMAL = bytes.fromhex(
    "4d5a9300020008002000ffffffff00000000ce8c020001001e000000010028000100300001004c00010054000100030001000e0001001000"
    "0100120001000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"
    "0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"
    "0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"
    "0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"
    "0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"
    "0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"
    "0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"
    "0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"
    "000000000000000000000102030405060708090a0b0c0d0e0f10ea0400000002756c22003400460000000000010083000000000000000000"
    "00000000010058000800000001005c00040000000000050067000000000005006e00000000000000030073000000000003007a0000000100"
    "0000464f4f005f5f61756c7374617274005f6564617461005f656e6400535f544558540053594d424f4c000000"
)


def test_minimal_is_read_whole():
    lib = qlb.parse(MINIMAL)
    assert [(s.name, s.frame, s.offset) for s in lib.code] == [("FOO", 0, 8), ("__aulstart", 0, 4)]
    assert [(s.flags, s.name, s.offset) for s in lib.data] == [(qlb.ABSOLUTE, "_edata", 0), (qlb.ABSOLUTE, "_end", 0)]
    assert [(s.name, s.frame) for s in lib.segments] == [("S_TEXT", 0), ("SYMBOL", 1)]
    assert lib.jump == (4, 0) and lib.symbol_frame == 1 and lib.dgroup == 0 and not lib.stack
    assert lib.image[:2] == bytes([1, 2]) and len(lib.image) == 16
    assert lib.zeros == (0, 0) and lib.routines() == ["FOO"]
    assert qlb.check(MINIMAL) == []


def test_minimal_writes_back_byte_for_byte():
    assert qlb.parse(MINIMAL).emit() == MINIMAL


def test_zero_runs_are_left_out_of_the_file_and_put_back():
    """LINK /Q leaves out the zeros a segment starts with, at the image start and at DGROUP, and writes how many. A reader
    that took the stored bytes for the image put every later symbol and relocation at the wrong address."""
    image = bytes(5) + bytes(range(1, 12)) + bytes(8) + bytes([7] * 8) + bytes(16)  # frame 0 code, 1 DGROUP, 2 padding
    lib = qlb.Qlb(image, [], [qlb.Symbol(qlb.CODE, "A", 5, 0)], [], [qlb.Symbol(qlb.SEGMENT, "SYMBOL", 0, 3)],
                  (0, 0), dgroup=1, zeros=(5, 8))
    data = lib.emit()
    stored = data[512 : len(data) - len(lib.symbol_block())]
    assert stored == struct.pack("<H", 5) + image[5:16] + struct.pack("<H", 8) + image[24:]
    back = qlb.parse(data)
    assert back.image == image and back.zeros == (5, 8) and back.dgroup == 1
    assert back.emit() == data


def test_a_file_without_a_group_has_one_zero_count_not_two():
    lib = qlb.parse(MINIMAL)
    assert lib.dgroup == 0
    assert len(MINIMAL) - 512 - len(lib.symbol_block()) == 2 + len(lib.image)


def test_far_data_sizes_are_32_bit():
    lib = qlb.parse(MINIMAL)
    lib.far_data, lib.far_bss = (1, 0x12345), (1, 0x2FFFF)
    again = qlb.parse(lib.emit())
    assert again.far_data == (1, 0x12345) and again.far_bss == (1, 0x2FFFF)


@pytest.mark.parametrize("damage", [
    lambda d: b"ZM" + d[2:],
    lambda d: d[:100],
    lambda d: d.replace(b"ul", b"lu", 1),
    lambda d: d[:-1],
    lambda d: d.replace(bytes([0xEA, 4, 0, 0, 0, 2]), bytes([0xEA, 4, 0, 0, 0, 3]), 1),
])
def test_what_is_not_a_quick_library_is_refused(damage):
    with pytest.raises((qlb.QlbError, struct.error, ValueError)):
        qlb.parse(damage(MINIMAL))


def libraries():
    for env, sub in (("QB45_DIR", "LIB"), ("PDS71_DIR", "LIB"), ("VBDOS_DIR", "LIB")):
        root = os.environ.get(env)
        if root and (Path(root) / sub).is_dir():
            yield from sorted((Path(root) / sub).glob("*.QLB"))


@pytest.mark.parametrize("path", list(libraries()) or [pytest.param(None, marks=pytest.mark.skip("no QB45_DIR/PDS71_DIR/VBDOS_DIR"))])
def test_every_shipped_library_checks_and_writes_back(path):
    data = path.read_bytes()
    assert qlb.check(data) == []
    lib = qlb.parse(data)
    assert lib.emit() == data
    assert lib.code[0].flags == qlb.CODE and lib.segments[-1].name == "SYMBOL"
