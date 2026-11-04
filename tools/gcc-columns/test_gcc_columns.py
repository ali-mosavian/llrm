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


def test_softfloat_z80_table_is_complete_and_current():
    """The 11-03 table was measured from the removed monolithic sf32.c; it stayed in the repo marked STALE.
    A table needs 32 gcc clk cells (2 variants x 2 levels x 8 ops), 4 library sizes and the cc1 md5, and no STALE mark."""
    t = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "tables", "softfloat-z80.tsv")).read()
    head = [l for l in t.splitlines() if l.startswith("#")]
    assert not any("STALE" in l for l in head), "stale table"
    assert re.search(r"cc1 md5 [0-9a-f]{32}", head[0]) and "runtime/compiler-rt" in head[0]
    rows = [l.split("\t") for l in t.splitlines() if l.startswith("gcc\t")]
    assert sum(r[3] == "clk" for r in rows) == 32 and sum(r[3] == "bytes" for r in rows) == 4


def test_z80_16bit_result_is_charged_the_de_store():
    """gcc's driver stored a 16-bit result with ld (nn),hl (16 T), the DE columns with ld (nn),de (20 T): every 16-bit gcc
    cell was 4 T optimistic (add16 read 78, by hand 82). 16-bit = ex de,hl 4 T + store 4 T; 8-bit in A for both."""
    assert Z.result_charge(Z.U16) == (1, 8, 1)
    assert Z.result_charge("int") == (1, 8, 1)
    assert Z.result_charge(Z.U8) == (0, 0, 0)


def test_z80_table_header_names_the_single_tu_fix():
    """A single-TU softfloat build ICEd the z80 cc1 at -O2 (LRA passes) until sp += n went through IX/IY; the softfloat header says which cc1 compiles it."""
    root = os.path.join(os.path.dirname(os.path.abspath(__file__)), "tables", "softfloat-z80.tsv")
    head = open(root).readline()
    assert "compiles at -O2 since the sp += n fix" in head and "KNOWN ISSUE" not in head


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


def test_vs_6502_bar_is_the_lower_of_both_gcc_columns():
    """vs_6502.py read only 6502-gcc.tsv: a cell the gcc-noivopts-Os column wins (memchr, minmax, sum16 -Os) showed the worse number."""
    import subprocess
    d = tempfile.mkdtemp()
    h = "target\tprog\tlevel\tcompiler\tcode_bytes\tclocks\tinstrs\tnote\n"
    open(f"{d}/6502.tsv", "w").write(h + "6502\tp\tOs\tours\t120\t1500\t300\t\n")
    open(f"{d}/6502-gcc.tsv", "w").write(h + "6502\tp\tOs\tgcc\t100\t1200\t310\t\n")
    open(f"{d}/6502-gcc.noivopts.tsv", "w").write(h + "6502\tp\tOs\tgcc-noivopts-Os\t110\t900\t320\t\n")
    out = f"{d}/vs.md"
    subprocess.run([sys.executable, os.path.join(os.path.dirname(os.path.abspath(__file__)), "vs_6502.py"), d, out, "2026-11-04"], check=True)
    row = next(l for l in open(out) if l.startswith("| p |"))
    assert row.split("|")[2:4] == [" 100 ", " 900 "], row


def test_vs_6502_names_the_cc1_of_the_tables_it_read():
    """vs_6502.py printed the md5s 4c91eb80/75c808c2 as literals: after a cc1 swap the card named a compiler the cells did not come from."""
    import subprocess
    d = tempfile.mkdtemp()
    h = "target\tprog\tlevel\tcompiler\tcode_bytes\tclocks\tinstrs\tnote\n"
    open(f"{d}/6502.tsv", "w").write(h + "6502\tp\tOs\tours\t120\t1500\t300\t\n")
    open(f"{d}/6502-gcc.tsv", "w").write("# x, cc1 md5 aaaaaaaa11111111\n" + h + "6502\tp\tOs\tgcc\t100\t1200\t310\t\n")
    open(f"{d}/6502-gcc.noivopts.tsv", "w").write("# x, cc1 md5 bbbbbbbb22222222\n" + h + "6502\tp\tOs\tgcc-noivopts-Os\t110\t900\t320\t\n")
    out = f"{d}/vs.md"
    subprocess.run([sys.executable, os.path.join(os.path.dirname(os.path.abspath(__file__)), "vs_6502.py"), d, out, "2026-11-04"], check=True)
    t = open(out).read()
    assert "md5 aaaaaaaa gcc" in t and "bbbbbbbb gcc-noivopts-Os" in t and "4c91eb80" not in t, t[:400]


def test_softfloat_6502_clocks_divide_by_the_records_run():
    """The 11-03 6502 softfloat table divided by 256 although add/sub/mul/div ran 361 records, and subtracted a base run that
    failed its records: hand add read 570 for 324. Records come from the vec.inc that ran, and the base run must pass."""
    d = os.path.join(os.path.dirname(os.path.abspath(__file__)), "sf-6502")
    for n in ("run.sh", "hand.sh", "../sf-z80/run.sh"):
        t = open(os.path.join(d, n)).read()
        assert "/nv" in t and "/256" not in t and not re.search(r"/\$N\b", t), n
        assert "base run must pass" in t, n
    t = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "tables", "softfloat-6502.tsv")).read()
    head = [l for l in t.splitlines() if l.startswith("#")]
    assert len(head) == 1 and "withdrawn" not in head[0] and "STALE" not in head[0] and re.search(r"cc1 md5 [0-9a-f]{32}", head[0])
    rows = [l.split("\t") for l in t.splitlines() if l.startswith("gcc\t")]
    assert sum(r[3] == "clk" for r in rows) == 32 and sum(r[3] == "bytes" for r in rows) == 32
    hand_add = next(l.split("\t")[5] for l in t.splitlines() if l.startswith("hand\t-\t-\tclk\tadd\t"))
    assert int(hand_add) < 400, hand_add


