"""Regression: a gcc cell must count the lookup table the kernel carries.

The Z80 crc16 -O2 cell read 120 B while its listing held a 256-entry .dw table in _DATA (512 B uncounted),
so the column claimed a 120 B / 5126 T win over the 77 B loop.
"""
import os, re, sys, tempfile
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gcc_z80_bench as Z


def rel(text):
    f = tempfile.NamedTemporaryFile("w", suffix=".rel", delete=False)
    f.write(text)
    f.close()
    return f.name


def test_table_in_data_is_counted():
    r = rel("A _CODE size 78 flags 0 addr 0\nA _DATA size 200 flags 0 addr 0\n")
    assert Z.code_size(r) == 0x78 and Z.data_size(r) == 0x200


def test_driver_style_object_without_data_counts_zero():
    assert Z.data_size(rel("A _CODE size 5 flags 0 addr 0\n")) == 0



def test_header_names_no_local_path():
    """The z80 table header carried the cc1 path, which the landing runner refuses."""
    import gcc_6502_bench as S
    f = tempfile.NamedTemporaryFile(delete=False, suffix="-cc1"); f.write(b"x"); f.close()
    for m in (Z, S):
        line = m.cc1_line(f.name)
        assert f.name not in line and "/" not in line and line.startswith("cc1 md5 ")


def test_z80_kernels_cover_the_shared_source():
    """The z80 column listed 23 of the 30 kernels in kernels.c; lz4, lz4c, xs8, minmax, fletcher, hash16, dotx had no gcc cell."""
    src = os.path.join(os.path.dirname(__file__), "..", "..", "crates/target/llrm-z80/bench/kernels.c")
    in_source = set(re.findall(r"K_([a-z0-9_]+)", open(src).read()))
    assert in_source == {k[0] for k in Z.K}


if __name__ == "__main__":
    test_z80_kernels_cover_the_shared_source()
    test_header_names_no_local_path()
    test_table_in_data_is_counted()
    test_driver_style_object_without_data_counts_zero()
    print("ok")


def test_noivopts_flag_only_at_os():
    """gcc-noivopts-Os must not switch ivopts off at -O1/-O2: lz4 -O2 went 28 to 46 k clocks without it."""
    import gcc_6502_bench as B
    assert B.level_flags("Os", True) == ["-Os", "-fno-ivopts"]
    assert B.level_flags("O2", True) == ["-O2"]
    assert B.level_flags("Os", False) == ["-Os"]


def test_z80_16bit_result_is_charged_the_de_store():
    """gcc's driver stored a 16-bit result with ld (nn),hl (16 T), the DE columns with ld (nn),de (20 T): every 16-bit gcc
    cell was 4 T optimistic (add16 read 78, by hand 82). 16-bit = ex de,hl 4 T + store 4 T; 8-bit in A for both."""
    assert Z.result_charge(Z.U16) == (1, 8, 1)
    assert Z.result_charge("int") == (1, 8, 1)
    assert Z.result_charge(Z.U8) == (0, 0, 0)


def test_z80_table_header_names_the_single_tu_ice():
    """The shipped z80 cc1 ICEs on a single-TU softfloat build at -O2; the table says so, so no gcc cell is added for such a build."""
    root = os.path.join(os.path.dirname(os.path.abspath(__file__)), "tables", "z80-gcc.tsv")
    assert "single-translation-unit softfloat build" in open(root).readline()
