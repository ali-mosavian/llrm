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


def test_sf_z80_harness_sources_exist():
    """build.sh read runtime/softfloat/src/sf32.c after a286a52c00 moved it to compiler-rt: the harness died with
    'No such file' on release/next and every softfloat gcc cell was unmeasurable."""
    root = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..")
    d = os.path.join(root, "tools", "gcc-columns", "sf-z80")
    for n in ("build.sh", "run.sh"):
        for rel in re.findall(r"\.\./\.\./\.\./(runtime/[A-Za-z0-9_/.-]+)", open(os.path.join(d, n)).read()):
            assert os.path.exists(os.path.join(root, rel)), f"{n}: {rel} does not exist"
    names = re.search(r'F="([^"]+)"', open(os.path.join(d, "build.sh")).read()).group(1).split()
    for f in names:
        assert os.path.exists(os.path.join(root, "runtime/compiler-rt/lib/fp/f32", f + ".c")), f


def test_host_check_catches_a_wrong_expect():
    """The kernel expects were hand-derived; host_check must flag one that is off by one, and pass the shipped list."""
    import host_check as H
    assert H.main() == 0
    saved = Z.K[:]
    try:
        n, r, a, w = Z.K[-1]
        Z.K[-1] = (n, r, a, w + 1)
        assert H.main() == 1
    finally:
        Z.K[:] = saved