def test_z80_tables_name_the_same_cc1():
    """The kernel and softfloat z80 tables are produced by one cc1: a swap that changes one header and not the other mixes compilers."""
    t = os.path.join(os.path.dirname(os.path.abspath(__file__)), "tables")
    md5s = {re.search(r"cc1 md5 ([0-9a-f]{32})", open(os.path.join(t, n)).readline()).group(1) for n in ("z80-gcc.tsv", "softfloat-z80.tsv")}
    assert len(md5s) == 1, md5s


def test_ice_corpus_tells_an_ice_from_a_source_error_and_a_hang():
    """A cc1 swap must not lose a file the old cc1 compiles: an ICE and a timeout count against the new cc1, a missing header does not."""
    import ice_corpus as C
    assert C.classify(1, "x.c:3: internal compiler error: maximum number of LRA assignment passes") == "ice"
    assert C.classify(1, "error: unable to find a register to spill") == "ice"
    assert C.classify(1, "fatal error: stdint.h: No such file or directory") == "error"
    assert C.classify(0, "") == "ok" and C.classify(1, "", True) == "timeout"


def test_z80_mul16_o2_uses_the_early_exit_helper_and_it_is_exact():
    """The shipped __mulhi3 always ran 16 iterations: mul16 -O2 read 1016 T where ours (early exit) reads 481. O2 now links
    rt-z80-speed/__mulhi3 (28 B, 508 T); O1/Os keep the small one. The helper must also be exact on edge operands."""
    here = os.path.dirname(os.path.abspath(__file__))
    rows = {}
    for l in open(os.path.join(here, "tables", "z80-gcc.tsv")):
        f = l.rstrip("\n").split("\t")
        if len(f) > 7 and f[1] == "mul16":
            rows[f[2]] = (int(f[5]), f[7])
    assert rows["O2"][0] < 600 and "speed helper" in rows["O2"][1], rows
    assert rows["O1"][0] > 1000 and rows["Os"][0] > 1000 and "speed helper" not in rows["Os"][1], rows
    import shutil
    if not (shutil.which("sdasz80") and shutil.which("ucsim_z80")):
        return
    w = tempfile.mkdtemp(prefix="mulhi-")
    assert Z.sh(["sdasz80", "-o", "m.rel", os.path.join(here, "rt-z80-speed", "__mulhi3.s")], w).returncode == 0
    for a, b in ((0, 0), (0, 7), (7, 0), (1, 1), (1234, 56), (56, 1234), (0xFFFF, 0xFFFF), (0x8000, 2), (2, 0x8000), (0xFFFF, 1), (255, 255), (0x1234, 0x5678)):
        open(f"{w}/h.s", "w").write(f"\t.globl ___mulhi3\n\t.area _CODE\n_main::\n\tld sp,#0\n\tld hl,#{a}\n\tld de,#{b}\n\tcall ___mulhi3\n\tld (0x{Z.R:x}),hl\n\thalt\n")
        open(f"{w}/c.s", "w").write(Z.CRT0)
        for s in ("h", "c"):
            assert Z.sh(["sdasz80", "-o", f"{s}.rel", f"{s}.s"], w).returncode == 0
        p = Z.sh(["sdldz80", "-i", "-b", "_HEADER=0", "-b", "_CODE=0x100", "-b", "_DATA=0x8000", "x", "c.rel", "h.rel", "m.rel"], w)
        assert p.returncode == 0, p.stderr[-200:]
        got = Z.run("x.ihx", w)[2]
        assert got == a * b & 0xFFFF, (a, b, got)


def test_6502_runtime_has_the_indirect_call_helper():
    """cc65's basic.c (a dispatch table) did not compile with the 6502 cc1: a call through a pointer was an ICE. The cc1 now stores the
    pointer in __icv and does jsr __icall; both must be defined by the runtime the bench links, or the cell is a link failure."""
    text = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "rt-6502", "icall.s")).read()
    assert re.search(r"\.export\s+__icall\s*,\s*__icv", text) and "__icall:" in text and "jmp (__icv)" in text


def test_z80_runtime_has_the_iy_indirect_call_helper():
    """The z80 cc1 calls through a pointer as `call ___call_iy` (hl carries the first word argument; a call through hl looped in LRA on
    static_pointers). Without rt-z80/__call_iy.s the cell is a link failure, and a hand-helper cell must be noted as such."""
    here = os.path.dirname(os.path.abspath(__file__))
    text = open(os.path.join(here, "rt-z80", "__call_iy.s")).read()
    assert ".globl\t___call_iy" in text and "___call_iy:\n\tjp\t(iy)" in text
    assert "call_iy" in open(os.path.join(here, "gcc_z80_bench.py")).read()


if __name__ == "__main__":
    # every test_*: a hand-kept list ran 4 of 17 and let "ok" stand for all of them
    tests = [f for n, f in sorted(globals().items()) if n.startswith("test_")]
    for t in tests:
        t()
    print(f"ok ({len(tests)} tests)")

