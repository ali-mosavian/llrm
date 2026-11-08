"""Regression tests of the instrument: each name says what went wrong and what it produced."""
import re
import os
import json
from pathlib import Path

import pytest

import harness

BENCH = harness.BENCH
HERE = Path(__file__).parent
RESULTS = harness.OUT / "results.jsonl"   # a run of run.sh, if there was one: read before any test points OUT elsewhere


@pytest.fixture(scope="module")
def built(tmp_path_factory):
    """A work directory of its own with what these tests run: the stub and the gcc and clang objects of scroll, fib, x_ll_div and nbody."""
    import os
    import subprocess
    import sys
    sys.path.insert(0, str(harness.REPO / "tools"))
    import linkrecipe
    work = tmp_path_factory.mktemp("vsgcc")
    env = {**os.environ, "VSGCC_WORK": str(work)}
    (work / "o").mkdir()
    (work / "b").mkdir()
    done = subprocess.run(["gcc", "-m32", "-c", str(HERE / "stub.s"), "-o", str(work / "stub.o")], capture_output=True, text=True)
    assert done.returncode == 0, done.stderr
    done = subprocess.run(["ld", "-m", linkrecipe.ld_emulation("x86-m32"), "-static", "-e", "0", "-Ttext=0x8000", "-o", str(work / "stub.elf"), str(work / "stub.o")], capture_output=True, text=True)
    assert done.returncode == 0, done.stderr
    previous = harness.OUT
    harness.OUT = work
    harness.record_stub(work)
    for prog in ("scroll", "fib", "x_ll_div", "nbody", "run_mul_constants"):
        done = subprocess.run([str(HERE / "build.sh"), prog], capture_output=True, text=True, env=env)
        assert done.returncode == 0 and "FAIL" not in done.stdout, (prog, done.stdout, done.stderr)
    yield work
    harness.OUT = previous


def expected(prog):
    return [int(x) for x in (BENCH / prog / f"{prog}.out").read_text().split()]


def test_memmove_stand_in_copies_overlap_backwards(built):
    """The stub's memmove copied forwards: gcc/clang scroll (loops turned into memmove) reported 32636400, not 32634864."""
    for v in ("gccO2", "clangO2"):
        assert harness.run("scroll", v)["reports"] == expected("scroll")


def test_a_divide_gcc_sends_to_libgcc_is_counted_in_its_code(built):
    """x_ll_div read 508 B for gcc -O2 against llrm's 1,174 B (2.31x, the table's worst code row): the i64 divides gcc calls
    are libgcc's, and the count was of its object, which leaves them out; linked, it is 1,477 B and llrm is 0.79x."""
    import subprocess
    obj = subprocess.run(["size", "-A", str(harness.OUT / "b" / "x_ll_div.gccO2.o")], capture_output=True, text=True).stdout
    object_text = sum(int(l.split()[1]) for l in obj.splitlines() if l.startswith(".text"))
    assert harness.code_bytes("x_ll_div", "gccO2") > object_text + 500, object_text


def test_the_per_mnemonic_clocks_add_up_to_the_clocks_of_the_run(built):
    """--hist prints which instruction kinds the clocks go to (f64 div: 800 IMULs at 31 clocks were the whole gap to libgcc); a table that
    drops or double-counts an instruction would point at the wrong one, so its sum must be the run's clocks and its counts the instructions."""
    r = harness.run("fib", "gccO2")
    assert sum(clk for _, clk in r["by_op"].values()) + r["misaligned"] * harness.MISALIGNED == r["clocks"]
    assert sum(count for count, _ in r["by_op"].values()) == r["ins"]


def test_a_tests_run_c_cell_yields_clocks(built):
    """tests/run/c cells (mul_constants, div_constants_short, ...) have only `main`, so the harness found no kernel and the table had no
    clocks for them; `run_NAME` now counts the cell's `main`, and its reports are the cell's `.out`."""
    for v in ("llrm", "llrmOs", "gccO2", "clangO2"):
        r = harness.run("run_mul_constants", v)
        assert r["clocks"] > 0 and r["ins"] > 0, r
        assert r["reports"] == [int(x) for x in (harness.REPO / "tests/run/c/mul_constants.out").read_text().split()]


def test_kernel_inlined_into_main_is_not_counted_as_zero(built):
    """gcc inlined bench_fib into main: the region was never entered and fib read 0 instructions for gcc."""
    assert harness.run("fib", "gccO2")["ins"] > 0


