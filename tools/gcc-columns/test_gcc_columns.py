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


def test_a_hung_fork_call_is_a_failed_call_not_a_cell():
    """compiler-rt f64/mul.c O1 hangs the native-long cc1 (>300 s): every cc1/tool call of both benches runs under a timeout and a timeout
    returns 124, which the benches count as a failed compile (n/a); a missing or zero-byte cell would read as a number."""
    import time
    import gcc_6502_bench as B6
    os.environ["GCC_COLUMNS_TIMEOUT"] = "1"
    try:
        for mod in (Z, B6):
            mod.TIMEOUT = 1
            t0 = time.time()
            p = mod.sh(["sleep", "5"], ".")
            assert p.returncode == 124 and time.time() - t0 < 4, (mod.__name__, p.returncode)
    finally:
        del os.environ["GCC_COLUMNS_TIMEOUT"]
        Z.TIMEOUT = B6.TIMEOUT = 120


def test_gcc_leads_takes_the_lower_of_the_two_columns():
    """The bar is the lower gcc column per metric: a lead by one column only (memmove clk by gcc, B by noivopts) must show both."""
    import gcc_leads as L
    h = "target\tprog\tlevel\tcompiler\tcode_bytes\tclocks\tinstrs\tnote\n"
    def tsv(name, body):
        f = tempfile.NamedTemporaryFile("w", suffix=".tsv", delete=False)
        f.write(h + body)
        f.close()
        return f.name
    m = tsv("m", "6502\tp\tOs\tours\t100\t1000\t300\t\n")
    a = tsv("a", "6502\tp\tOs\tgcc\t90\t1200\t310\t\n")
    b = tsv("b", "6502\tp\tOs\tgcc-noivopts-Os\t110\t900\t320\t\n")
    (r,) = L.leads(m, [a, b])
    assert r[1] == ("p", "Os") and [d[:3] for d in r[2]] == [("B", 90, 100), ("clk", 900, 1000)]
    assert [(x[1], [d[:3] for d in x[2]]) for x in L.leads(m, [a])] == [(("p", "Os"), [("B", 90, 100)])]