@pytest.mark.skipif(not RESULTS.exists(), reason="reads the results of a run: run crates/target/llrm-x86-m32/vsgcc/run.sh first, or it checks nothing")
def test_every_result_reproduces_the_benchmark_output():
    for line in RESULTS.read_text().splitlines():
        r = json.loads(line)
        assert r["ok"], (r["prog"], r["variant"], r["got"])


def test_build_script_flags_are_accepted_by_llrm_cc():
    """build.sh/ctime.py passed a flag the compiler refused: every llrm build failed with exit 2 and run.sh printed no tables."""
    import subprocess
    if not harness.LLRM.exists():
        pytest.skip(f"no llrm-cc at {harness.LLRM}")
    out = subprocess.run([str(harness.LLRM), *harness.llrmbin.c_args(32), "-O2", "-fno-inline-functions", "-c", "-o", "/dev/null", str(BENCH / "sieve/sieve.c")], capture_output=True, text=True, env=harness.cc_env())
    assert out.returncode == 0, out.stderr
    here = Path(__file__).parent


def test_a_multiply_costs_what_its_multiplier_is():
    """The 486's MUL and IMUL were priced at their least, 13 clocks, whatever the multiplier: gcc's reciprocal `mul` by
    0xCCCCCCCD (42 clocks by the data sheet) read as cheap, and frames' `/ 10` as 2.2x for llrm's `div`."""
    assert harness.multiply_clocks(0, False) == 13
    assert harness.multiply_clocks(5, True) == 13
    assert harness.multiply_clocks(0xFFFF, False) == 26
    assert harness.multiply_clocks(25173, True) == 10 + 15
    assert harness.multiply_clocks(0xCCCCCCCD, False) == 42
    assert harness.multiply_clocks(0xFFFFFFFF, True) == 15      # -1: n = 5 for a negative multiplier


def test_llrm_is_built_in_the_convention_gcc_uses_and_its_report_is_read_from_the_stack(built):
    """The table put llrm's watcom register arguments beside gcc's stack ones: part of every ratio was the ABI. build.sh passes
    -mabi=sysv and records it; the harness reads `report`'s value where that convention puts it."""
    assert harness.built_abi() == "sysv"
    result = harness.run("fib", "llrm")
    assert result["reports"] == expected("fib"), result["reports"]
    listing = subprocess_text([str(harness.LLRM), *harness.llrmbin.c_args(32), "-mabi=sysv", "-O2", "-S", "-o", "/dev/stdout", str(BENCH / "fib/fib.c")])
    assert "_fib proc" in listing and "fib_ proc" not in listing, "sysv names are undecorated"


def subprocess_text(argv):
    import subprocess
    done = subprocess.run(argv, capture_output=True, text=True, env=harness.cc_env())
    assert done.returncode == 0, done.stderr
    return done.stdout


def test_enter_costs_what_the_486_charges_for_it():
    """`enter` fell to the default one clock: a frame opened with it read 3 clocks cheaper than push/mov/sub's 3 when it is
    14 (Intel 240440-002), and the -Os rows that used it looked as fast as they were small."""
    from iced_x86 import Decoder
    enter = next(iter(Decoder(32, bytes([0xC8, 0x08, 0x00, 0x00]))))
    assert harness.cost(enter, True, True, None) == 14


def test_every_x_kernel_has_its_source_and_its_self_check():
    """The kernels live beside the harness, not in bench/: a lookup in bench/ alone found neither file."""
    names = sorted(p.name for p in harness.KERNELS.iterdir())
    assert len(names) == 66 and all(n.startswith("x_") for n in names)
    for n in names:
        assert (harness.source_dir(n) / f"{n}.c").is_file() and harness.source_dir(n) == harness.KERNELS / n
        assert harness.source_dir(n).joinpath(f"{n}.out").read_text().split()


def test_an_integer_memory_x87_operand_is_not_free_and_an_unpriced_x87_instruction_is_refused():
    """gcc's `fimul dword ptr [esp]` (x_matmul_f) fell through the clock model to 1 clock, where the 486 takes
    22 to 24: llrm read 13% slower than gcc on that kernel, most of it the instrument. An x87 instruction the model
    does not price is an error, not a clock."""
    from iced_x86 import Decoder

    def clocks(*code):
        return harness.cost(Decoder(32, bytes(code)).decode(), False, True, 1)

    assert clocks(0xDA, 0x0C, 0x24) == 22     # fimul dword ptr [esp]
    assert clocks(0xDE, 0x0C, 0x24) == 23     # fimul word ptr [esp]
    assert clocks(0xDA, 0x04, 0x24) == 19     # fiadd dword ptr [esp]
    assert clocks(0xDE, 0x34, 0x24) == 85     # fidiv word ptr [esp]

def test_run_sh_builds_the_programs_programs_py_lists_and_no_file():
    """`run.sh` listed `ls bench` and tried `bench_io.h` as a program: wrap.py died on `bench_io.h/bench_io.h.c`, the table never ran."""
    import subprocess
    import sys
    listed = subprocess.run([sys.executable, str(HERE / "programs.py")], capture_output=True, text=True, check=True).stdout.split()
    assert listed and all((harness.BENCH / name).is_dir() or (HERE / "kernels" / name).is_dir() for name in listed), listed
    assert "bench_io.h" not in listed and "programs.py" in (HERE / "run.sh").read_text()


def test_ctime_times_the_programs_programs_py_lists_and_no_file():
    """`ctime.py` listed `ls bench` and timed `bench_io.h` as a program: the run stopped before writing ctime.json."""
    import importlib
    import ctime
    importlib.reload(ctime)
    names = sorted(ctime.programs.sources())
    assert names and "bench_io.h" not in names and all(ctime.cmds(n) for n in names), names


def test_levels_time_gives_llrm_cc_its_tree_and_skips_programs_gcc_cannot_build(monkeypatch):
    """`levels_time.py` handed gcc bench/jpeg_fixed, which needs bench_io.h."""
    import levels_time
    levels_time.setup()
    names = levels_time.bench_sources()
    assert "bintree" in names and "jpeg_fixed" not in names, sorted(names)


def test_every_x87_encoding_has_a_clock_price():
    """#1659 made an unpriced x87 instruction an error, and `frndint` (Open Watcom's float-to-int, in floats and fpbench) was
    one: tools/bench died on those programs and main's gate failed. Every encoding of D8-DF, 16- and 32-bit, is priced."""
    from iced_x86 import Code, Decoder

    for bits in (16, 32):
        for opcode in range(0xD8, 0xE0):
            for modrm in range(256):
                ins = Decoder(bits, bytes([opcode, modrm, 0, 0, 0, 0, 0, 0])).decode()
                if ins.code != Code.INVALID:
                    harness.cost(ins, False, True, 1)


def test_a_dword_at_an_odd_address_is_counted_as_misaligned():
    """The 486 pays three clocks for a misaligned access and the table showed nothing of it: a loop of `mov eax, [ebx+1]` and one of
    `mov eax, [ebx]` cost the same in instructions, memory operands and clocks. A double wants 8 (Table 2.1)."""
    from iced_x86 import Decoder
    from unicorn import UC_ARCH_X86, UC_MODE_32, Uc
    from unicorn.x86_const import UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI

    uc = Uc(UC_ARCH_X86, UC_MODE_32)
    uc.reg_write(UC_X86_REG_EBX, 0x1000)
    uc.reg_write(UC_X86_REG_ESI, 0x2001)
    uc.reg_write(UC_X86_REG_EDI, 0x3000)

    def count(code):
        return harness.misaligned_operands(uc, Decoder(32, code, ip=0).decode())

    assert count(bytes.fromhex("8b4301")) == 1      # mov eax, [ebx+1]
    assert count(bytes.fromhex("8b03")) == 0        # mov eax, [ebx]
    assert count(bytes.fromhex("0fb74301")) == 1    # movzx eax, word [ebx+1]
    assert count(bytes.fromhex("0fb74302")) == 0    # movzx eax, word [ebx+2]
    assert count(bytes.fromhex("8a4301")) == 0      # mov al, [ebx+1]: a byte is never across
    assert count(bytes.fromhex("a5")) == 1          # movsd: the source esi is odd, the destination edi is not
    assert count(bytes.fromhex("dd4304")) == 1      # fld qword [ebx+4]: a double wants 8
    assert count(bytes.fromhex("dd4308")) == 0      # fld qword [ebx+8]


def test_nbody_executes_no_misaligned_operand_at_any_level(built):
    """main was entered with esp 4 off the System V boundary, so llrm's aligned frames (it keeps the call boundary, as gcc and clang do)
    stood at 4 mod 8: nbody -O3 read 14,616 misaligned operands, +43,848 clocks (354,259 -> 398,107, +12.4%), a regression no code change made.
    With the entry right, a hole of 12 bytes for spills moved the arrays of -O1/-O2/-Os to 4 mod 8: 18,404 operands, +55,212 clocks."""
    for variant in ("llrmO1", "llrm", "llrmO3", "llrmOs"):
        assert harness.run("nbody", variant)["misaligned"] == 0, variant


def test_code_bytes_counts_text_only_not_the_unwind_tables_the_pin_emits(tmp_path, monkeypatch):
    """The pin's llc object carries .eh_frame (56 B on x_switch) and ours none: summing sections read 419 against ours 379, where the code was 363 against 379 (w12)."""
    import subprocess
    (tmp_path / "b").mkdir()
    (tmp_path / "t.s").write_text(".text\n.fill 100,1,0x90\n.section .eh_frame,\"a\"\n.fill 56,1,0\n.data\n.long 1\n")
    done = subprocess.run(["gcc", "-m32", "-c", str(tmp_path / "t.s"), "-o", str(tmp_path / "b" / "p.pinllc.o")], capture_output=True, text=True)
    assert done.returncode == 0, done.stderr
    monkeypatch.setattr(harness, "OUT", tmp_path)
    assert harness.code_bytes("p", "pinllc") == 100


def test_data_bytes_counts_the_constants_gcc_keeps_in_rodata_not_only_text(tmp_path, monkeypatch):
    """The m32 table's bytes were .text only: gcc's 12 B of .rodata.cst4 in fpbench was invisible and llrm's constants in _DATA likewise, though the order was .text plus data."""
    import subprocess
    (tmp_path / "b").mkdir()
    (tmp_path / "t.s").write_text(".text\n.fill 100,1,0x90\n.section .rodata.cst4,\"aM\",@progbits,4\n.long 1,2,3\n.data\n.long 1\n.bss\n.fill 64\n")
    done = subprocess.run(["gcc", "-m32", "-c", str(tmp_path / "t.s"), "-o", str(tmp_path / "b" / "p.gccO2.o")], capture_output=True, text=True)
    assert done.returncode == 0, done.stderr
    monkeypatch.setattr(harness, "OUT", tmp_path)
    assert harness.data_bytes("p", "gccO2") == 16


def test_executed_alignment_nops_cost_their_clocks(tmp_path, monkeypatch):
    """The harness counted an executed alignment `nop` apart and charged it nothing: x_matmul_f -O3 ran 1,604 of them (two builds, one
    with the loop pad and one without, both read 67,696 clocks) while gcc, which pads with `lea esi,[esi]`, paid for its pad.
    A 3-trip loop with two `nop`s a trip costs 6 `nop` clocks more than the same loop without, and runs the same instructions."""
    import subprocess
    import sys
    sys.path.insert(0, str(harness.REPO / "tools"))
    import linkrecipe
    (tmp_path / "o").mkdir()
    (tmp_path / "b").mkdir()
    done = subprocess.run(["gcc", "-m32", "-c", str(HERE / "stub.s"), "-o", str(tmp_path / "stub.o")], capture_output=True, text=True)
    assert done.returncode == 0, done.stderr
    done = subprocess.run(["ld", "-m", linkrecipe.ld_emulation("x86-m32"), "-static", "-e", "0", "-Ttext=0x8000", "-o", str(tmp_path / "stub.elf"), str(tmp_path / "stub.o")], capture_output=True, text=True)
    assert done.returncode == 0, done.stderr
    monkeypatch.setattr(harness, "OUT", tmp_path)
    harness.record_stub(tmp_path)

    def run(prog, pad):
        (tmp_path / f"{prog}.s").write_text(
            f".text\n.globl main, bench_{prog}\nmain:\n call bench_{prog}\n ret\nbench_{prog}:\n mov $3,%ecx\n1:\n{pad} dec %ecx\n jnz 1b\n ret\n"
        )
        done = subprocess.run(["gcc", "-m32", "-c", str(tmp_path / f"{prog}.s"), "-o", str(tmp_path / f"{prog}.o")], capture_output=True, text=True)
        assert done.returncode == 0, done.stderr
        done = subprocess.run(["ld", "-m", linkrecipe.ld_emulation("x86-m32"), "-static", "-e", "0", "-Ttext=0x20000", "-o", str(tmp_path / "b" / f"{prog}.gccO2.elf"), str(tmp_path / f"{prog}.o")], capture_output=True, text=True)
        assert done.returncode == 0, done.stderr
        return harness.run(prog, "gccO2")

    plain, padded = run("plain", ""), run("padded", " nop\n nop\n")
    assert (plain["nops"], padded["nops"]) == (0, 6)
    assert padded["ins"] == plain["ins"]
    assert padded["clocks"] - plain["clocks"] == 6 * harness.clocks.clocks("nop")


def test_code_bytes_leaves_out_alignment_padding(tmp_path, monkeypatch):
    """The leg counted whole .text: a loop gcc aligned with nops read as growth (the judge's code_bytes leaves padding out, so the two disagreed).
    6 bytes of code, `.p2align 4`, 1 byte of code: 17 bytes in the section, 7 of code and 10 of padding."""
    import subprocess
    (tmp_path / "b").mkdir()
    (tmp_path / "t.s").write_text(".text\nmovl $1,%eax\nret\n.p2align 4\nret\n")
    done = subprocess.run(["gcc", "-m32", "-c", str(tmp_path / "t.s"), "-o", str(tmp_path / "b" / "p.gccO2.o")], capture_output=True, text=True)
    assert done.returncode == 0, done.stderr
    monkeypatch.setattr(harness, "OUT", tmp_path)
    monkeypatch.setattr(harness, "archive_code", lambda prog, variant: 0)
    assert harness.code_size("p", "gccO2") == (7, 10)
    assert harness.code_bytes("p", "gccO2") == 7


def build_ld(tmp_path, width, stub, **env):
    """build.sh of x_ldmath into a fresh work directory at `width`; the finished process."""
    import subprocess
    work = tmp_path / f"ld{width}"
    (work / "o").mkdir(parents=True)
    (work / "b").mkdir()
    for name in ("stub.o", "stub.elf", "stub.elf.src"):
        (work / name).write_bytes((stub / name).read_bytes())
    return work, subprocess.run([str(HERE / "build.sh"), "x_ldmath"], capture_output=True, text=True,
                                env={**os.environ, "VSGCC_WORK": str(work), "VSGCC_LD": str(width), "VSGCC_ONLY": "llrm", **env} if env else {**os.environ, "VSGCC_WORK": str(work), "VSGCC_LD": str(width)})


def bss_bytes(work, compiler="llrmElfO2"):
    """The .bss of x_ldmath's ELF object of `compiler`: its three static arrays of 48 long doubles."""
    import subprocess
    out = subprocess.run(["size", "-A", str(work / "b" / f"x_ldmath.{compiler}.o")], capture_output=True, text=True).stdout
    return next(int(l.split()[1]) for l in out.splitlines() if l.startswith(".bss"))


def test_long_double_is_one_width_in_llrm_and_the_reference(tmp_path, built):
    """x_ldmath, x_ldconv and x_ldcmp compared llrm's 8-byte long double with gcc's 12: the rows were doubles against 80-bit code."""
    for width, element in ((80, 12), (64, 8)):
        work, done = build_ld(tmp_path, width, built)
        assert done.returncode == 0 and "FAIL" not in done.stdout, (width, done.stdout, done.stderr)
        assert (work / "ld").read_text() == str(width)
        assert re.fullmatch(r"[0-9a-f]{12}\+[0-9a-f]{12}", (work / "o" / "x_ldmath.cc").read_text()), "build.sh stamps the llrm-cc and llrm-llc it used"
        assert bss_bytes(work) == bss_bytes(work, "gccO2") == 3 * 48 * element, (width, bss_bytes(work), bss_bytes(work, "gccO2"))


def test_a_build_whose_long_double_is_not_the_reference_s_is_refused(tmp_path, built):
    work, done = build_ld(tmp_path, 80, built, VSGCC_LLRM_LD="-mlong-double-64")
    assert done.returncode != 0 and "FAIL long double: llrm is 8 bytes, the reference 12 (80 bit)" in done.stdout, (done.stdout, done.stderr)
    assert build_ld(tmp_path, 70, built)[1].returncode == 2


def test_novec_build_turns_gccs_vectorizer_off(built, tmp_path):
    """gcc 13 vectorizes at -O3 even for i486 (nbody_fixed O3 1,514 B/4.60M clocks vs 1,960 B/4.63M scalar): the table had no scalar column.
    VSGCC_NOVEC=1 builds gcc with -fno-tree-vectorize -fno-tree-slp-vectorize: its -O3 object changes, clang's does not."""
    import subprocess
    work = tmp_path / "novec"
    (work / "o").mkdir(parents=True)
    (work / "b").mkdir()
    for name in ("stub.o", "stub.elf", "stub.elf.src"):
        (work / name).write_bytes((built / name).read_bytes())
    done = subprocess.run([str(HERE / "build.sh"), "nbody"], capture_output=True, text=True,
                          env={**os.environ, "VSGCC_WORK": str(work), "VSGCC_NOVEC": "1"})
    assert done.returncode == 0 and "FAIL" not in done.stdout, (done.stdout, done.stderr)
    obj = lambda w, c: (w / "b" / f"nbody.{c}O3.o").read_bytes()
    assert obj(work, "gcc") != obj(built, "gcc")
    assert obj(work, "clang") == obj(built, "clang")
